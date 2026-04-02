use ratatui::{
    layout::{Constraint, Rect},
    prelude::*,
    widgets::Paragraph,
    Frame,
};

use crate::tui::app::App;
use crate::tui::ui::autosize_popup;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let Some(form) = &app.deploy_form else {
        return;
    };

    let has_accounts = !form.cf_accounts.is_empty();
    let account = form.active_account();

    // Build text lines
    let account_label = if has_accounts {
        let a = account.unwrap();
        let name = if a.name.is_empty() {
            let visible = a.account_id.len().min(8);
            format!("{}...", &a.account_id[..visible])
        } else {
            a.name.clone()
        };
        format!(
            "< CF Account:  {} ({}/{}) >",
            name,
            form.selected_account + 1,
            form.cf_accounts.len()
        )
    } else {
        "CF Account:  (none configured)".to_string()
    };

    let worker_text = if form.focused_field == 0 {
        format!("Worker Name:  {}_", form.worker_name)
    } else {
        format!("Worker Name:  {}", form.worker_name)
    };
    let auth_text = if form.focused_field == 1 {
        let masked: String = "*".repeat(form.auth_token.len());
        format!("Auth Token:   {}_", masked)
    } else {
        let masked: String = "*".repeat(form.auth_token.len());
        if masked.is_empty() {
            "Auth Token:   (none)".to_string()
        } else {
            format!("Auth Token:   {}", masked)
        }
    };

    let is_deploying = form.status_message.as_deref() == Some("Deploying...");

    let hint_str = if let Some(msg) = &form.status_message {
        format!(" {msg}")
    } else if !has_accounts {
        " CF credentials missing -- add to cloudflare.json".to_string()
    } else {
        " [Tab] switch  [Enter] deploy  [</>] account  [Esc] cancel".to_string()
    };

    let lines: Vec<&str> = vec![&account_label, &worker_text, &auth_text, &hint_str];

    let constraints = vec![
        Constraint::Length(1), // CF account selector
        Constraint::Length(1), // spacer
        Constraint::Length(1), // worker name
        Constraint::Length(1), // auth token
        Constraint::Length(1), // spacer
        Constraint::Length(1), // hint
    ];

    let (_block, rows) = autosize_popup(
        frame, area, "Deploy Worker", Color::Yellow, &lines, &constraints, (2, 1),
    );

    // CF Account selector
    let acct_style = if has_accounts {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Red)
    };
    frame.render_widget(Paragraph::new(account_label).style(acct_style), rows[0]);

    // Worker name (editable when focused_field == 0)
    let worker_style = if form.focused_field == 0 {
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    frame.render_widget(Paragraph::new(worker_text).style(worker_style), rows[2]);

    // Auth token (editable when focused_field == 1)
    let auth_style = if form.focused_field == 1 {
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    frame.render_widget(Paragraph::new(auth_text).style(auth_style), rows[3]);

    // Hint / status
    let hint_style = if is_deploying {
        Style::default().fg(Color::Yellow)
    } else if form.status_message.is_some() || !has_accounts {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    frame.render_widget(Paragraph::new(hint_str).style(hint_style), rows[5]);
}
