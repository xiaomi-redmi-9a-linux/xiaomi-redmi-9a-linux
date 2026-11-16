# Timezone & 12-Hour Clock Architecture

## Overview

The Xiaomi Redmi 9A (`dandelion`/`blossom`) Linux system is configured with:
* **Timezone:** Chennai local time (`Asia/Kolkata` / Indian Standard Time, IST, UTC+05:30).
* **Format:** 12-hour clock with AM/PM indicators across both the Sxmo status bar and the desktop/lockscreen Conky widget.

---

## 1. System Timezone Configuration

Alpine Linux manages system timezone via `tzdata`:
* **Timezone Database:** `/usr/share/zoneinfo/Asia/Kolkata`
* **Localtime Symlink:** `/etc/localtime -> /usr/share/zoneinfo/Asia/Kolkata`
* **Timezone Name File:** `/etc/timezone` containing `Asia/Kolkata`

To verify the system time and timezone:
```bash
$ date
Mon Nov 16 22:45:00 IST 2026
```

---

## 2. Sxmo Status Bar (Top Right Clock)

Sxmo renders its top status bar using `sxmobar` (which drives the X11 root window name `WM_NAME` monitored by `dwm`).

### Time Format Variable (`SXMO_STATUS_DATE_FORMAT`)
Status bar time is formatted using the `SXMO_STATUS_DATE_FORMAT` environment variable:
* **Format String:** `"%I:%M %p"` (e.g. `10:45 PM`)
* **Configured in:**
  - [`overlay/home/user/.config/sxmo/profile`](../overlay/home/user/.config/sxmo/profile):
    ```sh
    export SXMO_STATUS_DATE_FORMAT="%I:%M %p"
    ```
  - [`overlay/usr/bin/sxmo_deviceprofile_mediatek,MT6765.sh`](../overlay/usr/bin/sxmo_deviceprofile_mediatek,MT6765.sh):
    ```sh
    export SXMO_TIME_FORMAT="%I:%M %p"
    export SXMO_STATUS_DATE_FORMAT="%I:%M %p"
    ```

### Periodic Execution Loop
Status bar time updates are orchestrated by Sxmo's periodic job runner:
```sh
sxmo_jobs.sh start statusbar_periodics sxmo_run_aligned.sh 60 \
    sxmo_hook_statusbar.sh periodics
```
Every 60 seconds (aligned to the top of the minute), `sxmo_hook_statusbar.sh periodics` runs `set_time`:
```sh
set_time() {
    if pidof peanutbutter > /dev/null; then
        sxmobar -d time
    else
        date "+${SXMO_STATUS_DATE_FORMAT:-%I:%M %p}" | while read -r date; do
            sxmobar -a time 99 "$date"
        done
    fi
}
```
Setting the fallback in [`overlay/usr/share/sxmo/default_hooks/sxmo_hook_statusbar.sh`](../overlay/usr/share/sxmo/default_hooks/sxmo_hook_statusbar.sh) ensures that even background sub-shells without an exported variable cleanly display the 12-hour format.

---

## 3. Desktop & Lockscreen Clock Widget (`conky`)

The center clock widget on the wallpaper is rendered by `conky` running as a user service under `superd` (`sxmo_conky`).

### Configuration ([`overlay/home/user/.config/sxmo/conky.conf`](../overlay/home/user/.config/sxmo/conky.conf))
```lua
conky.config = {
    out_to_wayland = true,
    alignment = 'middle_middle',
    own_window_argb_value = 0,
    own_window_type = 'desktop',
    default_shade_color = 'grey',
    draw_shades = true,
    font = 'Sxmo:size=13',
    use_xft = true,
    update_interval = 15,
}

conky.text = [[
${alignc}${font Sxmo:size=50:style=Bold}${time %I}:${time %M} ${font Sxmo:size=24}${time %p}${font}

${alignc}${font Sxmo:size=20}${time %a %d %b %Y}${font}
]]
```

### Visual Layout
The Conky widget is centered vertically and horizontally:
* **Primary Time:** `10:45` in bold 50pt Sxmo font.
* **Period:** `PM` in 24pt Sxmo font aligned alongside the minutes.
* **Date:** `Mon 16 Nov 2026` in 20pt Sxmo font.
