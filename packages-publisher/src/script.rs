use crate::error::Result;

use packages::packages::{Channel, TargetArch};

pub struct Script {
	inner: riji::Script,
}

impl Script {
	pub fn new(p: &str) -> Result<Self> {
		let inner = riji::Script::new(p).map_err(|e| {
			err!(format!("{:?}", e), "could not open script {:?}", p)
		})?;

		Ok(Self { inner })
	}

	// pub fn execute(&mut self, cmd: &str, args: Vec<String>) -> Result<()>

	/// calls the build function in the script
	pub fn build(
		&mut self,
		arch: &TargetArch,
		channel: &Channel,
		host_channel: Option<&Channel>,
	) -> Result<()> {
		let mut args = vec![arch.to_string(), channel.to_string()];
		if let Some(host_channel) = host_channel {
			args.push(host_channel.to_string());
		}

		self.inner
			.execute("build", args)
			.map_err(|e| err!(format!("{:?}", e), "failed to build"))
	}

	pub fn pack(
		&mut self,
		dest_path: &str,
		arch: &TargetArch,
		channel: &Channel,
	) -> Result<()> {
		self.inner
			.execute(
				"pack",
				vec![dest_path.into(), arch.to_string(), channel.to_string()],
			)
			.map_err(|e| err!(format!("{:?}", e), "failed to pack"))
	}
}
