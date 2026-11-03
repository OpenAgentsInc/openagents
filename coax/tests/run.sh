#!/usr/bin/env bash
# coax0 tests.
#   tests/pass/NAME.coax  must compile and run, printing exactly NAME.out
#                        (in debug and in --release).
#   tests/fail/NAME.coax  first line "// expect: E0xxx" must fail to compile
#                        with that error code; "// expect: trap TEXT" must
#                        compile, then stop at run time printing TEXT.
# Then every other suite, one named unit each.
#
#   tests/run.sh                    every unit, in order
#   tests/run.sh --list             name the units, run none
#   tests/run.sh --only UNIT...     just those units, in the order given
#   tests/run.sh --part I/N         the I-th of N parts: units I, I+N, I+2N...
#                                   in order (1 <= I <= N)
#
# The whole suite takes longer than one command may (AGENTS.md: 15 minutes:
# about 45 minutes of units on a loaded 28-core Linux machine, 2026-11-02), so
# run it in parts, --part 1/8 ... 8/8 (the longest about 11 minutes there:
# parser, provenance and results are 4-5 minutes each), or by unit. Each unit
# prints its time when it ends, and the run ends with the units that failed.
set -uo pipefail
cd "$(dirname "$0")/.."
COAX=bin/coax
COAX_ABS="$(cd "$(dirname "$COAX")" && pwd -P)/$(basename "$COAX")"
pass=0; fail=0
note() { echo "FAIL $1: $2"; fail=$((fail + 1)); }

# The fixtures count their own cases; a unit fails when one of its cases does.
fixtures_done() { echo "$pass passed, $fail failed"; [[ $fail -eq 0 ]]; }

unit_pass() {
  local src name mode got rc
  for src in tests/pass/*.coax; do
    name=$(basename "$src" .coax)
    for mode in "" --release; do
      got=$("$COAX" run "$src" $mode 2>&1)
      rc=$?
      if [[ $rc -eq 0 && "$got" == "$(cat "tests/pass/$name.out")" ]]; then pass=$((pass + 1))
      else note "$name ${mode:-debug}" "output differs:"; diff <(echo "$got") "tests/pass/$name.out" | head -20; fi
    done
  done
  fixtures_done
}

unit_fail() {
  local src name expect want got rc
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
  fixtures_done
}

# examples/NAME.coax with NAME.out: the shipped demos, in both modes.
unit_examples() {
  local src name mode got
  for src in examples/*.coax; do
    name=${src%.coax}
    [[ -f $name.out ]] || continue
    for mode in "" --release; do
      got=$("$COAX" run "$src" $mode 2>&1)
      if [[ "$got" == "$(cat "$name.out")" ]]; then pass=$((pass + 1)); else note "$src ${mode:-debug}" "output differs"; fi
    done
  done
  fixtures_done
}

# The C boundary: a Coax library and header linked into a C host. The verdict
# is the line that says so, wherever the C compiler's output puts it.
unit_pulse() {
  local got rc
  got=$(examples/pulse/run.sh 2>&1); rc=$?
  if [[ $rc -eq 0 && "$got" == *"matches the C reference byte for byte"* ]]; then pass=$((pass + 1)); else note pulse "exit $rc: $(head -20 <<< "$got")"; fi
  fixtures_done
}

unit_python() { python3 tests/crash_reporting.py && python3 tests/compile_time.py && python3 tests/tools.py && python3 tests/state_defaults.py && python3 tests/diagnostics_json.py; }
unit_native() { python3 tests/native.py; }
unit_native_checks() {
  python3 tests/native_checks.py && python3 tests/native_check_baseline.py && python3 tests/native_check_emission.py && python3 tests/native_check_execution.py && python3 tests/native_execution_transport.py && python3 tests/native_check_forall.py && python3 tests/native_report_inputs.py && python3 tests/native_compiler_identity.py && python3 tests/native_check_semantics.py
}
unit_lexer() { python3 tests/lexer.py; }
unit_parser() { python3 tests/parser.py; }
unit_budget() { python3 tests/run_budget.py; }
unit_strings() { python3 tests/ct_strings.py; }
unit_traversal() { python3 tests/traversal_work.py; }
unit_provenance() { python3 tests/provenance.py && web/tool.sh tests/stale-compiler.coax "$PWD" "$COAX_ABS"; }
unit_query() { web/tool.sh tests/query.coax "$COAX_ABS"; }
unit_steps() { web/tool.sh tests/steps.coax "$COAX_ABS"; }
unit_library_checks() { web/tool.sh tests/library-checks.coax "$COAX_ABS"; }
unit_authority_cache() { web/tool.sh tests/authority-cache.coax "$COAX_ABS"; }
unit_check_reuse() { web/tool.sh tests/check-reuse.coax "$COAX_ABS"; }
unit_parallel_checks() { web/tool.sh tests/parallel-checks.coax "$COAX_ABS"; }
unit_release() { python3 tests/release.py; }
unit_mutation_schedule() {
  python3 tests/mutation_schedule.py && python3 tests/mutation_parallel.py && python3 tests/mutation_scope.py && python3 tests/evidence.py && python3 tests/bootstrap_mutation_shards.py && tests/mutation-gate.sh
}
unit_build_profiles() { python3 tests/build_profiles.py; }
unit_semantic_profiles() { python3 tests/semantic_profiles.py; }
unit_checker() {
  python3 tests/checker.py && python3 tests/borrow_summaries.py && python3 tests/borrow_summary_systemone.py && \
    COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/retained_borrows.py && \
    COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/retained_borrows_native.py
}
# Actual System One admission and offline generated-Coax polling. Retain failures.
unit_systemone() {
  local systemone_output
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
    return 0
  fi
  echo "System One gate failed; evidence: $systemone_output" >&2
  return 1
}
# R2 source witnesses, evaluator/C agreement and real synchronous file effects.
# Preserve the raw observations on failure; each command also supports a focused
# selection or native profile for bounded iteration outside the whole suite.
unit_results() {
  local results_output
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
     python3 tests/sys/read-file.py --compiler "$PWD/$COAX" --output "$results_output/read-file"; then
    return 0
  fi
  echo "Result language/file gate failed; evidence: $results_output" >&2
  return 1
}
# The book against this compiler: every chapter's examples (debug and release
# output, checks, the ch15 mutation gate, the C/C++ and protobuf hosts), its
# navigation, links, checked excerpts and chapter registry, and its reviewed
# capability claims. `book/check-examples.sh --only NAME...` reruns single units.
unit_book() { bash book/check-examples.sh && python3 book/capability-claims.py; }
unit_sources() {
  python3 tests/sources.py && python3 tests/modules.py && python3 tests/lowering.py && python3 tests/generics.py && python3 tests/effects.py && python3 tests/quantity.py && python3 tests/quantity_native.py && python3 tests/quantity_derived.py && python3 tests/quantity_derived_native.py
}
unit_reflection() { python3 tests/reflection.py; }
# Book maintenance and bounded shared-library adoption, without model/network effects.
unit_library_tools() {
  python3 tests/book_claims.py && python3 tests/book_selection.py && python3 tests/library_recipes.py && \
    python3 ../scripts/book_routes.py --check && \
    PYTHONDONTWRITEBYTECODE=1 python3 ../scripts/tests/test_book_routes.py && \
    PYTHONDONTWRITEBYTECODE=1 python3 ../scripts/test_publish_coax_book.py
}
unit_sequence() {
  local sequence_output rc=1
  sequence_output=$(mktemp -d)
  if [[ -n "$sequence_output" ]] && "$COAX" test lib/sequence.coax --library && "$COAX" mutate lib/sequence.coax --library && \
     python3 tests/sequence.py --output "$sequence_output"; then
    rc=0
  fi
  [[ -z "$sequence_output" ]] || rm -rf "$sequence_output"
  return $rc
}
unit_bounded() { ./tests/bounded.sh "$COAX_ABS"; }
# lib/kept.coax, the owned fixed-size text the money and admission ledgers keep.
unit_kept() { "$COAX" test lib/kept.coax --library && "$COAX" mutate lib/kept.coax --library; }
unit_debugging() { python3 tests/debugging.py; }
unit_nostr() { python3 tests/nostr.py; }
unit_constant_time() { python3 tests/constant_time.py; }
unit_wrapping() { python3 tests/wrapping.py; }
unit_mul_wide() { python3 tests/mul_wide.py && python3 tests/foreign_buffers.py; }
unit_cuda() {
  python3 tests/cuda.py && python3 tests/metal_compute.py && "$COAX" test lib/compute/mma.coax --lib && "$COAX" mutate lib/compute/mma.coax --lib && "$COAX" test lib/compute/apple.coax --lib && "$COAX" mutate lib/compute/apple.coax --lib
}
unit_proto_cap() { tests/proto/check-cap.sh; }
unit_proto() { python3 tests/proto_import.py; }
unit_proto_policy() { python3 tests/proto/presence_policy.py; }
unit_proto_message() { python3 tests/proto/message_limit.py; }
unit_verify() { python3 tests/verify.py; }
unit_wire_checks() { "$COAX" test tests/proto/runtime.coax; }
unit_generated_checks() { python3 tests/nostr/proto_wire_parity.py && "$COAX" test tests/nostr/proto_event_validation.coax && "$COAX" test tests/nostr/proto_fi_validation.coax; }
unit_wire_core() { "$COAX" test lib/proto/wire.coax && "$COAX" mutate lib/proto/wire.coax; }
unit_importer_checks() { "$COAX" test lib/proto/importer.coax; }
unit_fingerprint_checks() {
  "$COAX" test lib/crypto/sha256-kernel.coax && "$COAX" mutate lib/crypto/sha256-kernel.coax && "$COAX" test lib/proto/fingerprint.coax && "$COAX" mutate lib/proto/fingerprint.coax
}
unit_unit_algebra() {
  "$COAX" test lib/compiler/quantity.coax && "$COAX" mutate lib/compiler/quantity.coax --root-only && "$COAX" test lib/compiler/unit-algebra.coax && "$COAX" mutate lib/compiler/unit-algebra.coax --root-only && python3 tests/unit_algebra.py
}
unit_unit_forms() { "$COAX" test lib/compiler/unit-forms.coax && "$COAX" mutate lib/compiler/unit-forms.coax --root-only && COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/unit_forms.py; }
unit_spatial_policy() { "$COAX" test lib/compiler/spatial-policy.coax && "$COAX" mutate lib/compiler/spatial-policy.coax --root-only && COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/spatial_policy.py; }
unit_borrow_summary() { "$COAX" test lib/compiler/borrow-summary.coax && "$COAX" mutate lib/compiler/borrow-summary.coax --root-only && COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/borrow_summary_native.py; }
# Source-token mutation has its own new policy and CLI witnesses. The unity
# native apply/emit/undo probe needs matching private frontend objects:
# tests/spatial/operator-native.md records its opt-in command.
unit_spatial_operators() { "$COAX" test lib/compiler/spatial-mutation.coax && "$COAX" mutate lib/compiler/spatial-mutation.coax --root-only && COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/spatial_operators.py; }
# M3 frontend/aggregate seams. Preserve private evidence on failure; these are
# new gates, distinct from the frozen spatial-policy oracle above.
unit_spatial() {
  local spatial_output
  spatial_output=$(mktemp -d "${TMPDIR:-/tmp}/coax-spatial.XXXXXX")
  if [[ -n "$spatial_output" ]] && \
     COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/spatial.py && \
     COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/spatial_interop.py && \
     COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/spatial_045.py && \
     COAX_TEST_COMPILER="$PWD/$COAX" COAX_SPATIAL_RECORD_DIR="$spatial_output/aggregate" python3 tests/spatial_native.py && \
     python3 tests/spatial/frame_parser_native.py --compiler "$PWD/$COAX" --output "$spatial_output/frame-parser" && \
     COAX_TEST_COMPILER="$PWD/$COAX" python3 tests/spatial_spacecam.py "$spatial_output/spacecam" && \
     "$COAX" test --library ../aethyr/spacecam.coax --only 'camera-relative displacement' && \
     "$COAX" test --library ../aethyr/spacecam.coax --only 'a hull at 5 AU'; then
    return 0
  fi
  echo "Spatial gate failed; evidence: $spatial_output" >&2
  return 1
}

# Every unit, in the order the whole suite runs them.
UNITS=(pass fail examples pulse python native native_checks lexer parser budget strings traversal
  provenance query steps library_checks authority_cache check_reuse parallel_checks release mutation_schedule
  build_profiles semantic_profiles checker systemone results book sources reflection library_tools
  sequence bounded kept debugging nostr constant_time wrapping mul_wide cuda proto_cap proto proto_policy
  proto_message verify wire_checks generated_checks wire_core importer_checks fingerprint_checks
  unit_algebra unit_forms spatial_policy borrow_summary spatial_operators spatial)

usage() { echo "usage: tests/run.sh [--list | --only UNIT... | --part I/N]" >&2; exit 2; }
selected=()
case "${1:-}" in
  "") selected=("${UNITS[@]}") ;;
  --list) printf '%s\n' "${UNITS[@]}"; exit 0 ;;
  --only)
    shift; [[ $# -gt 0 ]] || usage
    for u in "$@"; do
      declare -F "unit_$u" > /dev/null || { echo "tests/run.sh: no unit '$u' (tests/run.sh --list)" >&2; exit 2; }
      selected+=("$u")
    done ;;
  --part)
    [[ $# -eq 2 && "$2" =~ ^([0-9]+)/([0-9]+)$ ]] || usage
    part=${BASH_REMATCH[1]} parts=${BASH_REMATCH[2]}
    [[ $parts -ge 1 && $part -ge 1 && $part -le $parts ]] || usage
    for ((k = part - 1; k < ${#UNITS[@]}; k += parts)); do selected+=("${UNITS[$k]}"); done ;;
  *) usage ;;
esac

failed=()
for u in "${selected[@]}"; do
  t0=$SECONDS pass=0 fail=0
  echo "== $u"
  "unit_$u"
  rc=$?
  [[ $rc -eq 0 ]] || failed+=("$u")
  echo "== $u: $([[ $rc -eq 0 ]] && echo ok || echo "FAILED ($rc)") in $((SECONDS - t0)) s"
done
if [[ ${#failed[@]} -gt 0 ]]; then
  echo "tests/run.sh: ${#failed[@]} of ${#selected[@]} unit(s) failed: ${failed[*]}"
  exit 1
fi
echo "tests/run.sh: ${#selected[@]} unit(s) passed"
