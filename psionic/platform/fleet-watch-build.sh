#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
out=${1:?usage: fleet-watch-build.sh OUT [sanitized]}
mkdir -p "$out"
coax=${COAX:-$root/coax/bin/coax};cc=${COAX_CC:-cc}
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math)
if [[ ${2:-} == sanitized ]];then flags+=(-g -fsanitize=address,undefined -fno-omit-frame-pointer);fi
"$coax" c "$root/psionic/lib/ops/fleet-watch.coax" --library --header "$out/fleet-watch.h" > "$out/fleet-watch.c"
"$coax" c "$root/psionic/lib/ops/fleet-watch-probe.coax" --library --run-steps 250000000 --header "$out/fleet-watch-probe.h" > "$out/fleet-watch-probe.c"
"$cc" "${flags[@]}" -I"$out" "$out/fleet-watch.c" "$root/psionic/platform/fleet-watch-face.c" -lm -o "$out/fleet-watch"
"$cc" "${flags[@]}" -I"$out" "$out/fleet-watch-probe.c" "$root/psionic/platform/fleet-watch-probe-face.c" -lm -o "$out/fleet-watch-probe"
