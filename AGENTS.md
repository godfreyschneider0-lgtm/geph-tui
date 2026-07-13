# AGENTS.md — geph-tui

## Project Scope

This project is the **TUI (terminal UI) client for Geph5**. All development work is in the `src/` directory, covering UI rendering, event handling, state management, and local daemon orchestration.

The TUI is solely a **front-end controller for SOCKS5/HTTP proxies**. It does NOT include VPN/TUN functionality.

Release binary name: **MikuClub**. The Cargo package `gephgui-tui` is the development name.

## geph5/ Submodule Rules

`geph5/` is a git submodule pointing to the **maintainer's fork** at `https://github.com/godfreyschneider0-lgtm/geph5-socksOnly.git`.

**Never modify anything inside `geph5/` locally.** It is a read-only dependency. Upstream updates and fork syncs are managed by the maintainer — changes flow in via `git pull` on the fork, not local edits.

### Tracking Policy

The submodule **tracks latest `master`** — no longer pinned to a specific commit. To update:

```sh
cd geph5
git checkout master
git pull origin master
```

Then commit the updated pointer in the parent repo.

### Fork Workflow

The fork (`geph5-socksOnly`) exists to:
- **Control upstream merges**: the maintainer syncs upstream changes manually, testing before pulling into this repo.
- **Submit PRs upstream**: fixes and improvements to the geph5 engine can be proposed from this fork to `geph-official/geph5`.

## Architecture — Plan A: Thin Client Decoupling

The TUI is a **thin shell** that drives a separate `geph5-client` engine subprocess. It does NOT link the engine as a library.

```
┌─────────────────────────────┐
│  TUI process (smol runtime)  │
│  - ratatui rendering          │
│  - keyboard events (crossterm)│
│  - TCP RPC client (nanorpc)   │
│  - deps: geph5-misc-rpc       │
│          geph5-broker-protocol│
│          smol (lightweight)   │
└──────────┬──────────────────┘
           │ TCP 127.0.0.1:12222
┌──────────▼──────────────────┐
│  geph5-client subprocess     │
│  - tokio runtime (upstream)  │
│  - picomux + sosistab3       │
│  - SOCKS5/HTTP proxy          │
└─────────────────────────────┘
```

The TUI depends only on lightweight type crates from the geph5 workspace:
- `geph5-misc-rpc` — for the `Config` struct, `BrokerSource` types, and `ControlClient` RPC interface
- `geph5-broker-protocol` — for `ExitConstraint`, `Credential`, `UserInfo` types

These crates are intentionally lightweight (no engine code) so that front-end tools can configure and drive the engine without linking it.

## CLI Modes

The binary supports three modes:

| Mode | Args | Description |
|---|---|---|
| **Interactive TUI** | (none) | Full terminal UI with tabs, keybindings, real-time status |
| **Daemon control** | `--ctl <cmd>` | Headless control: `start`, `stop`, `status`. Used by `mikuctl` script. |
| **Help** | `-h` / `--help` | Print usage info |

The old `--daemon` and `--config` modes have been removed. Headless daemon operation is now handled by `mikuctl`.

## Code Structure

```
src/
├── main.rs          — Entry point, CLI arg parsing (TUI / --ctl), main event loop
├── state.rs         — AppState, TuiPrefs (persisted config), Tab/Focus enums,
│                      LogWriter (tracing sink with 1000-line ring buffer)
├── event.rs         — Global key handling, focused input handling
├── daemon.rs        — Daemon lifecycle: subprocess management + TCP RPC transport
│                      (nanorpc RpcTransport trait over TCP)
├── autoupdate.rs    — Auto-update checking
├── test_log.rs      — Test helper for log infrastructure
├── ui/
│   ├── mod.rs       — UI layout entry (tab switching)
│   ├── status.rs    — Status tab: connection status, notices, news bar
│   ├── nodes.rs     — Regions tab: node selection
│   ├── config.rs    — Config tab: account/ports/connection mode settings
│   └── debug.rs     — Debug tab: log viewer
└── default-config.yaml — geph5-client default config template

mikuctl              — Bash daemon control script (start/stop/status/log/restart)
```

## Design Constraints

- **No VPN/TUN**: The TUI is solely a front-end controller for SOCKS5/HTTP proxies.
- **No engine library linking**: The TUI does NOT depend on `geph5-client` as a library. It uses only `geph5-misc-rpc` and `geph5-broker-protocol` for type definitions.
- **Subprocess-only engine interaction**: `geph5-client` runs as a separate subprocess binary. All communication is over TCP RPC (`127.0.0.1:12222`) via `nanorpc`. No in-process engine fallback.
- **Config persistence**: User preferences are persisted to `geph5_tui_prefs.json` (user config directory, via `dirs::config_dir()`).
- **Low memory**: TUI uses `smol` (single-threaded, lightweight). Do NOT switch to tokio — the engine's tokio runtime lives in the separate subprocess where it belongs.
