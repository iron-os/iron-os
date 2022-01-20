
fn help() {
	print([
		"Service riji help:",
		"start_chrome <url> // starts chrome with the extension"
	]);
}

fn start_chrome() {
	print("start_chrome <url>");
}

fn start_chrome(url) {
	let extension = fs::full_path("./extension");
	print("extension " + extension);
	let chrome = cmd([
		"/snap/chromium/current/usr/lib/chromium-browser/chrome",
		"--disable-infobars",
		"--disable-restore-session-state",
		"--disable-session-storage",
		"--disable-rollback-option",
		"--disable-speech-api",
		"--disable-sync",
		"--disable-pinch",
		"--load-extension=" + extension,
		url
	]);
	chrome.execute();
}

fn check() {
	let cargo = cmd(["cargo", "check"]);
	cargo.execute();
	let cargo_rel = cmd(["cargo", "check", "--release"]);
	cargo_rel.execute();
	let cargo_test = cmd(["cargo", "test", "--release"]);
	cargo_test.execute();
}