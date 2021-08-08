

const PATCH_FILE = "./patches.patch";
const BUILDROOT_TAR = "buildroot-2021.02.3.tar.gz";
const DL_BUILDROOT = "https://buildroot.org/downloads/" + BUILDROOT_TAR;


fn help() {
	print([
		"Custom OS Riji help:",
		"	- build <release>",
		"	- create // creates a patch",
		"	- patch // applies the patch",
		"	- patch_3way // applies the patch with 3ways",
		"	- clean // removes all uncommitted changes"
	])
}

fn download() {
	// download buildroot
	let fetch = cmd("wget");
	fetch.arg(DL_BUILDROOT);
	fetch.execute();

	// extract
	let tar = cmd("tar");
	tar.args(["-xf", BUILDROOT_TAR]);
	tar.execute();

	delete(BUILDROOT_TAR);

	// install build deps
	let build_deps = cmd("./build/install-build-deps.sh");
	build_deps.dir(CHROM);
	build_deps.execute();

	checkout();
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

	// generate folder
	let gn = cmd(CHROM_DEPOT + "/gn");
	gn.dir(CHROM);
	gn.args(["gen", out_path]);
	gn.execute();

	// setup configuration
	let ctn = [
		// for a list gn args out/release 

		// "blink_symbol_level = 1",
		// "branding_file_path"
		// //ui/display/display.gni??
		// bundle_widevine_cdm
		// maybe ? chromeos_is_browser_only
		// dawn_complete_static_libs
		"dawn_enable_desktop_gl = false",
		"dawn_enable_null = false",
		"dawn_enable_opengles = false",
		"dawn_enable_vulkan = true",
		"disable_file_support = true",
		"disable_ftp_support = true",
		//"disable_histogram_support = true",
		// "enable_app_session_service"
		"enable_background_contents = false",
		// enable_background_mode
		"enable_chromecast_extensions = false",
		"enable_chromecast_webui = false",
		// "enable_expensive_dchecks"
		"enable_external_mojo_services = false",
		"enable_feed_v2_modern = false",
		// enable_ink
		"enable_keystone_registration_framework = false",
		"enable_library_cdms = false",
		"enable_media_remoting = false",
		"enable_media_remoting_rpc = false",
		// enable_message_center
		"enable_mojom_message_id_scrambling = false",
		"enable_mojom_typemapping = true",
		"enable_one_click_signin = false",
		// enable_openscreen
		"enable_paint_preview = false",
		"enable_pdf = false",
		// enable_perfetto_ipc
		// enable_platform_hevc
		"enable_print_preview = false",
		"enable_pseudolocales = false",
		"enable_remoting = false",
		"enable_reporting = false",
		"enable_service_discovery = false",
		// enable_stripping
		"enable_system_notifications = false",
		"enable_tagged_pdf = false",
		"enable_vr = false",
		"enable_wayland_server = true",
		"enable_webui_tab_strip = false",
		// ffmpeg_branding
		// grpc_use_static_linking
		// gtk_version
		"have_full_mixer = false",
		"is_component_build = false",
		"is_component_ffmpeg = false",
		"is_debug = false",
		//"is_official_build = true",
		"is_single_volume = true",
		"link_pulseaudio = true",
		"media_use_openh264 = true",
		// monolithic_binaries
		"optimize_webui = true",
		//"ozone_platform = \"wayland\"",
		"pdf_enable_v8 = false",
		"pdf_enable_xfa = false",
		// "proprietary_codecs"
		//"rtc_build_examples = false",
		//"rtc_build_json = false",
		//"rtc_build_libevent = false",
		//"rtc_build_libsrtp = false",
		//"rtc_build_libvpx = false",
		//"rtc_build_opus = false",
		//"rtc_build_ssl = false",
		//"rtc_build_tools = false",
		//"rtc_build_usrsctp = false",
		// probably a lot more rtc
		//"safe_browsing_mode"
		// use_aura
		// use_dawn
		// use_dbus
		// use_gtk
		"use_kerberos = false",
		// use_pangocairo
		// use_system_harfbuzz
		// use_system_freetype
		// use_thin_lto
		// use_v4l2_codec
		// use_vaapi_image_codecs
		"use_viz_devtools = true",
		//"use_x11 = false"
	];

	write_file(CHROM + "/" + out_path + "/args.gn", ctn);
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
	ninja.args(["-C", out_path, "chrome"]);
	ninja.execute();
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