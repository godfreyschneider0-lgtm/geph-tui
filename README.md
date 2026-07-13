# geph-tui

A lightweight terminal UI client for the [Geph 5](https://geph.io/) proxy service. Provides SOCKS5 and HTTP proxy with an interactive TUI for configuration, node selection, and connection management.

Powered by [geph-lite](https://github.com/godfreyschneider0-lgtm/geph-lite) — a smol-runtime fork of the geph5 engine that idles at **40–60 MB RSS**.

## Architecture

```
┌─────────────────────────────┐
│  geph-tui process (smol)     │
│  - ratatui rendering          │
│  - keyboard events (crossterm)│
│  - TCP RPC client (nanorpc)   │
└──────────┬──────────────────┘
           │ TCP 127.0.0.1:12222
┌──────────▼──────────────────┐
│  geph5-client subprocess     │
│  - smol single-threaded       │
│  - sosistab3 / picomux        │
│  - SOCKS5 / HTTP proxy        │
└─────────────────────────────┘
```

The TUI is a **thin shell** — it does not link the engine as a library. It drives a separate `geph5-client` subprocess over TCP RPC. Communication uses `nanorpc` (line-delimited JSON-RPC).

## Requirements

- Rust toolchain (latest stable)

## Compilation

```sh
git clone --recursive https://github.com/godfreyschneider0-lgtm/geph-lite.git
cd geph-lite
cargo build --release -p geph-tui
cargo build --release -p geph5-client
```

The build produces two binaries in `target/release/`:
- `geph-tui` — the TUI application
- `geph5-client` — the proxy engine subprocess

## Running

```sh
cargo run --release                  # interactive TUI (default)
geph-tui --ctl start                 # headless daemon start
geph-tui --ctl status                # check daemon status
geph-tui --ctl stop                  # stop daemon
```

Or use the bundled `mikuctl` script:

```sh
mikuctl start    # start the daemon
mikuctl stop     # stop the daemon
mikuctl status   # show connection status
mikuctl log      # tail the log
mikuctl restart  # stop then start
```

### Keybindings (TUI)

| Key | Action |
|-----|--------|
| `1`–`4` | Switch tabs (Status / Regions / Config / Debug) |
| `s` / `x` | Start / stop connection |
| `e` | Edit Account ID |
| `p` / `h` | Edit SOCKS5 port / HTTP port |
| `l` | Toggle listen-all-interfaces |
| `b` | Toggle direct vs bridged mode |
| `r` | Register a new account |
| `Up`/`Down` + `Enter` | Select exit region |
| `q` | Quit |

Settings are saved automatically to `geph5_tui_prefs.json` in your config directory.

## Packaging

### Linux (.deb)

```sh
./package.sh                  # cargo-deb (native amd64)
./package.sh --arm64          # cross-compile for aarch64
./package.sh --manual         # manual dpkg-deb fallback
./package.sh --install        # build + install immediately
```

Install the produced `.deb`:
```sh
sudo dpkg -i geph-tui_*.deb
sudo apt remove geph-tui      # remove
```

### Termux (Android)

```sh
git clone https://github.com/termux/termux-packages.git
cp -r packages/geph-tui termux-packages/packages/
cd termux-packages
TERMUX_DOCKER_RUN_EXTRA_ARGS="--security-opt apparmor=unconfined" \
    ./scripts/run-docker.sh ./build-package.sh -I -f geph-tui
```

## Project structure

```
geph-lite/
├── src/                  # TUI application
│   ├── main.rs           # entry point, CLI args, event loop
│   ├── state.rs          # AppState, TuiPrefs, persisted config
│   ├── daemon.rs         # subprocess lifecycle + TCP RPC transport
│   ├── event.rs          # keyboard handling
│   ├── ui/               # ratatui rendering (status, nodes, config, debug)
│   └── default-config.yaml
├── geph5/                # geph-lite engine (submodule)
├── package.sh            # .deb packaging
├── mikuctl               # daemon control script
├── packages/geph-tui/    # termux package definition
└── package/              # debian templates
```

## License

MPL 2.0.
