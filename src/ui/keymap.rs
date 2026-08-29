use crate::config::{Config, action_from_str};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::collections::HashMap;

pub use crate::config::Action;

pub struct Keymap {
    map: HashMap<String, Action>,
}

impl Keymap {
    pub fn build(cfg: &Config) -> Self {
        let mut map = HashMap::new();
        for (action, keys) in &cfg.keybindings {
            let Some(a) = action_from_str(action) else {
                continue;
            };
            if keys.iter().any(|k| k == "none") {
                continue;
            }
            for k in keys {
                map.insert(k.clone(), a);
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
        KeyCode::Home => Some("home".into()),
        KeyCode::End => Some("end".into()),
        KeyCode::PageUp => Some("pageup".into()),
        KeyCode::PageDown => Some("pagedown".into()),
        KeyCode::Insert => Some("insert".into()),
        KeyCode::Delete => Some("delete".into()),
        KeyCode::Backspace => Some("backspace".into()),
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
    fn none_unbinds_a_default_action() {
        let raw = "[keybindings]\nmove_up = \"none\"\n";
        let cfg = Config::parse(raw).unwrap();
        let km = Keymap::build(&cfg);
        assert_eq!(
            km.resolve(press(KC::Char('j'), KeyModifiers::NONE)),
            Some(Action::MoveDown)
        );
        assert_eq!(km.resolve(press(KC::Char('k'), KeyModifiers::NONE)), None);
        assert_eq!(km.resolve(press(KC::Char('x'), KeyModifiers::NONE)), None);
    }

    #[test]
    fn remapped_action_uses_new_key() {
        let raw = "[keybindings]\ntoggle = \"x\"\n";
        let cfg = Config::parse(raw).unwrap();
        let km = Keymap::build(&cfg);
        assert_eq!(
            km.resolve(press(KC::Char('x'), KeyModifiers::NONE)),
            Some(Action::Toggle)
        );
        assert_eq!(km.resolve(press(KC::Char(' '), KeyModifiers::NONE)), None);
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
