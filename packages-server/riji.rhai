fn publish(image) {
	fs::create_dir("./deps");
	_copy_dep("packages-api");

	let build = cmd([
		"docker", "build", "-t", image, ".", "--pull"
	]);
	build.execute();

	let push = cmd([
		"docker", "push", image
	]);
	push.execute();
}

fn _copy_dep(dep) {
	let final_dir = "./deps/" + dep;
	fs::delete(final_dir);
	fs::create_dir(final_dir);

	for child in fs::read_dir("../" + dep) {
		if child == "target" {
			continue
		}

		fs::copy("../" + dep + "/" + child, final_dir);
	}
}
