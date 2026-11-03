#!/usr/bin/env bash
# Firm reason: 2 (independent oracle: Valgrind's Memcheck decides secret flow in the compiled C, outside Coax)
# ctgrind-style secret-flow map of the compiled ECDSA signer (Bitcoin
# chapter 20): the release generated C of ecdsa_sign.coax, the key undefined
# under memcheck. Prints each distinct source line of the generated C where
# a key-derived value reaches a branch or an address. Passes only if every
# report is at the signer's one declared decision, the ec_declassify line
# (the validity of key, nonce, r and s), bound by line number, not by
# function name. A control program that branches on the key must be flagged.
# The probe signs with ecdsa_sign and with ecdsa_sign_recoverable (chapter 22).
#
#   coax/tests/bitcoin/ecdsa_sign_secret_probe.sh              # on Linux with nix
#   coax/tests/bitcoin/ecdsa_sign_secret_probe.sh --host HOST  # generate here, run on HOST over ssh
#   PLANT=1 coax/tests/bitcoin/ecdsa_sign_secret_probe.sh ...   # a planted key branch: must FAIL
#
# Memcheck runs on Linux only. With --host, the generated C and the probe are
# copied to a temporary directory on HOST (which needs nix and a C compiler)
# and removed afterwards.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../.." && pwd)";cd "$repo"
host=""
if [[ "${1:-}" == "--host" ]]; then host="$2"; fi
d="$(mktemp -d)";trap 'rm -rf "$d"' EXIT
coax/bin/coax c coax/lib/crypto/ecdsa_sign.coax --library --header "$d/es.h" --run-steps 250000000 --release > "$d/release.c"
cp coax/tests/bitcoin/ecdsa_sign_secret_probe.c "$d/probe.c"
# PLANT=1 adds one branch on a key bit to the generated signer, after the
# nonce derivation; the probe must then fail.
if [[ "${PLANT:-}" == 1 ]]; then
  sed -i.orig 's/^  coax_p_ec_candidates(l_key, l_hash, l_w);$/&\n  { static volatile int planted; if ((l_key).data[31] \& 1) planted++; }/' "$d/release.c"
  grep -q 'static volatile int planted' "$d/release.c" || { echo "FAIL the planted branch was not inserted";exit 1; }
fi
# The allowed lines: ec_declassify's body and the one line that calls it.
allowed="$(awk '/^static _Bool coax_p_ec_declassify\(uint64_t l_valid\) \{/{body=1} body{print NR} body&&/^\}/{body=0} /coax_p_ec_declassify\(l_valid\)\)/{print NR}' "$d/release.c" | sort -u | tr '\n' ' ')"
[[ -n "$allowed" ]] || { echo "FAIL ec_declassify not found in the generated C";exit 1; }
cat > "$d/run.sh" <<'EOF'
set -euo pipefail
cd "$(dirname "$0")"
vg="$(nix build --no-link --print-out-paths nixpkgs#valgrind.out)"
vgdev="$(nix build --no-link --print-out-paths nixpkgs#valgrind.dev)"
"${CC:-gcc}" -std=c11 -O2 -g -I. -I"$vgdev/include" release.c probe.c -o probe
"$vg/bin/valgrind" --tool=memcheck --default-suppressions=no --track-origins=no -q --num-callers=1 ./probe >out.txt 2>err.txt || true
"$vg/bin/valgrind" --tool=memcheck --default-suppressions=no -q ./probe control >/dev/null 2>control.txt || true
EOF
if [[ -n "$host" ]]; then
  remote="$(ssh "$host" mktemp -d)"
  scp -q "$d/release.c" "$d/es.h" "$d/probe.c" "$d/run.sh" "$host:$remote/"
  ssh "$host" bash "$remote/run.sh"
  scp -q "$host:$remote/out.txt" "$host:$remote/err.txt" "$host:$remote/control.txt" "$d/"
  ssh "$host" rm -rf "$remote"
else
  bash "$d/run.sh"
fi
echo "probe: $(cat "$d/out.txt")"
grep -q "^signed 1 " "$d/out.txt" || { echo "FAIL the probe did not sign";head -20 "$d/err.txt";exit 1; }
grep -q "^recoverable 1 id [0-3] same 1" "$d/out.txt" || { echo "FAIL the probe did not sign recoverably";head -20 "$d/err.txt";exit 1; }
echo "reports: $(grep -c 'depends on uninitialised\|Use of uninitialised' "$d/err.txt" || true)"
# The first frame of each report: function and line in the generated C.
sites="$(grep -A1 'depends on uninitialised\|Use of uninitialised' "$d/err.txt" | grep ' at \| by ' | sed -E 's/.*: (coax_p_)?([A-Za-z0-9_.]+) \(release\.c:([0-9]+)\)/\2 \3/' | sed 's/\.constprop[^ ]*//;s/\.isra[^ ]*//' | sort | uniq -c | sort -rn || true)"
echo "$sites"
echo "allowed lines: $allowed"
other=0
while read -r count fn line; do
  [[ -n "${line:-}" ]] || continue
  [[ " $allowed " == *" $line "* ]] || other=$((other+1))
done <<< "$sites"
control="$(grep -c 'depends on uninitialised' "$d/control.txt" || true)"
[[ $control -gt 0 ]] || { echo "FAIL the control branch on the key was not flagged";exit 1; }
echo "ok   control: a branch on the key is flagged ($control)"
if [[ $other -eq 0 ]]; then echo "ok   no key-dependent branch or address outside the ec_declassify line";else echo "FAIL $other report sites outside the ec_declassify line";exit 1;fi
