
use stream_api::action;

action! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
	pub enum Action {
		Empty = 0,
		// AllPackages = 10,
		PackageInfo = 11,
		SetPackageInfo = 13,
		ChangeWhitelist = 15,
		GetFile = 20,
		SetFile = 22,

		NewAuthKeyReader = 31,
		AuthenticateReader = 32,
		AuthenticateWriter1 = 34,
		AuthenticateWriter2 = 35
	}
}
