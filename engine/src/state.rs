//! Persistent engine state (~/.config/oxocarbon-morph/state.json).

use crate::json;
use crate::paths;
use std::fs;

#[derive(Debug, Clone)]
pub struct State {
    pub current: String,
    pub shield: bool,
    pub paused: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            current: String::new(),
            shield: true,
            paused: false,
        }
    }
}

impl State {
    pub fn load() -> State {
        let mut s = State::default();
        if let Ok(txt) = fs::read_to_string(paths::state_file()) {
            if let Ok(v) = json::parse(&txt) {
                if let Some(x) = v.get("current").and_then(|x| x.as_str()) {
                    s.current = x.to_string();
                }
                if let Some(b) = v.get("shield").and_then(|x| x.as_bool()) {
                    s.shield = b;
                }
                if let Some(b) = v.get("paused").and_then(|x| x.as_bool()) {
                    s.paused = b;
                }
            }
        }
        s
    }

    pub fn save(&self) -> std::io::Result<()> {
        let data = format!(
            "{{\n  \"current\": \"{}\",\n  \"shield\": {},\n  \"paused\": {}\n}}\n",
            json::escape(&self.current),
            self.shield,
            self.paused
        );
        paths::atomic_write(&paths::state_file(), &data)
    }
}
