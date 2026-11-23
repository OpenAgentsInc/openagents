#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax metal-compute and clang; holds no logic)
# Builds the native Psionic Metal runtime for Inkling 2 Mini:
#   - inkling-kernels.metal (emitted from psionic/lib/inkling/kernels.coax via coax metal-compute)
#   - psionic-inkling-serve (local HTTP /v1/chat/completions & /v1/responses server on Metal)
#   - psionic-inkling-cli   (standalone CLI runner on Metal)
# Usage: psionic/platform/inkling-build.sh [OUT_DIR]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
if [[ ! -x "$coax" ]]; then
  main_wt="$(git -C "$repo" worktree list --porcelain 2>/dev/null | awk '/^worktree /{print substr($0, 10); exit}')"
  if [[ -n "$main_wt" && -x "$main_wt/coax/bin/coax" ]]; then
    coax="$main_wt/coax/bin/coax"
  else
    cached="$(ls -t "$HOME/.cache/coax-release"/*/coax 2>/dev/null | head -n 1 || true)"
    [[ -n "$cached" && -x "$cached" ]] && coax="$cached"
  fi
fi
[[ -x "$coax" ]] || { echo "inkling-build.sh: coax not found at $coax (run coax/build.sh or set COAX)" >&2; exit 1; }
out="${1:-$repo/psionic/build}"
mkdir -p "$out/obj"

pids=()
"$coax" metal-compute "$repo/psionic/lib/inkling/kernels.coax" --release > "$out/inkling-kernels.metal" & pids+=($!)
"$coax" c "$repo/psionic/lib/inkling/json-schema.coax" --release --library --header "$out/obj/json-schema.h" > "$out/obj/json-schema.c" & pids+=($!)
for p in "${pids[@]}"; do wait "$p"; done

cflags=(-mmacosx-version-min=14.0 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -I"$here" -I"$out/obj")
pids=()
/usr/bin/clang -fobjc-arc -mmacosx-version-min=14.0 -O2 -I"$here" -c "$here/metal-host.m" -o "$out/obj/metal-host.o" & pids+=($!)
/usr/bin/clang -std=c11 "${cflags[@]}" -c "$here/inkling-gguf.c" -o "$out/obj/inkling-gguf.o" & pids+=($!)
/usr/bin/clang -std=c11 "${cflags[@]}" -c "$here/inkling-metal.c" -o "$out/obj/inkling-metal.o" & pids+=($!)
/usr/bin/clang -std=c11 -mmacosx-version-min=14.0 -O2 -Wno-unused-function -Wno-unused-variable -c "$out/obj/json-schema.c" -o "$out/obj/json-schema.o" & pids+=($!)
/usr/bin/clang -std=c11 "${cflags[@]}" -c "$here/inkling-serve.c" -o "$out/obj/inkling-serve.o" & pids+=($!)
/usr/bin/clang -std=c11 "${cflags[@]}" -c "$here/inkling-cli.c" -o "$out/obj/inkling-cli.o" & pids+=($!)
for p in "${pids[@]}"; do wait "$p"; done

pids=()
/usr/bin/clang -mmacosx-version-min=14.0 -O2 \
  "$out/obj/metal-host.o" "$out/obj/inkling-gguf.o" "$out/obj/inkling-metal.o" "$out/obj/json-schema.o" "$out/obj/inkling-serve.o" \
  -framework Metal -framework Foundation -lm -lpthread -o "$out/psionic-inkling-serve" & pids+=($!)
/usr/bin/clang -mmacosx-version-min=14.0 -O2 \
  "$out/obj/metal-host.o" "$out/obj/inkling-gguf.o" "$out/obj/inkling-metal.o" "$out/obj/inkling-cli.o" \
  -framework Metal -framework Foundation -lm -o "$out/psionic-inkling-cli" & pids+=($!)
for p in "${pids[@]}"; do wait "$p"; done

echo "built $out/psionic-inkling-serve, $out/psionic-inkling-cli, and $out/inkling-kernels.metal"
