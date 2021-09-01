#!/bin/bash
set -e
(cd service; ./pub.sh)
echo "service published"
(cd service-bootloader; ./pub.sh)
echo "service bootloader published"
(cd buildroot; riji patch)
echo "buildroot patched"
(cd buildroot; riji build)
echo "buildroot built"