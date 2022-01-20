
fn build(arch, channel) {
	// don't build
}

fn pack(dest, arch, channel) {
	if arch != "Amd64" {
		panic("only Amd64 supported");
	}

	fs::move("./output/default/firefox", "./output/default/chromium");

	fs::copy(
		"./output/default/chromium",
		dest + "/../"
	);

	fs::delete("./output/default/chromium");
}