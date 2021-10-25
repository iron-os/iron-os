
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
		"name = \"image-pi4-headless\"",
		"board = \"pi4-headless\"",
		"arch = \"Arm64\"",
		"single-arch = \"Arm64\""
	]);
}

fn apply_patch(channel, buildroot) {
	let common = cmd([
		"riji", "apply_patch", channel, buildroot
	]);
	common.dir("./../common-headless");
	common.execute();
}