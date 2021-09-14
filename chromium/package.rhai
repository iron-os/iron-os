
fn build(channel) {
	// don't build
}

fn pack(dest, channel) {
	fs::copy("./out/release/chromium/", dest + "/../");
}