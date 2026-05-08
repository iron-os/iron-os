# First build

You should have installed all requirements and run the packages setup
step.

## Buildroot

To start a buildroot script we first need to make sure buildroot is
available, for that we can run:
```bash
cd buildroot
riji download
```

This will download the buildroot repo and set it to the correct version.

After that we now can start a build (note this will take between 1h and 3h
depending on your system):
```bash
riji build intel Release
# riji build <board> <channel>
```

Board here can be one of `intel`, `intel-headless`, `pi4` and `pi4-headless` and channel can be `Debug` or `Release`.
In `Debug` builds ssh and other useful tools are included.

To finish the build we need to upload the image to the package server:
```bash
riji upload intel Debug server-name
```

Where `server-name` is the name you used with `publisher config`.

You have now completed the buildroot build, we now need to build the
packages you want to include in your final product.

## Chromium

If you built `intel` or `pi4` you will need to publish `chromium` as well.

### Fast

If you just want to get the current chromium (which is really old) you can
download the following [file](https://drive.google.com/file/d/1FI40A_R5ceuzOxsAgpwp1ly2XDTHdT-u/view?usp=sharing)

Extract the files into `chromium/amd64/release/chromium`.

### Manual

Run the following commands:
```bash
cd chromium
riji download
riji build amd64 Release
```

If you only did a buildroot `Debug` build you need to change the command
to:
```bash
riji build amd64 Release Debug
```

### Publish

Now inside the chromium folder you can run:
```bash
publisher upload --arch Amd64 server-name
```

Where `server-name` is the name you used with `publisher config`.

If you only did a buildroot `Debug` build you need to use:
```bash
publisher upload --arch Amd64 server-name --host-channel Debug
```

## Service

Now the next package you need is the service for the service
to work correctly we need to add a `build-config.toml` file:
```toml
[release]
whitelist = ["127.0.0.1", "localhost", "external.api"]
```

This needs to match what your chromium browser then needs to be able to
access since all other connections will be blocked.

After that config we can publish:
```bash
cd service
publisher upload --arch Amd64 server-name
```

Where `server-name` is the name you used with `publisher config`.

If you only did a buildroot `Debug` build you need to use:
```bash
publisher upload --arch Amd64 server-name --host-channel Debug
```

## App

Now you need a "user" application to run on the os.
For this documentation we will use `example-binary`.

```bash
cd example-binary
publisher upload --arch Amd64 server-name
```

Where `server-name` is the name you used with `publisher config`.

If you only did a buildroot `Debug` build you need to use:
```bash
publisher upload --arch Amd64 server-name --host-channel Debug
```

## Product

Now to build our final image we need to create a new product configuration.

Create a new file, for example `buildroot/products/basic.toml` in this repo.

With the content:
```toml
list = ["service", "chromium", "example-binary"]
channel = "Release"
on-run = "example-binary"
# product is the name that will be returned inside the os
# api, it can be the same for multiple product files
# it should more reflect the generation of product and not if its
# a debug or release build
# based on this you can then toggle features inside your application
product = "one"

[[source]]
address = "myaddr:port"
pub-key = "your-connection-public-key"
sign-key = "your-signature-public-key"
```

This config defines what packages to install how to call it and where to
get the packages from.

## Create image

We are now finally ready to create an image we can burn to a usb stick
or launch inside a vm.

To create the image we can run:
```bash
riji create_image basic intel Release
# riji create_image <product> <board> <host-channel>
```

The host channel here means if buildroot was a `Debug` or `Release` build.

This will download all required packages and then create
`buildroot/output/intel/Release/one.img`.

If virtual box is installed it will also create
`buildroot/output/intel/Release/one.vdi` which you can import to boot.

### Ssh

If you have done a `Debug` build you will be able to ssh into your new os:
```txt
user: user
password: Password!
```
