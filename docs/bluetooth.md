# Bluetooth Subsystem on MediaTek MT6765 (Helio G25) / MT6631

## Overview

The Xiaomi Redmi 9A (`dandelion`/`blossom`) uses a MediaTek MT6631 4-in-1 combo connectivity chip (Wi-Fi, Bluetooth, GPS, FM Radio) interfaced over SDIO and managed by the MediaTek Wireless Management Task (WMT).

Unlike standalone USB or UART dongles, MediaTek routes Bluetooth through its Serial Transport Protocol (STP) multiplexer via the character device `/dev/stpbt` (major 192, minor 0).

Under standard Linux distributions, BlueZ requires a kernel netdev interface (`hciX`). This is accomplished cleanly using Linux's Virtual HCI driver (`/dev/vhci`) paired with the native Rust daemon **`mtk-bt-bridge`**.

---

## Architecture

```
                  +-------------------------+
                  |  BlueZ Daemon & Tools   |
                  |  bluetoothd / bluetoothctl
                  +------------+------------+
                               | (AF_BLUETOOTH / HCI Socket)
                  +------------v------------+
                  |  Linux Bluetooth Core   |
                  |    net/bluetooth/hci    |
                  +------------+------------+
                               |
                  +------------v------------+
                  |  Kernel VHCI Subsystem  |
                  |     /dev/vhci (hci0)    |
                  +------------+------------+
                               |
                   (Bidirectional Packet Pump
                     & Transparent Quirks)
                               |
                  +------------v------------+
                  |      mtk-bt-bridge      |
                  |  (Native Rust, musl ELF)|
                  +------------+------------+
                               |
                  +------------v------------+
                  |   MTK STP Bluetooth Dev |
                  |       /dev/stpbt        |
                  +------------+------------+
                               | (SDIO Bus)
                  +------------v------------+
                  |  MT6631 Combo Hardware  |
                  |   Bluetooth 5.0 Radio   |
                  +-------------------------+
```

---

## Hardware Lifecycle Management

Opening `/dev/stpbt` triggers `BT_open()` in `drivers/misc/mediatek/connectivity/bt/mt66xx/legacy/stp_chrdev_bt.c`:
1. Issues WMT function control: `OPID(3) type(0)` to power on MT6631 Bluetooth hardware over SDIO.
2. Registers Bluetooth power management notifier (`conn2ap_btif0_wakeup_out_b` EIRQ).
3. Connects the STP Bluetooth channel.

Closing `/dev/stpbt` triggers `BT_close()`:
1. Issues WMT function control: `OPID(4) type(0)` to turn off MT6631 Bluetooth hardware.
2. Unregisters PM notifier and powers down the radio to save standby power.

---

## Transparent Quirk Handling

During Linux kernel HCI device probe (`hci_core.c:829`), standard Linux initialization queries Extended Features Page 2 (`0x1004` page 2).
MT6631 returns `0x05 0x03...`, advertising bit 2 (`0x04`: `LMP_SYNC_TRAIN`).

Because `LMP_SYNC_TRAIN` was reported, the Linux kernel attempted to send `HCI_OP_READ_SYNC_TRAIN_PARAMS` (`0x0c77`).
MT6631 firmware rejects this command with `Unknown HCI Command` (`0x01`), which causes `__hci_init(hdev)` to abort and prevents `hci0` from opening.

`mtk-bt-bridge` implements two transparent hardware fixes:
1. **Extended Features Masking:** When MT6631 returns Page 2 Extended Features, bit 2 is masked off (`features[0] &= ~0x04`). The Linux kernel cleanly skips `HCI_OP_READ_SYNC_TRAIN_PARAMS`.
2. **Fallback Trap:** If `0x0c77` is ever requested, the bridge synthesizes an immediate valid `HCI_EV_CMD_COMPLETE` response.

---

## Daemon Specifications & Performance

* **Location:** `/usr/local/bin/mtk-bt-bridge`
* **Binary Format:** 64-bit ELF static ARM64 musl (`aarch64-unknown-linux-musl`), stripped
* **Binary Size:** **334 KB**
* **RAM Consumption:** **284 kB resident memory (VmRSS)**, 520 kB virtual size
* **CPU Consumption:** **0.0%** (blocking `libc::poll(..., -1)` loop with signal self-pipe; zero periodic timer wakeups)
* **OpenRC Service:** `/etc/init.d/mtk-bt-bridge` (runlevel `default`)
  * `need mtk-wmt-loader`
  * `before bluetooth`

---

## Verification & Usage

Check controller status:
```bash
bluetoothctl show
```
Expected output:
```
Controller 00:00:46:76:63:01 (public)
	Manufacturer: 0x0046 (MediaTek)
	Version: 0x0b (Bluetooth 5.0)
	Name: BlueZ 5.86
	Powered: yes
	Roles: central, peripheral
	SupportedSecondaryChannels: 1M, 2M, Coded
```

Scan for nearby Bluetooth devices:
```bash
bluetoothctl scan on
```

List discovered devices:
```bash
bluetoothctl devices
```

---

## Power Management & Standby Optimization

1. **Boot Default (`AutoEnable=false`):**
   * Configured in `/etc/bluetooth/main.conf`.
   * Bluetooth starts powered down on boot, saving ~66 mA continuously until turned on via the UI toggle.

2. **Smart Screen-Off Sleep:**
   * Handled by [`sxmo_hook_screenoff.sh`](file:///media/primaryHdd/Coding/Android/xiaomi-redmi-9a-linux/overlay/home/user/.config/sxmo/hooks/sxmo_hook_screenoff.sh) and [`sxmo_hook_unlock.sh`](file:///media/primaryHdd/Coding/Android/xiaomi-redmi-9a-linux/overlay/home/user/.config/sxmo/hooks/sxmo_hook_unlock.sh).
   * When the screen locks or times out:
     * If no devices are connected, Bluetooth powers off automatically (saving 66 mA in standby).
     * If headphones/speakers/accessories are active, Bluetooth stays ON to maintain uninterrupted audio.
   * On screen unlock/wake, Bluetooth power is automatically restored if it was auto-slept.
