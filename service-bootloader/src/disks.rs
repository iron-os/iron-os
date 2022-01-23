
use crate::io_other;
use crate::command::Command;
use crate::version_info::update_version_info;
use crate::util::{list_files, root_uuid, mount, cp, umount, boot_image};

use std::path::{Path, PathBuf};
use std::fs::{self, File, read_to_string, create_dir_all, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::time::Duration;
use std::thread;

use gpt::{GptConfig, GptDisk};
use gpt::partition::{Partition as GptPartition};
use gpt::mbr::ProtectiveMBR;
use gpt::disk::LogicalBlockSize;
use linux_info::storage::{sector_size};

use uuid::Uuid;

use bootloader_api::Disk as ApiDisk;

// the size is set in genimage-efi.cfg
// bzImage max size is 20m which allows to have a bzImage tmp
const BOOT_SIZE: u64 = 52_396_032;

// should create around 500mb
// that should be enough since our rootfs at the moment
// is 180mb
//
// in bytes
// is divisable by 512 and 4096
const ROOTFS_SIZE: u64 = 524_288_000;

#[cfg(target_arch = "x86_64")]
const IMAGE_NAME: &str = "bzImage";
#[cfg(target_arch = "x86_64")]
const IMAGE_NAME_B: &str = "bzImageB";

#[cfg(target_arch = "aarch64")]
const IMAGE_NAME: &str = "Image.gz";
#[cfg(target_arch = "aarch64")]
const IMAGE_NAME_B: &str = "ImageB.gz";

pub fn api_disks() -> io::Result<Vec<ApiDisk>> {
	let mut list = vec![];
	let disks = Disks::read()?;

	for (name, disk) in disks.inner {
		let api_disk = ApiDisk {
			name,
			active: disk.is_root,
			initialized: disk.gpt_disk.is_some(),
			size: disk.size()?
		};
		list.push(api_disk);
	}

	Ok(list)
}

enum NewDisk {
	New(Disk),
	Active
}

pub fn install_on(name: String) -> io::Result<()> {

	let mut active = None;
	let mut new = None;

	let disks = Disks::read()?;

	for (disk_name, disk) in disks.inner {
		match (disk.is_root, name == disk_name) {
			(true, true) => {
				active = Some(disk);
				new = Some(NewDisk::Active);
			},
			(true, false) => {
				active = Some(disk);
			},
			(false, true) => {
				new = Some(NewDisk::New(disk));
			},
			_ => {}
		}
	}

	let (mut active, new) = active.and_then(|a| new.map(|b| (a, b)))
		.ok_or_else(|| io_other("active or new disk not found"))?;

	match new {
		NewDisk::New(mut disk) => {
			install_to_new_disk(&mut active, &mut disk)?;
			Ok(())
		},
		NewDisk::Active => todo!("not allowed")
	}
}

macro_rules! try_cont {
	($ex:expr) => (match $ex {
		Some(o) => o,
		None => continue
	})
}

#[derive(Debug)]
struct Disks {
	// the path (sda, sdb)
	inner: HashMap<String, Disk>
}

impl Disks {

	pub fn read() -> io::Result<Self> {
		let mut list = HashMap::new();

		// first get every disk
		let disks = list_files("/sys/block")?;

		for disk_path in disks {
			let name = try_cont!(disk_path.file_name());
			let name = try_cont!(name.to_str());
			if name.starts_with("loop") {
				continue
			}

			let name = name.to_string();

			let disk = Disk::new(&name)?;

			list.insert(name, disk);

		}

		let mut me = Self { inner: list };

		me.set_root(&root_uuid()?);

		Ok(me)
	}

	fn set_root(&mut self, uuid: &Uuid) {
		for (_, disk) in &mut self.inner {
			disk.set_root(uuid);
		}
	}

	fn root_disk() -> io::Result<Disk> {
		let disks = Disks::read()?;
		disks.inner.into_iter()
			.find_map(|(_, disk)|
				disk.is_root
					.then(|| disk)
			)
			.ok_or_else(|| io_other("root disk not found"))
	}

}

#[derive(Debug)]
struct Disk {
	path: PathBuf,
	gpt_disk: Option<GptDisk<'static>>,
	is_root: bool
}

impl Disk {

	pub fn new(name: &str) -> io::Result<Self> {
		let path = Path::new("/dev").join(name);

		let mut me = Self {
			path,
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
		let uuid = self.gpt_disk.as_ref()?
			.partitions()
			.values()
			.find(|v| v.name == name)
			.map(|v| v.part_guid)?;

		Some(Path::new("/dev/disk/by-partuuid").join(uuid.to_string()))
	}

	pub fn size(&self) -> io::Result<u64> {
		let mut file = File::open(&self.path)?;
		file.seek(SeekFrom::Start(0))?;
		let len = file.seek(SeekFrom::End(0))?;
		Ok(len)
	}

}

/// writes a new partition
fn install_to_new_disk(install_disk: &mut Disk, new_disk: &mut Disk) -> io::Result<()> {
	// do we need to write the entire drive
	// or is it enough to 

	write_gpt_to_new_disk(install_disk, new_disk)?;

	thread::sleep(Duration::from_secs(2));

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

		let len = new_disk.size()?;
		let sectors = len.checked_div(sector_size)
			.ok_or_else(|| io_other("file len not % sector size"))?;

		let mut file = new_disk.writable_file()?;

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
		.ok_or_else(|| io_other("could not get data partition path"))?;

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
	cp("/data/packages", "/mnt/")?;

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
	let boot_uuid = disk.get_part("boot")
		.ok_or_else(|| io_other("could not get boot partition"))?
		.part_guid.to_string();

	// now replace DATA_UUID with the uuid
	let data_uuid = disk.get_part("data")
		.ok_or_else(|| io_other("could not get data partition"))?
		.part_guid.to_string();

	mount(&root_path, "/mnt")?;
	let fstab = read_to_string("/mnt/etc/fstab.templ")?;
	let fstab = fstab.replace("EFI_UUID", &boot_uuid);
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
	let grub = grub.replace("KERNEL_NAME", IMAGE_NAME);
	fs::write("/mnt/EFI/BOOT/grub.tmp", grub)?;
	fs::rename("/mnt/EFI/BOOT/grub.tmp", "/mnt/EFI/BOOT/grub.cfg")?;

	// update version info
	update_version_info()?;

	Ok(())
}

// path to the folder 
// ## Expects the current disk to be an installed disk
pub fn update(path: &str) -> io::Result<()> {
	let boot_img = boot_image()?;
	let part_uuid = root_uuid()?;

	let disk = Disks::root_disk()?;

	let boot_uuid = disk.get_part("boot")
		.ok_or_else(|| io_other("boot partition not found"))?
		.part_guid.to_string();

	let part_a = disk.get_part("root a")
		.ok_or_else(|| io_other("root a partition not found"))?;
	let part_b = disk.get_part("root b")
		.ok_or_else(|| io_other("root b partition not found"))?;

	let data_uuid = disk.get_part("data")
		.ok_or_else(|| io_other("data partition not found"))?
		.part_guid.to_string();

	let (other_uuid, other) = if part_a.part_guid == part_uuid {
		(part_b.part_guid, "root b")
	} else if part_b.part_guid == part_uuid {
		(part_a.part_guid, "root a")
	} else {
		return Err(io_other("root partition not found"))
	};

	let part_path = disk.part_path(other).unwrap();
	let mut part_file = OpenOptions::new()
		.write(true)
		.open(&part_path)?;

	let rootfs_path = format!("{}/rootfs.ext2", path);
	let mut rootfs_file = File::open(&rootfs_path)?;

	io::copy(&mut rootfs_file, &mut part_file)?;

	// rootfs copied

	mount(&part_path, "/mnt")?;
	let fstab = read_to_string("/mnt/etc/fstab.templ")?;
	let fstab = fstab.replace("EFI_UUID", &boot_uuid);
	let fstab = fstab.replace("DATA_UUID", &data_uuid);
	fs::write("/mnt/etc/fstab", fstab)?;

	umount(&part_path)?;

	let kernel_image_path = format!("{}/{}", path, IMAGE_NAME);

	let other_kernel = if boot_img.strip_prefix("/") == Some(IMAGE_NAME) {
		IMAGE_NAME_B
	} else if boot_img.strip_prefix("/") == Some(IMAGE_NAME_B) {
		IMAGE_NAME
	} else {
		return Err(io_other("kernel image not found"))
	};

	let other_path = format!("/boot/{}", other_kernel);
	let _ = fs::remove_file(&other_path);

	fs::copy(&kernel_image_path, &other_path)?;

	// now change the grub settings
	let grub = read_to_string("/boot/EFI/BOOT/grub.templ")?;
	let grub = grub.replace("KERNEL_NAME", other_kernel);
	let grub = grub.replace("ROOTFS_UUID", &other_uuid.to_string());
	fs::write("/boot/EFI/BOOT/grub.tmp", grub)?;
	fs::rename("/boot/EFI/BOOT/grub.tmp", "/boot/EFI/BOOT/grub.cfg")?;

	Ok(())
}