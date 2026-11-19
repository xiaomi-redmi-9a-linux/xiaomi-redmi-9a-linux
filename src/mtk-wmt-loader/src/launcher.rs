use crate::ioctl::*;
use crate::patch::*;
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub const WMT_COMMAND_SRH_PATCH: &[u8] = b"srh_patch";
pub const WMT_COMMAND_SRH_ROM_PATCH: &[u8] = b"srh_rom_patch";

fn pwr_on_worker(fd: libc::c_int, running: Arc<AtomicBool>) {
    let magic_value: libc::c_ulong = 1;
    let mut count = 0;
    while count < 0x14 && running.load(Ordering::Relaxed) {
        let err = ioctl_val(fd, WMT_IOCTL_LPBK_POWER_CTRL, magic_value);
        if err == 0 {
            println!("[launcher] Power-on completed successfully!");
            break;
        }
        ioctl_val(fd, WMT_IOCTL_LPBK_POWER_CTRL, 0);
        eprintln!("[launcher] Power-on failed (err={}), retrying in 1s (count {}/20)...", err, count + 1);
        thread::sleep(Duration::from_secs(1));
        count += 1;
    }
}

fn wifi_enable_worker(running: Arc<AtomicBool>) {
    while running.load(Ordering::Relaxed) {
        if Path::new(WMT_WIFI).exists() {
            match std::fs::write(WMT_WIFI, b"1") {
                Ok(_) => {
                    println!("[launcher] WiFi enabled (/dev/wmtWifi <- 1)!");
                    return;
                }
                Err(e) => {
                    eprintln!("[launcher] Failed to enable WiFi: {}, retrying in 1s...", e);
                }
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
}

fn handle_srh_rom_patch(fd: libc::c_int) -> Result<(), String> {
    let chip_id = ioctl_val(fd, WMT_IOCTL_GET_CHIP_INFO, WMT_CHIPINFO_GET_CHIPID) as u32;
    let fwver = ioctl_val(fd, WMT_IOCTL_GET_CHIP_INFO, WMT_CHIPINFO_GET_FWVER) as u32;
    println!("[launcher] srh_rom_patch: chip_id=0x{:x}, fw_ver=0x{:x}", chip_id, fwver);

    let prefix = get_rom_patch_prefix(chip_id);
    let suffix = get_patch_suffix(chip_id);
    let pattern = format!("{}_ram_*_{}*", prefix, suffix);
    println!("[launcher] srh_rom_patch: Looking for patch using glob: {}", pattern);

    let patches = find_patches(&pattern)?;
    for p in &patches {
        if p.filename.contains("ram_bt") {
            if let Some(btver) = find_bluetooth_fw_ver(&p.contents) {
                println!("[launcher] srh_rom_patch: BT firmware version: {}", btver);
            }
        } else {
            println!("[launcher] srh_rom_patch: Patch build: {}", get_patch_build_id(&p.contents));
        }

        let patchinfo = get_patch_info(&p.contents)?;
        let patchver = get_patch_fwver(&p.contents)? as u32;
        println!("[launcher] srh_rom_patch: {} patchinfo={:02x?}, patchver=0x{:x}", p.filename, patchinfo, patchver);

        if patchver != fwver {
            return Err(format!("Patch version mismatch: expected 0x{:x}, got 0x{:x}", fwver, patchver));
        }

        let req = WmtPatchInfo::new_rom(&patchinfo, &p.filename)?;
        let err = ioctl_ptr(fd, WMT_IOCTL_SET_ROM_PATCH_INFO, &req);
        if err != 0 {
            return Err(format!("srh_rom_patch: WMT_IOCTL_SET_ROM_PATCH_INFO failed (err={})", err));
        }
    }

    Ok(())
}

fn handle_srh_patch(fd: libc::c_int) -> Result<(), String> {
    let chip_id = ioctl_val(fd, WMT_IOCTL_GET_CHIP_INFO, WMT_CHIPINFO_GET_CHIPID) as u32;
    let fwver = ioctl_val(fd, WMT_IOCTL_GET_CHIP_INFO, WMT_CHIPINFO_GET_FWVER) as u32;
    println!("[launcher] srh_patch: chip_id=0x{:x}, fw_ver=0x{:x}", chip_id, fwver);

    let prefix = format!("{}_patch", get_patch_prefix(chip_id)?);
    let suffix = get_patch_suffix(chip_id);
    let pattern = format!("{}*{}*", prefix, suffix);
    println!("[launcher] srh_patch: Looking for patch using glob: {}", pattern);

    let patches = find_patches(&pattern)?;
    let mut patch_count_set = false;

    for p in &patches {
        let patchinfo = get_patch_info(&p.contents)?;
        let patchver = get_patch_fwver(&p.contents)? as u32;
        println!("[launcher] srh_patch: {} patchinfo={:02x?}, patchver=0x{:x}", p.filename, patchinfo, patchver);

        if patchver != fwver {
            return Err(format!("Patch version mismatch: expected 0x{:x}, got 0x{:x}", fwver, patchver));
        }

        if !patch_count_set {
            let patch_num = (patchinfo[0] >> 4) as libc::c_ulong;
            let err = ioctl_val(fd, WMT_IOCTL_SET_PATCH_NUM, patch_num);
            if err != 0 {
                return Err(format!("Failed to set patch count (err={})", err));
            }
            patch_count_set = true;
        }

        let req = WmtPatchInfo::new_patch(&patchinfo, &p.filename)?;
        let err = ioctl_ptr(fd, WMT_IOCTL_SET_PATCH_INFO, &req);
        if err != 0 {
            return Err(format!("srh_patch: WMT_IOCTL_SET_PATCH_INFO failed (err={})", err));
        }
    }

    Ok(())
}

pub fn do_launcher(running: Arc<AtomicBool>) -> Result<(), String> {
    println!("[launcher] Opening {}", WMT_DEV);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(WMT_DEV)
        .map_err(|e| format!("Failed to open {}: {}", WMT_DEV, e))?;
    let fd = file.as_raw_fd();

    let mut chipid: i32;
    loop {
        chipid = ioctl_val(fd, WMT_IOCTL_WMT_QUERY_CHIPID, 0);
        if chipid != -1 {
            break;
        }
        if !running.load(Ordering::Relaxed) {
            return Ok(());
        }
        eprintln!("[launcher] WMT_IOCTL_WMT_QUERY_CHIPID failed, retrying in 300ms...");
        thread::sleep(Duration::from_millis(300));
    }
    println!("[launcher] Chip ID: 0x{:x}", chipid);

    let hwver = ioctl_val(fd, WMT_IOCTL_GET_CHIP_INFO, WMT_CHIPINFO_GET_FWVER);
    println!("[launcher] HW/FW Version: 0x{:x}", hwver);

    let weird_chip_id = (chipid - 0x6620) >> 1;
    let check_id = weird_chip_id | (chipid << 0x1F);
    let stp_mode = if check_id < 10 && (((1i32 << (chipid & 0x1F)) & 0x311) != 0) {
        return Err("FIXME: Unimplemented branch check_id < 10".into());
    } else {
        3
    };

    let fm_mode = 2;
    let baudrate = 4000000;
    let config = (baudrate << 8) | ((fm_mode & 0xF) << 4) | (stp_mode & 0xF);

    let cfg_name = b"WMT_SOC.cfg\0";
    ioctl_ptr(fd, WMT_IOCTL_SET_PATCH_NAME, cfg_name.as_ptr());
    ioctl_val(fd, WMT_IOCTL_SET_STP_MODE, config as libc::c_ulong);
    ioctl_val(fd, WMT_IOCTL_SET_LAUNCHER_KILL, 0);

    // Spawn background workers
    let r1 = Arc::clone(&running);
    thread::spawn(move || pwr_on_worker(fd, r1));

    let r2 = Arc::clone(&running);
    thread::spawn(move || wifi_enable_worker(r2));

    println!("[launcher] Waiting for patch requests on {}...", WMT_DEV);

    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };

    let mut buf = [0u8; 256];

    while running.load(Ordering::Relaxed) {
        let ret = unsafe { libc::poll(&mut pfd, 1, 1000) };
        if ret < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            eprintln!("[launcher] poll error: {}", err);
            break;
        }
        if ret == 0 {
            // Timeout (1s) - allow checking running flag
            continue;
        }

        if (pfd.revents & libc::POLLIN) != 0 {
            let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 {
                eprintln!("[launcher] read returned {}", n);
                thread::sleep(Duration::from_millis(100));
                continue;
            }

            let cmd = &buf[..n as usize];
            let cmd_str = String::from_utf8_lossy(cmd);
            println!("[launcher] Received kernel command: {:?}", cmd_str.trim());

            let res = if cmd.starts_with(WMT_COMMAND_SRH_ROM_PATCH) {
                handle_srh_rom_patch(fd)
            } else if cmd.starts_with(WMT_COMMAND_SRH_PATCH) {
                handle_srh_patch(fd)
            } else {
                Err(format!("Unknown kernel command: {:?}", cmd_str))
            };

            let resp = match res {
                Ok(_) => {
                    println!("[launcher] Command handled successfully -> sending 'ok'");
                    b"ok" as &[u8]
                }
                Err(e) => {
                    eprintln!("[launcher] Command failed: {} -> sending 'fail'", e);
                    b"fail" as &[u8]
                }
            };

            unsafe {
                libc::write(fd, resp.as_ptr() as *const libc::c_void, resp.len());
            }
        }
    }

    println!("[launcher] Exiting launcher loop.");
    Ok(())
}
