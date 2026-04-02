use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};

use crate::tui::app::{App, TunnelStatus};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    const MIN_WIDTH: u16 = 56;

    // Measure content width from tunnel rows
    let rows: Vec<String> = app
        .tunnels
        .iter()
        .map(|t| {
            let status = match t.status {
                TunnelStatus::Active => "ACTIVE  ",
                TunnelStatus::Connecting => "CONN    ",
                TunnelStatus::Stopped => "STOPPED ",
            };
            let profile = t.profile_name.as_deref().unwrap_or("?");
            format!(" [{status}] {}  ({})", t.config_summary, profile)
        })
        .collect();

    let hint = "[x] delete  [r] restart  [Esc] close";

    let content_width = rows
        .iter()
        .map(|r| r.len() as u16)
        .max()
        .unwrap_or(0)
        .max(hint.len() as u16)
        .max(MIN_WIDTH);

    let item_count = rows.len().max(1) as u16; // at least 1 for the "empty" message
    let pad_h: u16 = 1;
    let pad_v: u16 = 1;
    let border: u16 = 2;
    let hint_row: u16 = 1;
    let width = (content_width + pad_h * 2 + border).min(area.width);
    let height = (item_count + hint_row + pad_v + border + 1).min(area.height);

    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Tunnel Manager ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .padding(Padding::new(pad_h, pad_h, pad_v, 0));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [list_area, hint_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    if app.tunnels.is_empty() {
        let msg = Paragraph::new("No tunnels. Open one with [n].")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(msg, list_area);
    } else {
        let items: Vec<ListItem> = app
            .tunnels
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let (status_label, status_color) = match t.status {
                    TunnelStatus::Active => ("ACTIVE  ", Color::Green),
                    TunnelStatus::Connecting => ("CONN    ", Color::Yellow),
                    TunnelStatus::Stopped => ("STOPPED ", Color::DarkGray),
                };
                let profile = t.profile_name.as_deref().unwrap_or("?");
                let row = format!(
                    "[{status_label}] {}  ({})",
                    t.config_summary, profile
                );
                let base_style = if i == app.tunnel_manager_selected {
                    Style::default()
                        .fg(status_color)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(status_color)
                };
                ListItem::new(row).style(base_style)
            })
            .collect();

        let list = List::new(items);
        let mut state = ListState::default();
        state.select(Some(app.tunnel_manager_selected));
        frame.render_stateful_widget(list, list_area, &mut state);
    }

    let hint_widget = Paragraph::new(hint).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint_widget, hint_area);
}
