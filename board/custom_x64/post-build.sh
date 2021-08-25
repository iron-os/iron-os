#!/bin/sh

set -e

BOARD_DIR=$(dirname "$0")

(cd $BOARD_DIR; riji post_build $HOST_DIR $TARGET_DIR $BINARIES_DIR)