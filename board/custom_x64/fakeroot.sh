
set -e

BOARD_DIR=$(dirname "$0")

(cd $BOARD_DIR; riji fakeroot $HOST_DIR $1 $BINARIES_DIR)