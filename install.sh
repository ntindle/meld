#!/usr/bin/env bash
# Meld installer:  curl -fsSL https://raw.githubusercontent.com/siddhjagani/meld/main/install.sh | bash
# Downloads the latest release binary for your Mac; falls back to building
# from source with cargo if no prebuilt binary matches.
set -euo pipefail

REPO="${MELD_REPO:-siddhjagani/meld}"          # override with MELD_REPO=you/meld
INSTALL_DIR="${MELD_INSTALL_DIR:-/usr/local/bin}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

say()  { printf '\033[1;32m==>\033[0m %s\n' "$*"; }
fail() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || fail "this installer is for macOS — on Windows run: irm https://raw.githubusercontent.com/siddhjagani/meld/main/install.ps1 | iex"

case "$(uname -m)" in
  arm64)  TARGET="aarch64-apple-darwin" ;;
  x86_64) TARGET="x86_64-apple-darwin" ;;
  *)      fail "unsupported architecture: $(uname -m)" ;;
esac

LATEST_URL="https://api.github.com/repos/$REPO/releases/latest"
ASSET_URL="$(curl -fsSL "$LATEST_URL" 2>/dev/null \
  | grep -o "\"browser_download_url\": *\"[^\"]*meld-$TARGET.tar.gz\"" \
  | head -1 | sed 's/.*"\(https[^"]*\)"/\1/' || true)"

if [ -n "${ASSET_URL:-}" ]; then
  say "downloading prebuilt binary ($TARGET)"
  curl -fsSL "$ASSET_URL" -o "$TMP/meld.tar.gz"
  tar -xzf "$TMP/meld.tar.gz" -C "$TMP"
else
  say "no prebuilt binary found — building from source"
  command -v cargo >/dev/null 2>&1 \
    || fail "cargo not found. Install Rust first: https://rustup.rs"
  say "cloning $REPO"
  git clone --depth 1 "https://github.com/$REPO" "$TMP/src"
  (cd "$TMP/src" && cargo build --release)
  cp "$TMP/src/target/release/meld" "$TMP/meld"
fi

say "installing to $INSTALL_DIR/meld"
if [ -w "$INSTALL_DIR" ]; then
  install -m 755 "$TMP/meld" "$INSTALL_DIR/meld"
else
  sudo install -m 755 "$TMP/meld" "$INSTALL_DIR/meld"
fi

say "installed: $("$INSTALL_DIR/meld" --version)"
say "next steps:  meld doctor  →  meld sync --dry-run  →  meld sync"
