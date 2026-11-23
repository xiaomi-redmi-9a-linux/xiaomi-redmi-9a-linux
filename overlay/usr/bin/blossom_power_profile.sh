#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Xiaomi Redmi 9A (blossom / MT6765) runtime power profile
# Optimizes standby power by migrating execution to Efficiency Cluster 1 (cpu4)

case "$1" in
    screenoff|powersave)
        # 1. Bring efficiency core cpu4 online first
        [ -w /sys/devices/system/cpu/cpu4/online ] && echo 1 > /sys/devices/system/cpu/cpu4/online
        
        # 2. Set powersave governor on cpu4 (drops to 400 MHz minimum clock)
        [ -w /sys/devices/system/cpu/cpu4/cpufreq/scaling_governor ] && echo powersave > /sys/devices/system/cpu/cpu4/cpufreq/scaling_governor
        
        # 3. Power down all performance cores (0-3) and remaining efficiency cores (5-7)
        for c in 0 1 2 3 5 6 7; do
            [ -w "/sys/devices/system/cpu/cpu$c/online" ] && echo 0 > "/sys/devices/system/cpu/cpu$c/online" 2>/dev/null
        done
        
        # 4. Ensure Wi-Fi 802.11 power saving is enabled
        iw dev wlan0 set power_save on 2>/dev/null || true
        ;;
    unlock|performance|normal)
        # 1. Bring all 8 cores back online
        for c in 0 1 2 3 5 6 7; do
            [ -w "/sys/devices/system/cpu/cpu$c/online" ] && echo 1 > "/sys/devices/system/cpu/cpu$c/online" 2>/dev/null
        done
        
        # 2. Restore schedutil governor across all online cores for maximum UI fluidity
        for g in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
            [ -w "$g" ] && echo schedutil > "$g" 2>/dev/null
        done
        ;;
esac
