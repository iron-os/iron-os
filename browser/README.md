# Browser (Electron prototype)

Replaces the `chromium` package (Chromium 107 with the Manifest V2 extension
in `service/extension`). Newer Chromium versions no longer support Manifest V2.

It is a drop-in package named `chromium`: the service starts `chrome` like
before and the wrapper ignores the chromium arguments (except
`--remote-debugging-port`, which keeps DevTools working on Debug images).

The Electron app (`app/main.js`):
- connects to the service websocket `ws://127.0.0.1:8888/service-stream`
  like the landing page did, receives the urls from `open_page` and sends
  `StillAlive` every 5s for the service watchdog
- loads the url as the top level page instead of in an iframe, so the
  `X-Frame-Options` and `Content-Security-Policy` headers stay untouched
- blocks requests to hosts not in the whitelist of `app/config.json`
  (same rules as the extension: `host`, `*suffix` or `*`)
- reloads the page if it fails to load or the renderer crashes
- denies popups and permission requests (except fullscreen and clipboard
  writes)

## Building

Needs a Debian 13 (trixie) amd64 host, see `build.sh`.

```bash
./build.sh amd64
# output/amd64/chromium
```

## Open points
- the bundled libraries (mostly GTK3) should come from buildroot
- arm64 (pi4) is not supported yet
- the whitelist should come from `service/build-config.toml`; once this
  replaces chromium the extension, the landing page and the iframe in the
  service can be removed
- only tested in a VM (software rendering), not on real hardware or touch
