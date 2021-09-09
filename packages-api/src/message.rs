
use stream::packet::{Packet, EncryptedBytes, PacketBytes, PacketHeader, Result, PacketError};
use bytes::{Bytes, BytesMut, BytesRead, BytesWrite};



macro_rules! kind {
	($($name:ident = $num:expr),*) => (
		#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
		pub enum Kind {
			$($name),*
		}

		impl Kind {

			pub fn from_u16(num: u16) -> Option<Self> {
				match num {
					$($num => Some(Self::$name)),*,
					_ => None
				}
			}

			pub fn as_u16(&self) -> u16 {
				match self {
					$(Self::$name => $num),*
				}
			}

		}
	)
}

kind!{
	Empty = 0
}


#[derive(Debug)]
pub struct Header {
	body_len: u32,
	flags: u8,
	id: u32,
	msg_kind: Kind
}

impl Header {

	pub fn empty() -> Self {
		Self {
			body_len: 0,
			flags: 0,
			id: 0,
			msg_kind: Kind::Empty
		}
	}

	pub fn to_bytes(&self, mut bytes: BytesMut) {
		bytes.write_u32(self.body_len);
		bytes.write_u8(self.flags);
		bytes.write_u32(self.id);
		bytes.write_u16(self.msg_kind.as_u16());
	}

}

impl PacketHeader for Header {
	fn len() -> usize {
		4 + 1 + 4 + 2
	}

	fn from_bytes(mut bytes: Bytes) -> Result<Self> {
		let me = Self {
			body_len: bytes.read_u32(),
			flags: bytes.read_u8(),
			id: bytes.read_u32(),
			msg_kind: Kind::from_u16(bytes.read_u16())
				.ok_or_else(|| PacketError::Header("Kind unknown".into()))?
		};

		Ok(me)
	}

	fn body_len(&self) -> usize {
		self.body_len as usize
	}

	fn flags(&self) -> u8 {
		self.flags
	}

	fn set_flags(&mut self, flags: u8) {
		self.flags = flags;
	}

	fn id(&self) -> u32 {
		self.id
	}

	fn set_id(&mut self, id: u32) {
		self.id = id;
	}
}

#[derive(Debug)]
pub struct Message {
	header: Header,
	bytes: EncryptedBytes
}

impl Packet<EncryptedBytes> for Message {
	type Header = Header;

	fn header(&self) -> &Header {
		&self.header
	}

	fn header_mut(&mut self) -> &mut Header {
		&mut self.header
	}

	fn empty() -> Self {
		Self {
			header: Header::empty(),
			bytes: EncryptedBytes::new(Header::len())
		}
	}

	fn from_bytes_and_header(bytes: EncryptedBytes, header: Header) -> Result<Self> {
		Ok(Self { header, bytes })
	}

	fn into_bytes(mut self) -> EncryptedBytes {
		let body_len = self.bytes.body().len();
		self.header.body_len = body_len as u32;
		self.header.to_bytes(self.bytes.header_mut());
		self.bytes
	}
}