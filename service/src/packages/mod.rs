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
use crate::util::io_other;
use crate::Bootloader;

use std::collections::{hash_map::Entry, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use tokio::fs;
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

use rand::{thread_rng, Rng};

use api::requests::packages::Package;
use bootloader_api::requests::{Architecture, DeviceId, VersionInfo};
use file_db::FileDb;
use packages::packages::{
	BoardArch, Channel, Hash, Package as PackPackage, PackageCfg, PackagesCfg,
	Source,
};
use packages::requests::GetFileBuilder;

const PACKAGES_DIR: &str = "/data/packages";

// this should stay in sync with PackagesCfg::Left (see below)
const DEFAULT_FOLDER: &str = "left";

fn path(s: &str) -> PathBuf {
	Path::new(PACKAGES_DIR).join(s)
}

fn new_api() -> (PackagesApi, PackagesReceiver) {
	let (tx, rx) = mpsc::channel(2);

	(PackagesApi { tx }, PackagesReceiver { rx })
}

#[derive(Clone)]
pub struct Packages {
	api: PackagesApi,
	inner: SyncPackages,
}

impl Packages {
	/// Downloads a package and adds it to the list returns None if the package
	/// was not found
	pub async fn add_package(&self, name: String) -> Option<Package> {
		self.api.add_package(name).await
	}

	pub async fn update(&self) {
		self.api.update().await
	}

	/// Returns a list of all current packages
	pub async fn packages(&self) -> Vec<Package> {
		self.inner.packages().await
	}

	/// Returns the stored packages configuration
	pub async fn config(&self) -> PackagesCfg {
		self.inner.config().await
	}

	/// Returns the binary if it exists which should be run after the ui
	/// was started
	///
	/// The first string is the path of the package
	/// and the second string is the path to the binary
	pub async fn on_run_binary(&self) -> Option<(String, String)> {
		self.inner.on_run_binary().await
	}
}

#[derive(Debug)]
enum PackRequest {
	RequestUpdate(oneshot::Sender<()>),
	AddPackage(String, oneshot::Sender<Option<Package>>),
}

// impl Packages

/// Internal api to communicate with the packages background task.
#[derive(Clone)]
struct PackagesApi {
	tx: mpsc::Sender<PackRequest>,
}

impl PackagesApi {
	/// this might take a while
	pub async fn add_package(&self, name: String) -> Option<Package> {
		let (tx, rx) = oneshot::channel();

		// should we check if the package already exists?
		self.tx
			.send(PackRequest::AddPackage(name, tx))
			.await
			.expect("packages api failed");

		rx.await.expect("packages api failed")
	}

	/// this might take a while
	async fn update(&self) {
		let (tx, rx) = oneshot::channel();

		self.tx
			.send(PackRequest::RequestUpdate(tx))
			.await
			.expect("packages api failed");

		rx.await.expect("packages api failed");
	}
}

struct PackagesReceiver {
	rx: mpsc::Receiver<PackRequest>,
}

impl PackagesReceiver {
	pub async fn recv(&mut self) -> PackRequest {
		self.rx.recv().await.expect("service api failed")
	}
}

// if we look at release interval that means this may run for at least 120minutes
const UPDATE_STATE_ERROR_LIMIT: usize = 120;

pub async fn start(
	client: Bootloader,
) -> io::Result<(Packages, JoinHandle<()>)> {
	let (tx, mut rx) = new_api();

	let packages = SyncPackages::load().await?;

	let ret_pack = Packages {
		api: tx,
		inner: packages.clone(),
	};

	let task = tokio::spawn(async move {
		// get version info so we know if we should update or not
		let version_info = client
			.version_info()
			.await
			.expect("fetching version failed");

		if !version_info.installed {
			// not installed

			// for an installation to be finished
			// a restart is required so we don't need to check in a loop
			eprintln!("not installed, only updating when installed");
			return;
		}

		let times = UpdateIntervalTimes::from_channel(packages.channel().await);

		let mut failed = false;
		let mut update_state: Option<Update> = None;
		let mut update_state_error = 0;
		let mut req: Option<PackRequest> = None;

		loop {
			let time = if failed {
				times.failed_duration()
			} else {
				times.normal_duration()
			};

			tokio::select! {
				n_req = rx.recv(), if req.is_none() => {
					req = Some(n_req);
				},
				_ = sleep(time) => {}
			};

			if update_state_error >= UPDATE_STATE_ERROR_LIMIT {
				eprintln!("update state error limit reached");
				update_state = None;
				update_state_error = 0;
			}

			if update_state.is_none() {
				update_state =
					Some(packages.prepare_update(&version_info).await);
			}
			let mut update_data = update_state.as_mut().unwrap();

			// todo add package if we have a request
			match &req {
				Some(PackRequest::AddPackage(name, _)) => {
					if !update_data.packages.contains_key(name) {
						update_data.packages.insert(
							name.clone(),
							PackageUpdateState::GatherInfo {
								version: None,
								new_folder: DEFAULT_FOLDER.to_string(),
							},
						);
					}
				}
				_ => {}
			}

			let update_start_time = Instant::now();

			// update all packages and the image
			let update_res =
				update(&packages.sources().await, &mut update_data, &client)
					.await;
			match update_res {
				Ok(_) => failed = false,
				Err(e) => {
					eprintln!("update error {:?}", e);
					failed = true;
					update_state_error += 1;
					continue;
				}
			}

			packages
				.apply_update(&update_data)
				.await
				.expect("apply_update failed");

			let image_was_updated = update_data.image.was_updated();
			let any_package_was_updated = update_data.any_package_was_updated();

			eprintln!(
				"updated: {:?} took {}ms and {} tries",
				update_data,
				update_start_time.elapsed().as_millis(),
				update_state_error
			);
			update_state = None;
			update_state_error = 0;

			// send success
			match req.take() {
				Some(PackRequest::AddPackage(name, tx)) => {
					// let's check if the package was added
					let _ = tx.send(packages.get(&name).await);
				}
				Some(PackRequest::RequestUpdate(tx)) => {
					let _ = tx.send(());
				}
				None => {}
			}

			// if image was updated
			if image_was_updated {
				client
					.restart()
					.await
					.expect("could not restart the system");
			// if packages updated
			} else if any_package_was_updated {
				client
					.systemd_restart("service-bootloader")
					.await
					.expect("could not restart service-bootloader");
			}
		}
	});

	Ok((ret_pack, task))
}

struct UpdateIntervalTimes {
	failed: UpdateInterval,
	normal: UpdateInterval,
}

impl UpdateIntervalTimes {
	const DEBUG: Self = Self {
		failed: UpdateInterval::Fixed(30),
		normal: UpdateInterval::Fixed(30),
	};
	const ALPHA: Self = Self {
		failed: UpdateInterval::Fixed(30),
		normal: UpdateInterval::Fixed(60),
	};
	const BETA: Self = Self {
		failed: UpdateInterval::Range(30, 2 * 60),
		normal: UpdateInterval::Range(2 * 60, 10 * 60),
	};
	const RELEASE: Self = Self {
		// 1-5 minutes
		failed: UpdateInterval::Range(60, 5 * 60),
		// 5-15 minutes
		normal: UpdateInterval::Range(5 * 60, 15 * 60),
	};

	const fn from_channel(channel: Channel) -> Self {
		match channel {
			Channel::Debug => Self::DEBUG,
			Channel::Alpha => Self::ALPHA,
			Channel::Beta => Self::BETA,
			Channel::Release => Self::RELEASE,
		}
	}

	fn failed_duration(&self) -> Duration {
		self.failed.to_duration()
	}

	fn normal_duration(&self) -> Duration {
		self.normal.to_duration()
	}
}

#[derive(Debug, Clone, Copy)]
enum UpdateInterval {
	// in seconds
	Fixed(u64),
	// in seconds
	Range(u64, u64),
}

impl UpdateInterval {
	pub fn to_duration(&self) -> Duration {
		match *self {
			Self::Fixed(s) => Duration::from_secs(s),
			Self::Range(min, max) => {
				Duration::from_secs(thread_rng().gen_range(min..max))
			}
		}
	}
}

async fn extract(path: &str, to: &str) -> io::Result<()> {
	let stat = Command::new("tar")
		.args(&["-zxvf", path, "-C", to])
		.status()
		.await?;
	if stat.success() {
		Ok(())
	} else {
		Err(io_other("extraction failed"))
	}
}

/// A struct which manages the state of an update process
#[derive(Debug, Clone)]
struct Update {
	pub board: String,
	pub arch: BoardArch,
	pub channel: Channel,
	pub device_id: Option<DeviceId>,
	pub packages: HashMap<String, PackageUpdateState>,
	pub image: ImageUpdateState,
}

impl Update {
	pub fn is_finished(&self) -> bool {
		// find a package that is not already updated
		!self.packages.values().any(|v| !v.is_finished())
			&& self.image.is_finished()
	}

	/// Returns a list of package states which need to be updated
	pub fn not_finished_packages(
		&mut self,
	) -> impl Iterator<Item = (&str, &mut PackageUpdateState)> {
		self.packages
			.iter_mut()
			.filter(|(_, pack)| !pack.is_finished())
			.map(|(name, pack)| (name.as_str(), pack))
	}

	/// Check if any package was updated
	pub fn any_package_was_updated(&self) -> bool {
		self.packages.values().any(|p| p.was_updated())
	}
}

#[derive(Debug, Clone)]
enum PackageUpdateState {
	GatherInfo {
		// name is stored in the table
		/// version is used to check if the package needs to be updated
		version: Option<Hash>,
		new_folder: String,
	},
	DownloadFile {
		package: PackPackage,
		get_file: GetFileBuilder,
		new_folder: String,
	},
	NoUpdate,
	Updated(PackPackage),
	/// This means the package was not found in any source
	/// or that package was returned with the wrong signature
	NotFound,
}

impl PackageUpdateState {
	pub fn is_finished(&self) -> bool {
		match self {
			Self::NoUpdate | Self::Updated(_) | Self::NotFound => true,
			Self::GatherInfo { .. } | Self::DownloadFile { .. } => false,
		}
	}

	pub fn was_updated(&self) -> bool {
		matches!(self, Self::Updated(_))
	}
}

#[derive(Debug, Clone)]
enum ImageUpdateState {
	GatherInfo {
		version: Hash,
	},
	DownloadFile {
		package: PackPackage,
		get_file: GetFileBuilder,
	},
	NoUpdate,
	Updated(VersionInfo),
	NotFound,
}

impl ImageUpdateState {
	pub fn is_finished(&self) -> bool {
		match self {
			Self::NoUpdate | Self::Updated(_) | Self::NotFound => true,
			Self::GatherInfo { .. } | Self::DownloadFile { .. } => false,
		}
	}

	pub fn was_updated(&self) -> bool {
		matches!(self, Self::Updated(_))
	}
}

// writing to the packages location is only permitted by this struct / this
// module and one task
#[derive(Debug, Clone)]
struct SyncPackages {
	inner: Arc<RwLock<RawPackages>>,
}

impl SyncPackages {
	pub async fn load() -> io::Result<Self> {
		Ok(Self {
			inner: Arc::new(RwLock::new(RawPackages::load().await?)),
		})
	}

	// is this expensive?
	pub async fn channel(&self) -> Channel {
		self.inner.read().await.cfg.channel
	}

	/// Creates an update struct from the current packages information
	pub async fn prepare_update(&self, version: &VersionInfo) -> Update {
		self.inner.read().await.prepare_update(version)
	}

	pub async fn sources(&self) -> Vec<Source> {
		self.inner.read().await.cfg.sources.clone()
	}

	pub async fn apply_update(&self, update: &Update) -> io::Result<()> {
		self.inner.write().await.apply_update(update).await
	}

	pub async fn get(&self, name: &str) -> Option<Package> {
		self.inner
			.read()
			.await
			.list
			.get(name)
			.map(|db| package_cfg_to_package(db))
	}

	pub async fn packages(&self) -> Vec<Package> {
		self.inner.read().await.packages()
	}

	pub async fn config(&self) -> PackagesCfg {
		self.inner.read().await.cfg.clone()
	}

	pub async fn on_run_binary(&self) -> Option<(String, String)> {
		self.inner.read().await.on_run_binary()
	}
}

#[derive(Debug)]
struct RawPackages {
	cfg: FileDb<PackagesCfg>,
	list: HashMap<String, FileDb<PackageCfg>>,
}

impl RawPackages {
	/// Load the packages information from the filesystem
	pub async fn load() -> io::Result<Self> {
		let cfg = FileDb::open(path("packages.fdb")).await?;

		let mut list = HashMap::new();
		// read all directories

		let mut dirs = fs::read_dir(PACKAGES_DIR).await?;
		while let Some(entry) = dirs.next_entry().await? {
			if !entry.file_type().await?.is_dir() {
				continue;
			}

			let mut path = entry.path();
			path.push("package.fdb");
			let cfg = FileDb::<PackageCfg>::open(path).await?;

			list.insert(cfg.package().name.clone(), cfg);
		}

		Ok(Self { cfg, list })
	}

	/// Creates the update struct from the current packages
	pub fn prepare_update(&self, version: &VersionInfo) -> Update {
		let mut packs = HashMap::new();

		for (name, pack) in &self.list {
			let state = PackageUpdateState::GatherInfo {
				version: Some(pack.package().version.clone()),
				// todo maybe store &'static str
				new_folder: pack.other().to_string(),
			};
			packs.insert(name.clone(), state);
		}

		let image = ImageUpdateState::GatherInfo {
			version: version.version.clone(),
		};

		Update {
			board: version.board.clone(),
			arch: boot_arch_to_board_arch(version.arch),
			channel: self.cfg.channel,
			device_id: version.device_id.clone(),
			packages: packs,
			image,
		}
	}

	// changing values depending on a previous state without checking if the
	// state has changed is unproblematic here since only this module
	// has write access and it uses a single task, so the state cannot change
	// in betweeen read, & write
	pub async fn apply_update(&mut self, update: &Update) -> io::Result<()> {
		for (name, state) in &update.packages {
			match state {
				PackageUpdateState::NoUpdate => {}
				PackageUpdateState::Updated(package) => {
					match self.list.entry(name.clone()) {
						Entry::Occupied(mut o) => {
							let db = o.get_mut();
							db.switch(package.clone());
							db.write().await?;
						}
						Entry::Vacant(v) => {
							let path = format!(
								"{}/{}/package.fdb",
								PACKAGES_DIR, name
							);
							// this should stay in sync with DEFAULT_FOLDER
							let cfg = PackageCfg::Left(package.clone());
							let db = FileDb::new(path, cfg);
							let db = v.insert(db);
							db.write().await?;
						}
					};
				}
				PackageUpdateState::GatherInfo { .. }
				| PackageUpdateState::DownloadFile { .. } => unreachable!(),
				PackageUpdateState::NotFound => {}
			}
		}

		Ok(())
	}

	/// returns the binary
	/// that should be run after the ui has started
	/// The first string is the path of the package
	/// and the second string is the path to the binary
	pub fn on_run_binary(&self) -> Option<(String, String)> {
		let on_run = &self.cfg.on_run;
		let package = self.list.get(on_run)?;
		let binary = package.package().binary.as_ref()?;

		let dir = format!("{}/{}/{}", PACKAGES_DIR, on_run, package.current());

		let bin = format!("{}/{}", dir, binary);

		Some((dir, bin))
	}

	pub fn packages(&self) -> Vec<Package> {
		self.list
			.values()
			.map(|v| package_cfg_to_package(v))
			.collect()
	}
}

fn boot_arch_to_board_arch(boot_arch: Architecture) -> BoardArch {
	match boot_arch {
		Architecture::Amd64 => BoardArch::Amd64,
		Architecture::Arm64 => BoardArch::Arm64,
	}
}

fn package_cfg_to_package(cfg: &PackageCfg) -> Package {
	let pack = cfg.package();

	Package {
		name: pack.name.clone(),
		version_str: pack.version_str.clone(),
		version: pack.version.clone(),
		signature: pack.signature.clone(),
		binary: pack.binary.clone(),
		path: format!("/data/packages/{}/{}", pack.name, cfg.current()),
	}
}
