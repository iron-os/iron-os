# eGalax touchscreen: touch stops working permanently ("N points down but no surface focused")

Upstream report: https://gitlab.freedesktop.org/wayland/weston/-/work_items/627
(filed 2022 against weston 10.0.0; the relevant code is **unchanged in the
currently running weston 14.0.3** — verified against the checkout, see
"Code-level analysis" below).

## Summary

On devices with an eGalax `0eef:c000` touchscreen (eGalaxTouch P80H100,
Apollo Lake host, kiosk-shell + chromium, custom motion-sensor
sleep/wake protocol calling `weston_compositor_sleep`/`_wake`), touch
input occasionally dies **completely and permanently**. Weston logs:

    touch event received with 3 points down but no surface focused

on every subsequent touch. Restarting chromium or weston does not
recover; historically only a full power cut did.

Two independent bugs interact:

1. **Firmware/hardware:** the eGalax controller leaks MT slots — it
   keeps asserting fingers down (`tracking_id != -1`) that were never
   lifted. Kernel/libinput therefore deliver touch DOWNs that never get
   a matching UP.
2. **Weston (14.0.3):** touch focus/bookkeeping has no recovery path.
   Once `touch->focus == NULL` while `touch->num_tp > 0`, every future
   touch DOWN is logged and dropped, forever.

Bug (1) is what pushes us into the state; bug (2) is why it's sticky.
Weston is the proximate bug: even with flaky firmware it should be able
to self-heal.

## How the stuck state forms

Weston's `weston_touch` keeps a seat-global finger counter `num_tp` and
a focus view picked by the _first_ finger of a session:

- `num_tp++` on every DOWN, `num_tp--` on every UP
  (`libweston/input.c`, `notify_touch_normalized`).
- Focus is picked only when `num_tp == 1` on a DOWN, and only cleared
  at frame-time when `num_tp == 0` (`pending_focus_reset`).

Failure sequence:

1. Controller leaks one or more slots → weston receives DOWNs with no
   matching UPs → `num_tp` never drains back to 0 (e.g. stuck at 3).
2. Something clears focus while `num_tp > 0`. Verified paths:
    - the focused view/surface is destroyed
      (`touch_focus_view_destroyed`, `input.c:409`) — e.g. chromium
      restarts, navigates, or the kiosk surface is remapped;
    - focus resource destroyed (`touch_focus_resource_destroyed`).
3. From now on every DOWN takes the dead branch in
   `process_touch_normal()`: `num_tp` is never 1 (leaked slots), focus
   is NULL → log "touch event received with %d points down but no
   surface focused" → `return`. **No event is ever delivered again and
   nothing resets the state.**

This also explains why restarting chromium makes it _worse_, not
better: it destroys the focused surface (step 2) but `num_tp` survives
because it lives in the seat, not the client.

The sleep/wake protocol is a plausible trigger for step 1/2: DPMS off
can power-cycle the panel's internal USB (device disconnect/reconnect
mid-touch), and if fingers are on the glass when the sensor sleeps the
compositor, the session is never terminated cleanly.

## Code-level analysis (weston 14.0.3, verified in checkout)

All references are to the `weston` project root.

### a) No recovery on DOWN — `libweston/input.c:2875`

```c
case WL_TOUCH_DOWN:
    if (touch->num_tp == 1) {
        ev = weston_compositor_pick_view(ec, *pos);
        weston_touch_set_focus(touch, ev);
    } else if (!touch->focus) {
        weston_log("touch event received with %d points down "
                   "but no surface focused\n", touch->num_tp);
        return;
    }
```

Only the first finger picks focus. `focus == NULL && num_tp != 1` is
treated as unexpected and the event is dropped — permanently, since
nothing in this path ever repairs `num_tp` or focus.

### b) Empty cancel handler — `libweston/input.c:1002`

```c
static void
default_grab_touch_cancel(struct weston_touch_grab *grab)
{
}
```

The default touch-cancel is a no-op. So even the paths that _do_ call
cancel (`weston_seat_release_touch`, `input.c:4299`; nested wayland
backend) don't send `wl_touch.cancel` to clients. Note
`weston_seat_release_touch` does separately call
`weston_touch_reset_state()` (`num_tp = 0`), but only when the **last**
touch device of the seat is removed.

### c) `LIBINPUT_EVENT_TOUCH_CANCEL` is not handled — `libweston/libinput-device.c:796`

The DRM/libinput backend handles TOUCH_DOWN/MOTION/UP/FRAME only.
`LIBINPUT_EVENT_TOUCH_CANCEL` (emitted by libinput e.g. when a touch
device is suspended/removed with contacts down, or on palm/thumb
detection) falls into the `default:` branch → "unknown libinput event".
So libinput's _own_ attempt to terminate a broken session is silently
discarded, and `num_tp` keeps the phantom fingers. Only the nested
wayland backend calls `notify_touch_cancel()`.

### d) Sleep does not cancel touch — `libweston/compositor.c:6130`

```c
weston_compositor_sleep(struct weston_compositor *compositor)
{
    if (compositor->state == WESTON_COMPOSITOR_SLEEPING)
        return;
    wl_event_source_timer_update(compositor->idle_source, 0);
    compositor->state = WESTON_COMPOSITOR_SLEEPING;
    weston_compositor_dpms(compositor, WESTON_DPMS_OFF);
}
```

Our motion-sensor protocol calls this with fingers potentially on the
glass. Nothing cancels the in-flight touch session, so stale `num_tp`
is carried across the sleep/wake cycle.

Note: `weston_touch_cancel_grab()` is `static` in `input.c:2138`, so a
sleep-path fix needs it exported (or implemented inside input.c).

## Hardware-layer findings (on-device testing, 2026-06-04)

- Targeted `USBDEVFS_RESET` (via `egalax_reset.rs` /
  `service-bootloader egalax-reset`) fires correctly:
  `usb 1-1: reset full-speed USB device number 2 using xhci_hcd`.
- It is an **in-place port reset**: no disconnect/reconnect, devnum
  stays the same, `/dev/input/by-id/` symlinks unchanged (still
  event1/event2 from before the reset). The reset is transparent to
  evdev/libinput/weston — so a "weston fails to re-grab the node"
  theory is **ruled out**.
- After the reset touch still did **not** work — consistent with the
  stuck state living in weston's bookkeeping, not (only) in the
  controller. Whether the reset clears the firmware's leaked slots is
  still unconfirmed (see open questions).
- If `USBDEVFS_RESET` cannot clear leaked slots, the only
  software-controllable cure at the hardware level is a true power-on
  reset — e.g. a GPIO-controlled load switch (AP22804 or similar) on
  the panel's 5 V line in a future hardware revision.

## Monitoring already in place

`frame-os/frame-service/src/touch.rs` (`touch_info()` /
`active_touch_points()`) reads the kernel's MT slot state directly via
`EVIOCGMTSLOTS` (`tracking_id != -1` ⇒ finger down), independent of
weston's `num_tp`. This gives us ground truth from the field:

- **contacts > 0 while nobody is touching** ⇒ firmware slot leak
  confirmed (bug 1 active);
- **contacts == 0 but weston logs "N points down"** ⇒ weston's counter
  drifted on its own / the leak self-cleared but weston stayed stuck
  (bug 2 alone).

`weston/egalax_reset.rs` additionally provides `reset_egalax()`
(USBDEVFS_RESET) and `read_egalax_mt_slots()` /
`egalax_has_stuck_slots()` for before/after verification.

## Open questions

1. **When stuck: do events still reach the kernel?** On a stuck unit:
   `cat /dev/input/event1 | xxd`, touch the screen.
    - bytes appear → device fine, weston drops them → weston patches are
      the complete fix;
    - no bytes → controller itself is wedged → weston patches make it
      recoverable-on-next-working-touch but a POR path is also needed.
2. **Does USBDEVFS_RESET clear leaked slots?** Snapshot
   `read_egalax_mt_slots()` before/after a reset on a device with a
   confirmed leak. Also sanity-check the ioctl encodings on a healthy
   device first (all slots must read -1).
3. **What clears focus in our kiosk setup** — surface destruction from
   chromium, or something in the sleep/wake path? Correlate weston log
   timestamps with sleep/wake events and chromium lifecycle.
4. Does DPMS off actually power-cycle the panel USB on this hardware
   (check `dmesg` for USB disconnects around sleep)?

## Proposed fixes (layered — do all that apply)

### 1. Weston patch: self-healing focus (the real fix)

`libweston/input.c`, `process_touch_normal()`: pick a new view on DOWN
whenever `!touch->focus`, not only when `num_tp == 1`, and treat it as
the start of a fresh session for grab bookkeeping. Drafted in
`weston/egalax-stuck-touch.md` (Patch 1). This makes the stuck state
self-heal on the next touch regardless of how we got there.

### 2. Weston patch: meaningful cancel

Make `default_grab_touch_cancel` reset `num_tp = 0` and clear focus
(and ideally send `wl_touch.cancel` to focused resources). Then every
existing cancel path actually cleans up. Also part of Patch 1 draft.

### 3. Weston patch: cancel touch on sleep

`weston_compositor_sleep()` should cancel in-flight touch sessions on
all seats before DPMS off. Requires exporting the currently-static
`weston_touch_cancel_grab()` or adding a wrapper. Drafted as Patch 2 in
`weston/egalax-stuck-touch.md` (note the linkage caveat there).

### 4. Weston patch (new finding): handle `LIBINPUT_EVENT_TOUCH_CANCEL`

`libweston/libinput-device.c`: route the event to
`notify_touch_cancel()` like the nested wayland backend does. With fix
(2) in place this closes the device-removal/suspend-with-fingers-down
entry path.

### 5. OS layer: prophylactic USB reset + telemetry

- Keep `service-bootloader egalax-reset` (USBDEVFS_RESET) on wake from
  sleep as cheap prophylaxis — but treat it as unproven until open
  question 2 is answered.
- Extend frame-service to log/alert when `active_touch_points() > 0`
  for an extended period with the compositor idle — early detection of
  the firmware leak in the field.

### 6. Hardware (long term)

If the firmware leak persists across USB resets, spec a GPIO-gated 5 V
load switch for the panel so a true POR can be issued in software.

## Suggested plan of attack

1. Answer open questions 1 + 2 the next time a unit gets stuck (evdev
   byte check, MT-slot snapshot before/after reset).
2. Apply weston patches 1, 2, 4 (input.c + libinput-device.c) to our
   14.0.3 build; patch 3 if the sleep/wake correlation (question 3)
   confirms it as an entry path.
3. Reproduce artificially: hold fingers on the glass, restart chromium
   (destroys the focused surface) while a finger stays down; or inject
   phantom DOWNs via evdev uinput to simulate the leak. Verify the
   patched weston recovers on the next tap.
4. Ship telemetry (fix 5) so we can quantify how often the firmware
   leak occurs, then decide on the hardware POR (fix 6).
