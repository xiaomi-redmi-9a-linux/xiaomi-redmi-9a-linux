#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Xiaomi Redmi 9A (blossom / MT6765) runtime power profile

case "$1" in
    screenoff|powersave)
        # Drop CPU governor to powersave (clamps active core to 900MHz minimum clock)
        for g in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
            [ -w "$g" ] && echo powersave > "$g" 2>/dev/null
        done
        # Power down BIG cluster (cpu4-7) and 2 LITTLE cores (cpu2-3) to leave only cpu0 running
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
