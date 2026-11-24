//! Event-driven daemon: battery shield + Windows-style minimize pocket.
//!
//! Blocks on Hyprland's .socket2.sock (0% idle CPU) and reacts to:
//! - window/workspace events  -> recompute the mpv pause state (battery shield)
//! - `minimized>>addr,1|0`    -> hide to / restore from special:magic
//!   (Hyprland 0.53 foreign-toplevel minimize only posts this event; the
//!   actual hiding is the daemon's `movetoworkspacesilent special:magic`)
//! - `activewindowv2>>addr`   -> taskbar activate on a pocketed window pulls
//!   it back to the focused workspace (the overlay closes itself once empty)
//!
//! `theme magic toggle` (Super+S) marks its intent via a marker file so the
//! daemon never interferes while the user deliberately opens the pocket.

use crate::{hypr, mpv, paths, state::State};
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    ManualPause,
    ShieldOff,
    NoHyprland,
    Windows(usize),
}

impl Decision {
    pub fn paused(&self) -> bool {
        match self {
            Decision::ManualPause => true,
            Decision::ShieldOff => false,
            Decision::NoHyprland => false,
            Decision::Windows(n) => *n > 0,
        }
    }
}

/// Single source of truth for the pause decision. Used by the daemon,
/// `theme pause` and `theme shield` so behavior is always consistent.
pub fn recompute() -> Decision {
    let st = State::load();
    let d = if st.paused {
        Decision::ManualPause
    } else if !st.shield {
        Decision::ShieldOff
    } else {
        match hypr::active_workspace() {
            None => Decision::NoHyprland,
            Some(ws) => Decision::Windows(hypr::windows_on_workspace(ws).unwrap_or(0)),
        }
    };
    if !matches!(d, Decision::NoHyprland) {
        mpv::set_pause(d.paused());
    }
    d
}

fn should_react(event: &str) -> bool {
    [
        "openwindow",
        "closewindow",
        "movewindow",
        "workspace",
        "createworkspace",
        "destroyworkspace",
        "moveworkspace",
        "focusedmon",
        "fullscreen",
        "changefloatingmode",
        "monitoradded",
        "monitorremoved",
        "pin",
        "togglegroup",
        "moveintogroup",
        "moveoutofgroup",
    ]
    .iter()
    .any(|p| event.starts_with(p))
}

// --- special:magic pocket ---------------------------------------------------

fn marker_present() -> bool {
    paths::magic_marker().exists()
}

/// `5e17f1f602a0` (socket2 style) -> `0x5e17f1f602a0` (hyprctl style).
fn normalize_addr(addr: &str) -> Option<String> {
    let a = addr.trim().to_lowercase();
    if a.is_empty() {
        return None;
    }
    if a.starts_with("0x") {
        Some(a)
    } else {
        Some(format!("0x{}", a))
    }
}

/// Move a pocketed window back to the focused workspace and focus it.
/// The special:magic overlay closes itself once it runs empty.
fn restore_window(addr: &str) {
    if !hypr::window_in_special(addr) {
        return;
    }
    let Some(ws) = hypr::focused_workspace_name() else {
        return;
    };
    hypr::dispatch(&format!("movetoworkspacesilent {},address:{}", ws, addr));
    hypr::dispatch(&format!("focuswindow address:{}", addr));
}

/// `minimized>>addr,1|0` — waybar's taskbar minimize-raise lands here.
fn on_minimized(payload: &str) {
    let Some((addr, state)) = payload.split_once(',') else {
        return;
    };
    let Some(addr) = normalize_addr(addr) else {
        return;
    };
    match state {
        "1" => {
            // hide into the pocket (no-op if it already lives there)
            if hypr::window_in_special(&addr) {
                return;
            }
            hypr::dispatch(&format!("movetoworkspacesilent special:magic,address:{}", addr));
        }
        "0" => restore_window(&addr),
        _ => {}
    }
}

/// `activewindowv2>>addr` — a pocketed window just got activated (taskbar
/// click). If the user is not deliberately browsing the open overlay, pull
/// the window back out.
fn on_focus(addr_raw: &str) {
    if marker_present() {
        return; // Super+S toggle in flight
    }
    let Some(addr) = normalize_addr(addr_raw) else {
        return;
    };
    if !hypr::window_in_special(&addr) {
        return;
    }
    // let the activation settle (the overlay opens right after this event),
    // then restore unless the user changed their mind
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(180));
        if marker_present() {
            return;
        }
        if hypr::window_in_special(&addr) {
            restore_window(&addr);
        }
    });
}

/// `activespecial>>ws,mon` — clear the magic marker once the overlay closes.
fn on_activespecial(payload: &str) {
    let (ws, _mon) = payload.split_once(',').unwrap_or((payload, ""));
    if ws.is_empty() {
        let _ = std::fs::remove_file(paths::magic_marker());
    }
}

fn handle_line(line: &str) {
    let Some((event, payload)) = line.split_once(">>") else {
        return;
    };
    match event {
        "minimized" => on_minimized(payload),
        "activewindowv2" => on_focus(payload),
        "activespecial" => on_activespecial(payload),
        _ if should_react(event) => {
            recompute();
        }
        _ => {}
    }
}

/// Blocking event loop. Reconnects (with backoff sleep) if the socket dies,
/// e.g. Hyprland restart. While connected it consumes 0% CPU.
pub fn run() {
    eprintln!("theme-daemon: waiting for Hyprland socket2...");
    loop {
        if let Some(path) = hypr::socket2_discover() {
            if let Ok(stream) = UnixStream::connect(&path) {
                eprintln!("theme-daemon: listening on {}", path.display());
                let _ = std::fs::remove_file(paths::magic_marker());
                recompute();
                let reader = BufReader::new(stream);
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    handle_line(&line);
                }
                eprintln!("theme-daemon: socket closed, reconnecting...");
            }
        }
        thread::sleep(Duration::from_millis(750));
    }
}
