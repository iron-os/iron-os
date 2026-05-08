# Requirements

For production builds a Linux system is a hard requirement, however for
development of your own rust applications which require the service api
any system should work.

## Riji

[Riji](https://docs.rs/riji/latest/riji/) is the main scripting tool for
building and managing iron os. It requires [rust](https://rust-lang.org/learn/get-started/)
and can be installed via `cargo install riji`.

## Buildroot

Buildroot requires various packages to be installed see [Buildroot](https://buildroot.org/downloads/manual/manual.html#requirement).

For a smooth setup i recommend Ubuntu 24 or 25 with the mandatory packages
installed from the buildroot manual, other operating systems might work as well.

## Server

For running the packages server you will need a linux server for the easiest
setup docker should be installed.
