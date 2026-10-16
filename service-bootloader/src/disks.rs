use crate::command::Command;
use crate::io_other;
use crate::layout::{self, Plan, ALIGNMENT};
use crate::util::{boot_image, cp, list_files, mount, root_uuid, umount};
use crate::version_info::update_version_info;

use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::fmt::Write as FmtWrite;
use std::fs::{self, create_dir_all, read_to_string, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use gpt::disk::LogicalBlockSize;
use gpt::header::{Header, HeaderBuilder};
use gpt::mbr::{MBRError, ProtectiveMBR};
use gpt::partition::Partition as GptPartition;
use gpt::{GptConfig, GptDisk, GptError};
use linux_info::storage::sector_size;

use uuid::Uuid;

use bootloader_api::requests::{Architecture, Disk as ApiDisk, VersionInfo};
use rand::RngCore;

// the size of the boot partition for new installs (128M - 32K)
//
// the boot partition of the install disk gets copied into it, so it needs to
// be at least as big as the one in genimage-efi.cfg and genimage-uboot.cfg
//
// devices installed before 2026.10 have 52_396_032 bytes (50M - 32K) which
// limits the kernel to 20mb (bzImage and bzImageB need to fit), pack-image
// enforces that limit
const BOOT_SIZE: u64 = 134_184_960;

// the size of each root partition for new installs (1.5GiB)
//
// devices installed before 2026.10 have 500MiB and get migrated to 1000MiB
// on their next update (see layout.rs), pack-image limits the rootfs to the
// smaller size
//
// in bytes
// is divisable by 512 and 4096
const ROOTFS_SIZE: u64 = 1536 * 1024 * 1024;

#[cfg(target_arch = "x86_64")]
const IMAGE_NAME: &str = "bzImage";
#[cfg(target_arch = "x86_64")]
const IMAGE_NAME_B: &str = "bzImageB";

#[cfg(target_arch = "aarch64")]
const IMAGE_NAME: &str = "Image.gz";
#[cfg(target_arch = "aarch64")]
const IMAGE_NAME_B: &str = "ImageB.gz";

pub fn api_disks() -> io::Result<Vec<ApiDisk>> {
	let disks = Disks::read()?;

	let list = disks
		.inner
		.into_iter()
		.map(|(name, disk)| {
			ApiDisk {
				name,
				active: disk.is_root,
				initialized: disk.gpt_disk.is_some(),
				// there exist devices which are block devices but don't have a
				// file in /dev/.. (or maybe a driver is missing)
				// we don't skip those devices but set the size to 0
				size: disk.size().unwrap_or(0),
			}
		})
		.collect();

	Ok(list)
}

enum NewDisk {
	New(Disk),
	Active,
}

pub fn install_on(name: String) -> Result<(), InstallError> {
	let mut active = None;
	let mut new = None;

	let disks = Disks::read()?;

	for (disk_name, disk) in disks.inner {
		match (disk.is_root, name == disk_name) {
			(true, true) => {
				active = Some(disk);
				new = Some(NewDisk::Active);
			}
			(true, false) => {
				active = Some(disk);
			}
			(false, true) => {
				new = Some(NewDisk::New(disk));
			}
			_ => {}
		}
	}

	let (mut active, new) = active
		.and_then(|a| new.map(|b| (a, b)))
		.ok_or_else(|| io_other("active or new disk not found"))?;

	match new {
		NewDisk::New(mut disk) => {
			install_to_new_disk(&mut active, &mut disk)?;
			Ok(())
		}
		NewDisk::Active => todo!("not allowed"),
	}
}

macro_rules! try_cont {
	($ex:expr) => {
		match $ex {
			Some(o) => o,
			None => continue,
		}
	};
}

#[derive(Debug)]
struct Disks {
	// the path (sda, sdb)
	inner: HashMap<String, Disk>,
}

impl Disks {
	pub fn read() -> io::Result<Self> {
		let mut list = HashMap::new();

		// first get every disk
		let disks = list_files("/sys/block")?;

		for disk_path in disks {
			let name = try_cont!(disk_path.file_name());
			let name = try_cont!(name.to_str());
			// raspberry has disks that are named like ram1 ...
			if name.starts_with("loop") || name.starts_with("ram") {
				continue;
			}

			let name = name.to_string();

			let disk = Disk::new(&name);

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
		disks
			.inner
			.into_iter()
			.find_map(|(_, disk)| disk.is_root.then(|| disk))
			.ok_or_else(|| io_other("root disk not found"))
	}
}

#[derive(Debug)]
struct Disk {
	path: PathBuf,
	gpt_disk: Option<GptDisk<File>>,
	is_root: bool,
}

impl Disk {
	pub fn new(name: &str) -> Self {
		let path = Path::new("/dev").join(name);

		let mut me = Self {
			path,
			gpt_disk: None,
			is_root: false,
		};

		if let Err(e) = me.open_gpt() {
			eprintln!("could not open gpt on {:?} with {:?}", name, e);
		}

		// if necessary should load block_size (so we can show binary size)
		me
	}

	fn open_gpt(&mut self) -> Result<(), GptError> {
		let disk = GptConfig::new().open(&self.path)?;

		self.gpt_disk = Some(disk);
		Ok(())
	}

	fn set_root(&mut self, uuid: &Uuid) -> bool {
		let disk = match &self.gpt_disk {
			Some(d) => d,
			None => return false,
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
		self.gpt_disk
			.as_ref()
			.map(|d| (*d.logical_block_size()).into())
	}

	pub fn read_mbr(&self) -> Result<ProtectiveMBR, MBRError> {
		let mut file = File::open(&self.path)?;
		ProtectiveMBR::from_disk(&mut file, LogicalBlockSize::Lb512)
	}

	pub fn readable_file(&self) -> io::Result<File> {
		fs::OpenOptions::new().read(true).open(&self.path)
	}

	pub fn writable_file(&self) -> io::Result<File> {
		fs::OpenOptions::new()
			.read(true)
			.write(true)
			.open(&self.path)
	}

	pub fn get_part(&self, name: &str) -> Option<&GptPartition> {
		self.gpt_disk
			.as_ref()?
			.partitions()
			.values()
			.find(|v| v.name == name)
	}

	pub fn clone_part(&self, name: &str) -> Option<GptPartition> {
		self.get_part(name).map(Clone::clone)
	}

	pub fn part_path(&self, name: &str) -> Option<PathBuf> {
		// get partition number
		let uuid = self
			.gpt_disk
			.as_ref()?
			.partitions()
			.values()
			.find(|v| v.name == name)
			.map(|v| v.part_guid)?;

		Some(Path::new("/dev/disk/by-partuuid").join(uuid.to_string()))
	}

	/// the logical block size the gpt uses
	pub fn lb_size(&self) -> io::Result<LogicalBlockSize> {
		self.gpt_disk
			.as_ref()
			.map(|d| *d.logical_block_size())
			.ok_or_else(|| io_other("no gpt"))
	}

	/// Returns the device of a partition
	///
	/// Partitions added while running might not have a /dev/disk/by-partuuid
	/// link yet, in that case the kernel partition with the same start and
	/// size is used.
	pub fn part_device(&self, name: &str) -> io::Result<PathBuf> {
		let path = self
			.part_path(name)
			.ok_or_else(|| io_other(format!("{name} partition not found")))?;
		if path.exists() {
			return Ok(path);
		}

		let part = self.get_part(name).unwrap();
		find_kernel_partition(&self.path, self.lb_size()?.as_u64(), part)?
			.map(|kp| Path::new("/dev").join(kp.name))
			.ok_or_else(|| io_other(format!("no device for {name}")))
	}

	pub fn size(&self) -> io::Result<u64> {
		let mut file = File::open(&self.path)?;
		file.seek(SeekFrom::Start(0))?;
		let len = file.seek(SeekFrom::End(0))?;
		Ok(len)
	}
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
	#[error("partition not found {0}")]
	PartitionNotFound(&'static str),

	#[error("io error {0}")]
	Io(#[from] io::Error),

	#[error("reading or writing the mbr failed {0}")]
	Mbr(#[from] MBRError),

	#[error("writing or reading the gpt failed {0}")]
	Gpt(#[from] GptError),
}

/// writes a new partition
fn install_to_new_disk(
	install_disk: &mut Disk,
	new_disk: &mut Disk,
) -> Result<(), InstallError> {
	// the boot and root partition get copied, so they need to fit
	let lb_size = install_disk
		.lb_size()
		.map_err(|_| io_other("could not get sector_size"))?;
	for (name, max) in [("boot", BOOT_SIZE), ("root", ROOTFS_SIZE)] {
		let len = install_disk
			.get_part(name)
			.ok_or(InstallError::PartitionNotFound(name))?
			.bytes_len(lb_size)?;
		if len > max {
			return Err(io_other(format!(
				"{name} partition with {len} bytes is bigger than {max} bytes"
			))
			.into());
		}
	}

	// do we need to write the entire drive
	// or is it enough to
	write_gpt_to_new_disk(install_disk, new_disk)?;

	// wait until linux reads the new gpt table
	thread::sleep(Duration::from_secs(2));
	udev_settle();

	copy_to_new_disk(install_disk, new_disk)?;

	configure_disk(new_disk)?;

	// wait until really all files are written to disk
	thread::sleep(Duration::from_secs(10));

	// success
	// after a reboot we should boot on the new rootfs

	Ok(())
}

/// Creates a gpt table on the new disk
///
/// Using the install disk as a template
fn write_gpt_to_new_disk(
	install_disk: &mut Disk,
	new_disk: &mut Disk,
) -> Result<(), InstallError> {
	// delete previous gpt if it exists
	new_disk.gpt_disk = None;

	let sector_size = new_disk.raw_sector_size()?;
	let lbs = LogicalBlockSize::try_from(sector_size)?;

	// create mbr but don't write it
	{
		let prev_mbr = install_disk.read_mbr()?;

		let len = new_disk.size()?;
		let sectors = len
			.checked_div(sector_size)
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
		.change_partition_count(true)
		.logical_block_size(lbs)
		.create(&new_disk.path)?;

	// remove any previous partitions
	// and set headers
	disk.update_partitions(BTreeMap::new())?;
	let header = disk.primary_header().unwrap();

	// now add the boot partition
	let mut boot = install_disk
		.clone_part("boot")
		.ok_or_else(|| InstallError::PartitionNotFound("boot"))?;
	boot.part_guid = Uuid::new_v4();
	let boot_sectors = BOOT_SIZE / sector_size;
	boot.first_lba = header.first_usable;
	boot.last_lba = boot.first_lba + boot_sectors - 1; // -1 because inclusive

	let rootfs_sectors = ROOTFS_SIZE / sector_size;
	let align = (ALIGNMENT / sector_size).max(1);

	// now create first root fs partition
	let mut root_a = install_disk
		.clone_part("root")
		.ok_or_else(|| InstallError::PartitionNotFound("root"))?;
	root_a.part_guid = Uuid::new_v4();
	root_a.first_lba = (boot.last_lba + 1).div_ceil(align) * align;
	root_a.last_lba = root_a.first_lba + rootfs_sectors - 1;
	root_a.name = "root a".into();

	// now create second root fs partition
	let mut root_b = root_a.clone();
	root_b.part_guid = Uuid::new_v4();
	root_b.first_lba = root_a.last_lba + 1;
	root_b.last_lba = root_b.first_lba + rootfs_sectors - 1;
	root_b.name = "root b".into();

	// data partition
	if root_b.last_lba >= header.last_usable {
		return Err(io_other("disk is to small").into());
	}
	let data_lba = (header.last_usable - root_b.last_lba) / 2;
	let mut data = install_disk
		.clone_part("data")
		.ok_or_else(|| InstallError::PartitionNotFound("data"))?;
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
fn copy_to_new_disk(
	install_disk: &mut Disk,
	new_disk: &mut Disk,
) -> Result<(), InstallError> {
	// this should never fail if install_disk contains a gpt Disk
	let old_sector_size = install_disk
		.sector_size()
		.ok_or_else(|| io_other("could not get sector_size"))?;
	// this should never fail if new_disk contains a gpt Disk
	let new_sector_size = new_disk
		.sector_size()
		.ok_or_else(|| io_other("could not get sector_size"))?;

	// copy boot to new boot
	let old_boot = install_disk
		.get_part("boot")
		.ok_or_else(|| InstallError::PartitionNotFound("old boot"))?;
	let new_boot = new_disk
		.get_part("boot")
		.ok_or_else(|| InstallError::PartitionNotFound("new boot"))?;

	let old_first_byte = old_boot.first_lba * old_sector_size;
	let new_first_byte = new_boot.first_lba * new_sector_size;
	let length = (old_boot.last_lba + 1 - old_boot.first_lba) * old_sector_size;
	copy_len_to_new(
		install_disk,
		old_first_byte,
		length,
		new_disk,
		new_first_byte,
	)?;

	// copy rootfs to new rootfs
	let old_root = install_disk
		.get_part("root")
		.ok_or_else(|| InstallError::PartitionNotFound("old root"))?;
	let new_root = new_disk
		.get_part("root a")
		.ok_or_else(|| InstallError::PartitionNotFound("new root a"))?;

	let old_first_byte = old_root.first_lba * old_sector_size;
	let new_first_byte = new_root.first_lba * new_sector_size;
	let length = (old_root.last_lba + 1 - old_root.first_lba) * old_sector_size;
	copy_len_to_new(
		install_disk,
		old_first_byte,
		length,
		new_disk,
		new_first_byte,
	)?;

	// writing to the disk makes udev reread the partition table, which
	// recreates the /dev/disk/by-partuuid links
	udev_settle();

	// since the data filesystem is mounted rw
	// and /var can write to it
	// we need to copy the files manually
	// for this we need to first create a filesystem
	//
	let data_path = new_disk
		.part_path("data")
		.ok_or_else(|| InstallError::PartitionNotFound("data"))?;

	// wait max 10s until the path exists
	// todo this can probably be removed
	for _ in 0..10 {
		thread::sleep(Duration::from_secs(1));
		if data_path.exists() {
			break;
		}
	}

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
	new_first_byte: u64,
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
			break;
		}

		// fill buffer
		let read_b = install.read(&mut buf[..rem])?;
		if read_b == 0 {
			return Err(io_other("returned 0 bytes but did not read all"));
		}

		// this is just an info
		if rem != read_b {
			eprintln!(
				"could not fill entire buffer expected {} filled {}",
				rem, read_b
			);
		}

		read += read_b as u64;

		new.write_all(&buf[..read_b])?;
	}

	new.flush()?;

	Ok(())
}

/// Waits until udev handled all events
///
/// After a write to a whole disk is closed, udev rereads its partition table
/// and removes and recreates the partition links in /dev/disk, on a slow
/// device this can take seconds.
fn udev_settle() {
	// let udev notice the closed file first
	thread::sleep(Duration::from_secs(1));

	let r = Command::new("udevadm")
		.args(&["settle", "--timeout=60"])
		.exec();
	if let Err(e) = r {
		eprintln!("udevadm settle failed {e}");
	}
}

/// Creates a ramdom machine id which can be stored in
/// /etc/machine-id
fn random_machine_id() -> String {
	let mut bytes = [0u8; 16];
	rand::thread_rng().fill_bytes(&mut bytes);

	let mut s = String::with_capacity(32);
	for b in bytes {
		write!(s, "{b:02x}").unwrap();
	}

	s
}

/// configure the newly installed disk
fn configure_disk(disk: &mut Disk) -> Result<(), InstallError> {
	// update fstab to with the new uuid
	let root_path = disk
		.part_path("root a")
		.ok_or_else(|| InstallError::PartitionNotFound("root a"))?;

	let boot_uuid = disk
		.get_part("boot")
		.ok_or_else(|| InstallError::PartitionNotFound("boot"))?
		.part_guid
		.to_string();

	let data_uuid = disk
		.get_part("data")
		.ok_or_else(|| InstallError::PartitionNotFound("data"))?
		.part_guid
		.to_string();

	mount(&root_path, "/mnt")?;
	let fstab = read_to_string("/mnt/etc/fstab.templ")?;
	let fstab = fstab.replace("EFI_UUID", &boot_uuid);
	let fstab = fstab.replace("DATA_UUID", &data_uuid);
	fs::write("/mnt/etc/fstab", fstab)?;

	// since we are on a newly installed disk
	// let's create a new machine-id
	fs::write("/mnt/etc/machine-id", format!("{}\n", random_machine_id()))
		.map_err(|_| io_other("could not write the machine-id"))?;

	umount(&root_path)?;

	// update grub

	let boot_path = disk
		.part_path("boot")
		.ok_or_else(|| InstallError::PartitionNotFound("boot"))?;

	let root_uuid = disk
		.get_part("root a")
		.ok_or_else(|| InstallError::PartitionNotFound("root a"))?
		.part_guid
		.to_string();

	mount(&boot_path, "/mnt")?;
	// update bootloader
	if Path::new("/mnt/EFI/BOOT/grub.templ").is_file() {
		// update grub
		let grub = read_to_string("/mnt/EFI/BOOT/grub.templ")?;
		let grub = grub.replace("ROOTFS_UUID", &root_uuid);
		let grub = grub.replace("KERNEL_NAME", IMAGE_NAME);
		fs::write("/mnt/EFI/BOOT/grub.tmp", grub)?;
		fs::rename("/mnt/EFI/BOOT/grub.tmp", "/mnt/EFI/BOOT/grub.cfg")?;
	} else if Path::new("/mnt/extlinux/extlinux.templ").is_file() {
		// is uboot
		let uboot = read_to_string("/mnt/extlinux/extlinux.templ")?;
		let uboot = uboot.replace("ROOTFS_UUID", &root_uuid);
		let uboot = uboot.replace("KERNEL_NAME", IMAGE_NAME);
		fs::write("/mnt/extlinux/extlinux.tmp", uboot)?;
		fs::rename(
			"/mnt/extlinux/extlinux.tmp",
			"/mnt/extlinux/extlinux.conf",
		)?;
	} else {
		return Err(io_other("bootloader not identified").into());
	}

	// update version info
	update_version_info()?;

	Ok(())
}

fn write_zeroes(file: &mut File, len: usize) -> io::Result<()> {
	const CHUNK_SIZE: usize = 4096;

	let full_chunks = len / CHUNK_SIZE;
	let rem = len % CHUNK_SIZE;

	let buf = [0u8; CHUNK_SIZE];
	for _ in 0..full_chunks {
		file.write_all(&buf)?;
	}

	if rem > 0 {
		file.write_all(&buf[..rem])?;
	}

	file.flush()?;

	Ok(())
}

// path to the folder
// ## Expects the current disk to be an installed disk
pub fn update(path: &str, version: &VersionInfo) -> io::Result<()> {
	let boot_img = boot_image()?;
	let part_uuid = root_uuid()?;

	let rootfs_path = format!("{path}/rootfs.ext2");
	let mut rootfs_file = File::open(&rootfs_path)?;
	let rootfs_len = rootfs_file.metadata()?.len();

	let mut disk = Disks::root_disk()?;

	// find out which partition to write to and migrate the partition layout
	// if needed see #22
	let other = prepare_partitions(&mut disk, &part_uuid, rootfs_len)?;

	// get all partitions

	let boot_uuid = disk
		.get_part("boot")
		.ok_or_else(|| io_other("boot partition not found"))?
		.part_guid
		.to_string();

	let other_part = disk
		.get_part(other)
		.ok_or_else(|| io_other(format!("{other} partition not found")))?;
	let other_uuid = other_part.part_guid;

	let data_uuid = disk
		.get_part("data")
		.ok_or_else(|| io_other("data partition not found"))?
		.part_guid
		.to_string();

	// get the partition we wan't to write the update to
	let part_path = disk.part_device(other)?;
	let mut part_file = OpenOptions::new().write(true).open(&part_path)?;

	// clear partition first
	let part_len = other_part.bytes_len(disk.lb_size()?)?;
	eprintln!("writing zeroes {part_len} to {part_path:?}");
	write_zeroes(&mut part_file, part_len as usize)?;

	part_file.seek(SeekFrom::Start(0))?;

	// copy the rootfs to the partition
	io::copy(&mut rootfs_file, &mut part_file)?;

	part_file.flush()?;
	part_file.sync_all()?;

	// update fstab file to mount the correct partitions
	mount(&part_path, "/mnt")?;
	let fstab = read_to_string("/mnt/etc/fstab.templ")?;
	let fstab = fstab.replace("EFI_UUID", &boot_uuid);
	let fstab = fstab.replace("DATA_UUID", &data_uuid);
	fs::write("/mnt/etc/fstab", fstab)?;

	let machine_id = match read_to_string("/etc/machine-id") {
		Ok(s) => s,
		Err(e) => {
			eprintln!("could not read machine-id {e}");
			format!("{}\n", random_machine_id())
		}
	};

	fs::write("/mnt/etc/machine-id", machine_id)
		.map_err(|_| io_other("could not write the machine-id"))?;

	umount(&part_path)?;

	let kernel_image_path = Path::new(path).join(IMAGE_NAME);

	let other_kernel = if boot_img.strip_prefix("/") == Some(IMAGE_NAME) {
		IMAGE_NAME_B
	} else if boot_img.strip_prefix("/") == Some(IMAGE_NAME_B) {
		IMAGE_NAME
	} else {
		return Err(io_other("kernel image not found"));
	};

	let other_path = format!("/boot/{other_kernel}");
	let _ = fs::remove_file(&other_path);

	copy_synced(&kernel_image_path, &other_path)?;

	// update bootloader
	match version.arch {
		Architecture::Amd64 => {
			let exists = Path::new("/boot/EFI/BOOT/grub.templ").is_file();
			if !exists {
				return Err(io::Error::new(
					io::ErrorKind::NotFound,
					"/boot/EFI/BOOT/grub.templ not found",
				));
			}

			// update bootloader
			let new_bootloader = Path::new(path).join("bootx64.efi");
			if new_bootloader.is_file() {
				copy_synced(new_bootloader, "/boot/EFI/BOOT/bootx64.efi.tmp")?;
				fs::rename(
					"/boot/EFI/BOOT/bootx64.efi.tmp",
					"/boot/EFI/BOOT/bootx64.efi",
				)?;
			}

			// update grub cfg template
			let new_grub_templ = Path::new(path).join("grub.templ");
			if new_grub_templ.is_file() {
				// first copy the file to the same mount point and the rename
				// atomic
				copy_synced(new_grub_templ, "/boot/EFI/BOOT/grub.templ.tmp")?;
				fs::rename(
					"/boot/EFI/BOOT/grub.templ.tmp",
					"/boot/EFI/BOOT/grub.templ",
				)?;
			}

			// update grub
			let grub = read_to_string("/boot/EFI/BOOT/grub.templ")?;
			let grub = grub.replace("ROOTFS_UUID", &&other_uuid.to_string());
			let grub = grub.replace("KERNEL_NAME", other_kernel);
			write_synced("/boot/EFI/BOOT/grub.tmp", grub)?;
			fs::rename("/boot/EFI/BOOT/grub.tmp", "/boot/EFI/BOOT/grub.cfg")?;
		}
		Architecture::Arm64 => {
			let exists = Path::new("/boot/extlinux/extlinux.templ").is_file();
			if !exists {
				return Err(io::Error::new(
					io::ErrorKind::NotFound,
					"/boot/extlinux/extlinux.templ not found",
				));
			}

			// update bootloader
			let new_bootloader = Path::new(path).join("u-boot.bin");
			if new_bootloader.is_file() {
				copy_synced(new_bootloader, "/boot/u-boot.bin.tmp")?;
				fs::rename("/boot/u-boot.bin.tmp", "/boot/u-boot.bin")?;
			}

			// update uboot cfg template
			let new_extlinux_templ = Path::new(path).join("extlinux.templ");
			if new_extlinux_templ.is_file() {
				// first copy the file to the same mount point and the rename
				// atomic
				copy_synced(
					new_extlinux_templ,
					"/boot/extlinux/extlinux.templ.tmp",
				)?;
				fs::rename(
					"/boot/extlinux/extlinux.templ.tmp",
					"/boot/extlinux/extlinux.templ",
				)?;
			}

			// is uboot
			let uboot = read_to_string("/boot/extlinux/extlinux.templ")?;
			let uboot = uboot.replace("ROOTFS_UUID", &&other_uuid.to_string());
			let uboot = uboot.replace("KERNEL_NAME", other_kernel);
			write_synced("/boot/extlinux/extlinux.tmp", uboot)?;
			fs::rename(
				"/boot/extlinux/extlinux.tmp",
				"/boot/extlinux/extlinux.conf",
			)?;
		}
	}

	unsafe {
		libc::sync();
	}

	// wait until really all files are written to disk
	// i just had a system after an update not restart
	// pulling the plug and powering it on again, everything works fine
	// maybe something was not fully readable or something???
	thread::sleep(Duration::from_secs(10));

	Ok(())
}

/// Copies a file and makes sure it is on disk
///
/// Files on the boot partition get renamed into place after this, the rename
/// must never reach the disk before the content.
fn copy_synced(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
	fs::copy(from, &to)?;
	File::open(to)?.sync_all()
}

/// Writes a file and makes sure it is on disk, see copy_synced
fn write_synced(
	path: impl AsRef<Path>,
	data: impl AsRef<[u8]>,
) -> io::Result<()> {
	fs::write(&path, data)?;
	File::open(path)?.sync_all()
}

/// Returns the name of the root partition the update should be written to.
///
/// If the device still has the old partition layout, the partition table gets
/// migrated first (see layout.rs). Nothing is changed if the rootfs would not
/// fit into the target partition.
fn prepare_partitions(
	disk: &mut Disk,
	running: &Uuid,
	rootfs_len: u64,
) -> io::Result<&'static str> {
	let lb_size = disk.lb_size()?;
	let gpt = disk
		.gpt_disk
		.as_ref()
		.ok_or_else(|| io_other("root disk has no gpt"))?;
	let header = gpt
		.primary_header()
		.or_else(|_| gpt.backup_header())
		.map_err(io_other)?;

	let plan = layout::plan(
		gpt.partitions(),
		lb_size.as_u64(),
		header.last_usable,
		running,
	)
	.map_err(io_other)?;

	let Plan {
		target,
		change,
		skipped,
	} = plan;

	if let Some(reason) = skipped {
		eprintln!("not migrating the partition layout: {reason}");
	}

	let partitions = change
		.as_ref()
		.map(|c| &c.partitions)
		.unwrap_or_else(|| gpt.partitions());
	let target_len = partitions
		.values()
		.find(|p| p.name == target)
		.ok_or_else(|| io_other(format!("{target} partition not found")))?
		.bytes_len(lb_size)?;

	if rootfs_len > target_len {
		return Err(io_other(format!(
			"rootfs of {rootfs_len} bytes does not fit into {target} \
			 with {target_len} bytes"
		)));
	}

	if let Some(change) = change {
		eprintln!(
			"changing the partition table, removing {:?} adding {:?}",
			change.removed, change.added
		);

		write_gpt_safely(&disk.path, &change.partitions)?;
		update_kernel_partitions(&disk.path, lb_size.as_u64(), &change)?;
		udev_settle();

		disk.open_gpt().map_err(io_other)?;
	}

	// if the last update was interrupted after changing the partition table
	// the kernel might still have the old partitions
	let part = disk
		.get_part(target)
		.ok_or_else(|| io_other(format!("{target} partition not found")))?;
	sync_kernel_partition(&disk.path, lb_size.as_u64(), part)?;

	Ok(target)
}

/// Writes a new partition table to a disk which might be in use.
///
/// The gpt crate writes both partition arrays before both headers, a power
/// loss in between would leave no valid table. Here the backup array and
/// header are written and synced first and then the primary ones. Linux and
/// UEFI use the backup if the primary is invalid, so there is always one valid
/// table.
fn write_gpt_safely(
	path: &Path,
	partitions: &BTreeMap<u32, GptPartition>,
) -> io::Result<()> {
	let gpt = GptConfig::new().open(path).map_err(io_other)?;
	let lb_size = *gpt.logical_block_size();
	let header = gpt
		.primary_header()
		.or_else(|_| gpt.backup_header())
		.map_err(io_other)?;

	let used: Vec<_> = partitions.values().filter(|p| p.is_used()).collect();
	if used.len() > header.num_parts as usize {
		return Err(io_other("to many partitions"));
	}

	let mut sorted = used.clone();
	sorted.sort_by_key(|p| p.first_lba);
	for (i, p) in sorted.iter().enumerate() {
		let overlaps = i > 0 && sorted[i - 1].last_lba >= p.first_lba;
		if p.first_lba < header.first_usable
			|| p.last_lba > header.last_usable
			|| p.first_lba > p.last_lba
			|| overlaps
		{
			return Err(io_other(format!("invalid partition {p:?}")));
		}
	}

	let mut primary = HeaderBuilder::from_header(header)
		.primary(true)
		.build(lb_size)
		.map_err(io_other)?;
	let mut backup = HeaderBuilder::from_header(&primary)
		.primary(false)
		.build(lb_size)
		.map_err(io_other)?;
	drop(gpt);

	let mut file = OpenOptions::new().read(true).write(true).open(path)?;

	write_gpt_copy(&mut file, &mut backup, &used, lb_size)?;
	write_gpt_copy(&mut file, &mut primary, &used, lb_size)?;

	Ok(())
}

/// writes the partition array and then the header of one gpt copy
fn write_gpt_copy(
	file: &mut File,
	header: &mut Header,
	used: &[&GptPartition],
	lb_size: LogicalBlockSize,
) -> io::Result<()> {
	for (i, p) in used.iter().enumerate() {
		p.write_to_device(
			file,
			i as u64,
			header.part_start,
			lb_size,
			header.part_size,
		)?;
	}
	GptPartition::write_zero_entries_to_device(
		file,
		used.len() as u64,
		header.num_parts as u64 - used.len() as u64,
		header.part_start,
		lb_size,
		header.part_size,
	)?;

	if header.current_lba < header.backup_lba {
		header.write_primary(file, lb_size).map_err(io_other)?;
	} else {
		header.write_backup(file, lb_size).map_err(io_other)?;
	}

	file.flush()?;
	file.sync_all()
}

// see linux/blkpg.h
const BLKPG: libc::c_ulong = 0x1269;
const BLKPG_ADD_PARTITION: libc::c_int = 1;
const BLKPG_DEL_PARTITION: libc::c_int = 2;

#[repr(C)]
struct BlkpgPartition {
	// in bytes
	start: libc::c_longlong,
	// in bytes
	length: libc::c_longlong,
	pno: libc::c_int,
	devname: [libc::c_char; 64],
	volname: [libc::c_char; 64],
}

#[repr(C)]
struct BlkpgIoctlArg {
	op: libc::c_int,
	flags: libc::c_int,
	datalen: libc::c_int,
	data: *mut libc::c_void,
}

fn blkpg(
	disk: &File,
	op: libc::c_int,
	pno: u32,
	start: u64,
	length: u64,
) -> io::Result<()> {
	let mut part = BlkpgPartition {
		start: start as _,
		length: length as _,
		pno: pno as _,
		devname: [0; 64],
		volname: [0; 64],
	};
	let mut arg = BlkpgIoctlArg {
		op,
		flags: 0,
		datalen: std::mem::size_of::<BlkpgPartition>() as _,
		data: &mut part as *mut BlkpgPartition as *mut _,
	};

	let r = unsafe { libc::ioctl(disk.as_raw_fd(), BLKPG as _, &mut arg) };
	if r == -1 {
		Err(io::Error::last_os_error())
	} else {
		Ok(())
	}
}

/// A partition as the kernel knows it
#[derive(Debug, Clone)]
struct KernelPartition {
	name: String,
	number: u32,
	// in 512 byte sectors
	start: u64,
	// in 512 byte sectors
	size: u64,
}

fn kernel_partitions(disk_path: &Path) -> io::Result<Vec<KernelPartition>> {
	let disk_name = disk_path
		.file_name()
		.and_then(|n| n.to_str())
		.ok_or_else(|| io_other("invalid disk path"))?;

	let read_num = |path: PathBuf| -> io::Result<u64> {
		read_to_string(path)?.trim().parse().map_err(io_other)
	};

	let mut list = vec![];
	for entry in fs::read_dir(Path::new("/sys/block").join(disk_name))? {
		let entry = entry?;
		let dir = entry.path();
		let Ok(number) = read_num(dir.join("partition")) else {
			continue;
		};

		list.push(KernelPartition {
			name: entry.file_name().to_string_lossy().into_owned(),
			number: number as u32,
			start: read_num(dir.join("start"))?,
			size: read_num(dir.join("size"))?,
		});
	}

	Ok(list)
}

/// Returns the kernel partition which matches the gpt partition exactly
fn find_kernel_partition(
	disk_path: &Path,
	lb_size: u64,
	part: &GptPartition,
) -> io::Result<Option<KernelPartition>> {
	let start = part.first_lba * lb_size / 512;
	let size = (part.last_lba + 1 - part.first_lba) * lb_size / 512;

	Ok(kernel_partitions(disk_path)?
		.into_iter()
		.find(|kp| kp.start == start && kp.size == size))
}

/// Makes sure the kernel knows the partition with its current size
///
/// Partitions which overlap with it get removed from the kernel (not from the
/// disk). This fails if one of them is in use.
fn sync_kernel_partition(
	disk_path: &Path,
	lb_size: u64,
	part: &GptPartition,
) -> io::Result<()> {
	if find_kernel_partition(disk_path, lb_size, part)?.is_some() {
		return Ok(());
	}

	let file = OpenOptions::new().read(true).open(disk_path)?;

	let start = part.first_lba * lb_size / 512;
	let end = (part.last_lba + 1) * lb_size / 512;
	for kp in kernel_partitions(disk_path)? {
		if kp.start < end && start < kp.start + kp.size {
			eprintln!("removing kernel partition {kp:?}");
			blkpg(&file, BLKPG_DEL_PARTITION, kp.number, 0, 0)?;
		}
	}

	let number = kernel_partitions(disk_path)?
		.iter()
		.map(|kp| kp.number)
		.max()
		.unwrap_or(0)
		+ 1;

	eprintln!("adding kernel partition {number} for {part:?}");
	blkpg(
		&file,
		BLKPG_ADD_PARTITION,
		number,
		part.first_lba * lb_size,
		(part.last_lba + 1 - part.first_lba) * lb_size,
	)
}

/// Tells the kernel about the changed partitions without rereading the
/// partition table, which is not possible while partitions are mounted.
fn update_kernel_partitions(
	disk_path: &Path,
	lb_size: u64,
	change: &layout::Change,
) -> io::Result<()> {
	let file = OpenOptions::new().read(true).open(disk_path)?;

	for p in &change.removed {
		if let Some(kp) = find_kernel_partition(disk_path, lb_size, p)? {
			eprintln!("removing kernel partition {kp:?}");
			blkpg(&file, BLKPG_DEL_PARTITION, kp.number, 0, 0)?;
		}
	}

	for p in &change.added {
		sync_kernel_partition(disk_path, lb_size, p)?;
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::layout::{OLD_ROOT_A, ROOT_A, ROOT_B};

	use gpt::partition_types;

	const OLD_BOOT_SIZE: u64 = 52_396_032;
	const OLD_ROOTFS_SIZE: u64 = 524_288_000;
	// an 8GB disk
	const DISK_SIZE: u64 = 15_633_408 * 512;

	struct TmpFile(PathBuf);

	impl Drop for TmpFile {
		fn drop(&mut self) {
			let _ = fs::remove_file(&self.0);
		}
	}

	/// creates a sparse file with the layout install_on created until 2026.10
	fn old_disk(name: &str) -> TmpFile {
		let path = std::env::temp_dir()
			.join(format!("iron-disks-{}-{name}.img", std::process::id()));
		let file = File::create(&path).unwrap();
		file.set_len(DISK_SIZE).unwrap();
		drop(file);
		let tmp = TmpFile(path);

		let mut file = OpenOptions::new()
			.read(true)
			.write(true)
			.open(&tmp.0)
			.unwrap();
		ProtectiveMBR::with_lb_size((DISK_SIZE / 512 - 1) as u32)
			.overwrite_lba0(&mut file)
			.unwrap();

		let mut disk = GptConfig::new()
			.writable(true)
			.change_partition_count(true)
			.logical_block_size(LogicalBlockSize::Lb512)
			.create(&tmp.0)
			.unwrap();
		disk.update_partitions(BTreeMap::new()).unwrap();
		let header = disk.primary_header().unwrap().clone();

		let part = |name: &str, first_lba: u64, len: u64| GptPartition {
			part_type_guid: partition_types::LINUX_FS,
			part_guid: Uuid::new_v4(),
			first_lba,
			last_lba: first_lba + len / 512 - 1,
			flags: 0,
			name: name.into(),
		};
		let boot = part("boot", header.first_usable, OLD_BOOT_SIZE);
		let a = part(ROOT_A, boot.last_lba + 1, OLD_ROOTFS_SIZE);
		let b = part(ROOT_B, a.last_lba + 1, OLD_ROOTFS_SIZE);
		let data_len = (header.last_usable - b.last_lba) / 2 * 512;
		let data = part("data", b.last_lba + 1, data_len);

		disk.update_partitions(BTreeMap::from([
			(1, boot),
			(2, a),
			(3, b),
			(4, data),
		]))
		.unwrap();
		disk.write_inplace().unwrap();

		tmp
	}

	fn read(path: &Path) -> GptDisk<File> {
		GptConfig::new()
			.only_valid_headers(true)
			.open(path)
			.unwrap()
	}

	/// the partitions sorted like they are written to disk
	fn entries(parts: &BTreeMap<u32, GptPartition>) -> Vec<GptPartition> {
		parts.values().filter(|p| p.is_used()).cloned().collect()
	}

	fn uuid(disk: &GptDisk<File>, name: &str) -> Uuid {
		disk.partitions()
			.values()
			.find(|p| p.name == name)
			.unwrap()
			.part_guid
	}

	fn plan_on(path: &Path, running: &Uuid) -> layout::Plan {
		let disk = read(path);
		let header = disk.primary_header().unwrap();
		layout::plan(disk.partitions(), 512, header.last_usable, running)
			.unwrap()
	}

	/// lets sfdisk validate the disk if it is installed
	fn verify_with_sfdisk(path: &Path) {
		let Ok(out) = std::process::Command::new("sfdisk")
			.arg("--verify")
			.arg(path)
			.output()
		else {
			eprintln!("sfdisk not found, skipping");
			return;
		};
		let stdout = String::from_utf8_lossy(&out.stdout);
		let stderr = String::from_utf8_lossy(&out.stderr);
		assert!(out.status.success(), "sfdisk: {stdout} {stderr}");
		assert!(!stderr.contains("corrupt"), "sfdisk: {stderr}");
	}

	#[test]
	fn migrate_on_disk() {
		let tmp = old_disk("migrate");
		verify_with_sfdisk(&tmp.0);
		let running = uuid(&read(&tmp.0), ROOT_A);

		// step 1
		let plan = plan_on(&tmp.0, &running);
		let change = plan.change.unwrap();
		write_gpt_safely(&tmp.0, &change.partitions).unwrap();

		let disk = read(&tmp.0);
		assert_eq!(entries(disk.partitions()), entries(&change.partitions));
		assert_eq!(
			disk.primary_header().unwrap().crc32_parts,
			disk.backup_header().unwrap().crc32_parts
		);
		verify_with_sfdisk(&tmp.0);

		// step 2, running from the new root b
		let new_b = uuid(&disk, ROOT_B);
		let plan = plan_on(&tmp.0, &new_b);
		assert_eq!(plan.target, ROOT_A);
		let change = plan.change.unwrap();
		write_gpt_safely(&tmp.0, &change.partitions).unwrap();

		let disk = read(&tmp.0);
		assert_eq!(entries(disk.partitions()), entries(&change.partitions));
		let names: Vec<_> =
			disk.partitions().values().map(|p| p.name.clone()).collect();
		assert_eq!(names, ["boot", ROOT_A, "data", ROOT_B]);
		verify_with_sfdisk(&tmp.0);
	}

	/// simulates a power loss after the backup copy was written
	#[test]
	fn power_loss_between_copies() {
		let tmp = old_disk("power-loss");
		let before = read(&tmp.0);
		let running = uuid(&before, ROOT_B);
		let plan = plan_on(&tmp.0, &running);
		let new = plan.change.unwrap().partitions;

		// only write the backup, like write_gpt_safely does first
		let header = before.primary_header().unwrap();
		let primary = HeaderBuilder::from_header(header)
			.primary(true)
			.build(LogicalBlockSize::Lb512)
			.unwrap();
		let mut backup = HeaderBuilder::from_header(&primary)
			.primary(false)
			.build(LogicalBlockSize::Lb512)
			.unwrap();
		let used: Vec<_> = new.values().filter(|p| p.is_used()).collect();
		let mut file = OpenOptions::new()
			.read(true)
			.write(true)
			.open(&tmp.0)
			.unwrap();
		write_gpt_copy(&mut file, &mut backup, &used, LogicalBlockSize::Lb512)
			.unwrap();

		// both headers are valid, the primary still has the old table
		let disk = read(&tmp.0);
		assert_eq!(entries(disk.partitions()), entries(before.partitions()));

		// planning again with the old table gives the same result
		let again = plan_on(&tmp.0, &running);
		assert_eq!(again.target, ROOT_B);
		assert!(again.change.is_some());

		// a torn primary header falls back to the backup with the new table
		file.seek(SeekFrom::Start(512)).unwrap();
		file.write_all(&[0xff; 64]).unwrap();
		file.sync_all().unwrap();
		let disk = GptConfig::new().open(&tmp.0).unwrap();
		assert!(disk.primary_header().is_err());
		let header = disk.backup_header().unwrap();
		let names: Vec<_> = gpt::partition::file_read_partitions(
			&mut File::open(&tmp.0).unwrap(),
			header,
			LogicalBlockSize::Lb512,
		)
		.unwrap()
		.into_values()
		.map(|p| p.name)
		.collect();
		assert_eq!(names, ["boot", OLD_ROOT_A, "old root b", "data", ROOT_B]);

		// and writing the full table again repairs it
		write_gpt_safely(&tmp.0, &new).unwrap();
		let disk = read(&tmp.0);
		assert_eq!(entries(disk.partitions()), entries(&new));
		verify_with_sfdisk(&tmp.0);
	}

	#[test]
	fn rejects_overlapping_partitions() {
		let tmp = old_disk("overlap");
		let disk = read(&tmp.0);
		let mut parts = disk.partitions().clone();
		parts.get_mut(&3).unwrap().first_lba -= 1;
		assert!(write_gpt_safely(&tmp.0, &parts).is_err());
		// nothing was written
		assert_eq!(
			entries(read(&tmp.0).partitions()),
			entries(disk.partitions())
		);
	}
}
