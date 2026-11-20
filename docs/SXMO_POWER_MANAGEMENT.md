# Sxmo Mobile Desktop & Power Management on Xiaomi Redmi 9A

This guide documents the Sxmo configuration, hook management, power saving features, and audio device rules for the Xiaomi Redmi 9A.

---

## 1. Sxmo Configuration Migration System

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

## 2. Power Saving Hooks

Custom hooks in [`linux/overlay/home/user/.config/sxmo/hooks/`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/linux/overlay/home/user/.config/sxmo/hooks/):

### A. Screenoff Hook (`sxmo_hook_screenoff.sh`)
* **Header:** `# configversion: 66b67db06146e3b2dd3ae9a6f1411c5e`
* **Actions:**
  - Turns display off (`sxmo_wm.sh display off`)
  - Disables touchscreen input events (`sxmo_wm.sh inputevent touchscreen off`)
  - Starts periodic LED breathing blink (red/blue)
  - **Bluetooth Smart Idle Power Saving:** If Bluetooth is enabled but no devices (headphones, speakers) are actively connected, the hook automatically powers down the Bluetooth controller. This saves approximately **66 mA continuous current draw** during screen-off sleep.

### B. Unlock Hook (`sxmo_hook_unlock.sh`)
* **Header:** `# configversion: 5697e0579ea20a4a6b8dc05f5814d0c6`
* **Actions:**
  - Powers display on (`sxmo_wm.sh display on`)
  - Enables touchscreen input events
  - Stops idle LED blinking
  - **Bluetooth Auto-Restore:** Restores Bluetooth controller power if it was auto-slept by the screenoff hook.

---

## 3. Audio Clean-up Udev Rules

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
