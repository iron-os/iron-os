
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
	let image_str = fs::read("./../../package.toml");
	fs::write(image, [
		"name = \"image-intel-headless\"",
		"board = \"intel-headless\"",
		"arch = \"Amd64\"",
		"single-arch = \"Amd64\"",
		"",
		image_str
	]);

	fs::write(images + "/using_grub", "using_grub");
}

fn apply_patch(channel, buildroot) {
	let common = cmd([
		"riji", "apply_patch", channel, buildroot
	]);
	common.dir("./../common-headless");
	common.execute();
}
