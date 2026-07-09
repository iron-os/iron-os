# DPMS wake-by-touch test plan

Goal: confirm or rule out the **wake-by-touch race** as the source of the
leaked touch contacts (`num_tp` drift) behind the permanent
"N points down but no surface focused" failure.

## Hypothesis being tested

When the screen is in DPMS off and someone touches it, weston processes
the touch DOWN and *immediately* wakes the panel (`notify_touch_normalized`
→ `weston_compositor_idle_inhibit` → `weston_compositor_wake` → DPMS ON,
`libweston/input.c:3064` / `input.c:365`). The panel power transition
therefore happens **while the finger is on the glass**. If the eGalax
controller shares the panel's power rail or re-baselines at panel
power-on, the in-flight contact can be swallowed → the UP never arrives
→ one MT slot / `num_tp` count leaks. Three such incidents = the
observed "3 points down".

Prediction if true: touching a DPMS-off screen and holding through the
wake will occasionally leave a slot with `tracking_id != -1` after the
finger is lifted.

Prediction if false: DOWN/UP always pair correctly across the wake, and
the leak comes from somewhere else.

## Prerequisites

- SSH access to the unit (all observation happens over SSH; the local
  screen will be off during the interesting part).
- Root or enough permissions to read `/dev/input/event*`.
- The MT-slot reader: `frame-service`'s `touch_info()` /
  `active_touch_points()` (frame-os), or the standalone
  `read_egalax_mt_slots()` from `weston/egalax_reset.rs`, or the Python
  variant `weston/test-egalax-reset.py`. Any one of them works — they
  all read `EVIOCGMTSLOTS` `tracking_id`s (`-1` = free, `>= 0` = finger
  down).
- Identify the touch node once:
  `ls -l /dev/input/by-id/ | grep -i egalax` — the `if00` /
  "-event" touch entry (historically `event1`).
- A way to trigger sleep on demand: cover/avoid the motion sensor, or
  trigger the kiosk-shell sleep protocol directly if there's a handle
  for it. Note the wake side needs no tooling — touching the screen *is*
  the wake trigger under test.

## Test 0 — sanity check the slot reader (once, healthy device, screen on)

1. Nobody touching: read MT slots → expect **all `-1`**.
2. Touch and hold with one finger: read again → expect exactly **one
   slot `>= 0`**.
3. Lift: read again → all `-1`.

If this doesn't hold, the ioctl encoding/slot count is wrong — fix the
tooling before trusting any later result (known open point: the ioctl
encodings in `egalax_reset.rs` were never verified on hardware).

Also record the baseline: `journalctl -u <weston unit> | grep -i touch`
(or however weston logs are accessed) and `dmesg | tail` so later diffs
are meaningful.

## Test 1 — the main event: wake-by-touch, repeated

Repeat **at least 10 times** (the field failure took weeks to show, so
a low per-attempt probability is expected — more repetitions = more
confidence):

1. Let/make the unit go to sleep (screen off, DPMS off). Verify over
   SSH that weston considers itself sleeping if possible (log line from
   the sleep protocol).
2. Read MT slots over SSH → note the pre-touch state (should be all
   `-1`).
3. **Touch the dark screen with one finger and hold** until the panel
   is fully awake and showing content again, then hold ~2 s more.
4. While still holding, read MT slots over SSH → expect one slot
   `>= 0`. If it's already all `-1` while your finger is physically
   down, the contact was dropped mid-wake — half the smoking gun.
5. **Lift the finger.** Wait 1 s. Read MT slots again.
6. Record the result for this iteration:
   - all `-1` → clean pairing, no leak this round;
   - any slot `>= 0` with nothing touching → **leak reproduced. Smoking
     gun.** Freeze here and continue with "When a leak reproduces"
     below before resetting anything.

Variations worth mixing in across the 10+ runs:

- very short tap on the dark screen (lift *during* the panel power-up);
- slow firm press vs light touch;
- two fingers.

## Test 2 — event-stream view (optional but more precise)

Same as Test 1, but additionally stream the raw node during the cycle:

    cat /dev/input/eventX | xxd     # or evtest if installed

Watch for: a DOWN burst when touching the dark screen, then whether a
matching UP (`ABS_MT_TRACKING_ID = -1` / `BTN_TOUCH 0`) appears when
lifting after the wake. A DOWN without a later UP is the leak, observed
directly. Also note any gap/silence in the stream right at the moment
the panel powers on (controller reset window).

Caveat: only one reader should consume the node this way alongside
weston — reading is fine (evdev fans out to all open fds), but don't
run this during Test 3's log observation if it gets confusing.

## Test 3 — does the panel power transition touch USB at all?

Around a sleep→wake cycle (one from Test 1 is fine):

    dmesg -w    # leave running over SSH during the cycle

Look for anything from `usb 1-1` / the eGalax (`0eef:c000`) correlated
with DPMS off or on: disconnects, re-enumeration, resets, errors. Also
check afterwards whether `/dev/input/by-id` symlink timestamps or event
numbers changed.

- USB events present at DPMS transitions → controller power is coupled
  to panel power → strong support for the race.
- Total silence → the controller rides through DPMS untouched at the
  USB level; a baseline/recalibration glitch (no USB traffic) is still
  possible, so Test 1's result remains the decider.

## Test 4 — control: same procedure, screen awake

10 touch-and-hold-then-lift cycles with the screen already on. Expect
zero leaks. If leaks also happen here, the problem isn't DPMS-related
at all and the wake-race theory is out (or not exclusive).

## When a leak reproduces — do these before clearing it

In order, while the leaked slot is still present:

1. Read MT slots → save exact values (which slot, which tracking_id).
2. Check weston log for new `touch event received with N points down`
   or `Unmatched touch up event` lines; note whether touch still works
   on the screen (it may keep working until focus is lost — that's
   expected per the analysis in `ISSUE.md`).
3. Touch the screen normally a few times → does the leaked slot count
   grow with further wake cycles?
4. `cat /dev/input/eventX | xxd` + touch → do events still flow?
   (Open question 1 from `ISSUE.md`.)
5. Issue `service-bootloader egalax-reset` (USBDEVFS_RESET), then read
   MT slots again → does the reset clear the leaked slot?
   (Open question 2 from `ISSUE.md`.)

## Result interpretation

| Observation | Conclusion |
|---|---|
| Leak reproduces in Test 1, not in Test 4 | Wake-by-touch race confirmed as the entry path. Weston patches (self-healing focus + cancel handling) are the fix; consider cancel-on-wake as well. |
| No leak in 10+ runs of Test 1 | Race unconfirmed — leak likely comes from elsewhere (spontaneous firmware glitch, long-uptime drift). Weston patches still valid; keep field telemetry running to catch the real trigger. |
| Contact visibly dropped mid-wake (step 4 of Test 1) even if slot ends at `-1` | Controller *is* disturbed by the panel wake; even without a persistent kernel-side leak, libinput/weston may see unpaired sequences. Note it and check weston log for unmatched events. |
| USB disconnect/re-enumeration in Test 3 | Device drops off the bus on DPMS transitions — separate, additional entry path (device removal mid-touch, `TOUCH_CANCEL` unhandled by weston). |
| Reset clears leaked slot (step 5) | USBDEVFS_RESET on wake is useful prophylaxis. |
| Reset does NOT clear leaked slot | Firmware ignores USB reset → hardware POR (GPIO-gated 5 V) is the only hardware-side cure; weston patches become the only practical fix. |

## Record for each Test 1 iteration

    #  | pre-slots | held-slots | post-slots | weston log new lines | notes
    1  | all -1    | s0=42      | all -1     | -                    | clean
    2  | ...

Keep the raw numbers — if this goes upstream (weston issue 627), the
iteration table plus a `dmesg` excerpt is exactly the evidence needed.
