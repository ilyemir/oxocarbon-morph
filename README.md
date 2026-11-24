# oxocarbon-morph

A palette-driven Hyprland desktop environment powered by a pure-Rust theme engine, live wallpaper IPC, and runtime SVG/Xcursor generation.

## Architecture

`oxocarbon-morph` replaces fragmented shell scripts with a compiled Rust core (`engine/`) that coordinates window borders, status bars, icons, cursors, and GPU-accelerated video wallpapers atomically.

```text
~/oxocarbon-morph
|-- engine/          # Rust CLI (theme), background daemon (theme-daemon), & cursor rasterizer
|-- hypr/            # Hyprland compositor rules, keybinds, & dynamic palette borders
|-- waybar/          # Top status bar (config.jsonc) & Windows-style bottom dock (dock.jsonc)
|-- wofi/            # Keyboard-driven application launcher styling
|-- scripts/         # Clipboard manager, window snap/float helpers, & OSD notifications
+-- wallpapers/      # Loopable 1080p/1200p video backgrounds per palette
```

## Core Components

### 1. Rust Theme Engine (`theme` & `theme-daemon`)
* **Atomic Palette Switching:** Updates Hyprland active/inactive borders via `hyprctl keyword`, rewrites Waybar CSS color tokens, and triggers `SIGUSR2` reloads without dropping active windows.
* **Runtime Cursor & Icon Generator (`cursor.rs`):** Synthesizes Xcursor and Hyprcursor (`.hlc`) archives from scratch in pure Rust with zero external image/compression dependencies, and recolors application SVGs to match the active accent.
* **Battery Shield Daemon:** Monitors AC adapter state in the background; automatically pauses `mpvpaper` and falls back to a static frame (`SHIELD ON`) when running on battery.

### 2. Compositor & Shell (`Hyprland` + `Waybar`)
* **Dual-Layer Waybar Setup:**
  * **Top Bar:** Workspaces (1-9), active window title, live theme/battery pill, network, audio, and power menu.
  * **Bottom Dock:** `[ APPS ]` Wofi trigger, palette-tinted pinned launchers, centered `wlr/taskbar` with active window indicators, and screenshot shortcut.
* **Live Wallpaper Pipeline:** Renders an instant static frame via `swaybg` on session start, then hands off to hardware-accelerated (`vaapi`) `mpvpaper` controlled over `/tmp/mpvpaper.sock`.

## Keybindings

| Shortcut | Action |
| :--- | :--- |
| `Super + Enter` | Open Kitty terminal |
| `Super + Q` / `Alt + F4` | Close active window |
| `Super + Space` | Open application launcher (Wofi) |
| `Super + S` | Toggle `special:magic` scratchpad |
| `Super + C` | Open clipboard history menu |
| `Super + W` | Toggle theme HUD |
| `Super + Left/Right Click` | Move / resize floating windows |

## CLI Usage

```bash
theme status          # Show active palette and battery shield state
theme list            # List available palettes
theme set crimson     # Switch palette live across Hyprland, Waybar, icons, and cursor
theme next | prev     # Cycle palettes
theme shield toggle   # Manually toggle GPU wallpaper battery shield
theme sync-icons      # Regenerate palette-tinted SVG icons and dock config
```

## Installation

1. **Dependencies (Ubuntu / Debian):**
   ```bash
   sudo apt install hyprland waybar wofi kitty swaybg wl-clipboard cliphist grim slurp jq
   ```
   *(Build and install `mpvpaper` and Rust toolchain via `rustup` for the engine.)*

2. **Build Engine & Symlink Configs:**
   ```bash
   git clone https://github.com/ilyemir/oxocarbon-morph.git ~/oxocarbon-morph
   cd ~/oxocarbon-morph/engine && cargo build --release
   mkdir -p ~/.local/bin ~/.config/{hypr,waybar,wofi}
   cp target/release/theme target/release/theme-daemon ~/.local/bin/
   ln -sf ~/oxocarbon-morph/hypr/hyprland.conf ~/.config/hypr/hyprland.conf
   ln -sf ~/oxocarbon-morph/waybar/* ~/.config/waybar/
   ln -sf ~/oxocarbon-morph/wofi/* ~/.config/wofi/
   ```

3. **Initialize Theme:**
   ```bash
   theme set 01-crimson
   ```
