//! Cava config sync: rewrites only the [color] section, preserves everything else.

use crate::{paths, palette::Palette};
use std::fs;

struct Ini {
    preamble: Vec<String>,
    sections: Vec<(String, Vec<String>)>,
}

fn parse_ini(txt: &str) -> Ini {
    let mut ini = Ini {
        preamble: Vec::new(),
        sections: Vec::new(),
    };
    let mut cur: Option<(String, Vec<String>)> = None;
    for line in txt.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') && t.len() > 2 {
            if let Some(sec) = cur.take() {
                ini.sections.push(sec);
            }
            cur = Some((
                t[1..t.len() - 1].trim().to_string(),
                Vec::new(),
            ));
        } else {
            match &mut cur {
                Some((_, lines)) => lines.push(line.to_string()),
                None => ini.preamble.push(line.to_string()),
            }
        }
    }
    if let Some(sec) = cur {
        ini.sections.push(sec);
    }
    ini
}

fn render(ini: &Ini) -> String {
    let mut out = String::new();
    for l in &ini.preamble {
        out.push_str(l);
        out.push('\n');
    }
    for (name, lines) in &ini.sections {
        out.push('[');
        out.push_str(name);
        out.push_str("]\n");
        for l in lines {
            out.push_str(l);
            out.push('\n');
        }
    }
    out
}

pub fn write(pal: &Palette) -> std::io::Result<()> {
    let path = paths::cava_conf();
    let txt = fs::read_to_string(&path).unwrap_or_default();
    let mut ini = parse_ini(&txt);

    let colors = vec![
        format!("background = '{}'", pal.bg),
        format!("foreground = '{}'", pal.ink),
        "gradient = 1".to_string(),
        format!("gradient_color_1 = '{}'", pal.accent),
        format!("gradient_color_2 = '{}'", pal.accent_alt),
        format!("gradient_color_3 = '{}'", pal.accent),
        format!("gradient_color_4 = '{}'", pal.accent_alt),
        format!("gradient_color_5 = '{}'", pal.accent),
        format!("gradient_color_6 = '{}'", pal.accent_alt),
        format!("gradient_color_7 = '{}'", pal.accent),
        format!("gradient_color_8 = '{}'", pal.accent_alt),
    ];

    match ini
        .sections
        .iter_mut()
        .find(|(n, _)| n.eq_ignore_ascii_case("color"))
    {
        Some(sec) => sec.1 = colors,
        None => ini.sections.push(("color".to_string(), colors)),
    }

    paths::atomic_write(&path, &render(&ini))
}
