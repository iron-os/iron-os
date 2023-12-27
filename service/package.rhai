fn build(arch, channel) {
	build(arch, channel, "Release");
}

fn build(arch, channel, hostChannel) {
	// first build the service-bootloader because of #10
	let sb_path = fs::full_path("../service-bootloader");
	let sb_build = cmd([
		"riji", "rust_build", arch, channel, sb_path, "service_bootloader",
		hostChannel
	]);
	sb_build.dir("./../buildroot");
	sb_build.execute();

	let my_path = fs::full_path(".");
	let buildroot_riji = cmd([
		"riji", "rust_build", arch, channel, my_path, "service", hostChannel
	]);
	buildroot_riji.dir("./../buildroot");
	buildroot_riji.execute();
	// the files are stored in /target/rust_build
}

fn pack(dest, arch, channel) {
	fs::copy("../service-bootloader/target/rust_build/service_bootloader", dest);
	fs::copy("./target/rust_build/service", dest);
	fs::copy("./www", dest);
	fs::copy("./extension", dest);
}