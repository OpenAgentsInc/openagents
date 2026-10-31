#!/usr/bin/env bash
# The browser's math natively (docs/verse/native-gpu.md): wasi-libc's libm is musl's, and glibc's
# sinf, cosf, powf ... differ from it in the last bit, which a realm's simulation compounds (the
# villa's walkers, a few hundredths off after ten seconds). This takes musl's transcendental
# functions and their helpers out of the host's musl libc.a into DIR/musl-math.o's objects and
# prints their paths, for a native link to put ahead of -lm, so a native realm computes what its
# wasm computes. Exact functions (sqrt, floor, fabs, fmod) are the same everywhere and stay glibc's.
#   verse/native/musl-math.sh DIR     on a Linux host with nix (coderos-4080)
# Prints nothing (and the link keeps glibc's) where there is no musl.
set -euo pipefail
dir="${1:?usage: musl-math.sh DIR}"
[[ "$(uname)" == Linux ]] || exit 0
musl="$(nix-build '<nixpkgs>' -A musl --no-out-link 2>/dev/null | head -1 || true)"
[[ -n "$musl" && -f "$musl/lib/libc.a" ]] || { echo "musl-math.sh: no musl; glibc's math" >&2; exit 0; }
out="$dir/musl-math"
if [[ ! -f "$out/done" ]]; then
  rm -rf "$out"; mkdir -p "$out"
  names=()
  for f in sin cos tan asin acos atan atan2 sinh cosh tanh exp exp2 expm1 log log2 log10 log1p pow cbrt hypot; do
    names+=("$f" "${f}f")
  done
  want="$(printf '%s\n' "${names[@]}" | sed 's/$/.lo/')"
  members="$(ar t "$musl/lib/libc.a" | grep -E '^(__sin|__cos|__tan|__sindf|__cosdf|__tandf|__rem_pio2|__rem_pio2f|__rem_pio2_large|__expo2|__expo2f|__math_[a-z]+|[a-z0-9]+_data|__log1pf?)\.lo$' || true)"
  # shellcheck disable=SC2086
  (cd "$out" && ar x "$musl/lib/libc.a" $(printf '%s\n' "$want" $members | sort -u | grep -Fx -f <(ar t "$musl/lib/libc.a")))
  touch "$out/done"
fi
ls "$out"/*.lo | tr '\n' ' '
