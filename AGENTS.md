# AGENTS.md — geph-tui

## Project Scope

This project is the **TUI (terminal UI) client for Geph5**. All development work is in the `src/` directory, covering UI rendering, event handling, state management, and local daemon orchestration.

## geph5/ Submodule Rules

`geph5/` is a git submodule pointing to `https://github.com/geph-official/geph5.git`.

**Never modify anything inside `geph5/`.** It is a read-only upstream dependency.

### Pinned Commit

The geph5 submodule is pinned at `6f1373fc068097bb03cf6c30b5c961f19f5a2b19` (the last commit before VPN-by-default was merged). To restore:

```sh
cd geph5
git checkout 6f1373fc068097bb03cf6c30b5c961f19f5a2b19
```

## Code Structure

```
src/
├── main.rs          — Entry point, CLI arg parsing (TUI / --daemon / --config), main event loop
├── state.rs         — AppState, TuiPrefs (persisted config), Tab/Focus enums
├── event.rs         — Global key handling, focused input handling
├── daemon.rs        — Daemon lifecycle management, RPC transport layer
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

- This project does NOT include VPN/TUN functionality. The TUI is solely a front-end controller for SOCKS5/HTTP proxies.
- Config is persisted to `geph5_tui_prefs.json` (user config directory).
- The daemon is started as a subprocess via `--config <tmpfile>`, with control RPC over TCP (127.0.0.1:12222) or direct fallback.
