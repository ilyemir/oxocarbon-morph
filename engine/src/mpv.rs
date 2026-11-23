//! mpv IPC client (/tmp/mpv-socket) — raw JSON over a Unix socket, zero deps.

use crate::paths;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

pub fn available() -> bool {
    Path::new(paths::MPV_SOCKET).exists()
}

fn send(cmd: &str) -> bool {
    if !available() {
        return false;
    }
    match UnixStream::connect(paths::MPV_SOCKET) {
        Ok(mut s) => {
            let ok = s
                .write_all(format!("{}\n", cmd).as_bytes())
                .is_ok();
            let _ = s.shutdown(std::net::Shutdown::Both);
            ok
        }
        Err(_) => false,
    }
}

pub fn set_pause(pause: bool) -> bool {
    send(&format!(
        "{{\"command\": [\"set_property\", \"pause\", {}]}}",
        pause
    ))
}

/// Block until the mpv IPC socket appears (one-shot; used right after mpvpaper spawn).
pub fn wait_socket(timeout_ms: u64) -> bool {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < deadline {
        if available() {
            return true;
        }
        thread::sleep(Duration::from_millis(25));
    }
    available()
}
