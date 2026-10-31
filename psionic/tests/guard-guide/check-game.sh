#!/usr/bin/env bash
# Firm reason: thin compiler wrapper (5); C is an independent oracle (2).
set -euo pipefail
mkdir -p "$1"
"${COAX:-coax/bin/coax}" c verse/everglade/guard-guide-game.coax --library --header "$1/guide-game.h" > "$1/game-debug.c"
"${CC:-cc}" -std=c11 -O0 -g -I"$1" "$1/game-debug.c" psionic/tests/guard-guide/game-native.c -lm -o "$1/debug"
"$1/debug"
"${COAX:-coax/bin/coax}" c verse/everglade/guard-guide-game.coax --library --release --header "$1/guide-game.h" > "$1/game-release.c"
"${CC:-cc}" -std=c11 -O2 -I"$1" "$1/game-release.c" psionic/tests/guard-guide/game-native.c -lm -o "$1/release"
"$1/release"
"${CC:-cc}" -std=c11 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$1" "$1/game-debug.c" psionic/tests/guard-guide/game-native.c -lm -o "$1/sanitized"
"$1/sanitized"
