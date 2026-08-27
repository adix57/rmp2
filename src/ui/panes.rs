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
    Queue,
    Info,
}

impl Section {
    pub fn next(self) -> Self {
        match self {
            Section::Filter => Section::Queue,
            Section::Queue => Section::Info,
            Section::Info => Section::Filter,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Section::Filter => Section::Info,
            Section::Queue => Section::Filter,
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
    let mut bar = "#".repeat(filled);
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

pub fn queue_pane(
    frame: &mut Frame,
    area: Rect,
    media: &[MediaInfo],
    queue: &[i64],
    now: Option<&NowPlaying>,
    focused: bool,
    cursor: usize,
) {
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
            let artist = m
                .artist
                .as_ref()
                .filter(|a| !a.trim().is_empty())
                .map(|a| format!(" - {a}"))
                .unwrap_or_default();
            let text = format!("{play_mark}{fav} {name}{artist}");
            let mut style = Style::default();
            if focused && i == cursor {
                style = style.add_modifier(Modifier::REVERSED);
            }
            if now_marks {
                style = style.fg(Color::Green);
            }
            ListItem::new(text).style(style)
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

pub fn status_bar(frame: &mut Frame, area: Rect, snap: &Snapshot, msg: Option<&str>) {
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
    let mut spans = left;
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(right);
    let para = Paragraph::new(Line::from(spans));
    frame.render_widget(para, area);
}
