#!/usr/bin/env bash
# Book verification tooling only; not part of the compiler or app bootstrap.
# An .out fixture requires exact output AND the declared exit status (default 0).
# --list names units without executing them. --only NAME... selects exact names.
# Selection is validated in full before a compiler, build or fixture can run.
set -uo pipefail
cd "$(dirname "$0")"
sources=(examples/*.coax examples/ch12_debug/main.coax)
units=()
for src in "${sources[@]}"; do
  unit=${src#examples/}; units+=("${unit%.coax}")
done
units+=(ch17_glow ch27_frontend ch20_interop ch15_requests.mutation ch24_workflow structure)
only=0 list=0
requested=()
usage() { echo 'usage: check-examples.sh [--list] [--only NAME...] (exact names from --list)' >&2; }
while (($#)); do
  case "$1" in
    --list) list=1; shift ;;
    --only)
      if [[ $only -ne 0 ]]; then usage; exit 2; fi
      only=1; shift
      while (($#)) && [[ $1 != --* ]]; do requested+=("$1"); shift; done
      if [[ ${#requested[@]} -eq 0 ]]; then echo 'book checks: --only needs at least one name' >&2; exit 2; fi
      ;;
    *) usage; exit 2 ;;
  esac
done
known() {
  local candidate
  for candidate in "${units[@]}"; do [[ $candidate != "$1" ]] || return 0; done
  return 1
}
selected() {
  [[ $only -ne 0 ]] || return 0
  local wanted
  for wanted in "${requested[@]}"; do [[ $wanted != "$1" ]] || return 0; done
  return 1
}
seen=()
for unit in "${units[@]}"; do
  for previous in "${seen[@]+"${seen[@]}"}"; do
    if [[ $unit == "$previous" ]]; then echo "book checks: duplicate unit name '$unit'" >&2; exit 2; fi
  done
  seen+=("$unit")
done
for wanted in "${requested[@]+"${requested[@]}"}"; do
  if ! known "$wanted"; then echo "book checks: unknown name '$wanted' (use --list)" >&2; exit 2; fi
done
selected_units=()
for unit in "${units[@]}"; do selected "$unit" && selected_units+=("$unit"); done
if [[ $list -ne 0 ]]; then printf '%s\n' "${selected_units[@]}"; exit 0; fi
COAX=../bin/coax
[[ -x $COAX ]] || ../build.sh || exit 1
tmp=$(mktemp -d)
trap 'find "$tmp" -depth -delete' EXIT
fail=0
for src in "${sources[@]}"; do
  unit=${src#examples/}; selected "${unit%.coax}" || continue
  name=${src%.coax}
  if [[ -f $name.out ]]; then
    expected_status=0
    [[ ! -f $name.status ]] || expected_status=$(cat "$name.status")
    for mode in debug release; do
      flags=()
      [[ $mode != release ]] || flags+=(--release)
      fixture="$name.out"
      mode_status=$expected_status
      [[ ! -f $name.$mode.out ]] || fixture="$name.$mode.out"
      [[ ! -f $name.$mode.status ]] || mode_status=$(cat "$name.$mode.status")
      "$COAX" run "$src" "${flags[@]}" > "$tmp/actual" 2>&1
      status=$?
      if [[ $status != "$mode_status" ]] || ! cmp -s "$tmp/actual" "$fixture"; then
        echo "FAIL $src ($mode): exit $status, expected $mode_status"
        diff -u "$fixture" "$tmp/actual" || true
        fail=1
      fi
    done
  elif ! rg -q '^main\s*::' "$src"; then
    # No main: an #export_c library, built as an object file in both profiles.
    for mode in debug release; do
      flags=()
      [[ $mode != release ]] || flags+=(--release)
      if ! "$COAX" lib "$src" -o "$tmp/lib.o" "${flags[@]}" > /dev/null 2> "$tmp/compile.log"; then
        echo "FAIL $src ($mode) does not build as a library"; cat "$tmp/compile.log"; fail=1
      fi
    done
  elif ! "$COAX" c "$src" > /dev/null 2> "$tmp/compile.log"; then
    echo "FAIL $src does not compile"; cat "$tmp/compile.log"; fail=1
  fi
  if rg -q '^\s*#(test|forall)\b' "$src"; then
    "$COAX" test "$src" > "$tmp/checks.log" 2>&1 || { echo "FAIL $src checks"; cat "$tmp/checks.log"; fail=1; }
  fi
done
if selected ch17_glow; then
  "$COAX" test examples/lib/ch17_glow.coax > "$tmp/checks.log" 2>&1 || { echo "FAIL ch17 checks"; cat "$tmp/checks.log"; fail=1; }
  "$COAX" metal examples/lib/ch17_glow.coax > /dev/null || { echo "FAIL ch17 metal"; fail=1; }
fi
if selected ch27_frontend; then
  printf '#load "%s";\n' "$PWD/../lib/compiler/lexer.coax" "$PWD/../lib/compiler/parser.coax" "$PWD/examples/ch27_frontend/driver.coax" > "$tmp/frontend.coax"
  "$COAX" run "$tmp/frontend.coax" > "$tmp/actual" 2>&1
  status=$?
  if [[ $status != 0 ]] || ! cmp -s "$tmp/actual" examples/ch27_frontend/driver.out; then
    echo "FAIL ch27 frontend"; cat "$tmp/actual"; fail=1
  fi
fi
if selected ch20_interop; then
  examples/ch20_interop/run.sh > "$tmp/actual" 2>&1
  status=$?
  if [[ $status != 0 ]] || ! cmp -s "$tmp/actual" examples/ch20_interop/run.out; then
    echo "FAIL ch20 interop"; cat "$tmp/actual"; fail=1
  fi
fi
if selected ch15_requests.mutation; then
  "$COAX" mutate examples/ch15_requests.coax > "$tmp/mutate.log" 2>&1 || { echo "FAIL ch15 mutation gate"; cat "$tmp/mutate.log"; fail=1; }
fi
if selected ch24_workflow; then
  examples/ch24_workflow/run.sh > "$tmp/workflow.log" 2>&1
  status=$?
  if [[ $status != 0 ]] || ! cmp -s "$tmp/workflow.log" examples/ch24_workflow/run.out; then
    # Optional protoc adds a verified status line to the normal transcript.
    if [[ $status != 0 ]] || [[ -z ${PROTOC:-} ]] || ! cmp -s "$tmp/workflow.log" examples/ch24_workflow/run-protoc.out; then
      echo "FAIL ch24 workflow"; cat "$tmp/workflow.log"; fail=1
    fi
  fi
fi
if selected structure; then python3 check-structure.py || fail=1; fi
if [[ $fail -eq 0 ]]; then
  if [[ $only -ne 0 ]]; then
    echo "selected book checks pass (${#selected_units[@]} of ${#units[@]}; partial verification)"
  else
    echo 'all book examples pass (debug, release, checks, mutation, native hosts and structure)'
  fi
fi
exit "$fail"
