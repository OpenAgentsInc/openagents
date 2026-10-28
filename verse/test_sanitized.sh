#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
work=$(mktemp -d "${TMPDIR:-/tmp}/verse-sanitized.XXXXXX")
trap 'rm -rf "$work"' EXIT
coax/bin/coax c verse/home.coax --library --header "$work/Home.h" > "$work/Home.c"
"${CC:-cc}" -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer \
  -I "$work" -I verse/platform "$work/Home.c" verse/platform/host.c \
  verse/sanitized.c -lm -o "$work/test"
"$work/test" apps/openagents-ios/OpenAgents/Resources/Home.pb
