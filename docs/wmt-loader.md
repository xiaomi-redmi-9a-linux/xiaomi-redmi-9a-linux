# MediaTek WMT Native Loader (`mtk-wmt-loader`)

## Overview

On MediaTek SoCs (such as the Helio G25 / MT6765 on Xiaomi Redmi 9A `dandelion`/`blossom`), connectivity subsystems (Wi-Fi, Bluetooth, GPS, and FM Radio) are governed by the Wireless Management Task (**WMT**) kernel framework (`wmt_drv`, `wmt_detect`, `wmt_chrdev_wifi`).

Historically, postmarketOS and early Linux ports used **`wmt-pyloader`**, an 850-line Python daemon that:
1. Performed the `/dev/wmtdetect` hardware handshake.
2. Initialized STP (Serial Transport Protocol) on `/dev/stpwmt`.
3. Listened for kernel firmware requests (`srh_rom_patch` and `srh_patch`) and pushed firmware patch blobs from `/lib/firmware/` via ioctl.
4. Kept running 24/7 in OpenRC's `boot` runlevel, consuming **~14 MB of RAM** (`VmRSS: 13,920 kB`) just to keep the Python runtime resident.

We implemented a native Rust replacement daemon: **[`src/mtk-wmt-loader`](../src/mtk-wmt-loader/)**.

---

## Performance Comparison

| Metric | Legacy Python (`wmt-pyloader`) | Native Rust (`mtk-wmt-loader`) | Improvement |
| :--- | :---: | :---: | :---: |
| **Binary Size** | ~18 MB (Python 3.14 + stdlib) | **389 KB** (static musl ELF) | **~98% reduction** |
| **Resident Memory (VmRSS)** | **13,920 kB (~14 MB)** | **< 4 kB (~0 MB)** | **> 99.9% reduction** |
| **Virtual Memory (VmSize)** | 22,184 kB | 2,652 kB | **88% reduction** |
| **Runtime Dependencies** | Python 3, fcntl, struct, glob | **None** (statically linked musl) | Zero external dependencies |
| **OpenRC Boot Runlevel** | `boot` (required Python interpreter) | `boot` (native standalone executable) | Python removed from boot |

---

## Hardware Architecture & Protocol Flow

```mermaid
sequenceDiagram
    participant D as mtk-wmt-loader (userspace)
    participant K as Kernel (wmt_detect / wmt_drv)
    participant H as MT6631 Connectivity HW

    Note over D,K: Stage 1: Loader (/dev/wmtdetect)
    D->>K: ioctl(COMBO_IOCTL_CONNSYS_SOC_HW_INIT)
    D->>K: ioctl(COMBO_IOCTL_EXT_CHIP_PWR_ON)
    K-->>D: -1 (SOC integrated combo chip)
    D->>K: ioctl(COMBO_IOCTL_GET_SOC_CHIP_ID)
    K-->>D: 0x6765 (MT6765 SoC)
    D->>K: ioctl(COMBO_IOCTL_SET_CHIP_ID, 0x6765)
    D->>K: ioctl(COMBO_IOCTL_MODULE_CLEANUP, 0x6765)
    D->>K: ioctl(COMBO_IOCTL_DO_MODULE_INIT, 0x6765)
    D->>K: ioctl(COMBO_IOCTL_GET_ADIE_CHIP_ID)
    K-->>D: 0x6631 (MT6631 ADIE)

    Note over D,K: Stage 2: Launcher (/dev/stpwmt)
    D->>K: ioctl(WMT_IOCTL_WMT_QUERY_CHIPID)
    K-->>D: 0x6765
    D->>K: ioctl(WMT_IOCTL_GET_CHIP_INFO, FWVER)
    K-->>D: 0x8a00 (Firmware mapping version)
    D->>K: ioctl(WMT_IOCTL_SET_PATCH_NAME, "WMT_SOC.cfg")
    D->>K: ioctl(WMT_IOCTL_SET_STP_MODE, config) [4Mbps, FM=2, STP=3]
    D->>K: ioctl(WMT_IOCTL_SET_LAUNCHER_KILL, 0)

    par Background Workers
        D->>K: Loopback power control thread
    and
        D->>K: Auto-enable Wi-Fi (/dev/wmtWifi <- 1)
    and Event Loop
        K->>D: read(fd) -> "srh_rom_patch"
        D->>K: ioctl(WMT_IOCTL_SET_ROM_PATCH_INFO, soc1_0_ram_*)
        D->>K: write(fd, "ok")
        K->>D: read(fd) -> "srh_patch"
        D->>K: ioctl(WMT_IOCTL_SET_PATCH_INFO, soc1_0_patch_*)
        D->>K: write(fd, "ok")
    end
```

---

## Source Tree Structure

Located at [`src/mtk-wmt-loader/`](../src/mtk-wmt-loader/):

- `Cargo.toml`: Profile configured with `opt-level = "z"`, `lto = true`, `strip = true`, `panic = "abort"`.
- `Makefile`: Cross-compiles for `aarch64-unknown-linux-musl` with `ld.lld`.
- `src/ioctl.rs`: Linux ioctl definitions matching kernel `_IOW`/`_IOR` constants and 264-byte `WmtPatchInfo` C struct.
- `src/loader.rs`: Implements Stage 1 (`/dev/wmtdetect` handshake and chip identification).
- `src/launcher.rs`: Implements Stage 2 (`/dev/stpwmt` configuration, workers, and blocking `poll(..., -1)` event loop with signal self-pipe).
- `src/patch.rs`: Firmwares discovery (`glob`), patch validation, header unpacking, and Bluetooth build ID parsing.
- `src/main.rs`: Daemon entry point with self-pipe POSIX signal (`SIGINT`/`SIGTERM`) handling and zero idle polling wakeups.

---

## OpenRC Service Integration

Service script installed at `/etc/init.d/mtk-wmt-loader`:

```sh
#!/sbin/openrc-run

name="MediaTek WMT Native Loader"
description="Loads MTK combo chip firmwares and initializes WiFi (Rust daemon)"
command="/usr/local/bin/mtk-wmt-loader"
command_background="yes"
pidfile="/run/${RC_SVCNAME}.pid"

depend() {
	need localmount
	before net
}

start_pre() {
	ebegin "Setting up MTK firmware symlinks"
	rm -rf /vendor/firmware /system/etc/firmware
	mkdir -p /vendor /system/etc
	ln -sT /lib/firmware /vendor/firmware
	ln -sT /lib/firmware /system/etc/firmware
	eend 0
}

start_post() {
	ebegin "Waiting for wlan0"
	for i in $(seq 1 15); do
		if ip link show wlan0 >/dev/null 2>&1; then
			eend 0
			return 0
		fi
		sleep 0.5
	done
	eend 1
	return 1
}
```

Enabled at boot runlevel via:
```sh
rc-update add mtk-wmt-loader boot
rc-update del wmt-pyloader boot
```
