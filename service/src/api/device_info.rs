
use crate::Bootloader;

use std::io;

use api::requests::device::{
	DeviceInfo, CpuLoad, MemoryUsage, ActiveDisk, DataDisk
};

use linux_info::system::{LoadAvg, Uptime};
use linux_info::cpu::Cpu;
use linux_info::memory::Memory;
use linux_info::storage::MountPoints;
use linux_info::unit::{DataSizeUnit};

fn other_err(s: &'static str) -> io::Error {
	io::Error::new(io::ErrorKind::Other, s)
}

// read device infos
pub async fn read(bootloader: &Bootloader) -> io::Result<DeviceInfo> {
	// read cpu stuff
	let cpu = Cpu::read()?;

	let load_avg = LoadAvg::read()?;
	let threads = load_avg.threads()
		.ok_or_else(|| other_err("threads not found"))?;
	let load_avg = load_avg.average()
		.ok_or_else(|| other_err("average not found"))?;

	let uptime = Uptime::read()?;
	let idletime = uptime.idletime()
		.ok_or_else(|| other_err("idletime not found"))?;
	let uptime = uptime.uptime()
		.ok_or_else(|| other_err("uptime not found"))?;

	let cpu = CpuLoad {
		cores: cpu.cores(),
		load_avg_1min: load_avg.0,
		load_avg_5min: load_avg.1,
		load_avg_15min: load_avg.2,
		running_threads: threads.0,
		runnable_threads: threads.1,
		uptime: uptime.as_secs(),
		idletime: idletime.as_secs()
	};

	// read memory
	let memory = Memory::read()?;
	let mem_total = memory.total_memory()
		.ok_or_else(|| other_err("total memory not found"))?;
	let mem_avail = memory.available_memory()
		.ok_or_else(|| other_err("available memory not found"))?;

	let memory = MemoryUsage {
		total: mem_total.to(&DataSizeUnit::B) as u64,
		available: mem_avail.to(&DataSizeUnit::B) as u64
	};

	// read disk
	let disks = bootloader.disks().await
		.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
	let active_disk = disks.into_iter().find(|d| d.active)
		.ok_or_else(|| other_err("active disk not found"))?;
	let active_disk = ActiveDisk {
		name: active_disk.name,
		size: active_disk.size
	};

	let mounts = MountPoints::read()?;
	let data_mount = mounts.points()
		.find(|m| matches!(m.mount_point(), Some("/data")))
		.ok_or_else(|| other_err("data partition not found"))?;
	let data_stat = data_mount.stats()?;
	let data_total = data_stat.total()
		.ok_or_else(|| other_err("total disk size not found"))?;
	let data_used = data_stat.used()
		.ok_or_else(|| other_err("used disk size not found"))?;

	let data = DataDisk {
		total: data_total.to(&DataSizeUnit::B) as u64,
		used: data_used.to(&DataSizeUnit::B) as u64
	};

	Ok(DeviceInfo { cpu, memory, active_disk, data })
}