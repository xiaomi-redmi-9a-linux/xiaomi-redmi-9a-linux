mod ioctl;
mod launcher;
mod loader;
mod patch;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

static RUNNING: AtomicBool = AtomicBool::new(true);

extern "C" fn sig_handler(_sig: libc::c_int) {
    RUNNING.store(false, Ordering::Relaxed);
}

fn install_signals() {
    unsafe {
        libc::signal(libc::SIGINT, sig_handler as *const () as usize);
        libc::signal(libc::SIGTERM, sig_handler as *const () as usize);
    }
}

fn main() {
    println!("[mtk-wmt-loader] Starting MediaTek WMT native loader daemon...");
    install_signals();

    let running = Arc::new(AtomicBool::new(true));
    let r_clone = Arc::clone(&running);

    // Thread monitoring our static RUNNING flag to propagate to Arc
    std::thread::spawn(move || {
        while RUNNING.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        r_clone.store(false, Ordering::Relaxed);
    });

    // Step 1: Loader (/dev/wmtdetect)
    match loader::do_loader() {
        Ok(chip_id) => {
            println!("[mtk-wmt-loader] Loader stage completed (chip ID: 0x{:x})", chip_id);
        }
        Err(e) => {
            eprintln!("[mtk-wmt-loader] Loader stage failed: {}", e);
            std::process::exit(1);
        }
    }

    // Step 2: Launcher (/dev/stpwmt)
    match launcher::do_launcher(running) {
        Ok(_) => {
            println!("[mtk-wmt-loader] Daemon stopped cleanly.");
        }
        Err(e) => {
            eprintln!("[mtk-wmt-loader] Launcher error: {}", e);
            std::process::exit(1);
        }
    }
}
