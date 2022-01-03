
Chromium cannot built with the standard sysroot from buildroot:
libgcc.a & crtbeginS.o & crtendS.o needs to be added from host/lib/gcc/"triple"
to host/"triple"/lib/

## Create installer manually

- cp_execs: chrome, chrome_crashpad_handler, chrome_sandbox
- cp shlibs: libEGL.so, libGLESv2.so, libvk_swiftshader.so
-     "$root_out_dir/swiftshader/libEGL.so",
    "$root_out_dir/swiftshader/libGLESv2.so",
    "$root_out_dir/vk_swiftshader_icd.json",
    "$root_out_dir/xdg-mime",
    "$root_out_dir/xdg-settings",
    "$root_out_dir/locales/en-US.pak",
    "$root_out_dir/MEIPreload/manifest.json",
    "$root_out_dir/MEIPreload/preloaded_data.pb",

- strip chrome_binary

// branding maybe?
branding_dir = "//chrome/app/theme/$branding_path_component"
branding_dir_100 =
    "//chrome/app/theme/default_100_percent/$branding_path_component"

       "$branding_dir/BRANDING",
    "$branding_dir/linux/product_logo_32.xpm",
    "$branding_dir/product_logo_128.png",
    "$branding_dir/product_logo_24.png",
    "$branding_dir/product_logo_256.png",
    "$branding_dir/product_logo_48.png",
    "$branding_dir/product_logo_64.png",
    "$branding_dir_100/product_logo_16.png",
    "$branding_dir_100/product_logo_32.png",

## The build probably works
but libvk_swiftshader.so etc are to big instead of 200mb the entire thing is 500mb
it should be possible to go down to 200 again.