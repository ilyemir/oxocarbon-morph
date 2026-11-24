//! Central path table + atomic file writes.

use std::io;
use std::path::{Path, PathBuf};

pub const MPV_SOCKET: &str = "/tmp/mpvpaper.sock";
/// Pre-v1.0 socket; cleared during migration so a stale file never blocks IPC.
pub const MPV_SOCKET_LEGACY: &str = "/tmp/mpv-socket";

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into()))
}

pub fn base() -> PathBuf {
    home().join("oxocarbon-morph")
}

pub fn wall_dir() -> PathBuf {
    base().join("wallpapers")
}

pub fn pal_dir() -> PathBuf {
    base().join("palettes")
}

pub fn config_dir() -> PathBuf {
    home().join(".config").join("oxocarbon-morph")
}

pub fn state_file() -> PathBuf {
    config_dir().join("state.json")
}

pub fn active_palette_file() -> PathBuf {
    config_dir().join("active-palette.json")
}

pub fn kitty_conf() -> PathBuf {
    home().join(".config").join("kitty").join("current-theme.conf")
}

pub fn cava_conf() -> PathBuf {
    home().join(".config").join("cava").join("config")
}

pub fn waybar_colors() -> PathBuf {
    home().join(".config").join("waybar").join("oxocarbon-colors.css")
}

pub fn hyprlock_colors() -> PathBuf {
    home().join(".config").join("hypr").join("hyprlock-colors.conf")
}

pub fn wofi_style() -> PathBuf {
    home().join(".config").join("wofi").join("style.css")
}

pub fn cache_dir() -> PathBuf {
    home().join(".cache").join("oxocarbon-morph")
}

pub fn lock_image() -> PathBuf {
    cache_dir().join("lock.png")
}

// --- v1.0: SVG icon engine -------------------------------------------------

pub fn icons_dir() -> PathBuf {
    cache_dir().join("icons")
}

/// Local icon theme serving wlr/taskbar (index.theme + apps/scalable links).
pub fn icon_theme_dir() -> PathBuf {
    home()
        .join(".local")
        .join("share")
        .join("icons")
        .join("oxocarbon-morph")
}

// --- v1.0: dynamic cursor theme ----------------------------------------------

/// Palette-driven cursor theme (hyprcursor SVGs + XCursor binaries).
pub fn cursor_theme_dir() -> PathBuf {
    home()
        .join(".local")
        .join("share")
        .join("icons")
        .join("oxocarbon-cursor")
}

// --- v1.0: wallpaper frame cache -------------------------------------------

pub fn frames_dir() -> PathBuf {
    cache_dir().join("frames")
}

pub fn frame_path(wall: &str) -> PathBuf {
    frames_dir().join(format!("{}.png", crate::wallpaper::stem(wall)))
}

// --- v1.0: dock + hyprland color file ---------------------------------------

/// Repo-side dock template (contains @@DOCK_*@@ markers).
pub fn dock_template() -> PathBuf {
    base().join("waybar").join("dock.jsonc")
}

/// Engine-generated dock config (real file; never a symlink).
pub fn dock_deployed() -> PathBuf {
    home().join(".config").join("waybar").join("dock.jsonc")
}

/// Border colors sourced by hyprland.conf; written atomically on every switch.
pub fn hypr_colors() -> PathBuf {
    home().join(".config").join("hypr").join("hyprland-colors.conf")
}

/// Set by `theme magic toggle` while the user opens special:magic on purpose;
/// the daemon clears it when the overlay closes (activespecial>> empty).
pub fn magic_marker() -> PathBuf {
    cache_dir().join("magic-open")
}

/// Write `data` to `path` atomically (tmp file in same dir + fsync + rename).
pub fn atomic_write(path: &Path, data: &str) -> io::Result<()> {
    atomic_write_bytes(path, data.as_bytes())
}

pub fn atomic_write_bytes(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let tmp = path.with_file_name(format!(".{}.tmp-{}", name, std::process::id()));
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}
