
// target dir points to output/target
// binaries points to the images folder
fn post_build(host_dir, target_dir, binaries_dir) {
	let channel = fs::read(binaries_dir + "/channel.txt").trim();
	print("post_build channel: " + channel);

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
	if channel == "Debug" {
		fs::copy("./rootfs/weston-debug.service", systemd_folder + "weston.service");
	} else {
		fs::copy("./rootfs/weston.service", systemd_folder);
	}
	fs::copy("./rootfs/service-bootloader.service", systemd_folder);
	fs::copy("./rootfs/subprocess.service", systemd_folder);
	// symlink get's created in fakeroot
	fs::copy("./rootfs/chromium.service", systemd_folder);
	fs::copy("./rootfs/rules.d", target_dir + "/etc/udev/");
	fs::copy("./rootfs/weston", target_dir + "/etc/");
	fs::copy("./rootfs/NetworkManager.conf", target_dir + "/etc/NetworkManager/");

	// create empty data folder (needed for fresh build since else
	// device_table.mk failes)
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
	let ln = cmd([
		"ln", "-sf",
		"../service-bootloader.service", "service-bootloader.service"
	]);
	ln.dir(target_dir + "/etc/systemd/system/multi-user.target.wants/");
	ln.execute();

	// copy data folder (from common-headless)
	fs::delete(binaries_dir + "/data");
	fs::copy("./data", binaries_dir + "/");

	// copy var folder
	fs::copy(target_dir + "/var", binaries_dir + "/data/");
	fs::delete(target_dir + "/var");

	// delete home folder
	fs::delete(target_dir + "/home");

	// the rights for the home folder are given in create_image_fakeroot

	chmod("700", target_dir + "/etc/NetworkManager/system-connections");

	// check that lvm2-monitor.service does not exist
	if fs::is_file(target_dir + "/usr/lib/systemd/system/lvm2-monitor.service") {
		panic("lvm2 exists");
	}

	// cleanup
	fs::delete(target_dir + "/data");
	fs::create_dir(target_dir + "/data");
	fs::create_dir(target_dir + "/var");
	fs::create_dir(target_dir + "/home");

	let data_uuid = uuid_v4();
	fs::write(binaries_dir + "/data_uuid", data_uuid);

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
	let rootfs_uuid = fs_uuid(host_dir, binaries_dir + "/rootfs.ext2");
	let boot_uuid = fs::read(binaries_dir + "/boot_uuid").trim();
	let data_uuid = fs::read(binaries_dir + "/data_uuid").trim();


	if fs::is_file(binaries_dir + "/Image") {
		// gzip the image
		let gzip = cmd(["gzip", "-k", "-f", "Image"]);
		gzip.dir(binaries_dir);
		gzip.execute();
	}

	let image_file;
	if fs::is_file(binaries_dir + "/bzImage") {
		image_file = "bzImage";
	} else if fs::is_file(binaries_dir + "/Image.gz") {
		image_file = "Image.gz";
	} else {
		panic("Image bzImage or Image.gz not found");
	}

	if fs::is_file(binaries_dir + "/using_grub") {
		post_image_using_grub(
			image_file,
			boot_uuid,
			rootfs_uuid,
			data_uuid,
			binaries_dir
		);
	} else if fs::is_file(binaries_dir + "/using_uboot") {
		post_image_using_uboot(
			image_file,
			boot_uuid,
			rootfs_uuid,
			data_uuid,
			binaries_dir
		);
	} else {
		panic("unkown bootloader");
	}

	// now create the ./image.tar.gz file
	let publisher = cmd([
		binaries_dir + "/publisher", "pack-image"
	]);
	publisher.dir(binaries_dir);
	publisher.execute();
}

fn post_image_using_grub(
	image_file,
	boot_uuid,
	rootfs_uuid,
	data_uuid,
	binaries_dir
) {
	// fill grub.cfg
	let efi_dir = binaries_dir + "/efi-part/EFI/BOOT";
	let grub_cfg = efi_dir + "/grub.cfg";
	fs::copy("./grub-efi.cfg", grub_cfg);
	fs::copy("./grub-efi.cfg", efi_dir + "/grub.templ");
	let uuid_tmp_reg = regex("ROOTFS_UUID");
	uuid_tmp_reg.replace_file(grub_cfg, rootfs_uuid);
	let kernel_reg = regex("KERNEL_NAME");
	kernel_reg.replace_file(grub_cfg, image_file);
	// fill genimage-efi.cfg
	fs::copy("./genimage-efi.cfg", binaries_dir + "/");
	uuid_tmp_reg.replace_file(binaries_dir + "/genimage-efi.cfg", rootfs_uuid);

	// kernel name
	kernel_reg.replace_file(binaries_dir + "/genimage-efi.cfg", image_file);
	// todo fix this
	kernel_reg.replace_file(binaries_dir + "/genimage-efi.cfg", image_file);

	// boot uuid
	let boot_uuid_reg = regex("EFI_UUID");
	boot_uuid_reg.replace_file(binaries_dir + "/genimage-efi.cfg", boot_uuid);

	// data uuid
	let data_uuid_reg = regex("DATA_UUID");
	data_uuid_reg.replace_file(binaries_dir + "/genimage-efi.cfg", data_uuid);

	// let's create the startup.nsh file (some bios were using needs it)
	let efi_file;
	if fs::is_file(efi_dir + "/bootx64.efi") {
		efi_file = "bootx64.efi";
	} else if fs::is_file(efi_dir + "/bootaa64.efi") {
		efi_file = "bootaa64.efi";
	} else {
		panic("did not file the correct efi file");
	}
	fs::write(binaries_dir + "/efi-part/startup.nsh", efi_file);
}

fn post_image_using_uboot(
	image_file,
	boot_uuid,
	rootfs_uuid,
	data_uuid,
	binaries_dir
) {
	let extl_dir = binaries_dir + "/extlinux";
	// create extlinux folder
	fs::create_dir(extl_dir);

	let uboot_cfg = extl_dir + "/extlinux.conf";
	fs::copy("./extlinux-uboot.cfg", uboot_cfg);
	fs::copy("./extlinux-uboot.cfg", extl_dir + "/extlinux.templ");
	let uuid_tmp_reg = regex("ROOTFS_UUID");
	uuid_tmp_reg.replace_file(uboot_cfg, rootfs_uuid);
	let kernel_reg = regex("KERNEL_NAME");
	kernel_reg.replace_file(uboot_cfg, image_file);

	let args = trim(fs::read(binaries_dir + "/using_uboot"));

	// fill genimage-uboot.cfg
	if args != "dont_copy_genimage" {
		fs::copy("./genimage-uboot.cfg", binaries_dir + "/");
	}
	uuid_tmp_reg.replace_file(binaries_dir + "/genimage-uboot.cfg", rootfs_uuid);

	// kernel name
	kernel_reg.replace_file(binaries_dir + "/genimage-uboot.cfg", image_file);
	// todo fix this
	kernel_reg.replace_file(binaries_dir + "/genimage-uboot.cfg", image_file);

	// boot uuid
	let boot_uuid_reg = regex("EFI_UUID");
	boot_uuid_reg.replace_file(binaries_dir + "/genimage-uboot.cfg", boot_uuid);

	// data uuid
	let data_uuid_reg = regex("DATA_UUID");
	data_uuid_reg.replace_file(binaries_dir + "/genimage-uboot.cfg", data_uuid);
}

fn create_image_fakeroot(host_dir, binaries_dir, buildroot_dir) {
	if !fs::is_dir(binaries_dir + "/packages") {
		panic("you need to download the packages first");
	}

	fs::delete(binaries_dir + "/data/packages");
	fs::copy(binaries_dir + "/packages", binaries_dir + "/data/packages");

	// set corrects rights
	chown("14:15", binaries_dir + "/data/home");
	chown("0:0", binaries_dir + "/data/etc");
	chown("14:15", binaries_dir + "/data/packages");

	let data_uuid = trim(fs::read(binaries_dir + "/data_uuid"));

	// execute gen_image
	fs::delete(binaries_dir + "/data.ext2");
	let mkfs = cmd([
		host_dir + "/sbin/mkfs.ext4",
		"-d", binaries_dir + "/data",
		// "-r", "0",
		"-U", data_uuid,
		"-N", "0",
		"-m", "5",
		"-L", "data",
		binaries_dir + "/data.ext2",
		"400M"
	]);
	mkfs.dir(binaries_dir);
	mkfs.execute();
}

fn create_image(binaries_dir, buildroot_dir) {
	// first create the data partition
	print(binaries_dir + "/../host");
	let host_dir = fs::full_path(binaries_dir + "/../host");
	let current_dir = fs::full_path("./");
	let build_dir = fs::full_path(binaries_dir + "/../build");

	// execute create_image_fakeroot
	let fakeroot = cmd([
		host_dir + "/bin/fakeroot",
		"riji", "create_image_fakeroot",
		host_dir, binaries_dir, buildroot_dir
	]);
	fakeroot.dir(current_dir);
	fakeroot.execute();

	// now we need to create the image package +
	// version.fdb file
	let publisher = cmd([
		binaries_dir + "/publisher", "pack-image",
		"--use-existing-image", "--create-version-file"
	]);
	publisher.dir(binaries_dir);
	publisher.execute();
	

	let genimage_cfg;
	if fs::is_file(binaries_dir + "/genimage-efi.cfg") {
		genimage_cfg = binaries_dir + "/genimage-efi.cfg";
	} else if fs::is_file(binaries_dir + "/genimage-uboot.cfg") {
		genimage_cfg = binaries_dir + "/genimage-uboot.cfg";
	} else {
		panic("no genimage config found");
	}

	// now run genimage
	let genimage = cmd([
		buildroot_dir + "/support/scripts/genimage.sh",
		"-c", genimage_cfg
	]);
	genimage.env("BUILD_DIR", binaries_dir);
	genimage.env("BINARIES_DIR", binaries_dir);
	genimage.env("PATH", host_dir + "/bin:" + env_var("PATH"));
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
	fs::copy("./linux.config", linuxconfig);

	if channel == "Debug" {
		// append buildroot
		let debug_defconfig = fs::read("./debug-buildroot.config");
		fs::append(defconfig, "\n" + debug_defconfig);

		// append linux
		let debug_linux = fs::read("./debug-linux.config");
		fs::append(linuxconfig, "\n" + debug_linux);
	}
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
			"source \"package/setserial/Config.in\"\n" +
			"source \"package/service-bootloader/Config.in\""
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
	print("patch wayland-protocols");
	let wayland_diff = diff_from_file("./pre-patches/wayland-protocols.patch");
	buildroot_repo.apply_diff(wayland_diff);
	print("patch wayland");
	let wayland_diff = diff_from_file("./pre-patches/wayland.patch");
	buildroot_repo.apply_diff(wayland_diff);
	print("patch weston");
	let weston_diff = diff_from_file("./pre-patches/weston-kiosk-sleep.patch");
	buildroot_repo.apply_diff(weston_diff);
}