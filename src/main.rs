use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use daemon::{daemon_running, start_daemon, stop_daemon, DaemonArgs, ExitConstraint, running_cfg};
use geph5_broker_protocol::NetStatus;
use geph5_misc_rpc::client_control::{ConnInfo, RegistrationProgress};
use isocountry::CountryCode;
use nanorpc::{JrpcId, JrpcRequest};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Terminal,
};
use std::{io, time::Duration};
use tui_textarea::TextArea;

mod daemon;
mod autoupdate;

use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

#[derive(Clone)]
struct LogWriter {
    logs: Arc<Mutex<Vec<String>>>,
}

impl io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s = String::from_utf8_lossy(buf);
        let mut logs = self.logs.lock().unwrap();
        logs.extend(s.lines().map(|l| l.to_string()));
        if logs.len() > 1000 {
            let overflow = logs.len() - 1000;
            logs.drain(0..overflow);
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogWriter {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

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
            println!("gephgui-tui {} — Geph5 client (TUI + headless daemon)\n", env!("CARGO_PKG_VERSION"));
            println!("USAGE:");
            println!("    gephgui-tui [MODE] [ARGS]\n");
            println!("MODES:");
            println!("    (none)            Interactive TUI (default). Configure account, region,");
            println!("                      ports; press 's' to connect, 'q' to quit.");
            println!("    --daemon          Headless: connect using saved config, no UI. Logs go to");
            println!("                      stderr. Set up your account in the TUI first.");
            println!("    --config <FILE>   Run the core client with a YAML config file.");
            println!("    -h, --help        Show this help.\n");
            println!("HEADLESS USAGE:");
            println!("    nohup gephgui-tui --daemon > geph.log 2>&1 &");
            println!("    # stop with: kill <pid>   (or launch the TUI and press 'x')\n");
            println!("TUI KEYS:");
            println!("    1-4   tabs (Status / Regions / Config / Debug)");
            println!("    s/x   start / stop connection");
            println!("    e     edit Account ID      p  edit SOCKS5 port      h  edit HTTP port");
            println!("    v     toggle VPN mode      l  toggle listen-all      b  toggle direct/bridged");
            println!("    r     register a new account");
            println!("    q     quit");
            return Ok(());
        }
        _ => {}
    }
    if let Some("--config") = args.get(1).map(|s| s.as_str()) {
        let val: serde_json::Value = serde_yaml::from_slice(&std::fs::read(&args[2])?)?;
        let cfg: geph5_client::Config = serde_json::from_value(val)?;
        let client = geph5_client::Client::start(cfg);
        smol::future::block_on(client.wait_until_dead())?;
        return Ok(());
    }

    if let Some("--daemon") = args.get(1).map(|s| s.as_str()) {
        let _ = tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .try_init();

        let prefs = TuiPrefs::load();

        if prefs.secret.is_empty() {
            eprintln!("error: no account configured. Run the TUI once to set your Account ID, then use --daemon.");
            return Err(anyhow::anyhow!("secret not configured"));
        }

        let socks5_port: u16 = prefs.socks_port.parse().unwrap_or(9909);
        let http_proxy_port: u16 = prefs.http_port.parse().unwrap_or(9910);

        let args = DaemonArgs {
            secret: prefs.secret.clone(),
            metadata: serde_json::Value::Null,
            prc_whitelist: false,
            exit: match &prefs.selected_country {
                Some(country) => ExitConstraint::Country(country.clone()),
                None => ExitConstraint::Auto,
            },
            global_vpn: prefs.global_vpn,
            listen_all: prefs.listen_all,
            proxy_autoconf: false,
            allow_direct: prefs.allow_direct,
            socks5_port,
            http_proxy_port,
            enable_debug_log: prefs.enable_debug_log,
        };

        let listen_ip = if prefs.listen_all { "0.0.0.0" } else { "127.0.0.1" };
        let connection = if prefs.allow_direct { "Direct" } else { "Bridged" };
        let vpn_mode = if prefs.global_vpn { "ON" } else { "OFF" };
        let listen_all_str = if prefs.listen_all { "ON" } else { "OFF" };
        let exit_display = match &prefs.selected_country {
            Some(cc) => match CountryCode::for_alpha2(cc) {
                Ok(country) => format!("{} ({})", country.name(), country.alpha2()),
                Err(_) => cc.clone(),
            },
            None => "Auto".to_string(),
        };

        println!("gephgui-tui daemon (headless mode)");
        println!("==================================");
        println!("Account ID:    {}", prefs.secret);
        println!("Connection:    {}", connection);
        println!("VPN Mode:      {}", vpn_mode);
        println!("Listen all:    {}", listen_all_str);
        println!("SOCKS5:        {}:{}", listen_ip, socks5_port);
        println!("HTTP Proxy:    {}:{}", listen_ip, http_proxy_port);
        println!("Exit Node:     {}", exit_display);
        println!("==================================");
        println!();
        println!("Starting daemon... Press Ctrl+C to stop.");

        let cfg = running_cfg(args);
        let client = geph5_client::Client::start(cfg);
        smol::future::block_on(client.wait_until_dead())?;
        return Ok(());
    }

    // DO NOT run the autoupdate logic on flatpak, but otherwise it's good
    if std::env::var("FLATPAK_ID").is_err() {
        smolscale::spawn(autoupdate::download_update_loop()).detach();
    }

    let debug_logs = Arc::new(Mutex::new(Vec::new()));
    let writer = LogWriter { logs: debug_logs.clone() };
    
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

#[derive(PartialEq)]
enum TabIdx {
    Status,
    Nodes,
    Config,
    Debug,
}

#[derive(PartialEq)]
enum Focus {
    None,
    Secret,
    SocksPort,
    HttpPort,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct TuiPrefs {
    secret: String,
    socks_port: String,
    http_port: String,
    global_vpn: bool,
    listen_all: bool,
    enable_debug_log: bool,
    allow_direct: bool,
    selected_country: Option<String>,
}

impl TuiPrefs {
    fn load() -> Self {
        let path = dirs::config_dir().unwrap_or_else(|| std::env::temp_dir()).join("geph5_tui_prefs.json");
        if let Ok(b) = std::fs::read(&path) {
            if let Ok(p) = serde_json::from_slice(&b) {
                return p;
            }
        }
        Self {
            socks_port: "9909".into(),
            http_port: "9910".into(),
            ..Default::default()
        }
    }

    fn save(&self) {
        let path = dirs::config_dir().unwrap_or_else(|| std::env::temp_dir()).join("geph5_tui_prefs.json");
        let _ = std::fs::write(&path, serde_json::to_string(self).unwrap_or_default());
    }
}

struct AppState<'a> {
    tab: TabIdx,
    is_running: bool,
    conn_info: ConnInfo,
    
    // Config
    secret_textarea: TextArea<'a>,
    socks_textarea: TextArea<'a>,
    http_textarea: TextArea<'a>,
    listen_all: bool,
    global_vpn: bool,
    allow_direct: bool,
    
    // Nodes
    countries: Vec<CountryCode>,
    node_list_state: ListState,
    selected_country: Option<String>,
    
    focus: Focus,
    registration_status: String,
    registration_idx: Option<usize>,
    
    debug_logs: Arc<Mutex<Vec<String>>>,
    debug_scroll: u16,
    debug_auto_scroll: bool,
    enable_debug_log: bool,

    update_info: Option<(String, std::path::PathBuf)>,
}

impl<'a> AppState<'a> {
    fn new(debug_logs: Arc<Mutex<Vec<String>>>) -> Self {
        let prefs = TuiPrefs::load();
        
        let mut secret = TextArea::default();
        secret.insert_str(&prefs.secret);
        
        let mut socks = TextArea::default();
        socks.insert_str(&prefs.socks_port);
        
        let mut http = TextArea::default();
        http.insert_str(&prefs.http_port);
        
        Self {
            tab: TabIdx::Status,
            is_running: false,
            conn_info: ConnInfo::Disconnected,
            
            secret_textarea: secret,
            socks_textarea: socks,
            http_textarea: http,
            listen_all: prefs.listen_all,
            global_vpn: prefs.global_vpn,
            allow_direct: prefs.allow_direct,
            
            countries: vec![],
            node_list_state: ListState::default(),
            selected_country: prefs.selected_country.clone(),
            
            focus: Focus::None,
            registration_status: String::new(),
            registration_idx: None,
            
            debug_logs,
            debug_scroll: 0,
            debug_auto_scroll: true,
            enable_debug_log: prefs.enable_debug_log,

            update_info: None,
        }
    }
    
    fn sync_prefs(&self) {
        let prefs = TuiPrefs {
            secret: self.secret_textarea.lines().join(""),
            socks_port: self.socks_textarea.lines().join(""),
            http_port: self.http_textarea.lines().join(""),
            global_vpn: self.global_vpn,
            listen_all: self.listen_all,
            enable_debug_log: self.enable_debug_log,
            allow_direct: self.allow_direct,
            selected_country: self.selected_country.clone(),
        };
        prefs.save();
    }
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, debug_logs: Arc<Mutex<Vec<String>>>) -> anyhow::Result<()> {
    let mut state = AppState::new(debug_logs);
    state.secret_textarea.set_block(Block::default().borders(Borders::ALL).title("Login Secret"));
    state.socks_textarea.set_block(Block::default().borders(Borders::ALL).title("SOCKS5 Port"));
    state.http_textarea.set_block(Block::default().borders(Borders::ALL).title("HTTP Proxy Port"));

    loop {
        state.update_info = autoupdate::get_cached_update();

        // Fetch status
        state.is_running = daemon_running().await;
        if state.is_running {
            let jrpc = JrpcRequest {
                jsonrpc: "2.0".into(),
                method: "conn_info".into(),
                params: vec![],
                id: JrpcId::Number(1),
            };
            if let Ok(resp) = daemon::daemon_rpc(jrpc).await {
                if let Some(res) = resp.result {
                    if let Ok(info) = serde_json::from_value(res) {
                        state.conn_info = info;
                    }
                }
            }
        } else {
            state.conn_info = ConnInfo::Disconnected;
        }

        // Fetch nodes
        let mut user_level = geph5_broker_protocol::AccountLevel::Free;
        if state.is_running {
            let cred = geph5_broker_protocol::Credential::Secret(state.secret_textarea.lines().join(""));
            let cred_val = serde_json::to_value(&cred).unwrap_or(serde_json::Value::Null);
            let ui_jrpc = JrpcRequest {
                jsonrpc: "2.0".into(),
                method: "broker_rpc".into(),
                params: vec![
                    serde_json::Value::String("get_user_info_by_cred".into()),
                    serde_json::Value::Array(vec![cred_val]),
                ],
                id: JrpcId::Number(99),
            };
            if let Ok(resp) = daemon::daemon_rpc(ui_jrpc).await {
                if let Some(res) = resp.result {
                    if let Ok(Some(ui)) = serde_json::from_value::<Option<geph5_broker_protocol::UserInfo>>(res) {
                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                        if ui.plus_expires_unix.unwrap_or(0) > now {
                            user_level = geph5_broker_protocol::AccountLevel::Plus;
                        }
                    }
                }
            }
        }

        let ns_jrpc = JrpcRequest {
            jsonrpc: "2.0".into(),
            method: "net_status".into(),
            params: vec![],
            id: JrpcId::Number(3),
        };
        if let Ok(resp) = daemon::daemon_rpc(ns_jrpc).await {
            if let Some(res) = resp.result {
                if let Ok(ns) = serde_json::from_value::<NetStatus>(res) {
                    let mut seen = std::collections::HashSet::new();
                    state.countries = ns.exits.into_iter().filter_map(|(_, (_, desc, meta))| {
                        if !meta.allowed_levels.contains(&user_level) {
                            return None;
                        }
                        let cc = desc.country;
                        if seen.insert(cc.alpha2().to_string()) {
                            Some(cc)
                        } else {
                            None
                        }
                    }).collect();
                    state.countries.sort_by_key(|c| c.alpha2().to_string());
                }
            }
        }

        // Poll registration
        if let Some(idx) = state.registration_idx {
            let poll_jrpc = JrpcRequest {
                jsonrpc: "2.0".into(),
                method: "poll_registration".into(),
                params: vec![serde_json::json!(idx)],
                id: JrpcId::Number(4),
            };
            if let Ok(resp) = daemon::daemon_rpc(poll_jrpc).await {
                if let Some(res) = resp.result {
                    if let Ok(prog) = serde_json::from_value::<RegistrationProgress>(res) {
                        if let Some(sec) = prog.secret {
                            state.secret_textarea = TextArea::default();
                            state.secret_textarea.insert_str(&sec);
                            state.secret_textarea.set_block(Block::default().borders(Borders::ALL).title("Login Secret"));
                            state.registration_status = "Registration complete!".into();
                            state.registration_idx = None;
                            state.sync_prefs();
                        } else {
                            state.registration_status = format!("Registering... {:.1}%", prog.progress * 100.0);
                        }
                    }
                } else if let Some(err) = resp.error {
                    state.registration_status = format!("Registration failed: {}", err.message);
                    state.registration_idx = None;
                }
            }
        }

        terminal.draw(|f| draw_ui(f, &mut state))?;

        if event::poll(Duration::from_millis(250))? {
            let event = event::read()?;
            
            if state.focus != Focus::None {
                if let Event::Key(key) = event {
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter => {
                            state.focus = Focus::None;
                            state.secret_textarea.set_style(Style::default());
                            state.socks_textarea.set_style(Style::default());
                            state.http_textarea.set_style(Style::default());
                        }
                        _ => {
                            match state.focus {
                                Focus::Secret => { state.secret_textarea.input(key); }
                                Focus::SocksPort => { state.socks_textarea.input(key); }
                                Focus::HttpPort => { state.http_textarea.input(key); }
                                _ => {}
                            }
                        }
                    }
                }
                continue;
            }

            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('q') => {
                        let _ = stop_daemon().await;
                        return Ok(());
                    }
                    KeyCode::Char('1') => state.tab = TabIdx::Status,
                    KeyCode::Char('2') => state.tab = TabIdx::Nodes,
                    KeyCode::Char('3') => state.tab = TabIdx::Config,
                    KeyCode::Char('4') => state.tab = TabIdx::Debug,
                    
                    KeyCode::Char('s') => {
                        if !state.is_running {
                            let exit = match &state.selected_country {
                                Some(country) => ExitConstraint::Country(country.clone()),
                                None => ExitConstraint::Auto,
                            };

                            let args = DaemonArgs {
                                secret: state.secret_textarea.lines().join(""),
                                metadata: serde_json::Value::Null,
                                prc_whitelist: false,
                                exit,
                                global_vpn: state.global_vpn,
                                listen_all: state.listen_all,
                                proxy_autoconf: false,
                                allow_direct: state.allow_direct,
                                socks5_port: state.socks_textarea.lines().join("").parse().unwrap_or(9909),
                                http_proxy_port: state.http_textarea.lines().join("").parse().unwrap_or(9910),
                                enable_debug_log: state.enable_debug_log,
                            };
                            let _ = start_daemon(args).await;
                        }
                    }
                    KeyCode::Char('x') => {
                        if state.is_running {
                            let _ = stop_daemon().await;
                        }
                    }
                    KeyCode::Char('e') if state.tab == TabIdx::Config => {
                        state.focus = Focus::Secret;
                        state.secret_textarea.set_style(Style::default().fg(Color::Yellow));
                    }
                    KeyCode::Char('p') if state.tab == TabIdx::Config => {
                        state.focus = Focus::SocksPort;
                        state.socks_textarea.set_style(Style::default().fg(Color::Yellow));
                    }
                    KeyCode::Char('h') if state.tab == TabIdx::Config => {
                        state.focus = Focus::HttpPort;
                        state.http_textarea.set_style(Style::default().fg(Color::Yellow));
                    }
                    KeyCode::Char('r') if state.tab == TabIdx::Config => {
                        // Start registration
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
                            Some(i) => if i >= state.countries.len().saturating_sub(1) { 0 } else { i + 1 },
                            None => 0,
                        };
                        state.node_list_state.select(Some(i));
                    }
                    KeyCode::Up if state.tab == TabIdx::Nodes => {
                        let i = match state.node_list_state.selected() {
                            Some(i) => if i == 0 { state.countries.len().saturating_sub(1) } else { i - 1 },
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
                        state.selected_country = None; // Auto
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
            }
            state.sync_prefs();
        }
    }
}

fn draw_ui(f: &mut ratatui::Frame, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Min(10), Constraint::Length(3)].as_ref())
        .split(f.area());

    let titles = vec!["1: Status", "2: Nodes", "3: Config", "4: Debug"]
        .into_iter()
        .map(|t| Line::from(t))
        .collect::<Vec<_>>();
    let tabs = Tabs::new(titles)
        .select(match state.tab {
            TabIdx::Status => 0,
            TabIdx::Nodes => 1,
            TabIdx::Config => 2,
            TabIdx::Debug => 3,
        })
        .block(Block::default().title("Tabs").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow));
    f.render_widget(tabs, chunks[0]);

    match state.tab {
        TabIdx::Status => draw_status(f, state, chunks[1]),
        TabIdx::Nodes => draw_nodes(f, state, chunks[1]),
        TabIdx::Config => draw_config(f, state, chunks[1]),
        TabIdx::Debug => draw_debug(f, state, chunks[1]),
    }

    let controls = Paragraph::new(
        "Press 's' Start | 'x' Stop | 'q' Quit | '1'-'4' Tabs"
    )
    .block(Block::default().title("Global Controls").borders(Borders::ALL));
    f.render_widget(controls, chunks[2]);
}

fn draw_status(f: &mut ratatui::Frame, state: &mut AppState, area: Rect) {
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

    let mut lines = vec![
        Line::from(vec![
            Span::raw("Daemon: "),
            Span::styled(if state.is_running { "Running" } else { "Stopped" }, if state.is_running { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Red) }),
            Span::raw(" | Network: "),
            Span::styled(status_text, status_style),
            Span::raw(" | Exit: "),
            Span::styled(state.selected_country.as_deref().unwrap_or("Auto"), Style::default().fg(Color::Cyan)),
        ])
    ];

    if let Some((version, path)) = &state.update_info {
        lines.push(Line::from(""));
        
        let is_chinese = sys_locale::get_locale().unwrap_or_default().contains("zh");
        if is_chinese {
            lines.push(Line::from(Span::styled(format!("提示: 迷雾通新版本 ({}) 的更新包已下载至:", version), Style::default().fg(Color::Yellow))));
            lines.push(Line::from(Span::styled(path.display().to_string(), Style::default().fg(Color::Yellow))));
            lines.push(Line::from(Span::styled("请您前往该目录手动处理此更新。", Style::default().fg(Color::Yellow))));
        } else {
            lines.push(Line::from(Span::styled(format!("Notice: Update package for Geph ({}) downloaded to:", version), Style::default().fg(Color::Yellow))));
            lines.push(Line::from(Span::styled(path.display().to_string(), Style::default().fg(Color::Yellow))));
            lines.push(Line::from(Span::styled("Please go to this directory and handle the update manually.", Style::default().fg(Color::Yellow))));
        }
    }

    let p = Paragraph::new(lines)
    .block(Block::default().title("Status").borders(Borders::ALL));
    f.render_widget(p, area);
}

fn draw_nodes(f: &mut ratatui::Frame, state: &mut AppState, area: Rect) {
    let mut items = vec![];
    for cc in &state.countries {
        let is_selected = state.selected_country.as_deref() == Some(cc.alpha2());
        let prefix = if is_selected { "[*]" } else { "[ ]" };
        let content = format!("{} {} - {}", prefix, cc.alpha2(), cc.name());
        items.push(ListItem::new(content));
    }

    let list = List::new(items)
        .block(Block::default().title("Regions (Up/Down to select, Enter to apply, 'a' for Auto)").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, area, &mut state.node_list_state);
}

fn draw_config(f: &mut ratatui::Frame, state: &mut AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0)
        ].as_ref())
        .split(area);

    f.render_widget(&state.secret_textarea, chunks[0]);
    f.render_widget(&state.socks_textarea, chunks[1]);
    f.render_widget(&state.http_textarea, chunks[2]);

    let vpn_text = format!("VPN Mode: {} (Press 'v' to toggle)", if state.global_vpn { "ON" } else { "OFF" });
    let vpn_p = Paragraph::new(vpn_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(vpn_p, chunks[3]);

    let listen_text = format!("Listen All Interfaces: {} (Press 'l' to toggle)", if state.listen_all { "ON" } else { "OFF" });
    let listen_p = Paragraph::new(listen_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(listen_p, chunks[4]);

    let direct_text = format!("Connection Mode: {} (Press 'b' to toggle)", if state.allow_direct { "Direct" } else { "Bridged" });
    let direct_p = Paragraph::new(direct_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(direct_p, chunks[5]);

    let hints = Paragraph::new(format!("Press 'e' for Secret, 'p' for SOCKS5, 'h' for HTTP port editing.\nEnter/Esc to finish. Press 'r' to register new secret.\n{}", state.registration_status));
    f.render_widget(hints, chunks[6]);
}

fn draw_debug(f: &mut ratatui::Frame, state: &mut AppState, area: Rect) {
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
    let logs_text: Vec<Line> = logs
        .iter()
        .map(|l| Line::from(l.as_str()))
        .collect();
    
    let max_scroll = logs.len().saturating_sub(chunks[1].height.saturating_sub(2) as usize) as u16;
    if state.debug_auto_scroll {
        state.debug_scroll = max_scroll;
    } else if state.debug_scroll > max_scroll {
        state.debug_scroll = max_scroll;
    }
    
    let p = Paragraph::new(logs_text)
        .block(Block::default().title("GUI Debug Logs (Up/Down to scroll)").borders(Borders::ALL))
        .scroll((state.debug_scroll, 0));
    f.render_widget(p, chunks[1]);
}
