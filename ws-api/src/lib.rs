
use std::collections::HashMap;
use std::sync::Arc;
use std::io;

use fire::ws::JsonError;

use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;

#[doc(hidden)]
pub use fire::ws_route;

#[doc(hidden)]
pub use tokio::sync::mpsc;

#[doc(hidden)]
pub use tokio::task::JoinHandle;

/*

Websocket Api

- client can send requests
- additionally client can subscribe
  to stream

*/

#[derive(Debug)]
pub enum Error {
	Json(serde_json::Error),
	Fire(JsonError),
	Io(io::Error),
	/// Get's returned if the response was already sent
	AlreadySent,
	StreamClosed
}

pub type Result<T> = std::result::Result<T, Error>;




pub trait Handler<D>: Send + Sync {

	fn name() -> Name
	where Self: Sized;
	// data
	// + 
	// the senders type is () because the handler needs to be generic
	// but with the call transpose it will receive the correct ret type
	/// self here is required because else the function cannot be used with dyn types
	/// and we cannot use &self because it would need to be 'static (because of tokio task)
	fn handle(&self, req: Message, sender: Sender, data: &D) -> JoinHandle<()>;
}

#[macro_export]
macro_rules! msg_handler {

	(for $name_kind:expr, $(#[$attr:meta])* pub async $($toks:tt)*) => (
		$crate::msg_handler!(IMPL, for $name_kind, $(#[$attr])* (pub) async $($toks)*);
	);
	(for $name_kind:expr, $(#[$attr:meta])* pub ($($vis:tt)+) async $($toks:tt)*) => (
		$crate::msg_handler!(IMPL, for $name_kind, $(#[$attr])* (pub ($($vis)+)) async $($toks)*);
	);
	(for $name_kind:expr, $(#[$attr:meta])* async $($toks:tt)*) => (
		$crate::msg_handler!(IMPL, for $name_kind, $(#[$attr])* () async $($toks)*);
	);
	(
		IMPL,
		for $name_kind:expr,
		$(#[$attr:meta])*
		($($vis:tt)*) async fn $name:ident( $($tok_args:tt)* ) $($toks:tt)*
	) => (
		$crate::msg_handler!(
			IMPL,
			for $name_kind,
			$(#[$attr])*
			($($vis)*) async fn $name<Data>( $($tok_args)* ) $($toks)*
		);
	);
	(
		IMPL,
		for $name_kind:expr,
		$(#[$attr:meta])*
		($($vis:tt)*) async fn $name:ident<$data_ty:ty>(
			$req:ident: $req_ty:ty,
			$sender:ident
		) $($toks:tt)*
	) => (
		$crate::msg_handler!(
			IMPL,
			for $name_kind,
			$(#[$attr])*
			($($vis)*) async fn $name<$data_ty>(
				$req: $req_ty,
				$sender,
			) $($toks)*
		);
	);
	(
		IMPL,
		for $name_kind:expr,
		$(#[$attr:meta])*
		($($vis:tt)*) async fn $name:ident<$data_ty:ty>(
			$req:ident: $req_ty:ty,
			$sender:ident,
			$($data:ident),*
		) -> $ret_ty:ty
		$block:block
	) => (
		$(#[$attr])*
		#[allow(non_camel_case_types)]
		$($vis)* struct $name;

		impl $crate::Handler<$data_ty> for $name {
			fn name() -> $crate::Name { $name_kind }
			fn handle(
				&self,
				mut $req: $crate::Message,
				mut $sender: $crate::Sender,
				data: &$data_ty
			) -> $crate::JoinHandle<()> {
				// extract data
				$(
					let mut $data = data.$data().to_owned();
				)*

				tokio::spawn(async move {
					let r: $ret_ty = async move {
						let $req: $req_ty = $req.deserialize()?;
						$block
					}.await;

					if let Err(e) = r {
						eprintln!("ws error {:?}", e);
					}
				})
			}
		}
	);
}


enum SenderKind {
	Stream,
	// stores if a response was already sent
	Response(bool)
}


pub struct Sender {
	id: String,
	name: Name,
	kind: SenderKind,
	inner: mpsc::Sender<Message>
}

impl Sender {
	// returns None if an invalid message kind is give
	#[doc(hidden)]
	pub fn from_msg(msg: &Message, sender: mpsc::Sender<Message>) -> Option<Self> {
		Some(Self {
			id: msg.id.clone(),
			name: msg.name,
			kind: match msg.kind {
				MessageKind::Request => SenderKind::Response(false),
				MessageKind::RequestStream => SenderKind::Stream,
				MessageKind::Push |
				MessageKind::Response => return None
			},
			inner: sender
		})
	}

	pub async fn send<T>(&mut self, data: T) -> Result<()>
	where T: Serialize {
		let kind = match &mut self.kind {
			SenderKind::Stream => MessageKind::Push,
			SenderKind::Response(true) => return Err(Error::AlreadySent),
			SenderKind::Response(r) => {
				*r = true;
				MessageKind::Response
			}
		};

		// todo what should happen if there is an error
		self.inner.send(Message {
			id: self.id.clone(),
			// or is this a push??
			kind,
			name: self.name,
			data: serde_json::to_value(data)
				.map_err(Error::Json)?
		}).await
			.map_err(|_| Error::StreamClosed)
	}

}

/// this is the kind of message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Name {
	VersionInfo
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageKind {
	Request,
	RequestStream,
	Push,
	Response
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
	pub id: String,
	pub kind: MessageKind,
	pub name: Name,
	pub data: serde_json::Value
}

impl Message {

	pub fn deserialize<T>(self) -> Result<T>
	where T: DeserializeOwned {
		serde_json::from_value(self.data)
			.map_err(Error::Json)
	}

}



pub struct ConnectionBuilder<D> {
	handlers: HashMap<Name, Box<dyn Handler<D>>>
}

impl<D> ConnectionBuilder<D> {

	pub fn new() -> Self {
		Self {
			handlers: HashMap::new()
		}
	}

	pub fn register<H>(&mut self, handler: H)
	where H: Handler<D> + 'static {
		self.handlers.insert(H::name(), Box::new(handler));
	}

	/// returns the connection which should be added to data
	/// and the background task that will only exit
	/// if something panics
	pub fn build(self) -> Connection<D> {
		Connection {
			inner: Arc::new(self.handlers)
		}
	}

}

// async fn bg_task(
// 	builder: ConnectionBuilder,
// 	rx: mpsc::Receiver<(Message, mpsc::Sender<Message>)>
// ) {
// 	/*
// 	tasks:
// 	 - listen for new messages
// 	*/
// 	loop {

// 		let (msg, sender) = rx.recv().await.expect("connection closed");

// 		// create a real sender
// 		sender = match Sender::from_msg(&msg, sender) {
// 			Some(sender) => sender,
// 			None => {
// 				eprintln!("received invalid msg {:?}", msg);
// 				continue
// 			}
// 		};

// 		// now search a handler
// 		let handler = builder.handlers.get(&msg.name);

// 		match handler {
// 			Some(handler) => {
// 				handler.handle(msg, sender, )
// 			},
// 			None => {
// 				todo!("send a response that we don't have this handler")
// 			}
// 		}

// 	}
// }

#[derive(Clone)]
pub struct Connection<D> {
	inner: Arc<HashMap<Name, Box<dyn Handler<D>>>>
}

impl<D> Connection<D> {

	#[doc(hidden)]
	pub fn handle(&self, msg: Message, sender: Sender, data: &D) {
		let handler = self.inner.get(&msg.name);
		match handler {
			Some(handler) => {
				handler.handle(msg, sender, data);
				// maybe log the the handlers result
			},
			None => {
				todo!("send a response that we don't have this handler")
			}
		}
	}

}

#[macro_export]
macro_rules! route {
	($name:ident, $($toks:tt)*) => (
		$crate::route!($name<Data>, $($toks)*);
	);
	($name:ident<$data_ty:ty>, $path:expr, $ws_data:ident, $connection:ident) => (
		$crate::ws_route! {
			$name<$data_ty>, $path, |ws, $ws_data, $connection| -> $crate::Result<()> {
				let (tx, mut rx) = $crate::mpsc::channel(10);
				loop {

					tokio::select!{
						msg = ws.deserialize() => {
							let msg = msg.map_err($crate::Error::Fire)?;
							let msg = match msg {
								Some(m) => m,
								None => return Ok(())
							};

							let sender = match $crate::Sender::from_msg(&msg, tx.clone()) {
								Some(sender) => sender,
								None => {
									eprintln!("received invalid msg {:?}", msg);
									continue
								}
							};

							$connection.handle(msg, sender, &$ws_data);
						},
						Some(msg) = rx.recv() => {
							ws.serialize(&msg).await
								.map_err($crate::Error::Fire)?;
						}
					}

				}
			},
			|result| {
				if let Err(e) = result {
					eprintln!("websocket error {:?}", e);
				}
			}
		}
	)
}