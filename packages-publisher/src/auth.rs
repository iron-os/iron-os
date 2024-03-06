use crate::config::Config;
use crate::error::Result;
use crate::util::get_priv_key;

use packages::client::Client;
use packages::packages::Channel;

/// Requests an authentication reader key from a server.
///
/// Automatically stores it in the configuration file.
#[derive(clap::Parser)]
pub struct AuthOpts {
	channel: Channel,
}

pub async fn authenticate(opts: AuthOpts) -> Result<()> {
	// check config
	let mut config = Config::open().await?;
	let source = config.get_mut(&opts.channel)?;

	let priv_key = get_priv_key(&source).await?;

	println!("connecting to {:?}", source.addr);

	// build a connection
	let client = Client::connect(&source.addr, source.public_key.clone())
		.await
		.map_err(|e| err!(e, "connect to {} failed", source.addr))?;

	client
		.authenticate_writer(&opts.channel, &priv_key)
		.await
		.map_err(|e| err!(e, "failed to authenticate as writer"))?;

	let key = client
		.new_auth_key_reader()
		.await
		.map_err(|e| err!(e, "failed to get auth key reader"))?;

	source.auth_key = Some(key);

	config.write().await?;

	println!("new authentication key reader written to configuration");

	// wait until the client is closed
	// this is done since the background task has not time to close
	// the connection since this process ends here
	client.close().await;

	Ok(())
}
