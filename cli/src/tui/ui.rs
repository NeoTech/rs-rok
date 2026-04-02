use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::*,
    widgets::{Block, Borders, Clear, Padding, Paragraph},
    Frame,
};

use super::app::{App, Overlay};
use super::overlays;
use super::panels;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Minimum size check
    if area.width < 60 || area.height < 10 {
        let msg = Paragraph::new("Terminal too small (min 60x10)")
            .style(Style::default().fg(Color::Red));
        frame.render_widget(msg, area);
        return;
    }

    // Vertical split: main area + status bar
    let [main_area, status_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);

    // Horizontal split: tunnel list (30%) + log view (70%)
    let [list_area, log_area] =
        Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
            .areas(main_area);

    // Draw panels
    panels::tunnel_list::draw(frame, list_area, app);
    panels::log_view::draw(frame, log_area, app);

    // Status bar
    draw_status_bar(frame, status_area, app);

    // Overlay on top
    if let Some(overlay) = &app.overlay {
        match overlay {
            Overlay::NewTunnel => overlays::new_tunnel::draw(frame, area, app),
            Overlay::Settings => overlays::settings::draw(frame, area, app),
            Overlay::Profiles => overlays::profiles::draw(frame, area, app),
            Overlay::Deploy => overlays::deploy::draw(frame, area, app),
            Overlay::EndpointTest => overlays::endpoint_test::draw(frame, area, app),
        }
    }
}

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let profile_name = &app.settings.active_profile().name;

    let keys = Span::styled(
        " [n] new  [s] settings  [p] profiles  [D] deploy  [t] test  [d] stop  [r] restart  [x] delete  [Tab] focus  [q] quit ",
        Style::default().fg(Color::DarkGray),
    );
    let profile = Span::styled(
        format!("  Profile: {profile_name} "),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    );

    let bar = Line::from(vec![keys, profile]);
    frame.render_widget(Paragraph::new(bar), area);
}

/// Auto-sized, centered popup that fits its content.
///
/// `lines` is the list of text lines (used to measure width).
/// `constraints` is the vertical layout (one per visual row, including spacers).
/// `pad` is `(horizontal, vertical_top)` character padding inside the border.
/// `title` is shown on the top border.
/// `border_color` is the border/title color.
///
/// Returns `(popup_rect, inner_rows)` where `inner_rows` are the split layout rects.
pub fn autosize_popup<'a>(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    border_color: Color,
    lines: &[&str],
    constraints: &'a [Constraint],
    pad: (u16, u16),
) -> (Block<'a>, Vec<Rect>) {
    let content_width = lines.iter().map(|l| l.len() as u16).max().unwrap_or(20);
    let content_rows: u16 = constraints.iter().map(|c| match c {
        Constraint::Length(n) => *n,
        _ => 1,
    }).sum();

    let border: u16 = 2;
    let width = (content_width + pad.0 * 2 + border).min(area.width);
    let height = (content_rows + pad.1 + border).min(area.height);

    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .padding(Padding::new(pad.0, pad.0, pad.1, 0));

    let inner = block.inner(popup);
    frame.render_widget(block.clone(), popup);

    let rows = Layout::vertical(constraints.to_vec()).split(inner).to_vec();
    (block, rows)
}
