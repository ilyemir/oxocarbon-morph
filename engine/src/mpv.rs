//! mpv IPC client (/tmp/mpvpaper.sock) — raw JSON over a Unix socket, zero deps.
//!
//! v1.0: wallpaper switching happens via `loadfile` IPC (zero respawn, <80ms);
//! mpvpaper is only (re)spawned when no live socket answers.

use crate::paths;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub fn available() -> bool {
    Path::new(paths::MPV_SOCKET).exists()
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}

/// Send one command. With `read_reply`, drain the socket until the JSON
/// response line carrying "error" arrives (or a short timeout hits).
fn request(cmd: &str, read_reply: bool) -> Option<String> {
    if !available() {
        return None;
    }
    let mut s = UnixStream::connect(paths::MPV_SOCKET).ok()?;
    s.write_all(format!("{}\n", cmd).as_bytes()).ok()?;
    if !read_reply {
        let _ = s.shutdown(std::net::Shutdown::Both);
        return Some(String::new());
    }
    let _ = s.set_read_timeout(Some(Duration::from_millis(250)));
    let mut buf = String::new();
    let mut chunk = [0u8; 2048];
    let deadline = Instant::now() + Duration::from_millis(400);
    while Instant::now() < deadline {
        match s.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.push_str(&String::from_utf8_lossy(&chunk[..n]));
                if buf.contains("\"error\"") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let _ = s.shutdown(std::net::Shutdown::Both);
    Some(buf)
}

fn fire(cmd: &str) -> bool {
    request(cmd, false).is_some()
}

pub fn set_pause(pause: bool) -> bool {
    fire(&format!(
        "{{\"command\": [\"set_property\", \"pause\", {}]}}",
        pause
    ))
}

/// Instant wallpaper switch on the live mpvpaper instance.
/// Returns false when there is no answering socket (caller falls back to respawn).
pub fn loadfile(path: &str) -> bool {
    let cmd = format!(
        "{{\"command\": [\"loadfile\", \"{}\", \"replace\"]}}",
        json_escape(path)
    );
    match request(&cmd, true) {
        Some(reply) => reply.contains("\"error\":\"success\""),
        None => false,
    }
}

/// Currently playing file (diagnostics / tests).
pub fn playing_path() -> Option<String> {
    let reply = request("{\"command\": [\"get_property\", \"path\"]}", true)?;
    let rest = reply.split("\"data\":\"").nth(1)?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Spawn mpvpaper on every output with the v1.0 option set.
/// `-f` forks, so `status()` returns as soon as the daemonized child is up.
pub fn spawn(wall: &str) -> Result<(), String> {
    let _ = std::fs::remove_file(paths::MPV_SOCKET);
    let path = paths::wall_dir().join(wall);
    if !path.exists() {
        return Err(format!("duvar kagidi yok: {}", path.display()));
    }
    let opts = format!(
        "--input-ipc-server={} --hwdec=vaapi --loop-file=inf --no-audio --demuxer-readahead-secs=0.5",
        paths::MPV_SOCKET
    );
    let status = Command::new("mpvpaper")
        .arg("-f")
        .arg("-o")
        .arg(&opts)
        .arg("*")
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("mpvpaper: {}", e))?;
    if !status.success() {
        return Err(format!("mpvpaper exited with {}", status));
    }
    Ok(())
}

/// Block until the mpv IPC socket appears (one-shot; used right after spawn).
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
