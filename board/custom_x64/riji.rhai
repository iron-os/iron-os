
fn post_build(binaries_dir) {
	print("post build");
	fs::copy("./grub-efi.cfg", binaries_dir + "/efi-part/EFI/BOOT/grub.cfg")
	//cp -f "$BOARD_DIR/grub-efi.cfg" "$BINARIES_DIR/efi-part/EFI/BOOT/grub.cfg"
}