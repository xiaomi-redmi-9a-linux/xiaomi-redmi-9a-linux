TARGET ?= aarch64-unknown-linux-musl

.PHONY: all build clean install

all: build

build:
	cargo build --release --target $(TARGET) --config 'target.$(TARGET).linker = "ld.lld"'

install: build
	install -Dm755 target/$(TARGET)/release/mtk-sensor-proxy $(DESTDIR)/usr/local/bin/mtk-sensor-proxy

clean:
	cargo clean
