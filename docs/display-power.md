# Display & Power Management Architecture

## Display Hardware

* **Panel:** HD+ 720 x 1600 @ 60Hz (Novatek NT36572 / DJN LCM).
* **Driver:** MediaTek Framebuffer (`mtkfb`) + Primary Display engine (`videox/primary_display.c`).
* **Server:** Xorg X11 with `fbdev` driver and `dwm` (Dynamic Window Manager via Sxmo).

## Power Button & Sleep / Wake Handling

### 1. Framebuffer Blanking (`mtk-fb-blank`)
MediaTek's display engine responds to standard `FBIOBLANK` ioctls:
* `mtk-fb-blank off`: calls `ioctl(fd, FBIOBLANK, FB_BLANK_POWERDOWN)`.
* `mtk-fb-blank on`: calls `ioctl(fd, FBIOBLANK, FB_BLANK_UNBLANK)`.

When unblanked, the Novatek touch controller re-initializes and the display controller re-enables CMDQ execution.

### 2. Button Debounce & Cooldown Lockout
Sxmo uses `bonsaid` (Bonsai daemon) to handle hardware button transitions from evdev:
* **Hold Delay (`power_pressed`):** 800ms threshold for long-press actions.
* **Click Delay (`power_released`):** 200ms debounce delay for snappy release.
* **Lockout Cooldown Guard:** [`sxmo_hook_inputhandler.sh`](file:///media/primaryHdd/Coding/Android/xiaomi-redmi-9a-linux/overlay/home/user/.config/sxmo/hooks/sxmo_hook_inputhandler.sh) implements a 1.0s timestamp lockout. Rapid mechanical bounces or trailing taps within 1000ms are safely ignored.
* **Unconditional Wakeup:** When the screen is locked or off, any power button click explicitly runs `sxmo_state.sh set unlock` instead of toggling, ensuring the screen turns on and stays on.

### 3. Kernel Suspend Stability
* In [`primary_display.c`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/drivers/misc/mediatek/video/mt6765/videox/primary_display.c):
  Gated `primary_display_esd_check_enable(0)` during suspend to prevent asynchronous ESD check CMDQ packets to sleeping LCM.
* In [`mtk-pmic-keys.c`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/drivers/input/keyboard/mtk-pmic-keys.c):
  Enabled `device_init_wakeup(&pdev->dev, true)` for reliable wakeup from MT6357 PMIC.
