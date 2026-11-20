#!/bin/sh
# configversion: 5697e0579ea20a4a6b8dc05f5814d0c6
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright 2022 Sxmo Contributors
#
# Hook called when system becomes unlocked / screen on

# include common definitions
# shellcheck source=scripts/core/sxmo_common.sh
. sxmo_common.sh

sxmo_led.sh blink red green &

sxmo_wm.sh display on
sxmo_wm.sh inputevent touchscreen on

# Restore Bluetooth power if it was auto-slept by screenoff
if [ -f "${XDG_RUNTIME_DIR:-/tmp}/.sxmo_bt_auto_slept" ]; then
	rm -f "${XDG_RUNTIME_DIR:-/tmp}/.sxmo_bt_auto_slept"
	rfkill unblock bluetooth 2>/dev/null || true
	bluetoothctl power on >/dev/null 2>&1 || true
	sxmo_log "bluetooth: restored power on screen unlock"
fi

sxmo_jobs.sh stop periodic_blink
sxmo_jobs.sh stop periodic_wakelock_check

# Go to the next idle state after 120 seconds of inactivity
if [ -e "$XDG_CACHE_HOME/sxmo/sxmo.noidle" ]; then
	sxmo_jobs.sh stop idle_locker
else
	case "$SXMO_WM" in
		river)
			sxmo_jobs.sh start idle_locker sxmo_idle.sh -w \
				timeout "${SXMO_UNLOCK_IDLE_TIME:-120}" 'sh -c "
					exec sxmo_state.sh idle
				"'
			;;
		sway)
			sxmo_jobs.sh start idle_locker sxmo_idle.sh -w \
				timeout "${SXMO_UNLOCK_IDLE_TIME:-120}" 'sh -c "
					swaymsg mode default;
					exec sxmo_state.sh idle
				"'
			;;
		i3)
			sxmo_jobs.sh start idle_locker sxmo_idle.sh -w \
				timeout "${SXMO_UNLOCK_IDLE_TIME:-120}" \
					'sh -c "i3-msg mode default; exec sxmo_state.sh idle"'
			;;
		dwm)
			sxmo_jobs.sh start idle_locker sxmo_idle.sh -w \
				timeout "${SXMO_UNLOCK_IDLE_TIME:-120}" "sxmo_state.sh idle"
			;;
	esac
fi

wait
