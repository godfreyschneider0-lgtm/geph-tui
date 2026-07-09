use crossterm::event::{KeyCode, KeyEvent};
use nanorpc::{JrpcId, JrpcRequest};
use ratatui::style::{Color, Style};

use crate::daemon::{self, DaemonArgs, ExitConstraint};
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
            let _ = daemon::stop_daemon().await;
            return true;
        }
        KeyCode::Char('1') => state.tab = TabIdx::Status,
        KeyCode::Char('2') => state.tab = TabIdx::Nodes,
        KeyCode::Char('3') => state.tab = TabIdx::Config,
        KeyCode::Char('4') => state.tab = TabIdx::Debug,

        KeyCode::Char('s') => {
            if !state.is_running {
                let current_secret = state.secret_textarea.lines().join("");
                let exit = match &state.selected_country {
                    Some(country) => ExitConstraint::Country(country.clone()),
                    None => ExitConstraint::Auto,
                };

                let args = DaemonArgs {
                    secret: current_secret.clone(),
                    metadata: serde_json::Value::Null,
                    prc_whitelist: false,
                    exit,
                    global_vpn: state.global_vpn,
                    listen_all: state.listen_all,
                    proxy_autoconf: false,
                    allow_direct: state.allow_direct,
                    socks5_port: state
                        .socks_textarea
                        .lines()
                        .join("")
                        .parse()
                        .unwrap_or(9909),
                    http_proxy_port: state.http_textarea.lines().join("").parse().unwrap_or(9910),
                    enable_debug_log: state.enable_debug_log,
                };
                let secret_changed =
                    state.last_connected_secret.as_deref() != Some(current_secret.as_str());
                if secret_changed || state.needs_cache_clear {
                    daemon::clear_conn_token_cache();
                    state.needs_cache_clear = false;
                    state.level_notice = None;
                }
                state.last_connected_secret = Some(current_secret);
                state.sync_prefs();
                let _ = daemon::start_daemon(args).await;
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
            let reg_jrpc = JrpcRequest {
                jsonrpc: "2.0".into(),
                method: "start_registration".into(),
                params: vec![],
                id: JrpcId::Number(5),
            };
            if let Ok(resp) = daemon::daemon_rpc(reg_jrpc).await {
                if let Some(res) = resp.result {
                    if let Ok(idx) = serde_json::from_value::<usize>(res) {
                        state.registration_idx = Some(idx);
                        state.registration_status = "Registration started...".into();
                    }
                } else if let Some(err) = resp.error {
                    state.registration_status = format!("Failed to start: {}", err.message);
                }
            }
        }
        KeyCode::Char('v') if state.tab == TabIdx::Config => {
            state.global_vpn = !state.global_vpn;
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
