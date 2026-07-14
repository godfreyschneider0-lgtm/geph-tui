#!/usr/bin/env bash
set -euo pipefail

#
# package.sh — Build geph-tui .deb packages (Linux)
#
# Usage:
#   ./package.sh                     # Default: cargo deb (native amd64)
#   ./package.sh --manual            # Manual dpkg-deb (uses package/DEBIAN/ templates)
#   ./package.sh --arm64             # Cross-compile for aarch64
#   ./package.sh --manual --arm64    # Manual arm64 build
#   ./package.sh --skip-build        # cargo deb --no-build (use existing binary)
#   ./package.sh --install           # Install immediately after build
#
# Output:
#   cargo-deb -> target/debian/geph-tui_<VERSION>_<ARCH>.deb
#   manual    -> ./geph-tui_<VERSION>_<ARCH>.deb
#

SCRIPT_DIR="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
REPO_ROOT="$SCRIPT_DIR"

# ── Utility functions ───────────────────────────────────
info() { printf '\033[1;34m[INFO]\033[0m  %s\n' "$*"; }
ok()   { printf '\033[1;32m[ OK ]\033[0m  %s\n' "$*"; }
die()  { printf '\033[1;31m[ERR]\033[0m  %s\n' "$*" >&2; exit 1; }

# ── Argument parsing ───────────────────────────────────
MODE="cargo-deb"
INSTALL=false
ARM64=false
CARGO_DEB_EXTRA_ARGS=()

for arg in "$@"; do
    case "$arg" in
        --manual)      MODE="manual" ;;
        --arm64)       ARM64=true ;;
        --install)     INSTALL=true ;;
        --skip-build)  CARGO_DEB_EXTRA_ARGS+=("--no-build") ;;
        --help|-h)     cat <<EOF
Usage: $0 [options]

  (default)      cargo deb build (native amd64)
  --manual       Manual dpkg-deb (uses package/DEBIAN/ templates)
  --arm64        Cross-compile for aarch64 (aarch64-unknown-linux-gnu)
  --skip-build   Skip compilation
  --install      Install immediately after build (dpkg -i)
EOF
                        exit 0 ;;
        *) die "Unknown argument: $arg" ;;
    esac
done

# ── Target setup ────────────────────────────────────────
if $ARM64; then
    TARGET="aarch64-unknown-linux-gnu"
    ARCH="arm64"
    TARGET_FLAG=(--target "$TARGET")
    TARGET_DIR="$TARGET"
    info "Cross-compiling for arm64 ($TARGET)"
    command -v aarch64-linux-gnu-gcc >/dev/null 2>&1 \
        || die "aarch64 cross-compiler not found. Install: sudo apt install gcc-aarch64-linux-gnu"
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
else
    TARGET=""
    ARCH="$(uname -m)"
    case "$ARCH" in x86_64|amd64) ARCH="amd64" ;; aarch64|arm64) ARCH="arm64" ;; esac
    TARGET_FLAG=()
    TARGET_DIR="."
fi

# ── cargo-deb mode ─────────────────────────────────────
if [ "$MODE" = "cargo-deb" ]; then
    command -v cargo-deb >/dev/null 2>&1 \
        || die "cargo-deb not found. Install: cargo install cargo-deb"

    info "Pre-building geph5-client..."
    cargo build --release -p geph5-client \
        --manifest-path "$REPO_ROOT/Cargo.toml" "${TARGET_FLAG[@]}" \
        || die "geph5-client build failed"

    info "Pre-building geph-tui..."
    cargo build --release -p geph-tui \
        --manifest-path "$REPO_ROOT/Cargo.toml" "${TARGET_FLAG[@]}" \
        || die "geph-tui build failed"

    info "Packaging deb (no rebuild)..."
    cargo deb --no-build --manifest-path "$REPO_ROOT/Cargo.toml" "${TARGET_FLAG[@]}"

    OUTPUT="$(ls -t "$REPO_ROOT/target/debian/geph-tui_"*"_${ARCH}.deb" 2>/dev/null | head -1)"
    [ -f "$OUTPUT" ] || die "deb not found in target/debian/ for arch ${ARCH}"

    if $INSTALL; then
        info "Installing ${OUTPUT##*/} ..."
        sudo dpkg -i "$OUTPUT"
        ok "Installed. Run: geph-tui (interactive TUI) or gephctl start (daemon)"
    else
        echo ""
        ok "Package built successfully!"
        echo ""
        echo "  File:     $OUTPUT"
        echo "  Size:     $(du -h "$OUTPUT" | cut -f1)"
        echo ""
        echo "  Install:  sudo dpkg -i ${OUTPUT##*/}"
        echo "  Remove:   sudo apt remove geph-tui"
        echo "  Purge:    sudo apt purge geph-tui"
    fi
    exit 0
fi

# ── manual mode (dpkg-deb + package/DEBIAN/ templates) ──────
STAGING_DIR="$(mktemp -d)"
trap 'rm -rf "$STAGING_DIR"' EXIT

PACKAGE_NAME="geph-tui"
BINARY_NAME="geph-tui"
CTL_NAME="gephctl"
PREFIX="/usr"
DEST_DIR="${STAGING_DIR}${PREFIX}"
DEST_BIN="${DEST_DIR}/bin"
WORKSPACE_VERSION="2.1.0"

RELEASE_DIR="$REPO_ROOT/target/${TARGET_DIR}/release"

info "Compiling TUI (release)..."
cargo build --release -p geph-tui --manifest-path "$REPO_ROOT/Cargo.toml" "${TARGET_FLAG[@]}" \
    || die "cargo build failed"

info "Compiling geph5-client (release, aws_lambda)..."
cargo build --release -p geph5-client --features aws_lambda \
    --manifest-path "$REPO_ROOT/Cargo.toml" "${TARGET_FLAG[@]}" \
    || die "geph5-client build failed"

BINARY_SRC="$RELEASE_DIR/geph-tui"
[ -f "$BINARY_SRC" ] || die "Binary not found: $BINARY_SRC"

ENGINE_SRC="$RELEASE_DIR/geph5-client"
[ -f "$ENGINE_SRC" ] || die "Engine binary not found: $ENGINE_SRC"

VERSION="$(grep -A2 '^\[package\]' "$REPO_ROOT/Cargo.toml" \
         | grep 'version' | sed 's/.*version = "\(.*\)".*/\1/' || echo "$WORKSPACE_VERSION")"

info "Assembling package: ${PACKAGE_NAME} ${VERSION} (${ARCH})"
mkdir -p "$DEST_BIN"
cp "$BINARY_SRC" "${DEST_BIN}/${BINARY_NAME}"
cp "$ENGINE_SRC" "${DEST_BIN}/geph5-client"
cp "$REPO_ROOT/gephctl" "${DEST_BIN}/${CTL_NAME}"
chmod 755 "${DEST_BIN}/${BINARY_NAME}" "${DEST_BIN}/geph5-client" "${DEST_BIN}/${CTL_NAME}"

mkdir -p "${STAGING_DIR}/DEBIAN"
mkdir -p "${STAGING_DIR}/usr/share/doc/${PACKAGE_NAME}"
cp "$REPO_ROOT/package/deb-copyright" "${STAGING_DIR}/usr/share/doc/${PACKAGE_NAME}/copyright"
cp "$REPO_ROOT/package/DEBIAN/control.template" "${STAGING_DIR}/DEBIAN/control"
sed -i "s/%%VERSION%%/${VERSION}/g; s/%%ARCH%%/${ARCH}/g" "${STAGING_DIR}/DEBIAN/control"
cp "$REPO_ROOT/package/DEBIAN/postinst" "${STAGING_DIR}/DEBIAN/postinst"
cp "$REPO_ROOT/package/DEBIAN/postrm"  "${STAGING_DIR}/DEBIAN/postrm"
chmod 755 "${STAGING_DIR}/DEBIAN/postinst" "${STAGING_DIR}/DEBIAN/postrm"

OUTPUT="${REPO_ROOT}/${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"
info "Building ${OUTPUT##*/} (manual mode)..."
dpkg-deb --build --root-owner-group "$STAGING_DIR" "$OUTPUT" || die "dpkg-deb failed"

if $INSTALL; then
    info "Installing ${OUTPUT##*/} ..."
    sudo dpkg -i "$OUTPUT"
    ok "Installed."
else
    echo ""
    ok "Package built successfully!"
    echo ""
    echo "  File:     $OUTPUT"
    echo "  Size:     $(du -h "$OUTPUT" | cut -f1)"
    echo ""
    echo "  Install:  sudo dpkg -i ${OUTPUT##*/}"
fi
