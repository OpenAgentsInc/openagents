#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
out=${1:-/tmp/psionic-mesh-public-input}
mkdir -p "$out"
cc=${COAX_CC:-cc}
"$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer \
  -Ipsionic/platform psionic/platform/mesh-public-input.c psionic/tests/mesh21/mesh_public_input.c -lutil -o "$out/mesh-public-input-test"
ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/mesh-public-input-test"
