use crate::error::Result;
use crate::util::{read_toml, write_toml};

use std::env;
use std::path::PathBuf;

use crypto::signature::{Keypair, PublicKey};

use packages::packages::Channel;
use packages::requests::AuthKey;

use serde::{Deserialize, Serialize};

const PATH: &str = ".config/publisher/config.toml";

fn path() -> Result<PathBuf> {
	let home = env::var("HOME")
		.map_err(|e| err!(e, "please define the environment variable $HOME"))?;
	let mut path = PathBuf::from(home);
	path.push(PATH);
	Ok(path)
}

pub struct Config {
	inner: ConfigToml,
}

impl Config {
	/// tries to open an existing configuration if that failes
	/// creates a new one
	async fn new() -> Self {
		match Self::open().await {
			Ok(me) => me,
			Err(_) => Self {
				inner: ConfigToml {
					debug: None,
					alpha: None,
					beta: None,
					release: None,
				},
			},
		}
	}

	pub async fn open() -> Result<Self> {
		Ok(Self {
			inner: read_toml(&path()?).await?,
		})
	}

	pub fn get(&self, channel: &Channel) -> Result<&Source> {
		let opt = match channel {
			Channel::Debug => self.inner.debug.as_ref(),
			Channel::Alpha => self.inner.alpha.as_ref(),
			Channel::Beta => self.inner.beta.as_ref(),
			Channel::Release => self.inner.release.as_ref(),
		};

		match opt {
			Some(src) => Ok(src),
			None => Err(err!("None", "no configuration for {:?}", channel)),
		}
	}

	pub fn get_mut(&mut self, channel: &Channel) -> Result<&mut Source> {
		let opt = match channel {
			Channel::Debug => self.inner.debug.as_mut(),
			Channel::Alpha => self.inner.alpha.as_mut(),
			Channel::Beta => self.inner.beta.as_mut(),
			Channel::Release => self.inner.release.as_mut(),
		};

		match opt {
			Some(src) => Ok(src),
			None => Err(err!("None", "no configuration for {:?}", channel)),
		}
	}

	pub async fn write(&self) -> Result<()> {
		write_toml(&path()?, &self.inner).await
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigToml {
	debug: Option<Source>,
	alpha: Option<Source>,
	beta: Option<Source>,
	release: Option<Source>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
	pub addr: String,
	#[serde(rename = "public-key")]
	pub public_key: PublicKey,
	#[serde(rename = "private-key")]
	pub priv_key: Option<Keypair>,
	/// reader auth key
	#[serde(rename = "auth-key")]
	pub auth_key: Option<AuthKey>,
}

/// Configures an address and a public key to use for uploading for a
/// specific channel.
/// The configuration is stored under `~/.config/publisher/config.toml`.
#[derive(clap::Parser)]
pub struct ConfigOpts {
	channel: Channel,
	address: String,
	public_key: PublicKey,
}

pub async fn configure(opts: ConfigOpts) -> Result<()> {
	let mut cfg = Config::new().await;

	let src = match opts.channel {
		Channel::Debug => &mut cfg.inner.debug,
		Channel::Alpha => &mut cfg.inner.alpha,
		Channel::Beta => &mut cfg.inner.beta,
		Channel::Release => &mut cfg.inner.release,
	};

	*src = Some(Source {
		addr: opts.address,
		public_key: opts.public_key,
		priv_key: None,
		auth_key: None,
	});

	cfg.write().await?;

	println!("configuration written to `~/{}`", PATH);

	Ok(())
}
