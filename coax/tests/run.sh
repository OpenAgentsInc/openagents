#!/usr/bin/env bash
# coax0 tests.
#   tests/pass/NAME.coax  must compile and run, printing exactly NAME.out
#                        (in debug and in --release).
#   tests/fail/NAME.coax  first line "// expect: E0xxx" must fail to compile
#                        with that error code; "// expect: trap TEXT" must
#                        compile, then stop at run time printing TEXT.
set -uo pipefail
cd "$(dirname "$0")/.."
COAX=bin/coax
pass=0; fail=0
note() { echo "FAIL $1: $2"; fail=$((fail + 1)); }

for src in tests/pass/*.coax; do
  name=$(basename "$src" .coax)
  for mode in "" --release; do
    got=$("$COAX" run "$src" $mode 2>&1)
    if [[ "$got" == "$(cat "tests/pass/$name.out")" ]]; then pass=$((pass + 1))
    else note "$name ${mode:-debug}" "output differs:"; diff <(echo "$got") "tests/pass/$name.out" | head -20; fi
  done
done

for src in tests/fail/*.coax; do
  name=$(basename "$src" .coax)
  expect=$(head -1 "$src" | sed -n 's#^// expect: ##p')
  if [[ "$expect" == trap* ]]; then
    want=${expect#trap }
    got=$("$COAX" run "$src" 2>&1); rc=$?
    if [[ $rc -ne 0 && "$got" == *"$want"* ]]; then pass=$((pass + 1)); else note "$name" "expected a trap with '$want', got exit $rc: $got"; fi
  else
    got=$("$COAX" c "$src" 2>&1 >/dev/null); rc=$?
    if [[ $rc -ne 0 && "$got" == *"error[$expect]"* ]]; then pass=$((pass + 1)); else note "$name" "expected error[$expect], got exit $rc: $got"; fi
  fi
done

echo "$pass passed, $fail failed"
[[ $fail -eq 0 ]]
