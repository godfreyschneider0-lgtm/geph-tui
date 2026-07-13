# AGENTS.md — geph-tui

## Project Scope

This project is the **TUI (terminal UI) client for Geph5**. All development work is in the `src/` directory, covering UI rendering, event handling, state management, and local daemon orchestration.

The TUI is solely a **front-end controller for SOCKS5/HTTP proxies**. It does NOT include VPN/TUN functionality.

## geph5/ Submodule Rules

`geph5/` is a git submodule pointing to `https://github.com/geph-official/geph5.git`.

**Never modify anything inside `geph5/`.** It is a read-only upstream dependency.

### Tracking Policy

The submodule **tracks latest `master`** — no longer pinned to a specific commit. To update:

```sh
cd geph5
git checkout master
git pull origin master
```

Then commit the updated pointer in the parent repo.

## Architecture — Plan A: Thin Client Decoupling

The TUI is a **thin shell** that drives a separate `geph5-client` engine subprocess. It does NOT link the engine as a library.

```
┌─────────────────────────────┐
│  TUI process (smol runtime)  │
│  - ratatui rendering          │
│  - keyboard events (crossterm)│
│  - TCP RPC client             │
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
- `geph5-misc-rpc` — for the `Config` struct and `BrokerSource` types
- `geph5-broker-protocol` — for `ExitConstraint`, `Credential`, `UserInfo` types

These crates are intentionally lightweight (no engine code) so that front-end tools can configure and drive the engine without linking it.

## Code Structure

```
src/
├── main.rs          — Entry point, CLI arg parsing (TUI / --daemon), main event loop
├── state.rs         — AppState, TuiPrefs (persisted config), Tab/Focus enums
├── event.rs         — Global key handling, focused input handling
├── daemon.rs        — Daemon lifecycle: subprocess management + TCP RPC transport
├── autoupdate.rs    — Auto-update checking
├── ui/
│   ├── mod.rs       — UI layout entry (tab switching)
│   ├── status.rs    — Status tab: connection status, notices, news bar
│   ├── nodes.rs     — Regions tab: node selection
│   ├── config.rs    — Config tab: account/ports/connection mode settings
│   └── debug.rs     — Debug tab: log viewer
└── default-config.yaml — geph5-client default config template
```

## Design Constraints

- **No VPN/TUN**: The TUI is solely a front-end controller for SOCKS5/HTTP proxies.
- **No engine library linking**: The TUI does NOT depend on `geph5-client` as a library. It uses only `geph5-misc-rpc` and `geph5-broker-protocol` for type definitions.
- **Subprocess-only engine interaction**: `geph5-client` runs as a separate subprocess binary. All communication is over TCP RPC (`127.0.0.1:12222`). No in-process engine fallback.
- **Config persistence**: User preferences are persisted to `geph5_tui_prefs.json` (user config directory, via `dirs::config_dir()`).
- **Low memory**: TUI uses `smol` (single-threaded, lightweight). Do NOT switch to tokio — the engine's tokio runtime lives in the separate subprocess where it belongs.
