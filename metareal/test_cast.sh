#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
work=$(mktemp -d "${TMPDIR:-/tmp}/metareal-cast.XXXXXX")
trap 'rm -rf "$work"' EXIT
coax/bin/coax c metareal/cast.coax --library --header "$work/Cast.h" > "$work/Cast.c"
coax/bin/coax c metareal/player.coax --library --header "$work/Player.h" > "$work/Player.c"
"${CC:-cc}" -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer \
  -I "$work" "$work/Cast.c" "$work/Player.c" metareal/cast_smoke.c -lm -o "$work/test"
"$work/test" apps/openagents-ios/OpenAgents/Resources/Cast.bin
