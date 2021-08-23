
// target dir point sto output/target
// binaries points to the images folder
fn post_build(target_dir, binaries_dir) {
	// copy efi config
	fs::copy("./grub-efi.cfg", binaries_dir + "/efi-part/EFI/BOOT/grub.cfg");
	//cp -f "$BOARD_DIR/grub-efi.cfg" "$BINARIES_DIR/efi-part/EFI/BOOT/grub.cfg"

	// make root login bootable
	let root_login = regex("#PermitRootLogin .*");
	root_login.replace_file(target_dir + "/etc/ssh/sshd_config", "PermitRootLogin yes");

	// create .profile file
	//let profile_file = target_dir + "/etc/profile";
	//if fs::contains(profile_file, "XDG_RUNTIME_DIR") == false {
	//	fs::append(profile_file, "\nexport XDG_RUNTIME_DIR='/tmp'\n");
	//}

	// copy pam.d
	fs::copy("./pam.d", target_dir + "/etc/");

	// copy dbus
	fs::copy("./dbus/dbus.service", target_dir + "/usr/lib/systemd/user/");
	fs::copy("./dbus/dbus.socket", target_dir + "/usr/lib/systemd/user/");

	// create symlinks to /systemd/user/sockets.target.wants/dbus.socket
	// create directory
	let socket_want = target_dir + "/usr/lib/systemd/user/sockets.target.wants";
	fs::create_dir(socket_want);

	let ln = cmd(["ln", "-sf", "../dbus.socket", "dbus.socket"]);
	ln.dir(socket_want);
	ln.execute();

	// add weston.ini
	fs::create_dir(target_dir + "/home/user");
	fs::copy("./weston.ini", target_dir + "/home/user/");
}

fn post_image(binaries_dir) {
	/*

	UUID=$(dumpe2fs "$BINARIES_DIR/rootfs.ext2" 2>/dev/null | sed -n 's/^Filesystem UUID: *\(.*\)/\1/p')
	sed -i "s/UUID_TMP/$UUID/g" "$BINARIES_DIR/efi-part/EFI/BOOT/grub.cfg"
	sed "s/UUID_TMP/$UUID/g" board/pc/genimage-efi.cfg > "$BINARIES_DIR/genimage-efi.cfg"
	support/scripts/genimage.sh -c "$BINARIES_DIR/genimage-efi.cfg"

	*/
}