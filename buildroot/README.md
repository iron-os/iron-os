## Installing

After cloning and installing riji execute:
```
riji download
```
This will download buildroot and set the git head to the
correct version.

Then run:
```
riji patch
```
Which will copy all needed files.

*Todo Prepare packages*

## Changes

When changes are done not in the buildroot folder you should
call `riji patch`. If changes are done in the buildroot or
with `riji config` call `riji save`.

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
8. > custom process

## Partitions

Should have four partitions:

- efi
- boot 1
- boot 2
- data

## Compression (for updates)
brotli + tar

## Users syntax see
http://underpop.online.fr/b/buildroot/en/makeuser-syntax.htm.gz

## chromium
// To launch chromium the XDG_RUNTIME_DIR and WAYLAND_DISPLAY need to be defined.l
// XDG_RUNTIME_DIR=/run/user/14 WAYLAND_DISPLAY=wayland-0 ./chrome --cache-dir=/tmp/ --user-profile=/tmp/ --disable-infobars --disable-rollback-option --disable-speech-api --disable-sync --disable-pinch --kiosk --app="https://youtube.com"

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

## Data partition
- var
- storage
  // folder used for secure storage (namely the web api will store data here)

### Todo
- check what happens if weston crashes
  do we need to restart chromium?