use crate::config::Config;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::collections::HashMap;

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

fn action_from_str(name: &str) -> Option<Action> {
    Some(match name {
        "move_up" => Action::MoveUp,
        "move_down" => Action::MoveDown,
        "prev_section" => Action::PrevSection,
        "next_section" => Action::NextSection,
        "cycle_focus" => Action::CycleFocus,
        "cycle_focus_back" => Action::CycleFocusBack,
        "activate" => Action::Activate,
        "toggle" => Action::Toggle,
        "next_track" => Action::NextTrack,
        "prev_track" => Action::PrevTrack,
        "volume_up" => Action::VolumeUp,
        "volume_down" => Action::VolumeDown,
        "seek_back" => Action::SeekBack,
        "seek_fwd" => Action::SeekFwd,
        "repeat" => Action::Repeat,
        "shuffle" => Action::Shuffle,
        "add_media" => Action::AddMedia,
        "edit_media" => Action::EditMedia,
        "search" => Action::Search,
        "favorite" => Action::Favorite,
        "add_mini" => Action::AddMini,
        "focus_mini" => Action::FocusMini,
        "delete" => Action::Delete,
        "mini_move_up" => Action::MiniMoveUp,
        "mini_move_down" => Action::MiniMoveDown,
        "confirm_quit" => Action::ConfirmQuit,
        "detach" => Action::Detach,
        _ => return None,
    })
}

pub struct Keymap {
    map: HashMap<String, Action>,
}

impl Keymap {
    pub fn build(cfg: &Config) -> Self {
        let mut map = HashMap::new();
        for (key, action) in &cfg.keybindings {
            if let Some(a) = action_from_str(action) {
                map.insert(key.clone(), a);
            }
        }
        Keymap { map }
    }

    pub fn resolve(&self, key: KeyEvent) -> Option<Action> {
        key_string(key).and_then(|s| self.map.get(&s).copied())
    }
}

fn key_string(key: KeyEvent) -> Option<String> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                Some(format!("ctrl+{c}"))
            } else if c.is_ascii_uppercase() {
                Some(c.to_string())
            } else if c == ' ' {
                Some("space".into())
            } else {
                Some(c.to_string())
            }
        }
        KeyCode::Enter => Some("enter".into()),
        KeyCode::Esc => Some("esc".into()),
        KeyCode::Tab => Some("tab".into()),
        KeyCode::BackTab => Some("backtab".into()),
        KeyCode::Up => Some("up".into()),
        KeyCode::Down => Some("down".into()),
        KeyCode::Left => Some("left".into()),
        KeyCode::Right => Some("right".into()),
        KeyCode::F(n) => Some(format!("f{n}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode as KC, KeyEvent, KeyModifiers};

    fn press(code: KC, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn shift_letters_become_uppercase_keys() {
        assert_eq!(
            key_string(press(KC::Char('j'), KeyModifiers::NONE)).as_deref(),
            Some("j")
        );
        assert_eq!(
            key_string(press(KC::Char('J'), KeyModifiers::SHIFT)).as_deref(),
            Some("J")
        );
    }

    #[test]
    fn ctrl_letters_map_to_ctrl_prefix() {
        assert_eq!(
            key_string(press(KC::Char('j'), KeyModifiers::CONTROL)).as_deref(),
            Some("ctrl+j")
        );
        assert_eq!(
            key_string(press(KC::Char('k'), KeyModifiers::CONTROL)).as_deref(),
            Some("ctrl+k")
        );
    }

    #[test]
    fn uppercase_without_shift_flag_still_uppercase() {
        assert_eq!(
            key_string(press(KC::Char('Q'), KeyModifiers::NONE)).as_deref(),
            Some("Q")
        );
    }

    #[test]
    fn special_keys() {
        assert_eq!(
            key_string(press(KC::Enter, KeyModifiers::NONE)).as_deref(),
            Some("enter")
        );
        assert_eq!(
            key_string(press(KC::Tab, KeyModifiers::NONE)).as_deref(),
            Some("tab")
        );
        assert_eq!(
            key_string(press(KC::BackTab, KeyModifiers::NONE)).as_deref(),
            Some("backtab")
        );
        assert_eq!(
            key_string(press(KC::Char(' '), KeyModifiers::NONE)).as_deref(),
            Some("space")
        );
    }

    #[test]
    fn repeat_events_ignored() {
        let mut e = press(KC::Char('j'), KeyModifiers::NONE);
        e.kind = crossterm::event::KeyEventKind::Repeat;
        assert!(key_string(e).is_none());
    }

    #[test]
    fn default_bindings_resolve() {
        let cfg = Config::default();
        let km = Keymap::build(&cfg);
        assert_eq!(
            km.resolve(press(KC::Char('j'), KeyModifiers::NONE)),
            Some(Action::MoveDown)
        );
        assert_eq!(
            km.resolve(press(KC::Char('K'), KeyModifiers::SHIFT)),
            Some(Action::VolumeUp)
        );
        assert_eq!(
            km.resolve(press(KC::Char('Q'), KeyModifiers::SHIFT)),
            Some(Action::Detach)
        );
        assert_eq!(
            km.resolve(press(KC::Char(' '), KeyModifiers::NONE)),
            Some(Action::Toggle)
        );
    }
}
