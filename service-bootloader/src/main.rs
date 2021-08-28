
use std::thread;
use std::time::Duration;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::error::Error as StdError;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;

use gpt::{GptConfig, GptDisk};
use gpt::partition::{Partition as GptPartition};
use gpt::mbr::ProtectiveMBR;
use gpt::disk::LogicalBlockSize;
use linux_info::storage::{MountPoints, Partitions, sector_size};

use uuid::Uuid;

// the size is set in genimage-efi.cfg
const BOOT_SIZE: u64 = 16_744_448;

// should create around 500mb
// that should be enough since our rootfs at the moment
// is 180mb
//
// in bytes
// is divisable by 512 and 4096
const ROOTFS_SIZE: u64 = 500_002_816;



fn main() {

	let mut args = std::env::args();
	let _ = args.next();

	let arg = args.next().unwrap_or("".into());

	// get's started as root
	// where should we store it?
	// in usr/bin

/*
## Chnobli service bootloader

- manage pssplash
- start chnobli service package

- supports api via stdin
 - can switch boot img
 - update images from img file
 - can restart
 - watchdog for chnobli service
	 restart if chnobli service does not send
	 connected for a given period
 - start weston service
*/



	let disks = Disks::read().expect("disks could not be read");

	let mut sda = None;
	let mut sdb = None;


	for (name, disk) in disks.inner {
		// println!("disk {:?}", name);
		// println!("mbr {:#?}", disk.read_mbr());
		// println!("gpt {:#?}", disk.gpt_disk);

		if name == "sda" {
			sda = Some(disk);
		} else if name == "sdb" {
			sdb = Some(disk);
		}
	}

	println!("got sda {:#?}", sda);

	println!("got sdb {:#?}", sdb);

	let (mut sda, mut sdb) = match (sda, sdb) {
		(Some(sda), Some(sdb)) => (sda, sdb),
		_ => panic!("sda or sdb not found")
	};

	// now we wan't to install
	// 

	if arg == "install" {
		println!("doing install in 10s");
		thread::sleep(Duration::from_secs(5));
		println!("doing install in 5s");
		thread::sleep(Duration::from_secs(5));
		println!("doing install in 2s");
		thread::sleep(Duration::from_secs(2));
		println!("doing install");
		install_to_new_disk(&mut sda, &mut sdb)
			.expect("could not do install");
	}



	// println!("disks {:#?}", disks);

	// the user needs to decide if we should repartition us on this drive or if
	// we should install our self on another target


	// to be able to decide where to move to
	// we need to know which block device we're on which partitions
	// exists
	// the size
	// and how much space we have left




	// let mut sda = File::open("/dev/sda").expect("could not open /dev/sda");
	// // now we need to seek to 2048
	// // sda.seek(SeekFrom::Start(0)).unwrap();
	// sda.seek(SeekFrom::Start(2048 * 512)).unwrap();
	// // now let's read 512bytes
	// let mut b = [0; 4096];

	// let r = sda.read(&mut b).expect("could not read");

	// println!("read {}", r);
	// let mut s = String::from_utf8_lossy(&b).into_owned();
	// println!("ctn {}", s);

}


// returns the file name
fn list_files(dir: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
	let mut v = vec![];

	for entry in fs::read_dir(dir)? {
		let e = entry?;
		if e.file_type()?.is_dir() {
			continue
		}

		// so we have a file
		v.push(e.path());
	}

	Ok(v)
}

macro_rules! try_cont {
	($ex:expr) => (match $ex {
		Some(o) => o,
		None => continue
	})
}

#[derive(Debug)]
pub struct Disks {
	// the path (sda, sdb)
	inner: HashMap<String, Disk>
}

impl Disks {

	pub fn read() -> io::Result<Self> {
		let mut list = HashMap::new();


		// read partitions to get their sizes
		let mut part_blocks = HashMap::new();

		for part in Partitions::read()?.entries() {
			let name = try_cont!(part.name());
			let blocks = try_cont!(part.blocks());
			part_blocks.insert(name.to_string(), blocks);
		}


		// first get every disk
		let disks = list_files("/sys/block")?;

		for disk_path in disks {
			let name = try_cont!(disk_path.file_name());
			let name = try_cont!(name.to_str());
			if name.starts_with("loop") {
				continue
			}

			let name = name.to_string();

			let blocks = part_blocks.get(&name);

			let mut disk = Disk::new(&name, blocks.cloned())?;

			list.insert(name, disk);

		}

		let mut me = Self { inner: list };

		me.set_root(&root_uuid()?);

		Ok(me)
	}

	fn set_root(&mut self, uuid: &Uuid) {
		for (name, disk) in &mut self.inner {
			let r = disk.set_root(uuid);
			if r {
				println!("disk {:?} became root", name);
			}
		}
	}

}

#[derive(Debug)]
pub struct Disk {
	path: PathBuf,
	blocks: Option<usize>,
	gpt_disk: Option<GptDisk<'static>>,
	is_root: bool
}

impl Disk {

	pub fn new(name: &str, blocks: Option<usize>) -> io::Result<Self> {
		let path = Path::new("/dev").join(name);

		let mut me = Self {
			path, blocks,
			gpt_disk: None,
			is_root: false
		};

		if let Err(e) = me.open_gpt() {
			println!("could not open gpt on {:?} with {:?}", name, e);
		}

		// if necessary should load block_size (so we can show binary size)
		Ok(me)
	}

	fn open_gpt(&mut self) -> io::Result<()> {
		let disk = GptConfig::new()
			.writable(false)
			.open(&self.path)?;

		self.gpt_disk = Some(disk);
		Ok(())
	}

	fn set_root(&mut self, uuid: &Uuid) -> bool {
		let disk = match &self.gpt_disk {
			Some(d) => d,
			None => return false
		};

		if disk.guid() == uuid {
			self.is_root = true;
		}

		for (_, part) in disk.partitions() {
			if &part.part_guid == uuid {
				self.is_root = true;
			}
		}

		self.is_root
	}

	fn raw_sector_size(&self) -> io::Result<u64> {
		sector_size(&self.path)
	}

	pub fn read_mbr(&self) -> io::Result<ProtectiveMBR> {
		let mut file = File::open(&self.path)?;
		ProtectiveMBR::from_disk(&mut file, LogicalBlockSize::Lb512)
	}

	pub fn writable_file(&mut self) -> io::Result<File> {
		fs::OpenOptions::new()
			.read(true)
			.write(true)
			.open(&self.path)
	}

	pub fn clone_part(&self, name: &str) -> Option<GptPartition> {
		let disk = self.gpt_disk.as_ref()?;
		for (_, part) in disk.partitions() {
			if part.name == name {
				return Some(part.clone())
			}
		}

		None
	}

}


// installation partions
// efi
// rootfs
// data



fn root_uuid() -> io::Result<Uuid> {
	// read the cmdline and get the root parameter
	let cmd = fs::read_to_string("/proc/cmdline")?;
	cmd.split_ascii_whitespace()
		.find_map(|p| {
			p.split_once('=')
				.filter(|(a, _)| a == &"root")
				.map(|(_, b)| b)
		})
		.and_then(|v| {
			v.split_once('=')
				.filter(|(a, _)| matches!(*a, "UUID" | "PARTUUID"))
				.map(|(_, b)| b)
		})
		.map(Uuid::parse_str)
		.ok_or_else(|| io_other("no root or uuid"))
		.and_then(|o| o.map_err(io_other))
}

fn io_other<E>(e: E) -> io::Error
where E: Into<Box<dyn StdError + Send + Sync>> {
	io::Error::new(io::ErrorKind::Other, e)
}

/// writes a new partition
fn install_to_new_disk(install_disk: &mut Disk, new_disk: &mut Disk) -> io::Result<()> {
	// do we need to write the entire drive
	// or is it enough to 

	// delete previous gpt if it exists
	new_disk.gpt_disk = None;


	let sector_size = new_disk.raw_sector_size()?;
	let lbs = LogicalBlockSize::try_from(sector_size)?;

	// create mbr but don't write it
	{
		let prev_mbr = install_disk.read_mbr()?;

		let mut file = new_disk.writable_file()?;
		let len = file.seek(SeekFrom::End(0))?;
		let sectors = len.checked_div(sector_size)
			.ok_or_else(|| io_other("file len not % sector size"))?;

		let mut mbr = ProtectiveMBR::with_lb_size((sectors - 1) as u32);
		mbr.set_bootcode(prev_mbr.bootcode().clone());
		mbr.set_disk_signature(prev_mbr.disk_signature().clone());
		mbr.overwrite_lba0(&mut file)?;
	}

	// let's create a partion from scratch
	let mut disk = GptConfig::new()
		.writable(true)
		.initialized(false)
		.logical_block_size(lbs)
		.open(&new_disk.path)?;

	// remove any previous partitions
	// and set headers
	disk.update_partitions(BTreeMap::new())?;
	let header = disk.primary_header().unwrap();

	// now add boot partition
	let mut boot = install_disk.clone_part("boot")
		.ok_or_else(|| io_other("could not get boot partition"))?;
	boot.part_guid = Uuid::new_v4();
	let boot_sectors = BOOT_SIZE / sector_size;
	boot.first_lba = header.first_usable;
	boot.last_lba = boot.first_lba + boot_sectors - 1;// -1 because inclusive

	let rootfs_sectors = ROOTFS_SIZE / sector_size;

	// now create first root fs partition
	let mut root_a = install_disk.clone_part("root")
		.ok_or_else(|| io_other("could not get root partition"))?;
	root_a.part_guid = Uuid::new_v4();
	root_a.first_lba = boot.last_lba + 1;
	root_a.last_lba = root_a.first_lba + rootfs_sectors - 1;
	root_a.name = "root a".into();

	// now create second root fs partition
	let mut root_b = root_a.clone();
	root_b.part_guid = Uuid::new_v4();
	root_b.first_lba = root_a.last_lba + 1;
	root_b.last_lba = root_b.first_lba + rootfs_sectors - 1;
	root_b.name = "root b".into();

	// data partition
	let data_lba = (header.last_usable - root_b.last_lba) / 2;
	let mut data = install_disk.clone_part("data")
		.ok_or_else(|| io_other("could not get root partition"))?;
	data.part_guid = Uuid::new_v4();
	data.first_lba = root_b.last_lba + 1;
	data.last_lba = data.first_lba + data_lba - 1;
	data.name = "data".into();

	let mut map = BTreeMap::new();
	map.insert(0, boot);
	map.insert(1, root_a);
	map.insert(2, root_b);
	map.insert(3, data);

	disk.update_partitions(map)?;

	disk.write_inplace()?;

	new_disk.gpt_disk = Some(disk);

	Ok(())
}

// service api
// all requests start with :<:
// all responses start with :>:

// start weston with
// systemctl start weston

// this should probably be done better
fn is_real_disk(s: &str) -> bool {
	if s.starts_with("sd") {
		// sda
		// sda1
		let last = s.chars().last().unwrap();
		
		last.is_ascii_alphabetic()
	} else if s.starts_with("nvme") {
		// nvme0n1
		// nvme0n1p2
		let p = s.chars().rev().nth(1).unwrap();

		p != 'p'
	} else {
		false
	}
}