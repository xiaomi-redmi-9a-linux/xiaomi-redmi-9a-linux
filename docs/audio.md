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
- **Speaker:** Controlled by `Speaker_Amp_Switch` (numid 4) and `Ext_Speaker_Amp_Switch` (numid 12).
- **Headphone Jack:** Controlled by `Audio_Amp_L_Switch` (numid 2) and `Audio_Amp_R_Switch` (numid 1).
- **Headphone Pre-amp Gain:** `Headset_PGAL_GAIN` / `Headset_PGAR_GAIN` (numid 6 & 7) set to comfortable `-12Db`.
- **Lineout / Speaker Pre-amp Gain:** `Lineout_PGAL_GAIN` / `Lineout_PGAR_GAIN` set to `-10Db`.

When the speaker amplifier turns on, register `AUDDEC_ANA_CON0` is programmed to `0x3009`, cutting off the headphone path. To switch to the 3.5mm jack, speaker amplifiers must be turned off and the headphone amplifier switches toggled `Off` then `On` to trigger `Audio_Amp_Change()` and set `AUDDEC_ANA_CON0 = 0x3aff`.

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
