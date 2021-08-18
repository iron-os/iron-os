
## Changes

When changes are done not in the buildroot folder you should
call `riji patch`. If changes are done in the buildroot or
with `riji config` call `riji create`.

## TODO


Look at systemd-repart seams to be what is needed

## Boot process

1. EFI
2. Linux
3. Systemd
4. psplash
5. kiosk_bootloader
6. debug ? getty : kiosk_web(weston, chromium)
7. kiosk_kernel

## Users syntax see
http://underpop.online.fr/b/buildroot/en/makeuser-syntax.htm.gz

## chromium
// To launch chromium the XDG_RUNTIME_DIR and WAYLAND_DISPLAY need to be defined.l

## external disk
Create a partition with:
```
fdisk -l
```
format partition with:
```
mkfs -t ext4 /dev/sdb1
```
Then mount the filesystem:
```
mount /dev/sdb1 /data
```

## Debugging with gdbserver
Start the server on the vm.
```
gdbserver :<port> <binary>
```
On your system open the executable with symbols:
```
gdb <binary>
target remote <ip>:<port>
```