
fn build(arch, channel) {
	// don't build
}

fn pack(dest, arch, channel) {
	if arch != "Amd64" {
		panic("only Amd64 supported");
	}

	fs::copy(
		"./output/" + lowercase(arch) + "/release/chromium",
		dest + "/../"
	);
}