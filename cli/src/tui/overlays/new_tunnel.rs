use ratatui::{
    layout::{Constraint, Rect},
    prelude::*,
    widgets::Paragraph,
    Frame,
};

use crate::tui::app::App;
use crate::tui::ui::autosize_popup;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let form = &app.new_tunnel_form;
    let selected_profile = &app.settings.profiles[form.profile_idx];

    // Build all text lines
    let profile_label = if selected_profile.name.is_empty() {
        format!("Profile {}", form.profile_idx + 1)
    } else {
        selected_profile.name.clone()
    };
    let profile_text = format!(
        "Endpoint:  < {} >  ({}/{})",
        profile_label,
        form.profile_idx + 1,
        app.settings.profiles.len()
    );
    let endpoint_text = format!("  {}", selected_profile.endpoint);
    let type_text = format!("Type:      < {} >", form.tunnel_type.label());
    let port_text = format!("Port:      {}_", form.port);
    let name_display = if form.name.is_empty() {
        "Name:      (root)".to_string()
    } else {
        format!("Name:      {}_", form.name)
    };
    let host_text = format!("Listen to: {}_", form.host);
    let hint_text = "[</>] select  [v/^] navigate  [Enter] ok  [Esc] back";

    let lines: Vec<&str> = vec![
        &profile_text, &endpoint_text, &type_text,
        &port_text, &name_display, &host_text, hint_text,
    ];

    let constraints = vec![
        Constraint::Length(1), // profile
        Constraint::Length(1), // endpoint url
        Constraint::Length(1), // spacer
        Constraint::Length(1), // type
        Constraint::Length(1), // spacer
        Constraint::Length(1), // port
        Constraint::Length(1), // spacer
        Constraint::Length(1), // name
        Constraint::Length(1), // spacer
        Constraint::Length(1), // host
        Constraint::Length(1), // spacer
        Constraint::Length(1), // hint
    ];

    let (_block, rows) = autosize_popup(
        frame, area, "New Tunnel", Color::Cyan, &lines, &constraints, (2, 1),
    );

    // Profile selector
    let profile_style = field_style(form.focused_field == 0);
    frame.render_widget(Paragraph::new(profile_text).style(profile_style), rows[0]);

    // Endpoint URL (read-only)
    frame.render_widget(
        Paragraph::new(endpoint_text).style(Style::default().fg(Color::DarkGray)),
        rows[1],
    );

    // Type selector
    frame.render_widget(Paragraph::new(type_text).style(field_style(form.focused_field == 1)), rows[3]);

    // Port
    frame.render_widget(Paragraph::new(port_text).style(field_style(form.focused_field == 2)), rows[5]);

    // Name
    frame.render_widget(Paragraph::new(name_display).style(field_style(form.focused_field == 3)), rows[7]);

    // Host
    frame.render_widget(Paragraph::new(host_text).style(field_style(form.focused_field == 4)), rows[9]);

    // Hint
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
