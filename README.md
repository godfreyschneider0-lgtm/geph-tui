# gephgui-tui (formerly [gephgui-wry](https://github.com/geph-official/gephgui-wry))

This is the terminal user interface (TUI) client for Geph 5. It was previously a Webview-based GUI (`gephgui-wry`), but has been rewritten as a lightweight, cross-platform terminal app.

## Requirements

- Rust toolchain (latest stable)

## Compilation

Since this is now a pure Rust TUI application, building it is very straightforward:

```shell
git clone ...
cd gephgui-tui
cargo build --release
```

## Running

The binary has three modes. Run `gephgui-tui -h` (or `--help`) to see them all:

```shell
cargo run --release                      # interactive TUI (default)
cargo run --release -- --daemon          # headless daemon, uses saved config
cargo run --release -- --config <FILE>   # core client with a raw YAML config
```

### Interactive TUI (default)

Launch with no arguments. Configure your Account ID, pick a region, set ports in
the Config tab, then press `s` to connect. Settings are saved automatically to
`geph5_tui_prefs.json` in your config directory and reused by `--daemon`.

### Headless daemon (`--daemon`)

Starts the VPN with your previously-saved config — no UI. On startup it prints the
effective configuration (Account ID, connection mode, VPN mode, listen addresses,
exit region) to stdout, then runs until killed. Logs go to stderr.

```shell
# foreground (Ctrl+C to stop)
./gephgui-tui --daemon

# background
nohup ./gephgui-tui --daemon > geph.log 2>&1 &
# stop with: kill <pid>   (or launch the TUI and press 'x')
```

> First-time setup: run the TUI once to enter your Account ID and choose a region,
> then `--daemon` will reuse those settings.

>It can be compiled and run in termux.
>You need `pkg install perl` before compile.
>If you run the android version , you cannot use its `tun0` without root , but `socks5` is fine.

```sh
/data/data/com.termux/files/home/geph-tui/target/release # file gephgui-tui                                                                                                                       
gephgui-tui: ELF shared object, 64-bit LSB arm64, dynamic (/system/bin/linker64), for Android 24, built by NDK r29 (14206865), stripped
```

**Keybindings in the app:**
- `1`-`4`: Switch tabs (Status, Regions, Config, Debug)
- `s` / `x`: Start / stop VPN
- `q`: Quit application
- `e`, `p`, `h`: Edit Account ID, SOCKS5 port, HTTP port (in the Config tab)
- `v`, `l`, `b`: Toggle VPN mode / listen-all-interfaces / direct-vs-bridged
- `r`: Register a new account
- In the **Regions** tab: `Up`/`Down` to move, `Enter` to select a region, `a` for Auto
  (the specific exit node within a region is chosen automatically)

## License
The code is generally licensed under MPL 2.0. Low-level libraries useful to a wide variety of projects, such as the `sillad` framework, are generally licensed under the ISC license.
