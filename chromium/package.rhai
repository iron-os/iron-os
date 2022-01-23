
fn build(arch, channel) {
	// don't build
}

fn pack(dest, arch, channel) {
	fs::copy(
		"./output/" + lowercase(arch) + "/release/chromium",
		dest + "/../"
	);
}