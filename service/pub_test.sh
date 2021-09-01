#!/bin/bash

set -e

cargo build --release
rsync ./target/release/service user@192.168.14.62:/data/packages/service/left/
rsync -r ./www user@192.168.14.62:/data/packages/service/left/