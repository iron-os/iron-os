
# How to Install

## Fresh start
To install and use the custom os, a packages server is needed.
Before that, make sure to have the correct folder structure.

### Required folder structure
- custom-os
- riji
- fire-stream
- fire-http
- magma-css

### Setup Packages Server

`cargo run --release create`

Calling this command will build the package server and create all needed files and folders.

To create the needed signature key do:
`cargo run --release keys > keys.txt`

Now add the signature public key to the `config.toml` file:
```
sign-key = "<public-signature-key>"
```

And to start the server run
`cargo run --release`

### Build publisher

`cargo build --release`

And now make a symlink so you can call it only with `publisher`
`sudo ln -s <packages-publisher-absolute-folder>/target/release/packages-publisher ~/.local/bin/publisher`

### Publish chromium

Make sure the folder `out/release/chromium` exists.

Publish: `publisher upload <Channel> <address> <public-key>`

### Publish service

Make sure the folder `extension/fire-html` and `www/fire-html` exists.

Publish: `publisher upload <Channel> <address> <public-key>`

### Publish user programm (TODO)

### Build buildroot / rootfs

Download buildroot: `riji download`

// Todo: remove channel

Setup packages:
`packages.toml`:
```toml
list = ["service", "chromium", "<user-bin>"]
channel = "Debug"
on-run = "<user-bin>"

[[source]]
address = "<address>"
pub-key = "<con-pub-key>"
sign-key = "<sign-pub-key>"
```