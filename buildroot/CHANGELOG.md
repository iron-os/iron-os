## Unreleased
- first image from master, requires image 2026.11.2 so devices completed the
  partition migration (#22) on the lts line first
- update to buildroot 2026.02.3
- update weston to 16.0.0
- rootfs 600M (was 500M), the Debug rootfs was full
- installer: lock the new disk against udev while writing it

## 2026.11.2
- same as 2026.11.1, the second image update completes the partition
  migration of older devices (merges the old roots into root a, #22)

## 2026.11.1
- update to buildroot 2025.02.18
- new installs: boot partition 128M, root partitions 1.5GiB (#22)
- requires service 1.2.8, which migrates older devices to 1000MiB root
  partitions (#22)
- install_on: wait for udev before formatting the data partition, refuse
  disks and install images which don't fit

## 2025.9.1
- fix startup.nsh file being wrong
- add rtl8188eu support
- disable DNSEC (since it causes issues with rtl8188eu)
- update to buildroot 2025.02.6
- increase rootfs build size
- bootloader and bootloader configs are now getting updated
- allow the user to access the journal logs
- hardware fix gpe07 is now less strict
- make journal persistent
- generate a machine id (/etc/machine-id)
- fix state/updates might not always be written fully to disk
- enable weston screenshooter
- add weston mouse_click api

## 2022.12.1
- add wifi support (drivers, NetworkManager configuration)
- update to buildroot 2022.02.8
