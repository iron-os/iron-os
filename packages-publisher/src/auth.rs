
use crate::error::Result;
use crate::util::get_priv_key;
use crate::config::Config;

use packages::packages::Channel;
use packages::client::Client;

/// Requests an authentication key from a server.
/// 
/// Automatically stores it in the configuration file.
#[derive(clap::Parser)]
pub struct AuthOpts {
	channel: Channel
}

pub async fn authenticate(opts: AuthOpts) -> Result<()> {

	// check config
	let mut config = Config::open().await?;
	let source = config.get_mut(&opts.channel)?;

	let priv_key = get_priv_key(&source).await?;

	// build a connection
	let client = Client::connect(&source.addr, source.public_key.clone()).await
		.map_err(|e| err!(e, "connect to {} failed", source.addr))?;


	let challenge = client.auth_challenge().await
		.map_err(|e| err!(e, "failed to request new authentication challenge"))?;

	// sign the key
	let sign = priv_key.sign(challenge.as_ref());

	let key = client.auth_key(sign).await
		.map_err(|e| err!(e, "failed to request new auth key"))?;

	source.auth_key = Some(key);

	config.write().await?;

	println!("new authentication key written to configuration");

	// wait until the client is closed
	// this is done since the background task has not time to close
	// the connection since this process ends here
	client.close().await;

	Ok(())
}