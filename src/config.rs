use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveUp,
    MoveDown,
    PrevSection,
    NextSection,
    CycleFocus,
    CycleFocusBack,
    Activate,
    Toggle,
    NextTrack,
    PrevTrack,
    VolumeUp,
    VolumeDown,
    SeekBack,
    SeekFwd,
    Repeat,
    Shuffle,
    AddMedia,
    EditMedia,
    Search,
    Favorite,
    AddMini,
    FocusMini,
    Delete,
    MiniMoveUp,
    MiniMoveDown,
    ConfirmQuit,
    Detach,
}

pub const ACTIONS: &[(Action, &str)] = &[
    (Action::MoveUp, "move_up"),
    (Action::MoveDown, "move_down"),
    (Action::PrevSection, "prev_section"),
    (Action::NextSection, "next_section"),
    (Action::CycleFocus, "cycle_focus"),
    (Action::CycleFocusBack, "cycle_focus_back"),
    (Action::Activate, "activate"),
    (Action::Toggle, "toggle"),
    (Action::NextTrack, "next_track"),
    (Action::PrevTrack, "prev_track"),
    (Action::VolumeUp, "volume_up"),
    (Action::VolumeDown, "volume_down"),
    (Action::SeekBack, "seek_back"),
    (Action::SeekFwd, "seek_fwd"),
    (Action::Repeat, "repeat"),
    (Action::Shuffle, "shuffle"),
    (Action::AddMedia, "add_media"),
    (Action::EditMedia, "edit_media"),
    (Action::Search, "search"),
    (Action::Favorite, "favorite"),
    (Action::AddMini, "add_mini"),
    (Action::FocusMini, "focus_mini"),
    (Action::Delete, "delete"),
    (Action::MiniMoveUp, "mini_move_up"),
    (Action::MiniMoveDown, "mini_move_down"),
    (Action::ConfirmQuit, "confirm_quit"),
    (Action::Detach, "detach"),
];

pub fn action_from_str(name: &str) -> Option<Action> {
    ACTIONS.iter().find(|(_, n)| *n == name).map(|(a, _)| *a)
}

pub fn action_str(action: Action) -> &'static str {
    ACTIONS
        .iter()
        .find(|(a, _)| *a == action)
        .map(|(_, n)| *n)
        .unwrap_or("?")
}

mod keybindings {
    use serde::{Deserialize, Deserializer};
    use std::collections::BTreeMap;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum KeySpec {
        One(String),
        Many(Vec<String>),
    }

    pub fn deserialize<'de, D>(d: D) -> Result<BTreeMap<String, Vec<String>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw: BTreeMap<String, KeySpec> = BTreeMap::deserialize(d)?;
        Ok(raw
            .into_iter()
            .map(|(action, spec)| {
                let keys = match spec {
                    KeySpec::One(s) => vec![s],
                    KeySpec::Many(v) => v,
                };
                (action, keys)
            })
            .collect())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Titles {
    pub filter: String,
    pub queue: String,
    pub mini: String,
    pub info: String,
}

impl Default for Titles {
    fn default() -> Self {
        Titles {
            filter: "Filter".into(),
            queue: "Queue".into(),
            mini: "Mini Queue".into(),
            info: "Info".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub mpv_binary: String,
    pub volume_step: i32,
    pub seek_step: f64,
    pub titles: Titles,
    #[serde(with = "keybindings")]
    pub keybindings: BTreeMap<String, Vec<String>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mpv_binary: "mpv".into(),
            volume_step: 5,
            seek_step: 5.0,
            titles: Titles::default(),
            keybindings: default_bindings(),
        }
    }
}

pub fn default_bindings() -> BTreeMap<String, Vec<String>> {
    let pairs = [
        ("move_down", &["j", "down"][..]),
        ("move_up", &["k", "up"][..]),
        ("prev_section", &["h"][..]),
        ("next_section", &["l"][..]),
        ("cycle_focus", &["tab"][..]),
        ("cycle_focus_back", &["backtab"][..]),
        ("activate", &["enter"][..]),
        ("toggle", &["space"][..]),
        ("next_track", &["n"][..]),
        ("prev_track", &["p"][..]),
        ("repeat", &["r"][..]),
        ("shuffle", &["s"][..]),
        ("add_media", &["a"][..]),
        ("edit_media", &["e"][..]),
        ("search", &["/"][..]),
        ("favorite", &["f"][..]),
        ("add_mini", &["A"][..]),
        ("focus_mini", &["b"][..]),
        ("delete", &["d"][..]),
        ("mini_move_up", &["ctrl+k"][..]),
        ("mini_move_down", &["ctrl+j"][..]),
        ("volume_down", &["J"][..]),
        ("volume_up", &["K"][..]),
        ("seek_back", &["H", "left"][..]),
        ("seek_fwd", &["L", "right"][..]),
        ("confirm_quit", &["q", "esc"][..]),
        ("detach", &["Q"][..]),
    ];
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect()))
        .collect()
}

fn valid_plain_key(k: &str) -> bool {
    matches!(
        k,
        "space"
            | "enter"
            | "esc"
            | "tab"
            | "backtab"
            | "up"
            | "down"
            | "left"
            | "right"
            | "home"
            | "end"
            | "insert"
            | "delete"
            | "backspace"
            | "pageup"
            | "pagedown"
    )
}

fn is_valid_key(k: &str) -> bool {
    if valid_plain_key(k) {
        return true;
    }
    if k.len() == 1 && k.chars().all(|c| c.is_ascii_graphic()) {
        return true;
    }
    if let Some(r) = k.strip_prefix("ctrl+") {
        return r.len() == 1 && r.chars().all(|c| c.is_ascii_graphic());
    }
    if let Some(f) = k.strip_prefix('f')
        && let Ok(n) = f.parse::<u8>()
    {
        return (1..=24).contains(&n);
    }
    false
}

fn binding_text(action: &str, keys: &[String]) -> String {
    if keys.len() == 1 {
        format!("{action} = \"{}\"\n", keys[0])
    } else {
        let list = keys
            .iter()
            .map(|k| format!("\"{k}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{action} = [{list}]\n")
    }
}

pub fn default_config_text() -> String {
    let mut out = String::new();
    out.push_str("# rmp2 configuration\n");
    out.push('\n');
    out.push_str("# Keybindings map an action to a key or a list of keys.\n");
    out.push_str("# Use \"none\" to disable a default binding (e.g. confirm_quit = \"none\").\n");
    out.push('\n');
    let actions: Vec<&str> = ACTIONS.iter().map(|(a, _)| action_str(*a)).collect();
    out.push_str(&format!("# Actions: {}\n", actions.join(" ")));
    out.push_str("# Keys: single characters (letters, digits, symbols), uppercase for shift,\n");
    out.push_str("#       \"ctrl+<char>\" for control, \"f1\"..\"f12\", and the named keys\n");
    out.push_str(
        "#       \"space\" \"enter\" \"esc\" \"tab\" \"backtab\" \"up\" \"down\" \"left\"\n",
    );
    out.push_str(
        "#       \"right\" \"home\" \"end\" \"pageup\" \"pagedown\" \"insert\" \"delete\"\n",
    );
    out.push_str("#       \"backspace\"\n");
    out.push('\n');
    out.push_str("mpv_binary = \"mpv\"\n");
    out.push_str("volume_step = 5\n");
    out.push_str("seek_step = 5.0\n");
    out.push('\n');
    out.push_str("# Section titles used in the pane borders.\n");
    out.push_str("[titles]\n");
    out.push_str("filter = \"Filter\"\n");
    out.push_str("queue = \"Queue\"\n");
    out.push_str("mini = \"Mini Queue\"\n");
    out.push_str("info = \"Info\"\n");
    out.push('\n');
    out.push_str("[keybindings]\n");
    for (action, keys) in default_bindings() {
        out.push_str(&binding_text(&action, &keys));
    }
    out
}

fn migrate_legacy(raw: &str) -> Option<String> {
    let top: BTreeMap<String, toml::Value> = toml::from_str(raw).ok()?;
    let table = top.get("keybindings")?.as_table()?;
    if table.keys().any(|k| action_from_str(k).is_some()) {
        return None;
    }
    let legacy = table.iter().any(|(k, v)| {
        is_valid_key(k)
            && action_from_str(k).is_none()
            && v.as_str().is_some_and(|s| action_from_str(s).is_some())
    });
    if !legacy {
        return None;
    }
    let mut out = String::new();
    out.push_str("# rmp2 configuration\n");
    out.push_str("# Auto-migrated from the legacy \"key = action\" format.\n\n");
    out.push_str("mpv_binary = ");
    out.push_str(
        &top.get("mpv_binary")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "\"mpv\"".into()),
    );
    out.push('\n');
    out.push_str("volume_step = ");
    out.push_str(
        &top.get("volume_step")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "5".into()),
    );
    out.push('\n');
    out.push_str("seek_step = ");
    out.push_str(
        &top.get("seek_step")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "5.0".into()),
    );
    out.push('\n');
    out.push_str("[keybindings]\n");
    let mut merged: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let excluded: std::collections::HashSet<String> = table
        .iter()
        .filter(|(_, v)| matches!(v.as_str(), Some("none")))
        .map(|(k, _)| k.clone())
        .collect();
    for (k, v) in table {
        if let Some(action) = v.as_str()
            && action != "none"
            && !excluded.contains(k)
        {
            merged
                .entry(action.to_string())
                .or_default()
                .push(k.clone());
        }
    }
    for (action, default_keys) in default_bindings() {
        let entry = merged.entry(action.clone()).or_default();
        for dk in default_keys {
            if !excluded.contains(&dk) && !entry.contains(&dk) {
                entry.push(dk.clone());
            }
        }
    }
    for keys in merged.values_mut() {
        keys.sort();
    }
    for (action, keys) in merged {
        out.push_str(&binding_text(&action, &keys));
    }
    Some(out)
}

impl Config {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let raw = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => {
                let text = default_config_text();
                let _ = fs::write(path, text);
                return Ok(Config::default());
            }
        };
        if let Some(migrated) = migrate_legacy(&raw) {
            let _ = fs::write(path, &migrated);
            return Self::parse(&migrated);
        }
        Self::parse(&raw)
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut cfg: Config =
            toml::from_str(raw).map_err(|e| format!("config parse error: {e}"))?;
        for (action, keys) in default_bindings() {
            cfg.keybindings.entry(action).or_insert(keys);
        }
        let mut errors = Vec::new();
        let mut seen: HashMap<&str, &str> = HashMap::new();
        for (action, keys) in &cfg.keybindings {
            if action_from_str(action).is_none() {
                errors.push(format!("invalid action \"{action}\""));
                continue;
            }
            for k in keys {
                if k == "none" {
                    continue;
                }
                if !is_valid_key(k) {
                    errors.push(format!("invalid key \"{k}\" for action \"{action}\""));
                }
                if let Some(prev) = seen.insert(k, action.as_str())
                    && prev != action
                {
                    errors.push(format!(
                        "key \"{k}\" bound to multiple actions (\"{prev}\", \"{action}\")"
                    ));
                }
            }
        }
        if errors.is_empty() {
            Ok(cfg)
        } else {
            Err(format!("invalid config:\n{}", errors.join("\n")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bindings(cfg: &Config) -> &BTreeMap<String, Vec<String>> {
        &cfg.keybindings
    }

    #[test]
    fn parse_merges_defaults_into_partial_config() {
        let cfg = Config::parse("[keybindings]\ntoggle = \"x\"\n").unwrap();
        assert_eq!(bindings(&cfg).get("toggle"), Some(&vec!["x".to_string()]));
        assert_eq!(
            bindings(&cfg).get("move_up"),
            Some(&vec!["k".to_string(), "up".to_string()])
        );
    }

    #[test]
    fn none_unbinds_an_action() {
        let cfg = Config::parse("[keybindings]\nmove_down = \"none\"\n").unwrap();
        assert_eq!(
            bindings(&cfg).get("move_down"),
            Some(&vec!["none".to_string()])
        );
    }

    #[test]
    fn rejects_unknown_action() {
        assert!(Config::parse("[keybindings]\nfrobnicate = \"q\"\n").is_err());
    }

    #[test]
    fn rejects_invalid_key_name() {
        assert!(Config::parse("[keybindings]\ntoggle = \"ctrl+zz\"\n").is_err());
    }

    #[test]
    fn rejects_duplicate_keys_across_actions() {
        let raw = "[keybindings]\nsearch = \"q\"\nconfirm_quit = [\"q\", \"esc\"]\n";
        let err = Config::parse(raw).unwrap_err();
        assert!(err.contains("bound to multiple actions"), "{err}");
    }

    #[test]
    fn single_and_array_binding_forms_both_work() {
        let cfg =
            Config::parse("[keybindings]\nconfirm_quit = \"q\"\nseek_back = [\"x\", \"y\"]\n")
                .unwrap();
        assert_eq!(
            bindings(&cfg).get("confirm_quit"),
            Some(&vec!["q".to_string()])
        );
        assert_eq!(
            bindings(&cfg).get("seek_back"),
            Some(&vec!["x".to_string(), "y".to_string()])
        );
    }

    #[test]
    fn titles_parse_with_defaults_filling_in() {
        let cfg = Config::parse("[titles]\nqueue = \"Playlist\"\n").unwrap();
        assert_eq!(cfg.titles.filter, "Filter");
        assert_eq!(cfg.titles.queue, "Playlist");
        assert_eq!(cfg.titles.mini, "Mini Queue");
        assert_eq!(cfg.titles.info, "Info");
    }

    #[test]
    fn default_text_roundtrips() {
        let raw = default_config_text();
        let cfg = Config::parse(&raw).unwrap();
        assert_eq!(bindings(&cfg), &default_bindings());
        assert!(raw.contains("confirm_quit = [\"q\", \"esc\"]"));
    }

    #[test]
    fn action_names_roundtrip() {
        for (a, name) in ACTIONS {
            assert_eq!(action_from_str(name), Some(*a));
            assert_eq!(action_str(*a), *name);
        }
        assert_eq!(action_from_str("nope"), None);
    }

    #[test]
    fn legacy_config_is_migrated() {
        let raw = "mpv_binary = \"mpv\"\n[keybindings]\nj = \"move_down\"\nk = \"move_up\"\nA = \"add_mini\"\nq = \"none\"\n";
        let out = migrate_legacy(raw).unwrap();
        let cfg = Config::parse(&out).unwrap();
        assert_eq!(
            bindings(&cfg).get("move_down"),
            Some(&vec!["down".to_string(), "j".to_string()])
        );
        assert_eq!(bindings(&cfg).get("add_mini"), Some(&vec!["A".to_string()]));
        assert_eq!(cfg.mpv_binary, "mpv");
        assert!(out.contains("move_down = [\"down\", \"j\"]"));
        assert_eq!(
            bindings(&cfg).get("confirm_quit"),
            Some(&vec!["esc".to_string()])
        );
    }

    #[test]
    fn new_format_config_is_not_migrated() {
        let raw = "[keybindings]\nconfirm_quit = \"q\"\n";
        let out = migrate_legacy(raw);
        assert_eq!(out.as_deref(), None);
    }

    #[test]
    fn load_migrates_and_rewrites_file() {
        let dir = std::env::temp_dir().join(format!("rmp2-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("config.toml");
        let _ = fs::write(&path, "[keybindings]\nq = \"confirm_quit\"\n");
        let cfg = Config::load(&path).unwrap();
        assert!(cfg.keybindings.contains_key("confirm_quit"));
        let rewritten = fs::read_to_string(&path).unwrap();
        let cfg2 = Config::parse(&rewritten).unwrap();
        assert_eq!(cfg.keybindings, cfg2.keybindings);
        let _ = fs::remove_dir_all(&dir);
    }
}
