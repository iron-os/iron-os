
fn build(arch, channel) {
	let my_path = fs::full_path(".");
	let buildroot_riji = cmd([
		"riji", "rust_build", arch, channel, my_path, "service"
	]);
	buildroot_riji.dir("./../buildroot");
	buildroot_riji.execute();
	// the files are stored in /target/rust_build
}

fn pack(dest, arch, channel) {
	fs::copy("./target/rust_build/service", dest);

	if fs::is_dir("./www/fire-html") == false {
		panic("fire-html required");
	}
	fs::copy("./www", dest);

	fs::copy("./extension", dest);
}