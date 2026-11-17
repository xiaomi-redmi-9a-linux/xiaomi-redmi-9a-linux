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

### 3. Kernel Suspend Stability
* In [`mtkfb.c`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/drivers/misc/mediatek/video/mt6765/videox/mtkfb.c):
  Set `fbi->skip_vt_switch = true;` and `pm_set_vt_switch(0);` before registering the framebuffer. This prevents Linux's VT console subsystem (`CONFIG_VT_CONSOLE_SLEEP`) from switching to `SUSPEND_CONSOLE` on suspend entry, which would otherwise unblank the screen, turn on backlight to 1023, and trigger `pri_disp_wakelock` abort loops.
* In [`primary_display.c`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/drivers/misc/mediatek/video/mt6765/videox/primary_display.c):
  Gated `primary_display_esd_check_enable(0)` during suspend to prevent asynchronous ESD check CMDQ packets to sleeping LCM.
* In [`mtk-pmic-keys.c`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/drivers/input/keyboard/mtk-pmic-keys.c):
  Enabled `device_init_wakeup(&pdev->dev, true)` for reliable wakeup from MT6357 PMIC.

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

### 2. State-Based Autosuspend (`SXMO_SUSPENDABLE_STATES`)
Configured in [`overlay/usr/bin/sxmo_deviceprofile_mediatek,MT6765.sh`](../overlay/usr/bin/sxmo_deviceprofile_mediatek,MT6765.sh):
```sh
export SXMO_SUSPENDABLE_STATES="screenoff 5"
```
* When entering `screenoff`, Sxmo starts `idle_suspender` with a 5-second cooldown timer.
* After 5 seconds, `sxmo_wakelock.sh unlock sxmo_not_suspendable` releases the userspace inhibit lock.

### 3. Deep Sleep Daemon (`sxmo_autosuspend`)
The `sxmo_autosuspend` service runs continuously under `superd`:
1. Reads `/sys/power/wakeup_count` (which blocks until all kernel and userspace wakelocks are released).
2. Executes [`sxmo_hook_block_suspend.sh`](../overlay/usr/share/sxmo/default_hooks/sxmo_hook_block_suspend.sh) to check for active blockers:
   * **Active SSH Session:** Detects connections on `:22` and delays suspend while working remotely.
   * **Phone Call / Audio / Hotspot:** Inhibits suspend during active streaming, hotspot, or call operations.
3. Writes back the verified wakeup count and executes `rtcwake -m mem` via `sxmo_suspend.sh`.
4. The system enters low-power suspend-to-RAM (SPM deep sleep).
5. Pressing the physical **Power Button** triggers an interrupt via `mtk-pmic-keys`, instantaneously resuming execution and restoring the display.
