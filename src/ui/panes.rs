use std::path::Path;

use crate::proto::{MediaInfo, NowPlaying, RepeatMode, Snapshot, TagInfo};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Filter,
    Mini,
    Queue,
    Info,
}

impl Section {
    pub fn next(self) -> Self {
        match self {
            Section::Filter => Section::Mini,
            Section::Mini => Section::Queue,
            Section::Queue => Section::Info,
            Section::Info => Section::Filter,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Section::Filter => Section::Info,
            Section::Mini => Section::Filter,
            Section::Queue => Section::Mini,
            Section::Info => Section::Queue,
        }
    }
}

pub const ASCII_BORDER: symbols::border::Set = symbols::border::Set {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};

fn frame_block(title: &str, focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_set(ASCII_BORDER)
        .title(format!(" {title} "))
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(if focused {
            Color::Cyan
        } else {
            Color::DarkGray
        }))
}

fn display_name(m: &MediaInfo) -> String {
    if let Some(t) = &m.title
        && !t.trim().is_empty()
    {
        return t.clone();
    }
    Path::new(&m.uri)
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| m.uri.clone())
}

fn fmt_time(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "--:--".to_string();
    }
    let s = secs as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn progress_bar(pos: f64, dur: f64, width: usize) -> String {
    let pct = if dur <= 0.0 {
        0.0
    } else {
        (pos / dur).clamp(0.0, 1.0)
    };
    let filled = (pct * width as f64) as usize;
    let mut bar = "=".repeat(filled);
    bar.push_str(&"-".repeat(width.saturating_sub(filled)));
    format!("[{bar}]")
}

pub fn filter_pane(frame: &mut Frame, area: Rect, tags: &[TagInfo], focused: bool, cursor: usize) {
    let items: Vec<ListItem> = tags
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mark = if t.checked { "x" } else { " " };
            let text = format!("[{mark}] {} ({})", t.name, t.count);
            let style = if focused && i == cursor {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();
    let list = List::new(items).block(frame_block("Filter", focused));
    frame.render_widget(list, area);
}

#[allow(clippy::too_many_arguments)]
pub fn queue_pane(
    frame: &mut Frame,
    area: Rect,
    media: &[MediaInfo],
    queue: &[i64],
    now: Option<&NowPlaying>,
    focused: bool,
    cursor: usize,
    search: Option<&str>,
) {
    let re = search
        .filter(|p| !p.trim().is_empty())
        .and_then(|p| regex::Regex::new(p).ok());
    let by_id: std::collections::HashMap<i64, &MediaInfo> =
        media.iter().map(|m| (m.id, m)).collect();
    let items: Vec<ListItem> = queue
        .iter()
        .enumerate()
        .filter_map(|(i, id)| by_id.get(id).copied().map(|m| (i, m)))
        .map(|(i, m)| {
            let now_marks = now.filter(|n| n.id == m.id).is_some();
            let play_mark = if now_marks { ">" } else { " " };
            let fav = if m.favorite { "*" } else { " " };
            let name = display_name(m);
            let mut spans = vec![Span::raw(format!("{play_mark}{fav} "))];
            spans.extend(highlight_matches(&name, re.as_ref()));
            if let Some(a) = m.artist.as_ref().filter(|a| !a.trim().is_empty()) {
                spans.push(Span::raw(format!(" - {a}")));
            }
            let mut style = Style::default();
            if focused && i == cursor {
                style = style.add_modifier(Modifier::REVERSED);
            }
            if now_marks {
                style = style.fg(Color::Green);
            }
            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();
    let list = List::new(items).block(frame_block("Queue", focused));
    frame.render_widget(list, area);
}

fn highlight_matches(text: &str, re: Option<&regex::Regex>) -> Vec<Span<'static>> {
    let mut spans: Vec<Span> = Vec::new();
    let Some(re) = re else {
        return vec![Span::raw(text.to_string())];
    };
    let lowered = text.to_lowercase();
    let mut last = 0;
    for m in re.find_iter(&lowered) {
        if m.start() > last {
            spans.push(Span::raw(text[last..m.start()].to_string()));
        }
        spans.push(Span::styled(
            text[m.start()..m.end()].to_string(),
            Style::default().add_modifier(Modifier::UNDERLINED),
        ));
        last = m.end();
    }
    if last < text.len() {
        spans.push(Span::raw(text[last..].to_string()));
    }
    if spans.is_empty() {
        spans.push(Span::raw(text.to_string()));
    }
    spans
}

pub fn mini_pane(
    frame: &mut Frame,
    area: Rect,
    media: &[MediaInfo],
    mini_queue: &[i64],
    now: Option<&NowPlaying>,
    focused: bool,
    cursor: usize,
) {
    let by_id: std::collections::HashMap<i64, &MediaInfo> =
        media.iter().map(|m| (m.id, m)).collect();
    let items: Vec<ListItem> = mini_queue
        .iter()
        .enumerate()
        .filter_map(|(i, id)| by_id.get(id).copied().map(|m| (i, m)))
        .map(|(i, m)| {
            let now_marks = now.filter(|n| n.id == m.id).is_some();
            let mark = if now_marks { ">" } else { " " };
            let name = display_name(m);
            let mut spans = vec![Span::raw(format!("{mark}{}. {} ", i + 1, name))];
            if let Some(a) = m.artist.as_ref().filter(|a| !a.trim().is_empty()) {
                spans.push(Span::raw(format!("- {a}")));
            }
            let mut style = Style::default();
            if focused && i == cursor {
                style = style.add_modifier(Modifier::REVERSED);
            }
            if now_marks {
                style = style.fg(Color::Green);
            }
            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();
    let list = List::new(items).block(frame_block("Queue", focused));
    frame.render_widget(list, area);
}

pub fn state_pane(frame: &mut Frame, area: Rect, selected: Option<&MediaInfo>, focused: bool) {
    let mut lines = Vec::new();
    if let Some(m) = selected {
        lines.push(Line::from(format!("Title:     {}", display_name(m))));
        if let Some(a) = m.artist.as_ref().filter(|a| !a.trim().is_empty()) {
            lines.push(Line::from(format!("Artist:    {a}")));
        }
        let sep_w = area.width.saturating_sub(2) as usize;
        lines.push(Line::from(Span::styled(
            "-".repeat(sep_w),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(format!("Type:      {}", m.kind)));
        if let Some(s) = &m.source {
            lines.push(Line::from(format!("Source:    {s}")));
        }
        if let Some(d) = m.duration {
            lines.push(Line::from(format!("Duration:  {}", fmt_time(d))));
        }
        if let Some(b) = m.bitrate {
            lines.push(Line::from(format!("Bitrate:   {} bps", b)));
        }
        if !m.tags.is_empty() {
            lines.push(Line::from(Span::styled(
                "-".repeat(sep_w),
                Style::default().fg(Color::DarkGray),
            )));
            for t in &m.tags {
                lines.push(Line::from(format!("- {t}")));
            }
        }
    } else {
        lines.push(Line::from("Nothing selected"));
    }
    let para = Paragraph::new(lines)
        .block(frame_block("Info", focused))
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

pub fn search_bar(
    frame: &mut Frame,
    area: Rect,
    text: &str,
    cursor: usize,
    count: usize,
    total: usize,
    invalid: bool,
) {
    let mut spans: Vec<Span> = Vec::new();
    let prompt = Span::styled(
        "/",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    spans.push(prompt);
    let cur = cursor.min(text.len());
    spans.push(Span::raw(text[..cur].to_string()));
    spans.push(Span::styled(
        " ",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::REVERSED),
    ));
    spans.push(Span::raw(text[cur..].to_string()));
    let counter = if invalid {
        Span::styled("0/0", Style::default().fg(Color::Red))
    } else {
        Span::styled(
            format!("{count}/{total}"),
            Style::default().fg(Color::DarkGray),
        )
    };
    let left_w: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let right_w = counter.content.chars().count();
    let gap = area.width.saturating_sub((left_w + right_w) as u16) as usize;
    spans.push(Span::raw(" ".repeat(gap)));
    spans.push(counter);
    let para = Paragraph::new(Line::from(spans));
    frame.render_widget(para, area);
}

pub struct StatusBarHits {
    pub rep: Option<Rect>,
    pub shf: Option<Rect>,
}

pub fn status_bar(
    frame: &mut Frame,
    area: Rect,
    snap: &Snapshot,
    msg: Option<&str>,
) -> StatusBarHits {
    let mut left: Vec<Span> = Vec::new();
    if let Some(text) = msg {
        left.push(Span::styled(
            text,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    } else if let Some(n) = &snap.now {
        let title = snap
            .all_media
            .iter()
            .find(|m| m.id == n.id)
            .map(display_name)
            .unwrap_or_else(|| "unknown".into());
        let dur = n.duration.unwrap_or(0.0);
        left.push(Span::raw(format!(
            "{} / {} ",
            fmt_time(n.position),
            fmt_time(dur)
        )));
        let w = area.width.saturating_sub(46) as usize;
        left.push(Span::raw(progress_bar(n.position, dur, w.min(20))));
        left.push(Span::styled(
            format!(" {title}"),
            Style::default().fg(Color::Green),
        ));
    } else {
        left.push(Span::raw("stopped"));
    }
    let lbl = Style::default().fg(Color::Yellow);
    let mut right: Vec<Span> = Vec::new();
    right.push(Span::raw("  "));
    right.push(Span::styled("vol", lbl));
    right.push(Span::raw(format!(" {:>3}", snap.volume)));
    let rep = match snap.repeat {
        RepeatMode::Off => "off",
        RepeatMode::All => "all",
        RepeatMode::One => "one",
    };
    right.push(Span::raw("  "));
    right.push(Span::styled("rep", lbl));
    right.push(Span::raw(format!(" {rep}")));
    right.push(Span::raw("  "));
    right.push(Span::styled("shf", lbl));
    right.push(Span::raw(if snap.shuffle { " on" } else { " off" }));
    if let Some(tags) = snap.tags.iter().find(|t| t.checked) {
        right.push(Span::raw("  "));
        right.push(Span::styled("tags", lbl));
        right.push(Span::raw(format!(":{}", tags.name)));
    }
    let left_w: usize = left.iter().map(|s| s.content.chars().count()).sum();
    let right_w: usize = right.iter().map(|s| s.content.chars().count()).sum();
    let gap = area.width.saturating_sub((left_w + right_w) as u16) as usize;
    let x0 = area.x as i64 + area.width as i64 - right_w as i64;
    let mut x = x0;
    let mut hit_rep = None;
    let mut hit_shf = None;
    for (i, s) in right.iter().enumerate() {
        let w = s.content.chars().count() as u16;
        let rect = Rect::new(x.max(0) as u16, area.y, w, 1);
        match i {
            4 => hit_rep = Some(rect),
            7 => hit_shf = Some(rect),
            _ => {}
        }
        x += w as i64;
    }
    let mut spans = left;
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(right);
    let para = Paragraph::new(Line::from(spans));
    frame.render_widget(para, area);
    StatusBarHits {
        rep: hit_rep,
        shf: hit_shf,
    }
}
