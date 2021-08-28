#!/bin/bash

set -e

cargo build --release
cp ./target/release/service_bootloader ../buildroot/buildroot/output/images/service-bootloader