# See package/makedevs/README for details
#
# copied from system/device_table.txt
#
# <name>				<type>	<mode>	<uid>	<gid>	<major>	<minor>	<start>	<inc>	<count>
/dev					d	755	0	0	-	-	-	-	-
/tmp					d	1777	0	0	-	-	-	-	-
/etc					d	755	0	0	-	-	-	-	-
/root					d	700	0	0	-	-	-	-	-
/var/www				d	755	33	33	-	-	-	-	-
/etc/shadow				f	600	0	0	-	-	-	-	-
/etc/passwd				f	644	0	0	-	-	-	-	-
/etc/network/if-up.d			d	755	0	0	-	-	-	-	-
/etc/network/if-pre-up.d		d	755	0	0	-	-	-	-	-
/etc/network/if-down.d			d	755	0	0	-	-	-	-	-
/etc/network/if-post-down.d		d	755	0	0	-	-	-	-	-
# uncomment this to allow starting x as non-root
#/usr/X11R6/bin/Xfbdev		     	f	4755	0	0	-	-	-	-	-
/data					d	644 	user 	user - - - - -
/boot					d	700		0		0 -	-	-	-	-