TARGET ?= aarch64-unknown-linux-musl

.PHONY: all build clean install

all: build

build:
	PKG_CONFIG_ALLOW_CROSS=1 cargo build --release --target $(TARGET)

install: build
	install -Dm755 target/$(TARGET)/release/fm_radio_gui $(DESTDIR)/usr/local/bin/fm_radio_gui

clean:
	cargo clean
