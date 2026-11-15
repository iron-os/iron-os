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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::{io, thread};

use tokio::io::unix::{AsyncFd, AsyncFdReadyGuard};
use tokio::sync::{mpsc, oneshot, watch, Notify};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout, timeout_at, Duration, Instant};

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

/// Whether the compositor is connected and what the display should be,
/// the state and brightness are sent again after every reconnect.
///
/// Requests are only sent while holding the lock, so none can get lost or
/// be overtaken by the wanted values of a reconnect.
#[derive(Default)]
struct Wanted {
	connected: bool,
	/// counts the connections, a request which waited knows if a connection
	/// sent its value with send_all
	connections: u64,
	state: Option<State>,
	brightness: Option<u8>,
}

impl Wanted {
	/// Queues the wanted state and brightness, without a state the screen
	/// is powered on.
	fn send_all(&self, tx: &mpsc::Sender<Message>) {
		let state = self.state.unwrap_or(State::On);
		let _ = tx.try_send(Message::ChangeState(state));
		if let Some(b) = self.brightness {
			let _ = tx.try_send(Message::ChangeBrightness(b));
		}
	}
}

#[derive(Clone)]
pub struct Display {
	tx: mpsc::Sender<Message>,
	// subscribe to state change
	rx: watch::Receiver<State>,
	// state and brightness wait CONNECT_WAIT for the compositor (it starts
	// at the same time as the service), then fail; the wanted values are
	// applied when it is back
	wanted: Arc<Mutex<Wanted>>,
	// only changed while holding the wanted lock
	connected: watch::Receiver<bool>,
	// while requests wait the display task retries every second
	waiting: Arc<Waiting>,
}

#[derive(Default)]
struct Waiting {
	count: AtomicUsize,
	notify: Notify,
}

/// Counts a waiting request until it is dropped, also when the request is
/// cancelled.
struct WaitingGuard<'a>(&'a Waiting);

impl<'a> WaitingGuard<'a> {
	fn new(waiting: &'a Waiting) -> Self {
		waiting.count.fetch_add(1, Ordering::SeqCst);
		waiting.notify.notify_one();
		Self(waiting)
	}
}

impl Drop for WaitingGuard<'_> {
	fn drop(&mut self) {
		self.0.count.fetch_sub(1, Ordering::SeqCst);
	}
}

/// How long state and brightness requests wait for the compositor, long
/// enough for it to start at boot or after a crash.
const CONNECT_WAIT: Duration = Duration::from_secs(10);

impl Display {
	pub async fn set_state(&self, state: State) -> Option<()> {
		eprintln!("Display: set_state {:?}", state);
		self.set(|w| w.state = Some(state), Message::ChangeState(state))
			.await
	}

	pub async fn set_brightness(&self, brightness: u8) -> Option<()> {
		eprintln!("Display: set_brightness {:?}", brightness);
		let msg = Message::ChangeBrightness(brightness);
		self.set(|w| w.brightness = Some(brightness), msg).await
	}

	/// Clicks don't wait for the compositor, a late click would hit whatever
	/// is shown then.
	pub async fn mouse_click(&self, x: f32, y: f32) -> Option<()> {
		eprintln!("Display: mouse_click x:{} y:{}", x, y);
		let wanted = self.wanted.lock().unwrap();
		self.send(&wanted, Message::MouseClick { x, y })
	}

	/// Stores the wanted value and sends it, or waits up to CONNECT_WAIT for
	/// a new connection, which sends the wanted values when it starts.
	async fn set(
		&self,
		update: impl FnOnce(&mut Wanted),
		msg: Message,
	) -> Option<()> {
		let connections = {
			let mut wanted = self.wanted.lock().unwrap();
			update(&mut wanted);
			if wanted.connected {
				return self.send(&wanted, msg);
			}
			wanted.connections
		};

		let _guard = WaitingGuard::new(&self.waiting);
		let mut connected = self.connected.clone();
		let _ = timeout(CONNECT_WAIT, connected.wait_for(|c| *c)).await;
		let wanted = self.wanted.lock().unwrap();
		(wanted.connected && wanted.connections != connections).then_some(())
	}

	fn send(&self, wanted: &Wanted, msg: Message) -> Option<()> {
		if !wanted.connected {
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

	let wanted = Arc::new(Mutex::new(Wanted::default()));
	let (connected_tx, connected_rx) = watch::channel(false);
	let waiting = Arc::new(Waiting::default());
	let display = Display {
		tx: tx.clone(),
		rx: w_rx,
		wanted: wanted.clone(),
		connected: connected_rx,
		waiting: waiting.clone(),
	};

	if context::is_headless() {
		wanted.lock().unwrap().connected = true;
		let _ = connected_tx.send(true);
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

			// a connection which worked for a while starts the backoff again
			let started = Instant::now();
			let res = handle_display(
				w_tx.clone(),
				&tx,
				&mut rx,
				&wanted,
				&connected_tx,
			)
			.await;
			{
				let mut wanted = wanted.lock().unwrap();
				wanted.connected = false;
				let _ = connected_tx.send(false);
			}
			if started.elapsed() > Duration::from_secs(60) {
				wait = Duration::from_secs(2);
			}
			if let Err(e) = res {
				eprintln!("handle display error {:?}, retry in {:?}", e, wait);
			}

			// retry every second while a request waits, else after the
			// backoff
			let backoff = Instant::now() + wait;
			sleep(Duration::from_secs(1)).await;
			while waiting.count.load(Ordering::SeqCst) == 0 {
				if timeout_at(backoff, waiting.notify.notified())
					.await
					.is_err()
				{
					break;
				}
			}
			wait = (wait * 2).min(Duration::from_secs(10));
		}
	});

	(task, display)
}

// we are always interested in reading
const DEFAULT_INTEREST: Interest = Interest::READABLE;

async fn handle_display(
	tx: Arc<watch::Sender<State>>,
	msg_tx: &mpsc::Sender<Message>,
	rx: &mut mpsc::Receiver<Message>,
	wanted: &Arc<Mutex<Wanted>>,
	connected_tx: &watch::Sender<bool>,
) -> Result<(), WaylandError> {
	let display = WlDisplay::connect_to_env()?;
	let display_fd = AsyncFd::new(display.get_connection_fd())?;
	let (thread_tx, thread_rx) = mpsc::channel(5);
	let thread = spawn_thread(display, tx, thread_rx, wanted.clone());
	// the new connection gets the wanted state first
	{
		let mut wanted = wanted.lock().unwrap();
		wanted.connected = true;
		wanted.connections += 1;
		wanted.send_all(msg_tx);
		let _ = connected_tx.send(true);
	}

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
	wanted: Arc<Mutex<Wanted>>,
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
				// the compositor changes the state too (input, idle), after
				// a reconnect the last state is restored
				wanted.lock().unwrap().state = Some(state);
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
