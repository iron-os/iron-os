use std::{cell::RefCell, rc::Rc};

use tokio::sync::oneshot;
use wayland_client::{
	protocol::wl_output::WlOutput, GlobalError, GlobalManager, Interface, Main,
};

use crate::display::capture_api::{
	self, Event, WestonCaptureSourceV1, WestonCaptureV1,
};

pub struct Capturer {
	main: Main<WestonCaptureV1>,
	globals: GlobalManager,
	source: Option<CaptureOutput>,
}
impl Capturer {
	pub fn new(globals: &GlobalManager) -> Result<Self, GlobalError> {
		let main = globals
			.instantiate_exact::<WestonCaptureV1>(WestonCaptureV1::VERSION)?;

		Ok(Self {
			main,
			globals: globals.clone(),
			source: None,
		})
	}

	pub fn tick(&mut self) {
		// todo replace with take_if once msrv goes to 1.80
		let take = if let Some(source) = &mut self.source {
			// handle events
			source.is_completed()
		} else {
			false
		};

		if take {
			self.source = None;
		}
	}

	pub fn capture_output(
		&mut self,
		tx: oneshot::Sender<()>,
	) -> Result<(), GlobalError> {
		// need to send busy
		if self.source.is_some() {
			return Ok(());
		}

		// which wl output
		let wl_output = self
			.globals
			.instantiate_exact::<WlOutput>(WlOutput::VERSION)?;
		let csource = self
			.main
			.create(&wl_output, capture_api::Source::Framebuffer);

		// need a buffer
		// let buffer = todo!();
		// csource.capture(&buffer);
		// csource.destroy();
		//
		Ok(())
	}
}

struct CaptureOutput {
	tx: oneshot::Sender<()>,
	csource: Main<WestonCaptureSourceV1>,
	state: Rc<RefCell<CaptureState>>,
}

impl CaptureOutput {
	pub fn new(
		csource: Main<WestonCaptureSourceV1>,
		tx: oneshot::Sender<()>,
	) -> Self {
		let state = Rc::new(RefCell::new(CaptureState::new()));

		let state2 = state.clone();

		csource.quick_assign(move |_csource, ev, _| {
			state2.borrow_mut().handle_event(ev);
		});

		Self { csource, tx }
	}

	pub fn is_completed(&mut self) -> bool {
		false
	}
}

struct CaptureState {
	offset_x: i32,
	offset_y: i32,
	width: i32,
	height: i32,
	pixe_format_info: (),
}

impl CaptureState {
	fn new() -> Self {
		Self {}
	}

	fn handle_event(&mut self, ev: Event) {
		match ev {
			Event::Format { drm_format } => {}
			_ => {}
		}
	}
}
