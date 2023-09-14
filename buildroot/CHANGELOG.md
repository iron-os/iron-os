## 2023.09.1 (not released)
- fix startup.nsh file being wrong

## 2023.06.2 (yanked)
Was yanked because for some reason devices just shutdown (probably a kernel panic or something)
- add rtl8188eu support
- disable DNSEC (since it causes issues with rtl8188eu)
- update to buildroot 2023.02
- increase rootfs build size
- bootloader and bootloader configs are now getting updated
- allow the user to access the journal logs
- hardware fix gpe07 is now less strict

## 2022.12.1
- add wifi support (drivers, NetworkManager configuration)
- update to buildroot 2022.02.8