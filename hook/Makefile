include ./NickelHook/NickelHook.mk

override LIBRARY  := libbookshelfsync.so
override SOURCES  += $(wildcard src/*.cc src/**/*.cc)
override CFLAGS   += -Wall -Wextra -Werror -Wno-missing-field-initializers -fvisibility=hidden
override CXXFLAGS += -Wall -Wextra -Werror -Wno-missing-field-initializers -isystemminiz -fvisibility=hidden -fvisibility-inlines-hidden
override PKGCONF  += Qt5Widgets Qt5Network
override MOCS 	  += $(wildcard src/*.h src/**/*.h)
override QRCS     += res/bookshelfsync.qrc

include ./NickelHook/NickelHook.mk
