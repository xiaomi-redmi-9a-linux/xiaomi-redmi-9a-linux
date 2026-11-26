//! MediaTek MT6631 Bluetooth Userspace VHCI Bridge (`mtk-bt-bridge`)
//!
//! Bridges the Linux kernel Virtual HCI interface (`/dev/vhci`) to the
//! MediaTek Serial Transport Protocol character device (`/dev/stpbt`).
//!
//! Provides hardware power lifecycle management via WMT and transparent
//! quirk handling for MT6631 controller limitations.

use std::ffi::CString;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Duration;

static RUNNING: AtomicBool = AtomicBool::new(true);
static SIGNAL_PIPE_WR: AtomicI32 = AtomicI32::new(-1);

extern "C" fn handle_signal(_sig: libc::c_int) {
    RUNNING.store(false, Ordering::SeqCst);
    let wr = SIGNAL_PIPE_WR.load(Ordering::Relaxed);
    if wr >= 0 {
        let byte: u8 = 1;
        unsafe { libc::write(wr, &byte as *const u8 as *const libc::c_void, 1) };
    }
}

fn write_all(fd: libc::c_int, data: &[u8]) -> Result<(), String> {
    let mut offset = 0;
    while offset < data.len() {
        let ret = unsafe {
            libc::write(
                fd,
                data[offset..].as_ptr() as *const libc::c_void,
                data.len() - offset,
            )
        };
        if ret > 0 {
            offset += ret as usize;
        } else if ret < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            } else if err.raw_os_error() == Some(libc::EAGAIN)
                || err.raw_os_error() == Some(libc::EWOULDBLOCK)
            {
                // Wait briefly for descriptor to become writable
                let mut pfd = libc::pollfd {
                    fd,
                    events: libc::POLLOUT,
                    revents: 0,
                };
                unsafe { libc::poll(&mut pfd, 1, 100) };
            } else {
                return Err(format!("write error on fd {}: {}", fd, err));
            }
        }
    }
    Ok(())
}

fn open_nonblock(path: &str, flags: libc::c_int) -> Result<libc::c_int, String> {
    let c_path = CString::new(path).map_err(|e| e.to_string())?;
    let fd = unsafe { libc::open(c_path.as_ptr(), flags | libc::O_NONBLOCK) };
    if fd < 0 {
        return Err(format!(
            "Failed to open {}: {}",
            path,
            std::io::Error::last_os_error()
        ));
    }
    Ok(fd)
}

fn main() {
    println!("[mtk-bt-bridge] Starting MediaTek MT6631 Bluetooth VHCI bridge...");

    let mut sig_fds = [-1i32; 2];
    if unsafe { libc::pipe(sig_fds.as_mut_ptr()) } == 0 {
        unsafe {
            libc::fcntl(sig_fds[0], libc::F_SETFL, libc::O_NONBLOCK);
            libc::fcntl(sig_fds[1], libc::F_SETFL, libc::O_NONBLOCK);
        }
        SIGNAL_PIPE_WR.store(sig_fds[1], Ordering::SeqCst);
    }

    // Register signal handlers for clean shutdown
    unsafe {
        libc::signal(libc::SIGINT, handle_signal as *const () as usize);
        libc::signal(libc::SIGTERM, handle_signal as *const () as usize);
        libc::signal(libc::SIGHUP, handle_signal as *const () as usize);
    }

    // Step 1: Open /dev/vhci
    let vhci_fd = match open_nonblock("/dev/vhci", libc::O_RDWR) {
        Ok(fd) => fd,
        Err(e) => {
            eprintln!("[mtk-bt-bridge] Error: {}", e);
            std::process::exit(1);
        }
    };

    // Step 2: Register Primary HCI device by writing [0xff, 0x00]
    // 0xff = HCI_VENDOR_PKT, 0x00 = HCI_PRIMARY
    let init_pkt: [u8; 2] = [0xff, 0x00];
    if let Err(e) = write_all(vhci_fd, &init_pkt) {
        eprintln!("[mtk-bt-bridge] Failed to register VHCI controller: {}", e);
        unsafe { libc::close(vhci_fd) };
        std::process::exit(1);
    }

    // Read the 4-byte response from VHCI ([0xff, 0x00, dev_id_lo, dev_id_hi])
    let mut vhci_resp = [0u8; 64];
    let mut got_resp = false;
    for _ in 0..20 {
        let n = unsafe {
            libc::read(
                vhci_fd,
                vhci_resp.as_mut_ptr() as *mut libc::c_void,
                vhci_resp.len(),
            )
        };
        if n >= 4 {
            let dev_id = (vhci_resp[2] as u16) | ((vhci_resp[3] as u16) << 8);
            println!(
                "[mtk-bt-bridge] Registered Linux Bluetooth controller hci{}",
                dev_id
            );
            got_resp = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    if !got_resp {
        eprintln!("[mtk-bt-bridge] Warning: Did not receive instant registration confirmation from /dev/vhci");
    }

    // Step 3: Open /dev/stpbt (MTK Bluetooth STP character device)
    // Opening this triggers BT_open() in the kernel driver, powering on MT6631 hardware via WMT
    let mut stpbt_fd = -1;
    for attempt in 1..=10 {
        match open_nonblock("/dev/stpbt", libc::O_RDWR) {
            Ok(fd) => {
                stpbt_fd = fd;
                break;
            }
            Err(e) => {
                if attempt == 10 {
                    eprintln!("[mtk-bt-bridge] Failed to open /dev/stpbt after 10 attempts: {}", e);
                    unsafe { libc::close(vhci_fd) };
                    std::process::exit(1);
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
    println!("[mtk-bt-bridge] Opened /dev/stpbt (MT6631 hardware powered on via WMT)");

    let mut buf = [0u8; 4096];
    println!("[mtk-bt-bridge] Entering active event loop...");

    while RUNNING.load(Ordering::Relaxed) {
        let mut poll_fds = [
            libc::pollfd {
                fd: vhci_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: stpbt_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: sig_fds[0],
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let nfds = if sig_fds[0] >= 0 { 3 } else { 2 };
        let timeout_ms = if sig_fds[0] >= 0 { -1 } else { 1000 };
        let ret = unsafe { libc::poll(poll_fds.as_mut_ptr(), nfds, timeout_ms) };
        if ret < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            eprintln!("[mtk-bt-bridge] poll error: {}", err);
            break;
        }
        if nfds == 3 && (poll_fds[2].revents & libc::POLLIN != 0) {
            break;
        }

        // Handle packets from Kernel Virtual HCI -> MediaTek STP
        if poll_fds[0].revents & libc::POLLIN != 0 {
            let n = unsafe {
                libc::read(
                    vhci_fd,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                )
            };
            if n > 0 {
                let bytes = &buf[..n as usize];

                // Quirk 1: Intercept HCI_OP_READ_SYNC_TRAIN_PARAMS (0x0c77)
                // MT6631 firmware rejects this command with Unknown HCI Command (0x01),
                // which causes kernel __hci_init to abort. Synthesize immediate success.
                if bytes.len() >= 4 && bytes[0] == 0x01 {
                    let opcode = (bytes[1] as u16) | ((bytes[2] as u16) << 8);
                    if opcode == 0x0c77 {
                        let fake_resp: [u8; 11] = [
                            0x04, 0x0e, 0x08, 0x01, 0x77, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00,
                        ];
                        let _ = write_all(vhci_fd, &fake_resp);
                        continue;
                    }
                }

                // Forward packet to hardware STP
                if let Err(e) = write_all(stpbt_fd, bytes) {
                    eprintln!("[mtk-bt-bridge] Error forwarding to /dev/stpbt: {}", e);
                }
            }
        }

        // Handle packets from MediaTek STP -> Kernel Virtual HCI
        if poll_fds[1].revents & libc::POLLIN != 0 {
            let n = unsafe {
                libc::read(
                    stpbt_fd,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                )
            };
            if n > 0 {
                let mut data = buf[..n as usize].to_vec();

                // Quirk 2: Patch Read Local Extended Features (Page 2) Command Complete Event
                // MT6631 sets bit 2 of features[0] (LMP_SYNC_TRAIN = 0x04) in Page 2, but
                // does not actually support Synchronization Train. Clear bit 2 so kernel
                // skips issuing HCI_OP_READ_SYNC_TRAIN_PARAMS completely.
                if data.len() >= 10 && data[0] == 0x04 && data[1] == 0x0e {
                    let opcode = (data[4] as u16) | ((data[5] as u16) << 8);
                    if opcode == 0x1004 && data[7] == 0x02 {
                        // Page 2 Extended Features: features[0] is at index 9
                        data[9] &= !0x04; // Clear LMP_SYNC_TRAIN
                    }
                }

                // Forward packet to kernel Virtual HCI
                if let Err(e) = write_all(vhci_fd, &data) {
                    eprintln!("[mtk-bt-bridge] Error forwarding to /dev/vhci: {}", e);
                }
            }
        }

        // Check for error conditions on descriptors
        if poll_fds[0].revents & (libc::POLLERR | libc::POLLHUP) != 0 {
            eprintln!("[mtk-bt-bridge] /dev/vhci hung up or errored");
            break;
        }
        if poll_fds[1].revents & (libc::POLLERR | libc::POLLHUP) != 0 {
            eprintln!("[mtk-bt-bridge] /dev/stpbt hung up or errored");
            break;
        }
    }

    println!("[mtk-bt-bridge] Shutting down bridge and releasing hardware...");
    unsafe {
        libc::close(stpbt_fd); // Triggers BT_close() to power off MT6631 radio
        libc::close(vhci_fd);  // Unregisters hci0 from kernel
        if sig_fds[0] >= 0 {
            libc::close(sig_fds[0]);
        }
        if sig_fds[1] >= 0 {
            libc::close(sig_fds[1]);
        }
    }
    println!("[mtk-bt-bridge] Bridge stopped cleanly.");
}
