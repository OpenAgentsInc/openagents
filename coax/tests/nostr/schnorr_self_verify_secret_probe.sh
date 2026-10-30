#!/usr/bin/env bash
# ctgrind-style secret-flow probe of the compiled fixed-work self-verifier
# (#89db): checked and release generated C, a valid and an invalid
# candidate, each under memcheck with the candidate undefined. The release
# build (the one served and shipped) must show no memcheck error from
# ssv_step; the checked build's overflow and bounds traps (coax_add_u64,
# coax_sub_u64, coax_index) branch on candidate-derived values by design and
# are counted, not gated. The variable-time verifier must be flagged in both
# builds. Valgrind and its headers come from nixpkgs; CC defaults to gcc.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../.." && pwd)";cd "$repo"
vg="$(nix build --no-link --print-out-paths nixpkgs#valgrind.out)"
vgdev="$(nix build --no-link --print-out-paths nixpkgs#valgrind.dev)"
d="$(mktemp -d)";trap 'rm -rf "$d"' EXIT
fail=0
for mode in checked release; do
  flag=();[[ $mode == release ]] && flag=(--release)
  coax/bin/coax c coax/lib/crypto/schnorr_self_verify.coax --library --header "$d/schnorr-self-verify.h" --run-steps 250000000 "${flag[@]}" > "$d/$mode.c"
  "${CC:-gcc}" -std=c11 -O2 -g -I"$d" -I"$vgdev/include" "$d/$mode.c" coax/tests/nostr/schnorr_self_verify_secret_probe.c -o "$d/$mode"
  for args in "" "x invalid"; do
    out="$("$vg/bin/valgrind" --tool=memcheck --error-exitcode=9 --track-origins=yes -q "$d/$mode" $args 2>"$d/err")" && rc=0 || rc=$?
    n="$(grep -c 'depends on uninitialised\|Use of uninitialised' "$d/err" || true)"
    if [[ $mode == checked ]]; then
      other="$(grep -A1 'depends on uninitialised\|Use of uninitialised' "$d/err" | grep ' at ' | grep -v -c 'coax_add_u64\|coax_sub_u64\|coax_index\|cb_span\|cb_load_word32\|ssv_load_scalar' || true)"
      echo "note $mode ${args:-valid}: $out, $n checked-mode trap reports ($other outside the checked-arithmetic and bounds traps)"
      [[ $other -eq 0 ]] || { fail=1;head -30 "$d/err"; }
    elif [[ $rc -ne 0 ]]; then fail=1;echo "FAIL $mode ${args:-valid}: memcheck reported candidate-dependent behaviour";head -20 "$d/err";else echo "ok   $mode ${args:-valid}: $out, no candidate-dependent branch or address";fi
  done
  out="$("$vg/bin/valgrind" --tool=memcheck --error-exitcode=9 -q "$d/$mode" control 2>"$d/err")" && rc=0 || rc=$?
  n="$(grep -c 'depends on uninitialised\|Use of uninitialised' "$d/err" || true)"
  if [[ $rc -eq 9 && $n -gt 0 ]]; then echo "ok   $mode control: the variable-time verifier is flagged ($n reports)";else fail=1;echo "FAIL $mode control: the probe did not see the variable-time verifier";fi
done
exit $fail
