#!/bin/bash
# Build the Rust mobile core (crates/mobile) for the active Xcode platform and
# generate its Swift bindings. Run by the Zeron target's "Rust core" build
# phase; also runnable by hand (defaults to the simulator, dev profile).
#
#   scripts/ios/build-core.sh [iphonesimulator|iphoneos]
#
# Outputs (target/ios-core/<platform>/):
#   libzeron_mobile.a            linked via LIBRARY_SEARCH_PATHS
#   include/module.modulemap     `import zeron_coreFFI` (SWIFT_INCLUDE_PATHS)
# and refreshes apps/ios/Zeron/Core/Generated/zeron_core.swift — committed so
# Xcode's synchronized folder always sees it; CI fails if it drifts.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PLATFORM="${1:-${PLATFORM_NAME:-iphonesimulator}}"
case "$PLATFORM" in
  iphonesimulator) TARGET=aarch64-apple-ios-sim ;;
  iphoneos) TARGET=aarch64-apple-ios ;;
  *) echo "error: unsupported platform $PLATFORM" >&2; exit 1 ;;
esac
PROFILE=mobile
if [[ "${CONFIGURATION:-Debug}" == "Release" ]]; then PROFILE=mobile-dist; fi

CARGO_OUT="${CARGO_TARGET_DIR:-$ROOT/target}"
OUT="$ROOT/target/ios-core/$PLATFORM"
mkdir -p "$OUT/include"

CORE_FEATURES=()
if [[ "${ZERON_ACCOUNT_FIXTURE:-}" == "1" ]]; then
  if [[ "${CONFIGURATION:-Debug}" == "Release" || "$PLATFORM" != "iphonesimulator" ]]; then
    echo "error: account fixture is only available in a debug simulator build" >&2
    exit 1
  fi
  CORE_FEATURES=(--features account-fixture)
fi

# Iterating on Swift while the core is mid-edit: reuse the last good build.
if [[ "${ZERON_SKIP_CORE:-}" == "1" && -f "$OUT/libzeron_mobile.a" ]]; then
  expected=${ZERON_ACCOUNT_FIXTURE:-0}
  staged=$(cat "$OUT/account-fixture" 2>/dev/null || echo 0)
  if [[ "$expected" != "$staged" ]]; then
    echo "error: rebuild the Rust core after changing the account fixture setting" >&2
    exit 1
  fi
  echo "note: ZERON_SKIP_CORE=1 — reusing $OUT/libzeron_mobile.a"
  exit 0
fi

# Xcode exports SDKROOT/deployment vars for the *app* SDK; host build scripts
# (proc macros, build.rs) must not see them, so cargo runs in a clean env.
run_cargo() {
  env -i HOME="$HOME" USER="${USER:-}" TERM="${TERM:-dumb}" \
    PATH="$HOME/.cargo/bin:/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin" \
    IPHONEOS_DEPLOYMENT_TARGET=26.0 \
    RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.98.1}" \
    CARGO_TARGET_DIR="$CARGO_OUT" \
    cargo "$@"
}

cd "$ROOT"
run_cargo build --locked -p zeron-mobile --lib --profile "$PROFILE" --target "$TARGET" ${CORE_FEATURES[@]+"${CORE_FEATURES[@]}"}
run_cargo build --locked -p zeron-mobile --bin uniffi-bindgen --features bindgen --profile mobile

LIB="$CARGO_OUT/$TARGET/$PROFILE/libzeron_mobile.a"
cp -p "$LIB" "$OUT/libzeron_mobile.a"

GEN="$OUT/gen"
"$CARGO_OUT/mobile/uniffi-bindgen" generate --library "$LIB" --language swift --out-dir "$GEN" >/dev/null
cp "$GEN/zeron_coreFFI.h" "$OUT/include/zeron_coreFFI.h"
cp "$GEN/zeron_coreFFI.modulemap" "$OUT/include/module.modulemap"
# Only touch the Swift file when it changed so Xcode doesn't recompile it.
SWIFT_OUT="$ROOT/apps/ios/Zeron/Core/Generated/zeron_core.swift"
mkdir -p "$(dirname "$SWIFT_OUT")"
cmp -s "$GEN/zeron_core.swift" "$SWIFT_OUT" || cp "$GEN/zeron_core.swift" "$SWIFT_OUT"

printf "%s\n" "${ZERON_ACCOUNT_FIXTURE:-0}" > "$OUT/account-fixture"
