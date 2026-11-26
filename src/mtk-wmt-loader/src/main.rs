mod ioctl;
mod launcher;
mod loader;
mod patch;

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

static RUNNING: AtomicBool = AtomicBool::new(true);
static SIGNAL_PIPE_WR: AtomicI32 = AtomicI32::new(-1);

extern "C" fn sig_handler(_sig: libc::c_int) {
    RUNNING.store(false, Ordering::SeqCst);
    let wr = SIGNAL_PIPE_WR.load(Ordering::Relaxed);
    if wr >= 0 {
        let byte: u8 = 1;
        unsafe { libc::write(wr, &byte as *const u8 as *const libc::c_void, 1) };
    }
}

fn install_signals() {
    unsafe {
        libc::signal(libc::SIGINT, sig_handler as *const () as usize);
        libc::signal(libc::SIGTERM, sig_handler as *const () as usize);
    }
}

fn main() {
    println!("[mtk-wmt-loader] Starting MediaTek WMT native loader daemon...");
    let mut sig_fds = [-1i32; 2];
    if unsafe { libc::pipe(sig_fds.as_mut_ptr()) } == 0 {
        unsafe {
            libc::fcntl(sig_fds[0], libc::F_SETFL, libc::O_NONBLOCK);
            libc::fcntl(sig_fds[1], libc::F_SETFL, libc::O_NONBLOCK);
        }
        SIGNAL_PIPE_WR.store(sig_fds[1], Ordering::SeqCst);
    }
    install_signals();

    let running = Arc::new(AtomicBool::new(true));

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
    let res = launcher::do_launcher(running, &RUNNING, sig_fds[0]);
    if sig_fds[0] >= 0 {
        unsafe {
            libc::close(sig_fds[0]);
            libc::close(sig_fds[1]);
        }
    }
    match res {
        Ok(_) => {
            println!("[mtk-wmt-loader] Daemon stopped cleanly.");
        }
        Err(e) => {
            eprintln!("[mtk-wmt-loader] Launcher error: {}", e);
            std::process::exit(1);
        }
    }
}
