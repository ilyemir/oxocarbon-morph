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
/// Match order: exact filename -> exact stem -> substring -> palette family.
/// The family pass makes "02-crimson" resolve to the crimson palette even
/// though crimson's real prefix index is 01 (same for "04-frost", "frost", ...).
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
    if let Some(n) = all
        .iter()
        .find(|n| n.to_lowercase().contains(&qstem))
    {
        return Some(n.clone());
    }
    // "02-crimson" -> family "crimson": drop pure-numeric tokens from the
    // query and match them against the non-numeric part of every prefix.
    let family: String = qstem
        .split('-')
        .filter(|t| !t.is_empty() && !t.chars().all(|c| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .join("-");
    if !family.is_empty() {
        let family_of = |wall: &str| -> String {
            crate::palette::palette_prefix(wall)
                .split('-')
                .filter(|t| !t.is_empty() && !t.chars().all(|c| c.is_ascii_digit()))
                .collect::<Vec<_>>()
                .join("-")
        };
        if let Some(n) = all.iter().find(|n| family_of(n) == family) {
            return Some(n.clone());
        }
        if let Some(n) = all.iter().find(|n| n.to_lowercase().contains(&family)) {
            return Some(n.clone());
        }
    }
    None
}

pub fn index_of(wall: &str) -> Option<usize> {
    list().iter().position(|w| w == wall)
}
