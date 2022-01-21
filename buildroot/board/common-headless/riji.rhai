
// target dir points to output/target
// binaries points to the images folder
fn post_build(host_dir, target_dir, binaries_dir) {

	// copy pam.d
	fs::copy("./rootfs/pam.d", target_dir + "/etc/");

	// copy dbus
	fs::copy("./rootfs/dbus/dbus.service", target_dir + "/usr/lib/systemd/user/");
	fs::copy("./rootfs/dbus/dbus.socket", target_dir + "/usr/lib/systemd/user/");

	// create symlinks to /systemd/user/sockets.target.wants/dbus.socket
	// create directory
	let socket_want = target_dir + "/usr/lib/systemd/user/sockets.target.wants";
	fs::create_dir(socket_want);

	let ln = cmd(["ln", "-sf", "../dbus.socket", "dbus.socket"]);
	ln.dir(socket_want);
	ln.execute();

	// copy service-bootloader
	let systemd_folder = target_dir + "/etc/systemd/system/";
	fs::copy("./rootfs/weston.service", systemd_folder);
	fs::copy("./rootfs/service-bootloader.service", systemd_folder);
	fs::copy("./rootfs/subprocess.service", systemd_folder);
	// symlink get's created in fakeroot
	fs::copy("./rootfs/chromium.service", systemd_folder);
	fs::copy("./rootfs/rules.d", target_dir + "/etc/udev/");
	fs::copy("./rootfs/weston", target_dir + "/etc/");

	// copy network
	fs::copy("./rootfs/network/gsm-connection.mconnection", target_dir + "/etc/NetworkManager/system-connections/");

	// create empty data folder (needed for fresh build since else device_table.mk failes)
	fs::create_dir(target_dir + "/data");

	// create boot dir
	fs::create_dir(target_dir + "/boot");

	if fs::is_dir(target_dir + "/etc/ssh") == true {

		// make open ssh work without rw
		let sshd_keygen = regex("ExecStartPre=.*");
		sshd_keygen.replace_file(
			target_dir + "/usr/lib/systemd/system/sshd.service",
			"ExecStartPre=/usr/bin/ssh-keygen -A -f /data"
		);

		let sshd_config = target_dir + "/etc/ssh/sshd_config";
		if fs::contains(sshd_config, "/data/etc") == false {
			fs::append(sshd_config, [
				"HostKey /data/etc/ssh/ssh_host_rsa_key",
				"HostKey /data/etc/ssh/ssh_host_ecdsa_key",
				"HostKey /data/etc/ssh/ssh_host_ed25519_key"
			]);
		}

	}

	let logind = target_dir + "/etc/systemd/logind.conf";
	if fs::contains(logind, "NAutoVTs=0") == false {
		fs::append(logind, [
			"NAutoVTs=0",
			"ReserveVT=0"
		]);
	}

	// patch journald
	let journald = target_dir + "/etc/systemd/journald.conf";
	if fs::contains(journald, "SystemMaxUse=40M") == false {
		fs::append(journald, [
			"SystemMaxUse=40M",
			"Compress=yes"
		]);
	}

}

fn fakeroot(host_dir, target_dir, binaries_dir) {
	// target dir is in ext2/target but we can't see that folder

	// create service-bootloader symlink
	let ln = cmd(["ln", "-sf", "../service-bootloader.service", "service-bootloader.service"]);
	ln.dir(target_dir + "/etc/systemd/system/multi-user.target.wants/");
	ln.execute();

	// copy data folder
	fs::copy("./data", target_dir + "/");

	// copy var folder
	fs::copy(target_dir + "/var", target_dir + "/data/");
	fs::delete(target_dir + "/var");

	// delete home folder
	fs::delete(target_dir + "/home");

	if fs::is_dir(binaries_dir + "/packages") == true {
		fs::delete(target_dir + "/data/packages");
		fs::copy(binaries_dir + "/packages", target_dir + "/data/packages");
		print("using existing packages");
	}

	// set corrects rights
	chown("14:15", target_dir + "/data/home");
	chown("0:0", target_dir + "/data/etc");
	chown("14:15", target_dir + "/data/packages");

	chmod("700", target_dir + "/etc/NetworkManager/system-connections");

	// execute gen_image
	fs::delete(binaries_dir + "/data.ext2");
	let mkfs = cmd([
		host_dir + "/sbin/mkfs.ext4",
		"-d", target_dir + "/data",
		// "-r", "0",
		"-N", "0",
		"-m", "5",
		"-L", "data",
		binaries_dir + "/data.ext2",
		"400M"
	]);
	mkfs.dir(binaries_dir);
	mkfs.execute();

	// check that lvm2-monitor.service does not exist
	if fs::is_file(target_dir + "/usr/lib/systemd/system/lvm2-monitor.service") == true {
		panic("lvm2 exists");
	}

	// cleanup
	fs::delete(target_dir + "/data");
	fs::create_dir(target_dir + "/data");
	fs::create_dir(target_dir + "/var");
	fs::create_dir(target_dir + "/home");

	let data_uuid = fs_uuid(host_dir, binaries_dir + "/data.ext2");

	let data_uuid_reg = regex("DATA_UUID");
	fs::copy("./rootfs/fstab", target_dir + "/etc/fstab");
	fs::copy("./rootfs/fstab", target_dir + "/etc/fstab.templ");
	data_uuid_reg.replace_file(target_dir + "/etc/fstab", data_uuid);

	let boot_uuid = uuid_v4();
	let boot_uuid_reg = regex("EFI_UUID");
	boot_uuid_reg.replace_file(target_dir + "/etc/fstab", boot_uuid);
	fs::write(binaries_dir + "/boot_uuid", boot_uuid);

	// remove getty
	fs::delete(target_dir + "/usr/lib/systemd/system/multi-user.target.wants/getty.target");
}

fn post_image(buildroot_dir, host_dir, binaries_dir) {
	let uuid = fs_uuid(host_dir, binaries_dir + "/rootfs.ext2");

	// fill grub.cfg
	let efi_dir = binaries_dir + "/efi-part/EFI/BOOT";
	let grub_cfg = efi_dir + "/grub.cfg";
	fs::copy("./grub-efi.cfg", grub_cfg);
	fs::copy("./grub-efi.cfg", efi_dir + "/grub.templ");
	let uuid_tmp_reg = regex("ROOTFS_UUID");
	uuid_tmp_reg.replace_file(grub_cfg, uuid);
	let kernel_reg = regex("KERNEL_NAME");
	kernel_reg.replace_file(grub_cfg, "bzImage");
	// fill genimage-efi.cfg
	fs::copy("./genimage-efi.cfg", binaries_dir + "/");
	uuid_tmp_reg.replace_file(binaries_dir + "/genimage-efi.cfg", uuid);

	// boot uuid
	let boot_uuid = fs::read(binaries_dir + "/boot_uuid");
	let boot_uuid_reg = regex("EFI_UUID");
	boot_uuid_reg.replace_file(binaries_dir + "/genimage-efi.cfg", boot_uuid);

	// data uuid
	let data_uuid = fs_uuid(host_dir, binaries_dir + "/data.ext2");
	let data_uuid_reg = regex("DATA_UUID");
	data_uuid_reg.replace_file(binaries_dir + "/genimage-efi.cfg", data_uuid);

	if fs::is_file(binaries_dir + "/Image") == true {
		fs::copy(binaries_dir + "/Image", binaries_dir + "/bzImage");
	}

	let publisher = cmd([
		binaries_dir + "/publisher", "pack-image"
	]);
	publisher.dir(binaries_dir);
	publisher.execute();
	// now we need to create the image package +
	// version.fdb file


	// now run genimage
	let genimage = cmd([
		buildroot_dir + "/support/scripts/genimage.sh",
		"-c", binaries_dir + "/genimage-efi.cfg"
	]);
	genimage.execute();
}

fn fs_uuid(host_dir, path) {
	let get_uuid = cmd([host_dir + "/sbin/dumpe2fs", path]);
	let output = get_uuid.output();
	let uuid_reg = regex("Filesystem UUID:\\s+(\\S+)");
	let capts = uuid_reg.captures(output);
	let uuid = capts[1];
	return uuid;
}

// root is 0:0
// user is 14:15
fn chown(user_group, path) {
	let cmd = cmd(["chown", "-h", "-R", user_group, path]);
	cmd.execute();
}

fn chmod(modi, path) {
	let cmd = cmd(["chmod", "-R", modi, path]);
	cmd.execute();
}

// commands from main riji

fn create_config(channel, defconfig, linuxconfig) {
	fs::copy("./buildroot.config", defconfig);

	if channel == "Debug" {
		// append
		let debug_defconfig = fs::read("./debug-buildroot.config");
		fs::append(defconfig, "\n" + debug_defconfig);
	}

	fs::copy("./linux.config", linuxconfig);
}

fn apply_patch(channel, buildroot) {
	fs::delete(buildroot + "/package/service-bootloader");
	fs::copy("./package/service-bootloader", buildroot + "/package/");

	/*
		source "package/setserial/Config.in"
	source "package/service-bootloader/Config.in"
	*/
	let package_config = buildroot + "/package/Config.in";
	if fs::contains(package_config, "service-bootloader") == false {
		// make open ssh work without rw
		let pack_regex = regex("source \"package/setserial/Config.in\"");
		pack_regex.replace_file(
			package_config,
			"source \"package/setserial/Config.in\"" +
			"\nsource \"package/service-bootloader/Config.in\""
		);
	}

	// patch gcc.mk file
	let buildroot_repo = git(buildroot);
	print("patch gcc");
	let gcc_diff = diff_from_file("./pre-patches/gcc-copy-more-libs.patch");
	buildroot_repo.apply_diff(gcc_diff);
	print("patch pipewire");
	let pipwire_diff = diff_from_file("./pre-patches/pipewire-build-fails-with-vulkan.patch");
	buildroot_repo.apply_diff(pipwire_diff);
	print("patch wayland");
	let wayland_diff = diff_from_file("./pre-patches/update-wayland.patch");
	buildroot_repo.apply_diff(wayland_diff);
	print("patch mesa3d");
	let mesa3d_diff = diff_from_file("./pre-patches/mesa3d-update.patch");
	buildroot_repo.apply_diff(mesa3d_diff);
	print("patch psplash");
	let psplash_diff = diff_from_file("./pre-patches/psplash.patch");
	buildroot_repo.apply_diff(psplash_diff);
}