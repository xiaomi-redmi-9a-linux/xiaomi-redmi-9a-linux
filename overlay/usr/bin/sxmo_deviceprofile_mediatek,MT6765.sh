#!/bin/sh
# Xiaomi Redmi 9A (MediaTek Helio G25 / MT6765)
export SXMO_DISABLE_LEDS="1"
if [ -n "$DISPLAY" ] && command -v xinput >/dev/null 2>&1; then
	export SXMO_TOUCHSCREEN_ID="$(xinput list 2>/dev/null | awk -F'id=' '/NVTCapacitiveTouchScreen.*pointer/ {print $2}' | awk '{print $1}')"
fi
export SXMO_POWER_BUTTON="0:0:mtk-pmic-keys"
export SXMO_VOLUME_BUTTON="1:1:mtk-kpd"
export KEYBOARD="svkbd-mobile-intl"
export SXMO_SWAY_SCALE="2"
export SXMO_TIME_FORMAT="%H:%M  "
export SXMO_BAR_HEIGHT="64"
export SXMO_SIDEPAD="14"
export SXMO_BOTTOMPAD="24"
export SXMO_STATES="unlock screenoff"
export SXMO_SUSPENDABLE_STATES="none 0"
