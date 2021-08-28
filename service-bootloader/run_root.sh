#!/bin/bash

set -e

cargo build
sudo ./target/debug/service_bootloader $@
