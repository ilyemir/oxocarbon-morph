# oxocarbon-morph

> **Project status: archived / on hold (maintenance mode).**
>
> This repository is a completed **v1.0 architecture prototype**. It is **not
> under active development** and is archived for reference and personal use.
> The code, configs and documents are published as-is: no support, no feature
> work and no compatibility guarantees for newer Hyprland/Waybar releases.
> Sections marked *Known limitations* describe the state the project was
> frozen in.

A palette-driven Hyprland desktop environment with a dependency-free Rust
theme engine, an event-driven daemon, live video wallpapers over mpv IPC, and
runtime generation of SVG icons and Xcursor/Hyprcursor themes.

Everything visual derives from one of five palette files: borders, bar colors,
terminal accents, cava bars, application icons and the mouse cursor are
regenerated atomically on every theme switch.

---

## Repository layout

```text
oxocarbon-morph/
|-- engine/                  Rust workspace: library + two binaries
|   |-- Cargo.toml           zero dependencies, panic = "abort" in release
|   `-- src/
|       |-- bin/theme.rs     `theme` CLI
|       |-- bin/theme_daemon.rs  `theme-daemon` entry point
|       |-- sync.rs          atomic multi-target apply pipeline
|       |-- daemon.rs        .socket2.sock event loop: shield + pocket
|       |-- mpv.rs           raw JSON IPC client for mpvpaper
|       |-- wallpaper.rs     wallpaper discovery and fuzzy name resolution
|       |-- palette.rs       palette model, prefix -> palette mapping
|       |-- icons.rs         runtime SVG icon engine + dock generator
|       |-- cursor.rs        runtime Xcursor/Hyprcursor generator
|       |-- frames.rs        1920x1200 frame cache (boot image + hyprlock)
|       |-- hypr.rs          Hyprland socket discovery and hyprctl queries
|       |-- cava.rs          cava [color] section rewrite
|       |-- state.rs         ~/.config/oxocarbon-morph/state.json
|       |-- paths.rs         central path table + atomic writes
|       `-- json.rs          minimal JSON parser
|-- hypr/                    Hyprland config (symlinked into ~/.config)
|   |-- hyprland.conf        monitors, input, binds, rules, autostart
|   |-- hyprland-colors.conf engine-generated border palette (sourced)
|   `-- hyprlock.conf        lock screen, includes hyprlock-colors.conf
|-- waybar/
|   |-- config.jsonc         top bar (34 px)
|   |-- style.css            top bar styling
|   |-- dock.jsonc           TEMPLATE for the bottom dock (generated file, see below)
|   `-- dock.css             bottom dock styling
|-- wofi/
|   `-- config               launcher geometry and matching
|-- kitty/
|   `-- kitty.conf           terminal; includes engine-written current-theme.conf
|-- cava/
|   `-- config               audio visualizer; [color] section owned by the engine
|-- palettes/                five palette definitions (JSON)
|-- scripts/                 session helpers (bash, jq + hyprctl only)
|-- grub/                    reference /etc/default/grub for the dual-boot setup
|                            (os-prober enabled, 1920x1200 gfxmode)
`-- wallpapers/              video wallpapers, named by palette prefix
```

`wallpapers/*.mp4` is git-ignored: clone the repository, then drop your own
`<NN>-<palette>-<name>.mp4` files into `wallpapers/`.

---

## Architecture

### 1. Rust engine (`engine/`)

A single library crate with two binaries, no external dependencies, no
`unsafe`, and `panic = "abort"` in release. Total release build qualifies for
`opt-level = "s"` + LTO + `strip`.

| Binary | Purpose |
| :--- | :--- |
| `theme` | CLI: apply/switch palettes, query state, emit Waybar JSON, regenerate assets |
| `theme-daemon` | Long-lived process: battery shield and minimize pocket, driven by Hyprland events |

`theme set <name>` runs `sync::apply` in a fixed order, because live processes
must never observe a half-written state:

1. Write every generated file atomically (`*.tmp-<pid>` then `rename`):
   `waybar/oxocarbon-colors.css`, `dock.jsonc`, icon SVGs, kitty theme, cava
   `[color]`, `hypr/hyprland-colors.conf`, `hyprlock-colors.conf`, wofi style,
   `active-palette.json`.
2. Signal live consumers: `hyprctl keyword general:col.active_border`,
   `kitty @ set-colors --all`, `pkill -USR1 cava`, `pkill -USR2 waybar`.
3. Switch the wallpaper: mpv IPC `loadfile` when a socket answers (typically
   under 80 ms, no process restart), otherwise the `swaybg` + `mpvpaper` boot
   flow.
4. Rebuild the cursor theme and push it with `hyprctl setcursor`.

#### `cursor.rs` - generated cursor theme

Every `theme set` regenerates a complete cursor theme from vector geometry:

* Each shape is defined once in a 24x24 coordinate space as polygons, lines,
  circles and arcs (11 shapes: `left_ptr`, `hand2`, `xterm`, `crosshair`,
  `grabbing`, `sb_h_double_arrow`, `sb_v_double_arrow`, `bd_double_arrow`,
  `fd_double_arrow`, `watch`, `left_ptr_watch`).
* Rendering uses a 4x4 supersampled coverage rasterizer implemented in the
  crate itself; no Cairo, no SVG loader, no image crate.
* Two outputs per shape:
  * `~/.local/share/icons/oxocarbon-cursor/hyprcursors/<shape>.hlc` - the
    Hyprcursor distribution format. libhyprcursor expects a ZIP archive per
    shape containing `meta.hl` and the shape SVG, so the crate embeds a
    minimal STORE-method ZIP writer (CRC-32 + local/central headers).
  * `~/.local/share/icons/oxocarbon-cursor/cursors/<shape>` - classic Xcursor
    binaries at 24/32/48 px, premultiplied ARGB little-endian, plus symlinks
    for aliases (`default`, `pointer`, `text`, `ew-resize`, `wait`, ...).
* Aliases are additionally emitted as Hyprcursor `define_override` entries.
* Hotspots are stored as fractions in `meta.hl` and scaled per image size in
  the Xcursor binary.

#### `icons.rs` - generated icon theme and dock

Scans `.desktop` entries, renders one 24x24 SVG per application with the
active accent baked in (dedicated vector designs for known apps, geometric
monograms otherwise), installs a local icon theme for `wlr/taskbar`, and
expands `waybar/dock.jsonc` (template with `@@DOCK_LEFT@@` and
`@@DOCK_MODULES@@`) into `~/.config/waybar/dock.jsonc`.

`~/.config/waybar/dock.jsonc` is **generated**: edit the repository template,
never the deployed file.

#### `daemon.rs` - event loop, not polling

The daemon blocks on
`$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock`, which keeps
idle CPU at zero and reaction time in the low milliseconds. It handles:

* **Battery shield.** When any window is mapped on the active workspace, the
  wallpaper is paused over IPC (`pause` property); when the workspace is
  empty again it resumes. Controlled by `state.json` (`shield`, `paused`).
  The name predates the final behavior: it is a focus shield, not an AC/battery
  check.
* **Minimize pocket.** Hyprland 0.53 only emits `minimized>>addr,1` for
  foreign-toplevel minimize requests; the daemon performs
  `movetoworkspacesilent special:magic` itself, and on `activewindowv2>>addr`
  pulls a pocketed window back to the active workspace. This is what makes a
  dock click minimize and a second click restore.

#### `frames.rs` - instant wallpaper and lock image

One frame per wallpaper is extracted with `ffmpeg` into
`~/.cache/oxocarbon-morph/frames/`. It serves as the instant `swaybg` image
before `mpvpaper` is ready and as the `hyprlock` background through
`~/.cache/oxocarbon-morph/lock.png`. Extraction runs in a detached child so a
theme switch never blocks on it.

### 2. Wallpaper pipeline

```text
session start -> theme start
  -> swaybg with cached frame (instant, no black screen)
  -> mpvpaper -f -o --input-ipc-server=/tmp/mpvpaper.sock \
              --hwdec=vaapi --loop-file=inf --no-audio ...
  -> theme set <current> (full palette sync)

theme set <other>
  -> mpv IPC loadfile on /tmp/mpvpaper.sock   (no respawn)
  -> fallback: swaybg frame + respawn mpvpaper
```

`mpv::set_pause` and `loadfile` speak raw JSON over the Unix socket, so no
`mpv` CLI, `socat` or `jq` is required at runtime by the engine.

### 3. Waybar: two independent bars

| | Top bar | Bottom dock |
| :--- | :--- | :--- |
| Config | `waybar/config.jsonc` | `waybar/dock.jsonc` (generated) |
| Style | `waybar/style.css` | `waybar/dock.css` |
| Position / height | top, 34 px | bottom, 42 px |
| Left | workspaces 1-9, active window | `[ APPS ]` + pinned `image#<id>` launchers |
| Center | theme pill (`theme pill`, signal 8) | `wlr/taskbar` (icon only, centered) |
| Right | shield pill, volume, network, battery, clock, `[ PWR ]` | tray, `[SCR]` |

Interactions:

* Workspace scroll switches workspace (`workspace m-1` / `m+1`).
* Theme pill: left click opens the theme HUD, right click applies the next
  theme. It is refreshed via `pkill -RTMIN+8 waybar`.
* Taskbar click minimizes to `special:magic`, a second click restores.
* Pinned launchers are `image` modules; the engine writes one module per
  pinned app and the icons are recolored in place.

### 4. Palettes

Each palette file is a flat JSON object with `name`, `bg`, `surface`, `ink`,
`muted`, `accent`, `accent_alt`, `border`. The palette is derived from the
wallpaper filename prefix; if no palette matches, `03-oxocarbon.json` is used.

| Prefix | Palette | Accent |
| :--- | :--- | :--- |
| `01-` | Crimson Void | `#ee5396` |
| `02-` | Obsidian Mono | `#f2f4f8` |
| `03-` | Oxocarbon Neon | `#3ddbd9` |
| `04-` | Frost & Moss | `#82cfff` |
| `05-` | Amber Chill | `#ffe97b` |

### 5. Visual constraints

Sharp, flat, high contrast. No rounded corners, no blur, no shadows, no glow,
no bounce easing. Animations are short and decisive (180-220 ms). Fonts are
BlexMono Nerd Font (IBM Plex Mono Nerd Font). Turkish Q keyboard layout.

---

## Keybindings

Super is `$main`. Binds are defined in `hypr/hyprland.conf`.

### Applications and session

| Shortcut | Action |
| :--- | :--- |
| `Super + Return` | kitty |
| `Super + D` | Wofi application launcher |
| `Super + B` | Firefox |
| `Super + E` | Nautilus |
| `Super + W` | Theme HUD (`theme-hud`) |
| `Super + X` | Power menu (`power-menu`) |
| `Super + L` | hyprlock |
| `Super + Q`, `Alt + F4` | Close active window |
| `Super + Shift + M` | Exit Hyprland |

### Window management

| Shortcut | Action |
| :--- | :--- |
| `Super + V` | Toggle floating; floating opens at 80% x 80%, centered |
| `Super + Space` | Raw `togglefloating` |
| `Super + F` | Maximize (`fullscreen, 1`, bars stay visible) |
| `Super + Shift + F` | True fullscreen (`fullscreen, 0`) |
| `Super + Left/Right` | Windows-style half snap (tiled or floating) |
| `Alt + Arrows` | Move focus |
| `Super + Shift + Arrows` | Move window inside the tiling tree |
| `Super + P` | Pseudo tile |
| `Super + J` | Toggle split |
| `Super + Ctrl + Arrows` | Resize active window in 40 px steps |
| `Super + drag` | Move window |
| `Super + right-drag` | Resize window |

### Workspaces and pocket

| Shortcut | Action |
| :--- | :--- |
| `Super + 1..9` | Switch workspace |
| `Super + Shift + 1..9` | Move window to workspace |
| `Super + Tab` | Previous workspace |
| `Super + S` | Open/close `special:magic` pocket |
| `Super + Shift + S` | Send active window to the pocket |

### Media, clipboard, screenshots

| Shortcut | Action |
| :--- | :--- |
| `XF86AudioRaiseVolume` / `Lower` / `Mute` | Volume with palette OSD |
| `XF86MonBrightnessUp` / `Down` | Brightness with palette OSD |
| `Super + C` | Clipboard history picker |
| `Print` | Full screenshot to `~/Pictures/Screenshots` and clipboard |
| `Super + Print` | Region screenshot to clipboard |
| `Super + Alt + S` | Region screenshot to clipboard |

---

## CLI reference

```text
theme status                 Active wallpaper, palette, shield and IPC state
theme list [--plain]         List wallpapers (--plain: machine readable)
theme set <name>             Switch theme (fuzzy: crimson, 02-crimson, frost)
theme start                  Apply the stored theme at session start
theme next | prev            Cycle wallpapers
theme pause                  Toggle manual pause of the wallpaper
theme shield on|off|toggle   Toggle the workspace focus shield
theme shield-status          Waybar JSON for the shield module
theme pill                   Waybar JSON for the theme module
theme magic toggle           Open/close special:magic (Super+S)
theme sync-icons             Regenerate icons, icon theme and dock.jsonc
theme help                   Usage
```

Engine state lives in `~/.config/oxocarbon-morph/state.json`; the active
palette as JSON is mirrored to
`~/.config/oxocarbon-morph/active-palette.json` for shell scripts.

## Helper scripts

All are bash, `set -euo pipefail`, and depend only on `hyprctl`, `jq` and
standard tools. They are symlinked into `~/.local/bin`.

| Script | Purpose |
| :--- | :--- |
| `theme-hud` | Wofi wallpaper/palette picker |
| `power-menu` | Wofi session menu (shutdown, reboot, suspend, lock, logout) |
| `float-toggle` | Float/unfloat, 80% x 80% centered |
| `snap` | Half-screen snap, bar-safe geometry for floating windows |
| `oxo-osd` | Volume/brightness OSD via `hyprctl notify` in palette accent |
| `clip-store` | Clipboard history writer, started as `wl-paste --watch` |
| `clip-menu` | Clipboard history picker (base64 store, newest first, max 100) |

---

## Installation (Ubuntu / Debian)

The reference system is Ubuntu 24.04, Hyprland 0.53.3, Waybar 0.15.0, on a
1920x1200 eDP panel. Ubuntu's archive Hyprland is much older than 0.53 and
will not accept several dispatchers and windowrule options used here; install
a recent Hyprland (0.50 or newer) from upstream packages or build it.

### 1. System packages

```bash
sudo apt install \
  build-essential pkg-config git curl \
  hyprland hyprlock waybar wofi kitty swaybg cava \
  mpv libmpv-dev ffmpeg \
  jq socat wl-clipboard grim slurp \
  brightnessctl wireplumber \
  wayland-protocols libwayland-dev
```

`mpvpaper` is not packaged on Ubuntu; build it from source:

```bash
git clone https://github.com/GhostNaN/mpvpaper
cd mpvpaper && make && sudo make install
```

### 2. Rust toolchain and font

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# BlexMono = IBM Plex Mono Nerd Font; install from the Nerd Fonts release
# https://github.com/ryanoasis/nerd-fonts/releases (IBMPlexMono.zip)
mkdir -p ~/.local/share/fonts && fc-cache -f
```

The UI assumes BlexMono Nerd Font; without it the bars lose glyphs and the
terminal falls back to an unintended face.

### 3. Clone, build, install binaries

```bash
git clone git@github.com:ilyemir/oxocarbon-morph.git ~/oxocarbon-morph
cd ~/oxocarbon-morph/engine
cargo build --release

mkdir -p ~/.local/bin
install -m755 target/release/theme target/release/theme-daemon ~/.local/bin/
chmod +x ~/oxocarbon-morph/scripts/*
for s in theme-hud power-menu float-toggle snap oxo-osd clip-store clip-menu; do
  ln -sf ~/oxocarbon-morph/scripts/"$s" ~/.local/bin/"$s"
done
```

`theme` and `theme-daemon` must be installed as real files, not symlinks into
`engine/target`, because rebuilt binaries would otherwise be swapped under a
running daemon.

### 4. Deploy configuration

Static configuration is symlinked; generated files are left to the engine.

```bash
mkdir -p ~/.config/{hypr,waybar,wofi,kitty,cava}

ln -sf ~/oxocarbon-morph/hypr/hyprland.conf        ~/.config/hypr/hyprland.conf
ln -sf ~/oxocarbon-morph/hypr/hyprlock.conf        ~/.config/hypr/hyprlock.conf
ln -sf ~/oxocarbon-morph/waybar/config.jsonc       ~/.config/waybar/config.jsonc
ln -sf ~/oxocarbon-morph/waybar/style.css          ~/.config/waybar/style.css
ln -sf ~/oxocarbon-morph/waybar/dock.css           ~/.config/waybar/dock.css
ln -sf ~/oxocarbon-morph/wofi/config               ~/.config/wofi/config
ln -sf ~/oxocarbon-morph/kitty/kitty.conf          ~/.config/kitty/kitty.conf

cp ~/oxocarbon-morph/cava/config                   ~/.config/cava/config
```

Do not symlink `cava/config`: the engine rewrites its `[color]` section with
an atomic rename, which replaces a symlink with a regular file. Copy it once
and let the engine own it afterwards.

Do not create `~/.config/waybar/dock.jsonc` by hand - the first `theme set`
generates it from the repository template.

### 5. Adjust hardcoded paths

The shipped configs contain absolute paths (`/home/emir-alkan/...`) in
`hypr/hyprland.conf`, `waybar/config.jsonc`, the waybar module `exec` lines and
the engine-generated module definitions. Replace them with your own home
directory before starting:

```bash
grep -rn '/home/emir-alkan' ~/oxocarbon-morph/{hypr,waybar,engine/src}
```

### 6. Wallpapers and first run

```bash
cp /path/to/*.mp4 ~/oxocarbon-morph/wallpapers/   # name: <NN>-<palette>-<name>.mp4
theme set 04-frost
theme status
```

Then log out and back in; autostart is defined in `hypr/hyprland.conf`:

```text
exec-once = theme start
exec-once = theme-daemon
exec-once = env NO_AT_BRIDGE=1 GIO_USE_VFS=local GTK_USE_PORTAL=0 waybar
exec-once = (wait for dock.jsonc) waybar -c dock.jsonc -s dock.css
exec-once = wl-paste --watch clip-store
```

---

## Engineering notes

These are the non-obvious constraints the project was frozen with.

* **Waybar startup must bypass the portal/a11y buses.** Waybar registers a
  `Gtk::Application` named `fr.arouillard.waybar` and initializes the ATK
  bridge and GIO VFS at startup. If the a11y or portal bus is wedged, that
  registration blocks for the GDBus default timeout of 25 seconds ("Failed to
  register: Timeout was reached"). The autostart lines therefore use
  `NO_AT_BRIDGE=1 GIO_USE_VFS=local GTK_USE_PORTAL=0`, and both bars start in
  well under one second.
* **Waybar `image` modules need `"interval": "once"`.** Without an explicit
  interval, Waybar 0.15 schedules the image refresh every 1 ms and re-decodes
  the SVG in the GTK main loop, which starves the bar: the layer is created but
  no module is ever drawn, and a subsequent `waybar` invocation blocks on the
  unresponsive application instance. The dock generator always writes
  `"interval": "once"`.
* **Hyprcursor loads ZIP archives, not directories.** `cursors_directory` is
  iterated with `zip_open`; raw `<shape>/meta.hl` directories are skipped and
  the theme ends up with zero shapes. `cursor.rs` writes `.hlc` archives
  directly, so `hyprcursor-util` is not required.
* **Xcursor pixels are premultiplied.** The rasterizer premultiplies RGB by
  coverage before packing ARGB words; non-premultiplied output shows bright
  fringes on dark wallpapers.
* **The minimize path is daemon-owned.** Hyprland 0.53 only reports
  `minimized>>` events; the move into `special:magic` is performed by
  `theme-daemon`. Without the daemon running, taskbar minimize does nothing
  and windows can appear stuck.
* **`theme` output is intentionally ASCII** (Turkish messages without
  diacritics, `[####----]` style bars) so it renders identically in any
  terminal font.

## Known limitations

* `hypr/hyprland.conf` currently contains `windowrule = float on, match:class
  .*`, which floats every new window. It was added while validating dialog
  behavior and should be removed for a normal tiling workflow.
* Absolute `/home/emir-alkan` paths are hardcoded in several configs and in the
  generated dock module definitions.
* Target hardware is a single 1920x1200 monitor at scale 1.0; multi-monitor
  layouts and fractional scaling are untested.
* Wallpaper switching depends on `mpvpaper` exposing an IPC socket; a missing
  or stale socket falls back to the slower respawn path.
* `.mp4` assets are not tracked. A fresh clone has an empty `wallpapers/`
  directory and `theme list` returns nothing until media is added.
* No `LICENSE` file is present. `engine/Cargo.toml` declares MIT; add a license
  file before redistributing.
