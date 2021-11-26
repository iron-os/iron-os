
use stream::packet::PlainBytes;
use stream::basic::{message};

macro_rules! action {
	($($name:ident = $num:expr),*) => (
		#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
		pub enum Action {
			$($name),*
		}

		impl message::Action for Action {

			fn empty() -> Self {
				Self::Empty
			}

			fn from_u16(num: u16) -> Option<Self> {
				match num {
					$($num => Some(Self::$name)),*,
					_ => None
				}
			}

			fn as_u16(&self) -> u16 {
				match self {
					$(Self::$name => $num),*
				}
			}

		}
	)
}

action!{
	Empty = 0,
	SystemInfo = 4,
	DeviceInfo = 7,
	OpenPage = 10,
	SetDisplayState = 13,
	Disks = 16,
	InstallOn = 17,
	SetPowerState = 20,
	// packages
	ListPackages = 30,
	AddPackage = 32,
	RemovePackage = 34,
	// storage
	GetStorage = 40,
	SetStorage = 42,
	RemoveStorage = 44
}

pub type Message = message::Message<Action, PlainBytes>;