use crate::proto::RepeatMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastState {
    pub volume: i32,
    pub repeat: RepeatMode,
    pub shuffle: bool,
    pub active_tags: Vec<String>,
    pub media_id: Option<i64>,
    pub position: f64,
    pub playing: bool,
}

impl Default for LastState {
    fn default() -> Self {
        Self {
            volume: 70,
            repeat: RepeatMode::Off,
            shuffle: false,
            active_tags: Vec::new(),
            media_id: None,
            position: 0.0,
            playing: false,
        }
    }
}

pub fn load(path: &Path) -> LastState {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, state: &LastState) {
    if let Ok(s) = serde_json::to_string(state) {
        let _ = fs::write(path, s);
    }
}
