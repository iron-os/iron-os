//! This file handles turning the screen on and off
//!
//! This is archieved by talking with weston via a custom wayland protocol.
//!
//! The protocol allows to received notification if the screen state is changed
//! either by us or by user input.
//!
//! Todo: maybe this can be made nicer

use crate::context;

mod kiosk_api;

use kiosk_api::{WestonKioskShell, Event};
pub use kiosk_api::State;

use std::{io, thread};
use std::sync::Arc;

use tokio::sync::{watch, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tokio::io::unix::AsyncFd;

use wayland_client::{
	Display as WlDisplay, GlobalManager, ConnectError, GlobalError
};

#[derive(Debug)]
enum WaylandError {
	Connect(ConnectError),
	Io(io::Error),
	Global(GlobalError),
	ThreadError
}

impl From<ConnectError> for WaylandError {
	fn from(e: ConnectError) -> Self {
		Self::Connect(e)
	}
}

impl From<io::Error> for WaylandError {
	fn from(e: io::Error) -> Self {
		Self::Io(e)
	}
}

impl From<GlobalError> for WaylandError {
	fn from(e: GlobalError) -> Self {
		Self::Global(e)
	}
}

#[derive(Debug, Clone, Copy)]
enum Message {
	Request(State),
	Notification(State)
}

#[derive(Clone)]
pub struct Display {
	tx: Arc<watch::Sender<Message>>,
	rx: watch::Receiver<Message>
}

impl Display {
	pub fn new() -> Self {
		let (tx, rx) = watch::channel(Message::Request(State::On));

		Self { tx: Arc::new(tx), rx }
	}

	pub fn set_state(&self, state: State) -> Option<()> {
		eprintln!("Display: set_state {:?}", state);
		self.tx.send(Message::Request(state)).ok()
	}

	fn message(&self) -> Message {
		*self.rx.borrow()
	}

	// pub fn state(&self) -> State {
	// 	match *self.rx.borrow() {
	// 		Message::Request(s) => s,
	// 		Message::Notification(s) => s
	// 	}
	// }
}

// impl Clone for Display {
// 	fn clone(&self) -> Self {
// 		Self {
// 			tx: self.tx.clone(),
// 			rx: self.tx.subscribe()
// 		}
// 	}
// }

pub fn start(mut display: Display) -> JoinHandle<()> {
	if context::is_headless() {
		// if we are in headless
		// no display is available so just make a mok display
		return tokio::spawn(async move {
			while !display.rx.changed().await.is_err() {}
		});
	}

	tokio::spawn(async move {
		// needs to be the same as Display::new()
		let mut state = State::On;

		for _ in 0..10 {

			if let Err(e) = handle_display(display.clone(), &mut state).await {
				eprintln!("handle display error {:?}", e);
			}

			sleep(Duration::from_secs(2)).await;

		}

		panic!("display failed to many times")
	})
}

async fn handle_display(
	mut rx: Display,
	state: &mut State
) -> Result<(), WaylandError> {

	let display = WlDisplay::connect_to_env()?;
	let display_fd = AsyncFd::new(display.get_connection_fd())?;
	let (thread_tx, thread_rx) = mpsc::channel(5);
	let thread = spawn_thread(display.clone(), rx.tx.clone(), thread_rx);

	loop {

		let ready_guard = tokio::select!{
			r = rx.rx.changed() => {
				r.expect("channel closed");
				None
			},
			r = display_fd.readable() => {
				match r {
					Ok(r) => Some(r),
					Err(e) => {
						eprintln!("Display: readable failed {:?}", e);
						return Err(e.into())
					}
				}
			}
		};

		let (finished_tx, finished_rx) = oneshot::channel();

		// None if state should not change
		let n_state = match rx.message() {
			Message::Request(s) if s != *state => {
				*state = s;
				Some(s)
			},
			// already set
			Message::Request(_) => None,
			Message::Notification(s) => {
				*state = s;
				// nothing to tell weston since this message is from him
				None
			}
		};

		let thread_send_result = if let Some(state) = n_state {
			let r = thread_tx.send((
				ThreadWork::ReadWrite(state),
				finished_tx
			)).await;
			r
		} else if ready_guard.is_none() {
			// since this event is triggered from rx.changed()
			// and we don't need to change the state
			// let's just skip calling the thread
			continue
		} else {
			// we got a notification from tokio to read events
			thread_tx.send((ThreadWork::Read, finished_tx)).await
		};

		if thread_send_result.is_err() {
			// could not send to thread
			// the thread has probably fail let's se why
			let r = thread.join();
			eprintln!("Display: thread error {:?}", r);
			return Err(WaylandError::ThreadError)
		}

		match finished_rx.await {
			Ok(WouldBlock::Yes) => {
				if let Some(mut ready_guard) = ready_guard {
					ready_guard.clear_ready();
				}
			},
			Ok(WouldBlock::No) => {},
			Err(_) => {
				// could not send to thread
			// the thread has probably fail let's see why
			let r = thread.join();
			eprintln!("Display: thread error {:?}", r);
			return Err(WaylandError::ThreadError)
			}
		}
	
	}

}

#[derive(Debug, Clone)]
enum ThreadWork {
	Read,
	ReadWrite(State)
}

#[derive(Debug, Clone)]
enum WouldBlock {
	Yes,
	No
}

/// Since the wayland client (EventQueue) is not send we can't use it directly
/// in a tokio::spawn block.
/// Thats why we have a separate thread that talks with weston but which doesn't
/// block only waiting on new work from `handle_display`
fn spawn_thread(
	display: WlDisplay,
	tx: Arc<watch::Sender<Message>>,
	mut rx: mpsc::Receiver<(ThreadWork, oneshot::Sender<WouldBlock>)>
) -> thread::JoinHandle<Result<(), WaylandError>> {
	thread::spawn(move || {
		let mut event_queue = display.create_event_queue();
		let attached_display = display.clone().attach(event_queue.token());
		let globals = GlobalManager::new(&attached_display);

		// wait that all globals are set
		event_queue.sync_roundtrip(&mut (), |_, _, _| unreachable!())?;

		// get kiosk api
		let kiosk = globals.instantiate_exact::<WestonKioskShell>(1)?;

		kiosk.quick_assign(move |_kiosk, ev, _| {
			match ev {
				Event::StateChange { state } => {
					let state = State::from_raw(state).unwrap_or(State::Off);
					tx.send(Message::Notification(state))
						.expect("display tokio task failed");
				}
			}
		});

		loop {

			let (work, finished) = rx.blocking_recv()
				.expect("display tokio task failed");

			match work {
				ThreadWork::ReadWrite(state) => {
					kiosk.set_state(state.to_raw());
				},
				ThreadWork::Read => {
					// maybe we could ommit the flush??
				}
			}

			match display.flush() {
				Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
					eprintln!("Display: flush would block");
				},
				Err(e) => return Err(e.into()),
				Ok(_) => {}
			}

			let mut would_block = WouldBlock::No;

			// if there is already data in the queue we can skip reading more
			if let Some(guard) = event_queue.prepare_read() {
				match guard.read_events() {
					Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
						would_block = WouldBlock::Yes;
					},
					Err(e) => return Err(e.into()),
					Ok(_) => {}
				}
			}

			// now dispatch the events that we have cached
			event_queue.dispatch_pending(&mut (), |_, _, _| {})
				.expect("internal wayland error");

			finished.send(would_block).expect("Display: tokio task failed");

		}
	})
}