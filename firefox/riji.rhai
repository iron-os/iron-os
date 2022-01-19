// help: https://firefox-source-docs.mozilla.org/setup/linux_build.html

const DIR = "./mozilla-unified";
const STATE_DIR = "./state";

fn help() {
	print([
		"Firefox riji help:",
		" - prepare // installs necessary stuff",
		" - download // downloads mozilla source code",
		" - config // configures the build setup",
		" - build // build firefox",
		" - prebuilt // download pre built version"
	]);
}

fn prepare() {
	let inst_mercurial = cmd([
		"pip3", "install", "--user", "mercurial"
	]);
	inst_mercurial.execute();
}

fn download() {
	let curl = cmd([
		"curl", "https://hg.mozilla.org/mozilla-central/raw-file/default/python/mozboot/bin/bootstrap.py", "-O"
	]);
	curl.execute();
	let bootstrap = cmd([
		"python3", "bootstrap.py", "--vcs=git", "--no-interactive"
	]);
	bootstrap.execute();
}

// configure ./mach configure

fn config() {
	// ac_add_options --target-sysroot=SYSROOT
	let cfg = fs::read("./configs/default");

	let buildroot_riji = cmd(["riji", "sysroot", "Amd64", "intel"]);
	buildroot_riji.dir("./../buildroot");
	let sysroot = trim(buildroot_riji.output());

	// cfg += "\nac_add_options --target-sysroot=" + sysroot;

	fs::write(DIR + "/mozconfig", cfg);

	if !fs::is_dir(STATE_DIR) {
		fs::create_dir(STATE_DIR);
	}

	let mach = cmd([
		"./mach", "configure"
	]);
	mach.env("MOZBUILD_STATE_PATH", fs::full_path(STATE_DIR));
	mach.dir(DIR);
	mach.execute();
}

fn build() {

	let mach = cmd([
		"./mach", "build"
	]);
	mach.env("MOZBUILD_STATE_PATH", fs::full_path(STATE_DIR));
	mach.dir(DIR);
	mach.execute();

}

fn bundle() {
	let out_dir = "./output/default";
	fs::delete(out_dir);
	fs::create_dir(out_dir);

	bundle(DIR + "/out/default/dist/bin", out_dir);
}

fn bundle(build_dir, out_dir) {
	copy(build_dir + "/application.ini", out_dir);
	copy(build_dir + "/browser", out_dir);
	copy(build_dir + "/crashreporter", out_dir);
	copy(build_dir + "/crashreporter.ini", out_dir);
	copy(build_dir + "/defaults", out_dir);
	copy(build_dir + "/dependentlibs.list", out_dir);
	copy(build_dir + "/components", out_dir);
	//copy(build_dir + "/distribution", out_dir);
	copy(build_dir + "/firefox", out_dir);
	//copy(build_dir + "/firefox.sh", out_dir);
	copy(build_dir + "/fonts", out_dir);
	copy(build_dir + "/gmp-clearkey", out_dir);
	//copy(build_dir + "/libfreeblpriv3.chk", out_dir);
	copy(build_dir + "/libfreeblpriv3.so", out_dir);
	copy(build_dir + "/liblgpllibs.so", out_dir);
	copy(build_dir + "/libmozavcodec.so", out_dir);
	copy(build_dir + "/libmozavutil.so", out_dir);
	copy(build_dir + "/libmozgtk.so", out_dir);
	copy(build_dir + "/libmozsandbox.so", out_dir);
	copy(build_dir + "/libmozsqlite3.so", out_dir);
	copy(build_dir + "/libmozwayland.so", out_dir);
	copy(build_dir + "/libnspr4.so", out_dir);
	copy(build_dir + "/libnss3.so", out_dir);
	copy(build_dir + "/libnssckbi.so", out_dir);
	copy(build_dir + "/libnssutil3.so", out_dir);
	copy(build_dir + "/libplc4.so", out_dir);
	copy(build_dir + "/libplds4.so", out_dir);
	copy(build_dir + "/libsmime3.so", out_dir);
	//copy(build_dir + "/libsoftokn3.chk", out_dir);
	copy(build_dir + "/libsoftokn3.so", out_dir);
	copy(build_dir + "/libssl3.so", out_dir);
	copy(build_dir + "/libxul.so", out_dir);
	copy(build_dir + "/minidump-analyzer", out_dir);
	//copy(build_dir + "/omni.ja", out_dir);
	copy(build_dir + "/platform.ini", out_dir);
	copy(build_dir + "/plugin-container", out_dir);
	copy(build_dir + "/Throbber-small.gif", out_dir);
}

fn copy(from, to) {
	let cp = cmd(["cp", "-Lr", from, to]);
	cp.execute();
}

fn prebuilt() {
	let curl = cmd([
		"curl",
		"https://download-installer.cdn.mozilla.net/pub/firefox/" +
		"releases/96.0.1/linux-x86_64/en-US/firefox-96.0.1.tar.bz2",
		"--output",
		"firefox.tar.bz2"
	]);

	let out_dir = "./output";
	let def_out_dir = "./output/default";

	fs::delete(def_out_dir);
	fs::create_dir(def_out_dir);

	curl.dir(def_out_dir);
	curl.execute();

	// we now have a firefox.tar.bz2
	// extract file
	let tar = cmd([
		"tar", "-xvf", "firefox.tar.bz2"
	]);
	tar.dir(def_out_dir);
	tar.execute();

	fs::delete(def_out_dir + "/firefox.tar.bz2");
	fs::delete(def_out_dir + "/firefox/updater");
	fs::delete(def_out_dir + "/firefox/updater.ini");
	fs::delete(def_out_dir + "/firefox/update-settings.ini");
}