//! Wallpaper discovery + fuzzy name resolution.

use crate::paths;
use std::fs;

pub fn list() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(paths::wall_dir()) {
        for e in rd.filter_map(Result::ok) {
            let name = e.file_name().to_string_lossy().into_owned();
            let l = name.to_lowercase();
            if l.ends_with(".mp4") || l.ends_with(".webm") || l.ends_with(".mkv") {
                out.push(name);
            }
        }
    }
    out.sort();
    out
}

pub fn stem(name: &str) -> &str {
    name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name)
}

/// Resolve user query -> wallpaper filename.
/// Match order: exact filename -> exact stem -> substring (first sorted hit).
pub fn resolve(query: &str) -> Option<String> {
    let all = list();
    let ql = query.trim().to_lowercase();
    if ql.is_empty() {
        return None;
    }
    let qstem = stem(&ql);
    if let Some(n) = all.iter().find(|n| n.to_lowercase() == ql) {
        return Some(n.clone());
    }
    if let Some(n) = all.iter().find(|n| stem(n).to_lowercase() == qstem) {
        return Some(n.clone());
    }
    all.iter()
        .find(|n| n.to_lowercase().contains(&qstem))
        .cloned()
}

pub fn index_of(wall: &str) -> Option<usize> {
    list().iter().position(|w| w == wall)
}
