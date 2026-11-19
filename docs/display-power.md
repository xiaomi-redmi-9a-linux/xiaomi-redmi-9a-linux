# Display & Power Management Architecture

## Display Hardware

* **Panel:** HD+ 720 x 1600 @ 60Hz (Novatek NT36572 / DJN LCM).
* **Driver:** MediaTek Framebuffer (`mtkfb`) + Primary Display engine (`videox/primary_display.c`).
* **Server:** Xorg X11 with `fbdev` driver and `dwm` (Dynamic Window Manager via Sxmo).

---

## Power Button & Sleep / Wake Handling

### 1. Framebuffer Blanking (`mtk-fb-blank`)
MediaTek's display engine responds to standard `FBIOBLANK` ioctls:
* `mtk-fb-blank off`: calls `ioctl(fd, FBIOBLANK, FB_BLANK_POWERDOWN)`.
* `mtk-fb-blank on`: calls `ioctl(fd, FBIOBLANK, FB_BLANK_UNBLANK)`.

When unblanked, the Novatek touch controller re-initializes and the display controller re-enables CMDQ execution.

### 2. Button Debounce & Cooldown Lockout
Sxmo uses `bonsaid` (Bonsai daemon) to handle hardware button transitions from evdev:
* **Hold Delay (`power_pressed`):** 400ms threshold for long-press actions (matches upstream Sxmo standard configuration).
* **Click Delay (`power_released`):** 400ms debounce delay for multi-click transitions.
* **Lockout Cooldown Guard:** [`sxmo_hook_inputhandler.sh`](../overlay/home/user/.config/sxmo/hooks/sxmo_hook_inputhandler.sh) implements a 1.0s timestamp lockout. Rapid mechanical bounces or trailing taps within 1000ms are safely ignored.
* **Unconditional Wakeup:** When the screen is locked or off, any power button click explicitly runs `sxmo_state.sh set unlock` instead of toggling, ensuring the screen turns on and stays on.

### 3. Kernel Suspend Stability & Sleep-of-Death Prevention
* In [`kernel/power/suspend.c`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/kernel/power/suspend.c):
  Set `mem_sleep_default = PM_SUSPEND_TO_IDLE;`. On MT6765, hardware `deep` suspend delegates powerdown to MediaTek's proprietary SPM/SSPM firmware which relies on Android-specific Netlink daemons. Without them, prolonged `deep` sleep cuts power to the AP and ignores power key interrupts (Sleep of Death).
* In [`overlay/etc/local.d/02-power.start`](../overlay/etc/local.d/02-power.start):
  Explicitly sets `echo s2idle > /sys/power/mem_sleep` and ensures Wi-Fi power saving is disabled (`iw wlan0 set power_save off`). Leaving Wi-Fi `power_save on` on MT6631 SDIO caused ~700ms ping latency, packet drops, and CONSYS clock verification failures (`0x18007000(0x50001)`) upon wake.
* In [`overlay/usr/bin/sxmo_deviceprofile_mediatek,MT6765.sh`](../overlay/usr/bin/sxmo_deviceprofile_mediatek,MT6765.sh):
  - `export SXMO_SUSPENDABLE_STATES="none"`: Disables calling `rtcwake -m mem` (`s2idle`) on screenoff. In native Linux without Android SPM daemons, kernel `s2idle` cycles cause display CMDQ hangs and CONNSYS clock crashes after ~5 minutes. Blanking via `mtk-fb-blank off` turns off LCM panel power and suspends touch (`nvt_ts_suspend`) while keeping the CPU responsive for instant (0.1s) wakeups with zero Sleep of Death.
  - `export SXMO_NO_MODEM="1"`: Prevents spurious "Modem crashed! 30s recovery" alerts and 30-second wakelock loops on wakeup until MTK CCCI cellular drivers are ported.
* In [`mtkfb.c`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/drivers/misc/mediatek/video/mt6765/videox/mtkfb.c):
  Set `fbi->skip_vt_switch = true;` and `pm_set_vt_switch(0);` before registering the framebuffer. This prevents Linux's VT console subsystem (`CONFIG_VT_CONSOLE_SLEEP`) from switching to `SUSPEND_CONSOLE` on suspend entry, which would otherwise unblank the screen, turn on backlight to 1023, and trigger `pri_disp_wakelock` abort loops.
* In [`primary_display.c`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/drivers/misc/mediatek/video/mt6765/videox/primary_display.c):
  Gated `primary_display_esd_check_enable(0)` during suspend to prevent asynchronous ESD check CMDQ packets to sleeping LCM.
* In [`mtk-pmic-keys.c`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/drivers/input/keyboard/mtk-pmic-keys.c):
  Enabled `device_init_wakeup(&pdev->dev, true)` for reliable wakeup from MT6357 PMIC.

---

## Fuel Gauge & Battery State-of-Charge (SoC) Architecture

MediaTek's vendor driver historically relied on an Android daemon (`fuelgauged`) to compute SoC. In native Linux:
1. **$I \times R$ Internal Resistance Compensation:** [`drivers/power/supply/mtk_battery.c`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/drivers/power/supply/mtk_battery.c) calculates true open-circuit cell voltage ($V_{\text{ocv}} = V_{\text{term}} - I_{\text{batt}} \times R_{\text{int}}$) using $R_{\text{int}} \approx 150\text{ m}\Omega$ from real-time current sensing (`GAUGE_PROP_BATTERY_CURRENT`). This eliminates the 10–15% battery drops under CPU load and artificial jumps when charging.
2. **IIR Filter & Monotonic Rate Limiting:** Smooths OCV fluctuations and restricts capacity changes to 1% per 15s (or 3s if gap > 10%), ensuring monotonic progression (charging only steps up, discharging only steps down).
3. **Hardware PMIC RTC Restore:** On boot, the driver reads `GAUGE_PROP_RTC_UI_SOC` from MT6357 PMIC hardware registers so the battery percentage displays accurately immediately upon boot without any startup dip or crawling.
4. **Low Battery Fast-Track:** Bypasses rate-limiting below $3450\text{ mV}$ for emergency low-battery shutdown safety.

---

## Autosuspend & Idle Power Management

### 1. Automatic Screen Off (`idle_locker`)
The `idle_locker` background job runs under Sxmo:
```sh
sxmo_idle.sh -w timeout "${SXMO_UNLOCK_IDLE_TIME:-15}" "sxmo_state.sh idle"
```
After 15 seconds (configured via `SXMO_UNLOCK_IDLE_TIME=15` in the hardware profile) of user touch/button inactivity:
1. `sxmo_state.sh idle` transitions system state from `unlock` to `screenoff`.
2. `sxmo_hook_screenoff.sh` calls `mtk-fb-blank off` to turn off LCM power and touch controller.
3. MediaTek's display driver releases the kernel-level `pri_disp_wakelock`.

### 2. State-Based Power Management (`SXMO_SUSPENDABLE_STATES`)
Configured in [`overlay/usr/bin/sxmo_deviceprofile_mediatek,MT6765.sh`](../overlay/usr/bin/sxmo_deviceprofile_mediatek,MT6765.sh):
```sh
export SXMO_SUSPENDABLE_STATES="none"
```
* On MT6765 native Linux, calling `rtcwake -m mem` causes display CMDQ desync and CONSYS clock check failures on wake from deep sleep. Setting `SXMO_SUSPENDABLE_STATES="none"` ensures Sxmo holds `sxmo_not_suspendable`, keeping the kernel out of `s2idle` while still blanking the display and sleeping touch controller.
* Power consumption in screen-off state drops significantly through display powerdown, touch sleep, and CPU frequency scaling down to idle governor minimums, avoiding Sleep of Death entirely.

---

## Battery UI & D-Bus Launch Helper Recovery

### 1. D-Bus Service Activation Fix
On Alpine Linux, the system D-Bus daemon runs under the unprivileged `messagebus` account. Root-level on-demand service activation relies on `/usr/libexec/dbus-daemon-launch-helper`.

If permissions are set to `root:root` mode `0750`, `dbus-daemon` cannot execute the helper, triggering:
```
Failed to execute program org.freedesktop.UPower: Permission denied
```
Fixed by assigning group ownership and setuid permissions:
```sh
chown root:messagebus /usr/libexec/dbus-daemon-launch-helper
chmod 4750 /usr/libexec/dbus-daemon-launch-helper
```
With this fix, `upowerd` and `polkitd` activate cleanly on demand.

### 2. Status Bar Battery Percentage & UPower Monitoring
* **Battery Monitor Daemon:** `sxmo_battery_monitor` runs under `superd` executing `upower -m`, streaming kernel power supply uevents into `sxmo_hook_battery.sh`.
* **Percentage Configuration:** In [`overlay/etc/environment`](../overlay/etc/environment) and [`overlay/usr/share/sxmo/default_hooks/sxmo_hook_statusbar.sh`](../overlay/usr/share/sxmo/default_hooks/sxmo_hook_statusbar.sh), `SXMO_BAR_SHOW_BAT_PER=1` is enabled by default.
* **Status Bar Output:** Sxmo renders both the dynamic charging/discharging glyph and current percentage (e.g. `󰂉 66%`). Periodic statusbar refreshes ensure accurate display across suspend and unlock transitions.
