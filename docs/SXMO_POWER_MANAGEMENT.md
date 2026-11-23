# Sxmo Mobile Desktop & Power Management on Xiaomi Redmi 9A (blossom / MT6765)

This guide documents the Sxmo configuration, hook management, power management architecture, audio rules, and battery management on the Xiaomi Redmi 9A running Alpine Linux.

---

## 1. Power Management Architecture: Runtime Scaling vs. `s2idle`

### Why Deep System Suspend (`s2idle`) Fails on MT6765
The MediaTek Helio G25 (MT6765) System-on-Chip relies on a proprietary System Power Manager (SPM) firmware co-processor running in secure ARM Trusted Firmware (ATF / EL3). During deep suspend-to-idle (`s2idle` / `echo freeze > /sys/power/state`):
1. **The SPM PCM Hardware Watchdog:** ATF arms a 30-second SPM PCM hardware watchdog (`SPM_ARGS_PCM_WDT`) before CPU Wait-For-Interrupt (WFI). If sleep exceeds 30 seconds, the hardware watchdog bites and halts the SoC.
2. **Proprietary Power State Dependencies:** MT6765 low-power sleep modes assume Android's WakeLock / Power HAL infrastructure. In native Linux, unhandled clock domain and bus transitions cause unrecoverable hardware lockups ("Sleep of Death") upon deep sleep transitions.

### The Solution: Active Runtime Power Profile on Efficiency Cluster 1
Rather than putting the SoC into a fatal deep suspend, the Redmi 9A port utilizes **active runtime power management**:
- **Efficiency Core Standby (`cpu4`):** The MT6765 contains two clusters: Cluster 0 (`cpu0-3`, Performance, 900 MHz – 2.0 GHz) and Cluster 1 (`cpu4-7`, Efficiency, 400 MHz – 1.5 GHz). During screenoff, execution shifts entirely to **`cpu4`**, while the entire high-power Cluster 0 (cores 0–3) and remaining efficiency cores (5–7) are powered down.
- **Clock & Voltage Clamping (400 MHz):** The single active efficiency core (`cpu4`) is locked to the minimum factory-calibrated clock frequency (**400 MHz**) using the `powersave` governor.
- **Subsystem Sleep:** The display panel is unblanked/powered down, touchscreen interrupts are suspended, Wi-Fi 802.11 power saving is asserted, and disconnected Bluetooth radios are powered down.
- **Instant Wakeup:** The moment you press the Power button, all 7 offline cores are instantly brought back online and the governor is restored to `schedutil` across all 8 cores in less than 50 milliseconds, providing a fluid 8-core desktop experience with zero lag.

---

## 2. Power Profile Scripts & Hook Integration

### A. Power Profile Daemon: `/usr/bin/blossom_power_profile.sh`
Managed via `doas` permissions in `/etc/doas.d/blossom_power.conf`:
```sh
#!/bin/sh
# Xiaomi Redmi 9A (blossom / MT6765) runtime power profile

case "$1" in
    screenoff|powersave)
        # Drop CPU governor to powersave (clamps to 900MHz minimum clock)
        for g in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
            [ -w "$g" ] && echo powersave > "$g" 2>/dev/null
        done
        # Power down BIG cluster (cpu4-7) and 3 LITTLE cores (cpu1-3)
        for c in 1 2 3 4 5 6 7; do
            [ -w "/sys/devices/system/cpu/cpu$c/online" ] && echo 0 > "/sys/devices/system/cpu/cpu$c/online" 2>/dev/null
        done
        # Ensure Wi-Fi 802.11 power saving is enabled
        iw dev wlan0 set power_save on 2>/dev/null || true
        ;;
    unlock|performance|normal)
        # Bring all cores (cpu1-7) back online
        for c in 1 2 3 4 5 6 7; do
            [ -w "/sys/devices/system/cpu/cpu$c/online" ] && echo 1 > "/sys/devices/system/cpu/cpu$c/online" 2>/dev/null
        done
        # Restore CPU governor to schedutil for snappy UI responsiveness
        for g in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
            [ -w "$g" ] && echo schedutil > "$g" 2>/dev/null
        done
        ;;
esac
```

### B. Screenoff Hook (`sxmo_hook_screenoff.sh`)
* **Header:** `# configversion: 66b67db06146e3b2dd3ae9a6f1411c5e`
* **Actions:**
  - Invokes `doas /usr/bin/blossom_power_profile.sh screenoff`
  - Turns display off (`sxmo_wm.sh display off`) and blanks `/dev/fb0`
  - Disables touchscreen input events (`sxmo_wm.sh inputevent touchscreen off`)
  - **No Unnecessary Timer Loops:** Removed the default upstream 2-second periodic notification LED breathing loop (Redmi 9A hardware has no notification LEDs), keeping the CPU in pure low-power WFI idle.
  - **Bluetooth Smart Idle:** If Bluetooth is enabled but no devices are connected, the controller is powered down, saving **~66 mA continuous current**.

### C. Unlock Hook (`sxmo_hook_unlock.sh`)
* **Header:** `# configversion: 5697e0579ea20a4a6b8dc05f5814d0c6`
* **Actions:**
  - Invokes `doas /usr/bin/blossom_power_profile.sh unlock` to instantly restore 8 cores and `schedutil`
  - Powers display on (`sxmo_wm.sh display on`)
  - Enables touchscreen input events
  - Restores Bluetooth power if it was auto-slept by the screenoff hook

### D. Safe Suspend No-Op: `/usr/bin/sxmo_suspend.sh`
To prevent userspace daemons (like `sxmo_autosuspend.sh`) from invoking fatal `s2idle` states, `/usr/bin/sxmo_suspend.sh` is configured to safely exit 0.

---

## 3. Sxmo Configuration Migration System

Sxmo tracks customized user scripts located in `/home/user/.config/sxmo/hooks/` against upstream distribution defaults.

### The `configversion` Mechanism
Every hook in `/home/user/.config/sxmo/hooks/` must contain a valid `configversion` hash header on line 2 matching the system reference in `/usr/share/sxmo/default_hooks/`.

If this line is missing or has a differing hash:
1. Sxmo renames the hook to `<hook_name>.needs-migration`.
2. Sxmo displays a migration notification banner on screen.
3. `sxmo_migrate.sh state` returns an error code indicating files need attention.

### Resolving Migration Notifications
To verify whether any configs are pending migration:
```bash
su - user -c "sxmo_migrate.sh state"
```
If files are flagged, inspect the version hashes from the system default:
```bash
head -n 2 /usr/share/sxmo/default_hooks/<hook_name>
```
Ensure line 2 contains the proper `# configversion: <hash>` header, then remove the `.needs-migration` suffix and make the file executable (`chmod +x`).

---

## 4. Audio Clean-up Udev Rules & Device Profile

### A. Udev Rules
The stock MediaTek kernel creates numerous dummy ALSA PCM devices for proprietary modem/DSP routing (`pcmC1D2` through `pcmC1D3*`). These confuse standard ALSA, PipeWire, and WirePlumber audio servers.

File: [`linux/overlay/etc/udev/rules.d/89-mtk-audio.rules`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/linux/overlay/etc/udev/rules.d/89-mtk-audio.rules):
```udev
# Clean up non-functional MediaTek dummy and baseband DSP nodes
SUBSYSTEM=="sound", KERNEL=="pcmC1D[2-9]*", RUN+="/bin/rm -f /dev/snd/%k"
SUBSYSTEM=="sound", KERNEL=="pcmC1D1[0-9]*", RUN+="/bin/rm -f /dev/snd/%k"
SUBSYSTEM=="sound", KERNEL=="pcmC1D2[0-9]*", RUN+="/bin/rm -f /dev/snd/%k"
SUBSYSTEM=="sound", KERNEL=="pcmC1D3[0-9]*", RUN+="/bin/rm -f /dev/snd/%k"
```
This isolates the primary functional DL2 stereo DAC node for clean speaker and headphone output.

### B. Device Profile Audio Enablement
File: [`linux/overlay/usr/bin/sxmo_deviceprofile_mediatek,MT6765.sh`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/linux/overlay/usr/bin/sxmo_deviceprofile_mediatek,MT6765.sh):
- Native audio is enabled by ensuring `SXMO_NO_AUDIO` is unset.
- Enables the volume statusbar component (`set_volume` in `sxmo_hook_statusbar.sh`) displaying live speaker and microphone states.
- Connects hardware volume buttons (`mtk-kpd` keys `KEY_VOLUMEUP` and `KEY_VOLUMEDOWN`) via `bonsaid` and `sxmo_hook_inputhandler.sh` to PipeWire's `pactl` volume control.

---

## 5. PMIC Fuelgauge & Battery Management

The MT6765 port uses an in-kernel autonomous fuelgauge algorithm for the MT6357 PMIC, removing dependency on Android's userspace `healthd` / `android.hardware.health` service:
- **Real-Time OCV Mapping:** Maps open-circuit battery voltage to accurate State of Charge (SoC) for the Redmi 9A BN56 5000 mAh Li-Po battery.
- **Internal Resistance (IR) Compensation:** Uses real-time hardware current sensing (`GAUGE_PROP_BATTERY_CURRENT`) to dynamically offset voltage sags caused by the ~150 mΩ internal resistance under CPU/GPU load (`delta_V = I * R_int`).
- **Low-Pass IIR Filtering:** Eliminates capacity jitter while ensuring fast recovery on sleep wakeups and immediate poweroff protection below 3450 mV.
- **PMIC RTC Persistence:** Preserves last-known UI SoC across reboots and deep sleep states in hardware scratchpad registers (`GAUGE_PROP_RTC_UI_SOC`).
