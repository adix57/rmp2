pub mod dialog;
pub mod keymap;
pub mod panes;

use crate::config::Config;
use crate::proto::{Command, Snapshot};
use crate::ui::dialog::{Dialog, DialogOutcome, FormCommand};
use crate::ui::keymap::{Action, Keymap};
use crate::ui::panes::{Section, filter_pane, queue_pane, search_bar, state_pane, status_bar};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind, poll, read,
};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Clear;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

pub struct Connection {
    cmd_tx: Sender<Command>,
    snap_rx: Receiver<serde_json::Value>,
}

impl Connection {
    pub fn connect(path: &Path) -> io::Result<Connection> {
        let stream = UnixStream::connect(path)?;
        let cmd_stream = stream.try_clone()?;
        let (cmd_tx, cmd_rx) = mpsc::channel();
        thread::spawn(move || writer_loop(cmd_stream, cmd_rx));
        let (snap_tx, snap_rx) = mpsc::channel();
        thread::spawn(move || reader_loop(stream, snap_tx));
        Ok(Connection { cmd_tx, snap_rx })
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.cmd_tx.send(cmd);
    }

    fn pull(&mut self) -> Option<Snapshot> {
        let mut last = self.snap_rx.try_recv().ok();
        while let Ok(v) = self.snap_rx.try_recv() {
            last = Some(v);
        }
        last.and_then(|v| serde_json::from_value(v).ok())
    }
}

fn writer_loop(mut stream: UnixStream, rx: Receiver<Command>) {
    for cmd in rx {
        let mut body = serde_json::to_string(&cmd).unwrap_or_default();
        body.push('\n');
        if stream.write_all(body.as_bytes()).is_err() {
            break;
        }
    }
}

fn reader_loop(stream: UnixStream, snap_tx: Sender<serde_json::Value>) {
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line)
            && snap_tx.send(v).is_err()
        {
            break;
        }
    }
}

pub struct App {
    connection: Connection,
    cfg: Config,
    keymap: Keymap,
    snap: Option<Snapshot>,
    section: Section,
    queue_cursor: usize,
    tag_cursor: usize,
    search: Option<SearchBar>,
    dialog: Option<Dialog>,
    quit: bool,
    shutdown: bool,
    msg: Option<(String, Instant)>,
    queue_area: Rect,
    filter_area: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchBarOutcome {
    Done,
    Cancel,
    None,
}

struct SearchBar {
    text: String,
    cursor: usize,
}

impl SearchBar {
    fn new() -> Self {
        SearchBar {
            text: String::new(),
            cursor: 0,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> SearchBarOutcome {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return SearchBarOutcome::None;
        }
        match key.code {
            KeyCode::Esc => SearchBarOutcome::Cancel,
            KeyCode::Enter => SearchBarOutcome::Done,
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.text.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
                SearchBarOutcome::None
            }
            KeyCode::Delete => {
                if self.cursor < self.text.len() {
                    self.text.remove(self.cursor);
                }
                SearchBarOutcome::None
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                SearchBarOutcome::None
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(self.text.len());
                SearchBarOutcome::None
            }
            KeyCode::Home => {
                self.cursor = 0;
                SearchBarOutcome::None
            }
            KeyCode::End => {
                self.cursor = self.text.len();
                SearchBarOutcome::None
            }
            KeyCode::Char(c) => {
                self.text.insert(self.cursor, c);
                self.cursor += 1;
                SearchBarOutcome::None
            }
            _ => SearchBarOutcome::None,
        }
    }

    fn pattern(&self) -> Option<String> {
        let t = self.text.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    }
}

impl App {
    fn new(connection: Connection, cfg: Config) -> Self {
        let keymap = Keymap::build(&cfg);
        App {
            connection,
            cfg,
            keymap,
            snap: None,
            section: Section::Queue,
            queue_cursor: 0,
            tag_cursor: 0,
            search: None,
            dialog: None,
            quit: false,
            shutdown: false,
            msg: None,
            queue_area: Rect::default(),
            filter_area: Rect::default(),
        }
    }

    fn pull(&mut self) {
        if let Some(snap) = self.connection.pull() {
            if let Some(n) = &snap.notify {
                self.msg = Some((n.clone(), Instant::now()));
            }
            if let Some(sel) = snap.selected.clone()
                && let Some(i) = snap.queue.iter().position(|id| *id == sel.id)
            {
                self.queue_cursor = i;
            }
            self.queue_cursor = self.queue_cursor.min(snap.queue.len().saturating_sub(1));
            self.tag_cursor = self.tag_cursor.min(snap.tags.len().saturating_sub(1));
            self.snap = Some(snap);
        }
    }

    fn on_key(&mut self, key: KeyEvent) {
        if let Some(dlg) = self.dialog.as_mut() {
            let known: Vec<String> = self
                .snap
                .as_ref()
                .map(|s| s.tags.iter().map(|t| t.name.clone()).collect())
                .unwrap_or_default();
            match dlg.handle_key(key, &known) {
                DialogOutcome::Submit(cmd) => {
                    self.apply_form(cmd);
                    self.dialog = None;
                }
                DialogOutcome::ConfirmExitYes => {
                    self.shutdown = true;
                    self.quit = true;
                    self.dialog = None;
                }
                DialogOutcome::Cancel => {
                    self.dialog = None;
                }
                DialogOutcome::None => {}
            }
            return;
        }
        if let Some(s) = self.search.as_mut() {
            let before = s.text.clone();
            match s.handle_key(key) {
                SearchBarOutcome::Done => self.search = None,
                SearchBarOutcome::Cancel => {
                    self.connection.send(Command::SetSearch { pattern: None });
                    self.search = None;
                }
                SearchBarOutcome::None => {}
            }
            if let Some(s) = self.search.as_mut()
                && s.text != before
            {
                self.connection.send(Command::SetSearch {
                    pattern: s.pattern(),
                });
            }
            return;
        }
        match self.keymap.resolve(key) {
            Some(Action::ConfirmQuit) => self.dialog = Some(Dialog::ConfirmExit),
            Some(Action::Detach) => {
                self.quit = true;
                self.shutdown = false;
            }
            Some(Action::AddMedia) => self.dialog = Some(Dialog::Add(dialog::Form::new_add())),
            Some(Action::EditMedia) => {
                if let Some(sel) = self.snap.as_ref().and_then(|s| s.selected.clone()) {
                    self.dialog = Some(Dialog::Edit(dialog::Form::new_edit(&sel)));
                }
            }
            Some(Action::Search) => self.search = Some(SearchBar::new()),
            Some(Action::MoveUp) => self.move_cursor(-1),
            Some(Action::MoveDown) => self.move_cursor(1),
            Some(Action::PrevSection) => self.section = self.section.prev(),
            Some(Action::NextSection) | Some(Action::CycleFocus) => {
                self.section = self.section.next()
            }
            Some(Action::CycleFocusBack) => self.section = self.section.prev(),
            Some(Action::Activate) => match self.section {
                Section::Queue => self.play_cursor(),
                Section::Filter => self.toggle_tag_cursor(),
                Section::Info => {}
            },
            Some(Action::Toggle) => match self.section {
                Section::Queue => self.connection.send(Command::PlayPause),
                Section::Filter => self.toggle_tag_cursor(),
                Section::Info => {}
            },
            Some(Action::NextTrack) => self.connection.send(Command::Next),
            Some(Action::PrevTrack) => self.connection.send(Command::Prev),
            Some(Action::VolumeUp) => self.connection.send(Command::Volume {
                delta: self.cfg.volume_step,
            }),
            Some(Action::VolumeDown) => self.connection.send(Command::Volume {
                delta: -self.cfg.volume_step,
            }),
            Some(Action::SeekFwd) => self.connection.send(Command::Seek {
                delta: self.cfg.seek_step,
            }),
            Some(Action::SeekBack) => self.connection.send(Command::Seek {
                delta: -self.cfg.seek_step,
            }),
            Some(Action::Repeat) => self.connection.send(Command::RepeatCycle),
            Some(Action::Shuffle) => self.connection.send(Command::ShuffleToggle),
            Some(Action::Favorite) => {
                if let Some(sel) = self.snap.as_ref().and_then(|s| s.selected.clone()) {
                    self.connection.send(Command::ToggleFavorite { id: sel.id });
                }
            }
            None => {}
        }
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        if self.dialog.is_some() || self.search.is_some() {
            return;
        }
        let MouseEvent {
            kind, column, row, ..
        } = m;
        if let MouseEventKind::Down(MouseButton::Left) = kind {
            let pos = ratatui::layout::Position::new(column, row);
            if self.queue_area.contains(pos) {
                let rel = row.saturating_sub(self.queue_area.y + 1);
                let len = self.snap.as_ref().map(|s| s.queue.len()).unwrap_or(0);
                if usize::from(rel) < len {
                    self.queue_cursor = usize::from(rel);
                    if let Some(id) = self
                        .snap
                        .as_ref()
                        .and_then(|s| s.queue.get(self.queue_cursor))
                        .copied()
                    {
                        self.connection.send(Command::Select { id });
                    }
                }
            } else if self.filter_area.contains(pos) {
                let rel = row.saturating_sub(self.filter_area.y + 1);
                if let Some(t) = self
                    .snap
                    .as_ref()
                    .and_then(|s| s.tags.get(usize::from(rel)))
                {
                    let name = t.name.clone();
                    self.connection.send(Command::ToggleTag { tag: name });
                }
            }
        }
    }

    fn apply_form(&mut self, cmd: FormCommand) {
        match cmd {
            FormCommand::Add { uri, title, tags } => {
                self.connection.send(Command::Add {
                    uri,
                    title: if title.is_empty() { None } else { Some(title) },
                    tags,
                });
            }
            FormCommand::Update {
                id,
                title,
                artist,
                tags,
            } => {
                self.connection.send(Command::Update {
                    id,
                    title: if title.is_empty() { None } else { Some(title) },
                    artist: if artist.is_empty() {
                        None
                    } else {
                        Some(artist)
                    },
                    tags,
                });
            }
        }
    }

    fn move_cursor(&mut self, delta: i64) {
        match self.section {
            Section::Queue => {
                let len = self.snap.as_ref().map(|s| s.queue.len()).unwrap_or(0);
                if len == 0 {
                    return;
                }
                let new = (self.queue_cursor as i64 + delta).clamp(0, len as i64 - 1) as usize;
                if new != self.queue_cursor {
                    self.queue_cursor = new;
                    if let Some(id) = self.snap.as_ref().and_then(|s| s.queue.get(new)).copied() {
                        self.connection.send(Command::Select { id });
                    }
                }
            }
            Section::Filter => {
                let len = self.snap.as_ref().map(|s| s.tags.len()).unwrap_or(0);
                if len == 0 {
                    return;
                }
                self.tag_cursor =
                    (self.tag_cursor as i64 + delta).clamp(0, len as i64 - 1) as usize;
            }
            Section::Info => {}
        }
    }

    fn play_cursor(&mut self) {
        if let Some(id) = self
            .snap
            .as_ref()
            .and_then(|s| s.queue.get(self.queue_cursor))
            .copied()
        {
            self.connection.send(Command::Play { id });
        }
    }

    fn toggle_tag_cursor(&mut self) {
        if let Some(t) = self.snap.as_ref().and_then(|s| s.tags.get(self.tag_cursor)) {
            let name = t.name.clone();
            self.connection.send(Command::ToggleTag { tag: name });
        }
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.area();
        let [main, status] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);
        let [filter, queue, state] = Layout::horizontal([
            Constraint::Ratio(1, 4),
            Constraint::Ratio(2, 4),
            Constraint::Ratio(1, 4),
        ])
        .areas(main);
        self.filter_area = filter;
        self.queue_area = queue;
        let snap = self.snap.as_ref();
        match snap {
            Some(s) => {
                filter_pane(
                    frame,
                    filter,
                    &s.tags,
                    self.section == Section::Filter,
                    self.tag_cursor,
                );
                queue_pane(
                    frame,
                    queue,
                    &s.all_media,
                    &s.queue,
                    s.now.as_ref(),
                    self.section == Section::Queue,
                    self.queue_cursor,
                    s.search.as_deref(),
                );
                state_pane(
                    frame,
                    state,
                    s.selected.as_ref(),
                    self.section == Section::Info,
                );
                let msg = self
                    .msg
                    .as_ref()
                    .filter(|(_, t)| t.elapsed() < Duration::from_secs(4))
                    .map(|(m, _)| m.as_str());
                if let Some(sb) = &self.search {
                    let invalid =
                        sb.pattern().is_some() && regex::Regex::new(sb.text.trim()).is_err();
                    search_bar(
                        frame,
                        status,
                        &sb.text,
                        sb.cursor,
                        s.queue.len(),
                        s.all_media.len(),
                        invalid,
                    );
                } else {
                    status_bar(frame, status, s, msg);
                }
            }
            None => {
                frame.render_widget(ratatui::widgets::Paragraph::new("connecting..."), main);
            }
        }
        if let Some(d) = &self.dialog {
            frame.render_widget(Clear, queue);
            dialog::form_dialog(frame, queue, d);
        }
    }
}

pub fn run(connection: Connection, cfg: Config) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let res = (|| {
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
        let mut app = App::new(connection, cfg);
        while !app.quit {
            app.pull();
            terminal.draw(|f| app.render(f))?;
            if poll(Duration::from_millis(100))? {
                match read()? {
                    Event::Key(k) => app.on_key(k),
                    Event::Mouse(m) => app.on_mouse(m),
                    Event::Resize(..) => {}
                    _ => {}
                }
            }
        }
        if app.shutdown {
            app.connection.send(Command::Shutdown);
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
        Ok::<(), io::Error>(())
    })();
    ratatui::restore();
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn searching_accumulates_text_and_live_pattern() {
        let mut sb = SearchBar::new();
        assert_eq!(sb.pattern(), None);
        for c in "beat".chars() {
            assert_eq!(sb.handle_key(key(KeyCode::Char(c))), SearchBarOutcome::None);
        }
        assert_eq!(sb.pattern(), Some("beat".to_string()));
        assert_eq!(sb.handle_key(key(KeyCode::Esc)), SearchBarOutcome::Cancel);
    }

    #[test]
    fn search_backspace_and_enter() {
        let mut sb = SearchBar::new();
        for c in "abc".chars() {
            sb.handle_key(key(KeyCode::Char(c)));
        }
        sb.handle_key(key(KeyCode::Backspace));
        assert_eq!(sb.text, "ab");
        assert_eq!(sb.handle_key(key(KeyCode::Enter)), SearchBarOutcome::Done);
    }

    #[test]
    fn search_empty_or_whitespace_is_no_pattern() {
        let mut sb = SearchBar::new();
        assert_eq!(sb.pattern(), None);
        sb.handle_key(key(KeyCode::Char(' ')));
        sb.handle_key(key(KeyCode::Char(' ')));
        assert_eq!(sb.pattern(), None);
    }
}
