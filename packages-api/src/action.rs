
use stream_api::action;

action! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
	pub enum Action {
		Empty = 0,
		// AllPackages = 10,
		PackageInfo = 11,
		SetPackageInfo = 13,
		GetFile = 20,
		SetFile = 22,

		NewAuthKey = 30,
		Authentication = 32
	}
}
