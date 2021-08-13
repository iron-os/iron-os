
## Changes

When changes are done not in the buildroot folder you should
call `riji patch`. If changes are done in the buildroot or
with `riji config` call `riji create`.

## TODO

http://lists.busybox.net/pipermail/buildroot/2014-May/097233.html

for custom splash screen use plymouth  
and change grub-efi.cfg adding quiet splash  
addd https://github.com/googlefonts/noto-emoji

Look at systemd-repart seams to be what is needed


## Users see
http://underpop.online.fr/b/buildroot/en/makeuser-syntax.htm.gz

## chromium
to launch chromium the flag --in-process-gpu needs to be defined  
also XDG_RUNTIME_DIR and WAYLAND_DISPLAY need to be defined  
why in-process-gpu is required is not quite clear could be because of:
`ERROR:command_buffer_proxy_impl.cc(126)] ContextResult::kTransientFailure: Failed to send GpuControl.CreateCommandBuffer.`