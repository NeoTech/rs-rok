use ratatui::{
    layout::{Constraint, Rect},
    prelude::*,
    widgets::Paragraph,
    Frame,
};

use crate::tui::app::App;
use crate::tui::ui::autosize_popup;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let Some(form) = &app.settings_form else {
        return;
    };

    let profile_count = app.settings.profiles.len();

    // -- Build text lines for width measurement --
    let mut text_lines: Vec<String> = Vec::new();

    // Row 0: Profile selector
    let profile_name = if let Some(p) = app.settings.profiles.get(form.profile_idx) {
        if p.name.is_empty() { format!("Profile {}", form.profile_idx + 1) } else { p.name.clone() }
    } else {
        "---".to_string()
    };
    text_lines.push(format!(
        "< Profile:  {} ({}/{}) >",
        profile_name,
        form.profile_idx + 1,
        profile_count
    ));

    // Profile fields (rows 1..=4)
    for (label, value) in &form.profile_fields {
        text_lines.push(format!("  {label}:  {value}_"));
    }

    // CF selector row
    let cf_label = if form.cf_accounts.is_empty() {
        "(none)".to_string()
    } else if let Some(a) = form.cf_accounts.get(form.cf_account_idx) {
        if a.name.is_empty() {
            let vis = a.account_id.len().min(8);
            format!("{}...", &a.account_id[..vis])
        } else {
            a.name.clone()
        }
    } else {
        "---".to_string()
    };
    let cf_n = if form.cf_accounts.is_empty() { 0 } else { form.cf_account_idx + 1 };
    text_lines.push(format!(
        "< CF Account:  {} ({}/{}) >",
        cf_label,
        cf_n,
        form.cf_accounts.len()
    ));

    // CF fields
    for (label, value) in &form.cf_fields {
        text_lines.push(format!("  {label}:  {value}_"));
    }

    // Hint
    let hint = if form.editing {
        "[Enter] save  [Esc] cancel editing"
    } else if form.on_profile_selector() || form.on_cf_selector() {
        "[</>] switch  [v/^] navigate  [Esc] close"
    } else {
        "[Enter] edit  [v/^] navigate  [Esc] close"
    };
    text_lines.push(hint.to_string());

    let line_refs: Vec<&str> = text_lines.iter().map(|s| s.as_str()).collect();

    // -- Constraints --
    let mut constraints: Vec<Constraint> = Vec::new();
    constraints.push(Constraint::Length(1)); // profile selector
    for _ in &form.profile_fields {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1)); // spacer
    constraints.push(Constraint::Length(1)); // CF selector
    for _ in &form.cf_fields {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1)); // spacer
    constraints.push(Constraint::Length(1)); // hint

    let (_block, rows) = autosize_popup(
        frame, area, "Settings", Color::Yellow, &line_refs, &constraints, (2, 1),
    );

    // -- Render rows --
    let mut row = 0;

    // Profile selector (field 0)
    let sel_style = if form.on_profile_selector() {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };
    frame.render_widget(Paragraph::new(text_lines[0].as_str()).style(sel_style), rows[row]);
    row += 1;

    // Profile fields (fields 1..=PROFILE_FIELD_COUNT)
    for (i, (label, value)) in form.profile_fields.iter().enumerate() {
        let field_idx = 1 + i; // global focused_field index
        let is_focused = form.focused_field == field_idx;
        let is_editing = form.editing && is_focused;

        let style = if is_editing {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if is_focused {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        let cursor = if is_editing { "_" } else { "" };
        let display_value = if label == "Auth Token" && !value.is_empty() && !is_editing {
            let vis = value.len().min(4);
            format!("{}{}", &value[..vis], "*".repeat(value.len().saturating_sub(vis)))
        } else {
            value.clone()
        };

        let text = format!("  {label}:  {display_value}{cursor}");
        frame.render_widget(Paragraph::new(text).style(style), rows[row]);
        row += 1;
    }

    row += 1; // spacer

    // CF selector
    let cf_sel_style = if form.on_cf_selector() {
        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Magenta)
    };
    let cf_sel_line_idx = 1 + form.profile_fields.len(); // index into text_lines
    frame.render_widget(Paragraph::new(text_lines[cf_sel_line_idx].as_str()).style(cf_sel_style), rows[row]);
    row += 1;

    // CF fields
    for (i, (label, value)) in form.cf_fields.iter().enumerate() {
        let field_idx = form.cf_fields_start() + i;
        let is_focused = form.focused_field == field_idx;
        let is_editing = form.editing && is_focused;

        let style = if is_editing {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if is_focused {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        let cursor = if is_editing { "_" } else { "" };
        let display_value = if label == "API Token" && !value.is_empty() && !is_editing {
            let vis = value.len().min(4);
            format!("{}{}", &value[..vis], "*".repeat(value.len().saturating_sub(vis)))
        } else {
            value.clone()
        };

        let text = format!("  {label}:  {display_value}{cursor}");
        frame.render_widget(Paragraph::new(text).style(style), rows[row]);
        row += 1;
    }

    row += 1; // spacer

    // Hint
    if row < rows.len() {
        frame.render_widget(
            Paragraph::new(hint).style(Style::default().fg(Color::DarkGray)),
            rows[row],
        );
    }
}
