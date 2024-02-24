use crate::error::{Error, Result};

use serde::{Serialize, Deserialize};
use crypto::signature::{Keypair, PublicKey};

use packages::packages::Channel;

use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
	pub port: u16,
	pub sentry_url: Option<String>,
	pub files_dir: String,
	#[serde(default = "default_auths_file")]
	pub auths_file: String,
	#[serde(default = "default_packages_file")]
	pub packages_file: String,
	pub con_key: Keypair,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub sign_key: Option<PublicKey>,
	// channels
	pub debug: Option<ChannelCfg>,
	pub alpha: Option<ChannelCfg>,
	pub beta: Option<ChannelCfg>,
	pub release: Option<ChannelCfg>
}

fn default_auths_file() -> String {
	"./auths.fdb".into()
}

fn default_packages_file() -> String {
	"./packages.fdb".into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelCfg {
	#[serde(rename = "sign-key")]
	pub sign_key: PublicKey
}

impl Default for Config {
	fn default() -> Self {
		Self {
			port: 5426,
			sentry_url: None,
			files_dir: "./files".into(),
			auths_file: default_auths_file(),
			packages_file: default_packages_file(),
			con_key: Keypair::new(),
			sign_key: None,
			debug: None,
			alpha: None,
			beta: None,
			release: None
		}
	}
}

impl Config {
	pub async fn create(path: &str) -> Result<Self> {
		if fs::metadata(path).await.is_ok() {
			return Self::read(path).await;
		}

		let me = Self::default();
		let s = toml::to_string(&me)
			.expect("could not serialize config.toml");

		fs::write(path, s).await
			.map_err(|e| Error::new("could not create config.toml", e))?;

		Ok(me)
	}

	pub async fn read(path: &str) -> Result<Self> {
		let s = fs::read_to_string(path).await
			.map_err(|e| Error::new("config.toml not found", e))?;
		let mut s: Self = toml::from_str(&s)
			.map_err(|e| Error::other("config.toml error", e))?;

		if let Some(sign_key) = &s.sign_key {
			s.debug.get_or_insert_with(|| ChannelCfg {
				sign_key: sign_key.clone()
			});
			s.alpha.get_or_insert_with(||
				ChannelCfg { sign_key: sign_key.clone()
			});
			s.beta.get_or_insert_with(||
				ChannelCfg { sign_key: sign_key.clone()
			});
			s.release.get_or_insert_with(||
				ChannelCfg { sign_key: sign_key.clone()
			});
		}

		Ok(s)
	}

	pub fn has_sign_key(&self) -> bool {
		self.debug.is_some() ||
		self.alpha.is_some() ||
		self.beta.is_some() ||
		self.release.is_some()
	}

	/// you need to call has_sign_key before calling this
	pub fn sign_pub_key_by_channel(
		&self,
		channel: Channel
	) -> Option<&PublicKey> {
		match channel {
			Channel::Debug => self.debug.as_ref().map(|c| &c.sign_key),
			Channel::Alpha => self.alpha.as_ref().map(|c| &c.sign_key),
			Channel::Beta => self.beta.as_ref().map(|c| &c.sign_key),
			Channel::Release => self.release.as_ref().map(|c| &c.sign_key),
		}
	}
}