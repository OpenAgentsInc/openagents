#!/usr/bin/env bash
# Build chapter 115's load generator: OUT [debug|release] [plain|sanitized].
# Coax emits one C library from psionic/lib/bench-wire.coax (which loads
# bench.coax); psionic/platform/bench-drive.c links it.
set -euo pipefail
cd "$(dirname "$0")/../.."
out=${1:?usage: psionic/platform/bench-build.sh OUT [debug|release] [plain|sanitized]} mode=${2:-release} native=${3:-plain}
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mkdir -p "$out"
flags=() opt=-O0 sanitizers=()
if [[ $mode == release ]]; then flags+=(--release); opt=-O2; elif [[ $mode != debug ]]; then exit 2; fi
if [[ $native == sanitized ]]; then sanitizers=(-fsanitize=address,undefined -fno-omit-frame-pointer); elif [[ $native != plain ]]; then exit 2; fi
"$coax" c psionic/lib/bench-wire.coax --library --header "$out/bench.h" "${flags[@]+"${flags[@]}"}" > "$out/bench.c"
"$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter \
    -Wno-unknown-pragmas "${sanitizers[@]+"${sanitizers[@]}"}" -I"$out" -Ipsionic/platform \
    "$out/bench.c" psionic/platform/bench-drive.c -lpthread -lm -o "$out/bench-drive"
