use crate::{Buffer, Line};

use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout};

pub struct Handle<H> {
	inner: H,
	buffer: Buffer,
}

impl<H> Handle<H> {
	pub fn new(inner: H) -> Self {
		Self {
			inner,
			buffer: Buffer::new(),
		}
	}

	/// if none is returned the underlying
	/// buffer has closed
	pub fn read(&mut self) -> io::Result<Option<Line>>
	where
		H: BufRead,
	{
		loop {
			let r = self.inner.read_line(self.buffer.as_mut())?;
			if r == 0 {
				return Ok(None);
			}

			let line = self.buffer.parse_line();
			if let Some(line) = line {
				return Ok(Some(line));
			}
		}
	}

	pub fn write(&mut self, line: &Line) -> io::Result<()>
	where
		H: Write,
	{
		self.inner.write_all(line.as_str().as_bytes())
	}
}

pub struct Stdio {
	read: StdRead,
	write: StdWrite,
	buffer: Buffer,
}

impl Stdio {
	pub fn from_env() -> Self {
		Self {
			read: StdRead::This(io::stdin()),
			write: StdWrite::This(io::stdout()),
			buffer: Buffer::new(),
		}
	}

	pub fn from_child(child: &mut Child) -> Option<Self> {
		// to not steal one from child if we can't get both
		if child.stdin.is_none() || child.stdout.is_none() {
			return None;
		}

		Some(Self {
			read: StdRead::Child(BufReader::new(child.stdout.take()?)),
			write: StdWrite::Child(child.stdin.take()?),
			buffer: Buffer::new(),
		})
	}

	pub fn read(&mut self) -> io::Result<Option<Line>> {
		loop {
			let r = self.read.read(self.buffer.as_mut())?;
			if r == 0 {
				return Ok(None);
			}

			let line = self.buffer.parse_line();
			if let Some(line) = line {
				return Ok(Some(line));
			}
		}
	}

	pub fn write(&mut self, line: &Line) -> io::Result<()> {
		self.write.write(line.as_str())
	}
}

enum StdRead {
	Child(BufReader<ChildStdout>),
	This(io::Stdin),
}

impl StdRead {
	fn read(&mut self, buf: &mut String) -> io::Result<usize> {
		match self {
			Self::Child(c) => c.read_line(buf),
			Self::This(io) => io.lock().read_line(buf),
		}
	}
}

enum StdWrite {
	Child(ChildStdin),
	This(io::Stdout),
}

impl StdWrite {
	fn write(&mut self, s: &str) -> io::Result<()> {
		match self {
			Self::Child(c) => c.write_all(s.as_bytes()),
			Self::This(io) => io.lock().write_all(s.as_bytes()),
		}
		.map(|_| ())
	}
}
