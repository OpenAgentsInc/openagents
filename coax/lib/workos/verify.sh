#!/usr/bin/env bash
# Firm reason: 5 (thin verification wrapper around coax test, coax mutate, clang ASan/UBSan, wasm32-wasip1 compile, and oagen-coax drift check).
# Verifies WorkOS Coax SDK Phases 1-2 (#14098, #14099):
#   1. coax test --library on Phase 1, generated endpoints, flow.coax, c_unit.coax, app_unit.coax
#   2. coax mutate --library --root-only on every #authority module (0 survivors required)
#   3. C11 emission + native ASan/UBSan execution (including 600k-round PBKDF2 vector) + adapter C11 compile
#   4. wasm32-wasip1 freestanding compile check (phase1_unit + app_unit, zero host leaks)
#   5. oagen-coax drift check (when tools/oagen-coax/node_modules is present)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
coax="${COAX:-$repo/coax/bin/coax}"
modules=(pkce url webhook seal jwt error retry flow)

cd "$repo"

echo "==> [1/5] coax test --library"
"$coax" test coax/lib/workos/phase1_unit.coax --library
"$coax" test coax/lib/workos/gen/all.coax --library
"$coax" test coax/lib/workos/flow.coax --library
"$coax" test coax/lib/workos/adapters/c_unit.coax --library
"$coax" test coax/lib/workos/adapters/app_unit.coax --library

echo "==> [2/5] coax mutate --library --root-only (0 survivors required)"
for m in "${modules[@]}"; do
  echo "  mutating coax/lib/workos/$m.coax"
  "$coax" mutate "coax/lib/workos/$m.coax" --library --root-only
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "==> [3/5] C11 emission + native ASan/UBSan + adapter C11 compile"
"$coax" c coax/lib/workos/phase1_unit.coax --library --header "$work/workos_phase1.h" > "$work/workos_phase1.c"
"$coax" c coax/lib/workos/adapters/c_unit.coax --library --header "$work/workos_c_unit.h" > "$work/workos_c_unit.c"
"$coax" c coax/lib/workos/adapters/app_unit.coax --library --header "$work/workos_app_unit.h" > "$work/workos_app_unit.c"
cc="${CC:-clang}"
"$cc" -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer \
  -I"$work" "$work/workos_phase1.c" coax/lib/workos/tests/c_check.c -o "$work/c_check"
ASAN_OPTIONS="${ASAN_OPTIONS:-detect_leaks=0}" "$work/c_check"
"$cc" -std=c11 -O2 -Wall -Wno-unused-function -c "$work/workos_c_unit.c" -o "$work/workos_c_unit.o"
"$cc" -std=c11 -O2 -Wall -Wno-unused-function -c "$work/workos_app_unit.c" -o "$work/workos_app_unit.o"
echo "C11 adapters compiled cleanly (c_unit=$(wc -c < "$work/workos_c_unit.o" | tr -d ' ')B, app_unit=$(wc -c < "$work/workos_app_unit.o" | tr -d ' ')B)"

echo "==> [4/5] wasm32-wasip1 freestanding object compile check"
# shellcheck source=../../web/toolchain.sh
source "$repo/coax/web/toolchain.sh"
if [[ -x "${COAXIAL_CLANG:-}" && -d "${COAXIAL_SYSROOT:-}" ]]; then
  "$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" \
    -resource-dir="$COAXIAL_RESOURCES" -std=c11 -Os -c "$work/workos_phase1.c" -o "$work/workos_phase1.o"
  "$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" \
    -resource-dir="$COAXIAL_RESOURCES" -std=c11 -Os -c "$work/workos_app_unit.c" -o "$work/workos_app_unit.o"
  echo "wasm32-wasip1 objects compiled cleanly (phase1=$(wc -c < "$work/workos_phase1.o" | tr -d ' ')B, app_unit=$(wc -c < "$work/workos_app_unit.o" | tr -d ' ')B)"
else
  echo "verify.sh: Wasm toolchain not installed at $COAXIAL_CLANG; refusing to skip silently" >&2
  exit 1
fi

echo "==> [5/5] oagen-coax drift check"
if [[ -d "$repo/tools/oagen-coax/node_modules" ]] && command -v bun >/dev/null 2>&1; then
  (cd "$repo/tools/oagen-coax" && bun run typecheck && bun run check)
else
  echo "  note: tools/oagen-coax/node_modules not installed; skipping oagen-coax --check (run 'cd tools/oagen-coax && bun install && bun run check')"
fi

echo "==> workos SDK verification PASSED"
