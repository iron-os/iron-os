

const PATCH_FILE = "./patches.patch";
const DEPOT = "./depot_tools";
const CHROM_DEPOT = "../../depot_tools";
const CHROM = "./chromium/src";
const STABLE = "96.0.4664.110";


fn help() {
	print([
		"Chromium Riji help:",
		"	- download",
		" - update // download branch infos and tags (can take a while)",
		" - checkout // checkout to the current stable tag",
		"	- configure <release>",
		"	- build <release>",
		"	- create // creates a patch",
		"	- patch // applies the patch",
		"	- patch_3way // applies the patch with 3ways",
		"	- clean // removes all uncommitted changes"
	])
}

fn download() {
	let depot_tool = git_clone("https://chromium.googlesource.com/chromium/tools/depot_tools.git", DEPOT);
	fs::create_dir("./chromium");

	// download chromium
	//let fetch = cmd("../" + DEPOT + "/fetch");
	//fetch.dir("./chromium");
	//fetch.args(["--nohooks", "chromium"]);
	//fetch.execute();

	// install build deps
	let build_deps = cmd("./build/install-build-deps.sh");
	build_deps.dir(CHROM);
	build_deps.execute();

	//checkout();
	print("\n > call checkout");
}

fn update() {
	let pre_sync = cmd(CHROM_DEPOT + "/gclient");
	pre_sync.dir(CHROM);
	pre_sync.args(["sync", "--with_branch_heads", "--with_tags"]);
	pre_sync.execute();

	let git = cmd(["git", "fetch", "origin"]);
	git.dir(CHROM);
	git.execute();
}

fn checkout() {
	// sync tags
	let pre_sync = cmd(CHROM_DEPOT + "/gclient");
	pre_sync.dir(CHROM);
	pre_sync.args(["sync", "--with_tags"]);
	pre_sync.execute();

	// checkout current branch
	let repo = git(CHROM);
	repo.checkout_tag(STABLE);

	// remove unused stuff
	let gclient = cmd(CHROM_DEPOT + "/gclient");
	gclient.dir(CHROM);
	gclient.args(["sync", "-D", "--force", "--with_tags"]);
	gclient.execute();
}

fn add_pgo() {
	// "custom_vars": {}
	let custom_vars = regex("custom_vars"": \\{\\}");
	custom_vars.replace_file("./chromium/.gclient", "custom_vars"": {" +
		"""checkout_pgo_profiles"": True" +
	"}");

	let pre_sync = cmd([
		CHROM_DEPOT + "/gclient",
		"runhooks"
	]);
	pre_sync.dir(CHROM);
	pre_sync.execute();
}

fn all_args() {
	print("all_args <arch> <channel>");
}

fn all_args(arch, channel) {
	_configure(CHROM_DEPOT, CHROM, arch, channel);

	arch = lowercase(arch);
	channel = lowercase(channel);
	let out_path = "./out/" + arch + "/" + channel;

	let gn = cmd([
		"gn", "args", "--list",
		out_path
	]);
	gn.dir(CHROM);
	let args = gn.output();
	fs::write("./args.txt", args);
}

fn configure() {
	print("configure <arch> <channel>");
}

fn configure(arch, channel) {
	_configure(CHROM_DEPOT, CHROM, arch, channel);
}

fn _configure(depot, chrome, arch, channel) {
	let real_arch = arch;
	arch = lowercase(arch);
	channel = lowercase(channel);
	let out_path = "out/" + arch + "/" + channel;
	let chrome_out = chrome + "/" + out_path;
	fs::create_dir(chrome_out);

	// inspiration was taken from
	// https://github.com/OSSystems/meta-browser/blob/master/meta-chromium/

	let args_file = chrome_out + "/args.gn";
	fs::copy("./args/default.gn", args_file);
	fs::append(args_file, [
		"",
		fs::read("./args/" + arch + ".gn")
	]);
	fs::append(args_file, [
		"",
		fs::read("./args/" + channel + ".gn")
	]);

	let board;
	if real_arch == "Amd64" {
		board = "intel";
	} else if real_arch == "Arm64" {
		board = "pi4";
	} else {
		panic("only Amd64 && Arm64 are supported");
	}

	let buildroot_riji = cmd(["riji", "sysroot", real_arch, board]);
	buildroot_riji.dir("./../buildroot");
	let sysroot = trim(buildroot_riji.output());
	print("using sysroot: " + sysroot);

	fs::append(args_file, [
		"",
		"sysroot = \"" + sysroot + "\""
	]);

	// generate folder
	let gn = cmd(depot + "/gn");
	gn.dir(chrome);
	gn.args(["gen", out_path]);
	gn.execute();
}

fn build() {
	print("build <arch> <channel>");
}

fn build(arch, channel) {
	_configure(CHROM_DEPOT, CHROM, arch, channel);

	print("build arch: " + arch + " channel: " + channel);
	arch = lowercase(arch);
	channel = lowercase(channel);
	let out_path = "out/" + arch + "/" + channel;

	// generate folder
	let ninja = cmd(CHROM_DEPOT + "/autoninja");
	ninja.dir(CHROM);
	ninja.args(["-C", out_path, "chrome", "chrome_sandbox"]);
	ninja.execute();

	// copy out/release
	/*let files = [
		"chrome-sandbox",
		"chrome", // chromium-bin?
		"icudtl.dat"
	];*/

	bundle(
		CHROM,
		CHROM + "/" + out_path,
		"./output/" + arch + "/" + channel + "/chromium"
	);
}

// copy's every file that is needed
// out path must already exist
fn bundle(chrome, build_path, out_path) {
	if fs::is_dir(out_path) {
		fs::delete(out_path);
	}
	fs::create_dir(out_path);

	fs::delete(build_path + "/chrome.stripped");
	fs::delete(build_path + "/chrome.symbols");

	// strip binary
	let eu_strip = cmd([
		fs::full_path(chrome + "/buildtools/third_party/eu-strip/bin/eu-strip"),
		"-o", "chrome.stripped",
		"-f", "chrome.symbols",
		"chrome"
	]);
	eu_strip.dir(build_path);
	eu_strip.execute();

	fs::copy(build_path + "/chrome.stripped", out_path + "/chrome");
	fs::copy(build_path + "/chrome_crashpad_handler", out_path);
	fs::copy(build_path + "/chrome_sandbox", out_path + "/chrome-sandbox");
	fs::copy(build_path + "/libEGL.so", out_path);
	fs::copy(build_path + "/libGLESv2.so", out_path);
	fs::copy(build_path + "/libvulkan.so.1", out_path);
	fs::copy(build_path + "/libvk_swiftshader.so", out_path);
	fs::copy(build_path + "/libVkLayer_khronos_validation.so", out_path);
	fs::copy(build_path + "/icudtl.dat", out_path);
	fs::copy(build_path + "/v8_context_snapshot.bin", out_path);
	fs::copy(build_path + "/chrome_100_percent.pak", out_path);
	fs::copy(build_path + "/chrome_200_percent.pak", out_path);
	fs::copy(build_path + "/resources.pak", out_path);
	fs::copy(build_path + "/swiftshader", out_path);
	fs::copy(build_path + "/vk_swiftshader_icd.json", out_path);
	fs::create_dir(out_path + "/locales");
	fs::copy(build_path + "/locales/en-US.pak", out_path + "/locales");
	fs::copy(build_path + "/locales/de.pak", out_path + "/locales");
	fs::copy(build_path + "/MEIPreload", out_path);
	fs::copy(build_path + "/wayland_scanner", out_path);
}

// creates the patch
fn create() {
	print("create patch");

	let main = git(CHROM);
	let diff = main.diff();

	diff.to_file(PATCH_FILE);
}

// apply the patch
fn patch() {
	print("patch");

	let diff = diff_from_file(PATCH_FILE);
	let main = git(CHROM);
	main.apply_diff(diff);
}

fn patch_3way() {
	print("patch 3way");
	let make_patch = cmd("git");
	make_patch.dir(CHROM);
	make_patch.args(["apply", "--3way", PATCH_FILE]);
	make_patch.execute();
}

// removes all unchached files in main
fn clean_changes() {
	let yes = prompt("really remove unchaged files");
	if yes == false {
		print("nothing done");
		return;
	}
	let main = git(CHROM);
	main.force_head();
}

fn clean(target) {
	fs::delete(CHROM + "/out/" + target);
}


/*
zooming

ash/accessibility/magnifier controls the pinch zoom
*/


/*
setup device

// lookup touchcontrollers
xinput
// get props
xinput --list-props <id>
// set props
xinput set-prop <id> <prop-id> <args>
*/