# Audio Subsystem on MediaTek MT6765 (Helio G25)

## Overview

The Xiaomi Redmi 9A (`dandelion`/`blossom`) uses a MediaTek MT6765 / MT6357 PMIC audio subsystem integrated with the AFE (Audio Front End).

Under native Linux, audio hardware is exposed via ALSA SoC:
- **Card 0 (`mt63xxaccdet`):** Hardware 3.5mm headset jack detection controller (`/dev/input/by-path/platform-mt635x-accdet-event`).
- **Card 1 (`mtsndcard`):** Primary AFE DSP and MT6357 PMIC DAC / Amplifiers.

---

## Kernel Fixes

1. **DL2 DMA Buffer Oops Fix (`mtk-soc-pcm-dl2.c`):**
   * Default Android vendor logic expected a runtime preallocated DMA buffer on card probe.
   * Fixed in commit `9f2606e58`: added safe NULL check and fallback dynamic allocation when `substream->dma_buffer.area` is queried before allocation.

2. **ALSA Mixer Permission Handler:**
   * Enabled userspace access to mixer controls on Card 1 so PipeWire / ALSA userspace can configure routing without root.

---

## Distortion Fix & WirePlumber Format Lock (S16LE)

By default, PipeWire opened ALSA in 32-bit mode (`S32LE`). On MT6765, MediaTek's driver (`mtk-soc-pcm-dl1.c`) configures `AFE_WLEN_32_BIT_ALIGN_8BIT_0_24BIT_DATA` in 32-bit mode, expecting 8-bit zero padding followed by 24-bit audio. Standard Linux 32-bit PCM resulted in bit misalignment, producing pure static noise.

Locking the sink to 16-bit PCM (`audio.format = "S16LE"`) maps to `AFE_WLEN_16_BIT`, producing crystal-clear audio:

* **`/etc/wireplumber/wireplumber.conf.d/50-alsa-no-mmap.conf`:**
  ```json
  monitor.alsa.rules = [
    {
      matches = [
        {
          device.name = "~alsa_card.*"
        }
      ]
      actions = {
        update-props = {
          api.alsa.soft-mixer = true
        }
      }
    },
    {
      matches = [
        {
          node.name = "~alsa_output.*"
        },
        {
          node.name = "~alsa_input.*"
        }
      ]
      actions = {
        update-props = {
          audio.format = "S16LE"
          api.alsa.disable-mmap = true
          api.alsa.soft-mixer = true
        }
      }
    }
  ]
  ```

---

## Audio Routing: Speaker vs. 3.5mm Headphone Jack

On the MT6357 PMIC, the analog DAC register (`AUDDEC_ANA_CON0`) is shared between speaker and headphone paths:
- **Speaker:** Controlled by `Speaker_Amp_Switch` (numid 4) and `Ext_Speaker_Amp_Switch` (numid 12). Both switches require explicit `0 -> 1` toggling so the kernel's `on_change` handler executes `Speaker_Amp_Change(true)` (connecting DAC to LOL MUX) and `Ext_Speaker_Amp_Change(true)` (sending 1-wire pulses to wake the external amplifier IC).
- **Headphone Jack:** Controlled by `Audio_Amp_L_Switch` (numid 2) and `Audio_Amp_R_Switch` (numid 1).
- **Headphone Pre-amp Gain:** `Headset_PGAL_GAIN` / `Headset_PGAR_GAIN` (numid 6 & 7) set to comfortable `4Db`.
- **Lineout / Speaker Pre-amp Gain:** `Lineout_PGAL_GAIN` / `Lineout_PGAR_GAIN` set to `4Db` for loud, clear speaker output.

When the speaker amplifier turns on, register `AUDDEC_ANA_CON0` is programmed to `0x3009`, cutting off the headphone path. To switch to the 3.5mm jack, speaker amplifiers must be turned off and the headphone amplifier switches toggled `Off` then `On` to trigger `Audio_Amp_Change()` and set `AUDDEC_ANA_CON0 = 0x3aff`.

### Boot Default Routing

At system startup, `/usr/share/sxmo/default_hooks/sxmo_hook_start.sh` checks the 3.5mm headphone jack status:
- If headphones are plugged in at boot, `sxmo_audio.sh route headphones` is run.
- Otherwise, `sxmo_audio.sh route speaker` is run, ensuring music and system sounds work immediately without waiting for a jack event.

### Manual Switching via `sxmo_audio.sh`

* **Route to Headphones:**
  ```bash
  sxmo_audio.sh route headphones
  ```
* **Route to Speaker:**
  ```bash
  sxmo_audio.sh route speaker
  ```
* **Route to Both (simultaneous):**
  ```bash
  sxmo_audio.sh route both
  ```

The current route is also selectable with live checkmark indicators directly in the **Sxmo Audio Menu** (`Menu -> Audio`).

---

## Automatic 3.5mm Jack Detection (`sxmo_jackmon`)

The kernel's `mt635x-accdet` driver generates input events on `/dev/input/by-path/platform-mt635x-accdet-event` (`SW_HEADPHONE_INSERT` and `SW_MICROPHONE_INSERT` for 4-pole headsets).

The lightweight C daemon `sxmo_jackmon` ([`src/sxmo_jackmon.c`](../src/sxmo_jackmon.c)) runs as a background user service under `superd`:
1. On startup, reads current jack switch state via `ioctl(EVIOCGSW)` and routes audio appropriately.
2. When headphones are plugged in, automatically switches to headphones and displays an on-screen banner (` Audio Output: Switched to Headphones`).
3. When unplugged, automatically reverts audio to the built-in speaker (` Audio Output: Switched to Built-in Speaker`).

---

## FM Radio Architecture & Touch GUI

The Redmi 9A features an onboard MediaTek MT6631 FM receiver module (`/dev/fm`) communicating over I2S to the AFE and MT6357 PMIC.

### 1. Backend Daemon (`mtk_fm`)
The native C daemon [`src/mtk_fm.c`](../src/mtk_fm.c) runs at `/usr/local/bin/mtk_fm`:
* **Device Control:** Controls `/dev/fm` using MediaTek ioctls (`FM_IOCTL_POWERUP`, `FM_IOCTL_POWERDOWN`, `FM_IOCTL_TUNE`, `FM_IOCTL_GETRSSI`).
* **ALSA Stream Capture:** Opens PCM device 21 (`/dev/snd/pcmC1D21p`) via `tinyalsa` to route FM audio into the AFE interconnect.
* **MT6357 Codec Transitions:**
  - **Speaker Route:** `numid=5` (`Headset_Speaker_Amp_Switch`) must be set to `0` (enabling the Lineout buffer circuit in kernel lines 1000 & 3778). `numid=4` (`Speaker_Amp_Switch`) is toggled `0 -> 1` to configure `AUDDEC_ANA_CON4` connecting DAC to LOL MUX, and `numid=12` is asserted to pulse the external loudspeaker amplifier. Lineout PGAL/R gain is set to `4Db`.
  - **Headphone Route:** Speaker amplifiers are shut off, and `Audio_Amp_L/R` (numid 1, 2) are enabled with Headset PGAL/R gain set to `4Db`.
  - **Antenna Keepalive:** `numid=21` (`Headset_Speaker_Amp_Switch` antenna path) remains asserted so the 3.5mm headphone cable functions as the FM antenna even when routing audio to the loudspeaker.
* **Seek & Scan:** Implements a fast 200 kHz step-and-sample seek loop and full sweep routine across 87.5–108.0 MHz with a `-88 dBm` RSSI lock threshold.
* **Socket IPC:** Listens on `/run/mtk_fm.sock` supporting `STATUS`, `START <freq>`, `TUNE <freq>`, `SEEK <UP|DOWN>`, `SCAN`, `ROUTE <headphone|speaker>`, `VOL <0-100>`, and `STOP`.
* **Signal Safety:** Ignores `SIGPIPE` and uses `MSG_NOSIGNAL` on client disconnects to prevent daemon interruption.

### 2. Touchscreen Frontend (`fm_radio_gui.py`)
A touch-optimized GTK3 application located at [`overlay/usr/local/bin/fm_radio_gui.py`](../overlay/usr/local/bin/fm_radio_gui.py):
* Large live frequency display and RSSI badge.
* Touch buttons for Seek Down/Up, fine stepping (±0.1 MHz), and interactive frequency slider.
* Audio route buttons: **🎧 Headphones** and **🔊 Speaker**.
* Volume slider mapped to MT6357 digital gain (0–524288).
* Horizontal station presets carousel with "+ Favorite" and "🔍 Auto Scan" buttons.
* Dedicated touch **✕** close button in the header bar.

### 3. Desktop & Status Bar Integration
* **App Registration:** Added to [`overlay/usr/share/sxmo/default_hooks/sxmo_hook_apps.sh`](../overlay/usr/share/sxmo/default_hooks/sxmo_hook_apps.sh) as `󰐹 FM Radio` under the Applications menu.
* **Context Menu:** [`overlay/usr/share/sxmo/default_hooks/sxmo_hook_contextmenu.sh`](../overlay/usr/share/sxmo/default_hooks/sxmo_hook_contextmenu.sh) handles `*[Ff]m_radio*` offering *Close App*, *Toggle Audio Route*, and *Power Off FM*.
* **Dynamic Status Bar:** [`overlay/usr/share/sxmo/default_hooks/sxmo_hook_statusbar.sh`](../overlay/usr/share/sxmo/default_hooks/sxmo_hook_statusbar.sh) displays `󰐹 <frequency>` (e.g. `󰐹 98.3`) in real time when FM is playing, updates during tuning, and clears on stop.

---

## Music Player & MPRIS Media Control

### 1. Touchscreen Music Player: Amberol
The device uses **Amberol** as its primary touch music player:
* **Mobile Touch Interface:** Libadwaita responsive layout with large touch controls (Play, Pause, Previous, Next, Seek bar, Volume slider, and Queue).
* **Audio Engine:** Backed by GStreamer with `gst-plugin-pipewire` outputting to PipeWire 1.6.8 with 16-bit format locking (`S16LE`).
* **Menu Entry:** Registered natively in [`overlay/usr/share/sxmo/default_hooks/sxmo_hook_apps.sh`](../overlay/usr/share/sxmo/default_hooks/sxmo_hook_apps.sh) as `󰎆 Amberol` opening `/home/user/Music`.
* **Window Controls:** Touch `✕` button in the header bar allows 1-tap exit.

### 2. MPRIS & Hardware Key Integration (`playerctl`)
Amberol exposes standard `org.mpris.MediaPlayer2` D-Bus interfaces managed via `playerctl`:
* **Status Bar Playback Banner:** The Sxmo status bar dynamically displays the current track and artist (e.g. `Unknown artist - Track...`).
* **Hardware Media Keys:** Configured in `bonsai_tree.json` and `dwm` keybindings:
  ```bash
  playerctl play-pause  # Toggle play / pause
  playerctl next        # Next track in queue
  playerctl previous    # Previous track
  playerctl stop        # Stop playback
  ```
* **Lockscreen Playback:** Audio continues playing in the background with the display turned off (`mtk-fb-blank off`). Waking the screen restores the player state seamlessly without stuttering or pipe drops.
