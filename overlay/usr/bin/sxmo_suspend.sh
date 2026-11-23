#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Xiaomi Redmi 9A (blossom / MT6765)
# Deep system suspend (s2idle) causes Sleep-of-Death on MT6765 due to firmware watchdog
# and unhandled low-power state transitions. Power saving is handled via runtime CPU
# hotplugging in blossom_power_profile.sh. This script safely exits 0.
exit 0
