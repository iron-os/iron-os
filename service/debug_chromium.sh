#!/bin/bash

set -e

echo "call 'cargo run' to get a server running"
echo "---"

# if not the full path is given the extension cannot be loaded
/snap/chromium/current/usr/lib/chromium-browser/chrome \
	--disable-infobars --disable-restore-session-state --disable-session-storage --disable-rollback-option \
	--disable-speech-api --disable-sync --disable-pinch --load-extension="./extension" "http://127.0.0.1:8888"