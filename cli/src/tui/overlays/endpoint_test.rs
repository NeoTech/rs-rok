use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::*,
    widgets::Paragraph,
    Frame,
};

use crate::tui::app::App;
use crate::tui::ui::autosize_popup;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let Some(form) = &app.endpoint_test_form else {
        return;
    };

    let tunnel = app.tunnels.get(app.selected_tunnel);
    let public_url = tunnel
        .and_then(|t| t.public_url.as_deref())
        .unwrap_or("—");
    let test_result = tunnel.and_then(|t| t.test_result.as_ref());

    let not_running = !form.is_running;

    // Build display strings
    let url_text = format!("URL:     {public_url}");
    let method_text = format!("Method:  < {} >", form.method());
    let path_cur = if not_running && form.focused_field == 1 { "_" } else { "" };
    let body_cur = if not_running && form.focused_field == 2 { "_" } else { "" };
    let path_render = format!("Path:    {}{path_cur}", form.path);
    let body_render = format!("Body:    {}{body_cur}", form.body);

    // Status / result
    let status_text = if form.is_running {
        "Status:  Testing...".to_string()
    } else if let Some(res) = test_result {
        format!("Status:  {}  {}ms", res.status, res.latency_ms)
    } else {
        "Status:  Ready — press [Enter] to test".to_string()
    };

    let body_snippet_text = test_result
        .filter(|r| !r.body_snippet.is_empty())
        .map(|r| {
            let end = r.body_snippet.len().min(80);
            format!("Body:    {}", &r.body_snippet[..end])
        })
        .unwrap_or_default();

    let hint_text = "[v/^] navigate  [<//>] method  [Enter] send  [Esc] close";

    // Width-driver strings for autosize_popup (only affect popup width)
    let lines: Vec<&str> = vec![
        &url_text,
        &method_text,
        &path_render,
        &body_render,
        "-- Headers (Name = Value) --",
        "Header-Name-1234              Header-Value-1234",  // approximate header row width
        &status_text,
        &body_snippet_text,
        hint_text,
    ];

    let constraints = vec![
        Constraint::Length(1), // url
        Constraint::Length(1), // method
        Constraint::Length(1), // path
        Constraint::Length(1), // body
        Constraint::Length(1), // header section label
        Constraint::Length(1), // H1
        Constraint::Length(1), // H2
        Constraint::Length(1), // H3
        Constraint::Length(1), // separator rule
        Constraint::Length(1), // status
        Constraint::Length(1), // body snippet
        Constraint::Length(1), // hint
    ];

    let (_block, rows) = autosize_popup(
        frame, area, "Endpoint Test", Color::Green, &lines, &constraints, (2, 1),
    );

    // Row 0: URL (read-only)
    frame.render_widget(
        Paragraph::new(url_text).style(Style::default().fg(Color::DarkGray)),
        rows[0],
    );

    // Row 1: Method selector
    frame.render_widget(
        Paragraph::new(method_text).style(field_style(form.focused_field == 0)),
        rows[1],
    );

    // Row 2: Path
    frame.render_widget(
        Paragraph::new(path_render).style(field_style(form.focused_field == 1)),
        rows[2],
    );

    // Row 3: Body
    frame.render_widget(
        Paragraph::new(body_render).style(field_style(form.focused_field == 2)),
        rows[3],
    );

    // Row 4: Header section label
    frame.render_widget(
        Paragraph::new(" -- Headers (Name = Value) --").style(Style::default().fg(Color::DarkGray)),
        rows[4],
    );

    // Rows 5-7: header pairs (3 fixed slots)
    for (i, &row_rect) in rows[5..=7].iter().enumerate() {
        let (key, val) = form
            .headers
            .get(i)
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .unwrap_or(("", ""));

        let key_field = 3 + i * 2;
        let val_field = 4 + i * 2;
        let key_focused = form.focused_field == key_field;
        let val_focused = form.focused_field == val_field;

        let key_cur = if key_focused && not_running { "_" } else { "" };
        let val_cur = if val_focused && not_running { "_" } else { "" };

        // Horizontal split: key area | " = " separator | value area
        let [key_area, sep_area, val_area] = Layout::horizontal([
            Constraint::Percentage(45),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .areas(row_rect);

        frame.render_widget(
            Paragraph::new(format!("{key}{key_cur}")).style(field_style(key_focused)),
            key_area,
        );
        frame.render_widget(
            Paragraph::new(" = ").style(Style::default().fg(Color::DarkGray)),
            sep_area,
        );
        frame.render_widget(
            Paragraph::new(format!("{val}{val_cur}")).style(field_style(val_focused)),
            val_area,
        );
    }

    // Row 8: horizontal separator
    let rule = "─".repeat(rows[8].width as usize);
    frame.render_widget(
        Paragraph::new(rule).style(Style::default().fg(Color::DarkGray)),
        rows[8],
    );

    // Row 9: status / result line
    let status_style = if form.is_running {
        Style::default().fg(Color::Yellow)
    } else if let Some(res) = test_result {
        http_status_style(res.status)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    frame.render_widget(
        Paragraph::new(status_text).style(status_style),
        rows[9],
    );

    // Row 10: body snippet (only once result available)
    if !body_snippet_text.is_empty() {
        frame.render_widget(
            Paragraph::new(body_snippet_text).style(Style::default().fg(Color::Gray)),
            rows[10],
        );
    }

    // Row 11: hint
    frame.render_widget(
        Paragraph::new(hint_text).style(Style::default().fg(Color::DarkGray)),
        rows[11],
    );
}

fn field_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    }
}

fn http_status_style(status: u16) -> Style {
    match status {
        200..=299 => Style::default().fg(Color::Green),
        300..=399 => Style::default().fg(Color::Yellow),
        400..=499 => Style::default().fg(Color::LightRed),
        500..=599 => Style::default().fg(Color::Red),
        _ => Style::default().fg(Color::Gray),
    }
}
