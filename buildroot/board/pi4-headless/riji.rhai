
fn create_config(channel, defconfig, linuxconfig, images) {
	let common = cmd([
		"riji", "create_config", channel, defconfig, linuxconfig
	]);
	common.dir("./../common-headless");
	common.execute();

	let buildroot = fs::read("./buildroot.config");
	fs::append(defconfig, "\n" + buildroot);
	fs::append(
		defconfig,
		"\nBR2_TARGET_UBOOT_CONFIG_FRAGMENT_FILES=\"" + fs::full_path("./uboot.cfg") + "\""
	);

	// need to add the image.toml
	let image = images + "/image.toml";
	let image_str = fs::read("./../../package.toml");
	fs::write(image, [
		"name = \"image-pi4-headless\"",
		"board = \"pi4-headless\"",
		"arch = \"Arm64\"",
		"single-arch = \"Arm64\"",
		"",
		image_str
	]);

	fs::copy("./genimage-uboot.cfg", images + "/");
	fs::copy("./config.txt", images + "/");

	// setting dont_copy here so that common-headless does not copy
	// it's own genimage-uboot file
	fs::write(images + "/using_uboot", "dont_copy_genimage");
}

fn apply_patch(channel, buildroot) {
	let common = cmd([
		"riji", "apply_patch", channel, buildroot
	]);
	common.dir("./../common-headless");
	common.execute();
}

fn create_image(channel, images, buildroot) {
	let common = cmd([
		"riji", "create_image", images, buildroot
	]);
	common.dir("./../common-headless");
	common.execute();
}
