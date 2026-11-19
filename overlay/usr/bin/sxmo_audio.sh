#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright 2022 Sxmo Contributors

# shellcheck source=configs/default_hooks/sxmo_hook_icons.sh
if [ -f "$HOME/.config/sxmo/hooks/sxmo_hook_icons.sh" ]; then
	. "$HOME/.config/sxmo/hooks/sxmo_hook_icons.sh"
elif [ -f "/usr/share/sxmo/default_hooks/sxmo_hook_icons.sh" ]; then
	. "/usr/share/sxmo/default_hooks/sxmo_hook_icons.sh"
elif command -v sxmo_hook_icons.sh >/dev/null 2>&1; then
	. sxmo_hook_icons.sh
fi
. sxmo_common.sh

set -e

notifyvol() {
	vol="$1"
	if [ "-" = "$vol" ]; then
		vol="$(cat)"
	fi
	if [ "muted" = "$vol" ]; then
		vol=0
	fi
	if [ -z "$SXMO_WOB_DISABLE" ]; then
		printf "%s\n" "$vol" > "$XDG_RUNTIME_DIR"/sxmo.obsock
	else
		notify-send -r 999 "$icon_audio Volume $vol"
	fi
}

# adjust *output* vol/mute
volup() {
	step="${1:-5}"
	cur="$(volget)"
	if [ "$cur" = "muted" ]; then
		voltogglemute
		cur="$(volget)"
	fi
	cur="${cur:-0}"
	if [ "$cur" -ge 100 ] 2>/dev/null; then
		pactl set-sink-volume @DEFAULT_SINK@ 100%
	else
		new=$((cur + step))
		if [ "$new" -gt 100 ]; then
			new=100
		fi
		pactl set-sink-volume @DEFAULT_SINK@ "${new}%"
	fi
}

voldown() {
	step="${1:-5}"
	cur="$(volget)"
	if [ "$cur" = "muted" ]; then
		cur=0
	fi
	cur="${cur:-0}"
	if [ "$cur" -le 0 ] 2>/dev/null; then
		pactl set-sink-volume @DEFAULT_SINK@ 0%
	else
		new=$((cur - step))
		if [ "$new" -lt 0 ]; then
			new=0
		fi
		pactl set-sink-volume @DEFAULT_SINK@ "${new}%"
	fi
}

voltogglemute() {
	pactl set-sink-mute @DEFAULT_SINK@ toggle
}

volismuted() {
	pactl get-sink-mute @DEFAULT_SINK@ | grep -q "Mute: yes"
}

volget() {
	if volismuted; then
		printf "muted"
	else
		pactl get-sink-volume @DEFAULT_SINK@ | head -n1 | cut -d'/' -f2 | sed 's/ //g' | sed 's/\%//'
	fi
}

volset() {
	pactl set-sink-volume @DEFAULT_SINK@ "$1"%
}

# Hardware Audio Routing (Speaker / Headphones / Both)
route() {
	target="$1"
	case "$target" in
		headphones|hp)
			amixer -c 1 cset numid=4 0 >/dev/null 2>&1
			amixer -c 1 cset numid=12 0 >/dev/null 2>&1
			amixer -c 1 cset numid=5 0 >/dev/null 2>&1
			amixer -c 1 cset numid=1 0 >/dev/null 2>&1
			amixer -c 1 cset numid=2 0 >/dev/null 2>&1
			amixer -c 1 cset numid=1 1 >/dev/null 2>&1
			amixer -c 1 cset numid=2 1 >/dev/null 2>&1
			amixer -c 1 set 'Headset_PGAL_GAIN' -- '4Db' >/dev/null 2>&1
			amixer -c 1 set 'Headset_PGAR_GAIN' -- '4Db' >/dev/null 2>&1
			notify-send -r 998 "$icon_hdp Audio Output" "Switched to Headphones" 2>/dev/null || true
			;;
		speaker|spk)
			amixer -c 1 cset numid=1 0 >/dev/null 2>&1
			amixer -c 1 cset numid=2 0 >/dev/null 2>&1
			amixer -c 1 cset numid=5 0 >/dev/null 2>&1
			amixer -c 1 cset numid=4 0 >/dev/null 2>&1
			amixer -c 1 cset numid=4 1 >/dev/null 2>&1
			amixer -c 1 cset numid=12 0 >/dev/null 2>&1
			amixer -c 1 cset numid=12 1 >/dev/null 2>&1
			amixer -c 1 set 'Lineout_PGAL_GAIN' -- '4Db' >/dev/null 2>&1
			amixer -c 1 set 'Lineout_PGAR_GAIN' -- '4Db' >/dev/null 2>&1
			notify-send -r 998 "$icon_spk Audio Output" "Switched to Built-in Speaker" 2>/dev/null || true
			;;
		both)
			amixer -c 1 cset numid=4 0 >/dev/null 2>&1
			amixer -c 1 cset numid=4 1 >/dev/null 2>&1
			amixer -c 1 cset numid=12 0 >/dev/null 2>&1
			amixer -c 1 cset numid=12 1 >/dev/null 2>&1
			amixer -c 1 cset numid=1 0 >/dev/null 2>&1
			amixer -c 1 cset numid=2 0 >/dev/null 2>&1
			amixer -c 1 cset numid=1 1 >/dev/null 2>&1
			amixer -c 1 cset numid=2 1 >/dev/null 2>&1
			amixer -c 1 cset numid=5 1 >/dev/null 2>&1
			amixer -c 1 set 'Lineout_PGAL_GAIN' -- '4Db' >/dev/null 2>&1
			amixer -c 1 set 'Lineout_PGAR_GAIN' -- '4Db' >/dev/null 2>&1
			amixer -c 1 set 'Headset_PGAL_GAIN' -- '4Db' >/dev/null 2>&1
			amixer -c 1 set 'Headset_PGAR_GAIN' -- '4Db' >/dev/null 2>&1
			notify-send -r 998 "$icon_spk Audio Output" "Switched to Both (Speaker + Headphones)" 2>/dev/null || true
			;;
		get)
			spk="$(amixer -c 1 cget numid=4 2>/dev/null | grep ': values=' | cut -d= -f2)"
			hp="$(amixer -c 1 cget numid=1 2>/dev/null | grep ': values=' | cut -d= -f2)"
			combo="$(amixer -c 1 cget numid=5 2>/dev/null | grep ': values=' | cut -d= -f2)"
			if [ "$combo" = "1" ]; then
				echo "both"
			elif [ "$hp" = "1" ] && [ "$spk" = "0" ]; then
				echo "headphones"
			else
				echo "speaker"
			fi
			;;
	esac
}

_routesubmenu() {
	cur_route="$(route get)"
	if [ "$cur_route" = "both" ]; then
		printf "%s %s Route: Both ^ true\n" "$icon_chk" "$icon_spk"
		printf "  %s Switch to Speaker ^ route speaker\n" "$icon_spk"
		printf "  %s Switch to Headphones ^ route headphones\n" "$icon_hdp"
	elif [ "$cur_route" = "headphones" ]; then
		printf "%s %s Route: Headphones ^ true\n" "$icon_chk" "$icon_hdp"
		printf "  %s Switch to Speaker ^ route speaker\n" "$icon_spk"
		printf "  %s Switch to Both ^ route both\n" "$icon_spk"
	else
		printf "%s %s Route: Speaker ^ true\n" "$icon_chk" "$icon_spk"
		printf "  %s Switch to Headphones ^ route headphones\n" "$icon_hdp"
		printf "  %s Switch to Both ^ route both\n" "$icon_spk"
	fi
}

# adjust *input* vol/mute
micvolup() {
	pactl set-source-volume @DEFAULT_SOURCE@ +"${1:-5}%"
}

micvoldown() {
	pactl set-source-volume @DEFAULT_SOURCE@ -"${1:-5}%"
}

mictogglemute() {
	pactl set-source-mute @DEFAULT_SOURCE@ toggle
}

micismuted() {
	pactl get-source-mute @DEFAULT_SOURCE@ | grep -q "Mute: yes"
}

micvolget() {
	if micismuted; then
		printf "muted"
	else
		pactl get-source-volume @DEFAULT_SOURCE@ | head -n1 | cut -d'/' -f2 | sed 's/ //g' | sed 's/\%//'
	fi
}

micvolset() {
	pactl set-source-volume @DEFAULT_SOURCE@ "$1"%
}

# set the *active port* for output
deviceset() {
	pactl set-sink-port @DEFAULT_SINK@ "[Out] $1"
}

# set the *active port* for input
devicesetinput() {
	pactl set-source-port @DEFAULT_SOURCE@ "[In] $1"
}

# get the *active port* for input
devicegetinput() {
	[ -z "$1" ] && default_source="$(pactl get-default-source)" || default_source="$1"
	pactl --format=json list sources | jq -r ".[] | select(.name == \"$default_source\") | .active_port" | sed 's/\[In] //'
}

# get the *active port* for output
deviceget() {
	[ -z "$1" ] && default_sink="$(pactl get-default-sink)" || default_sink="$1"
	pactl --format=json list sinks | jq -r ".[] | select(.name == \"$default_sink\") | .active_port" | sed 's/\[Out] //'
}

# get the default sink
devicegetdefaultsink() {
	pactl get-default-sink
}

# get the default source
devicegetdefaultsource() {
	pactl get-default-source
}

sourceset() {
	pactl set-default-source "$1"
}

sinkset() {
	pactl set-default-sink "$1"
}

profileset() {
	pactl set-card-profile "$1" "$2"
}

# get a list of sinks
_sinkssubmenu() {
	[ -z "$1" ] && default_sink="$(pactl get-default-sink)" || default_sink="$1"
	pactl --format=json list sinks | jq -r '.[] | .name, .description' | while read -r line; do
		name="$line"
		read -r description
		if [ "$default_sink" = "$name" ]; then
			printf "%s %s %s ^ sinkset %s\n" "$icon_chk" "$icon_spk" "$description" "$name"
		else
			printf "  %s %s ^ sinkset %s\n" "$icon_spk" "$description" "$name"
		fi
	done
}

# get a list of output ports
_outportssubmenu() {
	[ -z "$1" ] && default_sink="$(pactl get-default-sink)" || default_sink="$1"
	active_out_port="$(deviceget "$default_sink")"
	pactl --format=json list sinks | jq -r ".[] | select(.name == \"$default_sink\" ) | .ports[] | select(.availability != \"not available\" ) | .name" | sed 's/\[Out] //' | while read -r line; do
		[ "$active_out_port" = "$line" ] && icon="$icon_ton" || icon="$icon_tof"
		printf "  %s %s ^ deviceset %s\n" "$icon" "$line" "$line"
	done
}

# get a list of input sources
_sourcessubmenu() {
	[ -z "$1" ] && default_source="$(pactl get-default-source)" || default_source="$1"
	pactl --format=json list sources | jq -r '.[] | select (.monitor_source == "") | .name, .description' | while read -r line; do
		name="$line"
		read -r description
		if [ "$default_source" = "$name" ]; then
			printf "%s %s %s ^ sourceset %s\n" "$icon_chk" "$icon_mic" "$description" "$name"
		else
			printf "  %s %s ^ sourceset %s\n" "$icon_mic" "$description" "$name"
		fi
	done
}

_cardssubmenu() {
	cards="$(pactl -f json list cards)"
	printf "%s\n" "$cards" | jq -r '.[] | [.name, .active_profile] | @tsv' | \
		while IFS="	" read -r card active_profile; do
		printf "%s %s\n" "$icon_nte" "$card"
		printf "%s\n" "$cards" | jq -r ".[] | select(.name == \"$card\") | .profiles | to_entries[] | select(.value.available) | [.key, .value.description] | @tsv" | \
			while IFS="	" read -r profile desc; do
			if [ "$profile" = "$active_profile" ]; then
				printf "%s %s ^ true\n" "$icon_chk" "$desc"
			else
				printf "  %s ^ profileset %s \"%s\"\n" "$desc" "$card" "$profile"
			fi
		done
	done
}

# get a list of input ports
_inportssubmenu() {
	[ -z "$1" ] && default_source="$(pactl get-default-source)" || default_source="$1"
	active_in_port="$(devicegetinput "$default_source")"
	pactl --format=json list sources | jq -r ".[] | select(.name == \"$default_source\" ) | .ports[] | select(.availability != \"not available\" ) | .name" | sed 's/\[In] //' | while read -r line; do
		[ "$active_in_port" = "$line" ] && icon="$icon_ton" || icon="$icon_tof"
		printf "  %s %s ^ devicesetinput %s\n" "$icon" "$line" "$line"
	done
}

_ringmodesubmenu() {
	if [ -f "$XDG_CONFIG_HOME"/sxmo/.noring ]; then
		printf " %s Ring ^ rm -f \"$XDG_CONFIG_HOME\"/sxmo/.noring\n" "$icon_tof"
	else
		printf " %s Ring ^ touch \"$XDG_CONFIG_HOME\"/sxmo/.noring\n" "$icon_ton"
	fi
	if [ -f "$XDG_CONFIG_HOME"/sxmo/.novibrate ]; then
		printf " %s Vibrate ^ rm -f \"$XDG_CONFIG_HOME\"/sxmo/.novibrate\n" "$icon_tof"
	else
		printf " %s Vibrate ^ touch \"$XDG_CONFIG_HOME\"/sxmo/.novibrate\n" "$icon_ton"
	fi
	if [ -f "$XDG_CONFIG_HOME"/sxmo/.busy ]; then
		end="$(cat "$XDG_CONFIG_HOME"/sxmo/.busy)"
		if [ -n "$end" ] && [ "$(date +%s)" -gt "$end" ]; then
			rm "$XDG_CONFIG_HOME"/sxmo/.busy
		fi
	fi
	if [ -f "$XDG_CONFIG_HOME"/sxmo/.busy ]; then
		printf " %s Busy ^ rm -f \"$XDG_CONFIG_HOME\"/sxmo/.busy\n" "$icon_ton"
	else
		printf " %s Busy ^ touch \"$XDG_CONFIG_HOME\"/sxmo/.busy\n" "$icon_tof"
		printf " %s Busy for 15 min ^ date '+%%s+15*60' | bc > \"$XDG_CONFIG_HOME\"/sxmo/.busy\n" "$icon_tmr"
		printf " %s Busy for 30 min ^ date '+%%s+30*60' | bc > \"$XDG_CONFIG_HOME\"/sxmo/.busy\n" "$icon_tmr"
		printf " %s Busy for 1 hour ^ date '+%%s+60*60' | bc > \"$XDG_CONFIG_HOME\"/sxmo/.busy\n" "$icon_tmr"
		printf " %s Busy for 2 hour ^ date '+%%s+2*60*60' | bc > \"$XDG_CONFIG_HOME\"/sxmo/.busy\n" "$icon_tmr"
		printf " %s Busy for 3 hour ^ date '+%%s+3*60*60' | bc > \"$XDG_CONFIG_HOME\"/sxmo/.busy\n" "$icon_tmr"
	fi
}

menuchoices() {
cur_vol="$(volget)"
cur_mic_vol="$(micvolget)"
default_sink_name="$(devicegetdefaultsink)"
default_source_name="$(devicegetdefaultsource)"
grep . <<EOF
$icon_cls Close Menu  ^ exit
Output:
$(_sinkssubmenu "$default_sink_name")
$(_routesubmenu)
$(
if [ "$cur_vol" != "muted" ]; then
	printf "  %s Volume (%s%%) ^ volup\n" "$icon_aru" "$cur_vol"
	printf "  %s Volume (%s%%) ^ voldown\n" "$icon_ard" "$cur_vol"
	printf "  %s Output Mute ^ voltogglemute\n" "$icon_tof"
else
	printf "  %s Output Mute ^ voltogglemute\n" "$icon_ton"
fi
)
$(_outportssubmenu "$default_sink_name")
Input:
$(_sourcessubmenu "$default_source_name")
$(
if [ "$cur_mic_vol" != "muted" ]; then
	printf "  %s Volume (%s%%) ^ micvolup\n" "$icon_aru" "$cur_mic_vol"
	printf "  %s Volume (%s%%) ^ micvoldown\n" "$icon_ard" "$cur_mic_vol"
	printf "  %s Input Mute ^ mictogglemute\n" "$icon_tof"
else
	printf "  %s Input Mute ^ mictogglemute\n" "$icon_ton"
fi
)
$(_inportssubmenu "$default_source_name")
Cards:
$(_cardssubmenu)
$(_ringmodesubmenu)
EOF
}

if [ -n "$SXMO_NO_AUDIO" ]; then
	sxmo_log "Audio is disabled."
	exit
fi

if [ -z "$*" ]; then
	set -- menu
fi

cmd="$1"
shift
case "$cmd" in
	menu)
		while : ; do
			CHOICES="$(menuchoices)"
			PICKED="$(
				printf "%s\n" "$CHOICES" |
					cut -d'^' -f1 |
					sxmo_dmenu.sh -i -p "Audio"
			)"

			CMD="$(printf "%s\n" "$CHOICES" | grep -m1 -F "$PICKED" | cut -d '^' -f2)"

			eval "$CMD"
		done
		;;
	vol)
		verb="$1"
		shift
		vol"$verb" "$@"
		;;
	mic)
		verb="$1"
		shift
		mic"$verb" "$@"
		;;
	device)
		verb="$1"
		shift
		device"$verb" "$1"
		;;
	route)
		verb="$1"
		shift
		route "$verb" "$@"
		;;
	notify)
		notifyvol "$(volget)"
		;;
	micnotify)
		notifyvol "$(micvolget)"
		;;
esac
