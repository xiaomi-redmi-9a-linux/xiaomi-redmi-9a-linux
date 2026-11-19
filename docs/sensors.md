# Sensor Subsystem & Native Proxy (`mtk-sensor-proxy`)

## 1. Overview
The Xiaomi Redmi 9A (`dandelion`/`blossom`, MediaTek Helio G25 / MT6765) includes:
- **3-Axis Accelerometer:** Exposed via character device `/dev/gsensor` (and `/dev/m_acc_misc`).
- **Ambient Light Sensor (ALS):** Exposed via `/dev/als_ps` (and `/dev/m_als_misc`).
- **Proximity Sensor (PS):** Exposed via `/dev/als_ps` (and `/dev/m_ps_misc`).

Standard Linux mobile environments (such as Sxmo, Phosh, GNOME, and KDE Plasma Mobile) communicate with sensors through the FreeDesktop standard **`net.hadess.SensorProxy`** D-Bus interface. The upstream `iio-sensor-proxy` daemon cannot communicate with MediaTek's proprietary character driver interfaces.

To resolve this, we implemented **`mtk-sensor-proxy`**, a native daemon written in **Rust** that bridges MediaTek's hardware ioctls directly into the `net.hadess.SensorProxy` D-Bus interface.

---

## 2. Hardware Interface & IOCTL Specifications

### Accelerometer (`/dev/gsensor`)
- **Initialization:**
  ```c
  #define GSENSOR_IOCTL_INIT 0x8501
  int val = 1;
  ioctl(fd, GSENSOR_IOCTL_INIT, &val);
  ```
- **Read Sensor Data:**
  ```c
  #define GSENSOR_IOCTL_READ_SENSORDATA 0x80048503
  char buf[64];
  ioctl(fd, GSENSOR_IOCTL_READ_SENSORDATA, buf);
  ```
  Returns a null-terminated string containing three hex integers (two's complement 32-bit hex) representing milligravities ($1\text{G} \approx 9800\text{ mg}$).

### Ambient Light & Proximity (`/dev/als_ps`)
- **Activation:**
  ```c
  #define ALSPS_SET_PS_MODE  0x40048401
  #define ALSPS_SET_ALS_MODE 0x40048405
  int val = 1;
  ioctl(fd, ALSPS_SET_PS_MODE, &val);
  ioctl(fd, ALSPS_SET_ALS_MODE, &val);
  ```
- **Read Ambient Light (Lux):**
  ```c
  #define ALSPS_GET_ALS_RAW_DATA 0x80048408
  int lux_raw;
  ioctl(fd, ALSPS_GET_ALS_RAW_DATA, &lux_raw);
  ```
- **Read Proximity State:**
  ```c
  #define ALSPS_GET_PS_TEST_RESULT 0x80048409
  int ps_res;
  ioctl(fd, ALSPS_GET_PS_TEST_RESULT, &ps_res);
  // ps_res == 0: NEAR
  // ps_res == 1: FAR
  ```

---

## 3. Native Rust Implementation (`mtk-sensor-proxy`)

The daemon is located in `src/mtk-sensor-proxy/`:
- **Language:** Rust (edition 2021)
- **D-Bus Library:** `zbus` 4.x
- **Target:** `aarch64-unknown-linux-musl` (pure static binary, zero dynamic dependencies)
- **Binary Footprint:** ~1.2 MB on disk, **~1.4 MB VmRSS in memory**
- **Read Latency:** ~0.43 ms per hardware poll; polling at 4 Hz uses **<0.1% CPU**.

### Implemented D-Bus Interface (`net.hadess.SensorProxy`)
- **Methods:**
  - `ClaimAccelerometer()`, `ReleaseAccelerometer()`
  - `ClaimLight()`, `ReleaseLight()`
  - `ClaimProximity()`, `ReleaseProximity()`
- **Properties & Change Signals:**
  - `HasAccelerometer` (boolean): `true`
  - `AccelerometerOrientation` (string): `"normal"`, `"bottom-up"`, `"left-up"`, `"right-up"`
  - `AccelerometerTilt` (string): `"face-up"`, `"face-down"`, `"portrait-up"`, `"portrait-down"`, `"landscape-left"`, `"landscape-right"`
  - `HasAmbientLight` (boolean): `true`
  - `LightLevel` (double): lux value
  - `LightLevelUnit` (string): `"lux"`
  - `HasProximity` (boolean): `true`
  - `ProximityNear` (boolean): `true` (near) / `false` (far)

---

## 4. Service Configuration (OpenRC)

The daemon is registered as an OpenRC service:
- Script: `/etc/init.d/mtk-sensor-proxy`
- Runlevel: `default` (`rc-update add mtk-sensor-proxy default`)
- Dependencies: `need dbus`, `after dbus`

Commands:
```sh
rc-service mtk-sensor-proxy status
rc-service mtk-sensor-proxy restart
```

---

## 5. Sxmo Integration

### Adaptive Brightness (`sxmo_adaptivebrightness.sh`)
Automatically adjusts screen backlight based on the ambient light sensor:
```sh
superctl start sxmo_adaptivebrightness
```

### Proximity Lock (`sxmo_proximitylock.sh`)
During calls or screen-off state transitions, turns off display backlight and blocks touchscreen events when an object is near the earpiece.

### Touchscreen Matrix & Orientation
Touchscreen is pointer ID `9` (`NVTCapacitiveTouchScreen [floating slave]`). Configured in `/usr/bin/sxmo_deviceprofile_mediatek,MT6765.sh`:
```sh
if [ -n "$DISPLAY" ] && command -v xinput >/dev/null 2>&1; then
	export SXMO_TOUCHSCREEN_ID="$(xinput list 2>/dev/null | awk -F'id=' '/NVTCapacitiveTouchScreen.*floating/ {print $2}' | awk '{print $1}')"
fi
export SXMO_TOUCHSCREEN_ID="${SXMO_TOUCHSCREEN_ID:-9}"
```
*Note on Xorg rotation:* Dynamic RandR rotation (`xrandr`) is not supported by `xf86-video-fbdev` at runtime without restarting X; the display operates natively in 720x1600 portrait mode.
