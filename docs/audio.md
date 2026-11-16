# Audio Subsystem on MediaTek MT6765 (Helio G25)

## Overview

The Xiaomi Redmi 9A (`dandelion`/`blossom`) uses a MediaTek MT6765 / MT6357 PMIC audio subsystem integrated with the AFE (Audio Front End).

Under native mainline/cip Linux kernels, audio hardware is accessed via ALSA SoC (`mtk-soc-pcm-routing`).

## Kernel Fixes

1. **DL2 DMA Buffer Oops Fix (`mtk-soc-pcm-dl2.c`):**
   * Default Android vendor logic expected a runtime preallocated DMA buffer on card probe.
   * Fixed in commit `9f2606e58`: added safe NULL check and fallback dynamic allocation when `substream->dma_buffer.area` is queried before allocation.

2. **ALSA Mixer Permission Handler:**
   * Replaced restricted vendor mixer controls with standard userspace permissions so PipeWire / ALSA userspace can configure routing without root.

## Routing Scripts & Services

On Alpine Linux with OpenRC:

* **`/etc/udev/rules.d/89-mtk-audio.rules`:**
  Ensures `/dev/snd/*` nodes are accessible by the `audio` group (and `user`).

* **`/etc/local.d/00-audio-dev.start`:**
  Creates symlinks for ALSA devices if dynamic udev naming places them under custom bus paths.

* **`/etc/local.d/01-audio.start`:**
  Configures ALSA routing for speaker playback:
  ```bash
  amixer -c 0 sset 'Audio_Amp_R' 'Hw_Speaker'
  amixer -c 0 sset 'Audio_Amp_L' 'Hw_Speaker'
  amixer -c 0 sset 'Speaker_Volume' 15
  ```

## Sound Server

* **PipeWire + WirePlumber** runs under the `user` session managed by `superd`.
* Test sound playback:
  ```bash
  pw-play /usr/share/sxmo/notify.ogg
  ```
