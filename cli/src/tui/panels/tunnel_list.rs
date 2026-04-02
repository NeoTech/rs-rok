use ratatui::{
    layout::Rect,
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::tui::app::{App, Focus, TunnelStatus};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let is_focused = app.focus == Focus::List;

    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(" Tunnels ")
        .borders(Borders::ALL)
        .border_style(border_style);

    if app.tunnels.is_empty() {
        let hint = ratatui::widgets::Paragraph::new("  Press [n] to create a tunnel")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(hint, area);
        return;
    }

    let items: Vec<ListItem> = app
        .tunnels
        .iter()
        .map(|t| {
            let (indicator, color) = match t.status {
                TunnelStatus::Active => ("*", Color::Green),
                TunnelStatus::Connecting => ("~", Color::Yellow),
                TunnelStatus::Stopped => (" ", Color::DarkGray),
            };

            let text = format!("{} {}  {}", indicator, t.name, t.config_summary);
            ListItem::new(text).style(Style::default().fg(color))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default();
    if !app.tunnels.is_empty() {
        state.select(Some(app.selected_tunnel));
    }

    frame.render_stateful_widget(list, area, &mut state);
}
