#!/usr/bin/env bash
# Every example the book shows must compile and print what the book says.
# examples/NAME.coax with examples/NAME.out: output must match exactly.
# examples/NAME.coax without .out: must compile (the chapter shows its run).
set -uo pipefail
cd "$(dirname "$0")"
COAX=../bin/coax
[[ -x $COAX ]] || ../build.sh
fail=0
for src in examples/*.coax; do
  name=${src%.coax}
  if [[ -f $name.out ]]; then
    got=$("$COAX" run "$src" 2>&1)
    if [[ "$got" != "$(cat "$name.out")" ]]; then echo "FAIL $src"; diff <(echo "$got") "$name.out"; fail=1; fi
  elif ! "$COAX" c "$src" >/dev/null; then
    echo "FAIL $src does not compile"; fail=1
  fi
done
[[ $fail -eq 0 ]] && echo "all book examples pass"
exit $fail
