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

To build correctly u need a `packages.toml` file containing at least the packages
`chromium` and `service`.

## Changes

If changes are done with `riji config` call `riji save`.

## Boot process

1. EFI
2. Linux
3. Systemd
4. psplash
5. service_bootloader
6. > weston
7. service
8. > chromium
9. $on_run

## Partitions

Should have four partitions:

- boot
- root a
- root b
- data

## Users syntax see
http://underpop.online.fr/b/buildroot/en/makeuser-syntax.htm.gz

## chromium
// To launch chromium the XDG_RUNTIME_DIR and WAYLAND_DISPLAY need to be defined.l
// XDG_RUNTIME_DIR=/run/user/14 WAYLAND_DISPLAY=wayland-0 ./chrome --cache-dir=/tmp/ --user-profile=/tmp/ --disable-infobars --disable-rollback-option --disable-speech-api --disable-sync --disable-pinch --kiosk --app="https://youtube.com"

### Todo this should be improved

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

## Debugging with perf
For this to work you need debug symbols:
```
sudo perf record -F 99 -a -g -- sleep 45
```
To get a report:
```
sudo perf report > perf.report.txt
```

## Data partition
- var
- etc (for configs)
- home
- packages
- storage
  // folder used for secure storage (namely the web api will store data here)

### Important
files in `board/*/data` are not automatically updated and should only be changed with care.

## Install to usb stick
```
dd bs=4M if=disk.img of=/dev/sdc status=progress oflag=sync
```