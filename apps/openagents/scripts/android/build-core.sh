#!/bin/bash
# Build the Rust mobile core (crates/mobile) for Android and generate its
# Kotlin bindings — the same library the iOS app links.
#
#   CARGO_TARGET_DIR=/path/to/shared/app-target \
#     scripts/android/build-core.sh [out_dir]
#
# The app's pinned Rust toolchain, Android NDK, and cargo-ndk must be available.
# Run from the pinned Android shell documented in os/README.md.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-}"
if [[ -z "$CARGO_TARGET_DIR" ]]; then
  echo "error: set CARGO_TARGET_DIR to the shared app target directory" >&2
  exit 1
fi
mkdir -p "$CARGO_TARGET_DIR"
CARGO_TARGET_DIR="$(cd "$CARGO_TARGET_DIR" && pwd -P)"
case "$CARGO_TARGET_DIR/" in
  "$ROOT/"*)
    echo "error: CARGO_TARGET_DIR must be outside the apps/openagents checkout" >&2
    exit 1
    ;;
esac
export CARGO_TARGET_DIR
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.98.1}"
export CARGO_INCREMENTAL=0

OUT="${1:-$CARGO_TARGET_DIR/android-core}"
mkdir -p "$OUT/kotlin" "$OUT/jniLibs"
OUT="$(cd "$OUT" && pwd -P)"
cd "$ROOT"
. "$ROOT/../../scripts/dev/cargo-build-env.sh"

if [[ -z "${ANDROID_NDK_HOME:-}" ]] || ! command -v cargo-ndk >/dev/null; then
  echo "error: enter the pinned Android shell from os/README.md" >&2
  exit 1
fi

# The Android shell exports the NDK compiler as generic CC. Build scripts for
# host tools (for example, libz-sys) need the host compiler; cargo-ndk supplies
# target-specific compiler variables for the Android library builds below.
unset CC CXX AR

cargo build --locked -p openagents-mobile --lib --bin uniffi-bindgen \
  --features bindgen --profile mobile
HOST_LIB="$CARGO_TARGET_DIR/mobile/libopenagents_mobile.$([[ "$(uname)" == Darwin ]] && echo dylib || echo so)"
"$CARGO_TARGET_DIR/mobile/uniffi-bindgen" generate --library "$HOST_LIB" --language kotlin \
  --no-format --out-dir "$OUT/kotlin"
echo "kotlin bindings: $OUT/kotlin"
APP_BINDING="$ROOT/apps/android/app/src/main/java/uniffi/openagents_core/openagents_core.kt"
SYNC_BINDING="$OUT/kotlin/openagents_core.kt"
mkdir -p "$(dirname "$APP_BINDING")"
python3 -c 'import pathlib, sys; source = pathlib.Path(sys.argv[1]).read_text().rstrip(); pathlib.Path(sys.argv[2]).write_text("".join(line.rstrip() + "\n" for line in source.splitlines()))' \
  "$OUT/kotlin/uniffi/openagents_core/openagents_core.kt" "$SYNC_BINDING"
if ! cmp -s "$SYNC_BINDING" "$APP_BINDING"; then
  cp "$SYNC_BINDING" "$APP_BINDING"
fi
echo "Android source binding: $APP_BINDING"

cargo ndk -t arm64-v8a -o "$OUT/jniLibs" \
  build --locked -p openagents-mobile --lib --profile mobile
cargo ndk -t x86_64 -o "$OUT/jniLibs" \
  build --locked -p openagents-mobile --lib --profile mobile
echo "jniLibs: $OUT/jniLibs"
