

const PATCH_FILE = "./patches.patch";
const DEPOT = "./depot_tools";
const CHROM_DEPOT = "../../depot_tools";
const CHROM = "./chromium/src";
const STABLE = "107.0.5304.87";


fn help() {
	print([
		"Chromium Riji help:",
		"	- download",
		"	- update // download branch infos and tags (can take a while)",
		"	- checkout // checkout to the current stable tag",
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
	let fetch = cmd("../" + DEPOT + "/fetch");
	fetch.dir("./chromium");
	fetch.args(["--nohooks", "chromium"]);
	fetch.execute();

	// install build deps
	let build_deps = cmd("./build/install-build-deps.sh");
	build_deps.dir(CHROM);
	build_deps.execute();

	_add_pgo(CHROM_DEPOT, CHROM);

	_checkout(CHROM_DEPOT, CHROM, STABLE);
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
	_checkout(CHROM_DEPOT, CHROM, STABLE);
}

fn _checkout(depot, chrome, stable) {
	// sync tags
	let pre_sync = cmd(depot + "/gclient");
	pre_sync.dir(chrome);
	pre_sync.args(["sync", "--with_tags"]);
	pre_sync.execute();

	// checkout current branch
	let repo = git(chrome);
	repo.checkout_tag(stable);

	// remove unused stuff
	let gclient = cmd(depot + "/gclient");
	gclient.dir(chrome);
	gclient.args(["sync", "-D", "--force", "--with_tags"]);
	gclient.execute();
}

fn add_pgo() {
	_add_pgo(CHROM_DEPOT, CHROM);
}

fn _add_pgo(depot, chrome) {
	// "custom_vars": {}
	let custom_vars = regex("custom_vars"": \\{\\}");
	custom_vars.replace_file("./chromium/.gclient", "custom_vars"": {" +
		"""checkout_pgo_profiles"": True" +
	"}");

	let pre_sync = cmd([
		depot + "/gclient",
		"runhooks"
	]);
	pre_sync.dir(chrome);
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

fn _configure(depot, chrome, arch, channel, sysrootChannel) {
	//_patch(chrome);

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

	let buildroot_riji = cmd([
		"riji", "sysroot", real_arch, board, sysrootChannel
	]);
	buildroot_riji.dir("./../buildroot");
	let sysroot = trim(buildroot_riji.output());
	print("using sysroot: " + sysroot);

	fs::append(args_file, [
		"",
		"target_sysroot = \"" + sysroot + "\""
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
	build(arch, channel, "Release");
}

fn build(arch, channel, sysrootChannel) {
	_configure(CHROM_DEPOT, CHROM, arch, channel, sysrootChannel);

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
	// fs::copy(build_path + "/swiftshader", out_path);
	fs::copy(build_path + "/vk_swiftshader_icd.json", out_path);
	fs::create_dir(out_path + "/locales");
	fs::copy(build_path + "/locales/en-US.pak", out_path + "/locales");
	fs::copy(build_path + "/locales/de.pak", out_path + "/locales");
	fs::copy(build_path + "/MEIPreload", out_path);
	// fs::copy(build_path + "/wayland_scanner", out_path);
}

// creates the patch
fn create() {
	print("create patch");

	let main = git(CHROM);
	let diff = main.diff();

	diff.to_file(PATCH_FILE);
}

fn patch() {
	_patch(CHROM);
}

// apply the patch
fn _patch(chrome) {
	let main = git(chrome);
	main.force_head();

	print("main");
	let diff = diff_from_file("./patches/all.patch");
	main.apply_diff(diff);

	// ffmpeg
	print("patch ffmpeg");
	patch_git(chrome + "/third_party/ffmpeg", "./patches/ffmpeg.patch");

	// skia
	print("patch skia");
	patch_git(chrome + "/third_party/skia", "./patches/skia.patch");

	// native client
	print("patch native client");
	patch_git(chrome + "/native_client", "./patches/native-client.patch");

	// devtools frontend
	print("patch devtools");
	patch_git(chrome + "/third_party/devtools-frontend/src", "./patches/devtools-frontend.patch");

	// v8
	print("patch v8");
	patch_git(chrome + "/v8", "./patches/v8.patch");
}

fn patch_git(git_dir, patch) {
	let g = git(git_dir);
	g.force_head();

	let diff = diff_from_file(patch);
	g.apply_diff(diff);
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
	let main = git(CHROM);
	main.force_head();

	// ffmpeg
	let ffmpeg = git(CHROM + "/third_party/ffmpeg");
	ffmpeg.force_head();

	// skia
	let skia = git(CHROM + "/third_party/skia");
	skia.force_head();

	// native client
	let nc = git(CHROM + "/native_client");
	nc.force_head();

	// devtools
	let nc = git(CHROM + "/third_party/devtools-frontend/src");
	nc.force_head();

	// v8
	let nc = git(CHROM + "/v8");
	nc.force_head();

	// breakpad
	let nc = git(CHROM + "/third_party/breakpad/breakpad");
	nc.force_head();
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