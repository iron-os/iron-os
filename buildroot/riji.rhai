
fn help() {
	print([
		"Iron OS Riji help:",
		"	- download",
		"	- patch <board> <channel>",
		"	- build <board> <channel>",
		"	- upload <board> <channel> <pub-channel>",
		"	- create_image <product> <board> <channel>",
		"	- config <board> <channel>",
		"	- linux_config <board> <channel>",
		"	- rebuild <board> <channel> <package>",
		"	- rust_build <arch> <channel> <path> <binary>",
		"	- sysroot <arch> <board>"
	]);
}

fn download() {
	// download buildroot
	let buildroot = git_clone("git://git.buildroot.net/buildroot", "./buildroot");
	buildroot.checkout_tag("2023.02");
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

// patch creates a combined configuration for the given board and channel
// and it applys patches to code if needed
fn patch_internal(buildroot, board, channel) {
	let buildroot_git = git(buildroot);
	buildroot_git.force_head();

	let br_out_dir = new_br_out_dir(buildroot, board, channel);
	print("patching: " + br_out_dir);

	fs::create_dir(br_out_dir);
	let full_br_out_dir = fs::full_path(br_out_dir);

	print("creating configuration");

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

	print("apply patches");

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
/// build calls patch and then runs make build
fn build_internal(board, channel, buildroot) {
	print("building " + board + " " + channel);

	let br_out_dir = new_br_out_dir(buildroot, board, channel);
	print("br_out_dir: " + br_out_dir);
	fs::create_dir(br_out_dir);

	let full_br_out_dir = fs::full_path(br_out_dir);
	let images = br_out_dir + "/images";
	let out_dir = "./output/" + board + "/" + channel;

	fs::create_dir(images);
	fs::write(images + "/channel.txt", channel);

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

	// build buildroot
	let make = cmd("make");
	make.dir(br_out_dir);
	print("build buildroot");
	make.execute();
}

fn upload() {
	print("upload <board> <channel> <pub-channel> (one-device-whitelist)");
}

fn upload(board, channel, pub_channel) {
	_upload(board, channel, pub_channel, []);
}

fn upload(board, channel, pub_channel, device_id) {
	_upload(board, channel, pub_channel, [device_id]);
}

fn _upload(board, channel, pub_channel, whitelist) {
	let buildroot = "./buildroot";
	let full_buildroot = fs::full_path(buildroot);

	let br_out_dir = new_br_out_dir("./buildroot", board, channel);
	let images = br_out_dir + "/images";
	let image = images + "/image.tar.gz";

	if !fs::is_file(image) {
		panic("you need to build " + board + " " + channel + " first");
	}

	// create output
	let out_dir = "./output/" + board + "/" + channel;
	fs::create_dir(out_dir);

	// now copy the image
	fs::copy(image, out_dir);
	fs::copy(images + "/image.toml", out_dir + "/package.toml");
	fs::copy("./package.rhai", out_dir);

	let publisher = cmd([
		"publisher", "upload", pub_channel
	]);
	if whitelist.len() > 0 {
		publisher.arg("--whitelist");
		publisher.args(whitelist);
	}
	publisher.dir(out_dir);
	publisher.execute();
}

fn create_image() {
	print("create_image <product> <board> <channel>");	
}

fn create_image(product, board, channel) {
	let buildroot = "./buildroot";
	let full_buildroot = fs::full_path(buildroot);

	// first download the packages
	let product_file = fs::full_path("./products/" + product + ".toml");
	let product_toml = toml::read(product_file);
	let product_name = product_toml.product;

	let br_out_dir = new_br_out_dir(buildroot, board, channel);
	let images = br_out_dir + "/images";
	if !fs::is_file(images + "/image.tar.gz") {
		panic("you first need to build " + board + " " + channel);
	}

	let publisher = cmd([
		"publisher", "download", product_file
	]);
	publisher.dir(images);
	publisher.execute();

	// create the image
	let full_images = fs::full_path(images);

	let board_riji = cmd([
		"riji", "create_image", channel, full_images, full_buildroot
	]);
	board_riji.dir("./board/" + board);
	board_riji.execute();

	// create output
	let out_dir = "./output/" + board + "/" + channel;
	fs::create_dir(out_dir);

	let img_file = product_name + ".img";
	let img_path = out_dir + "/" + img_file;

	fs::copy(images + "/disk.img", img_path);

	// create vdi file if vbox is installed
	if fs::is_prog("VBoxManage") {
		let vdi_path = out_dir + "/" + product_name + ".vdi";
		fs::delete(vdi_path);
		let vbox = cmd([
			"VBoxManage", "convertfromraw", "--format", "VDI",
			img_path, vdi_path
		]);
		vbox.execute();
		// the reverse is VBoxManage clonemedium --format RAW debian.vdi debian.img
	}

	print("created: " + img_path);
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

fn arch_by_board(board) {
	if board == "intel" {
		return "Amd64";
	} else if board == "intel-headless" {
		return "Amd64";
	} else if board == "pi4" {
		return "Arm64";
	} else if board == "pi4-headless" {
		return "Arm64";
	} else {
		panic("board " + board + " not found")
	}
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
	rust_build(arch, channel, path, binary, "Release");
}

fn rust_build(arch, channel, path, binary, hostChannel) {
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

	let minus_sign = regex("-");
	let upper_target = minus_sign.replace(uppercase(target), "_");

	let host = full_host_path(board, hostChannel);
	let sysroot = host + "/" + buildroot_target + "/sysroot";
	let cargo_bin = host + "/bin/cargo";
	let cargo_home = host + "/share/cargo";

	let cargo = cmd([cargo_bin, "build", "--release"]);
	cargo.dir(path);
	cargo.env_clear();
	cargo.env("BUILD_CHANNEL", channel);
	cargo.env("BUILD_SYSROOT", sysroot);
	cargo.env("PKG_CONFIG_SYSROOT_DIR", sysroot);
	cargo.env("PATH", host + "/bin" + ":" + "/usr/bin");
	cargo.env("CARGO_HOME", cargo_home);
	cargo.env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly");
	cargo.env("CARGO_UNSTABLE_TARGET_APPLIES_TO_HOST", "true");
	cargo.env("CARGO_TARGET_APPLIES_TO_HOST", "false");
	cargo.env("CARGO_BUILD_TARGET", target);
	cargo.env("CARGO_TARGET_" + upper_target + "_LINKER", buildroot_target + "-gcc");
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
	return full_host_path(board, "Release");
}

/// buildChannel: Debug | Release
fn full_host_path(board, buildChannel) {
	let host_raw = "./buildroot/output/" + board + "/" + buildChannel + "/host";
	if !fs::is_dir(host_raw) {
		panic("host folder not found: " + host_raw);
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
	sysroot(arch, board, "Release");
}

fn sysroot(arch, board, buildChannel) {
	let buildroot_target;
	if arch == "Amd64" {
		buildroot_target = "x86_64-buildroot-linux-gnu";
	} else if arch == "Arm64" {
		buildroot_target = "aarch64-buildroot-linux-gnu";
	} else {
		panic("only Amd64 && Arm64 supported");
	}

	let host = full_host_path(board, buildChannel);
	let sysroot = host + "/" + buildroot_target + "/sysroot";

	print(sysroot);
}