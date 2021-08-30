
// target dir point sto output/target
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
	fs::copy("./rootfs/weston.service", target_dir + "/etc/systemd/system/");
	fs::copy("./rootfs/service-bootloader.service", target_dir + "/etc/systemd/system/");

	fs::copy(binaries_dir + "/service-bootloader", target_dir + "/usr/bin/");
	// symlink get's created in fakeroot

	// create empty data folder (needed for fresh build since else device_table.mk failes)
	fs::create_dir(target_dir + "/data");

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

	let logind = target_dir + "/etc/systemd/logind.conf";
	if fs::contains(logind, "NAutoVTs=0") == false {
		fs::append(logind, [
			"NAutoVTs=0",
			"ReserveVT=0"
		]);
	}

}

fn fakeroot(host_dir, target_dir, binaries_dir) {
	// target dir is in ext2/target but we can't se that folder

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

	// copy chromium
	fs::copy(binaries_dir + "/chromium", target_dir + "/data/");

	// set corrects rights
	chown("14:15", target_dir + "/data/home");
	chown("0:0", target_dir + "/data/etc");
	chown("0:0", target_dir + "/data/chromium/chrome-sandbox");
	chmod("4755", target_dir + "/data/chromium/chrome-sandbox");

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
		"256M"
	]);
	mkfs.dir(binaries_dir);
	mkfs.execute();

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
	// fill genimage-efi.cfg
	fs::copy("./genimage-efi.cfg", binaries_dir + "/");
	uuid_tmp_reg.replace_file(binaries_dir + "/genimage-efi.cfg", uuid);

	// data uuid
	let data_uuid = fs_uuid(host_dir, binaries_dir + "/data.ext2");
	let data_uuid_reg = regex("DATA_UUID");
	data_uuid_reg.replace_file(binaries_dir + "/genimage-efi.cfg", data_uuid);

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