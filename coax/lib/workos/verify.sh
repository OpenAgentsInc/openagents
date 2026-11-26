#!/usr/bin/env bash
# Firm reason: 5 (thin verification wrapper around coax test, coax mutate, clang ASan/UBSan, and wasm32-wasip1 compile).
# Verifies Phase 1 WorkOS Coax SDK (#14098):
#   1. coax test --library on the combined unit (all 7 #authority modules + composition)
#   2. coax mutate --library --root-only on every Phase 1 #authority module (0 survivors required)
#   3. C11 emission + native ASan/UBSan execution (including 600k-round PBKDF2 vector)
#   4. wasm32-wasip1 compilation check (zero host leaks)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
coax="${COAX:-$repo/coax/bin/coax}"
modules=(pkce url webhook seal jwt error retry)

cd "$repo"

echo "==> [1/4] coax test --library (phase1_unit.coax)"
"$coax" test coax/lib/workos/phase1_unit.coax --library

echo "==> [2/4] coax mutate --library --root-only (0 survivors required)"
for m in "${modules[@]}"; do
  echo "  mutating coax/lib/workos/$m.coax"
  "$coax" mutate "coax/lib/workos/$m.coax" --library --root-only
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "==> [3/4] C11 emission + native ASan/UBSan check (tests/c_check.c)"
"$coax" c coax/lib/workos/phase1_unit.coax --library --header "$work/workos_phase1.h" > "$work/workos_phase1.c"
cc="${CC:-clang}"
"$cc" -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer \
  -I"$work" "$work/workos_phase1.c" coax/lib/workos/tests/c_check.c -o "$work/c_check"
ASAN_OPTIONS="${ASAN_OPTIONS:-detect_leaks=0}" "$work/c_check"

echo "==> [4/4] wasm32-wasip1 freestanding object compile check"
# shellcheck source=../../web/toolchain.sh
source "$repo/coax/web/toolchain.sh"
if [[ -x "${COAXIAL_CLANG:-}" && -d "${COAXIAL_SYSROOT:-}" ]]; then
  "$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" \
    -resource-dir="$COAXIAL_RESOURCES" -std=c11 -Os -c "$work/workos_phase1.c" -o "$work/workos_phase1.o"
  echo "wasm32-wasip1 object compiled cleanly ($(wc -c < "$work/workos_phase1.o" | tr -d ' ') bytes)"
else
  echo "verify.sh: Wasm toolchain not installed at $COAXIAL_CLANG; refusing to skip silently" >&2
  exit 1
fi

echo "==> workos Phase 1 verification PASSED"
