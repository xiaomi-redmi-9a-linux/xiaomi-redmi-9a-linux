# PowerVR Rogue GE8320 Hardware GPU Acceleration on Xiaomi Redmi 9A

This document details the complete hardware GPU acceleration implementation for the **Xiaomi Redmi 9A** (`blossom` / `dandelion`, MediaTek Helio G25 MT6765 / MT6762G) running native **Alpine Linux** (aarch64, musl libc).

---

## 1. Hardware & Architecture Overview

* **GPU Core:** Imagination Technologies PowerVR Rogue GE8320 (Series 8XE)
* **BVNC Hardware Revision:** `22.87.104.18`
* **Driver DDK Version:** Rogue DDK `1.13@5776728`
* **Firmware Files:** `rgx.fw` (198 KB) and `rgx.sh` (29 KB)
* **Kernel Driver:** `drivers/gpu/mediatek/gpu_rgx/m1.13ED5776728` (`pvrsrvkm`)
* **Userspace Bridge:** Patched `libhybris` for 64-bit musl libc connecting to vendor Android 11 64-bit bionic graphics HALs.

---

## 2. Kernel Configuration & Firmware

The PowerVR kernel module requires the firmware blobs to be embedded into the kernel image so they are available early before rootfs mounts:

In [`arch/arm64/configs/blossom_linux_defconfig`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/arch/arm64/configs/blossom_linux_defconfig):

```ini
# Embed Rogue GPU firmware
CONFIG_EXTRA_FIRMWARE="novatek_ts_djn_fw.bin ath9k_htc/htc_9271-1.4.0.fw rgx.fw rgx.sh"
CONFIG_EXTRA_FIRMWARE_DIR="firmware"

# MediaTek GPU Subsystem & DVFS Governor
CONFIG_MTK_GPU_SUPPORT=y
CONFIG_MTK_GPU_VERSION="rgx doma m1.13ED5776728"
CONFIG_MTK_GPU_COMMON_DVFS_SUPPORT=y
```

### Device Nodes Created
When the driver probes successfully, the following kernel device nodes appear:
* `/dev/pvr_sync` — PowerVR synchronization primitive
* `/dev/dri/card0` & `/dev/dri/renderD128` — DRM driver interface
* `/dev/ion` — Android ION memory allocator
* `/dev/mtk_cmdq` & `/dev/mtk_disp_mgr` & `/dev/mdp_sync` — MediaTek display command queue

---

## 3. Udev Rules & Permissions

By default, ION and MediaTek command queue device nodes are created with `0600 root:root`. To allow standard unprivileged users (such as `user` in Alpine Linux / Sxmo) to access 3D acceleration:

File: [`linux/overlay/etc/udev/rules.d/60-android-graphics.rules`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/linux/overlay/etc/udev/rules.d/60-android-graphics.rules) (installed to `/etc/udev/rules.d/60-android-graphics.rules` on device):

```udev
KERNEL=="ion", GROUP="video", MODE="0666"
KERNEL=="pvr_sync", GROUP="video", MODE="0666"
KERNEL=="mtk_cmdq", GROUP="video", MODE="0666"
KERNEL=="mtk_disp_mgr", GROUP="video", MODE="0666"
KERNEL=="mdp_sync", GROUP="video", MODE="0666"
KERNEL=="card*", SUBSYSTEM=="drm", GROUP="video", MODE="0666"
KERNEL=="renderD*", SUBSYSTEM=="drm", GROUP="video", MODE="0666"
KERNEL=="fb*", GROUP="video", MODE="0666"
```

---

## 4. Patched libhybris on musl libc

Stock `libhybris` targets glibc on Linux systems with legacy pre-Treble Android HALs. Running on native Alpine Linux with Android 11 HALs required three major architectural patches:

### A. 64-bit musl libc Linker & Hook Compatibility
* **musl Shims:** Provided [`musl_hooks_compat.h`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/linux/libhybris/patches/musl_hooks_compat.h) implementing missing GNU extensions (`scandirat`, `pvalloc`, `mallinfo`, `wmempcpy`, `TEMP_FAILURE_RETRY`, `__vsprintf_chk`).
* **`fpos_t` Layout:** Fixed `fgetpos` and `fsetpos` to read `my_fpos.__lldata` on musl instead of `my_fpos.__pos` (glibc).
* **Pthread Condition Variable:** Guarded `__wrefs` reference count reset with `#ifdef __GLIBC__`.
* **String Formatting:** Handled standard POSIX `strerror_r` return code vs GNU `char *` return.

### B. Treble gralloc1 Hardware Framebuffer (`/dev/fb0`) Backend
Android 8.0+ Treble HALs do not export the legacy `framebuffer_device_t`. Unmodified `libhybris` crashes immediately with:
```
Assertion failed: framebuffer_device (gralloc.c: hybris_gralloc_fbdev_format: 401)
```
**Fix:** Implemented direct Linux framebuffer integration inside [`hybris/gralloc/gralloc.c`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/linux/libhybris/patches/0001-libhybris-musl-android10-blossom.patch):
* Dynamically queries `/dev/fb0` resolution (`720 × 1600`) via `FBIOGET_VSCREENINFO` and `FBIOGET_FSCREENINFO`.
* `hybris_gralloc_fbdev_post()` locks the gralloc GPU buffer, copies pixels directly to the mmapped `/dev/fb0` framebuffer memory, and triggers page flipping via `ioctl(FBIOPAN_DISPLAY)`.

### C. Stack Canary Canary-Buffer Overflow Fix
In Rogue DDK `gralloc1_lock`, the HAL writes more than 8 bytes to the returned buffer address argument (writing pointer + stride/fences). A single `void *vaddr` pointer resulted in adjacent stack memory corruption and immediate `__stack_chk_fail`.
**Fix:**
```c
void *vaddr_buf[16];
memset(vaddr_buf, 0x55, sizeof(vaddr_buf));
int ret = hybris_gralloc_lock(handle,
    GRALLOC_USAGE_SW_READ_OFTEN | GRALLOC_USAGE_SW_WRITE_OFTEN,
    0, 0, w, h, (void **)&vaddr_buf[0]);
void *vaddr = vaddr_buf[0];
```

---

## 5. Performance & Verification Results

### Raw Compute Benchmark
Running [`gpu_fps_benchmark`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/linux/tests/gpu_fps_benchmark.c) (500 frames of full 3D Gouraud-shaded rotating tetrahedron, specular lighting, depth testing at 720 × 1600):
```
Benchmark Results:
  Frames rendered:  500
  Total time:       1.264 s
  Average FPS:      395.4 FPS
  Frame time:       2.53 ms/frame
```

### Silicon Hardware Utilization
Confirmed via kernel statistics during rendering:
* **Active GPU Silicon Load:** `/proc/pvr/status` shows GPU active load jumped from **0% to 64%**.
* **DVFS Dynamic Clock Frequency:** `/sys/kernel/ged/hal/current_freqency` dynamically scaled from idle up to **650000 KHz (650 MHz)**.
* **Physical Display:** 60 Hz smooth rendering on device screen via `/dev/fb0`.

---

## 6. Environment Configuration

File: [`linux/overlay/etc/profile.d/01-hybris-gpu.sh`](file:///media/primaryHdd/Coding/Android/kernel_xiaomi_blossom_linux/linux/overlay/etc/profile.d/01-hybris-gpu.sh):
```bash
export EGL_PLATFORM=fbdev
export HYBRIS_EGLPLATFORM=fbdev
```
