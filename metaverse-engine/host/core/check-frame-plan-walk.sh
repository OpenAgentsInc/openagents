#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: builds the C check with the host compiler and feeds it Coax's fixture)
# The frame plan's walk for C hosts (frame-plan-walk.h, #13303) against what Coax says a walk calls.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
out="${1:-$repo/coax/web/build/frame-plan-walk}"; mkdir -p "$out"
if [[ "$(uname -s)" == Darwin ]]; then cc=(/usr/bin/clang); else cc=("${CC:-gcc}"); fi
"${cc[@]}" -std=c11 -O1 -Wall -Wextra -Werror -o "$out/check" "$here/frame-plan-walk-check.c"
"$repo/coax/web/tool.sh" "$repo/metaverse-engine/tools/frame-plan-fixture.coax" > "$out/fixture.txt"
"$out/check" < "$out/fixture.txt"
