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
    rc=$?
    if [[ $rc -eq 0 && "$got" == "$(cat "tests/pass/$name.out")" ]]; then pass=$((pass + 1))
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

# examples/NAME.coax with NAME.out: the shipped demos, in both modes.
for src in examples/*.coax; do
  name=${src%.coax}
  [[ -f $name.out ]] || continue
  for mode in "" --release; do
    got=$("$COAX" run "$src" $mode 2>&1)
    if [[ "$got" == "$(cat "$name.out")" ]]; then pass=$((pass + 1)); else note "$src ${mode:-debug}" "output differs"; fi
  done
done

# The C boundary: a Coax library and header linked into a C host.
got=$(examples/pulse/run.sh 2>&1 | head -1)
if [[ "$got" == *"matches the C reference byte for byte"* ]]; then pass=$((pass + 1)); else note pulse "$got"; fi

echo "$pass passed, $fail failed"
[[ $fail -eq 0 ]]
fixtures_rc=$?
python3 tests/compile_time.py && python3 tests/tools.py
python_rc=$?
python3 tests/native.py
native_rc=$?
python3 tests/lexer.py
lexer_rc=$?
python3 tests/parser.py
parser_rc=$?
python3 tests/run_budget.py
budget_rc=$?
python3 tests/ct_strings.py
strings_rc=$?
python3 tests/traversal_work.py
traversal_rc=$?
python3 tests/provenance.py
provenance_rc=$?
python3 tests/mutation_schedule.py
mutation_schedule_rc=$?
python3 tests/build_profiles.py
build_profiles_rc=$?
python3 tests/checker.py
checker_rc=$?
python3 tests/sources.py
sources_rc=$?
python3 tests/reflection.py
reflection_rc=$?
python3 tests/debugging.py
debugging_rc=$?
python3 tests/nostr.py
nostr_rc=$?
python3 tests/constant_time.py
constant_time_rc=$?
python3 tests/wrapping.py
wrapping_rc=$?
python3 tests/mul_wide.py
mul_wide_rc=$?
python3 tests/proto_import.py
proto_rc=$?
python3 tests/verify.py
verify_rc=$?
"$COAX" test tests/proto/runtime.coax
wire_checks_rc=$?
"$COAX" test lib/proto/wire.coax && "$COAX" mutate lib/proto/wire.coax
wire_core_rc=$?
"$COAX" test lib/proto/importer.coax
importer_checks_rc=$?
"$COAX" test lib/proto/fingerprint.coax && "$COAX" mutate lib/proto/fingerprint.coax
fingerprint_checks_rc=$?
[[ $fixtures_rc -eq 0 && $python_rc -eq 0 && $native_rc -eq 0 && $lexer_rc -eq 0 && $parser_rc -eq 0 && $budget_rc -eq 0 && $strings_rc -eq 0 && $traversal_rc -eq 0 && $provenance_rc -eq 0 && $mutation_schedule_rc -eq 0 && $build_profiles_rc -eq 0 && $checker_rc -eq 0 && $sources_rc -eq 0 && $reflection_rc -eq 0 && $debugging_rc -eq 0 && $nostr_rc -eq 0 && $constant_time_rc -eq 0 && $wrapping_rc -eq 0 && $mul_wide_rc -eq 0 && $proto_rc -eq 0 && $verify_rc -eq 0 && $wire_checks_rc -eq 0 && $wire_core_rc -eq 0 && $importer_checks_rc -eq 0 && $fingerprint_checks_rc -eq 0 ]]
