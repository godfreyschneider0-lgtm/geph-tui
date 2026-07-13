use geph5_misc_rpc::client_control::ConnInfo;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::state::AppState;

pub fn draw(f: &mut ratatui::Frame, state: &mut AppState<'_>, area: Rect) {
    let status_text = match &state.conn_info {
        ConnInfo::Disconnected => "Disconnected",
        ConnInfo::Connecting => "Connecting...",
        ConnInfo::Connected { .. } => "Connected",
    };
    let status_style = match &state.conn_info {
        ConnInfo::Disconnected => Style::default().fg(Color::Red),
        ConnInfo::Connecting => Style::default().fg(Color::Yellow),
        ConnInfo::Connected { .. } => Style::default().fg(Color::Green),
    };

    let mut lines = vec![Line::from(vec![
        Span::raw("Daemon: "),
        Span::styled(
            if state.is_running {
                "Running"
            } else {
                "Stopped"
            },
            if state.is_running {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            },
        ),
        Span::raw(" | Network: "),
        Span::styled(status_text, status_style),
        Span::raw(" | Exit: "),
        Span::styled(
            state.selected_country.as_deref().unwrap_or("Auto"),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(" | Listen: "),
        Span::styled(
            if state.listen_all { "0.0.0.0" } else { "127.0.0.1" },
            if state.listen_all {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default().fg(Color::Gray)
            },
        ),
    ])];

    if let Some((version, path)) = &state.update_info {
        lines.push(Line::from(""));

        let is_chinese = sys_locale::get_locale().unwrap_or_default().contains("zh");
        if is_chinese {
            lines.push(Line::from(Span::styled(
                format!("提示: 迷雾通新版本 ({}) 的更新包已下载至:", version),
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(Span::styled(
                path.display().to_string(),
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(Span::styled(
                "请您前往该目录手动处理此更新。",
                Style::default().fg(Color::Yellow),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!(
                    "Notice: Update package for Geph ({}) downloaded to:",
                    version
                ),
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(Span::styled(
                path.display().to_string(),
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(Span::styled(
                "Please go to this directory and handle the update manually.",
                Style::default().fg(Color::Yellow),
            )));
        }
    }

    if let Some(notice) = &state.level_notice {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            notice.clone(),
            Style::default().fg(Color::Yellow),
        )));
    } else if state.last_detected_level == geph5_broker_protocol::AccountLevel::Plus {
        let is_chinese = sys_locale::get_locale().unwrap_or_default().contains("zh");
        let days = state
            .plus_expires_days
            .map(|d| format!(" ({} days left)", d.ceil() as u64))
            .unwrap_or_default();
        let msg = if is_chinese {
            format!("欢迎，尊贵的 Plus 用户！{}", days)
        } else {
            format!("Welcome, Plus user!{}", days)
        };
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            msg,
            Style::default().fg(Color::Green),
        )));
    }

    for item in &state.news_items {
        lines.push(Line::from(""));
        if item.important {
            lines.push(Line::from(Span::styled(
                format!("[!] {}", item.title),
                Style::default().fg(Color::Red),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!("[*] {}", item.title),
                Style::default().fg(Color::Cyan),
            )));
        }
        for content_line in item.contents.lines() {
            lines.push(Line::from(Span::styled(
                content_line.to_string(),
                Style::default().fg(Color::Gray),
            )));
        }
    }

    let inner_height = area.height.saturating_sub(2) as usize;
    let content_width = area.width.saturating_sub(2) as usize;

    let wrapped_count: usize = lines
        .iter()
        .map(|line| (line.width() + content_width.max(1) - 1) / content_width.max(1))
        .sum();

    let max_scroll = wrapped_count.saturating_sub(inner_height) as u16;
    if state.status_scroll > max_scroll {
        state.status_scroll = max_scroll;
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((state.status_scroll, 0))
        .block(
            Block::default()
                .title("Status (j/k to scroll)")
                .borders(Borders::ALL),
        );
    f.render_widget(p, area);
}
