
use crate::{Buffer, Line};

use tokio::io::{self, AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};

pub struct AsyncHandle<H> {
	inner: H,
	buffer: Buffer
}

impl<H> AsyncHandle<H> {

	pub fn new(inner: H) -> Self {
		Self {
			inner, buffer: Buffer::new()
		}
	}

	/// this function is not abortsafe
	pub async fn read(&mut self) -> io::Result<Line>
	where H: AsyncBufRead + Unpin {
		loop {
			self.inner.read_line(self.buffer.as_mut()).await?;
			let line = self.buffer.parse_line();
			if let Some(line) = line {
				return Ok(line)
			}
		}
	}

	/// this function is not abortsafe
	pub async fn write(&mut self, line: &Line) -> io::Result<()>
	where H: AsyncWrite + Unpin {
		self.inner.write_all(line.as_str().as_bytes()).await
	}

}


pub struct AsyncStdio {
	read: AsyncStdRead,
	write: AsyncStdWrite,
	buffer: Buffer
}

impl AsyncStdio {

	pub fn from_env() -> Self {
		Self {
			read: AsyncStdRead::This(BufReader::new(io::stdin())),
			write: AsyncStdWrite::This(io::stdout()),
			buffer: Buffer::new()
		}
	}

	pub fn from_child(child: &mut Child) -> Option<Self> {
		// to not steal one from child if we can't get both
		if child.stdin.is_none() || child.stdout.is_none() {
			return None
		}

		Some(Self {
			read: AsyncStdRead::Child(BufReader::new(child.stdout.take()?)),
			write: AsyncStdWrite::Child(child.stdin.take()?),
			buffer: Buffer::new()
		})
	}

	/// this function is not abortsafe
	pub async fn read(&mut self) -> io::Result<Line> {
		loop {
			self.read.read(self.buffer.as_mut()).await?;
			let line = self.buffer.parse_line();
			if let Some(line) = line {
				return Ok(line)
			}
		}
	}

	/// this function is not abortsafe
	pub async fn write(&mut self, line: &Line) -> io::Result<()> {
		self.write.write(line.as_str()).await
	}

}

enum AsyncStdRead {
	Child(BufReader<ChildStdout>),
	This(BufReader<io::Stdin>)
}

impl AsyncStdRead {

	async fn read(&mut self, buf: &mut String) -> io::Result<()> {
		match self {
			Self::Child(c) => c.read_line(buf).await,
			Self::This(io) => io.read_line(buf).await
		}.map(|_| ())
	}

}

enum AsyncStdWrite {
	Child(ChildStdin),
	This(io::Stdout)
}

impl AsyncStdWrite {

	async fn write(&mut self, s: &str) -> io::Result<()> {
		match self {
			Self::Child(c) => c.write_all(s.as_bytes()).await,
			Self::This(io) => io.write_all(s.as_bytes()).await
		}.map(|_| ())
	}

}