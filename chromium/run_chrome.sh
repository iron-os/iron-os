#!/bin/bash
# /snap/chromium/current/usr/lib/chromium-browser/chrome --user-data-dir=~/ChrUnsnapped --class="ChrUnsnapped" --load-extension="./extension" "http://localhost:3002"
chromium/src/out/release/chrome --load-extension="./test_extension" --window-position=0,0 --window-size=1920,1080
