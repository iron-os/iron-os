
use crate::error::{Error, Result};

use serde::{Serialize, Deserialize};
use crypto::signature::{Keypair, PublicKey};

use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
	pub port: u16,
	#[serde(rename = "files-dir")]
	pub files_dir: String,
	#[serde(rename = "con-key")]
	pub con_key: Keypair,
	#[serde(rename = "sign-key")]
	pub sign_key: Option<PublicKey>
}

impl Config {
	fn default() -> Self {
		Self {
			port: 5426,
			files_dir: "./files".into(),
			con_key: Keypair::new(),
			sign_key: None
		}
	}

	pub async fn create() -> Result<Self> {
		if fs::metadata("./config.toml").await.is_ok() {
			return Self::read().await;
		}

		let me = Self::default();
		let s = toml::to_string(&me)
			.expect("could not serialize config.toml");

		fs::write("./config.toml", s).await
			.map_err(|e| Error::new("could not create config.toml", e))?;

		Ok(me)
	}

	pub async fn read() -> Result<Self> {
		let s = fs::read_to_string("./config.toml").await
			.map_err(|e| Error::new("config.toml not found", e))?;
		toml::from_str(&s)
			.map_err(|e| Error::other("config.toml error", e))
	}
}