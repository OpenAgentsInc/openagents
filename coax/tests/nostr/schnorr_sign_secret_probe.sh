#!/usr/bin/env bash
# ctgrind-style secret-flow map of the compiled signer: the release generated
# C of event_sign.coax (which loads schnorr_sign.coax), the key and aux
# undefined under memcheck. Prints each distinct function and source line
# where a key-derived value reaches a branch or an address. It is a map for
# the #4828 declassification work, not a gate: it exits 0 and reports counts.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../.." && pwd)";cd "$repo"
vg="$(nix build --no-link --print-out-paths nixpkgs#valgrind.out)"
vgdev="$(nix build --no-link --print-out-paths nixpkgs#valgrind.dev)"
d="$(mktemp -d)";trap 'rm -rf "$d"' EXIT
coax/bin/coax c coax/lib/nostr/event_sign.coax --library --header "$d/es.h" --run-steps 250000000 --release > "$d/release.c"
"${CC:-gcc}" -std=c11 -O1 -g -fno-inline -I"$d" -I"$vgdev/include" "$d/release.c" coax/tests/nostr/schnorr_sign_secret_probe.c -o "$d/probe"
"$vg/bin/valgrind" --tool=memcheck --track-origins=no -q --num-callers=4 "$d/probe" 2>"$d/err" || true
echo "reports: $(grep -c 'depends on uninitialised\|Use of uninitialised' "$d/err" || true)"
# The first frame of each report: the function that branched or indexed.
grep -A1 'depends on uninitialised\|Use of uninitialised' "$d/err" | grep ' at \| by ' | sed -E 's/.*: (coax_p_)?([A-Za-z0-9_.]+) \(release\.c:([0-9]+)\)/\2/' | sed 's/\.constprop.*//;s/\.isra.*//' | sort | uniq -c | sort -rn
