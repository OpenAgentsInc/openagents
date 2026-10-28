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
# Examples that are libraries or need a host live in subfolders.
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
"$COAX" test examples/lib/ch14_glow.coax >/dev/null || { echo "FAIL examples/lib/ch14_glow.coax checks"; fail=1; }
"$COAX" metal examples/lib/ch14_glow.coax >/dev/null || { echo "FAIL examples/lib/ch14_glow.coax metal"; fail=1; }
cat ../lib/compiler/lexer.coax ../lib/compiler/parser.coax examples/ch20_frontend/driver.coax > "$tmp/frontend.coax"
if [[ "$("$COAX" run "$tmp/frontend.coax" 2>&1)" != "$(cat examples/ch20_frontend/driver.out)" ]]; then echo "FAIL examples/ch20_frontend"; fail=1; fi
if [[ "$(examples/ch17_interop/run.sh 2>&1)" != "$(cat examples/ch17_interop/run.out)" ]]; then echo "FAIL examples/ch17_interop"; fail=1; fi
[[ $fail -eq 0 ]] && echo "all book examples pass"
exit $fail
