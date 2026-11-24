//! 1920x1200 frame cache for every wallpaper (v1.0).
//!
//! Each wallpaper gets one extracted frame at ~/.cache/oxocarbon-morph/frames/
//! used as (a) the instant swaybg boot image before mpvpaper is up and
//! (b) the hyprlock background via the lock.png symlink. Frames are generated
//! in a detached child process so active switches never block on ffmpeg.

use crate::{paths, wallpaper};
use std::fs;
use std::process::{Command, Stdio};

pub fn exists(wall: &str) -> bool {
    paths::frame_path(wall).exists()
}

/// Extract one 1920x1200 frame (atomic: tmp file in the same dir + rename).
pub fn generate(wall: &str) -> bool {
    let src = paths::wall_dir().join(wall);
    if !src.exists() {
        return false;
    }
    let dst = paths::frame_path(wall);
    if let Some(p) = dst.parent() {
        let _ = fs::create_dir_all(p);
    }
    let stem = wallpaper::stem(wall);
    let tmp = paths::frames_dir().join(format!("{}.tmp{}.png", stem, std::process::id()));
    let ok = Command::new("ffmpeg")
        .args(["-y", "-hide_banner", "-loglevel", "error", "-ss", "1", "-i"])
        .arg(&src)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=1920:1200:force_original_aspect_ratio=increase:flags=lanczos,crop=1920:1200",
        ])
        .arg(&tmp)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        fs::rename(&tmp, &dst).is_ok()
    } else {
        let _ = fs::remove_file(&tmp);
        false
    }
}

/// Ensure the frame exists, generating it synchronously when missing
/// (used on the boot path where swaybg needs pixels right away).
pub fn ensure(wall: &str) -> bool {
    if exists(wall) {
        return true;
    }
    generate(wall)
}

/// Point ~/.cache/oxocarbon-morph/lock.png (hyprlock background) at the
/// active wallpaper's frame. The symlink may briefly dangle; the background
/// generator fills the target within seconds.
pub fn link_lock_image(wall: &str) -> bool {
    let frame = paths::frame_path(wall);
    let lock = paths::lock_image();
    if let Ok(target) = fs::read_link(&lock) {
        if target == frame {
            return true;
        }
    }
    let _ = fs::remove_file(&lock);
    std::os::unix::fs::symlink(&frame, &lock).is_ok()
}

/// Generate every missing frame in a detached child (`theme __frames <wall>`)
/// so `theme set` returns in milliseconds. The current wallpaper goes first.
pub fn ensure_all_background(current: &str) {
    let Ok(exe) = std::env::current_exe() else { return };
    let _ = Command::new(exe)
        .arg("__frames")
        .arg(current)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Hidden entry point used by the detached child above.
pub fn generate_all(current: &str) {
    let _ = ensure(current);
    for w in wallpaper::list() {
        if w != current {
            let _ = ensure(&w);
        }
    }
}
