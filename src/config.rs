use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    if let Some(dir) = env::var_os("RMP2_DIR") {
        return PathBuf::from(dir);
    }
    let base = match env::var_os("XDG_CONFIG_HOME") {
        Some(p) => PathBuf::from(p),
        None => match env::var_os("HOME") {
            Some(h) => PathBuf::from(h).join(".config"),
            None => PathBuf::from("."),
        },
    };
    base.join("rmp2")
}

pub struct Paths {
    pub root: PathBuf,
    pub config: PathBuf,
    pub db: PathBuf,
    pub sock: PathBuf,
    pub pid: PathBuf,
    pub state: PathBuf,
    pub log: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        let root = config_dir();
        let _ = fs::create_dir_all(&root);
        Self {
            config: root.join("config.toml"),
            db: root.join("library.sqlite3"),
            sock: root.join("rmp.sock"),
            pid: root.join("rmp.pid"),
            state: root.join("last-state.json"),
            log: root.join("rmp.log"),
            root,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub mpv_binary: String,
    pub volume_step: i32,
    pub seek_step: f64,
    pub keybindings: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mpv_binary: "mpv".into(),
            volume_step: 5,
            seek_step: 5.0,
            keybindings: default_bindings(),
        }
    }
}

pub fn default_bindings() -> BTreeMap<String, String> {
    let pairs = [
        ("j", "move_down"),
        ("k", "move_up"),
        ("down", "move_down"),
        ("up", "move_up"),
        ("h", "prev_section"),
        ("l", "next_section"),
        ("left", "prev_section"),
        ("right", "next_section"),
        ("tab", "cycle_focus"),
        ("backtab", "cycle_focus_back"),
        ("enter", "activate"),
        ("space", "toggle"),
        ("n", "next_track"),
        ("p", "prev_track"),
        ("r", "repeat"),
        ("s", "shuffle"),
        ("a", "add_media"),
        ("e", "edit_media"),
        ("/", "search"),
        ("f", "favorite"),
        ("A", "add_mini"),
        ("b", "focus_mini"),
        ("J", "volume_down"),
        ("K", "volume_up"),
        ("H", "seek_back"),
        ("L", "seek_fwd"),
        ("q", "confirm_quit"),
        ("esc", "confirm_quit"),
        ("Q", "detach"),
    ];
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

impl Config {
    pub fn load(path: &std::path::Path) -> Self {
        let raw = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return Config::default(),
        };
        match toml::from_str::<Config>(&raw) {
            Ok(mut c) => {
                for (k, v) in default_bindings() {
                    c.keybindings.entry(k).or_insert(v);
                }
                c
            }
            Err(_) => Config::default(),
        }
    }
}
