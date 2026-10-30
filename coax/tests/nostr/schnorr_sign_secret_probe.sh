#!/usr/bin/env bash
# ctgrind-style secret-flow map of the compiled signer: the release generated
# C of event_sign.coax (which loads schnorr_sign.coax), the key and aux
# undefined under memcheck. Prints each distinct function and source line
# where a key-derived value reaches a branch or an address. Passes only if
# every report is at one of the signer's declared decisions inside a call
# (sw_key: key validity, sm_equation: nonce and equation validity,
# sw_publish: publication), which the C driver can't declassify mid-call.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../.." && pwd)";cd "$repo"
vg="$(nix build --no-link --print-out-paths nixpkgs#valgrind.out)"
vgdev="$(nix build --no-link --print-out-paths nixpkgs#valgrind.dev)"
d="$(mktemp -d)";trap 'rm -rf "$d"' EXIT
coax/bin/coax c coax/lib/nostr/event_sign.coax --library --header "$d/es.h" --run-steps 250000000 --release > "$d/release.c"
"${CC:-gcc}" -std=c11 -O2 -g -I"$d" -I"$vgdev/include" "$d/release.c" coax/tests/nostr/schnorr_sign_secret_probe.c -o "$d/probe"
"$vg/bin/valgrind" --tool=memcheck --track-origins=no -q --num-callers=4 "$d/probe" 2>"$d/err" || true
echo "reports: $(grep -c 'depends on uninitialised\|Use of uninitialised' "$d/err" || true)"
# The first frame of each report: the function that branched or indexed.
sites="$(grep -A1 'depends on uninitialised\|Use of uninitialised' "$d/err" | grep ' at \| by ' | sed -E 's/.*: (coax_p_)?([A-Za-z0-9_.]+) \(release\.c:([0-9]+)\)/\2/' | sed 's/\.constprop.*//;s/\.isra.*//' | sort | uniq -c | sort -rn)"
echo "$sites"
other="$(echo "$sites" | grep -v -E ' (sw_key|sm_equation|sw_publish)$' | grep -c . || true)"
"$vg/bin/valgrind" --tool=memcheck -q "$d/probe" control >/dev/null 2>"$d/control" || true
control="$(grep -c 'depends on uninitialised' "$d/control" || true)"
[[ $control -gt 0 ]] || { echo "FAIL the control branch on the key was not flagged";exit 1; }
echo "ok   control: a branch on the key is flagged ($control)"
if [[ $other -eq 0 ]]; then echo "ok   no key-dependent branch or address outside the declared decisions";else echo "FAIL $other report sites outside sw_key, sm_equation and sw_publish";exit 1;fi
