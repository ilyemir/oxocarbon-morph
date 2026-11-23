//! Atomic multi-target color sync + mpvpaper lifecycle.

use crate::{cava, daemon, hypr, json, mpv, palette::Palette, paths};
use std::fs;
use std::process::{Command, Stdio};

pub struct SyncReport {
    pub wallpaper_started: bool,
    pub hyprland: bool,
    pub kitty_live: bool,
    pub lock_frame: bool,
}

fn write_active_palette(pal: &Palette) -> Result<(), String> {
    let data = format!(
        "{{\n  \"name\": \"{}\",\n  \"bg\": \"{}\",\n  \"surface\": \"{}\",\n  \"ink\": \"{}\",\n  \"muted\": \"{}\",\n  \"accent\": \"{}\",\n  \"accent_alt\": \"{}\",\n  \"border\": \"{}\"\n}}\n",
        json::escape(&pal.name),
        json::escape(&pal.bg),
        json::escape(&pal.surface),
        json::escape(&pal.ink),
        json::escape(&pal.muted),
        json::escape(&pal.accent),
        json::escape(&pal.accent_alt),
        json::escape(&pal.border)
    );
    paths::atomic_write(&paths::active_palette_file(), &data)
        .map_err(|e| format!("active-palette.json: {}", e))
}

fn write_kitty(pal: &Palette, wall: &str) -> Result<(), String> {
    let data = format!(
        "# oxocarbon-morph :: {} ({})\n\
         background {}\n\
         foreground {}\n\
         selection_background {}\n\
         selection_foreground {}\n\
         cursor {}\n\
         cursor_text_color {}\n\
         url_color {}\n\
         active_border_color {}\n\
         inactive_border_color {}\n\
         bell_border_color {}\n\
         active_tab_background {}\n\
         active_tab_foreground {}\n\
         inactive_tab_background {}\n\
         inactive_tab_foreground {}\n\
         tab_bar_background {}\n\
         tab_bar_margin_color {}\n\
         color0 {}\n\
         color1 {}\n\
         color2 {}\n\
         color3 {}\n\
         color4 {}\n\
         color5 {}\n\
         color6 {}\n\
         color7 {}\n\
         color8 {}\n\
         color9 {}\n\
         color10 {}\n\
         color11 {}\n\
         color12 {}\n\
         color13 {}\n\
         color14 {}\n\
         color15 {}\n",
        json::escape(&pal.name),
        json::escape(wall),
        pal.bg,
        pal.ink,
        pal.accent,
        pal.bg,
        pal.accent,
        pal.bg,
        pal.accent_alt,
        pal.accent,
        pal.border,
        pal.accent_alt,
        pal.surface,
        pal.ink,
        pal.bg,
        pal.muted,
        pal.bg,
        pal.bg,
        pal.bg,
        pal.accent_alt,
        pal.accent_alt,
        pal.accent,
        pal.accent,
        pal.accent_alt,
        pal.accent,
        pal.ink,
        pal.border,
        pal.accent_alt,
        pal.accent_alt,
        pal.accent,
        pal.accent,
        pal.accent_alt,
        pal.accent,
        pal.ink
    );
    paths::atomic_write(&paths::kitty_conf(), &data)
        .map_err(|e| format!("kitty current-theme.conf: {}", e))
}

fn write_waybar(pal: &Palette) -> Result<(), String> {
    let data = format!(
        "/* oxocarbon-morph palette :: {} */\n\
         @define-color bg {};\n\
         @define-color surface {};\n\
         @define-color ink {};\n\
         @define-color muted {};\n\
         @define-color accent {};\n\
         @define-color accent_alt {};\n\
         @define-color border {};\n",
        json::escape(&pal.name),
        pal.bg,
        pal.surface,
        pal.ink,
        pal.muted,
        pal.accent,
        pal.accent_alt,
        pal.border
    );
    paths::atomic_write(&paths::waybar_colors(), &data)
        .map_err(|e| format!("waybar colors: {}", e))
}

fn write_hyprlock_colors(pal: &Palette) -> Result<(), String> {
    let rgb = |c: &str| format!("rgb({})", c.trim_start_matches('#'));
    let data = format!(
        "# oxocarbon-morph :: hyprlock palette (generated)\n\
         $bg = {}\n\
         $surface = {}\n\
         $ink = {}\n\
         $muted = {}\n\
         $accent = {}\n\
         $accent_alt = {}\n\
         $border = {}\n",
        rgb(&pal.bg),
        rgb(&pal.surface),
        rgb(&pal.ink),
        rgb(&pal.muted),
        rgb(&pal.accent),
        rgb(&pal.accent_alt),
        rgb(&pal.border)
    );
    paths::atomic_write(&paths::hyprlock_colors(), &data)
        .map_err(|e| format!("hyprlock-colors.conf: {}", e))
}

fn write_wofi_style(pal: &Palette) -> Result<(), String> {
    let data = format!(
        "/* oxocarbon-morph :: wofi palette (generated) */\n\
         @define-color bg {bg};\n\
         @define-color surface {surface};\n\
         @define-color ink {ink};\n\
         @define-color muted {muted};\n\
         @define-color accent {accent};\n\
         @define-color accent_alt {accent_alt};\n\
         @define-color border {border};\n\
         \n\
         window {{\n  background-color: @bg;\n  border: 1px solid @border;\n}}\n\
         #input {{\n  background-color: @surface;\n  color: @ink;\n  border: 1px solid @border;\n  margin: 8px;\n  padding: 6px 10px;\n}}\n\
         #inner-box {{\n  background-color: transparent;\n}}\n\
         #outer-box {{\n  margin: 4px;\n}}\n\
         #scroll {{\n  margin: 0 4px;\n}}\n\
         #entry {{\n  padding: 4px 10px;\n  color: @muted;\n}}\n\
         #entry:selected {{\n  color: @ink;\n  background-color: @surface;\n  border-left: 2px solid @accent;\n}}\n\
         #entry image {{\n  margin-right: 8px;\n}}\n\
         #text {{\n  margin: 0 4px;\n}}\n",
        bg = pal.bg,
        surface = pal.surface,
        ink = pal.ink,
        muted = pal.muted,
        accent = pal.accent,
        accent_alt = pal.accent_alt,
        border = pal.border
    );
    paths::atomic_write(&paths::wofi_style(), &data)
        .map_err(|e| format!("wofi style.css: {}", e))
}

/// Extract a single frame from the current wallpaper for the hyprlock background.
fn extract_lock_frame(wall: &str) -> bool {
    let src = paths::wall_dir().join(wall);
    let dst = paths::lock_image();
    if let Some(p) = dst.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-ss",
            "1",
            "-i",
        ])
        .arg(&src)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=1920:1200:force_original_aspect_ratio=increase:flags=lanczos,crop=1920:1200",
        ])
        .arg(&dst)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    matches!(status, Ok(s) if s.success())
}

fn live_kitty() -> bool {
    let conf = paths::kitty_conf();
    let run = |extra: &[&str]| -> bool {
        let mut cmd = Command::new("kitty");
        cmd.arg("@");
        for e in extra {
            cmd.arg(e);
        }
        cmd.args(["set-colors", "--all"])
            .arg(&conf)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    // Preferred: shared abstract socket from kitty.conf listen_on.
    if run(&["--to", "unix:@oxocarbon-kitty"]) {
        return true;
    }
    // Fallback: KITTY_LISTEN_ON inherited from a kitty parent process.
    run(&[])
}

fn signal(process: &str, sig: &str) {
    let _ = Command::new("pkill")
        .arg(format!("-{}", sig))
        .arg(process)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn start_mpvpaper(wall: &str) -> Result<(), String> {
    let _ = Command::new("pkill")
        .args(["-x", "mpvpaper"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = fs::remove_file(paths::MPV_SOCKET);
    let path = paths::wall_dir().join(wall);
    let status = Command::new("mpvpaper")
        .arg("-f")
        .arg("-o")
        .arg("no-audio loop hwdec=vaapi input-ipc-server=/tmp/mpv-socket")
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

/// Apply a theme everywhere. Writes config files first (atomic), then
/// signals live processes, then restarts the wallpaper if in a Hyprland session.
pub fn apply(wall: &str, pal: &Palette) -> Result<SyncReport, String> {
    write_active_palette(pal)?;
    write_kitty(pal, wall)?;
    write_waybar(pal)?;
    write_hyprlock_colors(pal)?;
    write_wofi_style(pal)?;
    cava::write(pal).map_err(|e| format!("cava: {}", e))?;
    let lock_frame = extract_lock_frame(wall);

    let kitty_live = live_kitty();
    signal("cava", "USR1");
    signal("waybar", "USR2");
    hypr::apply_border(pal);

    let mut report = SyncReport {
        wallpaper_started: false,
        hyprland: hypr::running(),
        kitty_live,
        lock_frame,
    };

    if report.hyprland {
        start_mpvpaper(wall)?;
        if mpv::wait_socket(2500) {
            daemon::recompute();
        }
        report.wallpaper_started = true;
    }
    Ok(report)
}
