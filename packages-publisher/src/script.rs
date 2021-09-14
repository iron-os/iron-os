
use crate::error::{Result, Error};

use tokio::task::spawn_blocking;

use packages::packages::Channel;

pub struct Script {
	inner: riji::Script
}

impl Script {

	pub fn new(p: &str) -> Result<Self> {
		let inner = riji::Script::new(p)
			.map_err(|e| err!(format!("{:?}", e), "could not open script {:?}", p))?;

		Ok(Self { inner })
	}

	// pub fn execute(&mut self, cmd: &str, args: Vec<String>) -> Result<()> 

	/// calls the build function in the script
	pub fn build(&mut self, channel: &Channel) -> Result<()> {
		self.inner.execute("build", vec![channel.to_string()])
			.map_err(|e| err!(format!("{:?}", e), "failed to build"))
	}

	pub fn pack(&mut self, dest_path: &str, channel: &Channel) -> Result<()> {
		self.inner.execute("pack", vec![dest_path.into(), channel.to_string()])
			.map_err(|e| err!(format!("{:?}", e), "failed to pack"))
	}

}