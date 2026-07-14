use crossterm::event::{KeyCode, KeyEvent};
use geph5_misc_rpc::client_control::ControlClient;
use ratatui::style::{Color, Style};

use crate::daemon::{self, DaemonRpcTransport};
use crate::state::{AppState, Focus, TabIdx};

pub fn handle_focused_input<'a>(state: &mut AppState<'a>, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            state.focus = Focus::None;
            state.secret_textarea.set_style(Style::default());
            state.socks_textarea.set_style(Style::default());
            state.http_textarea.set_style(Style::default());
        }
        _ => match state.focus {
            Focus::Secret => {
                state.secret_textarea.input(key);
            }
            Focus::SocksPort => {
                state.socks_textarea.input(key);
            }
            Focus::HttpPort => {
                state.http_textarea.input(key);
            }
            _ => {}
        },
    }
}

pub async fn handle_global_key<'a>(state: &mut AppState<'a>, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') => {
            return true;
        }
        KeyCode::Char('1') => state.tab = TabIdx::Status,
        KeyCode::Char('2') => state.tab = TabIdx::Nodes,
        KeyCode::Char('3') => state.tab = TabIdx::Config,
        KeyCode::Char('4') => state.tab = TabIdx::Debug,

        KeyCode::Char('s') => {
            if !state.is_running {
                let current_secret = state.secret_textarea.lines().join("");
                let secret_changed =
                    state.last_connected_secret.as_deref() != Some(current_secret.as_str());
                if secret_changed || state.needs_cache_clear {
                    daemon::clear_conn_token_cache();
                    state.needs_cache_clear = false;
                    state.level_notice = None;
                }
                state.last_connected_secret = Some(current_secret);
                let prefs = state.to_prefs();
                prefs.save();
                let _ = daemon::start_daemon(&prefs).await;
            }
        }
        KeyCode::Char('x') => {
            if state.is_running {
                let _ = daemon::stop_daemon().await;
            }
        }
        KeyCode::Char('e') if state.tab == TabIdx::Config => {
            state.focus = Focus::Secret;
            state
                .secret_textarea
                .set_style(Style::default().fg(Color::Yellow));
        }
        KeyCode::Char('p') if state.tab == TabIdx::Config => {
            state.focus = Focus::SocksPort;
            state
                .socks_textarea
                .set_style(Style::default().fg(Color::Yellow));
        }
        KeyCode::Char('h') if state.tab == TabIdx::Config => {
            state.focus = Focus::HttpPort;
            state
                .http_textarea
                .set_style(Style::default().fg(Color::Yellow));
        }
        KeyCode::Char('r') if state.tab == TabIdx::Config => {
            if !state.is_running {
                let prefs = state.to_prefs();
                match daemon::start_daemon(&prefs).await {
                    Ok(()) => {
                        state.is_running = daemon::daemon_running().await;
                    }
                    Err(err) => {
                        state.registration_status =
                            format!("Daemon failed to start: {err:#}");
                        return false;
                    }
                }
            }
            match ControlClient(DaemonRpcTransport).start_registration().await {
                Ok(Ok(idx)) => {
                    state.registration_idx = Some(idx);
                    state.registration_status = "Registration started...".into();
                }
                Ok(Err(msg)) => {
                    state.registration_status = format!("Failed to start: {}", msg);
                }
                Err(err) => {
                    state.registration_status = format!("RPC error: {err:#}");
                }
            }
        }

        KeyCode::Char('l') if state.tab == TabIdx::Config => {
            state.listen_all = !state.listen_all;
        }
        KeyCode::Char('b') if state.tab == TabIdx::Config => {
            state.allow_direct = !state.allow_direct;
        }
        KeyCode::Down if state.tab == TabIdx::Nodes => {
            let i = match state.node_list_state.selected() {
                Some(i) => {
                    if i >= state.countries.len().saturating_sub(1) {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            state.node_list_state.select(Some(i));
        }
        KeyCode::Up if state.tab == TabIdx::Nodes => {
            let i = match state.node_list_state.selected() {
                Some(i) => {
                    if i == 0 {
                        state.countries.len().saturating_sub(1)
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            state.node_list_state.select(Some(i));
        }
        KeyCode::Enter if state.tab == TabIdx::Nodes => {
            if let Some(i) = state.node_list_state.selected() {
                if i < state.countries.len() {
                    state.selected_country = Some(state.countries[i].alpha2().to_string());
                }
            }
        }
        KeyCode::Char('a') if state.tab == TabIdx::Nodes => {
            state.selected_country = None;
        }
        KeyCode::Char('j') if state.tab == TabIdx::Status => {
            state.status_scroll = state.status_scroll.saturating_add(1);
        }
        KeyCode::Char('k') if state.tab == TabIdx::Status => {
            state.status_scroll = state.status_scroll.saturating_sub(1);
        }
        KeyCode::Up if state.tab == TabIdx::Debug => {
            state.debug_scroll = state.debug_scroll.saturating_sub(1);
            state.debug_auto_scroll = false;
        }
        KeyCode::Down if state.tab == TabIdx::Debug => {
            let max_scroll = state.debug_logs.lock().unwrap().len().saturating_sub(1) as u16;
            state.debug_scroll = std::cmp::min(state.debug_scroll + 1, max_scroll);
            if state.debug_scroll >= max_scroll {
                state.debug_auto_scroll = true;
            }
        }
        KeyCode::Char('d') if state.tab == TabIdx::Debug => {
            state.enable_debug_log = !state.enable_debug_log;
        }
        _ => {}
    }
    false
}
