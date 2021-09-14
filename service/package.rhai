
fn build(channel) {
	let cargo = cmd([
		"cargo", "build", "--release"
	]);
	cargo.execute();
}

fn pack(dest, channel) {
	fs::copy("./target/release/service", dest);

	if fs::is_dir("./www/fire-html") == false {
		panic("fire-html required");
	}
	fs::copy("./www", dest);

	if fs::is_dir("./extension/fire-html") == false {
		panic("fire-html required");
	}
	fs::copy("./extension", dest);
}