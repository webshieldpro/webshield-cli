#!/bin/sh
# webshield CLI installer: detects OS/architecture, downloads the release archive,
# verifies the checksum and puts the binary into ~/.local/bin.
#
#   curl -fsSL <url>/install.sh | sh              # latest version
#   curl -fsSL <url>/install.sh | sh -s -- 0.1.0  # specific version
#
# Environment variables:
#   WEBSHIELD_CLI_VERSION  — version (or first argument; empty = latest release)
#   WEBSHIELD_CLI_BASE_URL — release base URL (defaults to the repo's GitHub releases)
#   WEBSHIELD_CLI_BINDIR   — install directory (defaults to ~/.local/bin)
set -eu

REPO="webshieldpro/webshield-cli"
BASE="${WEBSHIELD_CLI_BASE_URL:-https://github.com/$REPO/releases/download}"
VERSION="${WEBSHIELD_CLI_VERSION:-${1:-}}"
BINDIR="${WEBSHIELD_CLI_BINDIR:-$HOME/.local/bin}"

# Without an explicit version, take the latest release tag from the GitHub API.
if [ -z "$VERSION" ]; then
  VERSION="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | sed -n 's/.*"tag_name": *"v\{0,1\}\([^"]*\)".*/\1/p' | head -1)"
  if [ -z "$VERSION" ]; then
    echo "failed to determine the latest version; set WEBSHIELD_CLI_VERSION=X.Y.Z" >&2
    exit 1
  fi
fi
TAG="v$VERSION"

os="$(uname -s)"; arch="$(uname -m)"
case "$os" in
  Linux)  plat="unknown-linux-musl" ;;
  Darwin) plat="apple-darwin" ;;
  *) echo "unsupported OS: $os" >&2; exit 1 ;;
esac
case "$arch" in
  x86_64|amd64)  cpu="x86_64" ;;
  aarch64|arm64) cpu="aarch64" ;;
  *) echo "unsupported architecture: $arch" >&2; exit 1 ;;
esac

target="$cpu-$plat"
asset="webshield-$VERSION-$target.tar.gz"
url="$BASE/$TAG/$asset"

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
echo "Downloading $url"
curl -fSL "$url" -o "$tmp/$asset"

# Verify the checksum when SHA256SUMS is available.
if curl -fsSL "$BASE/$TAG/SHA256SUMS" -o "$tmp/SHA256SUMS" 2>/dev/null; then
  ( cd "$tmp" && grep " ./$asset\$\| $asset\$" SHA256SUMS | sed "s| \./| |" | sha256sum -c - ) \
    || { echo "checksum mismatch" >&2; exit 1; }
fi

tar -xzf "$tmp/$asset" -C "$tmp"
mkdir -p "$BINDIR"
install -m 0755 "$tmp/webshield" "$BINDIR/webshield"
echo "Installed: $BINDIR/webshield ($($BINDIR/webshield --version))"
case ":$PATH:" in
  *":$BINDIR:"*) ;;
  *) echo "Add to PATH: export PATH=\"$BINDIR:\$PATH\"" ;;
esac
