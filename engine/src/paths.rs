//! Central path table + atomic file writes.

use std::io;
use std::path::{Path, PathBuf};

pub const MPV_SOCKET: &str = "/tmp/mpv-socket";

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

/// Write `data` to `path` atomically (tmp file in same dir + fsync + rename).
pub fn atomic_write(path: &Path, data: &str) -> io::Result<()> {
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
        f.write_all(data.as_bytes())?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}
