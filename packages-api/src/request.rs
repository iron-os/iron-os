
use crate::error::{Result, Error};
use crate::message::Message;

pub trait Request: Sized {
	type Response: Response;
	fn into_message(self) -> Result<Message>;
	fn from_message(msg: Message) -> Result<Self>;
}

pub trait Response: Sized {
	fn into_message(self) -> Result<Message>;
	fn from_message(msg: Message) -> Result<Self>;
}