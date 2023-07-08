

## Name

- iron OS

## Description
Iron OS is an operation system based on linux/buildroot
with chromium as it's gui.

## Rust
MSRV: 1.67

## image
the image of iron should be for all uses the same

## Image vs Packages
In iron there are two ways to add software either
in the rootfs or via packages.

The rootfs is updated bi yearly and if vulnerabilities are discovered
more.
Packages are a shorter release cycle and should be used
for software that changes fast.

## service bootloader

- start iron service package

- supports api via stdin
 - can switch boot img
 - update images from img file
 - can restart
 - watchdog for iron service
   restart if iron service does not send
   connected for a given period
 - start weston service

## service

- start chromium
- send logs

- api to start other packages
- api to interact with ui (reset, show display)

- start frame package



## iron ui
- extension needs to handle the keyboard
- allow secure storage
- set which page to show
- do we need iframes or can the background service redirect
  us
- trigger resets (when screen goes dark or the device is not used)



## packages folder
packages
 - packages.fdb
 - chnobli_ui
  - package.fdb // json_db containing information about the package
  - left
  - right

package.fdb
 - name
 - version_str
 - version // hash
 - signature // signature of the current version
 - current // folder of the current left|right
 - binary // Option<String>


## Install disk (everything as small as possible)
- efi partition
- rootfs partition
- data partition

## Final disk
- efi partition
- rootfs partition 1 (res. 500mb)
- rootfs partition 2 (res. 500mb)
- data partition (50% of target filesize)

## Todo
probably update psplash https://git.yoctoproject.org/cgit/cgit.cgi/psplash/
- reload on no internet (chrome) (or extension)
- pulseaudio in system mode
- unable to load firmware rtl_nic/rtl8168g-2.fw