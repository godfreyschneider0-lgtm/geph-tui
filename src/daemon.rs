use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    process::Command,
    sync::LazyLock,
    time::Duration,
};

use anyhow::Context;
use async_trait::async_trait;
use futures_util::{io::BufReader, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use geph5_misc_rpc::client_control::ControlClient;
use isocountry::CountryCode;
use nanorpc::{JrpcRequest, JrpcResponse, RpcTransport};
use oneshot::channel as oneshot_channel;
use oneshot::Receiver as OneshotReceiver;
use smol::future::FutureExt as SmolFutureExt;
use smol::net::TcpStream;
use smol_timeout2::TimeoutExt;
use std::process::Stdio;
use tempfile::NamedTempFile;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::state::TuiPrefs;

const DEFAULT_CONFIG_YAML: &str = include_str!("default-config.yaml");

const CONTROL_ADDR: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12222);

pub async fn start_daemon(prefs: &TuiPrefs) -> anyhow::Result<()> {
    // if args.proxy_autoconf {
    //     configure_proxy()?;
    // }
    let crash_rx = start_daemon_inner(prefs)?;
    let start_fut = async {
        wait_daemon_start()
            .timeout(Duration::from_secs(30))
            .await
            .context("daemon did not start in 30")?;
        Ok::<(), anyhow::Error>(())
    };
    let crash_fut = async {
        match crash_rx.await {
            Ok(stderr) => {
                anyhow::bail!("daemon exited before becoming reachable:\n{}", stderr)
            }
            Err(_) => {
                anyhow::bail!("daemon exited before becoming reachable")
            }
        }
    };
    start_fut.race(crash_fut).await?;
    smol::Timer::after(Duration::from_millis(500)).await;
    Ok(())
}

fn start_daemon_inner(prefs: &TuiPrefs) -> anyhow::Result<OneshotReceiver<String>> {
    let cfg = running_cfg(prefs);

    let mut tfile = NamedTempFile::with_suffix(".yaml")?;
    let val = serde_json::to_value(&cfg)?;

    tfile.write_all(serde_yaml::to_string(&val)?.as_bytes())?;
    tfile.flush()?;
    let (_, path) = tfile.keep()?;

    let (sender, receiver) = oneshot_channel::<String>();

    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.arg("--config").arg(path);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    cmd.stdout(Stdio::null()); // Mute stdout
    if prefs.enable_debug_log {
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("gephgui.log")
        {
            cmd.stderr(Stdio::from(file));
        } else {
            cmd.stderr(Stdio::piped());
        }
    } else {
        cmd.stderr(Stdio::null());
    }
    let mut child = cmd.spawn()?;
    std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            stderr.read_to_string(&mut buf).ok();
        }
        let _ = child.wait();
        let _ = sender.send(buf);
    });

    Ok(receiver)
}

async fn wait_daemon_start() {
    smol::Timer::after(Duration::from_millis(150)).await;
    while let Err(err) = check_daemon().await {
        tracing::warn!(err = debug(err), "daemon check result");
        smol::Timer::after(Duration::from_millis(250)).await;
    }
}

async fn check_daemon() -> anyhow::Result<()> {
    TcpStream::connect(CONTROL_ADDR)
        .timeout(Duration::from_millis(50))
        .await
        .context("timeout")??;
    Ok(())
}

pub async fn stop_daemon() -> anyhow::Result<()> {
    // let _ = deconfigure_proxy();
    stop_daemon_inner().await
}

async fn stop_daemon_inner() -> anyhow::Result<()> {
    let _ = ControlClient(DaemonRpcTransport).stop().await;
    smol::Timer::after(Duration::from_millis(1000)).await;
    Ok(())
}

pub async fn daemon_running() -> bool {
    check_daemon().await.is_ok()
}

/// Either dispatches to a running daemon, or virtually starts a dryrun daemon and runs with it
pub async fn daemon_rpc(inner: JrpcRequest) -> anyhow::Result<JrpcResponse> {
    match daemon_rpc_tcp(inner.clone())
        .timeout(Duration::from_secs(3))
        .await
    {
        Some(Ok(resp)) => Ok(resp),
        Some(Err(_err)) => {
            // tracing::warn!(
            //     method = debug(&inner.method),
            //     err = debug(err),
            //     "error calling TCP, falling back to direct"
            // );
            daemon_rpc_direct(inner).await
        }
        None => {
            anyhow::bail!("timed out")
        }
    }
}

async fn daemon_rpc_tcp(inner: JrpcRequest) -> anyhow::Result<JrpcResponse> {
    let conn = TcpStream::connect(CONTROL_ADDR)
        .timeout(Duration::from_millis(50))
        .await
        .context("timeout")
        .and_then(|s| Ok(s?))?;
    let (read, mut write) = conn.split();
    write
        .write_all(format!("{}\n", serde_json::to_string(&inner)?).as_bytes())
        .await?;
    let mut read = BufReader::new(read);
    let mut buf = String::new();
    read.read_line(&mut buf).await?;
    Ok(serde_json::from_str(&buf)?)
}

async fn daemon_rpc_direct(inner: JrpcRequest) -> anyhow::Result<JrpcResponse> {
    if inner.method == "stop" {
        anyhow::bail!("cannot stop now lol");
    }
    static DAEMON: LazyLock<geph5_client::Client> =
        LazyLock::new(|| geph5_client::Client::start(default_config().inert()));
    DAEMON.control_client().0.call_raw(inner).await
}

pub struct DaemonRpcTransport;

#[async_trait]
impl RpcTransport for DaemonRpcTransport {
    type Error = anyhow::Error;
    async fn call_raw(&self, req: JrpcRequest) -> Result<JrpcResponse, Self::Error> {
        daemon_rpc(req).await
    }
}

fn default_config() -> geph5_client::Config {
    static DEFAULT_CONFIG: LazyLock<geph5_client::Config> = LazyLock::new(|| {
        let value: serde_json::Value = serde_yaml::from_str(DEFAULT_CONFIG_YAML)
            .expect("default-config.yaml must deserialize into serde_json::Value");
        let mut cfg: geph5_client::Config = serde_json::from_value(value)
            .expect("default-config.yaml must deserialize into geph5_client::Config");

        let cache_dir = dirs::cache_dir().unwrap_or_else(|| std::env::temp_dir());
        let geph_cache = cache_dir.join("geph5_tui");
        let _ = std::fs::create_dir_all(&geph_cache);
        cfg.cache = Some(geph_cache.join("database.db"));
        cfg
    });

    DEFAULT_CONFIG.clone()
}

pub fn clear_conn_token_cache() {
    let cache_dir = dirs::cache_dir().unwrap_or_else(|| std::env::temp_dir());
    let db_dir = cache_dir.join("geph5_tui");
    for name in ["database.db", "database.db-wal", "database.db-shm"] {
        let _ = std::fs::remove_file(db_dir.join(name));
    }
    tracing::info!("cleared token cache database");
}

pub fn running_cfg(prefs: &TuiPrefs) -> geph5_client::Config {
    let mut cfg = default_config();

    cfg.passthrough_china = false;
    cfg.credentials = geph5_broker_protocol::Credential::Secret(prefs.secret.clone());
    let listen_ip = if prefs.listen_all {
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))
    } else {
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
    };
    let socks5_port: u16 = prefs.socks_port.parse().unwrap_or(9909);
    let http_proxy_port: u16 = prefs.http_port.parse().unwrap_or(9910);
    cfg.socks5_listen = Some(SocketAddr::new(listen_ip, socks5_port));
    cfg.http_proxy_listen = Some(SocketAddr::new(listen_ip, http_proxy_port));

    cfg.exit_constraint = match &prefs.selected_country {
        Some(cc) => geph5_client::ExitConstraint::Country(CountryCode::for_alpha2(cc).unwrap()),
        None => geph5_client::ExitConstraint::Auto,
    };

    cfg.sess_metadata = serde_json::Value::Null;
    cfg.allow_direct = prefs.allow_direct;

    cfg
}

#[cfg(test)]
mod tests {
    use crate::daemon::default_config;

    #[test]
    fn test_dump_default_config() {
        // Get the default configuration
        let config = default_config();

        // Convert to JSON and pretty print
        let json_config =
            serde_json::to_string_pretty(&config).expect("Failed to serialize config to JSON");

        // Print the JSON representation for inspection
        println!("Default config JSON representation:");
        println!("{}", json_config);

        // Assert that the config can be serialized (this should never fail if the previous step succeeded)
        assert!(serde_json::to_string(&config).is_ok());
    }
}
