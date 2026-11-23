# Project: oxocarbon-morph

## Hardware & Environment
- **Device:** Lenovo ThinkBook 14 G8 IAL (Intel Core Ultra 7 225H, Intel Arc 140T iGPU)
- **Display:** 1920x1200 (16:10) — dynamically detect monitor name via `hyprctl monitors -j`
- **OS & Shell:** Ubuntu 24.04 (Dual-boot with Windows 11), Zsh + Oh My Zsh + Starship
- **Compositor & Stack:** Hyprland, `mpvpaper` (`hwdec=vaapi`), Kitty, Cava, Wofi, Waybar (or Quickshell if installed)
- **Repo Root:** `~/oxocarbon-morph` (CLI command: `theme`)

## Current Progress
1. Wintux Matrix GRUB dual-boot screen is installed and working.
2. Core packages (`hyprland`, `kitty`, `waybar`, `wofi`, `mpv`, `libmpv-dev`, `cava`, `jq`, `ffmpeg`, `socat`, `intel-media-va-driver`) are installed.
3. 14 live wallpapers are in `~/oxocarbon-morph/wallpapers/` prefixed by palette (`01-crimson-*`, `02-obsidian-*`, `03-oxocarbon-*`, `04-frost-*`, `05-amber-*`).

## Hard Constraints & User Rules
- **NO Randomizer / NO Auto-Rotate:** Explicitly cancelled by Emir. Do NOT build auto-rotation or random modes.
- **ASK FONT BEFORE UI:** Before writing ANY UI, bar, Wofi launcher, Hyprland visual styling, or Kitty font config, STOP and ask Emir which monospace/Nerd Font family he wants to use.
- **ASK BEFORE NAMING / COMMITS:** Always ask Emir before inventing new top-level file/repo names or running `git commit`.
- **Anti-AI-Slop Visual Rules:**
  - Aesthetic: Sharp, minimalist, high-contrast Oxocarbon engineering look.
  - BANNED: Inter/Roboto/Arial defaults, purple/indigo gradients, neon glow borders (`drop_shadow` glow), heavy decorative blur, bounce/elastic animations, generic bento grids, and UI emojis.
  - Keep everything quiet and functional; let the active wallpaper's single accent color drive active borders, bar highlights, terminal accents, and `cava` bars cleanly.

## Power-User Architecture (Leverage Local CLI Access)
1. **Native Rust Engine (if `cargo` is available):** Build a fast, zero-bloat Rust binary (`theme` + `theme-daemon`) in `~/oxocarbon-morph/engine/` and symlink to `~/.local/bin/`. (Fallback to clean Python/C only if `cargo` is missing).
2. **Event-Driven Battery Shield (`socket2.sock`):** Do NOT use `sleep` polling loops. Listen directly to `$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock` (`openwindow`, `closewindow`, `workspace`, `movewindow`, `fullscreen`) for <2ms reaction and 0% idle CPU. Pause `/tmp/mpv-socket` when any window is mapped on the active workspace (if `shield == true` and `manual_pause == false`).
3. **Atomic Multi-Target Color Sync:** Switching themes must update:
   - `mpvpaper` (`hwdec=vaapi`, `input-ipc-server=/tmp/mpv-socket`)
   - Hyprland live borders (`hyprctl keyword general:col.active_border`)
   - Running & new Kitty terminals (`kitty @ set-colors --all` + `~/.config/kitty/current-theme.conf`)
   - Cava config (`~/.config/cava/config` + `pkill -USR1 cava`)
   - Top bar (`Waybar` CSS `@define-color` + `pkill -SIGUSR2 waybar`)
