

## Name

- chnobli

## Description
Chnobli OS is an operation system based on linux/buildroot
with chromium as it's gui.

## image
the image of chnobli should be for all uses the same

## Image vs Packages
In chnobli there are two ways to add software either
in the rootfs or via packages.

The rootfs is updated bi yearly and if vulnerabilities are discovered
more.
Packages are a shorter release cycle and should be used
for software that changes fast.

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

## Chnobli service

- start chnobli_ui (or chnobli_shell)
- start chromium
- start chnobli_packages
- send logs
- maybe need chromium debug protocol (to be able to log console.logs warnings etc)

- api to start other packages
- api to interact with ui (reset, show display)

- start installer if not installed

- start chnobli_core
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
  - package.jdb // json_db containing information about the package
  - package.jdb.tmp
  - left
  - right

package.db
 - name
 - version_str
 - version // hash
 - signature // signature of the current version
 - current // folder of the current left|right


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