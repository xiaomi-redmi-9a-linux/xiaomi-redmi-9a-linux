#![allow(dead_code)]

use std::os::unix::io::RawFd;

pub const DETECT_NODE: &str = "/dev/wmtdetect";
pub const WMT_DEV: &str = "/dev/stpwmt";
pub const WMT_WIFI: &str = "/dev/wmtWifi";
pub const PATCH_LOOKUP_DIRECTORY: &str = "/lib/firmware";

// Loader ioctls (_IOR('w', nr, int), _IOW('w', nr, int))
pub const COMBO_IOCTL_GET_CHIP_ID: libc::c_ulong = 0x80047700;
pub const COMBO_IOCTL_SET_CHIP_ID: libc::c_ulong = 0x40047701;
pub const COMBO_IOCTL_GET_SOC_CHIP_ID: libc::c_ulong = 0x80047703;
pub const COMBO_IOCTL_DO_MODULE_INIT: libc::c_ulong = 0x80047704;
pub const COMBO_IOCTL_MODULE_CLEANUP: libc::c_ulong = 0x80047705;
pub const COMBO_IOCTL_EXT_CHIP_PWR_ON: libc::c_ulong = 0x80047706;
pub const COMBO_IOCTL_EXT_CHIP_PWR_OFF: libc::c_ulong = 0x80047707;
pub const COMBO_IOCTL_DO_SDIO_AUDOK: libc::c_ulong = 0x80047708;
pub const COMBO_IOCTL_GET_ADIE_CHIP_ID: libc::c_ulong = 0x80047709;
pub const COMBO_IOCTL_CONNSYS_SOC_HW_INIT: libc::c_ulong = 0x8004770a;

// Launcher ioctls (_IOR(0xA0, nr, ...), _IOW(0xA0, nr, ...))
pub const WMT_IOCTL_SET_PATCH_NAME: libc::c_ulong = 0x4008a004;
pub const WMT_IOCTL_SET_STP_MODE: libc::c_ulong = 0x4004a005;
pub const WMT_IOCTL_LPBK_POWER_CTRL: libc::c_ulong = 0x4004a007;
pub const WMT_IOCTL_GET_CHIP_INFO: libc::c_ulong = 0x8004a00c;
pub const WMT_IOCTL_SET_LAUNCHER_KILL: libc::c_ulong = 0x4004a00d;
pub const WMT_IOCTL_SET_PATCH_NUM: libc::c_ulong = 0x4004a00e;
pub const WMT_IOCTL_SET_PATCH_INFO: libc::c_ulong = 0x4008a00f;
pub const WMT_IOCTL_WMT_QUERY_CHIPID: libc::c_ulong = 0x8004a016;
pub const WMT_IOCTL_SET_ROM_PATCH_INFO: libc::c_ulong = 0x4008a01f;

pub const WMT_CHIPINFO_GET_CHIPID: libc::c_ulong = 0;
pub const WMT_CHIPINFO_GET_HWVER: libc::c_ulong = 1;
pub const WMT_CHIPINFO_GET_FWVER: libc::c_ulong = 2;
pub const WMT_CHIPINFO_GET_IPVER: libc::c_ulong = 3;
pub const WMT_CHIPINFO_GET_CHIP_TYPE: libc::c_ulong = 4;

/// Matches Linux kernel struct WMT_PATCH_INFO and struct wmt_rom_patch_info (264 bytes)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct WmtPatchInfo {
    pub patch_type_or_seq: u32,
    pub address: [u8; 4],
    pub patch_name: [u8; 256],
}

impl WmtPatchInfo {
    pub fn new_rom(patchinfo: &[u8], filename: &str) -> Result<Self, String> {
        if patchinfo.len() != 8 {
            return Err(format!("Invalid patchinfo length: {}", patchinfo.len()));
        }
        if patchinfo[3] != 0xF0 {
            return Err(format!("Patchinfo byte 3 check failed: 0x{:02x} != 0xF0", patchinfo[3]));
        }
        if patchinfo[7] >= 6 {
            return Err(format!("Patch info type invalid: {} >= 6", patchinfo[7]));
        }
        let mut patch_name = [0u8; 256];
        let bytes = filename.as_bytes();
        if bytes.len() > 255 {
            return Err(format!("Patch filename exceeds 255 bytes: {}", filename));
        }
        patch_name[..bytes.len()].copy_from_slice(bytes);

        Ok(Self {
            patch_type_or_seq: patchinfo[7] as u32,
            address: [0x00, patchinfo[1], patchinfo[2], patchinfo[3]],
            patch_name,
        })
    }

    pub fn new_patch(patchinfo: &[u8], filename: &str) -> Result<Self, String> {
        let mut patch_name = [0u8; 256];
        let bytes = filename.as_bytes();
        if bytes.len() > 255 {
            return Err(format!("Patch filename exceeds 255 bytes: {}", filename));
        }
        patch_name[..bytes.len()].copy_from_slice(bytes);

        Ok(Self {
            patch_type_or_seq: (patchinfo[0] & 0x0F) as u32,
            address: [0x00, patchinfo[1], patchinfo[2], patchinfo[3]],
            patch_name,
        })
    }
}

pub fn ioctl_val(fd: RawFd, cmd: libc::c_ulong, arg: libc::c_ulong) -> i32 {
    let ret = unsafe { libc::ioctl(fd, cmd as libc::Ioctl, arg) };
    if ret < 0 {
        let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(1);
        -err
    } else {
        ret
    }
}

pub fn ioctl_ptr<T>(fd: RawFd, cmd: libc::c_ulong, ptr: *const T) -> i32 {
    let ret = unsafe { libc::ioctl(fd, cmd as libc::Ioctl, ptr) };
    if ret < 0 {
        let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(1);
        -err
    } else {
        ret
    }
}
