#!/usr/bin/env bash
# Book verification tooling only; not part of the compiler or app bootstrap.
# An .out fixture requires exact output AND the declared exit status (default 0).
set -uo pipefail
cd "$(dirname "$0")"
COAX=../bin/coax
[[ -x $COAX ]] || ../build.sh || exit 1
tmp=$(mktemp -d)
trap 'find "$tmp" -depth -delete' EXIT
fail=0
for src in examples/*.coax; do
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
  elif ! "$COAX" c "$src" > /dev/null 2> "$tmp/compile.log"; then
    echo "FAIL $src does not compile"; cat "$tmp/compile.log"; fail=1
  fi
  if rg -q '^\s*#(test|forall)\b' "$src"; then
    "$COAX" test "$src" > "$tmp/checks.log" 2>&1 || { echo "FAIL $src checks"; cat "$tmp/checks.log"; fail=1; }
  fi
done
"$COAX" test examples/lib/ch14_glow.coax > "$tmp/checks.log" 2>&1 || { echo "FAIL ch14 checks"; cat "$tmp/checks.log"; fail=1; }
"$COAX" metal examples/lib/ch14_glow.coax > /dev/null || { echo "FAIL ch14 metal"; fail=1; }
cat ../lib/compiler/lexer.coax ../lib/compiler/parser.coax examples/ch20_frontend/driver.coax > "$tmp/frontend.coax"
"$COAX" run "$tmp/frontend.coax" > "$tmp/actual" 2>&1
status=$?
if [[ $status != 0 ]] || ! cmp -s "$tmp/actual" examples/ch20_frontend/driver.out; then
  echo "FAIL ch20 frontend"; cat "$tmp/actual"; fail=1
fi
examples/ch17_interop/run.sh > "$tmp/actual" 2>&1
status=$?
if [[ $status != 0 ]] || ! cmp -s "$tmp/actual" examples/ch17_interop/run.out; then
  echo "FAIL ch17 interop"; cat "$tmp/actual"; fail=1
fi
"$COAX" mutate examples/ch23_requests.coax > "$tmp/mutate.log" 2>&1 || { echo "FAIL ch23 mutation gate"; cat "$tmp/mutate.log"; fail=1; }
examples/ch26_workflow/run.sh > "$tmp/workflow.log" 2>&1
status=$?
if [[ $status != 0 ]] || ! cmp -s "$tmp/workflow.log" examples/ch26_workflow/run.out; then
  # Optional protoc adds a verified status line to the normal transcript.
  if [[ $status != 0 ]] || [[ -z ${PROTOC:-} ]] || ! cmp -s "$tmp/workflow.log" examples/ch26_workflow/run-protoc.out; then
    echo "FAIL ch26 workflow"; cat "$tmp/workflow.log"; fail=1
  fi
fi
python3 check-structure.py || fail=1
[[ $fail -ne 0 ]] || echo "all book examples pass (debug, release, checks, mutation, native hosts and structure)"
exit "$fail"
