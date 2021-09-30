
// Re-export only the actual code, and then only use this re-export
// The `generated` module below is just some boilerplate to properly isolate stuff
// and avoid exposing internal details.
//
// You can use all the types from my_protocol as if they went from `wayland_client::protocol`.
pub use generated::weston_kiosk_shell::{WestonKioskShell, State, Event};

mod generated {
	// The generated code tends to trigger a lot of warnings
	// so we isolate it into a very permissive module
	#![allow(dead_code,non_camel_case_types,unused_unsafe,unused_variables)]
	#![allow(non_upper_case_globals,non_snake_case,unused_imports)]

	// These imports are used by the generated code
	use wayland_client::{Main, Attached, Proxy, ProxyMap, AnonymousObject};
	use wayland_commons::map::{Object, ObjectMetadata};
	use wayland_commons::{Interface, MessageGroup};
	use wayland_commons::wire::{Argument, MessageDesc, ArgumentType, Message};
	use wayland_commons::smallvec;
	use wayland_client::protocol::*;
	use wayland_client::sys;
	include!(concat!(env!("OUT_DIR"), "/weston_kiosk_shell.rs"));
}