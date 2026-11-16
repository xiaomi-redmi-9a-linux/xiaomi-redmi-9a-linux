#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright 2022 Sxmo Contributors

# include common definitions
# shellcheck source=scripts/core/sxmo_common.sh
. sxmo_common.sh

notify_volume_change() {
	vol="$(sxmo_audio.sh vol get)"

	if [ "$vol" != "$lastvol" ]; then
		sxmo_hook_statusbar.sh volume
		sxmo_audio.sh notify
	fi

	lastvol="$vol"
}

notify_mic_volume_change() {
	micvol="$(sxmo_audio.sh mic volget)"

	if [ "$micvol" != "$lastmicvol" ]; then
		sxmo_hook_statusbar.sh volume
		sxmo_audio.sh micnotify
	fi

	lastmicvol="$micvol"
}

while ! pactl info >/dev/null 2>&1; do
	sleep 1
done

while true; do
	pactl subscribe 2>/dev/null | while read -r line; do
		case "$line" in
			"Event 'change' on sink "*)
				notify_volume_change
				;;
			"Event 'change' on source "*)
				notify_mic_volume_change
				;;
		esac
	done
	sleep 1
done
