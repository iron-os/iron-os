
fn create_config(channel, defconfig, linuxconfig, images) {
	let common = cmd([
		"riji", "create_config", channel, defconfig, linuxconfig
	]);
	common.dir("./../common");
	common.execute();

	let buildroot = fs::read("./buildroot.config");
	fs::append(defconfig, "\n" + buildroot);

	let linux = fs::read("./linux.config");
	fs::append(linuxconfig, "\n" + linux);

	// need to add the image.toml
	let image = images + "/image.toml";
	fs::copy("./../../package.toml", image);
	fs::append(image, [
		"",
		"name = \"image-intel\"",
		"board = \"intel\"",
		"arch = \"Amd64\"",
		"single-arch = \"Amd64\""
	]);

	fs::write(images + "/using_grub", "using_grub");
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