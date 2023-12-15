fn docker_build() {
	fs::create_dir("./deps");
	_copy_dep("file-db");
	_copy_dep("packages-api");

	let docker = cmd(["docker", "build", "-t", "packages-server", "."]);
	docker.execute();
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

fn docker_save() {
	fs::create_dir("./images");

	let docker = cmd([
		"docker", "save", "-o", "./images/packages-server.tar",
		"packages-server"
	]);
	docker.execute();
}