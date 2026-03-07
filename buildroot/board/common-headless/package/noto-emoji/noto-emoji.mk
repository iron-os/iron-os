################################################################################
#
# noto-emoji
#
################################################################################

NOTO_EMOJI_VERSION = 2.051
NOTO_EMOJI_SITE = $(call github,googlefonts,noto-emoji,v$(NOTO_EMOJI_VERSION))
NOTO_EMOJI_LICENSE = OFL-1.1

define NOTO_EMOJI_INSTALL_TARGET_CMDS
	mkdir -p $(TARGET_DIR)/usr/share/fonts/noto-emoji/
	cp -pf $(@D)/fonts/Noto-COLRv1-noflags.ttf $(TARGET_DIR)/usr/share/fonts/noto-emoji/
endef

$(eval $(generic-package))
