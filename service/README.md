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

Improve error reporting

## Responses
```
:<:VersionInfo {"version_str":"2021.02.4-debug.6","version":"Abfb4ejTGamxvvzxYqrhs7CiM7c3mtjdrJFIMZ41yI-eaOKVkeq88HQlIDQDoPKPU5rvATvh9QH4BciJBT_GnA","signature":"QFZ9qArk_DU4kLcZ_8iw7xSpkaqV9qkuSi_NmVcbXeMkMKG-rL-b8n3sjrziG5yn8ZKn3ZYiPZCJ4EVjvO6DAw","installed":true}

:<:Disks [{"active":true,"initialized":true,"name":"sda","size":576733696}]
```