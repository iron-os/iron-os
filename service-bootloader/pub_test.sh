#!/bin/bash

set -e

cargo build --release
rsync ./target/release/service_bootloader user@192.168.14.62:/data/home/service-bootloader