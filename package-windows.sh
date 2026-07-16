#!/usr/bin/env bash
set -euo pipefail

#
# package-windows.sh — Cross-compile geph-tui for Windows (mingw) and produce a scoop-ready zip
#
# Prerequisites:
#   rustup target add x86_64-pc-windows-gnu
#   sudo apt install mingw-w64
#
# Usage:
#   ./package-windows.sh                  # Build + zip
#   ./package-windows.sh --skip-build     # Zip from existing binaries
#

SCRIPT_DIR="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
REPO_ROOT="$SCRIPT_DIR"
TARGET="x86_64-pc-windows-gnu"

# ── Utility functions ───────────────────────────────────
info() { printf '\033[1;34m[INFO]\033[0m  %s\n' "$*"; }
ok()   { printf '\033[1;32m[ OK ]\033[0m  %s\n' "$*"; }
die()  { printf '\033[1;31m[ERR]\033[0m  %s\n' "$*" >&2; exit 1; }

# ── Argument parsing ───────────────────────────────────
SKIP_BUILD=false
for arg in "$@"; do
    case "$arg" in
        --skip-build) SKIP_BUILD=true ;;
        --help|-h)
            cat <<EOF
Usage: $0 [options]

  (default)      Cross-compile for Windows (x86_64) and produce scoop-ready zip
  --skip-build   Zip from existing binaries without recompiling
EOF
            exit 0 ;;
        *) die "Unknown argument: $arg" ;;
    esac
done

# ── Environment checks ────────────────────────────────
command -v cargo >/dev/null 2>&1 || die "cargo not found. Install: https://rustup.rs"
command -v zip >/dev/null 2>&1   || die "zip not found. Install: sudo apt install zip"

if ! rustup target list --installed 2>/dev/null | grep -q "$TARGET"; then
    info "Installing Rust target $TARGET ..."
    rustup target add "$TARGET" || die "failed to add target $TARGET"
fi

if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    die "mingw-w64 cross-compiler not found. Install: sudo apt install mingw-w64"
fi

export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc

# ── Build ─────────────────────────────────────────────
if ! $SKIP_BUILD; then
    info "Cross-compiling geph5-client ($TARGET) ..."
    cargo build --release -p geph5-client \
        --manifest-path "$REPO_ROOT/Cargo.toml" \
        --target "$TARGET" \
        || die "geph5-client build failed"

    info "Cross-compiling geph-tui ($TARGET) ..."
    cargo build --release -p geph-tui \
        --manifest-path "$REPO_ROOT/Cargo.toml" \
        --target "$TARGET" \
        || die "geph-tui build failed"
fi

RELEASE_DIR="$REPO_ROOT/target/$TARGET/release"

TUI_BIN="$RELEASE_DIR/geph-tui.exe"
CLIENT_BIN="$RELEASE_DIR/geph5-client.exe"
[ -f "$TUI_BIN" ]    || die "Binary not found: $TUI_BIN"
[ -f "$CLIENT_BIN" ] || die "Binary not found: $CLIENT_BIN"

# ── Version ───────────────────────────────────────────
VERSION="$(grep '^version' "$REPO_ROOT/Cargo.toml" | head -1 | sed 's/.*version = "\(.*\)".*/\1/')"
ARCHIVE_NAME="geph-tui-${VERSION}-windows-x64.zip"
STAGING_DIR="$(mktemp -d)"
trap 'rm -rf "$STAGING_DIR"' EXIT

# ── Assemble zip ──────────────────────────────────────
info "Assembling $ARCHIVE_NAME ..."
mkdir -p "$STAGING_DIR/geph-tui"
cp "$TUI_BIN"    "$STAGING_DIR/geph-tui/"
cp "$CLIENT_BIN" "$STAGING_DIR/geph-tui/"

OUTPUT="$REPO_ROOT/$ARCHIVE_NAME"
info "Creating zip archive ..."
(cd "$STAGING_DIR" && zip -r "$OUTPUT" geph-tui/) || die "zip failed"

# ── Print result ──────────────────────────────────────
echo ""
ok "Package built successfully!"
echo ""
echo "  File:     $OUTPUT"
echo "  Size:     $(du -h "$OUTPUT" | cut -f1)"
echo "  Contents: geph-tui.exe, geph5-client.exe"
echo ""
echo "  Scoop:    Update hash in package/scoop/geph-tui.json"
echo ""
echo "  SHA256:   $(sha256sum "$OUTPUT" | cut -d' ' -f1)"
