
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Context {
	debug: bool,
	headless: bool
}

impl Context {
	const fn new() -> Self {
		Self {
			debug: false,
			headless: false
		}
	}
}

static mut CONTEXT: Context = Context::new();

/// this function is only allowed to be called once
/// before anything multithreaded starts
pub unsafe fn init() {

	let debug = cfg!(debug_assertions) || env::var("DEBUG").is_ok();
	let headless = env::var("HEADLESS").is_ok();

	// safe since we only store the context once
	// and before anybody has access to it
	CONTEXT = Context { debug, headless };

}

#[inline(always)]
fn get() -> Context {
	// safe since nobody can update the variable (except init)
	unsafe { CONTEXT }
}

pub fn is_debug() -> bool {
	get().debug
}

pub fn is_release() -> bool {
	!get().debug
}

pub fn is_headless() -> bool {
	get().headless
}