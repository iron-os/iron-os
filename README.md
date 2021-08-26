
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


## Name

- chnobli


## image
the image of chnobli should be for all uses the same

## Chnobli service bootloader

- manage pssplash
- start chnobli service package

- supports api via stdin
 - can switch boot img
 - update images from img file
 - can restart
 - watchdog for chnobli service
   restart if chnobli service does not send
   connected for a given period
 - start weston service

### Todo
- check what happens if weston crashes
  do we need to restart chromium?

## Chnobli service

- start chnobli_ui (or chnobli_shell)
- start chromium
- start chnobli_updater
- send logs
- maybe need chromium debug protocol (to be able to log console.logs warnings etc)

- api to start other packages
- api to interact with ui (reset, show display)

- start installer if not installed

- start frame package
 - 



## chnobli ui
- extension needs to handle the keyboard
- allow secure storage
- set which page to show
- do we need iframes or can the background service redirect
  us
- trigger resets (when screen goes dark or the device is not used)



## packages folder
packages
 - packages.jdb
 - chnobli_ui
  - chnobli_ui.cfg // json_db containing information about the package
  - chnobli_ui.cfg.tmp
  - left
  - right