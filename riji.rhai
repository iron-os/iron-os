
const BUILDROOT = "./buildroot";
const STABLE = "2021.02.3";

fn help() {
	print([
		"Custom OS Riji help:",
		"	- download // downloads buildroot",
		"	- config // opens menuconfig",
		"	- build // builds the os",
		"	- patch // apply the patches",
		"	- clean // prepares for a fresh build"
	])
}

fn download() {
	// download buildroot
	let buildroot = git_clone("https://git.busybox.net/buildroot", BUILDROOT);
	buildroot.checkout_tag(STABLE);

}

fn config() {
	//print("call with build release or build debug");

	let make = cmd(["make", "menuconfig"]);
	make.dir(BUILDROOT);
	make.execute();
}

fn linux_config() {
	let make = cmd(["make", "linux-menuconfig"]);
	make.dir(BUILDROOT);
	make.execute();

	// output in output/build/linux-<>/.config
}

fn build() {
	print("build");

	let make = cmd("make");
	make.dir(BUILDROOT);
	make.execute();

	// create output
	fs::create_dir("./output");

	// now copy the image
	fs::copy("./buildroot/output/images/disk.img", "./output");

	if fs::is_prog("VBoxManage") {
		fs::delete("./output/disk.vdi");
		let vbox = cmd([
			"VBoxManage", "convertfromraw", "--format", "VDI",
			"./output/disk.img", "./output/disk.vdi"
		]);
		vbox.execute();
	}
}

// creates the patch
fn save() {
	print("save");

	// copy board/custom_x64
	//fs::copy("./buildroot/board/custom_x64", "./board/");
	// copy .config
	fs::copy("./buildroot/.config", "./");
	fs::copy("./buildroot/output/build/linux-5.10.43/.config", "./board/linux.config");
}

// apply the patch
fn patch() {
	print("patch");

	// copy board/custom_x64
	fs::copy("./board/custom_x64", "./buildroot/board/");
	// copy .config
	fs::copy("./.config", "./buildroot/");

	//fs::copy("./board/linux.config", "./buildroot/output/build/linux-5.10.43/.config");

	let git = git(BUILDROOT);

	// get psplash patch
	// let psplash = diff_from_file("./patches/psplash.patch");
	// git.apply_diff(psplash);
}

// removes all unchached files in main
fn clean() {

	let make = cmd(["make", "clean"]);
	make.dir(BUILDROOT);
	make.execute();
}

fn rebuild(pack) {
	print("rebuilding package: " + pack);
	let make = cmd(["make", pack + "-dirclean", pack + "-rebuild"]);
	make.dir(BUILDROOT);
	make.execute();
}
