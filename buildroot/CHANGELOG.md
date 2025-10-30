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
