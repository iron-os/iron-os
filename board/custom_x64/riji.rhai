
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
	let profile_file = target_dir + "/etc/profile";
	if fs::contains(profile_file, "XDG_RUNTIME_DIR") == false {
		fs::append(profile_file, "\nexport XDG_RUNTIME_DIR='/tmp'\n");
	}

	// set session optional pam_systemd.so
	let pam_dir = target_dir + "/etc/pam.d";
	//if fs::contains(pam_dir + "/login", "pam_systemd") == false {
	//	fs::append(pam_dir + "/login", "\nsession optional pam_systemd.so\n");
	//}

	// rewrite pam
	fs::write(pam_dir + "/login", [
		"session		required	pam_loginuid.so",
		"auth			required	pam_unix.so nullok",
		"account		required	pam_unix.so",
		"password		required	pam_unix.so nullok",
		"session		required	pam_limits.so",
		"session		required	pam_env.so",
		"session		required	pam_unix.so",
		"session		optional	pam_lastlog.so",
		"session		optional	pam_systemd.so"
	]);

	// rewrite sudo
	fs::write(pam_dir + "/sudo", [
	//	"session		required	pam_loginuid.so",
		"auth			required	pam_unix.so nullok",
		"account		required	pam_unix.so",
		"password		required	pam_unix.so nullok",
		"session		required	pam_limits.so",
		"session		required	pam_env.so",
		"session		required	pam_unix.so",
	//	"session		optional	pam_lastlog.so",
		"session		optional	pam_systemd.so"
	]);

	//if fs::contains(pam_dir + "/sshd", "pam_systemd") == false {
	//	fs::append(pam_dir + "/sshd", "\nsession optional pam_systemd.so\n");
	//}
}

fn post_image(binaries_dir) {
	/*

	UUID=$(dumpe2fs "$BINARIES_DIR/rootfs.ext2" 2>/dev/null | sed -n 's/^Filesystem UUID: *\(.*\)/\1/p')
	sed -i "s/UUID_TMP/$UUID/g" "$BINARIES_DIR/efi-part/EFI/BOOT/grub.cfg"
	sed "s/UUID_TMP/$UUID/g" board/pc/genimage-efi.cfg > "$BINARIES_DIR/genimage-efi.cfg"
	support/scripts/genimage.sh -c "$BINARIES_DIR/genimage-efi.cfg"

	*/
}

fn qemu() {

	// qemu-system-x86_64 -m 4096 -drive format=raw,file=<img>.img

}