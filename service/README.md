## Chnobli service

- start ui server
- start chromium
- start package manager
- maybe need chromium debug protocol (to be able to log console.logs warnings etc)

- api to start other packages
- api to interact with ui (reset, show display)

- start installer if not installed

## chnobli ui
- extension needs to handle the keyboard
- allow secure storage
- set which page to show
- do we need iframes or can the background service redirect us
- trigger resets (when screen goes dark or the device is not used)

- install to disk
- detect which channel we are in

## Todo
make sure no client can connect to `/websocket`

## Responses
```
:<:VersionInfo {"buildroot_version":"2021.02.4","version":1,"installed":false,"channel":"Debug"}

:<:Disks [{"active":true,"initialized":true,"name":"sda","size":576733696}]
```