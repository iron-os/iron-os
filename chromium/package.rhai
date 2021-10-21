
fn build(arch, channel) {
	// don't build
}

fn pack(dest, arch, channel) {
	if arch == "Arm64" {
		panic("Arm64 not supported")
	}

	fs::copy("./out/" + arch + "/release/chromium/", dest + "/../");
}