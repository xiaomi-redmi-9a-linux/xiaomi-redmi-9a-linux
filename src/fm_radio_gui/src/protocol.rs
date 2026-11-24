use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::Duration;

pub const SOCK_PATH: &str = "/run/mtk_fm.sock";

#[derive(Debug, Clone)]
pub struct RadioStatus {
    pub is_playing: bool,
    pub freq: f32,
    pub rssi: i32,
    pub route: String,
    pub vol: i32,
}

pub fn send_cmd(cmd: &str, timeout_secs: u64) -> Option<String> {
    let mut stream = UnixStream::connect(SOCK_PATH).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(timeout_secs))).ok()?;
    stream.set_write_timeout(Some(Duration::from_secs(3))).ok()?;

    let cmd_line = format!("{}\n", cmd);
    stream.write_all(cmd_line.as_bytes()).ok()?;

    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).ok()?;
    if n == 0 {
        return None;
    }

    String::from_utf8(buf[..n].to_vec()).ok().map(|s| s.trim().to_string())
}

pub fn ensure_daemon() {
    if send_cmd("STATUS", 1).is_none() {
        let _ = Command::new("/usr/local/bin/mtk_fm")
            .arg("--daemon")
            .spawn();
        std::thread::sleep(Duration::from_millis(400));
    }
}

pub fn parse_status(resp: &str) -> Option<RadioStatus> {
    let parts: Vec<&str> = resp.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    if parts[0] == "PLAYING" {
        // Format: PLAYING <freq> <rssi> <route> <vol>
        if parts.len() >= 5 {
            let freq = parts[1].parse::<f32>().unwrap_or(98.3);
            let rssi = parts[2].parse::<i32>().unwrap_or(-100);
            let route = parts[3].to_string();
            let vol = parts[4].parse::<i32>().unwrap_or(75);
            return Some(RadioStatus {
                is_playing: true,
                freq,
                rssi,
                route,
                vol,
            });
        }
    } else if parts[0] == "STOPPED" {
        // Format: STOPPED <freq> <route> <vol>
        if parts.len() >= 4 {
            let freq = parts[1].parse::<f32>().unwrap_or(98.3);
            let route = parts[2].to_string();
            let vol = parts[3].parse::<i32>().unwrap_or(75);
            return Some(RadioStatus {
                is_playing: false,
                freq,
                rssi: -100,
                route,
                vol,
            });
        }
    }

    None
}
