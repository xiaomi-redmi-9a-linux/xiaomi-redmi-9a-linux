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
| **Audio (Speaker)** | ✅ Working | MediaTek DL2 ALSA routing + PipeWire / WirePlumber |
| **Wi-Fi** | ✅ Working | MediaTek WMT SDIO Wi-Fi (`wlan0`) |
| **Bluetooth** | ⚠️ Partial | BlueZ active; firmware loader configured |
| **Battery & Charging** | ✅ Working | Accurate OCV voltage/capacity reporting, fast charging active |
| **USB Networking** | ✅ Working | USB Gadget Ethernet (RNDIS / CDC-NCM / ACM serial console) |
| **Power Management** | ✅ Working | Debounced screen sleep/wake, 1.0s cooldown guard |
| **Internal Storage** | 🛡️ Protected | Never touched; system runs entirely from MicroSD (`/dev/mmcblk1p1`) |

---

## 📦 Repositories

* **Kernel Source:** [`kernel_xiaomi_blossom_linux`](https://github.com/henry-richard7/kernel_xiaomi_blossom_linux.git) (branch `recommended-picks`)
* **Userspace & Installer:** [`xiaomi-redmi-9a-linux`](https://github.com/henry-richard7/xiaomi-redmi-9a-linux.git) (this repository)

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
3. Install the device overlay (Sxmo profile, audio routing, tinydm auto-login, and input hooks).

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

## ⚙️ Key Technical Features

### 1. Framebuffer Blanking Utility (`mtk-fb-blank`)
Because standard Xorg DPMS calls do not directly control MediaTek's proprietary panel power sequences, we provide a lightweight static helper [`src/mtk-fb-blank.c`](src/mtk-fb-blank.c) that interfaces with the kernel `FBIOBLANK` ioctl interface to cleanly power down the display and Novatek touch controller during sleep.

### 2. Button Debounce & Cooldown Lockout
To eliminate key bounces and sleep/wake flapping:
* [`overlay/home/user/.config/sxmo/bonsai_tree.json`](overlay/home/user/.config/sxmo/bonsai_tree.json): Configured with a snappy 200ms debounce delay and an 800ms long-press hold delay.
* [`overlay/home/user/.config/sxmo/hooks/sxmo_hook_inputhandler.sh`](overlay/home/user/.config/sxmo/hooks/sxmo_hook_inputhandler.sh): Implements a 1.0-second timestamp lockout guard and unconditional wake logic so screen waking is instantaneous and stable.

### 3. Audio ALSA Routing
ALSA DL2 DMA playback is routed to the internal speaker using dedicated startup scripts:
* Detailed documentation: [`docs/audio.md`](docs/audio.md).
* Sleep/Wake documentation: [`docs/display-power.md`](docs/display-power.md).

---

## 📜 License

* Kernel: [GPL-2.0](https://www.gnu.org/licenses/old-licenses/gpl-2.0.html)
* Sxmo configs & scripts: [AGPL-3.0](https://www.gnu.org/licenses/agpl-3.0.html) / MIT
