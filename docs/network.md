# Wi-Fi Architecture & Status Bar Bridge (`nmcli`)

## Overview

The Xiaomi Redmi 9A (`dandelion`/`blossom`) uses a MediaTek MT6631 / MT7668 Wi-Fi module communicating over SDIO (`wlan0`).

On **Alpine Linux**, network connectivity is managed via **`wpa_supplicant`** and **`udhcpc`** rather than NetworkManager to maintain a minimal, resource-efficient system footprint.

However, the **Sxmo** desktop environment natively expects NetworkManager's `nmcli` utility for:
1. Detecting active Wi-Fi connections and SSIDs on the status bar.
2. Calculating dynamic signal strength (0–100%) to display signal bars.
3. Supplying an event loop for the `sxmo_networkmonitor` user daemon.

To bridge this gap cleanly without installing the heavy NetworkManager daemon, we implemented a lightweight POSIX shell bridge at [`overlay/usr/local/bin/nmcli`](../overlay/usr/local/bin/nmcli).

---

## The `nmcli` Bridge Architecture

The bridge is located at `/usr/local/bin/nmcli` and intercepts the specific queries made by Sxmo scripts:

### 1. Active Connection Detection (`nmcli -t -f ... connection show --active`)
Sxmo's status bar ([`sxmo_hook_statusbar.sh`](../overlay/usr/share/sxmo/default_hooks/sxmo_hook_statusbar.sh)) runs:
```bash
nmcli -t -f NAME,UUID,TYPE,DEVICE connection show --active
```
The script uses regex `^([^:]*):([^:]*):802-11-wireless:([^:]*)` to parse the connection. If the third field is not `802-11-wireless`, Sxmo treats the device as offline and displays the red disconnected icon (`󰤮`).

Our bridge uses Linux wireless netlink (`iw dev wlan0 link`) to detect the current connection state without requiring root privileges:
* **Connected:** Returns `Airtel_Henry-Richard:wlan0:802-11-wireless:wlan0`
* **Disconnected:** Returns empty output (exit code 0).

### 2. Live Signal Strength (`nmcli -t -f ... dev wifi`)
Sxmo parses the signal strength to display appropriate Wi-Fi bar icons:
```bash
nmcli -t -f IN-USE,SIGNAL,SSID dev wifi
```

The bridge reads the real-time link RSSI from `iw dev wlan0 link`:
```
Connected to 30:74:96:40:e0:10 (on wlan0)
    SSID: Airtel_Henry-Richard
    freq: 2437
    signal: -40 dBm
```
It computes the signal quality percentage using the standard Linux wireless formula:
$$\text{quality} = 2 \times (\text{signal\_dbm} + 100)$$
* At `-50 dBm` or better: **100% (Full 4 bars)**
* At `-70 dBm`: **60% (3 bars)**
* At `-85 dBm`: **30% (1–2 bars)**
* Clamped safely between `0%` and `100%`.

The bridge returns formatted output:
```
*:100:Airtel_Henry-Richard
```

### 3. Background Daemon Event Loop (`nmcli monitor`)
Sxmo launches `sxmo_networkmonitor` under `superd` to watch network state changes.
The bridge implements an event loop:
* Samples `iw dev wlan0 link` every 3 seconds.
* Emits state change messages (`wlan0: connected`, `wlan0: disconnected`) when the link status transitions.
* Keeps `sxmo_networkmonitor` running reliably with minimal CPU usage.

---

## Status Bar Wi-Fi Icons

Based on the signal quality returned by the bridge, Sxmo dynamically updates the status bar icon:

| Signal Range | Icon | Appearance | Meaning |
| :---: | :---: | :---: | :--- |
| Disconnected | **`󰤮`** | Red | Wi-Fi interface down or unassociated |
| 1% – 25% | **`󰤟`** | White | 1 bar (very weak signal) |
| 26% – 50% | **`󰤢`** | White | 2 bars (fair signal) |
| 51% – 75% | **`󰤥`** | White | 3 bars (good signal) |
| 76% – 100% | **`󰤨`** | White | 4 bars (excellent / full signal) |

---

## Connecting to Wi-Fi Networks via CLI

### Option 1: Quick Configuration (`wpa_passphrase`)
To connect to a new WPA2/WPA3 Wi-Fi network:

```bash
# Append credentials to wpa_supplicant config
wpa_passphrase "Your_SSID" "Your_Password" | grep -v '^[[:space:]]*#psk=' >> /etc/wpa_supplicant/wpa_supplicant.conf

# Reload wpa_supplicant configuration
wpa_cli -i wlan0 reconfigure
```

### Option 2: Interactive CLI (`wpa_cli`)
```bash
wpa_cli -i wlan0
> scan
> scan_results
> add_network
0
> set_network 0 ssid "Your_SSID"
> set_network 0 psk "Your_Password"
> enable_network 0
> save_config
> quit
```

Once connected, DHCP will automatically assign an IP address via `udhcpc`, and the status bar will immediately transition from red disconnected (`󰤮`) to the white connected icon (`󰤨`) with full signal bars.
