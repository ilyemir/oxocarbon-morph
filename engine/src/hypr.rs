//! Hyprland integration: event socket discovery, hyprctl queries, live border keywords.

use crate::{json, palette::Palette};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::SystemTime;

pub fn running() -> bool {
    std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

fn runtime_dir() -> Option<PathBuf> {
    if let Ok(x) = std::env::var("XDG_RUNTIME_DIR") {
        if !x.is_empty() {
            return Some(PathBuf::from(x));
        }
    }
    None
}

/// Explicit instance, else newest .socket2.sock under $XDG_RUNTIME_DIR/hypr/.
pub fn socket2_discover() -> Option<PathBuf> {
    let rt = runtime_dir()?;
    if let Ok(sig) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        if !sig.is_empty() {
            let p = rt.join("hypr").join(&sig).join(".socket2.sock");
            if p.exists() {
                return Some(p);
            }
        }
    }
    let hypr_dir = rt.join("hypr");
    let mut best: Option<(SystemTime, PathBuf)> = None;
    if let Ok(rd) = std::fs::read_dir(&hypr_dir) {
        for e in rd.filter_map(Result::ok) {
            let p = e.path().join(".socket2.sock");
            if p.exists() {
                if let Ok(meta) = e.metadata() {
                    let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    if best.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                        best = Some((mtime, p));
                    }
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

fn hyprctl_json(args: &[&str]) -> Option<json::Value> {
    let mut cmd = Command::new("hyprctl");
    cmd.args(args).arg("-j").stdin(Stdio::null());
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    json::parse(&String::from_utf8_lossy(&out.stdout)).ok()
}

pub fn active_workspace() -> Option<i64> {
    let v = hyprctl_json(&["activeworkspace"])?;
    v.get("id")?.as_f64().map(|f| f as i64)
}

/// Number of mapped windows on the given workspace.
pub fn windows_on_workspace(ws: i64) -> Option<usize> {
    let v = hyprctl_json(&["clients"])?;
    let arr = v.as_array()?;
    let mut n = 0;
    for c in arr {
        let id = c
            .get("workspace")
            .and_then(|w| w.get("id"))
            .and_then(json::Value::as_f64)
            .map(|f| f as i64);
        let mapped = c
            .get("mapped")
            .and_then(json::Value::as_bool)
            .unwrap_or(false);
        if id == Some(ws) && mapped {
            n += 1;
        }
    }
    Some(n)
}

/// Push live border colors (only when inside a Hyprland session).
pub fn apply_border(pal: &Palette) {
    if !running() {
        return;
    }
    let _ = Command::new("hyprctl")
        .args(["keyword", "general:col.active_border", &pal.accent_rgba()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = Command::new("hyprctl")
        .args(["keyword", "general:col.inactive_border", &pal.border_rgba()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}
