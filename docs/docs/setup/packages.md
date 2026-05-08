# Packages

## Packages server

The first part you need is a package server, which provides packages and images
to your devices, so they can update themselves.

### Installation

The simplest way to publish this binary is to have a docker container registry ready
then you can just execute the following command:

```bash
cd packages-server
# where myregistry/iron-os/packages-server should be your registry image name
riji publish myregistry/iron-os/packages-server
```

Then create a new folder on your server, for example `/data/packages-server`
and copy the config from `packages-server/compose.example.yaml` into the folder
inside your server. Configure `image`, `port` and `volumes` as needed.

Now we need to create the `data` folder inside the server directory. So
the configuration and packages are kept between restarts.
```bash
mkdir -p /data/packages-server/data
chown 1000:1000 /data/packages-server/data
```

Then we need to configure the container first, for that you can run:
```bash
docker compose run --entrypoint /bin/bash packages-server
# inside the container run
./packages-server --config "/data/config.toml" create
# then
./packages-server --config "/data/config.toml" keys
```

Remember the `Connection public key`, `New signature private key` and 
`New signature public key` as you will need them just bellow (and later).
To complete the configuration you need to modify the `config.toml`.
For than you can run **outside the container**:
```bash
sudo nano /data/packages-server/data/config.toml
# first edit these entries to those values
fies-dir = "/data/files"
auths-file = "/data/auths.fdb"
packages-file = "/data/packages.fdb"
```

And then you can either set one key for all environments or one for each
or both:
```toml
# for all environments
sign-key = "your-public-signature-key"

# for one (debug, alpha, beta, release)
[debug]
sign-key = "your-debug-public-signature-key"
```

How packages deployment works is that the user who publishes the package
needs to enter the **private signature key** which then will be used to sign
the package or image.

To complete the setup run these commands again, they will now create the
missing files folder, auths and packages files:
```bash
docker compose run --entrypoint /bin/bash packages-server
# inside the container run
./packages-server --config "/data/config.toml" create
```

The configuration for the packages server is now done and you can start it with
```bash
docker compose up --remove-orphans -d --pull always
```

### Store

It is important you store your `signature private key` somewhere safe but also
where you will remember, because if you lose this key, **you will not** be able to
publish any updates.

Keep the server ip or a domain, the port, the connection public key and the
signature public key handy, because we will use them for the devices
configuration. If at some point you loose your public connection key you
can look at it again with the keys command:
```bash
docker compose exec -it packages-server /bin/bash
./packages-server --config "/data/config.toml" keys
```

## Publisher

Publishing packages works via the `packages-publisher` binary. Go into the
`packages-publisher` folder and run `cargo install --path .` to install
globally.

If you want you can add an alias to your `.bashrc` file:
```bash
alias publisher="packages-publisher"
```

Remember to run `source ~/.bashrc` after that to apply the changes.

### Configure

To be able to publish to your server, you will need to configure the publisher.

To configure the client run the following command:
```bash
publisher config myserver Release "1.1.1.1:5426" "your-connection-public-key"
# Usage: packages-publisher config <SERVER_NAME> <CHANNEL> <ADDRESS> <PUBLIC_KEY>
```

The server name can be any name and will to used when publishing to target which server
and channel to use. If you intent to only use one server per channel you can name the
server the same as the channel.

### Todo

- add auth command
- add hint how to add the sign private key to the config
