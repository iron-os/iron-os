# kiosk-compositor

A wayland compositor for Iron OS written with [smithay](https://github.com/Smithay/smithay),
it replaces our weston fork.

It shows one fullscreen window (the newest toplevel, normally chromium) on the first connected
display and implements:
- `weston_kiosk_shell` (display on/off with state events, brightness, mouse click), the same
  protocol the service uses with weston
- `weston_capture_v1`, so `weston-screenshooter <path>` keeps working (screenshots also work
  while the display is off)
- xdg-shell incl. popups, linux-dmabuf, viewporter, wl_seat with touch, keyboard and (only while
  a mouse is connected or for a click) pointer

A touch on a display that is off only wakes the display, the touch is not passed to the app.
Touches still down are cancelled when the display turns off or the touchscreen disappears.

Not implemented, on purpose for a kiosk:
- popup grabs: a tap outside a popup (e.g. an open `<select>`) is passed to the window below and
  the popup gets no keyboard focus, chromium closes its popups itself
- no cursor is drawn, also not with a mouse connected

## Logs
The journal (40M on Iron OS) gets:
- `stats:` once a minute while frames are drawn: fps, render and flip times, failed frames
- `touch:` once a minute with touches: touches, how many only woke the display, how long fingers
  were down, touches again without a release, fingers still down and a 4x3 grid of where the
  touches were; a finger held for more than 30s is a warning right away
- errors and warnings, repeating ones at most once a minute, panics as `panic: ...`

## Debugging
A debug socket next to the wayland socket (`$XDG_RUNTIME_DIR/wayland-0-debug.sock`):

```
kiosk-compositor debug stats        # fps, commits/s, render times of the last 120 frames
kiosk-compositor debug events       # live stream of all events as json lines
kiosk-compositor debug dump         # the last 2000 rare events
kiosk-compositor debug damage on    # flash every area a client repaints
kiosk-compositor debug hud on       # graph: frame interval (line = 60fps), render time, commits
```

Rare events (kept for `dump`): `display`, `kiosk_*` requests, `touch_down`/`touch_up` (position, if
a window got it, if it only woke the display), `touch_cancel`, `pointer_button`, `key`,
`input_device_*`, `window_*`, `output`, `capture`. Frequent events (only live): `commit` (client,
surface, damage rects), `frame` (render time, interval), `flip` (time until the frame was on the
display), `touch_motion`. The overlays also show up in screenshots.

## Run
`kiosk-compositor -S wayland-0` from weston.service (logind session on tty1, `Type=notify`: ready
once the wayland socket exists). The unit keeps the name weston, the service-bootloader and
chromium.service use it.

For testing without a display: `kiosk-compositor --headless --headless-size 1280x800` renders
offscreen on `/dev/dri/renderD128`. `cargo run --example kiosk-ctl -- on|off|brightness <n>|click <x> <y>`
sends kiosk protocol requests like the service.
