#!/bin/sh

set -e

BOARD_DIR=$(dirname "$0")

BUILDROOT_DIR=$(pwd)

(cd $BOARD_DIR/../; riji post_image $BUILDROOT_DIR $HOST_DIR $BINARIES_DIR)