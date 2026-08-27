use crate::proto::MediaInfo;
use crate::ui::panes::ASCII_BORDER;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub enum FormCommand {
    Add {
        uri: String,
        title: String,
        tags: Vec<String>,
    },
    Update {
        id: i64,
        title: String,
        artist: String,
        tags: Vec<String>,
    },
}

pub enum FormOutcome {
    Submit(FormCommand),
    Cancel,
    None,
}

pub struct Form {
    mode: FormMode,
    fields: Vec<Field>,
    focus: usize,
    suggestions: Vec<String>,
    sug_idx: usize,
}

pub enum FormMode {
    Add,
    Edit { id: i64 },
}

struct Field {
    label: &'static str,
    value: String,
    cursor: usize,
}

impl Field {
    fn new(label: &'static str, value: &str) -> Self {
        Field {
            label,
            value: value.to_string(),
            cursor: value.len(),
        }
    }
}

impl Form {
    pub fn new_add() -> Self {
        Form {
            mode: FormMode::Add,
            fields: vec![
                Field::new("uri", ""),
                Field::new("title", ""),
                Field::new("tags", ""),
            ],
            focus: 0,
            suggestions: Vec::new(),
            sug_idx: 0,
        }
    }

    pub fn new_edit(m: &MediaInfo) -> Self {
        Form {
            mode: FormMode::Edit { id: m.id },
            fields: vec![
                Field::new("title", m.title.as_deref().unwrap_or("")),
                Field::new("artist", m.artist.as_deref().unwrap_or("")),
                Field::new("tags", &m.tags.join(", ")),
            ],
            focus: 0,
            suggestions: Vec::new(),
            sug_idx: 0,
        }
    }

    fn tags_index(&self) -> usize {
        match self.mode {
            FormMode::Add => 2,
            FormMode::Edit { .. } => 2,
        }
    }

    fn tag_tokens(&self) -> Vec<String> {
        let i = self.tags_index();
        self.fields[i]
            .value
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn current_token(&self) -> &str {
        let i = self.tags_index();
        let v = self.fields[i].value.as_str();
        v.rsplit(',').next().unwrap_or("").trim()
    }

    fn refresh_suggestions(&mut self, known: &[String]) {
        self.suggestions = if self.focus == self.tags_index() {
            let prefix = self.current_token().to_lowercase();
            let taken = self.tag_tokens();
            known
                .iter()
                .filter(|t| t.len() > prefix.len() && t.starts_with(&prefix) && !taken.contains(t))
                .take(10)
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        self.sug_idx = 0;
    }

    fn accept_suggestion(&mut self) {
        let i = self.tags_index();
        if let Some(sug) = self.suggestions.get(self.sug_idx).cloned() {
            let token_len = self.current_token().len();
            let f = &mut self.fields[i];
            f.value.push_str(&sug[token_len..]);
            f.value.push_str(", ");
            f.cursor = f.value.len();
            self.suggestions.clear();
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, known: &[String]) -> FormOutcome {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return FormOutcome::None;
        }
        let focus = self.focus;
        let f = &mut self.fields[focus];
        match key.code {
            KeyCode::Esc => return FormOutcome::Cancel,
            KeyCode::Enter => {
                if self.suggestions.is_empty() {
                    if focus + 1 < self.fields.len() {
                        self.focus = focus + 1;
                    } else {
                        return FormOutcome::Submit(self.build());
                    }
                } else {
                    self.accept_suggestion();
                }
            }
            KeyCode::Tab => {
                if self.suggestions.is_empty() {
                    self.focus = (focus + 1) % self.fields.len();
                } else {
                    self.accept_suggestion();
                }
            }
            KeyCode::BackTab => {
                self.focus = (focus + self.fields.len() - 1) % self.fields.len();
            }
            KeyCode::Right => {
                if !self.suggestions.is_empty() {
                    self.accept_suggestion();
                } else if f.cursor < f.value.len() {
                    f.cursor += 1;
                }
            }
            KeyCode::Left => {
                if f.cursor > 0 {
                    f.cursor -= 1;
                }
            }
            KeyCode::Up => {
                if !self.suggestions.is_empty() {
                    self.sug_idx = self.sug_idx.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if !self.suggestions.is_empty() {
                    self.sug_idx = (self.sug_idx + 1).min(self.suggestions.len().saturating_sub(1));
                }
            }
            KeyCode::Home => f.cursor = 0,
            KeyCode::End => f.cursor = f.value.len(),
            KeyCode::Backspace => {
                if f.cursor > 0 {
                    f.value.remove(f.cursor - 1);
                    f.cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if f.cursor < f.value.len() {
                    f.value.remove(f.cursor);
                }
            }
            KeyCode::Char(c) => {
                f.value.insert(f.cursor, c);
                f.cursor += 1;
            }
            _ => return FormOutcome::None,
        }
        self.refresh_suggestions(known);
        FormOutcome::None
    }

    fn build(&self) -> FormCommand {
        match self.mode {
            FormMode::Add => FormCommand::Add {
                uri: self.fields[0].value.trim().to_string(),
                title: self.fields[1].value.trim().to_string(),
                tags: self.tag_tokens(),
            },
            FormMode::Edit { id } => FormCommand::Update {
                id,
                title: self.fields[0].value.trim().to_string(),
                artist: self.fields[1].value.trim().to_string(),
                tags: self.tag_tokens(),
            },
        }
    }
}

pub enum Dialog {
    ConfirmExit,
    Add(Form),
    Edit(Form),
}

pub enum DialogOutcome {
    Submit(FormCommand),
    ConfirmExitYes,
    Cancel,
    None,
}

impl Dialog {
    pub fn title(&self) -> &'static str {
        match self {
            Dialog::ConfirmExit => " Quit ",
            Dialog::Add(_) => " Add Media ",
            Dialog::Edit(_) => " Edit Media ",
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, known: &[String]) -> DialogOutcome {
        match self {
            Dialog::ConfirmExit => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    DialogOutcome::ConfirmExitYes
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => DialogOutcome::Cancel,
                _ => DialogOutcome::None,
            },
            Dialog::Add(f) | Dialog::Edit(f) => match f.handle_key(key, known) {
                FormOutcome::Submit(c) => DialogOutcome::Submit(c),
                FormOutcome::Cancel => DialogOutcome::Cancel,
                FormOutcome::None => DialogOutcome::None,
            },
        }
    }
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [rect] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    let [rect] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(rect);
    rect
}

pub fn confirm_dialog(frame: &mut Frame, area: Rect) {
    let rect = centered(area, 40, 5);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ASCII_BORDER)
        .title(" Quit ")
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(Color::Yellow));
    let text = vec![
        Line::from("Quit and stop rmp? (y/n)"),
        Line::from(""),
        Line::from("Shift+q detaches and keeps playing instead"),
    ];
    frame.render_widget(block, rect);
    frame.render_widget(
        Paragraph::new(text).alignment(Alignment::Center),
        Rect {
            y: rect.y + 2,
            height: rect.height.saturating_sub(2),
            ..rect
        },
    );
}

pub fn form_dialog(frame: &mut Frame, area: Rect, dialog: &Dialog) {
    if let Dialog::ConfirmExit = dialog {
        confirm_dialog(frame, area);
        return;
    }
    let form = match dialog {
        Dialog::Add(f) | Dialog::Edit(f) => f,
        Dialog::ConfirmExit => unreachable!(),
    };
    let tags_field = form.tags_index() == form.focus && !form.suggestions.is_empty();
    let height = if tags_field { 14 } else { 10 };
    let rect = centered(area, 60, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ASCII_BORDER)
        .title(dialog.title())
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let mut lines = Vec::new();
    for (i, f) in form.fields.iter().enumerate() {
        let focus = i == form.focus;
        let arrow = if focus { ">" } else { " " };
        let mut value = f.value.clone();
        if focus {
            let pos = f.cursor.min(value.len());
            value.insert(pos, '_');
        }
        let label = format!("{arrow} {:<6} ", f.label);
        let span_label = Span::styled(
            label,
            if focus {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        );
        lines.push(Line::from(vec![span_label, Span::raw(value)]));
    }
    if tags_field {
        lines.push(Line::from(""));
        for (i, s) in form.suggestions.iter().enumerate() {
            let mark = if i == form.sug_idx { ">" } else { " " };
            lines.push(Line::from(format!("   {mark} {s}")));
        }
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from("   tab/enter next | esc cancel | enter submit"));
    }
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        Rect {
            y: inner.y + 1,
            height: inner.height.saturating_sub(2),
            ..inner
        },
    );
}
