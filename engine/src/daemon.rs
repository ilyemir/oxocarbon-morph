//! Event-driven battery shield daemon.
//!
//! Blocks on Hyprland's .socket2.sock (0% idle CPU) and recomputes the
//! desired mpv pause state only on window/workspace events.

use crate::{hypr, mpv, state::State};
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

/// Blocking event loop. Reconnects (with backoff sleep) if the socket dies,
/// e.g. Hyprland restart. While connected it consumes 0% CPU.
pub fn run() {
    eprintln!("theme-daemon: waiting for Hyprland socket2...");
    loop {
        if let Some(path) = hypr::socket2_discover() {
            if let Ok(stream) = UnixStream::connect(&path) {
                eprintln!("theme-daemon: listening on {}", path.display());
                recompute();
                let reader = BufReader::new(stream);
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    let event = line.split(">>").next().unwrap_or("");
                    if should_react(event) {
                        recompute();
                    }
                }
                eprintln!("theme-daemon: socket closed, reconnecting...");
            }
        }
        thread::sleep(Duration::from_millis(750));
    }
}
