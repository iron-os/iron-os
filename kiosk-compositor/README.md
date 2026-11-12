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

## Run
`kiosk-compositor -S wayland-0` from weston.service (logind session on tty1).
`/data/use-weston` makes weston.service start weston instead.

For testing without a display: `kiosk-compositor --headless --headless-size 1280x800` renders
offscreen on `/dev/dri/renderD128`. `cargo run --example kiosk-ctl -- on|off|brightness <n>|click <x> <y>`
sends kiosk protocol requests like the service.
