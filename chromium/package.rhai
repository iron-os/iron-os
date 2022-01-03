
fn build(arch, channel) {
	// don't build
}

fn pack(dest, arch, channel) {
	if arch == "Arm64" {
		panic("Arm64 not supported")
	}

	fs::copy("./output/Release/chromium", dest + "/../");
}