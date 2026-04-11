KDIR ?= /lib/modules/$(shell uname -r)/build
SUBDIRS := tuxedo-ec tuxedo-uniwill tuxedo-clevo tuxedo-tuxi tuxedo-nb04

all:
	@for dir in $(SUBDIRS); do \
		$(MAKE) -C $(KDIR) M=$(CURDIR)/$$dir modules || exit 1; \
	done

clean:
	@for dir in $(SUBDIRS); do \
		$(MAKE) -C $(KDIR) M=$(CURDIR)/$$dir clean; \
	done

.PHONY: all clean
