use std::collections::HashMap;

use crate::auth::AuthDb;
use crate::config::{ChannelCfg, Config};
use crate::files::Files;
use crate::packages::PackagesDb;

use crypto::hash::Hasher;
use crypto::signature::Keypair;

use packages::error::Error;
use packages::packages::{BoardArch, Channel, Package, TargetArch};
use packages::requests::{
	AuthenticateWriter1Req, AuthenticateWriter2Req, ChangeWhitelistReq,
	DeviceId, GetFileBuilder, GetFilePartReq, GetFileReq, PackageInfoReq,
	SetFileReq, SetPackageInfoReq, WhitelistChange,
};
use packages::server::{Server, TestingServer};
use semver::VersionReq;

struct SignKey(Keypair);

fn init() -> TestingServer {
	let mut config = Config::default();
	let sign_key = SignKey(Keypair::new());
	config.debug = Some(ChannelCfg {
		sign_key: sign_key.0.public().clone(),
	});
	assert!(config.has_sign_key());

	let pack_db = PackagesDb::new(&config);

	let files = Files::new_temp();

	let auth_db = AuthDb::new(&config);

	let priv_key = Keypair::new();

	let mut server = Server::new_testing(priv_key);

	server.register_data(pack_db);
	server.register_data(files);
	server.register_data(config);
	server.register_data(auth_db);
	server.register_data(sign_key);

	crate::server::register_requests(&mut server);

	server.build()
}

// Authentication stuff
async fn auth_as_writer(server: &TestingServer) {
	let resp = server
		.request(AuthenticateWriter1Req {
			channel: Channel::Debug,
		})
		.await
		.unwrap();

	let signature = server.get_data::<SignKey>().0.sign(resp.challenge);

	let _ = server
		.request(AuthenticateWriter2Req { signature })
		.await
		.unwrap();
}

async fn add_package_with_ctn_and_requirements(
	name: &str,
	server: &TestingServer,
	version_str: &str,
	ctn: &[u8],
	requirements: HashMap<String, VersionReq>,
) -> Package {
	let hash = Hasher::hash(ctn);
	let signature = server.get_data::<SignKey>().0.sign(&hash);

	let _ = server
		.request(SetFileReq::from_bytes(signature.clone(), ctn))
		.await
		.unwrap();

	let package = Package {
		name: name.into(),
		version_str: version_str.into(),
		version: hash,
		signature,
		arch: TargetArch::Amd64,
		binary: None,
	};

	let _ = server
		.request(SetPackageInfoReq {
			package: package.clone(),
			requirements,
			whitelist: Default::default(),
			auto_whitelist_limit: 0,
		})
		.await;

	package
}

async fn add_test_package_with_ctn(
	server: &TestingServer,
	version_str: &str,
	ctn: &[u8],
) -> Package {
	add_package_with_ctn_and_requirements(
		"test",
		server,
		version_str,
		ctn,
		HashMap::new(),
	)
	.await
}

const TEST_FILE_CONTENT: &[u8] = b"this is a test file 1231346531468113153";

async fn add_test_package(server: &TestingServer) -> Package {
	add_test_package_with_ctn(server, "v1.0", TEST_FILE_CONTENT).await
}

#[tokio::test]
async fn test_package_info() {
	let mut server = init();

	let package_req = PackageInfoReq {
		channel: Channel::Debug,
		arch: BoardArch::Amd64,
		name: "test".into(),
		device_id: None,
		image_version: None,
		package_versions: None,
		ignore_requirements: false,
	};

	// no package
	let res_1 = server.request(package_req.clone()).await.unwrap();
	assert!(res_1.package.is_none());

	auth_as_writer(&server).await;
	let package = add_test_package(&server).await;
	server.reset_session();

	let package_req = PackageInfoReq {
		channel: Channel::Debug,
		arch: BoardArch::Amd64,
		name: "test".into(),
		device_id: None,
		image_version: None,
		package_versions: None,
		ignore_requirements: false,
	};

	// no package
	let res_2 = server.request(package_req.clone()).await.unwrap();
	let res_package = res_2.package.unwrap();

	assert_eq!(res_package, package);
}

#[tokio::test]
async fn test_get_file() {
	let mut server = init();

	auth_as_writer(&server).await;
	let package = add_test_package(&server).await;
	server.reset_session();

	let res = server
		.request(GetFileReq::new(package.version))
		.await
		.unwrap();
	assert_eq!(res.file(), TEST_FILE_CONTENT);
}

#[tokio::test]
async fn test_get_file_part() {
	let mut server = init();

	auth_as_writer(&server).await;
	let package = add_test_package(&server).await;
	server.reset_session();

	// empty request
	let resp = server
		.request(GetFilePartReq::new(package.version.clone(), 0, 0))
		.await
		.unwrap();
	assert_eq!(resp.file_part().len(), 0);

	// to long request
	let resp = server
		.request(GetFilePartReq::new(package.version.clone(), 0, 1000))
		.await
		.unwrap();
	assert_eq!(resp.total_file_len(), resp.file_part().len() as u64);

	// out of bound request
	let resp = server
		.request(GetFilePartReq::new(package.version.clone(), 1000, 1000))
		.await;
	assert!(matches!(resp, Err(Error::StartUnreachable)));

	// parts request
	let mut parts = vec![];
	for i in 0..=100 {
		let resp = server
			.request(GetFilePartReq::new(package.version.clone(), i * 10, 10))
			.await
			.unwrap();

		parts.extend_from_slice(resp.file_part());

		if resp.total_file_len() == parts.len() as u64 {
			break;
		}
	}
	assert_eq!(parts, TEST_FILE_CONTENT);
}

#[tokio::test]
async fn test_get_file_builder() {
	let mut server = init();

	auth_as_writer(&server).await;
	let package = add_test_package(&server).await;
	server.reset_session();

	let mut builder = GetFileBuilder::new(package.version, 10);
	loop {
		let req = builder.next_req();
		let resp = server.request(req).await.unwrap();
		builder.add_resp(resp);

		if builder.is_complete() {
			break;
		}
	}
	assert_eq!(builder.file(), TEST_FILE_CONTENT);
}

#[tokio::test]
async fn test_set_package() {
	let server = init();

	auth_as_writer(&server).await;

	let package = add_test_package(&server).await;

	let _ = server
		.request(SetPackageInfoReq {
			package,
			requirements: HashMap::new(),
			whitelist: Default::default(),
			auto_whitelist_limit: 0,
		})
		.await
		.unwrap();
	// this will override the version
}

fn test_pack_req(device_id: &DeviceId) -> PackageInfoReq {
	PackageInfoReq {
		channel: Channel::Debug,
		arch: BoardArch::Amd64,
		name: "test".into(),
		device_id: Some(device_id.clone()),
		image_version: None,
		package_versions: None,
		ignore_requirements: false,
	}
}

#[tokio::test]
async fn test_whitelist() {
	let server = init();
	auth_as_writer(&server).await;

	let device_ids = (0..=100).map(|_| DeviceId::new()).collect::<Vec<_>>();

	let package_1 =
		add_test_package_with_ctn(&server, "v1.0", TEST_FILE_CONTENT).await;

	let package_2 = add_test_package_with_ctn(
		&server,
		"v2.0",
		&[TEST_FILE_CONTENT, b"123"].concat(),
	)
	.await;

	// check that all have access to v2.0
	for id in &device_ids {
		let resp = server
			.request(test_pack_req(id))
			.await
			.unwrap()
			.package
			.unwrap();

		assert_eq!(resp.version_str, package_2.version_str);
	}

	// reset current entry
	let _ = server
		.request(SetPackageInfoReq {
			package: package_2.clone(),
			requirements: HashMap::new(),
			whitelist: device_ids.iter().take(4).cloned().collect(),
			auto_whitelist_limit: 0,
		})
		.await
		.unwrap();

	// first test
	// the first four should see package 2 and the rest package 1
	for (i, id) in device_ids.iter().enumerate() {
		let resp = server
			.request(test_pack_req(id))
			.await
			.unwrap()
			.package
			.unwrap();

		if i < 4 {
			assert_eq!(resp.version_str, package_2.version_str);
		} else {
			assert_eq!(resp.version_str, package_1.version_str);
		}
	}

	// allow the last four devices to be whitelisted as well
	let _ = server
		.request(ChangeWhitelistReq {
			arch: TargetArch::Amd64,
			name: "test".into(),
			version: package_2.version.clone(),
			change: WhitelistChange::Add(
				device_ids.iter().rev().take(4).cloned().collect(),
			),
		})
		.await
		.unwrap();

	// check that the first 4 and the last 4 see package 2
	for (i, id) in device_ids.iter().enumerate() {
		let resp = server
			.request(test_pack_req(id))
			.await
			.unwrap()
			.package
			.unwrap();

		if i < 4 || i >= device_ids.len() - 4 {
			assert_eq!(resp.version_str, package_2.version_str);
		} else {
			assert_eq!(resp.version_str, package_1.version_str);
		}
	}

	// authorize in total 50 devices to be whitelisted automatically
	let _ = server
		.request(ChangeWhitelistReq {
			arch: TargetArch::Amd64,
			name: "test".into(),
			version: package_2.version.clone(),
			change: WhitelistChange::SetMinAuto(50),
		})
		.await
		.unwrap();

	// check that the first 46 and the last 4 see package 2
	for (i, id) in device_ids.iter().enumerate() {
		let resp = server
			.request(test_pack_req(id))
			.await
			.unwrap()
			.package
			.unwrap();

		eprintln!("{i}: {}", resp.version_str);

		if i < 50 - 4 || i >= device_ids.len() - 4 {
			assert_eq!(resp.version_str, package_2.version_str);
		} else {
			assert_eq!(resp.version_str, package_1.version_str);
		}
	}
}

#[tokio::test]
async fn test_requirements() {
	let server = init();
	auth_as_writer(&server).await;

	let package_v1 = add_test_package_with_ctn(
		&server,
		"v1.0.0",
		&[TEST_FILE_CONTENT, b"v1"].concat(),
	)
	.await;

	let package_v2 = add_package_with_ctn_and_requirements(
		"test",
		&server,
		"v2.0.0",
		&[TEST_FILE_CONTENT, b"v2"].concat(),
		HashMap::from([("alice".into(), ">=2.0".parse().unwrap())]),
	)
	.await;

	// test with no versions provided
	let package = server
		.request(PackageInfoReq {
			channel: Channel::Debug,
			arch: BoardArch::Amd64,
			name: "test".into(),
			device_id: None,
			image_version: None,
			package_versions: None,
			ignore_requirements: false,
		})
		.await
		.unwrap()
		.package
		.unwrap();
	assert_eq!(package.version_str, package_v1.version_str);

	// test with old versions provided
	let package = server
		.request(PackageInfoReq {
			channel: Channel::Debug,
			arch: BoardArch::Amd64,
			name: "test".into(),
			device_id: None,
			image_version: None,
			package_versions: Some(HashMap::from([(
				"alice".into(),
				"v1.0.0".into(),
			)])),
			ignore_requirements: false,
		})
		.await
		.unwrap()
		.package
		.unwrap();
	assert_eq!(package.version_str, package_v1.version_str);

	// test with new versions provided
	let package = server
		.request(PackageInfoReq {
			channel: Channel::Debug,
			arch: BoardArch::Amd64,
			name: "test".into(),
			device_id: None,
			image_version: None,
			package_versions: Some(HashMap::from([(
				"alice".into(),
				"v2.0.0".into(),
			)])),
			ignore_requirements: false,
		})
		.await
		.unwrap()
		.package
		.unwrap();
	assert_eq!(package.version_str, package_v2.version_str);

	// test with ignore requirements
	let package = server
		.request(PackageInfoReq {
			channel: Channel::Debug,
			arch: BoardArch::Amd64,
			name: "test".into(),
			device_id: None,
			image_version: None,
			package_versions: None,
			ignore_requirements: true,
		})
		.await
		.unwrap()
		.package
		.unwrap();
	assert_eq!(package.version_str, package_v2.version_str);
}
