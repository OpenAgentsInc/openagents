#!/usr/bin/env bash
# The bench's grader on the build host (#11211). In slot SLOT at BASE:
# apply the change, check that it compiles, lay the fix commit's own test
# changes over it, and run the fix's tests.
#
#   eval.sh SLOT BASE PKG TIMEOUT_SECS   (stdin: a tar of change.patch,
#                                         tests.patch and names.txt)
#
# Prints one JSON object as its last line.
set -u
slot=$1 base=$2 pkg=$3 limit=$4
dir=$HOME/ab/slot$slot
exec 9>"$HOME/ab/slot$slot.lock"
flock 9
in=$(mktemp -d)
trap 'rm -rf "$in"' EXIT
tar -x -C "$in"
cd "$dir" || exit 98
git reset -q --hard "$base" 2>/dev/null || { git -C "$HOME/openagents" fetch -q origin && git reset -q --hard "$base"; } || exit 98
git clean -fdq
export CARGO_TARGET_DIR=$HOME/ab/target CARGO_TERM_COLOR=never CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
tests_compiled=null applied=true compiles=null tests_applied=null tests_pass=null passed=0 failed=0
t0=$(date +%s)
if [ -s "$in/change.patch" ]; then
  git apply --binary --whitespace=nowarn "$in/change.patch" 2>"$in/apply.err" || applied=false
fi
if $applied; then
  if timeout -k 10 "$limit" cargo check -q -p "$pkg" --tests --keep-going --message-format short >"$in/check.out" 2>&1; then compiles=true; else compiles=false; fi
  git add -A >/dev/null 2>&1
  if [ -s "$in/tests.patch" ]; then
    if git apply --3way --whitespace=nowarn "$in/tests.patch" >"$in/tests.err" 2>&1; then tests_applied=true; else tests_applied=false; fi
    if [ "$tests_applied" = false ]; then
      git checkout -q -- . 2>/dev/null
      git apply --whitespace=nowarn -C1 --recount "$in/tests.patch" >>"$in/tests.err" 2>&1 && tests_applied=true
    fi
  else
    tests_applied=true
  fi
  if [ "$tests_applied" = true ] && [ -s "$in/names.txt" ]; then
    mapfile -t names < "$in/names.txt"
    targets=()
    [ -s "$in/targets.txt" ] && mapfile -t targets < "$in/targets.txt"
    timeout -k 10 "$limit" cargo test -p "$pkg" "${targets[@]}" --no-fail-fast -- "${names[@]}" >"$in/test.out" 2>&1
    rc=$?
    if grep -q "could not compile" "$in/test.out"; then tests_compiled=false; else tests_compiled=true; fi
    passed=$(grep -Eo 'test result: [a-zA-Z]+\. [0-9]+ passed' "$in/test.out" | awk '{s+=$4} END {print s+0}')
    failed=$(grep -Eo '[0-9]+ failed' "$in/test.out" | awk '{s+=$1} END {print s+0}')
    if [ $rc = 0 ] && [ "$passed" -ge "${#names[@]}" ]; then tests_pass=true; else tests_pass=false; fi
  fi
fi
secs=$(( $(date +%s) - t0 ))
git reset -q --hard "$base"; git clean -fdq
python3 - "$in" <<PY
import json,sys,os
d=sys.argv[1]
def tail(n, k=3000):
    p=os.path.join(d,n)
    return open(p,errors="replace").read()[-k:] if os.path.exists(p) else ""
print(json.dumps({"applied": "$applied"=="true", "compiles": None if "$compiles"=="null" else "$compiles"=="true",
  "tests_applied": None if "$tests_applied"=="null" else "$tests_applied"=="true",
  "tests_compiled": None if "$tests_compiled"=="null" else "$tests_compiled"=="true",
  "tests_pass": None if "$tests_pass"=="null" else "$tests_pass"=="true",
  "passed": int("$passed"), "failed": int("$failed"), "eval_secs": $secs,
  "errors": sorted({__import__("re").sub(r":\\d+:\\d+:", ":", l.strip())[:300] for l in (open(os.path.join(d,"check.out"),errors="replace").read().splitlines() if os.path.exists(os.path.join(d,"check.out")) else []) if __import__("re").search(r"(^|: )error(\\[E\\d+\\])?:", l)})[:60],
  "apply_err": tail("apply.err",800), "check_tail": tail("check.out"), "tests_err": tail("tests.err",800), "test_tail": tail("test.out")}))
PY
