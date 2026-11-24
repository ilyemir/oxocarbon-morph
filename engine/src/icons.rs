//! Dynamic SVG icon engine (v1.0).
//!
//! Scans .desktop entries, renders one 24x24 SVG per app with the active
//! palette baked into stroke/fill (custom vector designs for known apps,
//! geometric monograms for the rest), installs a local icon theme for
//! wlr/taskbar and generates the deployed dock.jsonc from the repo template.

use crate::{json, palette::Palette, paths};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

pub struct AppEntry {
    /// desktop-file id (filename without .desktop)
    pub id: String,
    /// Name= (tooltip / fallback letter)
    pub name: String,
    /// Exec= (first token, field codes stripped)
    pub exec: String,
    /// Every name this icon should answer to (desktop id, Icon=, WM class, ...).
    pub icon_names: Vec<String>,
}

/// Dock pins: resolved against the scan, skipped when missing.
const PINNED: &[&str] = &["firefox_firefox", "kitty", "org.gnome.Nautilus", "com.microsoft.VSCode"];

/// Desktop entries that are system plumbing, not launchable apps.
const SKIP_IDS: &[&str] = &[
    "display-im7.q16",
    "im-config",
    "info",
    "org.freedesktop.IBus.Setup",
    "update-manager",
    "org.gnome.Yelp",
    "gnome-language-selector",
    "desktop-security-center_desktop-security-center",
    "firmware-updater_firmware-updater",
    "firmware-updater_firmware-updater-app",
    "prompting-client_daemon",
    "snap-store_packagekit-session-installer",
    "snap-store_show-updates",
    "snap-store_snap-store",
];

/// (token, design) — token matched case-insensitively as a substring of the
/// desktop id / Icon= / StartupWMClass / exec basename.
const KNOWN: &[(&str, &str)] = &[
    ("firefox", "firefox"),
    ("kitty", "kitty"),
    ("nautilus", "nautilus"),
    ("vscode", "code"),
    ("code", "code"),
    ("spotify", "spotify"),
    ("mpv", "mpv"),
    ("vlc", "vlc"),
    ("btop", "btop"),
    ("pavucontrol", "pavucontrol"),
    ("org.gnome.settings", "settings"),
    ("calculator", "calculator"),
    ("texteditor", "texteditor"),
    ("loupe", "loupe"),
    ("diskutility", "diskutility"),
    ("gnome-disks", "diskutility"),
    ("baobab", "baobab"),
    ("ptyxis", "terminal"),
    ("terminal", "terminal"),
    ("papers", "papers"),
    ("resources", "monitor"),
    ("vokoscreen", "screenrec"),
    ("opencode", "opencode"),
    ("opera", "opera"),
];

// ---------------------------------------------------------------------------
// Desktop scan
// ---------------------------------------------------------------------------

fn desktop_dirs() -> Vec<PathBuf> {
    let home = paths::home();
    vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
        home.join(".local/share/applications"),
    ]
}

fn sanitize_name(s: &str) -> Option<String> {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
        .collect();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn parse_desktop(path: &PathBuf, id: String) -> Option<AppEntry> {
    let txt = fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut name = String::new();
    let mut exec = String::new();
    let mut icon = String::new();
    let mut wmclass = String::new();
    let (mut hidden, mut terminal, mut only_show_in, mut is_app) = (false, false, false, false);

    for line in txt.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_entry = t == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((k, v)) = t.split_once('=') else { continue };
        let v = v.trim();
        match k {
            "Type" => is_app = v == "Application",
            "Name" if name.is_empty() => name = v.to_string(),
            "Exec" if exec.is_empty() => exec = v.to_string(),
            "Icon" if icon.is_empty() => icon = v.to_string(),
            "StartupWMClass" if wmclass.is_empty() => wmclass = v.to_string(),
            "NoDisplay" | "Hidden" => hidden = hidden || v.eq_ignore_ascii_case("true"),
            "Terminal" => terminal = terminal || v.eq_ignore_ascii_case("true"),
            "OnlyShowIn" => only_show_in = only_show_in || !v.is_empty(),
            _ => {}
        }
    }

    if !is_app || hidden || terminal || only_show_in {
        return None;
    }
    if name.is_empty() || exec.is_empty() {
        return None;
    }

    // Exec: first token without field codes (%U %F ...) for gtk-launch fallback
    let exec_bin = exec
        .split_whitespace()
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();

    let mut names: Vec<String> = vec![id.clone()];
    if !icon.is_empty() && !icon.starts_with('/') {
        if let Some(i) = sanitize_name(&icon) {
            names.push(i);
        }
    }
    if !wmclass.is_empty() {
        if let Some(w) = sanitize_name(&wmclass) {
            names.push(w);
        }
    }
    if !exec_bin.is_empty() {
        if let Some(e) = sanitize_name(&exec_bin) {
            names.push(e);
        }
    }
    names.retain(|n| !n.is_empty());
    names.dedup();

    Some(AppEntry {
        id,
        name,
        exec: exec_bin,
        icon_names: names,
    })
}

pub fn scan() -> Vec<AppEntry> {
    let mut by_id: BTreeMap<String, AppEntry> = BTreeMap::new();
    for dir in desktop_dirs() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.filter_map(Result::ok) {
            let fname = e.file_name().to_string_lossy().into_owned();
            if !fname.ends_with(".desktop") {
                continue;
            }
            let id = fname.trim_end_matches(".desktop").to_string();
            if SKIP_IDS.contains(&id.as_str()) {
                continue;
            }
            if let Some(app) = parse_desktop(&e.path(), id.clone()) {
                by_id.insert(id, app); // later dirs (e.g. ~/.local) override
            }
        }
    }
    by_id.into_values().collect()
}

// ---------------------------------------------------------------------------
// SVG rendering
// ---------------------------------------------------------------------------

fn design_for(app: &AppEntry) -> String {
    let keys: Vec<String> = app
        .icon_names
        .iter()
        .chain(std::iter::once(&app.exec))
        .map(|k| k.to_lowercase())
        .collect();
    for (token, design) in KNOWN {
        if keys.iter().any(|k| k.contains(token)) {
            return design.to_string();
        }
    }
    "mono".to_string()
}

fn monogram_letter(name: &str) -> char {
    name.chars()
        .find(|c| c.is_ascii_alphanumeric())
        .unwrap_or('x')
        .to_ascii_uppercase()
}

/// Palette-baked 24x24 SVG body. a = accent, i = ink, m = muted.
fn design_body(design: &str, app: &AppEntry, pal: &Palette) -> String {
    let (a, i, m) = (&pal.accent, &pal.ink, &pal.muted);
    match design {
        "firefox" => format!(
            "<circle cx=\"12\" cy=\"12\" r=\"8.2\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <ellipse cx=\"12\" cy=\"12\" rx=\"3.4\" ry=\"8.2\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1\"/>\n\
             <path d=\"M3.8 12 H20.2\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1\"/>"
        ),
        "kitty" => format!(
            "<path d=\"M4.5 13 V10.4 L7 4.5 L10 8 H14 L17 4.5 L19.5 10.4 V13 C19.5 17.2 16.2 20.5 12 20.5 C7.8 20.5 4.5 17.2 4.5 13 Z\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <path d=\"M9.2 13.4 L10.6 14.8\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.4\"/>\n\
             <path d=\"M14.8 13.4 L13.4 14.8\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.4\"/>"
        ),
        "nautilus" => format!(
            "<path d=\"M3.5 6.5 H9.5 L11.5 9 H20.5 V19.5 H3.5 Z\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>"
        ),
        "code" => format!(
            "<path d=\"M8.5 7 L4 12 L8.5 17\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.6\"/>\n\
             <path d=\"M15.5 7 L20 12 L15.5 17\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.6\"/>\n\
             <path d=\"M10.8 16 L13.2 8\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.2\"/>"
        ),
        "spotify" => format!(
            "<circle cx=\"12\" cy=\"12\" r=\"8.2\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.2\"/>\n\
             <path d=\"M8 10.4 C10.4 9.4 13.6 9.6 16 11\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.6\"/>\n\
             <path d=\"M8.4 12.9 C10.6 12.1 13.4 12.2 15.6 13.4\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.6\"/>\n\
             <path d=\"M8.9 15.3 C10.7 14.7 13.2 14.7 15.1 15.7\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.6\"/>"
        ),
        "mpv" => format!(
            "<circle cx=\"12\" cy=\"12\" r=\"8.2\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <path d=\"M10 8.6 V15.4 L16.2 12 Z\" fill=\"{a}\"/>"
        ),
        "vlc" => format!(
            "<path d=\"M9.6 5.5 H14.4 L17.6 18 H6.4 Z\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <path d=\"M5 20 H19\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.4\"/>"
        ),
        "btop" => format!(
            "<path d=\"M6 19 V12\" fill=\"none\" stroke=\"{a}\" stroke-width=\"2\"/>\n\
             <path d=\"M10 19 V7\" fill=\"none\" stroke=\"{a}\" stroke-width=\"2\"/>\n\
             <path d=\"M14 19 V10\" fill=\"none\" stroke=\"{a}\" stroke-width=\"2\"/>\n\
             <path d=\"M18 19 V5\" fill=\"none\" stroke=\"{a}\" stroke-width=\"2\"/>\n\
             <path d=\"M4 19 H20\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1\"/>"
        ),
        "pavucontrol" => format!(
            "<path d=\"M4 10 H7.5 L11.5 6.5 V17.5 L7.5 14 H4 Z\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <path d=\"M14.5 9.5 C16 11 16 13 14.5 14.5\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.2\"/>\n\
             <path d=\"M17 7.5 C19.5 10.3 19.5 13.7 17 16.5\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.2\"/>"
        ),
        "settings" => format!(
            "<circle cx=\"12\" cy=\"12\" r=\"4\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <circle cx=\"12\" cy=\"12\" r=\"1.1\" fill=\"{i}\"/>\n\
             <path d=\"M12 3.5 V6 M12 18 V20.5 M3.5 12 H6 M18 12 H20.5 M6 6 L7.8 7.8 M16.2 16.2 L18 18 M18 6 L16.2 7.8 M7.8 16.2 L6 18\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>"
        ),
        "calculator" => {
            let dots = [(7.9, 12.1), (11.3, 12.1), (14.7, 12.1), (7.9, 15.7), (11.3, 15.7), (14.7, 15.7)];
            let rects: String = dots
                .iter()
                .map(|(x, y)| format!("<rect x=\"{x}\" y=\"{y}\" width=\"1.4\" height=\"1.4\" fill=\"{i}\"/>"))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "<rect x=\"4.5\" y=\"4.5\" width=\"15\" height=\"15\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
                 <path d=\"M4.5 9.5 H19.5\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
                 {rects}"
            )
        }
        "texteditor" => format!(
            "<rect x=\"5.5\" y=\"4\" width=\"13\" height=\"16\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <path d=\"M8.5 8.5 H15.5 M8.5 12 H15.5 M8.5 15.5 H12.5\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.2\"/>"
        ),
        "loupe" => format!(
            "<circle cx=\"10.5\" cy=\"10.5\" r=\"5.2\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.6\"/>\n\
             <path d=\"M14.6 14.6 L20 20\" fill=\"none\" stroke=\"{a}\" stroke-width=\"2\"/>"
        ),
        "diskutility" => format!(
            "<circle cx=\"12\" cy=\"12\" r=\"8\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <circle cx=\"12\" cy=\"12\" r=\"3.2\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.2\"/>\n\
             <circle cx=\"12\" cy=\"12\" r=\"0.8\" fill=\"{i}\"/>"
        ),
        "baobab" => format!(
            "<circle cx=\"12\" cy=\"12\" r=\"8\" fill=\"none\" stroke=\"{m}\" stroke-width=\"1.2\"/>\n\
             <path d=\"M12 4 A8 8 0 1 1 5.5 16.5\" fill=\"none\" stroke=\"{a}\" stroke-width=\"2\"/>\n\
             <path d=\"M12 12 V4\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1\"/>"
        ),
        "terminal" => format!(
            "<rect x=\"3.5\" y=\"4.5\" width=\"17\" height=\"15\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <path d=\"M7 9.5 L10 12.2 L7 14.9\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.4\"/>\n\
             <path d=\"M12.5 15 H16.5\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.4\"/>"
        ),
        "papers" => format!(
            "<path d=\"M6 4 H14.5 L18.5 8 V20 H6 Z\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <path d=\"M14.5 4 V8 H18.5\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.2\"/>"
        ),
        "monitor" => format!(
            "<rect x=\"3.5\" y=\"4.5\" width=\"17\" height=\"12\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <path d=\"M6.5 13 H9 L10.5 8.5 L13 14.5 L14.5 11.5 H17.5\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.2\"/>\n\
             <path d=\"M12 16.5 V19.5 M8.5 19.5 H15.5\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.2\"/>"
        ),
        "screenrec" => format!(
            "<rect x=\"3.5\" y=\"5\" width=\"17\" height=\"11.5\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <circle cx=\"12\" cy=\"10.75\" r=\"2\" fill=\"{i}\"/>\n\
             <path d=\"M12 16.5 V19.5 M8.5 19.5 H15.5\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.2\"/>"
        ),
        "opencode" => format!(
            "<path d=\"M7 6.5 L12.5 12 L7 17.5\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.6\"/>\n\
             <rect x=\"14.8\" y=\"10.7\" width=\"2.6\" height=\"2.6\" fill=\"{i}\"/>"
        ),
        "opera" => format!(
            "<circle cx=\"12\" cy=\"12\" r=\"8.2\" fill=\"none\" stroke=\"{a}\" stroke-width=\"1.4\"/>\n\
             <ellipse cx=\"12\" cy=\"12\" rx=\"3.4\" ry=\"6.4\" fill=\"none\" stroke=\"{i}\" stroke-width=\"1.2\"/>"
        ),
        // geometric monogram fallback
        _ => format!(
            "<rect x=\"3.5\" y=\"3.5\" width=\"17\" height=\"17\" fill=\"none\" stroke=\"{m}\" stroke-width=\"1.2\"/>\n\
             <text x=\"12\" y=\"16.6\" font-family=\"BlexMono Nerd Font, monospace\" font-size=\"12\" font-weight=\"700\" fill=\"{i}\" text-anchor=\"middle\">{}</text>",
            monogram_letter(&app.name)
        ),
    }
}

fn render_svg(app: &AppEntry, pal: &Palette) -> String {
    let body = design_body(&design_for(app), app, pal);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\">\n{}\n</svg>\n",
        body
    )
}

// ---------------------------------------------------------------------------
// Icon theme install + dock generation
// ---------------------------------------------------------------------------

fn install_theme(apps: &[AppEntry]) -> Result<(), String> {
    let scalable = paths::icon_theme_dir().join("apps").join("scalable");
    fs::create_dir_all(&scalable).map_err(|e| format!("icon theme: {}", e))?;

    let index = "[Icon Theme]\n\
                 Name=oxocarbon-morph\n\
                 Comment=Palette-driven Oxocarbon application icons (generated)\n\
                 Inherits=hicolor\n\
                 Directories=scalable/apps\n\
                 \n\
                 [scalable/apps]\n\
                 Size=24\n\
                 Context=Applications\n\
                 Type=Scalable\n\
                 MinSize=16\n\
                 MaxSize=128\n";
    paths::atomic_write(&paths::icon_theme_dir().join("index.theme"), index)
        .map_err(|e| format!("index.theme: {}", e))?;

    for app in apps {
        let canonical = paths::icons_dir().join(format!("{}.svg", app.id));
        for name in &app.icon_names {
            let Some(safe) = sanitize_name(name) else { continue };
            let link = scalable.join(format!("{}.svg", safe));
            let _ = fs::remove_file(&link);
            let _ = symlink(&canonical, &link);
        }
    }
    Ok(())
}

fn write_dock(apps: &[AppEntry]) -> Result<(), String> {
    let template =
        fs::read_to_string(paths::dock_template()).map_err(|e| format!("dock template: {}", e))?;

    let pinned: Vec<&AppEntry> = PINNED
        .iter()
        .filter_map(|id| apps.iter().find(|a| a.id == *id))
        .collect();

    let mut left = vec!["\"custom/launcher\"".to_string()];
    left.extend(pinned.iter().map(|a| format!("\"image/{}\"", a.id)));
    let left_json = left.join(", ");

    let modules: Vec<String> = pinned
        .iter()
        .map(|a| {
            format!(
                "\"image/{id}\": {{\n      \"path\": \"{path}\",\n      \"size\": 22,\n      \"tooltip\": \"{tip}\",\n      \"on-click\": \"gtk-launch {id}\"\n    }}",
                id = a.id,
                path = paths::icons_dir().join(format!("{}.svg", a.id)).display(),
                tip = json::escape(&a.name),
            )
        })
        .collect();
    // each module carries its own trailing separator so the generated file
    // stays valid JSON whether the pin list is empty or not
    let modules_json = if modules.is_empty() {
        String::new()
    } else {
        format!("{},", modules.join(",\n  "))
    };

    let out = template
        .replace("@@DOCK_LEFT@@", &left_json)
        .replace("@@DOCK_MODULES@@", &modules_json);
    paths::atomic_write(&paths::dock_deployed(), &out)
        .map_err(|e| format!("dock.jsonc: {}", e))
}

/// Regenerate every icon with `pal`, install the icon theme and the dock.
pub fn sync(pal: &Palette) -> Result<usize, String> {
    let apps = scan();
    let dir = paths::icons_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("icons dir: {}", e))?;
    for app in &apps {
        let svg = render_svg(app, pal);
        let path = dir.join(format!("{}.svg", app.id));
        paths::atomic_write(&path, &svg).map_err(|e| format!("{}: {}", path.display(), e))?;
    }
    install_theme(&apps)?;
    write_dock(&apps)?;
    Ok(apps.len())
}
