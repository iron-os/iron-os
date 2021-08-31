
use crate::io_other;

use std::process::{Child, Command as StdCommand, Stdio};
use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::os::unix::process::CommandExt;


pub struct Command(StdCommand);

impl Command {

	pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
		Self(StdCommand::new(program))
	}

	pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
		self.0.arg(arg);
		self
	}

	pub fn args<I, S>(&mut self, args: I) -> &mut Self
	where
		I: IntoIterator<Item = S>,
		S: AsRef<OsStr>
	{
		self.0.args(args);
		self
	}

	/// executes this command as a non root user
	pub fn as_user(&mut self) -> &mut Self {
		self.0.uid(14);
		self.0.gid(15);
		self
	}

	pub fn current_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
		self.0.current_dir(path);
		self
	}

	pub fn exec(&mut self) -> io::Result<()> {
		self.0.status()
			.and_then(|s| {
				s.success()
					.then(|| ())
					.ok_or_else(|| io_other("command exited with non success status"))
			})
	}

	pub fn stdin_piped(&mut self) -> &mut Self {
		self.0.stdin(Stdio::piped());
		self
	}

	pub fn stdout_piped(&mut self) -> &mut Self {
		self.0.stdout(Stdio::piped());
		self
	}

	pub fn stderr_piped(&mut self) -> &mut Self {
		self.0.stderr(Stdio::piped());
		self
	}

	pub fn spawn(&mut self) -> io::Result<Child> {
		self.0.spawn()
	}

}