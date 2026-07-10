#!/usr/bin/env bash
# Cross-builds webshield release binaries and packages the artifacts into dist/.
#
# Cross-compilation — via cargo-zigbuild (zig as the C compiler): one tool for all
# targets, no per-target toolchains and no Docker. Setup:
#   cargo install cargo-zigbuild && pip install ziglang   # or a system zig
#
# Exception — Apple targets: apple-darwin↔apple-darwin builds go through the plain
# Apple/host toolchain (Apple clang cross-compiles between darwin arches natively),
# so on a macOS host both darwin targets build with the SDK already present and no
# zig required. This is how the release CI builds macOS on a macos-latest runner.
#
# Without zig the script builds only the native target (skipping the rest with a
# warning), so a run always produces something. Targets can be passed as arguments.
set -uo pipefail
cd "$(dirname "$0")"

BIN=webshield
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
OUT=dist

DEFAULT_TARGETS=(
  x86_64-unknown-linux-musl
  aarch64-unknown-linux-musl
  x86_64-pc-windows-gnu
  x86_64-apple-darwin
  aarch64-apple-darwin
)
HOST="$(rustc -vV | sed -n 's/host: //p')"

# Are cargo-zigbuild and zig itself available (binary or the ziglang python module)?
HAVE_ZIG=0
if command -v cargo-zigbuild >/dev/null 2>&1 \
   && { command -v zig >/dev/null 2>&1 || python3 -c 'import ziglang' 2>/dev/null; }; then
  HAVE_ZIG=1
fi

# Target selection: arguments > full matrix (when zig is present) > native target only.
if [ "$#" -gt 0 ]; then
  TARGETS=("$@")
elif [ "$HAVE_ZIG" = 1 ]; then
  TARGETS=("${DEFAULT_TARGETS[@]}")
else
  echo "ⓘ cargo-zigbuild/zig not found — building only the native target $HOST (see the script header)."
  TARGETS=("$HOST")
fi

rm -rf "$OUT"; mkdir -p "$OUT"

# Completions are generated once by the native binary and put into every archive.
echo ">> native build (to generate completions)"
cargo build --release >/dev/null || { echo "native build failed"; exit 1; }
COMP="$(mktemp -d)"
for sh in bash zsh fish powershell; do
  "./target/release/$BIN" completion "$sh" > "$COMP/$BIN.$sh" 2>/dev/null || true
done

sha256() { command -v sha256sum >/dev/null 2>&1 && sha256sum "$@" || shasum -a 256 "$@"; }

# Can this target be built with the plain host toolchain (no zig)? True for the
# exact host, and for a darwin target while on a darwin host (Apple clang handles
# apple-darwin↔apple-darwin cross-compilation itself).
native_buildable() {
  [ "$1" = "$HOST" ] && return 0
  case "$HOST:$1" in
    *-apple-darwin:*-apple-darwin) return 0 ;;
  esac
  return 1
}

built=0
for target in "${TARGETS[@]}"; do
  if ! native_buildable "$target" && [ "$HAVE_ZIG" != 1 ]; then
    echo "skipping $target: cargo-zigbuild required"
    continue
  fi

  rustup target add "$target" >/dev/null 2>&1 || true
  echo ">> building $target"
  if native_buildable "$target"; then
    cargo build --release --target "$target"
  else
    cargo zigbuild --release --target "$target"
  fi || { echo "⚠ $target: build failed, skipping"; continue; }

  # Path to the binary (+ .exe on windows).
  suffix=""; case "$target" in *windows*) suffix=".exe" ;; esac
  src="target/$target/release/$BIN$suffix"
  [ -f "$src" ] || { echo "⚠ $target: binary not found ($src)"; continue; }

  # Flat packaging: binary + completions + README at the archive root.
  stage="$(mktemp -d)"
  cp "$src" "$stage/$BIN$suffix"
  cp "$COMP"/* "$stage/" 2>/dev/null || true
  cp README.md "$stage/" 2>/dev/null || true

  name="$BIN-$VERSION-$target"
  case "$target" in
    *windows*) ( cd "$stage" && zip -q -r "-" . ) > "$OUT/$name.zip"; art="$name.zip" ;;
    *)         tar -czf "$OUT/$name.tar.gz" -C "$stage" .;              art="$name.tar.gz" ;;
  esac
  rm -rf "$stage"
  echo "   → $OUT/$art"
  built=$((built+1))
done

if [ "$built" = 0 ]; then echo "nothing was built"; exit 1; fi

# Checksums of all artifacts.
( cd "$OUT" && sha256 ./* > SHA256SUMS )
echo
echo "Done: $built artifact(s) built. See $OUT/ (+ SHA256SUMS)."
