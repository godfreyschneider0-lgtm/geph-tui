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

Run the compiled executable:

```shell
cargo run --release
```

>It can be compiled and run in termux.
>You need `pkg install perl` before compile.
>If you run the android version , you cannot use its `tun0` without root , but `socks5` is fine.

```sh
/data/data/com.termux/files/home/geph-tui/target/release # file gephgui-tui                                                                                                                       
gephgui-tui: ELF shared object, 64-bit LSB arm64, dynamic (/system/bin/linker64), for Android 24, built by NDK r29 (14206865), stripped
```

**Keybindings in the app:**
- `1`-`4`: Switch tabs (Status, Nodes, Config, Debug)
- `s`: Start VPN
- `x`: Stop VPN
- `q`: Quit application
- `e`, `p`, `h`: Edit Secret, SOCKS5 port, HTTP port respectively in the Config tab.

## License
The code is generally licensed under MPL 2.0. Low-level libraries useful to a wide variety of projects, such as the `sillad` framework, are generally licensed under the ISC license.
