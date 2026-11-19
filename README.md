# Alpine Linux on Xiaomi Redmi 9A (`dandelion` / `blossom`)

Complete native Linux port for the **Xiaomi Redmi 9A** (MediaTek Helio G25 / MT6765) running **Alpine Linux** with **Sxmo** (Simple X Mobile on `dwm`).

Boots directly from an external MicroSD card via native Android fastboot. **Internal eMMC storage (`/dev/mmcblk0*`) remains 100% untouched and safe.**

---

## 📱 Hardware Support Matrix

| Hardware Component | Status | Details |
| :--- | :---: | :--- |
| **SoC / CPU** | ✅ Working | MediaTek Helio G25 (8x Cortex-A53 up to 2.0 GHz) |
| **Display** | ✅ Working | HD+ 720x1600 @ 60Hz, 32bpp framebuffer (`mtkfb`) + X11 |
| **Touchscreen** | ✅ Working | Novatek NT36572 / DJN multi-touch capacitive panel |
| **Backlight Control** | ✅ Working | PWM brightness (`/sys/class/leds/lcd-backlight/brightness`) |
| **Hardware Keys** | ✅ Working | PMIC Power button + Volume Up/Down with Bonsai debounce |
| **Audio (Speaker & Jack)**| ✅ Working | Built-in speaker + 3.5mm jack auto-detection daemon (`sxmo_jackmon`) |
| **Music Player** | ✅ Working | Amberol touch music player (`amberol`), MPRIS lockscreen media controls (`playerctl`), PipeWire S16LE |
| **FM Radio** | ✅ Working | Built-in MT6631 FM receiver, loudspeaker & headphone routing, touch GUI app (`fm_radio_gui.py`), seek/scan |
| **Sensors** | ✅ Working | Native Rust proxy (`mtk-sensor-proxy`), FreeDesktop `net.hadess.SensorProxy`, Accelerometer (orientation/tilt), Ambient Light (lux), Proximity (near/far) |
| **Wi-Fi & Networking** | ✅ Working | MediaTek WMT SDIO Wi-Fi (`wlan0`), live signal bars (`nmcli` bridge), static IP (`192.168.1.50`) |
| **Bluetooth** | ⚠️ Partial | BlueZ active; firmware loader configured |
| **Battery & Charging** | ✅ Working | True OCV fuelgauge, monotonic rate limiting, status bar glyph (`󰂉`) & percentage |
| **USB Networking** | ✅ Working | USB Gadget Ethernet (RNDIS / CDC-NCM / ACM serial console) |
| **Power Management** | ✅ Working | Sleep-of-death fix (rtcwake autosuspend disabled), 2-min idle locker, `mtk-fb-blank` 0W display powerdown, debounced power key |
| **Clock & Timezone** | ✅ Working | Asia/Kolkata (IST) with 12-hour AM/PM status bar & lockscreen widget |
| **Internal Storage** | 🛡️ Protected | Never touched; system runs entirely from MicroSD (`/dev/mmcblk1p1`) |

---

## 📦 Repositories

* **Kernel Source:** [`kernel_xiaomi_blossom_linux`](https://github.com/henry-richard7/kernel_xiaomi_blossom_linux.git) (branch `recommended-picks`)
* **Userspace & Installer:** [`xiaomi-redmi-9a-linux`](https://github.com/henry-richard7/xiaomi-redmi-9a-linux.git) (this repository)

---

## 📱 Sxmo User Interface & Navigation Guide

Sxmo (Simple X Mobile) runs on a customized build of **`dwm` (Dynamic Window Manager)** optimized for touchscreens and mobile physical buttons.

### 1. Interface Layout Overview

```
+-----------------------------------------------------------+
| [3] [M]                 󰥍  󰤨      10:45 PM              |  <- Top Status Bar
+-----------------------------------------------------------+
|                                                           |
|                                                           |
|                         10:45 PM                          |  <- Center Conky Clock
|                     Mon 16 Nov 2026                       |
|                                                           |
|                                                           |
|                                                           |
|                                                           |
| [sxmo]                                                    |
+-----------------------------------------------------------+
```

* **Top Status Bar:**
  * **Tag / Workspace Number (`[3]`):** Current active desktop tag.
  * **Window Layout Mode (`[M]` / `[]=`):** Monocle (fullscreen) or Master-Stack tiling.
  * **Cellular / Modem (`󰥍`):** Shows SIM registration or red indicator if no SIM is inserted.
  * **Wi-Fi Indicator (`󰤨`):** Connected state with dynamic signal bars (0–100% quality based on real-time link RSSI).
  * **Headphone Jack Indicator (`󰍬` / ``):** Appears automatically when 3.5mm headphones or headsets are plugged in.
  * **Audio Volume (``):** Current volume level.
  * **Battery & Power (``):** Battery level circle and charging state.
  * **Clock (`10:45 PM`):** 12-hour local time with AM/PM indicator.
* **Center Desktop Widget:**
  * Rendered by `conky` with bold 50pt 12-hour time, AM/PM, and full date.

---

### 2. Hardware Button Controls

Physical buttons on the Redmi 9A are managed by the Bonsai daemon (`bonsaid`) with custom debounce timings:

| Button | Action | Behavior |
| :--- | :--- | :--- |
| **Power Button** | **Short Tap** | **Screen Lock / Sleep:** If awake, blanks the screen and locks input. If asleep, instantaneously wakes the screen (`sxmo_state.sh set unlock`). Protected by a 1.0s cooldown guard to eliminate key bounce. |
| **Power Button** | **Long Press (>800ms)** | **Power Menu:** Opens system power options (`Lock`, `Screen Off`, `Suspend`, `Reboot`, `Power Off`). |
| **Volume Up** | **Single Tap** | Increases audio volume by 5% (capped at 100% to prevent overflow). Triggers visual `xob` overlay. |
| **Volume Down** | **Single Tap** | Decreases audio volume by 5% (bounded at 0%). Triggers visual `xob` overlay. |

> [!NOTE]
> The **on-screen volume overlay (`xob`)** is configured at `y = 15%`, positioning it cleanly below the Redmi 9A's teardrop camera notch so volume levels are never obscured.

---

### 3. Touchscreen Gestures & Navigation

* **Open Main Application Menu (`dmenu`):**
  * **Tap anywhere on the Top Status Bar**, or swipe down from the top edge.
  * The Sxmo application menu will glide down, offering access to **Applications**, **Settings**, **Audio**, **Networks**, and **System Tools**.
* **Scrolling & Selecting in Menus:**
  * **Scroll:** Swipe vertically up or down through list items.
  * **Select / Launch:** Tap the desired menu entry.
  * **Dismiss / Back:** Tap outside the menu area or swipe right to return to the previous screen.
* **On-Screen Keyboard (`svkbd`):**
  * The `svkbd-mobile-intl` on-screen keyboard automatically summons when focusing text input fields.
  * To toggle manually: Open **Menu -> Toggle Keyboard**.
  * Use the bottom pad margin (configured at 24px) for comfortable thumbs typing.

---

### 4. Audio Routing & Switching (Headphones vs Speaker)

The MT6357 PMIC audio hardware features automatic jack sensing and an interactive routing menu:

* **Automatic Switching (`sxmo_jackmon`):**
  * **Plugging in 3.5mm headphones:** Immediately silences the internal speaker, enables headphone DAC charge-pumps, and shows an on-screen notification (` Audio Output: Switched to Headphones`).
  * **Unplugging headphones:** Reverts audio to the internal speaker (` Audio Output: Switched to Built-in Speaker`).
* **Manual Routing Menu:**
  * Open **Menu -> Audio -> Output Route**.
  * Select between **Headphones**, **Speaker**, or **Both (simultaneous playback)**.
  * The active route is marked with a checkmark (``).

---

### 5. Connecting to Wi-Fi

Wi-Fi is powered by the lightweight `nl80211` netlink bridge:

* **Checking Status:** The status bar Wi-Fi icon dynamically reflects connection state (`󰤮` disconnected, `󰤨` connected with full signal bars).
* **Adding a New Network:**
  ```bash
  # Generate network configuration
  wpa_passphrase "Your_SSID" "Your_Password" | grep -v '^[[:space:]]*#psk=' >> /etc/wpa_supplicant/wpa_supplicant.conf

  # Reload connection
  wpa_cli -i wlan0 reconfigure
  ```
* Once connected, DHCP assigns an IP via `udhcpc` and the status bar updates immediately.

---

## 🚀 Quick Start Guide

### Prerequisites
1. **Xiaomi Redmi 9A** with unlocked bootloader.
2. **MicroSD Card** (8GB or larger, Class 10 / A1 recommended).
3. **Linux Host PC** with `fastboot`, `parted`, `e2fsprogs`, and an SD card reader.

---

### Step 1: Prepare the MicroSD Card

Connect your MicroSD card to your PC, identify its device node (e.g. `/dev/sdb`), and run the automated installer:

```bash
git clone https://github.com/henry-richard7/xiaomi-redmi-9a-linux.git
cd xiaomi-redmi-9a-linux

# Run installer (replace /dev/sdb with your actual SD card device!)
sudo ./scripts/install-sd.sh /dev/sdb
```

The script will:
1. Format partition 1 as `ext4` labeled `linux-rootfs`.
2. Download and extract the latest Alpine Linux ARM64 rootfs.
3. Install the device overlay (Sxmo profile, audio routing, tinydm auto-login, `nmcli` bridge, and input hooks).

---

### Step 2: Get or Build the Kernel

Download the prebuilt `boot-linux.img` from [Releases](https://github.com/henry-richard7/kernel_xiaomi_blossom_linux/releases), or compile it yourself from source:

```bash
git clone -b recommended-picks https://github.com/henry-richard7/kernel_xiaomi_blossom_linux.git
cd kernel_xiaomi_blossom_linux

# Compile kernel and package boot-linux.img
./build.sh kernel
./build.sh boot
```

The generated boot image will be located at `out/boot-linux.img`.

---

### Step 3: Boot into Linux

1. Insert the prepared MicroSD card into your phone.
2. Power off the phone, then hold **Volume Down + Power** to enter **Fastboot Mode**.
3. Connect the phone to your PC via USB and flash the boot image:
   ```bash
   fastboot flash boot boot-linux.img
   fastboot reboot
   ```
4. Within ~15 seconds, the phone will display the native Linux console, initialize hardware, and launch the **Sxmo mobile desktop**.

---

## ⚙️ Detailed Technical Documentation

For in-depth hardware architecture and driver implementation details, refer to the technical documents in [`docs/`](docs/):

* **[Audio Subsystem & Jack Detection](docs/audio.md):** MT6357 PMIC DAC switching, WirePlumber S16LE format locking, ALSA DL2 DMA routing, and `sxmo_jackmon` daemon.
* **[Display & Power Management](docs/display-power.md):** MediaTek framebuffer blanking (`mtk-fb-blank`), Bonsai button debounce, and kernel LCM suspend handling.
* **[Wi-Fi & Network Tracking](docs/network.md):** `nl80211` netlink `nmcli` bridge, RSSI signal quality math, and `wpa_supplicant` management.
* **[Timezone & 12-Hour Clock Architecture](docs/time-clock.md):** Asia/Kolkata timezone configuration, `sxmobar` periodic runner, and centered Conky desktop clock.

---

## 📜 License

* Kernel: [GPL-2.0](https://www.gnu.org/licenses/old-licenses/gpl-2.0.html)
* Sxmo configs & scripts: [AGPL-3.0](https://www.gnu.org/licenses/agpl-3.0.html) / MIT
