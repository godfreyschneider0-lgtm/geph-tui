use crossterm::{
    event::Event,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use isocountry::CountryCode;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    widgets::{Block, Borders},
    Terminal,
};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod autoupdate;
mod daemon;
mod event;
mod state;
mod ui;

use daemon::{daemon_running, start_daemon, stop_daemon, DaemonRpcTransport};
use geph5_misc_rpc::client_control::{ConnInfo, ControlClient};
use state::{AppState, LogWriter, TuiPrefs};

fn main() -> anyhow::Result<()> {
    unsafe {
        std::env::remove_var("http_proxy");
        std::env::remove_var("https_proxy");
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("HTTPS_PROXY");
    }

    let args = std::env::args().collect::<Vec<_>>();
    match args.get(1).map(|s| s.as_str()) {
        Some("-h") | Some("--help") => {
            println!(
                "geph-tui {} — lightweight Geph5 client (TUI + headless daemon)\n",
                env!("CARGO_PKG_VERSION")
            );
            println!("USAGE:");
            println!("    geph-tui [MODE] [ARGS]\n");
            println!("MODES:");
            println!("    (none)            Interactive TUI (default). Configure account, region,");
            println!("                      ports; press 's' to connect, 'q' to quit.");
            println!("    --ctl <cmd>       Control the daemon without UI.");
            println!("                      Commands: start, stop, status");
            println!("    -h, --help        Show this help.\n");
            println!("TUI KEYS:");
            println!("    1-4   tabs (Status / Regions / Config / Debug)");
            println!("    s/x   start / stop connection");
            println!("    e     edit Account ID      p  edit SOCKS5 port      h  edit HTTP port");
            println!(
                "    l     toggle listen-all      b  toggle direct/bridged"
            );
            println!("    r     register a new account");
            println!("    q     quit");
            return Ok(());
        }
        _ => {}
    }
    if let Some("--ctl") = args.get(1).map(|s| s.as_str()) {
        let _ = tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .try_init();

        let subcmd = args.get(2).map(|s| s.as_str()).unwrap_or("status");
        smol::block_on(async {
            match subcmd {
                "start" => ctl_start().await,
                "stop" => ctl_stop().await,
                "status" => ctl_status().await,
                _ => {
                    eprintln!("Usage: geph-tui --ctl start|stop|status");
                    std::process::exit(1);
                }
            }
        })?;
        return Ok(());
    }

    // DO NOT run the autoupdate logic on flatpak, but otherwise it's good
    if std::env::var("FLATPAK_ID").is_err() {
        smolscale::spawn(autoupdate::download_update_loop()).detach();
    }

    let debug_logs = Arc::new(Mutex::new(Vec::new()));
    let writer = LogWriter {
        logs: debug_logs.clone(),
    };

    // Ensure terminal output is completely clean
    let _ = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false) // Disable ANSI color codes which might mess up TUI if any leak
        .try_init();

    smolscale::block_on(async {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Clear screen explicitly before running
        terminal.clear()?;

        let res = run_app(&mut terminal, debug_logs).await;

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        if let Err(err) = res {
            println!("{:?}", err)
        }

        anyhow::Ok(())
    })?;

    Ok(())
}

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    debug_logs: Arc<Mutex<Vec<String>>>,
) -> anyhow::Result<()> {
    let mut state = AppState::new(debug_logs);
    state
        .secret_textarea
        .set_block(Block::default().borders(Borders::ALL).title("Login Secret"));
    state
        .socks_textarea
        .set_block(Block::default().borders(Borders::ALL).title("SOCKS5 Port"));
    state.http_textarea.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title("HTTP Proxy Port"),
    );

    let prefs = state.to_prefs();
    if !prefs.secret.is_empty() && !daemon_running().await {
        let _ = start_daemon(&prefs).await;
    }

    loop {
        state.update_info = autoupdate::get_cached_update();

        // Fetch status
        state.is_running = daemon_running().await;
        if state.is_running {
            if let Ok(info) = ControlClient(DaemonRpcTransport).conn_info().await {
                state.conn_info = info;
            }
        } else {
            state.conn_info = ConnInfo::Disconnected;
        }

        let mut user_level = state.last_detected_level;
        if state.is_running {
            let secret = state.secret_textarea.lines().join("");
            if !secret.is_empty() {
                let cred = geph5_broker_protocol::Credential::Secret(secret);
                let cred_val = serde_json::to_value(&cred).unwrap_or(serde_json::Value::Null);
                if let Ok(Ok(ui_val)) = ControlClient(DaemonRpcTransport)
                    .broker_rpc("get_user_info_by_cred".into(), vec![cred_val])
                    .await
                {
                    if let Ok(Some(ui)) =
                        serde_json::from_value::<Option<geph5_broker_protocol::UserInfo>>(ui_val)
                    {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        state.plus_expires_days = ui.plus_expires_unix.map(|exp| {
                            let secs = exp.saturating_sub(now);
                            secs as f64 / 86400.0
                        });
                        if ui.plus_expires_unix.unwrap_or(0) > now {
                            user_level = geph5_broker_protocol::AccountLevel::Plus;
                        } else {
                            user_level = geph5_broker_protocol::AccountLevel::Free;
                        }
                    }
                }
            }

            if user_level != state.last_detected_level {
                state.needs_cache_clear = true;
                state.level_notice = Some(match user_level {
                    geph5_broker_protocol::AccountLevel::Plus => {
                        "VIP detected! Press x then s to reconnect with VIP access.".into()
                    }
                    geph5_broker_protocol::AccountLevel::Free => {
                        "VIP expired. Press x then s to reconnect.".into()
                    }
                });
                state.last_detected_level = user_level;
            }
        }

        if let Ok(Ok(ns)) = ControlClient(DaemonRpcTransport).net_status().await {
            let mut seen = std::collections::HashSet::new();
            state.countries = ns
                .exits
                .into_iter()
                .filter_map(|(_, (_, desc, meta))| {
                    if !meta.allowed_levels.contains(&user_level) {
                        return None;
                    }
                    let cc = desc.country;
                    if seen.insert(cc.alpha2().to_string()) {
                        Some(cc)
                    } else {
                        None
                    }
                })
                .collect();
            state.countries.sort_by_key(|c| c.alpha2().to_string());
        }

        if state.news_items.is_empty() && state.is_running {
            let lang = if sys_locale::get_locale()
                .unwrap_or_default()
                .contains("zh")
            {
                "zh"
            } else {
                "en"
            };
            if let Ok(Ok(news)) =
                ControlClient(DaemonRpcTransport).latest_news(lang.to_string()).await
            {
                state.news_items = news;
            }
        }

        // Poll registration
        if let Some(idx) = state.registration_idx {
            match ControlClient(DaemonRpcTransport)
                .poll_registration(idx)
                .await
            {
                Ok(Ok(prog)) => {
                    if let Some(sec) = prog.secret {
                        state.secret_textarea = tui_textarea::TextArea::default();
                        state.secret_textarea.insert_str(&sec);
                        state.secret_textarea.set_block(
                            Block::default().borders(Borders::ALL).title("Login Secret"),
                        );
                        state.registration_status = "Registration complete!".into();
                        state.registration_idx = None;
                        state.needs_cache_clear = true;
                        state.last_detected_level = geph5_broker_protocol::AccountLevel::Free;
                        state.plus_expires_days = None;
                        state.level_notice = if state.is_running {
                            Some("New account registered. Press 'x' then 's' to reconnect.".into())
                        } else {
                            None
                        };
                        state.sync_prefs();
                    } else {
                        state.registration_status =
                            format!("Registering... {:.1}%", prog.progress * 100.0);
                    }
                }
                Ok(Err(msg)) => {
                    state.registration_status = format!("Registration failed: {}", msg);
                    state.registration_idx = None;
                }
                Err(_) => {}
            }
        }

        terminal.draw(|f| ui::draw_ui(f, &mut state))?;

        if crossterm::event::poll(Duration::from_millis(250))? {
            let ev = crossterm::event::read()?;

            if state.focus != state::Focus::None {
                if let Event::Key(key) = ev {
                    event::handle_focused_input(&mut state, key);
                }
                continue;
            }

            if let Event::Key(key) = ev {
                if event::handle_global_key(&mut state, key).await {
                    return Ok(());
                }
            }
            state.sync_prefs();
        }
    }
}

async fn ctl_start() -> anyhow::Result<()> {
    if daemon_running().await {
        println!("already running");
        return Ok(());
    }
    let prefs = TuiPrefs::load();
    if prefs.secret.is_empty() {
        eprintln!("no account configured");
        std::process::exit(1);
    }
    if prefs.last_connected_secret.as_deref() != Some(prefs.secret.as_str()) {
        daemon::clear_conn_token_cache();
        let mut saved = prefs.clone();
        saved.last_connected_secret = Some(prefs.secret.clone());
        saved.save();
    }
    start_daemon(&prefs).await?;
    println!("started");
    Ok(())
}

async fn ctl_stop() -> anyhow::Result<()> {
    if !daemon_running().await {
        println!("not running");
        return Ok(());
    }
    stop_daemon().await?;
    println!("stopped");
    Ok(())
}

async fn ctl_status() -> anyhow::Result<()> {
    let prefs = TuiPrefs::load();
    let socks5 = prefs.socks_port;
    let http = prefs.http_port;
    let listen_addr = if prefs.listen_all {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };

    if !daemon_running().await {
        println!("running=no");
        println!("socks5_port={socks5}");
        println!("http_port={http}");
        println!("listen_addr={listen_addr}");
        return Ok(());
    }

    let conn = ControlClient(DaemonRpcTransport)
        .conn_info()
        .await
        .unwrap_or(ConnInfo::Disconnected);

    let state = match &conn {
        ConnInfo::Connected { .. } => "connected",
        ConnInfo::Connecting => "connecting",
        ConnInfo::Disconnected => "disconnected",
    };

    println!("running=yes");
    println!("conn={state}");
    println!("socks5_port={socks5}");
    println!("http_port={http}");
    println!("listen_addr={listen_addr}");

    if let ConnInfo::Connected { sessions } = &conn {
        if let Some(s) = sessions.first() {
            let cc = &s.exit.country;
            println!("exit={} ({})", cc.name(), cc.alpha2());
        }
    }

    Ok(())
}
