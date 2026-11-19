use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::thread;
use std::time::Duration;
use zbus::blocking::connection::Builder;
use zbus::interface;

// MediaTek IOCTL codes
const GSENSOR_IOCTL_INIT: i32 = 0x8501;
const GSENSOR_IOCTL_READ_SENSORDATA: i32 = 0x80048503u32 as i32;

const ALSPS_SET_PS_MODE: i32 = 0x40048401u32 as i32;
const ALSPS_SET_ALS_MODE: i32 = 0x40048405u32 as i32;
const ALSPS_GET_ALS_RAW_DATA: i32 = 0x80048408u32 as i32;
const ALSPS_GET_PS_TEST_RESULT: i32 = 0x80048409u32 as i32;

struct SensorProxy {
    has_accel: bool,
    orientation: String,
    tilt: String,

    has_als: bool,
    light: f64,

    has_ps: bool,
    proximity_near: bool,

    accel_claims: u32,
    light_claims: u32,
    proximity_claims: u32,
}

#[interface(name = "net.hadess.SensorProxy")]
impl SensorProxy {
    fn claim_accelerometer(&mut self) {
        self.accel_claims = self.accel_claims.saturating_add(1);
        println!("[mtk-sensor-proxy] ClaimAccelerometer (total: {})", self.accel_claims);
    }

    fn release_accelerometer(&mut self) {
        self.accel_claims = self.accel_claims.saturating_sub(1);
        println!("[mtk-sensor-proxy] ReleaseAccelerometer (total: {})", self.accel_claims);
    }

    fn claim_light(&mut self) {
        self.light_claims = self.light_claims.saturating_add(1);
        println!("[mtk-sensor-proxy] ClaimLight (total: {})", self.light_claims);
    }

    fn release_light(&mut self) {
        self.light_claims = self.light_claims.saturating_sub(1);
        println!("[mtk-sensor-proxy] ReleaseLight (total: {})", self.light_claims);
    }

    fn claim_proximity(&mut self) {
        self.proximity_claims = self.proximity_claims.saturating_add(1);
        println!("[mtk-sensor-proxy] ClaimProximity (total: {})", self.proximity_claims);
    }

    fn release_proximity(&mut self) {
        self.proximity_claims = self.proximity_claims.saturating_sub(1);
        println!("[mtk-sensor-proxy] ReleaseProximity (total: {})", self.proximity_claims);
    }

    #[zbus(property)]
    fn has_accelerometer(&self) -> bool {
        self.has_accel
    }

    #[zbus(property)]
    fn accelerometer_orientation(&self) -> &str {
        &self.orientation
    }

    #[zbus(property)]
    fn accelerometer_tilt(&self) -> &str {
        &self.tilt
    }

    #[zbus(property)]
    fn has_ambient_light(&self) -> bool {
        self.has_als
    }

    #[zbus(property)]
    fn light_level_unit(&self) -> &str {
        "lux"
    }

    #[zbus(property)]
    fn light_level(&self) -> f64 {
        self.light
    }

    #[zbus(property)]
    fn has_proximity(&self) -> bool {
        self.has_ps
    }

    #[zbus(property)]
    fn proximity_near(&self) -> bool {
        self.proximity_near
    }
}

fn calculate_orientation(x: i32, y: i32, z: i32, current_orientation: &str, current_tilt: &str) -> (String, String) {
    if z.abs() > 7500 {
        let tilt = if z > 0 { "face-up" } else { "face-down" };
        return (current_orientation.to_string(), tilt.to_string());
    }

    let abs_x = x.abs();
    let abs_y = y.abs();
    let hysteresis = 1200;

    if abs_y > abs_x + hysteresis {
        if y > 0 {
            ("normal".into(), "portrait-up".into())
        } else {
            ("bottom-up".into(), "portrait-down".into())
        }
    } else if abs_x > abs_y + hysteresis {
        if x > 0 {
            ("right-up".into(), "landscape-right".into())
        } else {
            ("left-up".into(), "landscape-left".into())
        }
    } else {
        (current_orientation.to_string(), current_tilt.to_string())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("[mtk-sensor-proxy] Starting native MediaTek Sensor Proxy (Rust)...");

    // 1. Initialize Accelerometer (/dev/gsensor)
    let gsensor_file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open("/dev/gsensor");

    let has_accel = match &gsensor_file {
        Ok(file) => {
            let mut init_val: i32 = 1;
            let ret = unsafe { libc::ioctl(file.as_raw_fd(), GSENSOR_IOCTL_INIT, &mut init_val) };
            if ret >= 0 {
                println!("[mtk-sensor-proxy] /dev/gsensor initialized successfully");
                true
            } else {
                eprintln!("[mtk-sensor-proxy] GSENSOR_IOCTL_INIT failed: {}", ret);
                false
            }
        }
        Err(e) => {
            eprintln!("[mtk-sensor-proxy] Could not open /dev/gsensor: {}", e);
            false
        }
    };

    // 2. Initialize Ambient Light & Proximity (/dev/als_ps)
    let alsps_file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open("/dev/als_ps");

    let (has_als, has_ps) = match &alsps_file {
        Ok(file) => {
            let fd = file.as_raw_fd();
            let mut val: i32 = 1;
            let ret_ps = unsafe { libc::ioctl(fd, ALSPS_SET_PS_MODE, &mut val) };
            let ret_als = unsafe { libc::ioctl(fd, ALSPS_SET_ALS_MODE, &mut val) };
            println!("[mtk-sensor-proxy] /dev/als_ps initialized (PS: {}, ALS: {})", ret_ps, ret_als);
            (ret_als >= 0, ret_ps >= 0)
        }
        Err(e) => {
            eprintln!("[mtk-sensor-proxy] Could not open /dev/als_ps: {}", e);
            (false, false)
        }
    };

    let proxy = SensorProxy {
        has_accel,
        orientation: "normal".into(),
        tilt: "portrait-up".into(),
        has_als,
        light: 0.0,
        has_ps,
        proximity_near: false,
        accel_claims: 0,
        light_claims: 0,
        proximity_claims: 0,
    };

    // 3. Connect to D-Bus System Bus
    let conn = Builder::system()?
        .name("net.hadess.SensorProxy")?
        .serve_at("/net/hadess/SensorProxy", proxy)?
        .build()?;

    println!("[mtk-sensor-proxy] Registered net.hadess.SensorProxy on system bus");

    let iface_ref = conn.object_server().interface::<_, SensorProxy>("/net/hadess/SensorProxy")?;

    // 4. Polling Loop
    let mut gsensor_buf = [0u8; 64];

    loop {
        thread::sleep(Duration::from_millis(250));

        let mut orientation_update: Option<String> = None;
        let mut tilt_update: Option<String> = None;
        let mut light_update: Option<f64> = None;
        let mut proximity_update: Option<bool> = None;

        // Poll Accelerometer
        if has_accel {
            if let Ok(file) = &gsensor_file {
                let ret = unsafe {
                    libc::ioctl(file.as_raw_fd(), GSENSOR_IOCTL_READ_SENSORDATA, gsensor_buf.as_mut_ptr())
                };
                if ret >= 0 {
                    if let Ok(s) = std::str::from_utf8(&gsensor_buf) {
                        let s = s.trim_matches('\0').trim();
                        let parts: Vec<i32> = s.split_whitespace()
                            .filter_map(|p| u32::from_str_radix(p, 16).ok().map(|v| v as i32))
                            .collect();
                        if parts.len() >= 3 {
                            let (x, y, z) = (parts[0], parts[1], parts[2]);
                            let iface = iface_ref.get();
                            let (new_ori, new_tilt) = calculate_orientation(x, y, z, &iface.orientation, &iface.tilt);
                            if new_ori != iface.orientation {
                                orientation_update = Some(new_ori);
                            }
                            if new_tilt != iface.tilt {
                                tilt_update = Some(new_tilt);
                            }
                        }
                    }
                }
            }
        }

        // Poll Light & Proximity
        if let Ok(file) = &alsps_file {
            let fd = file.as_raw_fd();

            if has_als {
                let mut lux_raw: i32 = 0;
                let ret = unsafe { libc::ioctl(fd, ALSPS_GET_ALS_RAW_DATA, &mut lux_raw) };
                if ret >= 0 {
                    let lux = lux_raw as f64;
                    let iface = iface_ref.get();
                    if (lux - iface.light).abs() >= (5.0_f64).max(iface.light * 0.10) {
                        light_update = Some(lux);
                    }
                }
            }

            if has_ps {
                let mut ps_res: i32 = 0;
                let ret = unsafe { libc::ioctl(fd, ALSPS_GET_PS_TEST_RESULT, &mut ps_res) };
                if ret >= 0 {
                    let is_near = ps_res == 0;
                    let iface = iface_ref.get();
                    if is_near != iface.proximity_near {
                        proximity_update = Some(is_near);
                    }
                }
            }
        }

        // Apply updates and emit signals
        if orientation_update.is_some() || tilt_update.is_some() || light_update.is_some() || proximity_update.is_some() {
            let mut iface = iface_ref.get_mut();

            if let Some(new_ori) = orientation_update {
                println!("[mtk-sensor-proxy] Orientation: {}", new_ori);
                iface.orientation = new_ori;
                let _ = zbus::block_on(iface.accelerometer_orientation_changed(iface_ref.signal_context()));
            }

            if let Some(new_tilt) = tilt_update {
                iface.tilt = new_tilt;
                let _ = zbus::block_on(iface.accelerometer_tilt_changed(iface_ref.signal_context()));
            }

            if let Some(new_light) = light_update {
                println!("[mtk-sensor-proxy] Light: {:.1} lux", new_light);
                iface.light = new_light;
                let _ = zbus::block_on(iface.light_level_changed(iface_ref.signal_context()));
            }

            if let Some(new_near) = proximity_update {
                println!("[mtk-sensor-proxy] Proximity: {}", if new_near { "NEAR" } else { "FAR" });
                iface.proximity_near = new_near;
                let _ = zbus::block_on(iface.proximity_near_changed(iface_ref.signal_context()));
            }
        }
    }
}
