
static mut CONTEXT: Context = Context::Release;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
	Debug,
	Release
}

/// this function is only allowed to be called
/// before anything multithreaded starts
pub unsafe fn set(ctx: Context) {
	CONTEXT = ctx;
}

pub fn get() -> Context {
	unsafe { CONTEXT }
}

impl Context {
	pub fn is_release(&self) -> bool {
		matches!(self, Self::Release)
	}

	pub fn is_debug(&self) -> bool {
		matches!(self, Self::Debug)
	}
}