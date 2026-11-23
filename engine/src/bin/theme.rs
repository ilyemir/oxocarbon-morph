use oxocarbon_engine::outln;
use oxocarbon_engine::{daemon, daemon::Decision, mpv, palette::Palette, state::State, sync, wallpaper};
use std::process::{Command, Stdio};

const USAGE: &str = "Kullanim:
  theme status                 Aktif tema ve durum ozeti
  theme list [--plain]         Duvar kagitlarini listele (--plain: makine okunur)
  theme set <isim>             Tema sec (ornek: theme set obito)
  theme start                  Kayitli temayi oturum baslangicinda uygula
  theme next | prev            Siradaki / onceki tema
  theme pause                  Manuel duraklat / devam (toggle)
  theme shield on|off|toggle   Pil kalkani";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(&args));
}

fn run(args: &[String]) -> i32 {
    let cmd = args.first().map(String::as_str).unwrap_or("status");
    match cmd {
        "status" | "-s" => show_status(),
        "list" | "-l" => show_list(args.get(1).map(String::as_str) == Some("--plain")),
        "start" => start_current(),
        "set" => match args.get(1) {
            None => {
                eprintln!("[HATA] Kullanim: theme set <isim>");
                1
            }
            Some(q) => match wallpaper::resolve(q) {
                None => {
                    eprintln!("[HATA] Eslesen duvar kagidi yok: {}", q);
                    1
                }
                Some(w) => switch_to(&w),
            },
        },
        "next" | "prev" => cycle(cmd),
        "pause" => toggle_pause(),
        "shield" => shield(args.get(1).map(String::as_str)),
        "help" | "-h" | "--help" => {
            outln!("{}", USAGE);
            0
        }
        other => {
            eprintln!("[HATA] Bilinmeyen komut: {}", other);
            eprintln!("{}", USAGE);
            1
        }
    }
}

fn mpvpaper_running() -> bool {
    Command::new("pgrep")
        .args(["-x", "mpvpaper"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn switch_to(wall: &str) -> i32 {
    let pal = match Palette::for_wall(wall) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[HATA] {}", e);
            return 1;
        }
    };
    let mut st = State::load();
    st.current = wall.to_string();
    st.paused = false;
    if let Err(e) = st.save() {
        eprintln!("[HATA] state.json: {}", e);
        return 1;
    }
    match sync::apply(wall, &pal) {
        Ok(rep) => {
            outln!("[OK] Tema    : {}", wall);
            outln!("[OK] Palet   : {} (accent {})", pal.name, pal.accent);
            if rep.hyprland {
                outln!("[OK] Hyprland kenarlik + mpvpaper guncellendi");
            } else {
                outln!("[BILGI] Hyprland oturumu yok: kenarlik/mpvpaper atlandi, konfig dosyalari yazildi");
            }
            if rep.kitty_live {
                outln!("[OK] Acik kitty pencereleri renklendirildi");
            }
            if rep.lock_frame {
                outln!("[OK] Hyprlock arka plan karesi guncellendi");
            }
            0
        }
        Err(e) => {
            eprintln!("[HATA] {}", e);
            1
        }
    }
}

fn show_status() -> i32 {
    let st = State::load();
    let current = st.current.clone();
    let (wall, pal) = if current.is_empty() {
        (String::from("-"), None)
    } else {
        let p = Palette::for_wall(&current).ok();
        (current.clone(), p)
    };
    outln!("Tema        : {}", wall);
    match &pal {
        Some(p) => outln!("Palet       : {} (accent {})", p.name, p.accent),
        None => outln!("Palet       : -"),
    }
    outln!(
        "Pil kalkani : {}",
        if st.shield { "ACIK" } else { "KAPALI" }
    );
    outln!(
        "Oynatma     : {}",
        if st.paused {
            "DURAKLATILDI (manuel)"
        } else if mpvpaper_running() {
            "OYNATILIYOR"
        } else {
            "OYNATILIYOR (mpvpaper kapali)"
        }
    );
    outln!("mpvpaper    : {}", if mpvpaper_running() { "calisiyor" } else { "calismiyor" });
    outln!(
        "Ayarlar     : {}",
        oxocarbon_engine::paths::config_dir().display()
    );
    0
}

fn show_list(plain: bool) -> i32 {
    let st = State::load();
    let walls = wallpaper::list();
    if walls.is_empty() {
        eprintln!("[HATA] {} icinde duvar kagidi yok", oxocarbon_engine::paths::wall_dir().display());
        return 1;
    }
    for (i, w) in walls.iter().enumerate() {
        let pal = Palette::for_wall(w).map(|p| p.name).unwrap_or_default();
        if plain {
            outln!("{}\t{}", w, pal);
            continue;
        }
        let mark = if *w == st.current { "->" } else { "  " };
        outln!("{} {:2}. {:38} [{}]", mark, i + 1, w, pal);
    }
    0
}

/// Session bootstrap: re-apply the persisted theme (or the first wallpaper
/// if no state exists yet). Used by Hyprland exec-once.
fn start_current() -> i32 {
    let st = State::load();
    let walls = wallpaper::list();
    if walls.is_empty() {
        eprintln!("[HATA] Duvar kagidi yok");
        return 1;
    }
    let wall = if walls.iter().any(|w| *w == st.current) {
        st.current
    } else {
        walls[0].clone()
    };
    switch_to(&wall)
}

fn cycle(cmd: &str) -> i32 {
    let walls = wallpaper::list();
    if walls.is_empty() {
        eprintln!("[HATA] Duvar kagidi yok");
        return 1;
    }
    let st = State::load();
    let len = walls.len();
    let idx = wallpaper::index_of(&st.current).unwrap_or(0);
    let target = if cmd == "next" {
        (idx + 1) % len
    } else {
        (idx + len - 1) % len
    };
    switch_to(&walls[target])
}

fn toggle_pause() -> i32 {
    let mut st = State::load();
    st.paused = !st.paused;
    if let Err(e) = st.save() {
        eprintln!("[HATA] state.json: {}", e);
        return 1;
    }
    if st.paused {
        mpv::set_pause(true);
        outln!("Oynatma     : DURAKLATILDI (manuel)");
    } else {
        match daemon::recompute() {
            Decision::ManualPause => outln!("Oynatma     : DURAKLATILDI"),
            Decision::ShieldOff => outln!("Oynatma     : DEVAM (kalkan kapali)"),
            Decision::NoHyprland => outln!("Oynatma     : DEVAM"),
            Decision::Windows(0) => outln!("Oynatma     : DEVAM"),
            Decision::Windows(_) => outln!("Oynatma     : DURAKLATILDI (kalkan: pencere var)"),
        }
    }
    0
}

fn shield(sub: Option<&str>) -> i32 {
    let sub = sub.unwrap_or("toggle");
    let mut st = State::load();
    st.shield = match sub {
        "on" => true,
        "off" => false,
        "toggle" => !st.shield,
        other => {
            eprintln!("[HATA] Bilinmeyen shield alt komutu: {}", other);
            eprintln!("Kullanim: theme shield on|off|toggle");
            return 1;
        }
    };
    if let Err(e) = st.save() {
        eprintln!("[HATA] state.json: {}", e);
        return 1;
    }
    outln!("Pil kalkani : {}", if st.shield { "ACIK" } else { "KAPALI" });
    if st.shield {
        match daemon::recompute() {
            Decision::ShieldOff => unreachable!(),
            Decision::ManualPause => outln!("Kalkan      : manuel duraklatma aktif, dokunulmadi"),
            Decision::NoHyprland => outln!("Kalkan      : Hyprland oturumu yok, pasif beklemede"),
            Decision::Windows(0) => outln!("Kalkan      : aktif workspace bos -> oynatiliyor"),
            Decision::Windows(n) => outln!("Kalkan      : aktif workspace'te {} pencere -> duraklatildi", n),
        }
    } else if !st.paused {
        mpv::set_pause(false);
        outln!("Kalkan      : kapatildi -> oynatiliyor");
    }
    0
}
