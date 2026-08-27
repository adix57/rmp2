use serde::{Deserialize, Serialize};

pub const FAVORITE_TAG: &str = "favorite";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn cycle(self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaInfo {
    pub id: i64,
    pub uri: String,
    pub name: String,
    pub kind: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration: Option<f64>,
    pub bitrate: Option<u64>,
    pub source: Option<String>,
    pub tags: Vec<String>,
    pub favorite: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagInfo {
    pub name: String,
    pub checked: bool,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NowPlaying {
    pub id: i64,
    pub position: f64,
    pub duration: Option<f64>,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub all_media: Vec<MediaInfo>,
    pub queue: Vec<i64>,
    pub tags: Vec<TagInfo>,
    pub search: Option<String>,
    pub selected: Option<MediaInfo>,
    pub now: Option<NowPlaying>,
    pub volume: i32,
    pub repeat: RepeatMode,
    pub shuffle: bool,
    pub notify: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Command {
    PlayPause,
    Next,
    Prev,
    Play {
        id: i64,
    },
    Select {
        id: i64,
    },
    Seek {
        delta: f64,
    },
    Volume {
        delta: i32,
    },
    SetVolume {
        volume: i32,
    },
    RepeatCycle,
    ShuffleToggle,
    ToggleTag {
        tag: String,
    },
    ClearTags,
    SetSearch {
        pattern: Option<String>,
    },
    Add {
        uri: String,
        name: Option<String>,
        tags: Vec<String>,
    },
    Update {
        id: i64,
        name: String,
        title: Option<String>,
        artist: Option<String>,
        tags: Vec<String>,
    },
    Delete {
        id: i64,
    },
    ToggleFavorite {
        id: i64,
    },
    Shutdown,
}
