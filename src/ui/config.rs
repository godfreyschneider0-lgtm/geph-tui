use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph},
};

use crate::state::AppState;

pub fn draw(f: &mut ratatui::Frame, state: &mut AppState<'_>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ]
            .as_ref(),
        )
        .split(area);

    f.render_widget(&state.secret_textarea, chunks[0]);
    f.render_widget(&state.socks_textarea, chunks[1]);
    f.render_widget(&state.http_textarea, chunks[2]);

    let vpn_text = format!(
        "VPN Mode: {} (Press 'v' to toggle)",
        if state.global_vpn { "ON" } else { "OFF" }
    );
    let vpn_p = Paragraph::new(vpn_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(vpn_p, chunks[3]);

    let listen_text = format!(
        "Listen All Interfaces: {} (Press 'l' to toggle)",
        if state.listen_all { "ON" } else { "OFF" }
    );
    let listen_p = Paragraph::new(listen_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(listen_p, chunks[4]);

    let direct_text = format!(
        "Connection Mode: {} (Press 'b' to toggle)",
        if state.allow_direct {
            "Direct"
        } else {
            "Bridged"
        }
    );
    let direct_p = Paragraph::new(direct_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(direct_p, chunks[5]);

    let hints = Paragraph::new(format!(
        "Press 'e' for Secret, 'p' for SOCKS5, 'h' for HTTP port editing.\nEnter/Esc to finish. Press 'r' to register new secret.\n{}",
        state.registration_status
    ));
    f.render_widget(hints, chunks[6]);
}
