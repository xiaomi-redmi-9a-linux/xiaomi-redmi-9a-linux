#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

pub const PATCH_LOOKUP_DIRECTORY: &str = "/lib/firmware";

#[derive(Debug, Clone)]
pub struct Patch {
    pub contents: Vec<u8>,
    pub path: PathBuf,
    pub filename: String,
}

pub fn glob_match(pattern: &str, s: &str) -> bool {
    let p_bytes = pattern.as_bytes();
    let s_bytes = s.as_bytes();
    let (p_len, s_len) = (p_bytes.len(), s_bytes.len());
    let mut px = 0;
    let mut sx = 0;
    let mut next_px = 0;
    let mut next_sx = 0;

    while sx < s_len {
        if px < p_len && (p_bytes[px] == b'?' || p_bytes[px] == s_bytes[sx]) {
            px += 1;
            sx += 1;
        } else if px < p_len && p_bytes[px] == b'*' {
            next_px = px + 1;
            next_sx = sx + 1;
            px += 1;
        } else if next_px > 0 {
            px = next_px;
            sx = next_sx;
            next_sx += 1;
        } else {
            return false;
        }
    }
    while px < p_len && p_bytes[px] == b'*' {
        px += 1;
    }
    px == p_len
}

pub fn find_patches(pattern: &str) -> Result<Vec<Patch>, String> {
    let dir = Path::new(PATCH_LOOKUP_DIRECTORY);
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read {}: {}", PATCH_LOOKUP_DIRECTORY, e))?;

    let mut patches = Vec::new();
    for entry in entries.flatten() {
        if let Ok(file_name) = entry.file_name().into_string() {
            if glob_match(pattern, &file_name) {
                let path = entry.path();
                match fs::read(&path) {
                    Ok(contents) => {
                        patches.push(Patch {
                            contents,
                            path,
                            filename: file_name,
                        });
                    }
                    Err(e) => {
                        eprintln!("[warn] Failed to read firmware file {:?}: {}", path, e);
                    }
                }
            }
        }
    }

    if patches.is_empty() {
        return Err(format!("Failed to find patch matching glob: {}", pattern));
    }

    patches.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(patches)
}

pub fn get_patch_info(data: &[u8]) -> Result<[u8; 8], String> {
    if data.len() < 0x20 {
        return Err("Patch data too short".into());
    }
    let mut info = [0u8; 8];
    info.copy_from_slice(&data[0x18..0x20]);
    if info[0] != 0x11 {
        return Err(format!("patchinfo[0] is not 0x11 (got 0x{:02x})", info[0]));
    }
    Ok(info)
}

pub fn get_patch_fwver(data: &[u8]) -> Result<u16, String> {
    get_patch_info(data)?;
    if data.len() < 0x18 {
        return Err("Patch data too short for fwver".into());
    }
    Ok(((data[0x16] as u16) << 8) | (data[0x17] as u16))
}

pub fn get_patch_build_id(data: &[u8]) -> String {
    let slice = if data.len() >= 16 { &data[..16] } else { data };
    String::from_utf8_lossy(slice).trim_end_matches('\0').trim().to_string()
}

pub fn find_bluetooth_fw_ver(data: &[u8]) -> Option<String> {
    let babeface = b"BABEFACE";
    let deadbeef = b"DEADBEEF";
    let t_neptune = b"t-neptune";

    let babeface_pos = data.windows(babeface.len()).position(|w| w == babeface)?;
    let slice_after = &data[babeface_pos..];
    let deadbeef_rel = slice_after.windows(deadbeef.len()).position(|w| w == deadbeef)?;
    let deadbeef_pos = babeface_pos + deadbeef_rel;

    let sub = &data[babeface_pos..deadbeef_pos];
    let t_nep_rel = sub.windows(t_neptune.len()).position(|w| w == t_neptune);

    let (start, end) = match t_nep_rel {
        Some(pos) => (babeface_pos + pos, deadbeef_pos),
        None => {
            let debug_needle = b"= debug";
            if sub.windows(debug_needle.len()).position(|w| w == debug_needle).is_some() {
                (0, 0x0E.min(data.len()))
            } else {
                return None;
            }
        }
    };

    let slice = &data[start..end];
    let term_pos = slice.iter().position(|&b| b == b'\n' || b == 0).unwrap_or(slice.len());
    Some(String::from_utf8_lossy(&slice[..term_pos]).trim().to_string())
}

pub fn get_rom_patch_prefix(chip_id: u32) -> &'static str {
    match chip_id {
        0x6779 | 0x6873 | 0x6853 => "soc2_0",
        0x6781 | 0x6833 => "soc2_2",
        _ => "soc1_0",
    }
}

pub fn get_patch_prefix(chip_id: u32) -> Result<String, String> {
    match chip_id {
        0x6572 | 0x6582 | 0x6592 | 0x6595 => Ok("ROMv1".into()),
        0x8127 => Ok("ROMv2".into()),
        0x321 | 0x326 | 0x335 | 0x337 | 0x6735 | 0x6739 | 0x6752 | 0x6755 | 0x6757 | 0x6763 => {
            Ok("ROMv2_lm".into())
        }
        0x279 => Ok("ROMv3".into()),
        0x507 | 0x6759 => Ok("ROMv4".into()),
        0x6771 | 0x6775 => Ok("ROMv4_be".into()),
        0x6761 | 0x6765 | 0x6768 | 0x6785 | 0x3967 | 0x8163 => Ok("soc1_0".into()),
        0x6779 | 0x6853 | 0x6873 => Ok("soc2_0".into()),
        _ => {
            if (0x6570..=0x6592).contains(&chip_id) {
                Ok("ROMv1".into())
            } else if (0x6736..=0x6796).contains(&chip_id) {
                Ok(format!("mt{:x}", chip_id))
            } else {
                Err(format!("Unknown patch prefix for chip_id 0x{:x}", chip_id))
            }
        }
    }
}

pub fn get_patch_suffix(_chip_id: u32) -> &'static str {
    "1_1"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match() {
        assert!(glob_match("soc1_0_ram_*_1_1*", "soc1_0_ram_bt_1_1_hdr.bin"));
        assert!(glob_match("soc1_0_ram_*_1_1*", "soc1_0_ram_mcu_1_1_hdr.bin"));
        assert!(glob_match("soc1_0_ram_*_1_1*", "soc1_0_ram_wifi_1_1_hdr.bin"));
        assert!(!glob_match("soc1_0_ram_*_1_1*", "soc1_0_patch_mcu_1_1_hdr.bin"));
        assert!(glob_match("soc1_0_patch*1_1*", "soc1_0_patch_mcu_1_1_hdr.bin"));
    }
}
