#!/usr/bin/env bash
set -euo pipefail

#
# package.sh — Build MikuClub .deb packages
#
# Usage:
#   ./package.sh                # Default: cargo deb (Linux, auto-compile + package)
#   ./package.sh --skip-build    # cargo deb --no-build (use existing binary)
#   ./package.sh --manual       # Manual dpkg-deb (uses package/DEBIAN/ templates, Linux)
#   ./package.sh --termux       # Docker CI build for Termux .deb (aarch64)
#   ./package.sh --install      # Install immediately after build
#
# Output:
#   cargo-deb -> target/debian/mikuclub_<VERSION>_<ARCH>.deb
#   manual    -> ./mikuclub_<VERSION>_<ARCH>.deb
#   termux     -> ./mikuclub_<VERSION>_aarch64.deb
#

SCRIPT_DIR="$(cd "$(dirname "$(readlink -f "$0")")") && pwd)"
REPO_ROOT="$SCRIPT_DIR"

# ── Utility functions ───────────────────────────────────
info() { printf '\033[1;34m[INFO]\033[0m  %s\n' "$*"; }
ok()   { printf '\033[1;32m[ OK ]\033[0m  %s\n' "$*"; }
die()  { printf '\033[1;31m[ERR]\033[0m  %s\n' "$*" >&2; exit 1; }

# ── Argument parsing ───────────────────────────────────
MODE="cargo-deb"
INSTALL=false
CARGO_DEB_EXTRA_ARGS=()

for arg in "${@:-}"; do
    case "$arg" in
        --termux)      MODE="termux" ;;
        --manual)      MODE="manual" ;;
        --install)     INSTALL=true ;;
        --skip-build)  CARGO_DEB_EXTRA_ARGS+=("--no-build") ;;
        --help|-h)     cat <<EOF
Usage: $0 [options]

  (default)      cargo deb build (Linux)
  --termux       Docker CI build (Termux, aarch64)
  --manual       Manual dpkg-deb (uses package/DEBIAN/ templates)
  --skip-build   Skip compilation
  --install      Install immediately after build (dpkg -i / pkg install)
EOF
                        exit 0 ;;
        *) die "Unknown argument: $arg" ;;
    esac
done

# ── cargo-deb mode ─────────────────────────────────────
if [ "$MODE" = "cargo-deb" ]; then
    command -v cargo-deb >/dev/null 2>&1 \
        || die "cargo-deb not found. Install: cargo install cargo-deb"

    info "Building deb via cargo-deb ..."
    cargo deb --manifest-path "$REPO_ROOT/Cargo.toml" "${CARGO_DEB_EXTRA_ARGS[@]}"

    OUTPUT="$(ls -t "$REPO_ROOT/target/debian/"mikuclub_*.deb 2>/dev/null | head -1)"
    [ -f "$OUTPUT" ] || die "deb not found in target/debian/"

    if $INSTALL; then
        info "Installing ${OUTPUT##*/} ..."
        sudo dpkg -i "$OUTPUT"
        ok "Installed. Run: MikuClub (interactive TUI) or mikuctl start (daemon)"
    else
        echo ""
        ok "Package built successfully!"
        echo ""
        echo "  File:     $OUTPUT"
        echo "  Size:     $(du -h "$OUTPUT" | cut -f1)"
        echo ""
        echo "  Install:  sudo dpkg -i ${OUTPUT##*/}"
        echo "  Remove:   sudo apt remove mikuclub"
        echo "  Purge:    sudo apt purge mikuclub"
    fi
    exit 0
fi

# ── termux mode (Docker CI) ────────────────────────────
if [ "$MODE" = "termux" ]; then
    command -v docker >/dev/null 2>&1 \
        || die "docker not found. Install Docker first."

    TERMUX_PACKAGES_DIR="${TERMUX_PACKAGES_DIR:-${REPO_ROOT}/termux-packages}"
    if [ ! -d "$TERMUX_PACKAGES_DIR" ]; then
        die "termux-packages not found at '$TERMUX_PACKAGES_DIR'."
              "Clone it: git clone https://github.com/termux/termux-packages.git"
    fi

    # Check if mikuclub build.sh exists
    if [ ! -f "$TERMUX_PACKAGES_DIR/packages/mikuclub/build.sh" ]; then
        die "packages/mikuclub/build.sh not found in '$TERMUX_PACKAGES_DIR'."
              "See PACKAGING.md for setup instructions."
    fi

    # Extra Docker CI arguments (AppArmor compatibility)
    DOCKER_EXTRA_ARGS="${TERMUX_DOCKER_EXTRA_ARGS:---security-opt apparmor=unconfined}"

    info "Building Termux deb via Docker CI ..."
    info "  termux-packages: $TERMUX_PACKAGES_DIR"
    info "  docker extra:  $DOCKER_EXTRA_ARGS"
    echo ""

    BUILD_CMD="TERMUX_DOCKER_RUN_EXTRA_ARGS=\"$DOCKER_EXTRA_ARGS\" ./scripts/run-docker.sh ./build-package.sh -I -f mikuclub"

    if $INSTALL; then
        die "--install is not supported for Termux mode."
              "Copy the output .deb to your device and run: pkg install mikuclub_*.deb"
    fi

    ( cd "$TERMUX_PACKAGES_DIR" && eval "$BUILD_CMD" )

    # Copy output from Docker container
    DEB_FILE="$TERMUX_PACKAGES_DIR/output/mikuclub_"*"*.deb
    if ls $DEB_FILE 2>/dev/null | head -1 | grep -q .; then
        cp $DEB_FILE "$REPO_ROOT/"
        echo ""
        ok "Termux deb built successfully!"
        for f in $DEB_FILE; do
            [ -f "$f" ] || continue
            cp "$f" "$REPO_ROOT/"
            echo "  File:     $(basename "$f") -> $REPO_ROOT/"
            echo "  Size:     $(du -h "$REPO_ROOT/$(basename "$f")" | cut -f1)"
        done
        echo ""
        echo "  Transfer to device and install:"
        echo "    adb push $(basename $DEB_FILE) /data/local/tmp/"
        echo "    termux-setup-storage  # if needed"
        echo "    cp /data/local/tmp/$(basename $DEB_FILE) ~/storage/downloads/"
        echo "    pkg install ~/storage/downloads/$(basename $DEB_FILE)"
    else
        echo ""
        die "Build completed but no .deb found in output/. Check build logs above."
    fi
    exit 0
fi

# ── manual mode (dpkg-deb + package/DEBIAN/ templates) ──────
STAGING_DIR="$(mktemp -d)"
trap 'rm -rf "$STAGING_DIR"' EXIT

PACKAGE_NAME="mikuclub"
BINARY_NAME="MikuClub"
CTL_NAME="mikuctl"
PREFIX="/usr/local"
DEST_DIR="${STAGING_DIR}${PREFIX}"
DEST_BIN="${DEST_DIR}/bin"
WORKSPACE_VERSION="1.0.0"

# Still need to compile (in manual mode)
info "Compiling (release)..."
cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" \
    || die "cargo build failed"

BINARY_SRC="$REPO_ROOT/target/release/gephgui-tui"
[ -f "$BINARY_SRC" ] || die "Binary not found: $BINARY_SRC"

VERSION="$(grep -A1 '^\[workspace.package\]' "$REPO_ROOT/Cargo.toml" \
         | grep 'version' | sed 's/.*version = "\(.*\)".*/\1/' || echo "$WORKSPACE_VERSION")"

ARCH="$(uname -m)"
case "$ARCH" in x86_64|amd64) ARCH="amd64" ;; aarch64|arm64) ARCH="arm64" ;; esac

info "Assembling package: ${PACKAGE_NAME} ${VERSION} (${ARCH})"
mkdir -p "$DEST_BIN"
cp "$BINARY_SRC" "${DEST_BIN}/${BINARY_NAME}"
cp "$REPO_ROOT/mikuctl" "${DEST_BIN}/${CTL_NAME}"
chmod 755 "${DEST_BIN}/${BINARY_NAME}" "${DEST_BIN}/${CTL_NAME}"

mkdir -p "${STAGING_DIR}/DEBIAN"
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
