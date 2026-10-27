#!/usr/bin/env bash
# Prepares a wgpu-native install for af-gpu's `native` feature:
#
#   <out>/include/webgpu.h   standard webgpu.h that af-gpu binds against
#   <out>/include/wgpu.h     wgpu-native extensions (device polling, instance flags)
#   <out>/lib/               libwgpu_native.{so,dylib} (the static library is skipped)
#
# By default it downloads the pinned wgpu-native release for this host and checks
# its SHA-256. With --source it builds a wgpu-native checkout instead (needs
# cargo and libclang) and copies the same files out of it.
#
# Usage:
#   tools/build-webgpu-native.sh [--out DIR] [--version vX.Y.Z.W] [--source CHECKOUT]
#
# Then build from crates/aethyr with:
#   AF_WEBGPU_DIR=DIR AF_WEBGPU_IMPL=wgpu-native cargo build -p af-render-lab --features native --release
set -euo pipefail

# The web package is apps/aethyr; keep the install outside the openagents checkout.
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
monorepo="$(cd "$repo/../.." && pwd -P)"
version="v29.0.1.1"
out="$(dirname "$monorepo")/webgpu-native"
source_dir=""

while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift 2 ;;
    --version) version="$2"; shift 2 ;;
    --source) source_dir="$2"; shift 2 ;;
    -h|--help) sed -n '2,19p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

mkdir -p "$out/include" "$out/lib"

if [ -n "$source_dir" ]; then
  source_dir="$(cd "$source_dir" && pwd -P)"
  target="${CARGO_TARGET_DIR:-$source_dir/target}"
  (cd "$source_dir" && git submodule update --init --depth 1 ffi/webgpu-headers 2>/dev/null || true)
  (cd "$source_dir" && CARGO_TARGET_DIR="$target" cargo build --release --locked)
  cp "$source_dir/ffi/webgpu-headers/webgpu.h" "$source_dir/ffi/wgpu.h" "$out/include/"
  for lib in libwgpu_native.so libwgpu_native.dylib wgpu_native.dll; do
    [ -f "$target/release/$lib" ] && cp "$target/release/$lib" "$out/lib/"
  done
  (cd "$source_dir" && git describe --tags --always 2>/dev/null || echo unknown) > "$out/lib/wgpu-native-git-tag"
else
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) platform="linux-x86_64" ;;
    Linux-aarch64|Linux-arm64) platform="linux-aarch64" ;;
    Darwin-arm64) platform="macos-aarch64" ;;
    Darwin-x86_64) platform="macos-x86_64" ;;
    *) echo "no prebuilt wgpu-native for $(uname -s)-$(uname -m); use --source" >&2; exit 1 ;;
  esac
  zip="wgpu-$platform-release.zip"
  # Checksums of the pinned release's archives (computed when pinning).
  expected=""
  if [ "$version" = "v29.0.1.1" ]; then
    case "$platform" in
      linux-x86_64) expected="95a4d90c071005a98d03eab348beaa6b07e16eb00d1dcdb9f8348f75eb97ec5a" ;;
      linux-aarch64) expected="015fcdf1dbae82e614a783cc38017e5399ae0927a889fe9b69c9b664bc61b47a" ;;
      macos-aarch64) expected="a5797a37b1adf720bcd5dcffb291edbbd5b7b14be0a3874c28e6393a655a7a3e" ;;
      macos-x86_64) expected="8e2f7378548ddd0e2cf21e7d864dda46e953f0af724855a33778b85ead206d41" ;;
    esac
  fi
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  url="https://github.com/gfx-rs/wgpu-native/releases/download/$version/$zip"
  echo "Downloading $url" >&2
  curl -fsSL -o "$tmp/$zip" "$url"
  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$tmp/$zip" | cut -d' ' -f1)"
  else
    actual="$(shasum -a 256 "$tmp/$zip" | cut -d' ' -f1)"
  fi
  if [ -n "$expected" ] && [ "$actual" != "$expected" ]; then
    echo "checksum mismatch for $zip: $actual (expected $expected)" >&2
    exit 1
  fi
  [ -z "$expected" ] && echo "warning: no pinned checksum for $version $platform ($actual)" >&2
  # Headers ship under include/webgpu/; af-gpu's build.rs expects them flat.
  if command -v unzip >/dev/null 2>&1; then
    unzip -o -j -q "$tmp/$zip" 'include/webgpu/*' -d "$out/include"
    unzip -o -j -q "$tmp/$zip" 'lib/libwgpu_native.so' 'lib/libwgpu_native.dylib' 'wgpu-native-meta/wgpu-native-git-tag' -d "$out/lib" 2>/dev/null || true
  else
    # Hosts without unzip (NixOS by default): the same extraction in Python.
    python3 -I - "$tmp/$zip" "$out" <<'PY'
import os, sys, zipfile
archive, out = sys.argv[1], sys.argv[2]
wanted = {"lib/libwgpu_native.so": "lib", "lib/libwgpu_native.dylib": "lib", "wgpu-native-meta/wgpu-native-git-tag": "lib"}
with zipfile.ZipFile(archive) as z:
    for name in z.namelist():
        dest = "include" if name.startswith("include/webgpu/") and not name.endswith("/") else wanted.get(name)
        if dest:
            with open(os.path.join(out, dest, os.path.basename(name)), "wb") as f:
                f.write(z.read(name))
PY
  fi
  case "$platform" in
    # Let binaries find the dylib through the rpath af-gpu's build.rs adds.
    macos-*) install_name_tool -id "@rpath/libwgpu_native.dylib" "$out/lib/libwgpu_native.dylib" 2>/dev/null || true ;;
  esac
fi

for f in include/webgpu.h include/wgpu.h; do
  [ -f "$out/$f" ] || { echo "missing $out/$f" >&2; exit 1; }
done
ls "$out"/lib/libwgpu_native.* >/dev/null 2>&1 || { echo "missing library in $out/lib" >&2; exit 1; }
echo "wgpu-native $(cat "$out/lib/wgpu-native-git-tag" 2>/dev/null || echo "$version") in $out" >&2
echo "export AF_WEBGPU_DIR=$out AF_WEBGPU_IMPL=wgpu-native"
