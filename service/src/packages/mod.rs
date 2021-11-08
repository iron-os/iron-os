//! ## packages folder
//! packages
//!  - packages.fdb
//!  - chnobli_ui
//!   - package.fdb // json_db containing information about the package
//!   - left
//!   - right
//! 
//! package.fdb
//!  - name
//!  - version_str
//!  - version // hash
//!  - signature // signature of the current version
//!  - current // folder of the current left|right
//!  - binary // Option<String>

mod update;

use update::update;

// use crypto::signature::PublicKey;
use crate::Bootloader;
use crate::util::io_other;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::{HashMap, hash_map::Entry};

use tokio::fs;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};
use tokio::process::Command;
use tokio::sync::{mpsc, watch, RwLock};

use rand::{thread_rng, Rng};

use bootloader_api::{
	VersionInfoReq, VersionInfo, SystemdRestart, Architecture
};
use packages::packages::{
	PackagesCfg, PackageCfg, Package as PackPackage, BoardArch,
	Channel, Hash, Source
};
use api::requests::packages::Package;
use file_db::FileDb;

use bootloader_api::{RestartReq};

const PACKAGES_DIR: &str = "/data/packages";

// this should stay in sync with PackagesCfg::Left (see below)
const DEFAULT_FOLDER: &str = "left";

fn path(s: &str) -> PathBuf {
	Path::new(PACKAGES_DIR).join(s)
}

#[derive(Debug)]
enum PackRequest {
	RequestUpdate,
	AddPackage(String)
}

fn new_api() -> (PackagesApi, PackagesReceiver) {
	let (m_tx, m_rx) = mpsc::channel(2);
	let (w_tx, w_rx) = watch::channel(Updated::new());

	(
		PackagesApi {
			tx: m_tx,
			rx: w_rx
		},
		PackagesReceiver {
			tx: w_tx,
			rx: m_rx
		}
	)
}

#[derive(Clone)]
pub struct Packages {
	api: PackagesApi,
	inner: SyncPackages
}

impl Packages {
	pub async fn add_package(&mut self, name: String) -> Package {
		let _ = self.api.add_package(name.clone()).await;
		loop {
			if let Some(pack) = self.inner.get(&name).await {
				return pack
			}

			self.api.recv_updated().await;
		}
	}

	pub async fn packages(&self) -> Vec<Package> {
		self.inner.packages().await
	}

	pub async fn config(&self) -> PackagesCfg {
		self.inner.config().await
	}

	pub async fn on_run_binary(&self) -> Option<(String, String)> {
		self.inner.on_run_binary().await
	}
}

#[derive(Debug, Clone)]
pub struct Updated {
	pub packages: Vec<String>,
	// image version
	pub image: bool
}

impl Updated {
	fn new() -> Self {
		Self {
			packages: vec![],
			image: false
		}
	}
}

// impl Packages 

/// Internal api to communcate with the packages background task.
#[derive(Clone)]
struct PackagesApi {
	tx: mpsc::Sender<PackRequest>,
	// names, image
	rx: watch::Receiver<Updated>
}

impl PackagesApi {
	pub async fn add_package(&mut self, name: String) -> Updated {
		// should we check if the package already exists?
		self.tx.send(PackRequest::AddPackage(name)).await
			.expect("packages api failed");

		self.wait_on_new().await
	}

	/// this may take a long time
	#[allow(dead_code)]
	pub async fn force_update(&mut self) -> Updated {
		self.tx.send(PackRequest::RequestUpdate).await
			.expect("packages api failed");
		// clear the receiver so we don't receive old values
		self.wait_on_new().await
	}

	pub async fn wait_on_new(&mut self) -> Updated {
		let _ = self.rx.borrow_and_update();
		self.recv_updated().await
	}

	pub async fn recv_updated(&mut self) -> Updated {
		self.rx.changed().await.expect("packages api failed");
		self.rx.borrow().clone()
	}
}

struct PackagesReceiver {
	rx: mpsc::Receiver<PackRequest>,
	tx: watch::Sender<Updated>
}

impl PackagesReceiver {

	pub async fn recv(&mut self) -> PackRequest {
		self.rx.recv().await.expect("service api failed")
	}

	pub fn send(&self, updated: Updated) {
		self.tx.send(updated).expect("packages api failed")
	}

}

// we need to have two watc

pub async fn start(
	client: Bootloader
) -> io::Result<(Packages, JoinHandle<()>)> {

	let (tx, mut rx) = new_api();

	let packages = SyncPackages::load().await?;

	let ret_pack = Packages {
		api: tx,
		inner: packages.clone()
	};

	let task = tokio::spawn(async move {
		// get version info so we know if we should update or not
		let version_info = client.request(&VersionInfoReq).await
			.expect("fetching version failed");

		if !version_info.installed {
			// not installed

			// for an installation to be finished
			// a restart is required so we don't need to check it in the loop
			eprintln!("not installed, only updating when installed");
			return
		}

		let mut failed = false;

		loop {

			let (min, max) = if failed {
				// retry between 2-8 minutes
				(2, 8)
			} else {
				// check version every 5-15 minutes
				(5, 15)
			};

			// we do this step on every iteration to
			// always get a new random value
			let time = match packages.is_debug().await {
				// check version 30 seconds
				true => Duration::from_secs(30),
				false => Duration::from_secs(
					thread_rng()
						.gen_range((60 * min)..(60 * max))
				)
			};

			let req = tokio::select! {
				req = rx.recv() => {
					Some(req)
				},
				_ = sleep(time) => {
					None
				}
			};

			let mut update_data = packages.prepare_update(&version_info).await;

			// todo add package if we have a request
			match req {
				Some(PackRequest::AddPackage(name)) => {
					if !update_data.packages.contains_key(&name) {
						update_data.packages.insert(
							name,
							PackageUpdateState::ToUpdate {
								version: None,
								new_folder: DEFAULT_FOLDER.to_string()
							}
						);
					}
				},
				_ => {}
			}

			// update every
			let update_res = update(
				&packages.sources().await,
				&mut update_data,
				&client
			).await;
			match update_res {
				Ok(_) => failed = false,
				Err(e) => {
					eprintln!("update error {:?}", e);
					failed = true;
				}
			}

			packages.apply_update(&update_data).await
				.expect("apply_update failed");

			eprintln!("updated: {:?}", update_data);

			rx.send(update_data.updated());

			// if image was updated
			if update_data.image.was_updated() {
				client.request(&RestartReq).await
					.expect("could not restart the system");
			// if packages updated
			} else if update_data.package_was_updated() {
				client.request(&SystemdRestart {
					name: "service-bootloader".into()
				}).await
					.expect("could not restart service-bootloader");
			}

		}
	});

	Ok((ret_pack, task))
}

// // needs to stay internal
// struct Packages {
// 	pub cfg: PackagesCfg,
// 	pub list: Vec<PackageCfg>
// }

// impl Packages {

// 	pub async fn load() -> io::Result<Self> {

// 		let cfg = FileDb::open(path("packages.fdb")).await?
// 			.into_data();

// 		let mut list = vec![];
// 		// read all directories

// 		let mut dirs = fs::read_dir(PACKAGES_DIR).await?;
// 		while let Some(entry) = dirs.next_entry().await? {
// 			if !entry.file_type().await?.is_dir() {
// 				continue
// 			}

// 			let mut path = entry.path();
// 			path.push("package.fdb");
// 			let cfg = FileDb::open(path).await?
// 				.into_data();

// 			list.push(cfg);
// 		}

// 		Ok(Self { cfg, list })
// 	}

// }

async fn extract(path: &str, to: &str) -> io::Result<()> {
	let stat = Command::new("tar")
		.args(&["-zxvf", path, "-C", to])
		.status().await?;
	if stat.success() {
		Ok(())
	} else {
		Err(io_other("extraction failed"))
	}
}

#[derive(Debug, Clone)]
struct Update {
	pub board: String,
	pub arch: BoardArch,
	pub channel: Channel,
	pub packages: HashMap<String, PackageUpdateState>,
	pub image: ImageUpdateState
}

impl Update {
	pub fn is_finished(&self) -> bool {
		// find a package that is not already updated
		!self.packages.values().any(|v| !v.is_finished())
		&&
		self.image.is_finished()
	}

	pub fn to_update(
		&mut self
	) -> impl Iterator<Item=(&str, &mut PackageUpdateState)> {
		self.packages.iter_mut()
			.filter(|(_, pack)| pack.is_finished())
			.map(|(name, pack)| (name.as_str(), pack))
	}

	pub fn package_was_updated(&self) -> bool {
		self.packages.values().any(|p| p.was_updated())
	}

	pub fn updated(&self) -> Updated {
		Updated {
			packages: self.packages.iter()
				.filter(|(_, state)| state.was_updated())
				.map(|(n, _)| n.to_string())
				.collect(),
			image: self.image.was_updated()
		}
	}
}

#[derive(Debug, Clone)]
enum PackageUpdateState {
	ToUpdate {
		// is stored in the table
		// name: 
		version: Option<Hash>,
		new_folder: String
	},
	NoUpdate,
	Updated(PackPackage)
}

impl PackageUpdateState {
	pub fn is_finished(&self) -> bool {
		!matches!(self, Self::ToUpdate {..})
	}

	pub fn was_updated(&self) -> bool {
		matches!(self, Self::Updated(_))
	}
}

#[derive(Debug, Clone)]
enum ImageUpdateState {
	ToUpdate {
		version: Hash
	},
	NoUpdate,
	Updated(VersionInfo)
}

impl ImageUpdateState {
	pub fn is_finished(&self) -> bool {
		!matches!(self, Self::ToUpdate {..})
	}

	pub fn was_updated(&self) -> bool {
		matches!(self, Self::Updated(_))
	}
}


// writing is only permitted by this 
#[derive(Debug, Clone)]
struct SyncPackages {
	inner: Arc<RwLock<RawPackages>>
}

impl SyncPackages {
	pub async fn load() -> io::Result<Self> {
		Ok(Self {
			inner: Arc::new(RwLock::new(RawPackages::load().await?))
		})
	}

	// is this expensive?
	pub async fn is_debug(&self) -> bool {
		self.inner.read().await
			.cfg.channel.is_debug()
	}

	pub async fn prepare_update(&self, version: &VersionInfo) -> Update {
		self.inner.read().await.prepare_update(version)
	}

	pub async fn sources(&self) -> Vec<Source> {
		self.inner.read().await
			.cfg.sources.clone()
	}

	pub async fn apply_update(&self, update: &Update) -> io::Result<()> {
		self.inner.write().await
			.apply_update(update).await
	}

	pub async fn get(&self, name: &str) -> Option<Package> {
		self.inner.read().await.list.get(name)
			.map(|db| convert_pack_cfg(db))
	}

	pub async fn packages(&self) -> Vec<Package> {
		self.inner.read().await
			.packages()
	}

	pub async fn config(&self) -> PackagesCfg {
		self.inner.read().await
			.cfg.clone()
	}

	pub async fn on_run_binary(&self) -> Option<(String, String)> {
		self.inner.read().await.on_run_binary()
	}
}

#[derive(Debug)]
struct RawPackages {
	cfg: FileDb<PackagesCfg>,
	list: HashMap<String, FileDb<PackageCfg>>
}

impl RawPackages {

	pub async fn load() -> io::Result<Self> {

		let cfg = FileDb::open(path("packages.fdb")).await?;

		let mut list = HashMap::new();
		// read all directories

		let mut dirs = fs::read_dir(PACKAGES_DIR).await?;
		while let Some(entry) = dirs.next_entry().await? {
			if !entry.file_type().await?.is_dir() {
				continue
			}

			let mut path = entry.path();
			path.push("package.fdb");
			let cfg = FileDb::<PackageCfg>::open(path).await?;

			list.insert(cfg.package().name.clone(), cfg);
		}

		Ok(Self { cfg, list })
	}

	pub fn prepare_update(&self, version: &VersionInfo) -> Update {
		let mut packs = HashMap::new();

		for (name, pack) in &self.list {
			let state = PackageUpdateState::ToUpdate {
				version: Some(pack.package().version.clone()),
				// todo maybe store &'static str
				new_folder: pack.other().to_string()
			};
			packs.insert(name.clone(), state);
		}

		let image = ImageUpdateState::ToUpdate {
			version: version.version.clone()
		};

		Update {
			board: version.board.clone(),
			arch: boot_to_board_arch(version.arch),
			channel: self.cfg.channel,
			packages: packs,
			image
		}
	}

	// changing values depending on a previous state without checking if the
	// state has changed is unproblematic here since only this module
	// has write access and it uses a single task, so the state cannot change
	// in betweeen read, & write
	pub async fn apply_update(&mut self, update: &Update) -> io::Result<()> {
		for (name, state) in &update.packages {
			match state {
				PackageUpdateState::NoUpdate => {},
				PackageUpdateState::Updated(package) => {
					match self.list.entry(name.clone()) {
						Entry::Occupied(mut o) => {
							let db = o.get_mut();
							db.switch(package.clone());
							db.write().await?;
						},
						Entry::Vacant(v) => {
							let path = format!(
								"{}/{}/package.fdb",
								PACKAGES_DIR,
								name
							);
							// this should stay in sync with DEFAULT_FOLDER
							let cfg = PackageCfg::Left(package.clone());
							let db = FileDb::new(path, cfg);
							let db = v.insert(db);
							db.write().await?;
						}
					};
				},
				// this should never happen
				PackageUpdateState::ToUpdate {..} => {
					return Err(io_other("package was not found"))
				}
			}
		}

		Ok(())
	}

	pub fn on_run_binary(&self) -> Option<(String, String)> {
		let on_run = &self.cfg.on_run;
		let package = self.list.get(on_run)?;
		let binary = package.package().binary.as_ref()?;

		let dir = format!(
			"{}/{}/{}",
			PACKAGES_DIR,
			on_run,
			package.current()
		);

		let bin = format!("{}/{}", dir, binary);

		Some((dir, bin))
	}

	pub fn packages(&self) -> Vec<Package> {
		self.list.values()
			.map(|v| convert_pack_cfg(v))
			.collect()
	}
}

fn boot_to_board_arch(boot_arch: Architecture) -> BoardArch {
	match boot_arch {
		Architecture::Amd64 => BoardArch::Amd64,
		Architecture::Arm64 => BoardArch::Arm64
	}
}

fn convert_pack_cfg(cfg: &PackageCfg) -> Package {
	let pack = cfg.package();

	Package {
		name: pack.name.clone(),
		version_str: pack.version_str.clone(),
		version: pack.version.clone(),
		signature: pack.signature.clone(),
		binary: pack.binary.clone(),
		path: format!("/data/packages/{}/{}", pack.name, cfg.current())
	}
}