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
    got=$(python3 tests/quiet_crash.py -- "$COAX" run "$src" 2>&1); rc=$?
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
python3 tests/crash_reporting.py && python3 tests/compile_time.py && python3 tests/tools.py && python3 tests/state_defaults.py && python3 tests/diagnostics_json.py
python_rc=$?
python3 tests/native.py
native_rc=$?
python3 tests/native_checks.py && python3 tests/native_check_baseline.py && python3 tests/native_check_emission.py && python3 tests/native_check_execution.py && python3 tests/native_execution_transport.py && python3 tests/native_check_forall.py && python3 tests/native_report_inputs.py && python3 tests/native_compiler_identity.py && python3 tests/native_check_semantics.py
native_check_baseline_rc=$?
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
python3 tests/provenance.py && web/tool.sh tests/stale-compiler.coax "$PWD" "$(cd "$(dirname "$COAX")" && pwd -P)/$(basename "$COAX")"
provenance_rc=$?
web/tool.sh tests/query.coax "$(cd "$(dirname "$COAX")" && pwd -P)/$(basename "$COAX")"
query_rc=$?
web/tool.sh tests/steps.coax "$(cd "$(dirname "$COAX")" && pwd -P)/$(basename "$COAX")"
steps_rc=$?
web/tool.sh tests/library-checks.coax "$(cd "$(dirname "$COAX")" && pwd -P)/$(basename "$COAX")"
library_checks_rc=$?
web/tool.sh tests/authority-cache.coax "$(cd "$(dirname "$COAX")" && pwd -P)/$(basename "$COAX")"
authority_cache_rc=$?
python3 tests/release.py
release_rc=$?
python3 tests/mutation_schedule.py && python3 tests/mutation_parallel.py && python3 tests/mutation_scope.py && python3 tests/evidence.py && python3 tests/bootstrap_mutation_shards.py && tests/mutation-gate.sh
mutation_schedule_rc=$?
python3 tests/build_profiles.py
build_profiles_rc=$?
python3 tests/semantic_profiles.py
semantic_profiles_rc=$?
python3 tests/checker.py && python3 tests/borrow_summaries.py && python3 tests/borrow_summary_systemone.py && \
   COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/retained_borrows.py && \
   COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/retained_borrows_native.py
checker_rc=$?
# Actual System One admission and offline generated-Coax polling. Retain failures.
systemone_output=$(mktemp -d "${TMPDIR:-/tmp}/coax-systemone.XXXXXX")
if [[ -n "$systemone_output" ]] && \
   "$COAX" test lib/systemone/call-policy.coax --library && \
   "$COAX" mutate lib/systemone/call-policy.coax --library --root-only && \
   "$COAX" test lib/systemone/owner-policy.coax --library && \
   "$COAX" mutate lib/systemone/owner-policy.coax --library --root-only && \
   COAX="$PWD/$COAX" tests/systemone-lifecycle.sh "$systemone_output/scripted" && \
   tests/systemone-process-lifecycle.sh "$systemone_output/process" "$systemone_output/scripted" && \
   python3 tests/systemone-owner.py --compiler "$PWD/$COAX" --out "$systemone_output/owner" && \
   python3 tests/systemone-owned-process.py --generated-dir "$systemone_output/owner" --out "$systemone_output/owner-process" && \
   python3 tests/systemone-outcomes.py --compiler "$PWD/$COAX" --out "$systemone_output/outcomes" && \
   python3 ../apps/openagents-desktop/tests/lab-consumer.py --coax "$PWD/$COAX" --out "$systemone_output/lab" --emit-only && \
   python3 ../apps/openagents-desktop/tests/lab-consumer-process.py --generated-dir "$systemone_output/lab" --out "$systemone_output/lab-process"; then
  systemone_rc=0
else
  echo "System One gate failed; evidence: $systemone_output" >&2
  systemone_rc=1
fi
# R2 source witnesses, evaluator/C agreement and real synchronous file effects.
# Preserve the raw observations on failure; each command also supports a focused
# selection or native profile for bounded iteration outside the whole suite.
results_output=$(mktemp -d "${TMPDIR:-/tmp}/coax-results.XXXXXX")
if [[ -n "$results_output" ]] && \
   "$COAX" test tests/results-borrow-policy.coax && \
   "$COAX" test tests/results-sum-policy.coax && \
   "$COAX" test tests/results-cache-policy.coax && \
   "$COAX" test tests/results-projection-policy.coax && \
   "$COAX" test tests/results-origin-policy.coax && \
   python3 tests/results.py --compiler "$PWD/$COAX" --output "$results_output/sources" && \
   COAX_TEST_COMPILER="$PWD/$COAX" COAX_RESULTS_PROFILE_DIR="$results_output/profiles" python3 tests/results_profiles.py && \
   "$COAX" test lib/sys/file-policy.coax --library && \
   "$COAX" mutate lib/sys/file-policy.coax --library && \
   python3 tests/sys/read-file.py --compiler "$PWD/$COAX" --output "$results_output/read-file" && \
   bash book/check-examples.sh --only ch05_tagged ch07_results; then
  results_rc=0
else
  echo "Result language/file gate failed; evidence: $results_output" >&2
  results_rc=1
fi
python3 tests/sources.py && python3 tests/modules.py && python3 tests/lowering.py && python3 tests/generics.py && python3 tests/effects.py && python3 tests/quantity.py && python3 tests/quantity_native.py && python3 tests/quantity_derived.py && python3 tests/quantity_derived_native.py
sources_rc=$?
python3 tests/reflection.py
reflection_rc=$?
# Book maintenance and bounded shared-library adoption, without model/network effects.
python3 tests/book_claims.py && python3 tests/book_selection.py && python3 tests/library_recipes.py && \
   python3 ../scripts/book_routes.py --check && \
   PYTHONDONTWRITEBYTECODE=1 python3 ../scripts/tests/test_book_routes.py && \
   PYTHONDONTWRITEBYTECODE=1 python3 ../scripts/test_publish_coax_book.py
library_tools_rc=$?
sequence_output=$(mktemp -d)
if [[ -n "$sequence_output" ]] && "$COAX" test lib/sequence.coax --library && "$COAX" mutate lib/sequence.coax --library && \
   python3 tests/sequence.py --output "$sequence_output"; then
  sequence_rc=0
else
  sequence_rc=1
fi
[[ -z "$sequence_output" ]] || rm -rf "$sequence_output"
./tests/bounded.sh "$(cd "$(dirname "$COAX")" && pwd -P)/$(basename "$COAX")"
bounded_rc=$?
python3 tests/debugging.py
debugging_rc=$?
python3 tests/nostr.py
nostr_rc=$?
python3 tests/constant_time.py
constant_time_rc=$?
python3 tests/wrapping.py
wrapping_rc=$?
python3 tests/mul_wide.py && python3 tests/foreign_buffers.py
mul_wide_rc=$?
python3 tests/cuda.py && python3 tests/metal_compute.py && "$COAX" test lib/compute/mma.coax --lib && "$COAX" mutate lib/compute/mma.coax --lib && "$COAX" test lib/compute/apple.coax --lib && "$COAX" mutate lib/compute/apple.coax --lib
cuda_rc=$?
tests/proto/check-cap.sh
proto_cap_rc=$?
python3 tests/proto_import.py
proto_rc=$?
python3 tests/proto/presence_policy.py
proto_policy_rc=$?
python3 tests/proto/message_limit.py
proto_message_rc=$?
python3 tests/verify.py
verify_rc=$?
"$COAX" test tests/proto/runtime.coax
wire_checks_rc=$?
python3 tests/nostr/proto_wire_parity.py && "$COAX" test tests/nostr/proto_event_validation.coax && "$COAX" test tests/nostr/proto_fi_validation.coax
generated_checks_rc=$?
"$COAX" test lib/proto/wire.coax && "$COAX" mutate lib/proto/wire.coax
wire_core_rc=$?
"$COAX" test lib/proto/importer.coax
importer_checks_rc=$?
"$COAX" test lib/crypto/sha256-kernel.coax && "$COAX" mutate lib/crypto/sha256-kernel.coax && "$COAX" test lib/proto/fingerprint.coax && "$COAX" mutate lib/proto/fingerprint.coax
fingerprint_checks_rc=$?
"$COAX" test lib/compiler/quantity.coax && "$COAX" mutate lib/compiler/quantity.coax --root-only && "$COAX" test lib/compiler/unit-algebra.coax && "$COAX" mutate lib/compiler/unit-algebra.coax --root-only && python3 tests/unit_algebra.py
unit_algebra_rc=$?
"$COAX" test lib/compiler/unit-forms.coax && "$COAX" mutate lib/compiler/unit-forms.coax --root-only && COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/unit_forms.py
unit_forms_rc=$?
"$COAX" test lib/compiler/spatial-policy.coax && "$COAX" mutate lib/compiler/spatial-policy.coax --root-only && COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/spatial_policy.py
spatial_policy_rc=$?
"$COAX" test lib/compiler/borrow-summary.coax && "$COAX" mutate lib/compiler/borrow-summary.coax --root-only && COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/borrow_summary_native.py
borrow_summary_rc=$?
# Source-token mutation has its own new policy and CLI witnesses. The unity
# native apply/emit/undo probe needs matching private frontend objects:
# tests/spatial/operator-native.md records its opt-in command.
"$COAX" test lib/compiler/spatial-mutation.coax && "$COAX" mutate lib/compiler/spatial-mutation.coax --root-only && COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/spatial_operators.py
spatial_operators_rc=$?
# M3 frontend/aggregate seams. Preserve private evidence on failure; these are
# new gates, distinct from the frozen spatial-policy oracle above.
spatial_output=$(mktemp -d "${TMPDIR:-/tmp}/coax-spatial.XXXXXX")
if [[ -n "$spatial_output" ]] && \
   COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/spatial.py && \
   COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/spatial_interop.py && \
   COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/spatial_045.py && \
   COAX_TEST_COMPILER="$PWD/$COAX" COAX_SPATIAL_RECORD_DIR="$spatial_output/aggregate" python3 tests/spatial_native.py && \
   python3 tests/spatial/frame_parser_native.py --compiler "$PWD/$COAX" --output "$spatial_output/frame-parser" && \
   COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/spatial_spacecam.py "$spatial_output/spacecam" && \
   "$COAX" test --library ../verse/spacecam.coax --only 'camera-relative displacement' && \
   "$COAX" test --library ../verse/spacecam.coax --only 'a hull at 5 AU'; then
  spatial_rc=0
else
  echo "Spatial gate failed; evidence: $spatial_output" >&2
  spatial_rc=1
fi
[[ $results_rc -eq 0 && $systemone_rc -eq 0 && $spatial_operators_rc -eq 0 && $spatial_rc -eq 0 && $borrow_summary_rc -eq 0 && $spatial_policy_rc -eq 0 && $unit_forms_rc -eq 0 && $unit_algebra_rc -eq 0 && $fixtures_rc -eq 0 && $python_rc -eq 0 && $native_rc -eq 0 && $native_check_baseline_rc -eq 0 && $lexer_rc -eq 0 && $parser_rc -eq 0 && $budget_rc -eq 0 && $strings_rc -eq 0 && $traversal_rc -eq 0 && $provenance_rc -eq 0 && $query_rc -eq 0 && $steps_rc -eq 0 && $library_checks_rc -eq 0 && $authority_cache_rc -eq 0 && $release_rc -eq 0 && $mutation_schedule_rc -eq 0 && $build_profiles_rc -eq 0 && $semantic_profiles_rc -eq 0 && $checker_rc -eq 0 && $sources_rc -eq 0 && $reflection_rc -eq 0 && $library_tools_rc -eq 0 && $sequence_rc -eq 0 && $bounded_rc -eq 0 && $debugging_rc -eq 0 && $nostr_rc -eq 0 && $constant_time_rc -eq 0 && $wrapping_rc -eq 0 && $mul_wide_rc -eq 0 && $cuda_rc -eq 0 && $proto_cap_rc -eq 0 && $proto_rc -eq 0 && $proto_policy_rc -eq 0 && $proto_message_rc -eq 0 && $verify_rc -eq 0 && $wire_checks_rc -eq 0 && $generated_checks_rc -eq 0 && $wire_core_rc -eq 0 && $importer_checks_rc -eq 0 && $fingerprint_checks_rc -eq 0 ]]
