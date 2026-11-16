# Audio Subsystem on MediaTek MT6765 (Helio G25)

## Overview

The Xiaomi Redmi 9A (`dandelion`/`blossom`) uses a MediaTek MT6765 / MT6357 PMIC audio subsystem integrated with the AFE (Audio Front End).

Under native Linux, audio hardware is exposed via ALSA SoC:
- **Card 0 (`mt63xxaccdet`):** Headset jack detection controller.
- **Card 1 (`mtsndcard`):** Primary AFE DSP and MT6357 PMIC DAC / Amplifiers.

## Kernel Fixes

1. **DL2 DMA Buffer Oops Fix (`mtk-soc-pcm-dl2.c`):**
   * Default Android vendor logic expected a runtime preallocated DMA buffer on card probe.
   * Fixed in commit `9f2606e58`: added safe NULL check and fallback dynamic allocation when `substream->dma_buffer.area` is queried before allocation.

2. **ALSA Mixer Permission Handler:**
   * Enabled userspace access to mixer controls on Card 1 so PipeWire / ALSA userspace can configure routing without root.

## Routing Scripts & Services

On Alpine Linux with OpenRC:

* **`/etc/udev/rules.d/89-mtk-audio.rules`:**
  Ensures all `/dev/snd/*` nodes are accessible by the `audio` group (and `user`).

* **`/etc/local.d/01-audio.start`:**
  Enables speaker routing switches and sets the analog pre-amp attenuation:
  ```sh
  #!/bin/sh
  # Enable speaker switches on card 1
  amixer -c 1 cset numid=1 1
  amixer -c 1 cset numid=2 1
  amixer -c 1 cset numid=4 1
  amixer -c 1 cset numid=12 1

  # Configure comfortable analog gain baseline (-30 dB total cut)
  amixer -c 1 set 'Lineout_PGAL_GAIN' -- '-10Db'
  amixer -c 1 set 'Lineout_PGAR_GAIN' -- '-10Db'
  amixer -c 1 set 'Apply_N12DB_Gain' 'On'
  ```

## Sound Server Configuration (PipeWire + WirePlumber)

Because the MediaTek vendor ALSA driver does not expose a standard master hardware volume control, PipeWire by default passes unattenuated 100% full-scale (0 dBFS) digital audio to the DAC.

To enable proper digital volume attenuation in software:
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
          api.alsa.disable-mmap = true
          api.alsa.soft-mixer = true
        }
      }
    }
  ]
  ```

## Testing Playback

Test playback from user session:
```bash
paplay /usr/share/sxmo/notify.ogg
```
Adjust volume cleanly with PulseAudio / WirePlumber:
```bash
pactl set-sink-volume @DEFAULT_SINK@ 20%
```
