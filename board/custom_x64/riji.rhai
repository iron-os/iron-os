
// target dir point sto output/target
// binaries points to the images folder
fn post_build(host_dir, target_dir, binaries_dir) {
	// make ssh root login
	//let root_login = regex("#PermitRootLogin .*");
	//root_login.replace_file(target_dir + "/etc/ssh/sshd_config", "PermitRootLogin yes");

	// create .profile file
	//let profile_file = target_dir + "/etc/profile";
	//if fs::contains(profile_file, "XDG_RUNTIME_DIR") == false {
	//	fs::append(profile_file, "\nexport XDG_RUNTIME_DIR='/tmp'\n");
	//}

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

	// add weston.ini
	fs::create_dir(target_dir + "/home/user");
	fs::copy("./rootfs/weston.ini", target_dir + "/home/user/");


	// prepare data folder
}

fn post_image(buildroot_dir, host_dir, binaries_dir) {
	let uuid = fs_uuid(host_dir, binaries_dir + "/rootfs.ext2");

	let uuid_tmp_reg = regex("ROOTFS_UUID");
	// fill grub.cfg
	let grub_cfg = binaries_dir + "/efi-part/EFI/BOOT/grub.cfg";
	fs::copy("./grub-efi.cfg", grub_cfg);
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


	/*

	UUID=$(dumpe2fs "$BINARIES_DIR/rootfs.ext2" 2>/dev/null | sed -n 's/^Filesystem UUID: *\(.*\)/\1/p')
	sed -i "s/UUID_TMP/$UUID/g" "$BINARIES_DIR/efi-part/EFI/BOOT/grub.cfg"
	sed "s/UUID_TMP/$UUID/g" board/pc/genimage-efi.cfg > "$BINARIES_DIR/genimage-efi.cfg"
	support/scripts/genimage.sh -c "$BINARIES_DIR/genimage-efi.cfg"

	UUID=$(dumpe2fs "$BINARIES_DIR/rootfs.ext2" 2>/dev/null | sed -n 's/^Filesystem UUID: *\(.*\)/\1/p')
	sed -i "s/UUID_TMP/$UUID/g" "$BINARIES_DIR/efi-part/EFI/BOOT/grub.cfg"
	sed "s/UUID_TMP/$UUID/g" board/custom_x64/genimage-efi.cfg > "$BINARIES_DIR/genimage-efi.cfg"
	support/scripts/genimage.sh -c "$BINARIES_DIR/genimage-efi.cfg"

	*/
}

fn fs_uuid(host_dir, path) {
	let get_uuid = cmd([host_dir + "/sbin/dumpe2fs", path]);
	let output = get_uuid.output();
	let uuid_reg = regex("Filesystem UUID:\\s+(\\S+)");
	let capts = uuid_reg.captures(output);
	let uuid = capts[1];
	return uuid;
}

fn fakeroot(host_dir, target_dir, binaries_dir) {
	// target dir is in ext2/target but we can't se that folder

	// copy data folder
	fs::copy("./data", target_dir + "/");

	fs::copy(target_dir + "/var", target_dir + "/data/");
	fs::delete(target_dir + "/var");

	// set corrects rights
	chown("14:15", target_dir + "/data/home");

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

	let data_uuid = fs_uuid(host_dir, binaries_dir + "/data.ext2");

	let data_uuid_reg = regex("DATA_UUID");
	fs::copy("./rootfs/fstab", target_dir + "/etc/fstab");
	data_uuid_reg.replace_file(target_dir + "/etc/fstab", data_uuid);
}

// root is 0:0
// user is 14:15
fn chown(user_group, path) {
	let cmd = cmd(["chown", "-h", "-R", user_group, path]);
	cmd.execute();
}