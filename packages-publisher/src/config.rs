use crate::error::Result;
use crate::util::{read_toml, write_toml};

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use crypto::signature::{Keypair, PublicKey};

use packages::packages::Channel;
use packages::requests::AuthKey;

use serde::{Deserialize, Serialize};

const PATH: &str = ".config/publisher/config.toml";

fn path() -> Result<PathBuf> {
	let home = env::var("HOME")
		.map_err(|e| err!(e, "please define the environment variable $HOME"))?;

	Ok(Path::new(&home).join(PATH))
}

pub struct Config {
	path: PathBuf,
	inner: HashMap<String, Source>,
}

impl Config {
	/// tries to open an existing configuration if that failes
	/// creates a new one
	async fn new() -> Result<Self> {
		Ok(match Self::open().await {
			Ok(me) => me,
			Err(_) => Self {
				path: path()?,
				inner: HashMap::new(),
			},
		})
	}

	pub async fn open() -> Result<Self> {
		let path = path()?;
		Ok(Self {
			inner: read_toml(&path).await?,
			path,
		})
	}

	pub fn get(&self, server_name: &str) -> Result<&Source> {
		self.inner
			.get(server_name)
			.ok_or_else(|| err!("None", "no configuration for {server_name}"))
	}

	pub fn get_mut(&mut self, server_name: &str) -> Result<&mut Source> {
		self.inner
			.get_mut(server_name)
			.ok_or_else(|| err!("None", "no configuration for {server_name}"))
	}

	pub async fn write(&self) -> Result<()> {
		write_toml(&self.path, &self.inner).await
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Source {
	pub addr: String,
	pub channel: Channel,
	pub public_key: PublicKey,
	pub private_key: Option<Keypair>,
	/// reader auth key
	pub auth_key: Option<AuthKey>,
}

/// Configures an address and a public key to use for uploading for a
/// specific channel.
/// The configuration is stored under `~/.config/publisher/config.toml`.
#[derive(clap::Parser)]
pub struct ConfigOpts {
	server_name: String,
	channel: Channel,
	address: String,
	public_key: PublicKey,
}

pub async fn configure(opts: ConfigOpts) -> Result<()> {
	let mut cfg = Config::new().await?;

	cfg.inner.insert(
		opts.server_name,
		Source {
			addr: opts.address,
			channel: opts.channel,
			public_key: opts.public_key,
			private_key: None,
			auth_key: None,
		},
	);

	cfg.write().await?;

	println!("configuration written to `{}`", cfg.path.display());

	Ok(())
}
