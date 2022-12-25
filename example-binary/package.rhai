fn build(arch, channel) {
	build(arch, channel, "Release");
}

fn build(arch, channel, hostChannel) {
	let my_path = fs::full_path(".");
	let buildroot_riji = cmd([
		"riji", "rust_build", arch, channel, my_path, "example-binary",
		hostChannel
	]);
	buildroot_riji.dir("./../buildroot");
	buildroot_riji.execute();
	// the files are stored in /target/rust_build
}

fn pack(dest, arch, channel) {
	fs::copy("./target/rust_build/example-binary", dest);
}