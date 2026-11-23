use oxocarbon_engine::{daemon, outln};

fn main() {
    let once = std::env::args().any(|a| a == "--once" || a == "once");
    if once {
        let d = daemon::recompute();
        outln!(
            "karar: {:?} -> mpv pause = {}",
            d,
            d.paused()
        );
        return;
    }
    daemon::run();
}
