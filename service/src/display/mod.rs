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

pub use kiosk_api::State;
use kiosk_api::{Event, WestonKioskShell};

use std::sync::Arc;
use std::{io, thread};

use tokio::io::unix::AsyncFd;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

use wayland_client::{
	ConnectError, Display as WlDisplay, GlobalError, GlobalManager,
};

#[derive(Debug)]
enum WaylandError {
	Connect(ConnectError),
	Io(io::Error),
	Global(GlobalError),
	ThreadError,
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
	ChangeState(State),
	ChangeBrightness(u8),
}

#[derive(Clone)]
pub struct Display {
	tx: mpsc::Sender<Message>,
	// subscribe to state change
	rx: watch::Receiver<State>,
}

impl Display {
	pub async fn set_state(&self, state: State) -> Option<()> {
		eprintln!("Display: set_state {:?}", state);
		self.tx.send(Message::ChangeState(state)).await.ok()
	}

	pub async fn set_brightness(&self, brightness: u8) -> Option<()> {
		eprintln!("Display: set_brightness {:?}", brightness);
		self.tx
			.send(Message::ChangeBrightness(brightness))
			.await
			.ok()
	}

	#[allow(dead_code)]
	pub async fn state_change(&mut self) -> State {
		self.rx.changed().await.expect("display thread failed");
		*self.rx.borrow()
	}
}

const MPSC_CAP: usize = 10;

pub fn start() -> (JoinHandle<()>, Display) {
	let (w_tx, w_rx) = watch::channel(State::On);
	let (tx, rx) = mpsc::channel(MPSC_CAP);

	let display = Display {
		tx: tx.clone(),
		rx: w_rx,
	};

	if context::is_headless() {
		// if we are in headless
		// no display is available so just make a mok display
		let task = tokio::spawn(async move {
			let mut rx = rx;
			while !rx.recv().await.is_none() {}
		});

		return (task, display);
	}

	let task = tokio::spawn(async move {
		let w_tx = Arc::new(w_tx);
		let mut rx = rx;
		let tx = tx;

		for _ in 0..10 {
			// no message in the channel
			// create one to power on the screen
			if MPSC_CAP == tx.capacity() {
				let _ = tx.try_send(Message::ChangeState(State::On));
			}

			if let Err(e) = handle_display(w_tx.clone(), &mut rx).await {
				eprintln!("handle display error {:?}", e);
			}

			sleep(Duration::from_secs(2)).await;
		}

		panic!("display failed to many times")
	});

	(task, display)
}

async fn handle_display(
	tx: Arc<watch::Sender<State>>,
	rx: &mut mpsc::Receiver<Message>,
) -> Result<(), WaylandError> {
	let display = WlDisplay::connect_to_env()?;
	let display_fd = AsyncFd::new(display.get_connection_fd())?;
	let (thread_tx, thread_rx) = mpsc::channel(5);
	let thread = spawn_thread(display.clone(), tx, thread_rx);

	loop {
		let (ready_guard, msg) = tokio::select! {
			msg = rx.recv() => {
				let msg = msg.expect("channel closed");
				(None, Some(msg))
			},
			r = display_fd.readable() => {
				match r {
					Ok(r) => (Some(r), None),
					Err(e) => {
						eprintln!("Display: readable failed {:?}", e);
						return Err(e.into())
					}
				}
			}
		};

		let (finished_tx, finished_rx) = oneshot::channel();

		let thread_work = match (ready_guard.is_some(), msg) {
			// we got a notification from the wayland fd to read events
			(true, None) => ThreadWork::Read,
			(false, Some(Message::ChangeState(s))) => {
				ThreadWork::ChangeState(s)
			}
			(false, Some(Message::ChangeBrightness(b))) => {
				ThreadWork::ChangeBrightness(b)
			}
			(true, Some(_)) => unreachable!(),
			(false, None) => unreachable!(),
		};

		let thread_send_result =
			thread_tx.send((thread_work, finished_tx)).await;

		if thread_send_result.is_err() {
			// could not send to thread
			// the thread has probably fail let's see why
			let r = thread.join();
			eprintln!("Display: thread error {:?}", r);
			return Err(WaylandError::ThreadError);
		}

		match finished_rx.await {
			Ok(WouldBlock::Yes) => {
				if let Some(mut ready_guard) = ready_guard {
					ready_guard.clear_ready();
				}
			}
			Ok(WouldBlock::No) => {}
			Err(_) => {
				// could not receive from thread
				// the thread has probably fail let's see why
				let r = thread.join();
				eprintln!("Display: thread error {:?}", r);
				return Err(WaylandError::ThreadError);
			}
		}
	}
}

#[derive(Debug, Clone)]
enum ThreadWork {
	Read,
	ChangeState(State),
	ChangeBrightness(u8),
}

#[derive(Debug, Clone)]
enum WouldBlock {
	Yes,
	No,
}

/// Since the wayland client (EventQueue) is not send we can't use it directly
/// in a tokio::spawn block.
/// Thats why we have a separate thread that talks with weston but which doesn't
/// block only waiting on new work from `handle_display`
fn spawn_thread(
	display: WlDisplay,
	tx: Arc<watch::Sender<State>>,
	mut rx: mpsc::Receiver<(ThreadWork, oneshot::Sender<WouldBlock>)>,
) -> thread::JoinHandle<Result<(), WaylandError>> {
	thread::spawn(move || {
		let mut event_queue = display.create_event_queue();
		let attached_display = display.clone().attach(event_queue.token());
		let globals = GlobalManager::new(&attached_display);

		// wait that all globals are set
		event_queue.sync_roundtrip(&mut (), |_, _, _| unreachable!())?;

		// get kiosk api
		let kiosk = globals.instantiate_exact::<WestonKioskShell>(1)?;

		kiosk.quick_assign(move |_kiosk, ev, _| match ev {
			Event::StateChange { state } => {
				let state = State::from_raw(state).unwrap_or(State::Off);
				tx.send(state).expect("display tokio task failed");
			}
		});

		loop {
			let (work, finished) =
				rx.blocking_recv().expect("display tokio task failed");

			match work {
				ThreadWork::ChangeState(state) => {
					kiosk.set_state(state.to_raw());
				}
				ThreadWork::ChangeBrightness(brightness) => {
					kiosk.set_brightness(brightness as u32);
				}
				ThreadWork::Read => {
					// maybe we could ommit the flush??
				}
			}

			match display.flush() {
				Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
					eprintln!("Display: flush would block");
				}
				Err(e) => return Err(e.into()),
				Ok(_) => {}
			}

			let mut would_block = WouldBlock::No;

			// if there is already data in the queue we can skip reading more
			if let Some(guard) = event_queue.prepare_read() {
				match guard.read_events() {
					Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
						would_block = WouldBlock::Yes;
					}
					Err(e) => return Err(e.into()),
					Ok(_) => {}
				}
			}

			// now dispatch the events that we have cached
			event_queue
				.dispatch_pending(&mut (), |_, _, _| {})
				.expect("internal wayland error");

			finished
				.send(would_block)
				.expect("Display: tokio task failed");
		}
	})
}
