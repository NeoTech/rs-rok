use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};

use crate::tui::app::App;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    // Measure width from profile names
    let mut lines: Vec<String> = app
        .settings
        .profiles
        .iter()
        .map(|p| format!("  * {}", p.name))
        .collect();
    lines.push("[Enter] switch  [n] new  [d] delete  [Esc] close".to_string());
    if let Some(input) = &app.new_profile_input {
        lines.push(format!("Name: {input}_"));
    }

    let content_width = lines.iter().map(|l| l.len() as u16).max().unwrap_or(20);
    let list_rows = app.settings.profiles.len() as u16;
    let extra = if app.new_profile_input.is_some() { 2 } else { 1 }; // input or hint
    let content_rows = list_rows + extra;

    let pad_h: u16 = 2;
    let pad_v: u16 = 1;
    let border: u16 = 2;
    let width = (content_width + pad_h * 2 + border).min(area.width);
    let height = (content_rows + pad_v + border).min(area.height);

    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Profiles ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .padding(Padding::new(pad_h, pad_h, pad_v, 0));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if let Some(input) = &app.new_profile_input {
        let [input_area, list_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
        ])
        .areas(inner);

        let input_widget = Paragraph::new(format!("Name: {input}_"))
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(input_widget, input_area);
        draw_profile_list(frame, list_area, app);
    } else {
        let [list_area, hint_area] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(inner);

        draw_profile_list(frame, list_area, app);

        let hint = Paragraph::new("[Enter] switch  [n] new  [d] delete  [Esc] close")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, hint_area);
    }
}

fn draw_profile_list(frame: &mut Frame, area: Rect, app: &App) {
    let active_idx = app.settings.active_idx;
    let items: Vec<ListItem> = app
        .settings
        .profiles
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let active = if i == active_idx { "* " } else { "  " };
            let text = format!("{active}{}", p.name);
            let style = if i == active_idx {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(app.profiles_selected));

    frame.render_stateful_widget(list, area, &mut state);
}
