# Field observation 2026-07-02 — leaked slots survive warm reboot, touch works anyway

## What happened

- A device hit the known failure:

      Jul 02 02:08:17 iron-os weston[311]: [02:08:17.588] touch event received with 2 points down but no surface focused

- It was rebooted with `shutdown -r now`.
- After boot, `active_touch_points()` (frame-service `touch.rs`,
  reading `EVIOCGMTSLOTS` tracking_ids) returned **2**.
- Touch nevertheless **worked normally**.

## Explanation

### 1. The firmware leak survived the warm reboot

`shutdown -r` does not cut USB VBUS on this board — the eGalax
controller never loses power, so its internal "2 fingers down" state
rides straight through the reboot. After boot the controller re-reports
the phantom contacts to the fresh kernel (kernel MT slots initialize to
`-1`; a tracking_id `>= 0` after boot means the device actively
re-asserted them). That is what the slot reader sees.

⇒ The leak lives in the **controller firmware**, and only a real power
cut (POR) clears it — matching the historical "only a power cut fixed
it" observation.

### 2. Weston's stuck state is purely runtime — the reboot cleared it

`num_tp` and touch focus live in weston's process memory. Fresh weston
= `num_tp = 0`, focus = NULL.

### 3. The phantom contacts are invisible to libinput/weston

libinput deliberately does **not** emit `TOUCH_DOWN` for contacts that
were already down when it opened the device. Weston documents this
exact case (`libweston/input.c:3071`, weston 14.0.3):

```c
/* This can happen if we start out with one or
 * more fingers on the touch screen, in which
 * case we didn't get the corresponding down
 * event. */
weston_log("Unmatched touch up event ...");
```

So the 2 leaked slots just sit there, inert — they never feed `num_tp`.

### 4. New real touches use free slots

The controller tracks new fingers in the remaining free slots, sends
proper DOWN/UP pairs, weston sees `num_tp == 1` on the first finger,
picks focus normally → touch works.

## What this proves / changes

- **Two-bug model confirmed with a clean split:** the hardware leak is
  persistent (survives warm reboot); the weston stuck state is
  transient (cleared by restart). The *sticky* part of the failure was
  always weston — so restarting only weston would likely also have
  recovered touch (contrary to the impression from the 2022 upstream
  report), and the weston patches in `ISSUE.md` are the real fix.
- **The slot reader is validated as a field detector:**
  `active_touch_points() > 0` right after boot, before anyone touches,
  ⇒ firmware leak present. Quick sanity check while it reads 2: touch
  with one finger → should read 3; lift → back to 2.
- **Residual risk of leaving slots leaked:** each leaked slot
  permanently consumes one of the controller's contact slots. If leaks
  accumulate over months, the controller eventually runs out — then
  touch dies at the hardware level and no weston patch helps. The leak
  still needs an answer (USB reset or hardware POR), even with weston
  fixed.

## Immediate opportunity

This device is currently **leaked but functional** — ideal for
answering `ISSUE.md` open question 2 *without* waiting for a fully
stuck unit:

1. Read MT slots → expect 2 leaked tracking_ids. Record them.
2. Run `service-bootloader egalax-reset` (USBDEVFS_RESET).
3. Read MT slots again:
   - all `-1` → the USB reset **does** clear firmware leaks → reset-on-wake
     is worthwhile prophylaxis;
   - still 2 → firmware ignores the USB reset → only a true POR
     (power cut / future GPIO-gated 5 V) clears leaks.
4. Verify touch still works after the reset either way.
