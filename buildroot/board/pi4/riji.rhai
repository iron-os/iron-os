
fn create_config(channel, defconfig, linuxconfig, images) {
	let common = cmd([
		"riji", "create_config", channel, defconfig, linuxconfig
	]);
	common.dir("./../common");
	common.execute();

	let buildroot = fs::read("./buildroot.config");
	fs::append(defconfig, "\n" + buildroot);

	// let linux = fs::read("./linux.config");
	// fs::append(linuxconfig, "\n" + linux);

	// need to add the image.toml
	let image = images + "/image.toml";
	fs::copy("./../../package.toml", image);
	fs::append(image, [
		"",
		"name = \"image-pi4\"",
		"board = \"pi4\"",
		"arch = \"Arm64\"",
		"single-arch = \"Arm64\""
	]);

	fs::copy("./../pi4-headless/genimage-uboot.cfg", images + "/");
	fs::copy("./../pi4-headless/config.txt", images + "/");

	// setting dont_copy here so that common-headless does not copy
	// it's own genimage-uboot file
	fs::write(images + "/using_uboot", "dont_copy_genimage");
}

fn apply_patch(channel, buildroot) {
	let common = cmd([
		"riji", "apply_patch", channel, buildroot
	]);
	common.dir("./../common");
	common.execute();
}

fn create_image(channel, images, buildroot) {
	let common = cmd([
		"riji", "create_image", images, buildroot
	]);
	common.dir("./../common-headless");
	common.execute();
}