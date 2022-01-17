
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
	let buildroot = git_clone("git://git.buildroot.net/buildroot", "./buildroot");
	buildroot.checkout_tag("2021.08.3");

}

fn validate_channel(channel) {
	if channel != "Debug" && channel != "Release" {
		panic!("channel not recognized only Debug | Release");
	}
}

fn patch() {
	print("patch <board> <channel>");
}

fn patch(board, channel) {
	validate_channel(channel);
	patch_internal("./buildroot", board, channel);
}

fn patch_internal(buildroot, board, channel) {
	let buildroot_git = git(buildroot);
	buildroot_git.force_head();

	let br_out_dir = new_br_out_dir(buildroot, board, channel);
	print("patching: " + br_out_dir);

	fs::create_dir(br_out_dir);
	let full_br_out_dir = fs::full_path(br_out_dir);

	let defconfig = full_br_out_dir + "/defconfig";
	let linuxconfig = full_br_out_dir + "/linux.config";
	let images = full_br_out_dir + "/images";
	fs::create_dir(images);
	let board_riji = cmd([
		"riji", "create_config", channel, defconfig, linuxconfig, images
	]);
	board_riji.dir("./board/" + board);
	board_riji.execute();

	// add BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES
	let full_linuxconfig = fs::full_path(linuxconfig);
	fs::append(
		defconfig,
		"\nBR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES=\"" + full_linuxconfig + "\""
	);

	let full_buildroot = fs::full_path(buildroot);
	let patch_riji = cmd([
		"riji", "apply_patch", channel, full_buildroot
	]);
	patch_riji.dir("./board/" + board);
	patch_riji.execute();

	// create .config file
	let make_config = cmd([
		"make", "O=" + full_br_out_dir, "defconfig", "BR2_DEFCONFIG=" + full_br_out_dir + "/defconfig"
	]);
	make_config.dir(buildroot);
	print("build config");
	make_config.execute();
}

fn build() {
	print("build <board> <channel>");
}

fn build(board, channel) {
	validate_channel(channel);

	build_internal(board, channel, "./buildroot");
}

// channel: Debug|Release
fn new_br_out_dir(buildroot, board, channel) {
	return buildroot + "/output/" + board + "/" + channel;
}

// channel: Debug|Release
fn build_internal(board, channel, buildroot) {
	print("building " + board + " " + channel);

	let br_out_dir = new_br_out_dir(buildroot, board, channel);
	print("br_out_dir: " + br_out_dir);
	fs::create_dir(br_out_dir);

	let full_br_out_dir = fs::full_path(br_out_dir);
	let images = br_out_dir + "/images";
	let out_dir = "./output/" + board + "/" + channel;

	fs::create_dir(images);

	// patch
	patch_internal(buildroot, board, channel);

	// build packages-publisher
	let cargo = cmd([
		"cargo", "build", "--release"
	]);
	cargo.dir("./../packages-publisher");
	print("build packages-publisher");
	cargo.execute();

	fs::copy(
		"./../packages-publisher/target/release/packages-publisher",
		images + "/publisher"
	);

	// download packages
	//let publish = cmd([
	//	pack_pub, "download"
	//]);
	//print("download packages");
	//publish.execute();

	//fs::delete(images + "/packages");
	//fs::move("./packages", images + "/packages");

	// build buildroot
	let make = cmd("make");
	make.dir(br_out_dir);
	print("build buildroot");
	make.execute();

	// create output
	fs::create_dir(out_dir);

	// now copy the image
	fs::copy(images + "/disk.img", out_dir);
	fs::copy(images + "/image.tar.gz", out_dir);
	fs::copy(images + "/image.toml", out_dir + "/package.toml");
	fs::copy("./package.rhai", out_dir);

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

fn publish() {
	print("publish <board> <channel> <pub-channel>");
}

fn publish(board, channel, pub_channel) {
	let out_dir = "./output/" + board + "/" + channel;
	let publisher = cmd([
		"publisher", "upload", pub_channel
	]);
	publisher.dir(out_dir);
	publisher.execute();
}

fn download_packages() {
	print("download_packages <product> <board> <channel>");
}

// channel is only used for the board
fn download_packages(product, board, channel) {
	patch_internal("./buildroot", board, channel);

	let publisher = cmd([
		"publisher", "download", "./products/" + product + ".toml"
	]);
	publisher.execute();

	let br_out_dir = new_br_out_dir("./buildroot", board, channel);
	let images = br_out_dir + "/images";
	fs::create_dir(images);

	let packages_dir = images + "/packages";

	fs::delete(packages_dir);

	fs::copy("./packages", packages_dir);
	fs::delete("./packages");
}

fn config() {
	print("use config <board> <channel>");
}

fn config(board, channel) {
	validate_channel(channel);
	config_internal("./buildroot", board, channel);
}

fn config_internal(buildroot, board, channel) {
	patch_internal(buildroot, board, channel);
	let br_out_dir = new_br_out_dir(buildroot, board, channel);

	let make = cmd(["make", "menuconfig"]);
	make.dir(br_out_dir);
	make.execute();
}

// TODO
fn linux_config(board, channel) {
	validate_channel(channel);
	patch_internal("./buildroot", board, channel);
	let br_out_dir = new_br_out_dir("./buildroot", board, channel);

	let make = cmd(["make", "linux-menuconfig"]);
	make.dir(br_out_dir);
	make.execute();

	// output in output/build/linux-<>/.config
	// make linux-savedefconfig
}

// creates the patch
fn save() {
	print("use save <board> <channel>");
}

fn save(board, channel) {
	validate_channel(channel);
	save_internal("./buildroot", board, channel);
}

fn save_internal(buildroot, board, channel) {
	let br_out_dir = new_br_out_dir(buildroot, board, channel);
	print("save to " + br_out_dir + "/defconfig");

	let make = cmd(["make", "savedefconfig"]);
	make.dir(br_out_dir);
	make.execute();

	// copy board/custom_x64
	//fs::copy("./buildroot/board/custom_x64", "./board/");
	// copy .config
	// fs::copy("./buildroot/output/Debug/.config", "./");
	//fs::copy("./buildroot/output/build/linux-5.10.43/.config", "./board/linux.config");
}

// removes all unchached files in main
fn clean() {
	print("use clean <board> <channel>");
}

fn clean(board, channel) {
	validate_channel(channel);
	clean_internal("./buildroot", board, channel);
}

fn clean_internal(buildroot, board, channel) {
	let br_out_dir = new_br_out_dir(buildroot, board, channel);
	let make = cmd(["make",  "clean"]);
	make.dir(br_out_dir);
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

fn rebuild() {
	print("use rebuild <board> <channel> <package>");
}

fn rebuild(board, channel, pack) {
	validate_channel(channel);
	print("rebuilding package: " + pack);
	let br_out_dir = new_br_out_dir("./buildroot", board, channel);
	let make = cmd(["make", pack + "-dirclean", pack + "-rebuild"]);
	make.dir(br_out_dir);
	make.execute();
}

fn board_by_arch(arch) {
	let br = "./buildroot/output/";
	if arch == "Amd64" {
		if fs::is_dir(br + "intel") {
			return "intel";
		} else if fs::is_dir(br + "intel-headless") {
			return "intel-headless";
		}
	} else if arch == "Arm64" {
		if fs::is_dir(br + "pi4") {
			return "pi4";
		} else if fs::is_dir(br + "pi4-headless") {
			return "pi4-headless";
		}
	}

	panic("no board found for arch " + arch);
}

fn rust_build(arch, channel, path, binary) {
	let board;
	let buildroot_target;
	let target;
	if arch == "Amd64" {
		board = board_by_arch(arch);
		buildroot_target = "x86_64-buildroot-linux-gnu";
		target = "x86_64-unknown-linux-gnu";
	} else if arch == "Arm64" {
		board = board_by_arch(arch);
		buildroot_target = "aarch64-buildroot-linux-gnu";
		target = "aarch64-unknown-linux-gnu";
	} else {
		panic("unknown " + arch + " architecture");
	}

	let host = full_host_path(board);
	let sysroot = host + "/" + buildroot_target + "/sysroot";
	let cargo_bin = host + "/bin/cargo";
	let cargo_home = host + "/share/cargo";

	let cargo = cmd([
		cargo_bin, "-Z", "target-applies-to-host",
		"build", "--release", "--target", target,
		
	]);
	cargo.dir(path);
	cargo.env_clear();
	cargo.env("PATH", host + "/bin" + ":" + "/usr/bin");
	cargo.env("CARGO_HOME", cargo_home);
	cargo.env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly");
	cargo.env("CARGO_TARGET_APPLIES_TO_HOST", "false");
	cargo.execute();

	// where is the executable??
	// maybe store it in target/rust_build/.
	let bin = path + "/target/" + target + "/release/" + binary;
	let rust_out = path + "/target/rust_build";
	fs::delete(rust_out);
	fs::create_dir(rust_out);
	fs::copy(bin, rust_out);
}

fn full_host_path(board) {
	let host_raw_release = "./buildroot/output/" + board + "/Release/host";
	let host_raw_debug = "./buildroot/output/" + board + "/Debug/host";
	let host_raw = host_raw_release;
	if !fs::is_dir(host_raw) {
		host_raw = host_raw_debug;
	}
	if !fs::is_dir(host_raw) {
		panic("host folder not found: " + host_raw_release + " & " + host_raw_debug);
	}

	return fs::full_path(host_raw);
}

fn sysroot(arch) {
	let board;
	if arch == "Amd64" {
		board = "intel-headless";
	} else {
		panic("only Amd64 supported");
	}
	sysroot(arch, board);
}

fn sysroot(arch, board) {
	let buildroot_target;
	if arch == "Amd64" {
		buildroot_target = "x86_64-buildroot-linux-gnu";
	} else if arch == "Arm64" {
		buildroot_target = "aarch64-buildroot-linux-gnu";
	} else {
		panic("only Amd64 && Arm64 supported");
	}

	let host = full_host_path(board);
	let sysroot = host + "/" + buildroot_target + "/sysroot";

	print(sysroot);
}