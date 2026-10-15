## 1.2.7
- Grow the root partitions of devices installed before 2026.10 to 1000MiB on
  the next image update, without touching the data partition (#22)
- Refuse image updates whose rootfs doesn't fit into the root partition
- Write partition table changes so that a power loss always leaves one valid
  copy

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
