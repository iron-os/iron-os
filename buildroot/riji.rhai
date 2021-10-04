
const BUILDROOT = "./buildroot";
const STABLE = "2021.02.5";
const PACK_PUB = "./../packages-publisher/target/release/packages-publisher";
// const IMAGES = "./buildroot/output/images";

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
	// make linux-savedefconfig
}

fn validate_channel(channel) {
	if channel != "Debug" && channel != "Release" {
		panic!("channel not recognized only Debug | Release");
	}
}

fn build(channel) {
	validate_channel(channel);

	build_internal(channel, BUILDROOT, PACK_PUB);
}

fn build() {
	build_internal("Debug", BUILDROOT, PACK_PUB);
}

// channel: Debug|Release
fn new_br_out_dir(buildroot, channel) {
	return buildroot + "/output/" + channel;
}

// channel: Debug|Release
fn build_internal(channel, buildroot, pack_pub) {
	print("building " + channel);

	let br_out_dir = new_br_out_dir(buildroot, channel);
	let full_br_out_dir = fs::full_path(br_out_dir);
	let images = br_out_dir + "/images";
	let out_dir = "./output/" + channel;

	fs::create_dir(images);

	// patch
	patch_internal(buildroot, br_out_dir);

	// build service-bootloader
	let cargo = cmd([
		"cargo", "build", "--release"
	]);
	cargo.dir("./../service-bootloader");
	print("build service bootloader");
	cargo.execute();

	// copy service-bootloader
	fs::copy(
		"./../service-bootloader/target/release/service_bootloader",
		images + "/service-bootloader"
	);

	fs::copy("./package.toml", images + "/package.toml");

	// build packages-publisher
	let cargo = cmd([
		"cargo", "build", "--release"
	]);
	cargo.dir("./../packages-publisher");
	print("build packages-publisher");
	cargo.execute();

	fs::copy(pack_pub, images + "/publisher");

	// download packages
	let publish = cmd([
		pack_pub, "download"
	]);
	print("download packages");
	publish.execute();

	fs::delete(images + "/packages");
	fs::move("./packages", images + "/packages");

	// first create .config file
	let make_config = cmd([
		"make", "O=" + full_br_out_dir, "defconfig", "BR2_DEFCONFIG=" + full_br_out_dir + "/defconfig"
	]);
	make_config.dir(buildroot);
	print("build config");
	make_config.execute();

	// build buildroot
	let make = cmd([
		"make", "O=" + fs::full_path(br_out_dir)
	]);
	make.dir(buildroot);
	print("build buildroot");
	make.execute();

	// create output
	fs::create_dir(out_dir);

	// now copy the image
	fs::copy(images + "/disk.img", out_dir);
	fs::copy(images + "/image.tar.gz", out_dir);

	// create vdi file if vbox is installed
	if fs::is_prog("VBoxManage") {
		fs::delete(out_dir + "/disk.vdi");
		let vbox = cmd([
			"VBoxManage", "convertfromraw", "--format", "VDI",
			out_dir + "/disk.img", out_dir + "/disk.vdi"
		]);
		vbox.execute();
		// the reverse is VBoxManage clonemedium --format RAW debian.vdi debian.img
	}
}

fn patch() {
	patch_internal(BUILDROOT, BUILDROOT + "/output/Debug");
	patch_internal(BUILDROOT, BUILDROOT + "/output/Release");
}

fn patch_internal(buildroot, br_out_dir) {
	print("patching: " + br_out_dir);

	fs::create_dir(br_out_dir);

	// patch
	// copy board/custom_x64
	fs::copy("./board/custom_x64", buildroot + "/board/");
	// copy .config
	fs::copy("./board/custom_x64/buildroot.config", br_out_dir + "/defconfig");

	// patch weston
	fs::delete(buildroot + "/package/weston");
	fs::copy("./board/custom_x64/package/weston", buildroot + "/package/");
}

// creates the patch
fn save() {
	print("save");

	// copy board/custom_x64
	//fs::copy("./buildroot/board/custom_x64", "./board/");
	// copy .config
	fs::copy("./buildroot/output/Debug/.config", "./");
	//fs::copy("./buildroot/output/build/linux-5.10.43/.config", "./board/linux.config");
}

// removes all unchached files in main
fn clean() {
	clean_internal(BUILDROOT, "Debug");
	clean_internal(BUILDROOT, "Release");
}

fn clean(channel) {
	validate_channel(channel);
	clean_internal(BUILDROOT, channel);
}

fn clean_internal(buildroot, channel) {
	let br_out_dir = new_br_out_dir(buildroot, channel);
	let make = cmd(["make", "O=" + br_out_dir,  "clean"]);
	make.dir(buildroot);
	make.execute();
}


/// todo this does not seem to work
fn clean_target() {
	/*
	rm -rf output/target
find output/ -name ".stamp_target_installed" |xargs rm -rf
*/
	fs::delete("./buildroot/output/target");
	//let del_stamp_target = cmd([
	//	"find", "./buildroot/output/", "-name", "\".stamp_target_installed\"",
	//	"|", "xargs", "rm", "-rf"
	//]);



	let del_stamp_target = cmd([
		"bash", "-c", "rm -f buildroot/output/build/*/.stamp_target_installed"
	]);
	del_stamp_target.execute();

	let del_stamp_installed = cmd([
		"bash", "-c", "rm -f buildroot/output/build/*/.stamp_installed"
	]);
	del_stamp_installed.execute();

}

fn rebuild(pack) {
	print("rebuilding package: " + pack);
	let make = cmd(["make", pack + "-dirclean", pack + "-rebuild"]);
	make.dir(BUILDROOT);
	make.execute();
}