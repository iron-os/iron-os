//! Decides which root partition an image update is written to and migrates
//! devices installed with the old partition layout (see #22).
//!
//! Old layout (install_on before 2026.10):
//! `| boot 50M | root a 500M | root b 500M | data | free |`
//!
//! The data partition only got half of the remaining space, so the end of the
//! disk is free. The migration never touches data or the running root:
//!
//! 1. Running on the old root a or b: the old partitions are renamed to
//!    `old root a` and `old root b` and a new `root b` with the size of both
//!    together is added at the end of the disk. The update is written to the
//!    new `root b`.
//! 2. Running on the new `root b`: the old partitions are replaced by one
//!    `root a` spanning both and the update is written to it.
//!
//! `| boot 50M | root a (old a + b) | data | free | root b |`
//!
//! After every step the table is valid and the bootloader still points to a
//! working root, so a power loss at any point only repeats the step.

use std::collections::BTreeMap;
use std::fmt;

use gpt::partition::Partition;
use uuid::Uuid;

pub const ROOT_A: &str = "root a";
pub const ROOT_B: &str = "root b";
pub const OLD_ROOT_A: &str = "old root a";
pub const OLD_ROOT_B: &str = "old root b";

/// Root partitions smaller than this are from the old layout and get migrated.
///
/// The old layout used 500MiB, new installs use 1.5GiB and migrated devices
/// have two partitions with the size of the old a and b together.
pub const MIGRATE_BELOW_ROOTFS_SIZE: u64 = 1000 * 1024 * 1024;

/// New partitions are aligned to 1MiB.
pub const ALIGNMENT: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
	PartitionNotFound(&'static str),
	RunningRootNotFound,
	OldRootsNotAdjacent,
	InvalidPartition(String),
	/// the free space at the end of the disk is to small for the new root b
	NotEnoughSpace {
		needed: u64,
	},
}

impl fmt::Display for LayoutError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::PartitionNotFound(n) => write!(f, "partition {n} not found"),
			Self::RunningRootNotFound => {
				write!(f, "the running root is not a root partition")
			}
			Self::OldRootsNotAdjacent => {
				write!(f, "old root a and old root b are not adjacent")
			}
			Self::InvalidPartition(n) => write!(f, "invalid partition {n}"),
			Self::NotEnoughSpace { needed } => write!(
				f,
				"not enough free space at the end of the disk to migrate the \
				 partition layout, need {needed} bytes"
			),
		}
	}
}

impl std::error::Error for LayoutError {}

/// What needs to happen before the update can be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
	/// the name of the partition the update should be written to
	pub target: &'static str,
	/// if the partition table needs to change, the new table
	pub change: Option<Change>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
	/// the full new partition table
	pub partitions: BTreeMap<u32, Partition>,
	/// partitions the kernel needs to forget
	pub removed: Vec<Partition>,
	/// partitions the kernel needs to learn about
	pub added: Vec<Partition>,
}

/// Plans the next update.
///
/// - `lb_size`: the logical block size of the disk in bytes
/// - `last_usable`: the last usable lba of the gpt header
/// - `running`: the partition uuid of the running root
pub fn plan(
	partitions: &BTreeMap<u32, Partition>,
	lb_size: u64,
	last_usable: u64,
	running: &Uuid,
) -> Result<Plan, LayoutError> {
	let old_a = find(partitions, OLD_ROOT_A);
	let old_b = find(partitions, OLD_ROOT_B);

	match (old_a, old_b) {
		(Some(old_a), Some(old_b)) => {
			continue_migration(partitions, old_a, old_b, running)
		}
		(None, None) => {
			start_migration(partitions, lb_size, last_usable, running)
		}
		(Some(_), None) => Err(LayoutError::PartitionNotFound(OLD_ROOT_B)),
		(None, Some(_)) => Err(LayoutError::PartitionNotFound(OLD_ROOT_A)),
	}
}

fn find<'a>(
	partitions: &'a BTreeMap<u32, Partition>,
	name: &str,
) -> Option<(u32, &'a Partition)> {
	partitions
		.iter()
		.find(|(_, p)| p.name == name)
		.map(|(k, p)| (*k, p))
}

fn sectors(p: &Partition) -> Result<u64, LayoutError> {
	p.sectors_len()
		.map_err(|_| LayoutError::InvalidPartition(p.name.clone()))
}

fn start_migration(
	partitions: &BTreeMap<u32, Partition>,
	lb_size: u64,
	last_usable: u64,
	running: &Uuid,
) -> Result<Plan, LayoutError> {
	let (key_a, root_a) = find(partitions, ROOT_A)
		.ok_or(LayoutError::PartitionNotFound(ROOT_A))?;
	let (key_b, root_b) = find(partitions, ROOT_B)
		.ok_or(LayoutError::PartitionNotFound(ROOT_B))?;

	let other = if &root_a.part_guid == running {
		ROOT_B
	} else if &root_b.part_guid == running {
		ROOT_A
	} else {
		return Err(LayoutError::RunningRootNotFound);
	};

	let sectors_a = sectors(root_a)?;
	let sectors_b = sectors(root_b)?;

	let is_old_layout = root_a.last_lba + 1 == root_b.first_lba
		&& sectors_a == sectors_b
		&& sectors_a * lb_size < MIGRATE_BELOW_ROOTFS_SIZE;
	if !is_old_layout {
		return Ok(Plan {
			target: other,
			change: None,
		});
	}

	// the new root b gets the size of both old roots together, which is
	// the size root a will have after the migration
	//
	// the migration is required, images can be bigger than the old roots, so
	// the update fails if there is not enough space (shouldn't happen since
	// install_on only used half of the free space for data)
	let new_sectors = sectors_a + sectors_b;
	let not_enough_space = || LayoutError::NotEnoughSpace {
		needed: new_sectors * lb_size,
	};
	let align = (ALIGNMENT / lb_size).max(1);
	// exclusive end
	let end = (last_usable + 1) / align * align;
	let Some(first_lba) = end.checked_sub(new_sectors) else {
		return Err(not_enough_space());
	};
	let first_lba = first_lba / align * align;
	let last_lba = first_lba + new_sectors - 1;

	let used_until = partitions
		.values()
		.filter(|p| p.is_used())
		.map(|p| p.last_lba)
		.max()
		.unwrap_or(0);
	if first_lba <= used_until {
		return Err(not_enough_space());
	}

	let mut new = partitions.clone();

	let mut old_a = root_a.clone();
	old_a.name = OLD_ROOT_A.into();
	new.insert(key_a, old_a);

	let mut old_b = root_b.clone();
	old_b.name = OLD_ROOT_B.into();
	new.insert(key_b, old_b);

	let new_b = Partition {
		part_type_guid: root_b.part_type_guid.clone(),
		part_guid: Uuid::new_v4(),
		first_lba,
		last_lba,
		flags: root_b.flags,
		name: ROOT_B.into(),
	};
	let key = new.keys().max().copied().unwrap_or(0) + 1;
	new.insert(key, new_b.clone());

	Ok(Plan {
		target: ROOT_B,
		change: Some(Change {
			partitions: new,
			removed: vec![],
			added: vec![new_b],
		}),
	})
}

fn continue_migration(
	partitions: &BTreeMap<u32, Partition>,
	(key_a, old_a): (u32, &Partition),
	(key_b, old_b): (u32, &Partition),
	running: &Uuid,
) -> Result<Plan, LayoutError> {
	let (_, root_b) = find(partitions, ROOT_B)
		.ok_or(LayoutError::PartitionNotFound(ROOT_B))?;

	if find(partitions, ROOT_A).is_some() {
		return Err(LayoutError::InvalidPartition(ROOT_A.into()));
	}

	// step 1 did not boot into the new root b yet, so write it again
	if &old_a.part_guid == running || &old_b.part_guid == running {
		return Ok(Plan {
			target: ROOT_B,
			change: None,
		});
	}

	if &root_b.part_guid != running {
		return Err(LayoutError::RunningRootNotFound);
	}

	if old_a.last_lba + 1 != old_b.first_lba {
		return Err(LayoutError::OldRootsNotAdjacent);
	}

	// step 2: merge the old roots into the new root a
	let root_a = Partition {
		part_type_guid: old_a.part_type_guid.clone(),
		part_guid: Uuid::new_v4(),
		first_lba: old_a.first_lba,
		last_lba: old_b.last_lba,
		flags: old_a.flags,
		name: ROOT_A.into(),
	};

	let mut new = partitions.clone();
	new.remove(&key_b);
	new.insert(key_a, root_a.clone());

	Ok(Plan {
		target: ROOT_A,
		change: Some(Change {
			partitions: new,
			removed: vec![old_a.clone(), old_b.clone()],
			added: vec![root_a],
		}),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	use gpt::partition_types;

	const LB: u64 = 512;
	// an 8GB usb stick / ssd
	const DISK_SECTORS: u64 = 15_633_408;
	const LAST_USABLE: u64 = DISK_SECTORS - 34;
	const OLD_ROOT_SECTORS: u64 = 524_288_000 / LB;
	const BOOT_SECTORS: u64 = 52_396_032 / LB;

	fn part(name: &str, first_lba: u64, sectors: u64) -> Partition {
		Partition {
			part_type_guid: partition_types::LINUX_FS,
			part_guid: Uuid::new_v4(),
			first_lba,
			last_lba: first_lba + sectors - 1,
			flags: 0,
			name: name.into(),
		}
	}

	/// the layout install_on created until 2026.10
	fn old_layout(root_sectors: u64) -> BTreeMap<u32, Partition> {
		let boot = part("boot", 34, BOOT_SECTORS);
		let a = part(ROOT_A, boot.last_lba + 1, root_sectors);
		let b = part(ROOT_B, a.last_lba + 1, root_sectors);
		let data_sectors = (LAST_USABLE - b.last_lba) / 2;
		let data = part("data", b.last_lba + 1, data_sectors);

		BTreeMap::from([(1, boot), (2, a), (3, b), (4, data)])
	}

	fn guid(parts: &BTreeMap<u32, Partition>, name: &str) -> Uuid {
		find(parts, name).unwrap().1.part_guid
	}

	fn assert_valid(parts: &BTreeMap<u32, Partition>) {
		let mut list: Vec<_> = parts.values().collect();
		list.sort_by_key(|p| p.first_lba);
		for w in list.windows(2) {
			assert!(w[0].last_lba < w[1].first_lba, "overlap {w:?}");
		}
		for p in list {
			assert!(p.first_lba <= p.last_lba);
			assert!(p.last_lba <= LAST_USABLE);
		}
	}

	#[test]
	fn full_migration_from_root_a() {
		full_migration(ROOT_A);
	}

	#[test]
	fn full_migration_from_root_b() {
		full_migration(ROOT_B);
	}

	fn full_migration(running_name: &str) {
		let old = old_layout(OLD_ROOT_SECTORS);
		let running = guid(&old, running_name);

		// step 1
		let planned = plan(&old, LB, LAST_USABLE, &running).unwrap();
		assert_eq!(planned.target, ROOT_B);
		let change = planned.change.unwrap();
		let step1 = change.partitions;
		assert_valid(&step1);
		assert!(change.removed.is_empty());
		assert_eq!(change.added.len(), 1);

		// the old partitions keep their uuids, only the names change
		assert_eq!(guid(&step1, OLD_ROOT_A), guid(&old, ROOT_A));
		assert_eq!(guid(&step1, OLD_ROOT_B), guid(&old, ROOT_B));
		// data is untouched
		assert_eq!(find(&step1, "data"), find(&old, "data"));

		let (_, new_b) = find(&step1, ROOT_B).unwrap();
		assert_eq!(new_b, &change.added[0]);
		assert_eq!(sectors(new_b).unwrap(), 2 * OLD_ROOT_SECTORS);
		assert_eq!(new_b.first_lba % (ALIGNMENT / LB), 0);
		assert!(new_b.last_lba <= LAST_USABLE);
		assert_ne!(new_b.part_guid, guid(&old, ROOT_B));

		// power loss before booting the new root b: same target again
		let again = plan(&step1, LB, LAST_USABLE, &running).unwrap();
		assert_eq!(again.target, ROOT_B);
		assert_eq!(again.change, None);

		// step 2, booted into the new root b
		let running = new_b.part_guid;
		let planned = plan(&step1, LB, LAST_USABLE, &running).unwrap();
		assert_eq!(planned.target, ROOT_A);
		let change = planned.change.unwrap();
		let step2 = change.partitions;
		assert_valid(&step2);
		assert_eq!(change.removed.len(), 2);
		assert_eq!(step2.len(), 4);

		let (_, root_a) = find(&step2, ROOT_A).unwrap();
		let (_, root_b) = find(&step2, ROOT_B).unwrap();
		assert_eq!(root_a, &change.added[0]);
		assert_eq!(root_a.first_lba, find(&old, ROOT_A).unwrap().1.first_lba);
		assert_eq!(root_a.last_lba, find(&old, ROOT_B).unwrap().1.last_lba);
		assert_eq!(sectors(root_a).unwrap(), sectors(root_b).unwrap());
		assert_eq!(root_b, new_b);
		assert_eq!(find(&step2, "data"), find(&old, "data"));
		assert_eq!(find(&step2, "boot"), find(&old, "boot"));
		assert!(find(&step2, OLD_ROOT_A).is_none());
		assert!(find(&step2, OLD_ROOT_B).is_none());

		// after the migration it's a normal a/b layout
		let plan_b = plan(&step2, LB, LAST_USABLE, &running).unwrap();
		assert_eq!(
			plan_b,
			Plan {
				target: ROOT_A,
				change: None
			}
		);
		let plan_a = plan(&step2, LB, LAST_USABLE, &root_a.part_guid).unwrap();
		assert_eq!(
			plan_a,
			Plan {
				target: ROOT_B,
				change: None
			}
		);
	}

	#[test]
	fn odd_old_size_keeps_equal_sizes() {
		// install_on before september 2021
		let old = old_layout(500_002_816 / LB);
		let running = guid(&old, ROOT_A);

		let step1 = plan(&old, LB, LAST_USABLE, &running)
			.unwrap()
			.change
			.unwrap()
			.partitions;
		let (_, new_b) = find(&step1, ROOT_B).unwrap();
		assert_eq!(sectors(new_b).unwrap(), 2 * 500_002_816 / LB);

		let step2 = plan(&step1, LB, LAST_USABLE, &new_b.part_guid)
			.unwrap()
			.change
			.unwrap()
			.partitions;
		assert_valid(&step2);
		let (_, root_a) = find(&step2, ROOT_A).unwrap();
		assert_eq!(sectors(root_a).unwrap(), sectors(new_b).unwrap());

		// smaller than MIGRATE_BELOW_ROOTFS_SIZE but not adjacent anymore
		let planned = plan(&step2, LB, LAST_USABLE, &new_b.part_guid).unwrap();
		assert_eq!(planned.change, None);
	}

	#[test]
	fn new_install_is_not_migrated() {
		let sectors = 1536 * 1024 * 1024 / LB;
		let new = old_layout(sectors);
		let running = guid(&new, ROOT_A);
		let planned = plan(&new, LB, LAST_USABLE, &running).unwrap();
		assert_eq!(
			planned,
			Plan {
				target: ROOT_B,
				change: None
			}
		);
	}

	#[test]
	fn no_space_fails() {
		let mut old = old_layout(OLD_ROOT_SECTORS);
		// data takes the full disk
		old.get_mut(&4).unwrap().last_lba = LAST_USABLE;
		let running = guid(&old, ROOT_B);

		let err = plan(&old, LB, LAST_USABLE, &running).unwrap_err();
		assert_eq!(
			err,
			LayoutError::NotEnoughSpace {
				needed: 2 * OLD_ROOT_SECTORS * LB
			}
		);

		// a disk smaller than the new root b
		let err = plan(&old, LB, OLD_ROOT_SECTORS, &running).unwrap_err();
		assert!(matches!(err, LayoutError::NotEnoughSpace { .. }));
	}

	#[test]
	fn running_root_unknown() {
		let old = old_layout(OLD_ROOT_SECTORS);
		let err = plan(&old, LB, LAST_USABLE, &Uuid::new_v4()).unwrap_err();
		assert_eq!(err, LayoutError::RunningRootNotFound);

		let running = guid(&old, ROOT_A);
		let step1 = plan(&old, LB, LAST_USABLE, &running)
			.unwrap()
			.change
			.unwrap()
			.partitions;
		let err = plan(&step1, LB, LAST_USABLE, &Uuid::new_v4()).unwrap_err();
		assert_eq!(err, LayoutError::RunningRootNotFound);
	}

	#[test]
	fn four_k_sectors() {
		let lb = 4096;
		let last_usable = DISK_SECTORS / 8 - 6;
		let boot = part("boot", 6, 52_396_032 / lb);
		let a = part(ROOT_A, boot.last_lba + 1, 524_288_000 / lb);
		let b = part(ROOT_B, a.last_lba + 1, 524_288_000 / lb);
		let data = part("data", b.last_lba + 1, (last_usable - b.last_lba) / 2);
		let old = BTreeMap::from([(1, boot), (2, a), (3, b), (4, data)]);

		let running = guid(&old, ROOT_A);
		let step1 = plan(&old, lb, last_usable, &running)
			.unwrap()
			.change
			.unwrap()
			.partitions;
		let (_, new_b) = find(&step1, ROOT_B).unwrap();
		assert_eq!(new_b.first_lba % (ALIGNMENT / lb), 0);
		assert!(new_b.last_lba <= last_usable);
		assert_eq!(sectors(new_b).unwrap() * lb, 2 * 524_288_000);
	}
}
