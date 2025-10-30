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
