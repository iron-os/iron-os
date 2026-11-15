## 1.2.11
- Requires image 2026.11.2 (devices with the new partition layout, #22),
  1.2.9 and 1.2.10 were only test builds on two devices
- Keep running while the compositor is away or chromium can't start, so
  package updates still arrive: the display connection is retried forever
- Display state and brightness requests wait up to 10s for the compositor,
  the last state and brightness are restored when it comes back, old clicks
  are dropped
- Apply finished package updates even if another package or the image failed
- Reboot into an image update which was written but not booted (at most once
  per image version)
- Sync package.fdb and version.fdb to disk when writing them (file-db 0.2.1)

## 1.2.8
- Grow the root partitions of devices installed before 2026.10 to 1000MiB on
  the next image update, without touching the data partition (#22)
- Refuse image updates whose rootfs doesn't fit into the root partition
- Write partition table changes so that a power loss always leaves one valid
  copy
- Sync the kernel and bootloader files before switching to them
- Refuse to write an incomplete rootfs

## 1.2.6
- Add Screenshot API
- Add MouseClick API

## 1.2.5
- Trigger StillAlive websocket request when the connection get's opened
- Include service-bootloader inside the service package this allows us to fix #10
- Fix #10 sometimes the disk would get corrupted on updates
- send image_version and package_version to the packages server

## 1.2.4
- allow to trigger an update
- allow to request current available access points and connections
- allow to connect to an access point
- allow to add gsm connections
- updates are now downloaded in chunks allowing to continue downloading when the connection get's lost
