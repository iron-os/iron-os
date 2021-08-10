
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
	fs::append(target_dir + "/etc/profile", "\nXDG_RUNTIME_DIR=/tmp");
}

fn post_image(binaries_dir) {
	/*

	UUID=$(dumpe2fs "$BINARIES_DIR/rootfs.ext2" 2>/dev/null | sed -n 's/^Filesystem UUID: *\(.*\)/\1/p')
	sed -i "s/UUID_TMP/$UUID/g" "$BINARIES_DIR/efi-part/EFI/BOOT/grub.cfg"
	sed "s/UUID_TMP/$UUID/g" board/pc/genimage-efi.cfg > "$BINARIES_DIR/genimage-efi.cfg"
	support/scripts/genimage.sh -c "$BINARIES_DIR/genimage-efi.cfg"

	*/
}