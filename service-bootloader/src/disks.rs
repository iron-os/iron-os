
use std::thread;
use std::time::Duration;
use std::path::{Path, PathBuf};
use std::fs::{self, File, read_to_string, create_dir_all};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::error::Error as StdError;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::ffi::OsStr;

use gpt::{GptConfig, GptDisk};
use gpt::partition::{Partition as GptPartition};
use gpt::mbr::ProtectiveMBR;
use gpt::disk::LogicalBlockSize;
use linux_info::storage::{MountPoints, Partitions, sector_size};

use uuid::Uuid;

// the size is set in genimage-efi.cfg
// bzImage max size is 20m which allows to have a bzImage tmp
const BOOT_SIZE: u64 = 52_396_032;

// should create around 500mb
// that should be enough since our rootfs at the moment
// is 180mb
//
// in bytes
// is divisable by 512 and 4096
const ROOTFS_SIZE: u64 = 500_002_816;

// get's started as root
fn main() {

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
 - api for setuid root
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
		thread::sleep(Duration::from_secs(3));
		println!("doing install in 2s");
		thread::sleep(Duration::from_secs(2));
		println!("doing install");
		install_to_new_disk(&mut sda, &mut sdb)
			.expect("could not do install");
	}
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

	// gets the sector size via ioctl
	// this should return the sector size advertised the harddrive
	// and not what gpt actually uses
	fn raw_sector_size(&self) -> io::Result<u64> {
		sector_size(&self.path)
	}

	/// returns the sector size if gpt was opened
	fn sector_size(&self) -> Option<u64> {
		self.gpt_disk.as_ref()
			.map(|d| (*d.logical_block_size()).into())
	}

	pub fn read_mbr(&self) -> io::Result<ProtectiveMBR> {
		let mut file = File::open(&self.path)?;
		ProtectiveMBR::from_disk(&mut file, LogicalBlockSize::Lb512)
	}

	pub fn readable_file(&self) -> io::Result<File> {
		fs::OpenOptions::new()
			.read(true)
			.open(&self.path)
	}

	pub fn writable_file(&self) -> io::Result<File> {
		fs::OpenOptions::new()
			.read(true)
			.write(true)
			.open(&self.path)
	}

	pub fn get_part(&self, name: &str) -> Option<&GptPartition> {
		self.gpt_disk.as_ref()?
			.partitions()
			.values()
			.find(|v| v.name == name)
	}

	pub fn clone_part(&self, name: &str) -> Option<GptPartition> {
		self.get_part(name)
			.map(Clone::clone)
	}

	pub fn part_path(&self, name: &str) -> Option<PathBuf> {
		// get partition number
		let id = self.gpt_disk.as_ref()?
			.partitions()
			.iter()
			.find(|(id, v)| v.name == name)
			.map(|(id, _)| *id)?;
		let mut os_str = self.path.clone().into_os_string();
		let id_str = format!("{}", id);
		os_str.push(id_str);
		Some(os_str.into())
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

/// writes a new partition
fn install_to_new_disk(install_disk: &mut Disk, new_disk: &mut Disk) -> io::Result<()> {
	// do we need to write the entire drive
	// or is it enough to 

	write_gpt_to_new_disk(install_disk, new_disk)?;

	copy_to_new_disk(install_disk, new_disk)?;

	configure_disk(new_disk)?;

	// success
	// after a reboot we should boot on the new rootfs

	Ok(())
}

fn write_gpt_to_new_disk(install_disk: &mut Disk, new_disk: &mut Disk) -> io::Result<()> {
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
		.ok_or_else(|| io_other("could not get data partition"))?;
	data.part_guid = Uuid::new_v4();
	data.first_lba = root_b.last_lba + 1;
	data.last_lba = data.first_lba + data_lba - 1;
	data.name = "data".into();

	let mut map = BTreeMap::new();
	map.insert(1, boot);
	map.insert(2, root_a);
	map.insert(3, root_b);
	map.insert(4, data);

	disk.update_partitions(map)?;

	disk.write_inplace()?;

	new_disk.gpt_disk = Some(disk);

	Ok(())
}

///
/// This function expects
/// all new partitions to be bigger or the same size as the previous ones
fn copy_to_new_disk(install_disk: &mut Disk, new_disk: &mut Disk) -> io::Result<()> {

	let old_sector_size = install_disk.sector_size()
		.ok_or_else(|| io_other("could not get sector_size"))?;
	let new_sector_size = new_disk.sector_size()
		.ok_or_else(|| io_other("could not get sector_size"))?;

	// copy boot to new boot
	let old_boot = install_disk.get_part("boot")
		.ok_or_else(|| io_other("could not get old boot partition"))?;
	let new_boot = new_disk.get_part("boot")
		.ok_or_else(|| io_other("could not get new boot partition"))?;

	let old_first_byte = old_boot.first_lba * old_sector_size;
	let new_first_byte = new_boot.first_lba * new_sector_size;
	let length = (old_boot.last_lba + 1 - old_boot.first_lba) * old_sector_size;
	copy_len_to_new(install_disk, old_first_byte, length, new_disk, new_first_byte)?;


	// copy rootfs to new rootfs
	let old_root = install_disk.get_part("root")
		.ok_or_else(|| io_other("could not get old root partition"))?;
	let new_root = new_disk.get_part("root a")
		.ok_or_else(|| io_other("could not get new root a partition"))?;

	let old_first_byte = old_root.first_lba * old_sector_size;
	let new_first_byte = new_root.first_lba * new_sector_size;
	let length = (old_root.last_lba + 1 - old_root.first_lba) * old_sector_size;
	copy_len_to_new(install_disk, old_first_byte, length, new_disk, new_first_byte)?;

	// since the data filesystem is mounted rw
	// and /var can write to it
	// we need to copy the files manually
	// for this we need to first create a filesystem
	// 
	let data_path = new_disk.part_path("data")
		.ok_or_else(|| io_other("could not data path"))?;

	// create data filesystem
	Command::new("mkfs")
		.args(&["-F", "-t", "ext4"])
		.arg(&data_path)
		.exec()?;

	mount(&data_path, "/mnt")?;

	// now we need to copy everything
	// let's use the cp command
	cp("/data/home", "/mnt/")?;
	create_dir_all("/mnt/etc/ssh")?;

	umount(&data_path)?;

	Ok(())
}

// length should be div by 4096
fn copy_len_to_new(
	install_disk: &mut Disk,
	install_first_byte: u64,
	length: u64,
	new_disk: &mut Disk,
	new_first_byte: u64
) -> io::Result<()> {

	let mut install = install_disk.readable_file()?;
	let mut new = new_disk.writable_file()?;

	let mut buf = [0; 4096];
	let mut read = 0u64;

	// seek to the correct locations
	install.seek(SeekFrom::Start(install_first_byte))?;
	new.seek(SeekFrom::Start(new_first_byte))?;

	loop {

		let rem = (length - read).min(buf.len() as u64) as usize;

		if rem == 0 {
			break
		}

		// fill buffer
		let read_b = install.read(&mut buf[..rem])?;
		if read_b == 0 {
			return Err(io_other("returned 0 bytes but did not read all"))
		}

		// this is just an info
		if rem != read_b {
			println!("could not fill entire buffer expected {} filled {}", rem, read_b);
		}

		read += read_b as u64;

		new.write_all(&buf[..read_b])?;
	}

	Ok(())
}

fn configure_disk(disk: &mut Disk) -> io::Result<()> {

	// update fstab to with the new uuid

	let root_path = disk.part_path("root a")
		.ok_or_else(|| io_other("could not get root path"))?;

	// now replace DATA_UUID with the uuid
	let data_uuid = disk.get_part("data")
		.ok_or_else(|| io_other("could not get data partition"))?
		.part_guid.to_string();

	mount(&root_path, "/mnt")?;
	let fstab = read_to_string("/mnt/etc/fstab.templ")?;
	let fstab = fstab.replace("DATA_UUID", &data_uuid);
	fs::write("/mnt/etc/fstab", fstab)?;

	umount(&root_path)?;

	// update grub

	let boot_path = disk.part_path("boot")
		.ok_or_else(|| io_other("could not get boot path"))?;

	let root_uuid = disk.get_part("root a")
		.ok_or_else(|| io_other("could not get root partition"))?
		.part_guid.to_string();

	mount(&boot_path, "/mnt")?;
	let grub = read_to_string("/mnt/EFI/BOOT/grub.templ")?;
	let grub = grub.replace("ROOTFS_UUID", &root_uuid);
	fs::write("/mnt/EFI/BOOT/grub.tmp", grub)?;
	fs::rename("/mnt/EFI/BOOT/grub.tmp", "/mnt/EFI/BOOT/grub.cfg")?;

	Ok(())
}

fn mount(path: impl AsRef<Path>, dest: impl AsRef<Path>) -> io::Result<()> {
	let dest = dest.as_ref();
	// first unmount
	// but ignore the result since it returns an error of nothing is mounted
	let _ = umount(dest);
	Command::new("mount")
		.arg(path.as_ref())
		.arg(dest)
		.exec()
}

fn umount(path: impl AsRef<Path>) -> io::Result<()> {
	Command::new("umount")
		.arg("-f")
		.arg(path.as_ref())
		.exec()
}

fn cp(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
	Command::new("cp")
		.args(&["-r", "-p"])
		.arg(from.as_ref())
		.arg(to.as_ref())
		.exec()
}