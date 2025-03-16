
fn create_config(channel, defconfig, linuxconfig, images) {
	let common = cmd([
		"riji", "create_config", channel, defconfig, linuxconfig
	]);
	common.dir("./../common-headless");
	common.execute();

	let buildroot = fs::read("./buildroot.config");
	fs::append(defconfig, "\n" + buildroot);

	// need to add the image.toml
	let image = images + "/image.toml";
	fs::copy("./../../package.toml", image);
	fs::append(image, [
		"",
		"name = \"image-pi5-headless\"",
		"board = \"pi5-headless\"",
		"arch = \"Arm64\"",
		"single-arch = \"Arm64\""
	]);

	fs::copy("./genimage-boot.cfg", images + "/");
	fs::copy("./config.txt", images + "/");

	fs::write(images + "/using_rpi", "yes");
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
