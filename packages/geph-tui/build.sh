TERMUX_PKG_HOMEPAGE=https://github.com/godfreyschneider0-lgtm/geph-lite
TERMUX_PKG_DESCRIPTION="Lightweight TUI client for Geph 5"
TERMUX_PKG_LICENSE="MPL-2.0"
TERMUX_PKG_MAINTAINER="@godfreyschneider0-lgtm"
TERMUX_PKG_VERSION="2.1.0"
TERMUX_PKG_SRCURL=https://github.com/godfreyschneider0-lgtm/geph-lite.git
TERMUX_PKG_GIT_BRANCH="main"
TERMUX_PKG_BUILD_IN_SRC=true
TERMUX_PKG_AUTO_UPDATE=false

termux_step_pre_configure() {
    cd "$TERMUX_PKG_SRCDIR"
    git submodule update --init --recursive --depth=1

    termux_setup_rust
    : "${CARGO_HOME:=$HOME/.cargo}"
    export CARGO_HOME
    cargo fetch --target "${CARGO_TARGET_NAME}"
}

termux_step_make() {
    cd "$TERMUX_PKG_SRCDIR"
    cargo build --jobs "$TERMUX_PKG_MAKE_PROCESSES" --release \
        --target "$CARGO_TARGET_NAME" -p geph-tui
    cargo build --jobs "$TERMUX_PKG_MAKE_PROCESSES" --release \
        --target "$CARGO_TARGET_NAME" -p geph5-client --features aws_lambda
}

termux_step_make_install() {
    install -Dm700 \
        "$TERMUX_PKG_SRCDIR/target/${CARGO_TARGET_NAME}/release/geph-tui" \
        "$TERMUX_PREFIX/bin/geph-tui"

    install -Dm700 \
        "$TERMUX_PKG_SRCDIR/target/${CARGO_TARGET_NAME}/release/geph5-client" \
        "$TERMUX_PREFIX/bin/geph5-client"

    install -Dm700 \
        "$TERMUX_PKG_SRCDIR/gephctl" \
        "$TERMUX_PREFIX/bin/gephctl"
}
