use oxocarbon_engine::outln;
use oxocarbon_engine::{
    daemon, daemon::Decision, frames, hypr, icons, mpv, palette::Palette, paths, state::State,
    sync, wallpaper,
};
use std::process::{Command, Stdio};

const USAGE: &str = "Kullanim:
  theme status                 Aktif tema ve durum ozeti
  theme list [--plain]         Duvar kagitlarini listele (--plain: makine okunur)
  theme set <isim>             Tema sec (ornek: theme set crimson, theme set 02-crimson)
  theme start                  Kayitli temayi oturum baslangicinda uygula
  theme next | prev            Siradaki / onceki tema
  theme pause                  Manuel duraklat / devam (toggle)
  theme shield on|off|toggle   Pil kalkani
  theme pill                   Waybar tema modulu ciktisi (JSON)
  theme shield-status          Waybar pil kalkani modulu ciktisi (JSON)
  theme magic toggle           special:magic cep ac/kapa (Super+S)
  theme sync-icons             Simge setini ve dock'u yeniden uret";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(&args));
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

fn run(args: &[String]) -> i32 {
    let cmd = args.first().map(String::as_str).unwrap_or("status");
    match cmd {
        // hidden: detached frame-cache generator (spawned by sync::apply)
        "__frames" => {
            frames::generate_all(args.get(1).map(String::as_str).unwrap_or(""));
            0
        }
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
        "pill" => pill(),
        "shield-status" => shield_status(),
        "magic" => magic(args.get(1).map(String::as_str)),
        "sync-icons" => sync_icons(),
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
            if !rep.hyprland {
                outln!("[BILGI] Hyprland oturumu yok: kenarlik/mpvpaper atlandi, konfig dosyalari yazildi");
            } else if rep.wallpaper_ipc {
                outln!("[OK] Duvar kagidi: mpv IPC ile aninda degisti (yeniden baslatma yok)");
            } else if rep.wallpaper_started {
                outln!("[OK] Duvar kagidi: swaybg + mpvpaper akisiyla baslatildi");
            } else {
                outln!("[UYARI] Duvar kagidi: mpvpaper IPC yayina gecemedi");
            }
            outln!("[OK] Simge   : {} uygulama simgesi paletle yeniden uretildi", rep.icons);
            if rep.kitty_live {
                outln!("[OK] Acik kitty pencereleri renklendirildi");
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
    outln!("IPC         : {}", if mpv::available() { paths::MPV_SOCKET } else { "yok" });
    match mpv::playing_path() {
        Some(p) => outln!("Oynayan     : {}", p),
        None => outln!("Oynayan     : -"),
    }
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
    sync::waybar_rtmin();
    0
}

// --- waybar module outputs ---------------------------------------------------

fn pill() -> i32 {
    let st = State::load();
    let text = if st.current.is_empty() {
        String::from("-")
    } else {
        oxocarbon_engine::palette::palette_prefix(&st.current)
    };
    outln!(
        "{{\"text\": \"{}\", \"class\": \"themed\", \"tooltip\": \"{}\"}}",
        oxocarbon_engine::json::escape(&text),
        "sol tik: tema HUD (Super+W) - sag tik: sonraki tema"
    );
    0
}

fn shield_status() -> i32 {
    let st = State::load();
    let (text, class) = if st.shield {
        ("SHIELD ON", "shield-on")
    } else {
        ("SHIELD OFF", "shield-off")
    };
    outln!(
        "{{\"text\": \"{}\", \"class\": \"{}\", \"tooltip\": \"{}\"}}",
        text,
        class,
        "pil kalkani: aktif workspace'te pencere varken duvar kagini duraklatir - tikla: ac/kapa"
    );
    0
}

// --- special:magic pocket -----------------------------------------------------

fn magic(sub: Option<&str>) -> i32 {
    match sub.unwrap_or("toggle") {
        "toggle" => {
            if !hypr::running() {
                eprintln!("[HATA] Hyprland oturumu yok");
                return 1;
            }
            // mark intent so the daemon does not "restore" windows the user
            // is deliberately opening the pocket to see
            let marker = paths::magic_marker();
            let _ = std::fs::remove_file(&marker);
            let _ = std::fs::write(&marker, b"1");
            hypr::dispatch("togglespecialworkspace magic");
            0
        }
        other => {
            eprintln!("[HATA] Bilinmeyen magic alt komutu: {}", other);
            eprintln!("Kullanim: theme magic toggle");
            1
        }
    }
}

fn sync_icons() -> i32 {
    let st = State::load();
    if st.current.is_empty() {
        eprintln!("[HATA] Aktif tema yok; once `theme set` ile bir tema secin");
        return 1;
    }
    let pal = match Palette::for_wall(&st.current) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[HATA] {}", e);
            return 1;
        }
    };
    match icons::sync(&pal) {
        Ok(n) => {
            outln!("[OK] {} uygulama simgesi uretildi ({})", n, pal.accent);
            outln!("[OK] Icon theme: {}", paths::icon_theme_dir().display());
            outln!("[OK] Dock      : {}", paths::dock_deployed().display());
            0
        }
        Err(e) => {
            eprintln!("[HATA] {}", e);
            1
        }
    }
}
