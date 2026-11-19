use crate::ioctl::*;
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;

pub fn identify_chip_type_magic(err: i32, chipid: u32) -> Option<u32> {
    let fallback = chipid;
    if err < 0x507 {
        if err < 0x321 + 0x17 {
            if err == 0x321 || err == 0x335 || err == 0x337 {
                return Some(0x6735);
            }
            if err == 0x326 {
                return Some(0x6755);
            }
            Some(fallback)
        } else {
            if err == -1 {
                eprintln!("[loader] Chip ID error: err(-1)");
                return None;
            }
            if err == 0x279 {
                return Some(0x6797);
            }
            Some(fallback)
        }
    } else if err < 0x690 {
        if err == 0x507 {
            return Some(0x6759);
        }
        if err == 0x551 {
            return Some(0x6757);
        }
        Some(fallback)
    } else {
        if err == 0x690 {
            return Some(0x6763);
        }
        if err == 0x713 {
            return Some(0x6775);
        }
        if err == 0x788 {
            return Some(0x6771);
        }
        Some(fallback)
    }
}

pub fn do_loader() -> Result<u32, String> {
    println!("[loader] Opening {}", DETECT_NODE);
    let file = OpenOptions::new()
        .write(true)
        .read(true)
        .open(DETECT_NODE)
        .map_err(|e| format!("Failed to open {}: {}", DETECT_NODE, e))?;
    let fd = file.as_raw_fd();

    // HW initialization
    let err = ioctl_val(fd, COMBO_IOCTL_CONNSYS_SOC_HW_INIT, 0);
    if err == 0 {
        println!("[loader] COMBO_IOCTL_CONNSYS_SOC_HW_INIT initialized");
    }

    let chipid: u32 = loop {
        let err = ioctl_val(fd, COMBO_IOCTL_EXT_CHIP_PWR_ON, 0);
        let (is_inited, chip_id_ioctl) = if err == 0 {
            return Err("Unimplemented branch: ioctl(COMBO_IOCTL_EXT_CHIP_PWR_ON) with err(0)".into());
        } else if err == -1 {
            // SOC chip: no need for external combo chip power on
            (true, COMBO_IOCTL_GET_SOC_CHIP_ID)
        } else {
            return Err(format!("External combo power-on failed with err({})", err));
        };

        let raw_chipid = ioctl_val(fd, chip_id_ioctl, 0);
        let id = raw_chipid as u32;
        println!("[loader] Detected chip ID: 0x{:x}", id);

        if is_inited {
            break id;
        }

        let err = ioctl_val(fd, COMBO_IOCTL_DO_SDIO_AUDOK, id as libc::c_ulong);
        if err != 0 {
            return Err(format!("COMBO_IOCTL_DO_SDIO_AUDOK failed with err({})", err));
        }

        let err = ioctl_val(fd, COMBO_IOCTL_EXT_CHIP_PWR_OFF, 0);
        if err != 0 {
            return Err(format!("External combo chip power off failed with err({})", err));
        }
    };

    let err = ioctl_val(fd, COMBO_IOCTL_SET_CHIP_ID, chipid as libc::c_ulong);
    let chip_type = identify_chip_type_magic(err, chipid);

    match chip_type {
        None => {
            eprintln!("[loader] Warning: Invalid loaderfd: 0x{:x}", err);
        }
        Some(ct) => {
            let err = ioctl_val(fd, COMBO_IOCTL_MODULE_CLEANUP, ct as libc::c_ulong);
            if err == 0 {
                let err = ioctl_val(fd, COMBO_IOCTL_DO_MODULE_INIT, ct as libc::c_ulong);
                if err == 0 {
                    println!("[loader] COMBO_IOCTL_DO_MODULE_INIT success!");
                } else {
                    eprintln!("[loader] COMBO_IOCTL_DO_MODULE_INIT failed: err 0x{:x}", err);
                }
            } else {
                eprintln!("[loader] COMBO_IOCTL_MODULE_CLEANUP failed: err({})", err);
            }
        }
    }

    let adie_chipid = ioctl_val(fd, COMBO_IOCTL_GET_ADIE_CHIP_ID, 0);
    if adie_chipid == -1 {
        return Err(format!("Get ADIE chip ID failed: err(0x{:x})", adie_chipid));
    }
    println!("[loader] ADIE chip ID: 0x{:x}", adie_chipid);

    Ok(chipid)
}
