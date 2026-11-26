use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
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

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_signal(_sig: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

#[derive(Default)]
struct ClaimState {
    accel_claims: u32,
    light_claims: u32,
    proximity_claims: u32,
    stop: bool,
}

impl ClaimState {
    fn any_claimed(&self) -> bool {
        self.accel_claims > 0 || self.light_claims > 0 || self.proximity_claims > 0
    }
}

fn set_sensor_mode(fd: RawFd, ioctl_code: i32, enable: bool) -> bool {
    let mut val: i32 = if enable { 1 } else { 0 };
    unsafe { libc::ioctl(fd, ioctl_code, &mut val) >= 0 }
}

struct SensorProxy {
    has_accel: bool,
    orientation: String,
    tilt: String,

    has_als: bool,
    light: f64,

    has_ps: bool,
    proximity_near: bool,

    gsensor_fd: Option<RawFd>,
    alsps_fd: Option<RawFd>,
    claims: Arc<(Mutex<ClaimState>, Condvar)>,
}

#[interface(name = "net.hadess.SensorProxy")]
impl SensorProxy {
    fn claim_accelerometer(&mut self) {
        let (lock, cvar) = &*self.claims;
        let mut st = lock.lock().unwrap();
        let prev = st.accel_claims;
        st.accel_claims = st.accel_claims.saturating_add(1);
        if prev == 0 && self.has_accel {
            if let Some(fd) = self.gsensor_fd {
                if !set_sensor_mode(fd, GSENSOR_IOCTL_INIT, true) {
                    eprintln!("[mtk-sensor-proxy] Failed to enable /dev/gsensor");
                }
            }
        }
        println!("[mtk-sensor-proxy] ClaimAccelerometer (total: {})", st.accel_claims);
        cvar.notify_all();
    }

    fn release_accelerometer(&mut self) {
        let (lock, cvar) = &*self.claims;
        let mut st = lock.lock().unwrap();
        st.accel_claims = st.accel_claims.saturating_sub(1);
        if st.accel_claims == 0 && self.has_accel {
            if let Some(fd) = self.gsensor_fd {
                let _ = set_sensor_mode(fd, GSENSOR_IOCTL_INIT, false);
            }
        }
        println!("[mtk-sensor-proxy] ReleaseAccelerometer (total: {})", st.accel_claims);
        cvar.notify_all();
    }

    fn claim_light(&mut self) {
        let (lock, cvar) = &*self.claims;
        let mut st = lock.lock().unwrap();
        let prev = st.light_claims;
        st.light_claims = st.light_claims.saturating_add(1);
        if prev == 0 && self.has_als {
            if let Some(fd) = self.alsps_fd {
                if !set_sensor_mode(fd, ALSPS_SET_ALS_MODE, true) {
                    eprintln!("[mtk-sensor-proxy] Failed to enable ALS");
                }
            }
        }
        println!("[mtk-sensor-proxy] ClaimLight (total: {})", st.light_claims);
        cvar.notify_all();
    }

    fn release_light(&mut self) {
        let (lock, cvar) = &*self.claims;
        let mut st = lock.lock().unwrap();
        st.light_claims = st.light_claims.saturating_sub(1);
        if st.light_claims == 0 && self.has_als {
            if let Some(fd) = self.alsps_fd {
                let _ = set_sensor_mode(fd, ALSPS_SET_ALS_MODE, false);
            }
        }
        println!("[mtk-sensor-proxy] ReleaseLight (total: {})", st.light_claims);
        cvar.notify_all();
    }

    fn claim_proximity(&mut self) {
        let (lock, cvar) = &*self.claims;
        let mut st = lock.lock().unwrap();
        let prev = st.proximity_claims;
        st.proximity_claims = st.proximity_claims.saturating_add(1);
        if prev == 0 && self.has_ps {
            if let Some(fd) = self.alsps_fd {
                if !set_sensor_mode(fd, ALSPS_SET_PS_MODE, true) {
                    eprintln!("[mtk-sensor-proxy] Failed to enable PS");
                }
            }
        }
        println!("[mtk-sensor-proxy] ClaimProximity (total: {})", st.proximity_claims);
        cvar.notify_all();
    }

    fn release_proximity(&mut self) {
        let (lock, cvar) = &*self.claims;
        let mut st = lock.lock().unwrap();
        st.proximity_claims = st.proximity_claims.saturating_sub(1);
        if st.proximity_claims == 0 && self.has_ps {
            if let Some(fd) = self.alsps_fd {
                let _ = set_sensor_mode(fd, ALSPS_SET_PS_MODE, false);
            }
        }
        println!("[mtk-sensor-proxy] ReleaseProximity (total: {})", st.proximity_claims);
        cvar.notify_all();
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

fn disable_all_sensors(gsensor: Option<&File>, alsps: Option<&File>, has_accel: bool, has_als: bool, has_ps: bool) {
    if has_accel {
        if let Some(f) = gsensor {
            let _ = set_sensor_mode(f.as_raw_fd(), GSENSOR_IOCTL_INIT, false);
        }
    }
    if let Some(f) = alsps {
        let fd = f.as_raw_fd();
        if has_als {
            let _ = set_sensor_mode(fd, ALSPS_SET_ALS_MODE, false);
        }
        if has_ps {
            let _ = set_sensor_mode(fd, ALSPS_SET_PS_MODE, false);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("[mtk-sensor-proxy] Starting native MediaTek Sensor Proxy (Rust)...");

    unsafe {
        libc::signal(libc::SIGINT, handle_signal as *const () as usize);
        libc::signal(libc::SIGTERM, handle_signal as *const () as usize);
    }

    // 1. Probe Accelerometer (/dev/gsensor) while keeping hardware disabled until claimed
    let gsensor_file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open("/dev/gsensor")
        .ok();

    let has_accel = match &gsensor_file {
        Some(file) => {
            if set_sensor_mode(file.as_raw_fd(), GSENSOR_IOCTL_INIT, false) {
                println!("[mtk-sensor-proxy] /dev/gsensor probed (standby until claimed)");
                true
            } else {
                eprintln!("[mtk-sensor-proxy] /dev/gsensor probe failed");
                false
            }
        }
        None => {
            eprintln!("[mtk-sensor-proxy] Could not open /dev/gsensor");
            false
        }
    };

    // 2. Probe Ambient Light & Proximity (/dev/als_ps) while keeping hardware disabled until claimed
    let alsps_file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open("/dev/als_ps")
        .ok();

    let (has_als, has_ps) = match &alsps_file {
        Some(file) => {
            let fd = file.as_raw_fd();
            let ok_ps = set_sensor_mode(fd, ALSPS_SET_PS_MODE, false);
            let ok_als = set_sensor_mode(fd, ALSPS_SET_ALS_MODE, false);
            println!("[mtk-sensor-proxy] /dev/als_ps probed (PS: {}, ALS: {}, standby until claimed)", ok_ps, ok_als);
            (ok_als, ok_ps)
        }
        None => {
            eprintln!("[mtk-sensor-proxy] Could not open /dev/als_ps");
            (false, false)
        }
    };

    let claims = Arc::new((Mutex::new(ClaimState::default()), Condvar::new()));

    let proxy = SensorProxy {
        has_accel,
        orientation: "normal".into(),
        tilt: "portrait-up".into(),
        has_als,
        light: 0.0,
        has_ps,
        proximity_near: false,
        gsensor_fd: gsensor_file.as_ref().map(|f| f.as_raw_fd()),
        alsps_fd: alsps_file.as_ref().map(|f| f.as_raw_fd()),
        claims: Arc::clone(&claims),
    };

    // 3. Connect to D-Bus System Bus
    let conn = Builder::system()?
        .name("net.hadess.SensorProxy")?
        .serve_at("/net/hadess/SensorProxy", proxy)?
        .build()?;

    println!("[mtk-sensor-proxy] Registered net.hadess.SensorProxy on system bus");

    let iface_ref = conn.object_server().interface::<_, SensorProxy>("/net/hadess/SensorProxy")?;

    // Signal watcher thread to wake Condvar on SIGINT/SIGTERM
    let claims_sig = Arc::clone(&claims);
    thread::spawn(move || {
        while !SHUTDOWN.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(500));
        }
        let (lock, cvar) = &*claims_sig;
        let mut st = lock.lock().unwrap();
        st.stop = true;
        cvar.notify_all();
    });

    // 4. On-Demand Polling Loop (sleeps indefinitely with zero wakeups when unclaimed)
    let mut gsensor_buf = [0u8; 64];

    loop {
        let (accel_active, light_active, ps_active) = {
            let (lock, cvar) = &*claims;
            let mut st = lock.lock().unwrap();
            while !st.stop && !st.any_claimed() {
                st = cvar.wait(st).unwrap();
            }
            if st.stop {
                break;
            }
            (st.accel_claims > 0, st.light_claims > 0, st.proximity_claims > 0)
        };

        thread::sleep(Duration::from_millis(250));

        let mut orientation_update: Option<String> = None;
        let mut tilt_update: Option<String> = None;
        let mut light_update: Option<f64> = None;
        let mut proximity_update: Option<bool> = None;

        if has_accel && accel_active {
            if let Some(file) = &gsensor_file {
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

        if let Some(file) = &alsps_file {
            let fd = file.as_raw_fd();

            if has_als && light_active {
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

            if has_ps && ps_active {
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

    disable_all_sensors(gsensor_file.as_ref(), alsps_file.as_ref(), has_accel, has_als, has_ps);
    println!("[mtk-sensor-proxy] Sensors disabled; exiting cleanly.");
    Ok(())
}
