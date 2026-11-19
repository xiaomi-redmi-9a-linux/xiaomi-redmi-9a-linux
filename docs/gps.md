# GPS / GNSS Subsystem & Investigation on Xiaomi Redmi 9A (Helio G25 / MT6765)

This document details the hardware architecture, kernel driver interfaces, diagnostic tests, and conclusions regarding the GPS / GNSS subsystem on the Xiaomi Redmi 9A (`dandelion`/`blossom`) running Alpine Linux.

---

## 1. Hardware Architecture

- **Connectivity SoC:** MediaTek MT6631 4-in-1 combo wireless chip (Wi-Fi 802.11n, Bluetooth 5.0, FM Radio, and GNSS).
- **Host Interface:** High-speed SDIO bus communicating via MediaTek's **STP (Serial Transport Protocol)** multiplexer layer.
- **GNSS Capabilities:** GPS, GLONASS, Galileo, BeiDou (B1) L1 receiver.
- **External LNA:** GPIO 91 on MT6765 controls the external GPS Low Noise Amplifier.
- **Power Regulators:** VCN28 LDO powers the analog RF and baseband circuitry of the MT6631 GNSS core in co-clock mode.

---

## 2. Kernel Drivers & Device Nodes

The MediaTek GPS subsystem is implemented by two kernel driver components in `drivers/misc/mediatek/connectivity/gps/`:

### 2.1 `/dev/stpgps` (Character Device: Major 191, Minor 0)
- **Source:** `stp_chrdev_gps.c`
- **Role:** Handles power lifecycle and data exchange for task index 2 (`GPS_TASK_INDX`).
- **Power Lifecycle:**
  - `open("/dev/stpgps")` calls `GPS_open()`, which invokes `mtk_wcn_wmt_func_on(WMTDRV_TYPE_GPS)` (`OPID(3) type(2)`), turns on the VCN28 LDO, asserts GPIO 91 (LNA), takes an internal kernel wake lock (`gps_wake_lock`), and registers `GPS_event_cb`.
  - `close(fd)` calls `GPS_close()`, turning off GPS (`OPID(4) type(2)`), disabling VCN28, releasing GPIO 91, and dropping the wake lock.
  - **The device node must remain held open by a persistent process to keep the hardware powered.**

### 2.2 `/dev/gps_emi` (Character Device: Major 238, Minor 0)
- **Source:** `gps_emi.c`
- **Role:** Configures shared EMI DRAM memory required by the GNSS DSP.
- **MPU Region:** Region 29 (`0xbf281000`, size `0x17e000`).
- **Initialization:** Invoking `ioctl(fd, 1, 0)` (`IOCTL_EMI_MEMORY_INIT`) configures the host EMI Memory Protection Unit (MPU) allowing MT6631 DMA access to the reserved buffer.

### 2.3 Supported Control Ioctls on `/dev/stpgps`
| Ioctl Code | Macro / Command | Description |
|:---|:---|:---|
| `7` | `COMBO_IOC_GPS_IC_HW_VERSION` | Returns IC hardware version (`0x00008a00` on MT6631). |
| `8` | `COMBO_IOC_GPS_IC_FW_VERSION` | Returns IC firmware version (`0x00008a00` on MT6631). |
| `10` | `COMBO_IOC_CO_CLOCK_FLAG` | Returns `1` (system operates in co-clock mode). |
| `16` | `COMBO_IOC_TAKE_GPS_WAKELOCK` | Explicitly holds kernel wake lock. |
| `17` | `COMBO_IOC_GIVE_GPS_WAKELOCK` | Releases kernel wake lock. |
| `18` | `COMBO_IOC_GPS_HW_SUSPEND` | Sends opcode 4 (`GPS_FWCTL_OPCODE_ENTER_STOP_MODE`) to the MT6631 MCU. |
| `19` | `COMBO_IOC_GPS_HW_RESUME` | Sends opcode 5 (`GPS_FWCTL_OPCODE_EXIT_STOP_MODE`) to the MT6631 MCU. Verified to acknowledge in ~2 ms (`rx0=0, rx1=5`). |
| `22` | `COMBO_IOC_GET_GPS_LNA_PIN` | Queries external LNA GPIO number (returns pin 91). |

---

## 3. Display DPMS & WMT Powerdown Interlock

During idle screen lock, X11 DPMS triggers a powerdown sequence on the framebuffer (`FB_BLANK_POWERDOWN`). 

In `wmt_dev.c`, `wmt_fb_notifier_callback` intercepts display blanking:
- When the screen blanks, WMT enters early powerdown (`OPID(4) type(9)`), putting SDIO links into deep sleep.
- To maintain background connectivity without dropping SDIO traffic, screen blanking can be managed, or WMT's `always_pwr_on_flag` must be configured.
- Unblanking the display (`DISPLAY=:0 XAUTHORITY=/home/user/.Xauthority xset dpms force on; xset -dpms`) restores WMT to full `UNBLANK` (`OPID(3) type(9) ok`).

---

## 4. Why Standard `gpsd` Cannot Function Alone

Standard Linux GPS stacks (like `gpsd` or GeoClue serial drivers) expect a GPS receiver that outputs standard ASCII **NMEA 0183** sentences (`$GPGGA`, `$GPRMC`, `$GPGSV`) over a serial TTY.

On the MediaTek MT6631 combo platform:
1. **Raw Character Device:** `/dev/stpgps` is an STP character device, not a TTY. It lacks standard `termios` ioctls (`TCGETS`/`TCSETS`).
2. **Baseband / Correlator Architecture:** The MT6631 GNSS core is a low-power RF baseband correlator. It does **not** solve navigation equations or generate NMEA sentences in hardware.
3. **Proprietary Userspace Engine (`mnld`):** In MediaTek's architecture, raw satellite tracking loops, pseudo-range measurements, Doppler calculations, ephemeris decoding, and PVT (position/velocity/time) fix calculations are performed on the host CPU by MediaTek's proprietary closed-source daemon: **`mnld`** (*MediaTek Navigation Location Daemon*).
4. **Command Testing Results:**
   - Standard ASCII `$PMTK` configuration commands (`$PMTK000`, `$PMTK605`, `$PMTK101`, `$PMTK253,0`, `$PMTK314`) and MTK binary frames (`0x04 0x24 ...`) written to `/dev/stpgps` produce zero RX output.
   - The hardware core remains completely silent until driven by `mnld`'s measurement processing loop and ephemeris tables.

---

## 5. Potential Future Paths

| Approach | Feasibility | Description |
|:---|:---|:---|
| **Stock `mnld` in Bionic Chroot / Container** | Feasible | Extract `/vendor/bin/mnld` and Android libraries (`libcutils.so`, `liblog.so`) from stock MIUI for `dandelion`. Run `mnld` in a minimal Android Bionic chroot on Alpine to generate standard NMEA for `gpsd`. |
| **Network Geolocation (GeoClue)** | Ready | GeoClue can provide approximate location using Wi-Fi BSSID mapping (via Mozilla Location Services or BeaconDB) and Cell Tower IDs via ModemManager. |
| **External USB / Bluetooth GNSS** | Plug-and-Play | Standalone USB GPS dongles or Bluetooth GPS receivers stream standard NMEA directly into `gpsd` without proprietary daemons. |
