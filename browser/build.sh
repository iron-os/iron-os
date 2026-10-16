#!/bin/bash
# Builds the browser package in ./output/<arch>/chromium
#
# Downloads the Electron release, adds the app and the wrapper and bundles
# the shared libraries Electron needs that are not in the image (mostly
# GTK3). The libraries are copied from the host, so this needs a Debian 13
# (trixie) amd64 host, its glibc 2.41 matches the image. Long term these
# libraries should be built by buildroot instead.
set -euo pipefail

ELECTRON_VERSION=44.8.0

# the host packages providing the bundled libraries
DEB_PACKAGES="libatk-bridge2.0-0t64 libatk1.0-0t64 libatspi2.0-0t64
	libavahi-client3 libavahi-common3 libcloudproviders0 libcups2t64 libepoxy0
	libgmp10 libgnutls30t64 libgssapi-krb5-2 libgtk-3-0t64 libhogweed6t64
	libidn2-0 libk5crypto3 libkeyutils1 libkrb5-3 libkrb5support0
	libnettle8t64 libp11-kit0 libtasn1-6 libunistring5 libxcomposite1
	libxcursor1 libxdamage1 libxi6 libxinerama1 libxrandr2"

# missing in the image or (libepoxy) built without glx which gtk3 needs
LIBS="libXcomposite.so.1 libXcursor.so.1 libXdamage.so.1 libXi.so.6
	libXinerama.so.1 libXrandr.so.2 libatk-1.0.so.0 libatk-bridge-2.0.so.0
	libatspi.so.0 libavahi-client.so.3 libavahi-common.so.3
	libcloudproviders.so.0 libcups.so.2 libgdk-3.so.0 libgmp.so.10
	libgnutls.so.30 libgssapi_krb5.so.2 libgtk-3.so.0 libhogweed.so.6
	libidn2.so.0 libk5crypto.so.3 libkeyutils.so.1 libkrb5.so.3
	libkrb5support.so.0 libnettle.so.8 libp11-kit.so.0 libtasn1.so.6
	libunistring.so.5 libepoxy.so.0"

ARCH=${1:?usage: build.sh <amd64>}
case $ARCH in
	amd64) E_ARCH=x64; LIB_DIR=/usr/lib/x86_64-linux-gnu ;;
	*) echo "arch $ARCH is not supported yet"; exit 1 ;;
esac

cd "$(dirname "$0")"
DL=./output/dl
OUT=./output/$ARCH/chromium
ZIP=electron-v$ELECTRON_VERSION-linux-$E_ARCH.zip
URL=https://github.com/electron/electron/releases/download/v$ELECTRON_VERSION

mkdir -p "$DL"
[ -f "$DL/$ZIP" ] || curl -fsSL -o "$DL/$ZIP" "$URL/$ZIP"
curl -fsSL -o "$DL/SHASUMS256.txt" "$URL/SHASUMS256.txt"
(cd "$DL" && grep " \*$ZIP\$" SHASUMS256.txt | sha256sum -c --quiet)

for p in $DEB_PACKAGES; do
	dpkg -s "$p" >/dev/null 2>&1 || { echo "missing host package $p"; exit 1; }
done

rm -rf "$OUT"
mkdir -p "$OUT/lib" "$OUT/resources/app"
unzip -q "$DL/$ZIP" -d "$OUT"
rm -f "$OUT/resources/default_app.asar"
cp app/* "$OUT/resources/app/"
cp chrome "$OUT/chrome"
for l in $LIBS; do
	cp -L "$LIB_DIR/$l" "$OUT/lib/"
done

echo "built $OUT ($(du -sh "$OUT" | cut -f1))"
