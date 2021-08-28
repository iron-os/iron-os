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

## Installing
Installing can be done in two ways either
via an usb stick or with direct flashing
where the partitions need to be moved later.

both modes need to be supported.
// probably gpt will need to be used.

## version
there should probably be a file in maybe /home/user
which contains the version of this filesystem (and the partition uuid??)

## Todo gpt
document the default sector size