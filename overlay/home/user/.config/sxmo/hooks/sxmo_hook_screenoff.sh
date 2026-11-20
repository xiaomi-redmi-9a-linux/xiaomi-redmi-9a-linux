#!/bin/sh
# configversion: 66b67db06146e3b2dd3ae9a6f1411c5e
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright 2022 Sxmo Contributors
#
# Hook called when system reaches screenoff state

# include common definitions
# shellcheck source=scripts/core/sxmo_common.sh
. sxmo_common.sh

sxmo_led.sh blink red blue &

[ "$SXMO_WM" = "sway" ] && swaymsg mode default
sxmo_wm.sh display off
sxmo_wm.sh inputevent touchscreen off

# Bluetooth Smart Power Saving:
# If Bluetooth is on, but NO devices are connected (e.g. not listening to music),
# put the controller in low-power off state to save 66 mA continuously.
if bluetoothctl show 2>/dev/null | grep -q "Powered: yes"; then
	if [ -z "$(bluetoothctl devices Connected 2>/dev/null)" ]; then
		touch "${XDG_RUNTIME_DIR:-/tmp}/.sxmo_bt_auto_slept"
		bluetoothctl power off >/dev/null 2>&1 || true
		sxmo_log "bluetooth: auto-powered off on screenoff (no connected devices)"
	fi
fi

if [ ! -e "$XDG_CACHE_HOME"/sxmo/sxmo.nosuspend ]; then
	sxmo_jobs.sh start periodic_blink \
		sxmo_run_periodically.sh -w 2 -- sxmo_led.sh blink red blue
fi

case "$SXMO_WM" in
	dwm)
		if ! sxmo_jobs.sh running proximity_lock -q; then
			sxmo_dmenu.sh close
		fi
		;;
esac

sxmo_jobs.sh stop idle_locker

wait

case "$SXMO_WM" in
	sway)
		if command -v peanutbutter > /dev/null; then
			peanutbutter --font Sxmo --statuscommand sxmo_hook_lockstatusbar.sh && sxmo_hook_statusbar.sh state_change &
		fi
		;;
esac
