fn build(arch, channel) {
	let build = cmd(["./build.sh", lowercase(arch)]);
	build.execute();
}

fn pack(dest, arch, channel) {
	fs::copy(
		"./output/" + lowercase(arch) + "/chromium",
		dest + "/../"
	);
}
