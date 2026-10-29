#!/usr/bin/env bash
# Ice Cream Man's rules: the Coax checks, then every recorded Rust trace
# replayed natively, step for step, under sanitizers.
#   verse/icecream/test.sh [TRACE_DIR]
# TRACE_DIR defaults to ~/work/aethyr-demo/tests/fixtures/icecream (all
# thirteen Rust traces) when it exists; the checked-in fixtures/*.trace.gz
# always run.
set -euo pipefail
cd "$(dirname "$0")/../.."
work=$(mktemp -d "${TMPDIR:-/tmp}/verse-icecream.XXXXXX")
trap 'rm -rf "$work"' EXIT
coax/bin/coax test verse/icecream/rules.coax --library
coax/bin/coax test verse/icecream/trace.coax --library --steps 2000000000
coax/bin/coax c verse/icecream/rules.coax --library --header "$work/IceCream.h" > "$work/IceCream.c"
"${CC:-/usr/bin/clang}" -std=c11 -O1 -g -ffp-contract=off -fsanitize=address,undefined -fno-omit-frame-pointer \
  -Wno-unused-value -I "$work" "$work/IceCream.c" verse/icecream/replay.c -lm -o "$work/replay"
mkdir -p "$work/traces"
for f in verse/icecream/fixtures/*.trace.gz; do gunzip -c "$f" > "$work/traces/$(basename "$f" .gz)"; done
"$work/replay" "$work/traces"/*.trace
dir="${1:-$HOME/work/aethyr-demo/tests/fixtures/icecream}"
if [[ -d "$dir" ]]; then "$work/replay" "$dir"/*.trace; fi
"$work/replay" --checks
