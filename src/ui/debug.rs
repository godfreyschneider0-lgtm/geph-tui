use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

use crate::state::AppState;

pub fn draw(f: &mut ratatui::Frame, state: &mut AppState<'_>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)].as_ref())
        .split(area);

    let toggle_text = format!(
        "Debug Logging to 'gephgui.log': {} (Press 'd' to toggle)\nNote: Changes take effect on next Start ('s').",
        if state.enable_debug_log { "ON" } else { "OFF" }
    );
    let toggle_p = Paragraph::new(toggle_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(toggle_p, chunks[0]);

    let logs = state.debug_logs.lock().unwrap();
    let logs_text: Vec<Line> = logs.iter().map(|l| Line::from(l.as_str())).collect();

    let max_scroll = logs
        .len()
        .saturating_sub(chunks[1].height.saturating_sub(2) as usize) as u16;
    if state.debug_auto_scroll {
        state.debug_scroll = max_scroll;
    } else if state.debug_scroll > max_scroll {
        state.debug_scroll = max_scroll;
    }

    let p = Paragraph::new(logs_text)
        .block(
            Block::default()
                .title("GUI Debug Logs (Up/Down to scroll)")
                .borders(Borders::ALL),
        )
        .scroll((state.debug_scroll, 0));
    f.render_widget(p, chunks[1]);
}
