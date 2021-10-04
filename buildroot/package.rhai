
fn sanitize_channel(channel) {
	if channel == "Debug" {
		return "Debug";
	} else if channel == "Alpha" {
		return "Debug";
	} else if channel == "Beta" {
		return "Release";
	} else if channel == "Release" {
		return "Release";
	} else {
		panic("channel: " + channel + "not recognized")
	}
}

fn build(channel) {
	let channel = sanitize_channel(channel);
	fs::copy("./output/" + channel + "/image.tar.gz", "./");
}