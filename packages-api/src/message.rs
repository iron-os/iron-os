
use stream::packet::EncryptedBytes;
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
	AllPackages = 10,
	PackageInfo = 11,
	ImageInfo = 15,
	GetFile = 20
}

pub type Message = message::Message<Action, EncryptedBytes>;