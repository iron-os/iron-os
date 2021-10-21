
fn build(arch, channel) {
	if fs::is_file("./image.tar.gz") == false {
		panic("image not found")
	}
	fs::copy("./image.tar.gz", "./image-copy.tar.gz");
}