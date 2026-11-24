pub mod cava;
pub mod cursor;
pub mod daemon;
pub mod frames;
pub mod hypr;
pub mod icons;
pub mod json;
pub mod mpv;
pub mod palette;
pub mod paths;
pub mod state;
pub mod sync;
pub mod wallpaper;

/// println! that survives a closed stdout (e.g. `theme list | head`).
#[macro_export]
macro_rules! outln {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let mut o = std::io::stdout();
        let _ = writeln!(o, $($arg)*);
    }};
}
