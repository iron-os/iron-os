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
use tokio::io::{Interest, Ready};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{io, thread};

use tokio::io::unix::{AsyncFd, AsyncFdReadyGuard};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration, Instant};

use wayland_client::{
	ConnectError, Display as WlDisplay, GlobalError, GlobalManager,
};

#[derive(Debug)]
#[allow(dead_code)]
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
	MouseClick { x: f32, y: f32 },
}

/// What the display should be, sent again after every reconnect.
#[derive(Default)]
struct Wanted {
	state: Option<State>,
	brightness: Option<u8>,
}

#[derive(Clone)]
pub struct Display {
	tx: mpsc::Sender<Message>,
	// subscribe to state change
	rx: watch::Receiver<State>,
	// never wait for the compositor: while it is away requests fail right
	// away, the wanted state and brightness are applied when it is back
	connected: Arc<AtomicBool>,
	wanted: Arc<Mutex<Wanted>>,
}

impl Display {
	pub async fn set_state(&self, state: State) -> Option<()> {
		eprintln!("Display: set_state {:?}", state);
		self.wanted.lock().unwrap().state = Some(state);
		self.send(Message::ChangeState(state))
	}

	pub async fn set_brightness(&self, brightness: u8) -> Option<()> {
		eprintln!("Display: set_brightness {:?}", brightness);
		self.wanted.lock().unwrap().brightness = Some(brightness);
		self.send(Message::ChangeBrightness(brightness))
	}

	pub async fn mouse_click(&self, x: f32, y: f32) -> Option<()> {
		eprintln!("Display: mouse_click x:{} y:{}", x, y);
		self.send(Message::MouseClick { x, y })
	}

	fn send(&self, msg: Message) -> Option<()> {
		if !self.connected.load(Ordering::Relaxed) {
			return None;
		}
		self.tx.try_send(msg).ok()
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

	let connected = Arc::new(AtomicBool::new(false));
	let wanted = Arc::new(Mutex::new(Wanted::default()));
	let display = Display {
		tx: tx.clone(),
		rx: w_rx,
		connected: connected.clone(),
		wanted: wanted.clone(),
	};

	if context::is_headless() {
		connected.store(true, Ordering::Relaxed);
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

		// never give up: if this task ended the whole service would restart
		// and with it a running package update, so a broken compositor
		// could not be fixed by an update
		let mut wait = Duration::from_secs(2);
		loop {
			// what is left in the channel is from the last connection, old
			// clicks would hit whatever is shown now
			while rx.try_recv().is_ok() {}
			// the new connection gets the wanted state, without one the
			// screen is powered on
			let (state, brightness) = {
				let wanted = wanted.lock().unwrap();
				(wanted.state.unwrap_or(State::On), wanted.brightness)
			};
			let _ = tx.try_send(Message::ChangeState(state));
			if let Some(b) = brightness {
				let _ = tx.try_send(Message::ChangeBrightness(b));
			}

			// a connection which worked for a while starts the backoff again
			let started = Instant::now();
			let res = handle_display(w_tx.clone(), &mut rx, &connected).await;
			connected.store(false, Ordering::Relaxed);
			if started.elapsed() > Duration::from_secs(60) {
				wait = Duration::from_secs(2);
			}
			if let Err(e) = res {
				eprintln!("handle display error {:?}, retry in {:?}", e, wait);
			}

			sleep(wait).await;
			wait = (wait * 2).min(Duration::from_secs(10));
		}
	});

	(task, display)
}

// we are always interested in reading
const DEFAULT_INTEREST: Interest = Interest::READABLE;

async fn handle_display(
	tx: Arc<watch::Sender<State>>,
	rx: &mut mpsc::Receiver<Message>,
	connected: &AtomicBool,
) -> Result<(), WaylandError> {
	let display = WlDisplay::connect_to_env()?;
	let display_fd = AsyncFd::new(display.get_connection_fd())?;
	let (thread_tx, thread_rx) = mpsc::channel(5);
	let thread = spawn_thread(display, tx, thread_rx);
	connected.store(true, Ordering::Relaxed);

	enum SelectState<'a> {
		Message(Message),
		Ready(AsyncFdReadyGuard<'a, i32>),
	}

	let mut interest = DEFAULT_INTEREST;

	loop {
		let state = tokio::select! {
			msg = rx.recv() => {
				let msg = msg.expect("channel closed");
				SelectState::Message(msg)
			},
			r = display_fd.ready(interest) => {
				match r {
					Ok(r) => SelectState::Ready(r),
					Err(e) => {
						eprintln!("Display: readable failed {:?}", e);
						return Err(e.into())
					}
				}
			}
		};

		let (finished_tx, finished_rx) = oneshot::channel();

		let thread_work = match &state {
			// we got a notification from the wayland fd to read events
			SelectState::Ready(_) => ThreadWork::Ready,
			SelectState::Message(Message::ChangeState(s)) => {
				ThreadWork::ChangeState(*s)
			}
			SelectState::Message(Message::ChangeBrightness(b)) => {
				ThreadWork::ChangeBrightness(*b)
			}
			SelectState::Message(Message::MouseClick { x, y }) => {
				ThreadWork::MouseClick { x: *x, y: *y }
			}
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
			Ok((n_interest, clear_ready)) => {
				interest = n_interest;

				if let SelectState::Ready(mut ready_guard) = state {
					if !clear_ready.is_empty() {
						ready_guard.clear_ready_matching(clear_ready);
					}
				}
			}
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

#[derive(Debug)]
enum ThreadWork {
	Ready,
	ChangeState(State),
	ChangeBrightness(u8),
	MouseClick { x: f32, y: f32 },
}

/// Since the wayland client (EventQueue) is not send we can't use it directly
/// in a tokio::spawn block.
/// Thats why we have a separate thread that talks with weston but which doesn't
/// block only waiting on new work from `handle_display`
fn spawn_thread(
	display: WlDisplay,
	tx: Arc<watch::Sender<State>>,
	mut rx: mpsc::Receiver<(ThreadWork, oneshot::Sender<(Interest, Ready)>)>,
) -> thread::JoinHandle<Result<(), WaylandError>> {
	thread::spawn(move || {
		let mut event_queue = display.create_event_queue();
		let attached_display = display.clone().attach(event_queue.token());
		let globals = GlobalManager::new(&attached_display);

		// wait that all globals are set
		event_queue.sync_roundtrip(&mut (), |_, _, _| unreachable!())?;

		// get kiosk api
		let kiosk = globals.instantiate_exact::<WestonKioskShell>(1)?;

		// listen for events from kiosk
		kiosk.quick_assign(move |_kiosk, ev, _| match ev {
			Event::StateChange { state } => {
				let state = State::from_raw(state).unwrap_or(State::Off);
				// fails only if nobody watches anymore
				let _ = tx.send(state);
			}
		});

		// ends when handle_display is done with this connection
		while let Some((work, finished)) = rx.blocking_recv() {
			match work {
				ThreadWork::ChangeState(state) => {
					kiosk.set_state(state.to_raw());
				}
				ThreadWork::ChangeBrightness(brightness) => {
					kiosk.set_brightness(brightness as u32);
				}
				ThreadWork::MouseClick { x, y } => {
					kiosk.mouse_click(x as f64, y as f64);
				}
				ThreadWork::Ready => {
					// maybe we could ommit the flush??
				}
			}

			let mut interest = DEFAULT_INTEREST;
			let mut clear_ready = Ready::EMPTY;

			match display.flush() {
				Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
					interest |= Interest::WRITABLE;
					clear_ready |= Ready::WRITABLE;
				}
				Err(e) => return Err(e.into()),
				Ok(_) => {}
			}

			// if there is already data in the queue we can skip reading more
			if let Some(guard) = event_queue.prepare_read() {
				match guard.read_events() {
					Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
						// we are always interested in reading
						clear_ready |= Ready::READABLE;
					}
					Err(e) => return Err(e.into()),
					Ok(_) => {}
				}
			}

			// now dispatch the events that we have cached
			event_queue
				.dispatch_pending(&mut (), |_, _, _| {})
				.map_err(|_| WaylandError::ThreadError)?;

			// handle_display may have given up on this connection already
			let _ = finished.send((interest, clear_ready));
		}

		Ok(())
	})
}
