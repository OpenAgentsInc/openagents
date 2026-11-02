#!/usr/bin/env bash
# Build the Coax residual CNN consumer; C supplies file, clock and logging effects.
set -euo pipefail
cd "$(dirname "$0")/../.."
out=${1:?usage: residual-build.sh OUT debug|release 0|1}
mode=${2:?missing debug or release}
sanitize=${3:-0}
[[ $mode == debug || $mode == release ]] || exit 2
[[ $sanitize == 0 || $sanitize == 1 ]] || exit 2
mkdir -p "$out"
out=$(cd "$out" && pwd)
compiler=${COAX:-$PWD/coax/bin/coax}
[[ -x $compiler ]] || { echo 'Build the compiler first: coax/build.sh' >&2; exit 2; }
flags=()
opt=-O0
if [[ $mode == release ]]; then flags+=(--release); opt=-O2; fi
"$compiler" c psionic/lib/residual-mlp-experiment.coax --library "${flags[@]}" --header "$out/residual.h" > "$out/residual.c"
checks=()
if [[ $sanitize == 1 ]]; then checks+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
"${COAX_CC:-cc}" -std=c11 "$opt" -g -ffp-contract=off "${checks[@]}" -I"$out" \
    "$out/residual.c" psionic/platform/residual-mlp-host.c -lm -o "$out/residual"
