use ratatui::{
    layout::Rect,
    prelude::*,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::app::{App, Focus, LogStyle};

const ORANGE: Color = Color::Rgb(255, 140, 0);

fn style_decoration(s: LogStyle) -> (&'static str, Color, Color) {
    match s {
        LogStyle::In       => ("[IN ]", Color::Green,   Color::Green),
        LogStyle::Out      => ("[OUT]", ORANGE,          ORANGE),
        LogStyle::OutError => ("[ERR]", Color::Red,      Color::Red),
        LogStyle::System   => ("[---]", Color::DarkGray, Color::White),
        LogStyle::Error    => ("[ERR]", Color::Red,      Color::Red),
    }
}

/// Split `text` into slices of at most `max_chars` display columns.
/// Soft-breaks at the last whitespace within each chunk; hard-breaks otherwise.
fn split_to_width(text: &str, max_chars: usize) -> Vec<String> {
    if max_chars == 0 || text.is_empty() {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut remaining = text;
    loop {
        if remaining.chars().count() <= max_chars {
            out.push(remaining.to_string());
            break;
        }
        // byte offset of char at column max_chars
        let hard = remaining
            .char_indices()
            .nth(max_chars)
            .map(|(i, _)| i)
            .unwrap_or(remaining.len());
        // prefer breaking at last whitespace
        let split = remaining[..hard]
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(hard);
        out.push(remaining[..split].to_string());
        remaining = remaining[split..].trim_start_matches(|c: char| c.is_whitespace());
        if remaining.is_empty() { break; }
    }
    out
}

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let is_focused = app.focus == Focus::Logs;

    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Compute inner dimensions directly — borders are always 1 col/row each.
    // This avoids consuming the Block before we can attach it to the Paragraph.
    let inner_w = (area.width as usize).saturating_sub(2);
    let inner_h = (area.height as usize).saturating_sub(2);

    let block = Block::default()
        .title(" Logs ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let tunnel = app.tunnels.get(app.selected_tunnel);

    let Some(tunnel) = tunnel else {
        let empty = Paragraph::new("  No tunnel selected")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(empty, area);
        return;
    };

    // Fixed prefix layout: "HH:MM:SS " (9) + "[TAG] " (6) = 15 columns
    const PREFIX: usize = 15;
    let text_w = inner_w.saturating_sub(PREFIX);
    let indent = " ".repeat(PREFIX);

    // Build complete flat display buffer from the raw log entries.
    // Each entry may expand into multiple visual rows; we own the wrapping.
    let mut buffer: Vec<Line> = Vec::new();

    for log in &tunnel.logs {
        let (badge, badge_color, text_color) = style_decoration(log.style);
        let chunks = split_to_width(&log.text, text_w.max(1));
        for (i, chunk) in chunks.iter().enumerate() {
            if i == 0 {
                buffer.push(Line::from(vec![
                    Span::styled(
                        format!("{} ", log.timestamp),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{badge} "),
                        Style::default().fg(badge_color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(chunk.clone(), Style::default().fg(text_color)),
                ]));
            } else {
                buffer.push(Line::from(vec![
                    Span::raw(indent.clone()),
                    Span::styled(chunk.clone(), Style::default().fg(text_color)),
                ]));
            }
        }
    }

    // Scroll: clamp and slice — no .scroll() on the Paragraph, no .wrap().
    let total = buffer.len();
    let max_scroll = total.saturating_sub(inner_h);
    let scroll = if tunnel.auto_scroll {
        max_scroll
    } else {
        (tunnel.scroll_offset as usize).min(max_scroll)
    };

    let visible: Vec<Line> = buffer
        .into_iter()
        .skip(scroll)
        .take(inner_h)
        .collect();

    frame.render_widget(Paragraph::new(visible).block(block), area);
}


