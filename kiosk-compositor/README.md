# kiosk-compositor

A wayland compositor for Iron OS written with [smithay](https://github.com/Smithay/smithay),
meant to replace our weston fork.

It shows one fullscreen window (the newest toplevel, normally chromium) on the first connected
display and implements:
- `weston_kiosk_shell` (display on/off with state events, brightness, mouse click), the same
  protocol the service uses with weston
- `weston_capture_v1`, so `weston-screenshooter <path>` keeps working (screenshots also work
  while the display is off)
- xdg-shell incl. popups, linux-dmabuf, viewporter, wl_seat with touch, keyboard and (only while
  a mouse is connected or for a click) pointer

A touch on a display that is off only wakes the display, the touch is not passed to the app.

## Debugging
The compositor keeps the last 2000 events in memory and has a debug socket next to the wayland
socket (`$XDG_RUNTIME_DIR/kiosk-compositor.sock`):

```
kiosk-compositor debug stats        # fps, commits/s, render times of the last 120 frames
kiosk-compositor debug events       # live stream of json lines
kiosk-compositor debug dump         # the last 2000 events
kiosk-compositor debug damage on    # flash every area a client repaints
kiosk-compositor debug hud on       # graph: frame interval (line = 60fps), render time, commits
```

Events: `commit` (client, surface, damage rects), `frame` (render time, interval), `flip` (time until
the frame was on the display), `display`, `kiosk_*` requests, `touch_*` (position, if a window got it,
if it only woke the display), `pointer_button`, `key`, `window_*`, `output`, `capture`.
Once a minute while drawing, the journal gets a `stats:` line. The overlays also show up in
screenshots.

## Run
`kiosk-compositor -S wayland-0` from weston.service (logind session on tty1).
`/data/use-weston` makes weston.service start weston instead.

For testing without a display: `kiosk-compositor --headless --headless-size 1280x800` renders
offscreen on `/dev/dri/renderD128`. `cargo run --example kiosk-ctl -- on|off|brightness <n>|click <x> <y>`
sends kiosk protocol requests like the service.
