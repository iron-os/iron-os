

const PATCH_FILE = "./patches.patch";
const DEPOT = "./depot_tools";
const CHROM_DEPOT = "../../depot_tools";
const CHROM = "./chromium/src";
const STABLE = "92.0.4515.131";


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

fn configure() {
	print("call with build release or build debug");
}

fn configure(target) {
	print("build target: " + target);
	let out_path = "out/" + target;
	let chrome_out = CHROM + "/" + out_path;
	fs::create_dir(chrome_out);

	// inspiration was taken from
	// https://github.com/OSSystems/meta-browser/blob/master/meta-chromium/

	let args_file = chrome_out + "/args.gn";
	fs::copy("./args/default.gn", args_file);
	fs::append(args_file, [
		"",
		fs::read("./args/" + target + ".gn")
	]);

	// generate folder
	let gn = cmd(CHROM_DEPOT + "/gn");
	gn.dir(CHROM);
	gn.args(["gen", out_path]);
	gn.execute();
}

fn build() {
	print("call with build release or build debug");
}

fn build(target) {
	print("build target: " + target);
	let out_path = "out/" + target;

	// generate folder
	let ninja = cmd(CHROM_DEPOT + "/autoninja");
	ninja.dir(CHROM);
	ninja.args(["-C", out_path, "chrome", "chrome_sandbox", "chrome/installer/linux:stable"]);
	ninja.execute();

	// copy out/release
	/*let files = [
		"chrome-sandbox",
		"chrome", // chromium-bin?
		"icudtl.dat"
	];*/
}

/*
RuntimeError: requested profile "/big-drive/chromium/chromium/src/chrome/build/pgo_profiles/chrome-linux-4515-1627572323-7b80f2d12f40afe6f7872c5528a069e31662276b.profdata" doesn't exist, please make sure "checkout_pgo_profiles" is set to True in the "custom_vars" section of your .gclient file, e.g.: 
solutions = [ 
  { 
    "name": "src", 
    # ...  
    "custom_vars": { 
      "checkout_pgo_profiles": True, 
    }, 
  }, 
], 
and then run "gclient runhooks" to download it. You can also simply disable the PGO optimizations by setting |chrome_pgo_phase = 0| in your GN arguments.

*/

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
fn clean() {
	let yes = prompt("really remove unchaged files");
	if yes == false {
		print("nothing done");
		return;
	}
	let main = git(CHROM);
	main.force_head();
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