
fn create_config(channel, defconfig, linuxconfig) {
	let common_headless = cmd([
		"riji", "create_config", channel, defconfig, linuxconfig
	]);
	common_headless.dir("./../common-headless");
	common_headless.execute();

	let buildroot = fs::read("./buildroot.config");
	fs::append(defconfig, "\n" + buildroot);

	let linux = fs::read("./linux.config");
	fs::append(linuxconfig, "\n" + linux);
}

fn apply_patch(channel, buildroot) {
	let common_headless = cmd([
		"riji", "apply_patch", channel, buildroot
	]);
	common_headless.dir("./../common-headless");
	common_headless.execute();

	// patch weston
	fs::delete(buildroot + "/package/weston");
	fs::copy("./package/weston", buildroot + "/package/");
}