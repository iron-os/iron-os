macro_rules! serde_req {
	($action:expr, $req:ident, $resp:ident) => (
		impl stream::basic::request::Request<Action, stream::packet::EncryptedBytes> for $req {
			type Response = $resp;

			fn action() -> Action {
				$action
			}
			fn into_message(self) -> stream::Result<Message> {
				Message::serialize(&self)
			}
			fn from_message(msg: Message) -> stream::Result<Self> {
				msg.deserialize()
			}
		}
	)
}

macro_rules! serde_res {
	($res:ident) => (
		impl stream::basic::request::Response<Action, stream::packet::EncryptedBytes> for $res {
			fn into_message(self) -> stream::Result<Message> {
				Message::serialize(&self)
			}
			fn from_message(msg: Message) -> stream::Result<Self> {
				msg.deserialize()
			}
		}
	)
}