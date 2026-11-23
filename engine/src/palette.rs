//! Palette model + loading (palette file derives from wallpaper prefix).

use crate::{json, paths};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Palette {
    pub name: String,
    pub bg: String,
    pub surface: String,
    pub ink: String,
    pub muted: String,
    pub accent: String,
    pub accent_alt: String,
    pub border: String,
}

/// "01-crimson-obito.mp4" -> "01-crimson"
pub fn palette_prefix(wall: &str) -> String {
    let stem = crate::wallpaper::stem(wall);
    let parts: Vec<&str> = stem.split('-').collect();
    if parts.len() >= 2 {
        format!("{}-{}", parts[0], parts[1])
    } else {
        stem.to_string()
    }
}

impl Palette {
    pub fn load(path: &Path) -> Result<Palette, String> {
        let txt = fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let v = json::parse(&txt)
            .map_err(|e| format!("{}: {}", path.display(), e))?;
        let get = |k: &str| -> Result<String, String> {
            v.get(k)
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| format!("{}: '{}' token missing", path.display(), k))
        };
        Ok(Palette {
            name: get("name")?,
            bg: get("bg")?,
            surface: get("surface")?,
            ink: get("ink")?,
            muted: get("muted")?,
            accent: get("accent")?,
            accent_alt: get("accent_alt")?,
            border: get("border")?,
        })
    }

    pub fn for_wall(wall: &str) -> Result<Palette, String> {
        let p = paths::pal_dir().join(format!("{}.json", palette_prefix(wall)));
        if p.exists() {
            Palette::load(&p)
        } else {
            Palette::load(&paths::pal_dir().join("03-oxocarbon.json"))
        }
    }

    fn rgba(color: &str, alpha: &str) -> String {
        format!("rgba({}{})", color.trim_start_matches('#'), alpha)
    }

    pub fn accent_rgba(&self) -> String {
        Palette::rgba(&self.accent, "ff")
    }

    pub fn border_rgba(&self) -> String {
        Palette::rgba(&self.border, "ff")
    }
}
