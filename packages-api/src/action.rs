use stream_api::Action as ActionTrait;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ActionTrait)]
#[repr(u16)]
pub enum Action {
	// AllPackages = 10,
	PackageInfo = 11,
	SetPackageInfo = 13,
	// ChangeWhitelistV1 = 15
	ChangeWhitelist = 16,
	GetFile = 20,
	GetFilePart = 21,
	SetFile = 22,

	NewAuthKeyReader = 31,
	AuthenticateReader = 32,
	AuthenticateWriter1 = 34,
	AuthenticateWriter2 = 35,
}
