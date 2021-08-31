#!/bin/bash
set -e

cargo build --release
cp target/release/service ../buildroot/packages/service/left/