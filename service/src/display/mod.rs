
mod kiosk_api;

use kiosk_api::{WestonKioskShell, Event};
pub use kiosk_api::State;

use std::{io, thread};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::runtime::Handle;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

use wayland_client::{
	Display as WlDisplay, GlobalManager, ConnectError, GlobalError
};

#[derive(Debug)]
enum WaylandError {
	Connect(ConnectError),
	Io(io::Error),
	Global(GlobalError)
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

pub struct Display {
	tx: Arc<watch::Sender<Message>>,
	rx: watch::Receiver<Message>
}

impl Display {
	pub fn new() -> Self {
		let (tx, rx) = watch::channel(Message::new(State::On));
		Self {
			tx: Arc::new(tx),
			rx
		}
	}

	pub fn set_state(&self, state: State) -> Option<()> {
		eprintln!("Display set_state: {:?}", state);
		self.tx.send(Message::new(state)).ok()
	}
}

impl Clone for Display {
	fn clone(&self) -> Self {
		Self {
			tx: self.tx.clone(),
			rx: self.tx.subscribe()
		}
	}
}

// we need wayland-client

// how do we call set_state

#[derive(Debug, Clone, Copy)]
enum Flag {
	Nothing,
	Close
}

#[derive(Debug, Clone)]
struct Message {
	state: State,
	// is true if the state should not be sent to the display
	prevent: bool,
	// closes the sender
	flag: Flag
}

impl Message {
	pub fn new(state: State) -> Self {
		Self {
			state,
			prevent: false,
			flag: Flag::Nothing
		}
	}

	fn from_display(state: State) -> Self {
		Self {
			state,
			prevent: true,
			flag: Flag::Nothing
		}
	}

	fn close() -> Self {
		Self {
			state: State::On,
			prevent: true,
			flag: Flag::Close
		}
	}
}

#[derive(Debug)]
struct HasExited {
	inner: Arc<AtomicBool>,
	modify: bool
}

impl HasExited {
	pub fn new() -> Self {
		Self {
			inner: Arc::new(AtomicBool::new(false)),
			modify: false
		}
	}

	pub fn set_modify(&mut self, v: bool) {
		self.modify = v;
	}

	pub fn has_exited(&self) -> bool {
		self.inner.load(Ordering::Relaxed)
	}
}

impl Clone for HasExited {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
			modify: false
		}
	}
}

impl Drop for HasExited {
	fn drop(&mut self) {
		if self.modify {
			self.inner.store(true, Ordering::Relaxed);
		}
	}
}

pub async fn start(
	display: Display
) -> JoinHandle<()> {
	tokio::spawn(async move {
		let runtime = Handle::current();

		let Display { tx, rx } = display;

		let mut has_exited = HasExited::new();
		// we need to modify the flag when has_exited goes out of scope
		// so the thread knows it should cleanup and exit
		has_exited.set_modify(true);

		let mut display_task = has_exited.clone();
		let handle = thread::spawn(move || {
			// we need to modify the flag when has_exited goes out of scope
			// so the thread knows it should cleanup and exit
			display_task.set_modify(true);

			loop {
				let display_task_c = display_task.clone();

				let tx = tx.clone();
				let rx = rx.clone();
				let runtime = runtime.clone();
				let re = manage_display(runtime, tx, rx, display_task_c);
				println!("mange_display {:?}", re);

				if display_task.has_exited() {
					break
				}

				thread::sleep(Duration::from_secs(2));
			}
		});

		loop {

			sleep(Duration::from_secs(5)).await;

			if has_exited.has_exited() {
				eprintln!("display thread exited");
				handle.join().expect("display thread failed");
				return
			}

		}


	})
}

/// does not return correctly if there is an error
fn manage_display(
	runtime: Handle,
	tx: Arc<watch::Sender<Message>>,
	rx: watch::Receiver<Message>,
	display_task: HasExited
) -> Result<(), WaylandError> {

	let display = WlDisplay::connect_to_env()?;
	let recv_display = display.clone();
	let recv_runtime = runtime.clone();
	let recv_tx = tx.clone();

	let display_task_c = display_task.clone();

	let sender_thread = thread::spawn(move || {
		sender(display, runtime, rx, display_task_c)
	});

	let recv_thread = thread::spawn(move || {
		receiver(recv_display, recv_runtime, recv_tx, display_task)
	});
	let recv_thread = recv_thread.join();
	eprintln!("recv_thread closed {:?}", recv_thread);
	tx.send(Message::close()).expect("channel closed");
	let sender_thread = sender_thread.join();
	eprintln!("sender_thread closed {:?}", sender_thread);

	Ok(())
}


fn sender(
	display: WlDisplay,
	runtime: Handle,
	mut rx: watch::Receiver<Message>,
	display_task: HasExited
) -> Result<(), WaylandError> {
	// event queue for state change
	let mut event_queue = display.create_event_queue();

	let attached_display = display.clone().attach(event_queue.token());

	let globals = GlobalManager::new(&attached_display);

	// wait that all globals are set
	event_queue.sync_roundtrip(&mut (), |_, _, _| unreachable!())?;

	// get kiosk api
	let kiosk = globals.instantiate_exact::<WestonKioskShell>(1)?;

	kiosk.set_state(State::On.to_raw());

	loop {
		eprintln!("Display: waiting on message");
		runtime.block_on(rx.changed()).expect("channel closed");
		let msg = rx.borrow().clone();
		eprintln!("Display received Message: {:?}", msg);
		match (msg.flag, msg.prevent) {
			(Flag::Nothing, true) => {},
			(Flag::Nothing, false) => {
				kiosk.set_state(msg.state.to_raw());
				// make sure the request get's sent
				event_queue.sync_roundtrip(&mut (), |_, _, _| {})?;
			},
			(Flag::Close, _) => return Ok(())
		}

		if display_task.has_exited() {
			return Ok(())
		}
	}
}


fn receiver(
	display: WlDisplay,
	_runtime: Handle,
	tx: Arc<watch::Sender<Message>>,
	display_task: HasExited
) -> Result<(), WaylandError> {

	// event queue for state change
	let mut event_queue = display.create_event_queue();

	let attached_display = display.clone().attach(event_queue.token());

	let globals = GlobalManager::new(&attached_display);

	// wait that all globals are set
	event_queue.sync_roundtrip(&mut (), |_, _, _| unreachable!())?;

	// get kiosk api
	let kiosk = globals.instantiate_exact::<WestonKioskShell>(1)?;

	kiosk.quick_assign(move |_kiosk, ev, _| {
		println!("received event {:?}", ev);
		match ev {
			Event::StateChange { state } => {
				let state = State::from_raw(state).unwrap_or(State::Off);
				tx.send(Message::from_display(state)).expect("channel closed");
			}
		}
	});

	// wait that the configuration has settletd
	event_queue.sync_roundtrip(&mut (), |_, _, _| {})?;


	loop {
		event_queue.dispatch(&mut (), |_, _, _| {})?;
		eprintln!("display dispatch");

		if display_task.has_exited() {
			return Ok(())
		}
	}
}