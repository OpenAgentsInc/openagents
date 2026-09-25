#!/usr/bin/env bash
# Exercise scripts/coderdev against a stub Cargo: reuse, rebuild, a failed
# build, argument forwarding, an unrelated launch directory, and a
# CARGO_TARGET_DIR override. No real build runs.
set -euo pipefail
cd "$(dirname "$0")/.."
launcher="$PWD/scripts/coderdev"

work="$(mktemp -d /tmp/coderdev-test.XXXXXX)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/elsewhere"

# The stub records each build in a log, fails when asked, and writes a
# "coder" executable that prints its cwd and arguments.
cat >"$work/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -eu
echo "build $PWD $*" >>"$STUB_LOG"
test -z "${STUB_FAIL:-}" || { echo "error: could not compile" >&2; exit 101; }
mkdir -p "$CARGO_TARGET_DIR/debug"
cat >"$CARGO_TARGET_DIR/debug/coder" <<'BIN'
#!/usr/bin/env bash
printf 'cwd=%s\n' "$PWD"
for a in "$@"; do printf 'arg=[%s]\n' "$a"; done
printf 'env=%s\n' "${CODERDEV_TEST_SECRET:-unset}"
BIN
chmod +x "$CARGO_TARGET_DIR/debug/coder"
EOF
chmod +x "$work/bin/cargo"
export STUB_LOG="$work/build.log" CODERDEV_CARGO="$work/bin/cargo"
export CARGO_TARGET_DIR="$work/target"

fail() { echo "test-coderdev: $*" >&2; exit 1; }

# Launch from an unrelated directory; forward odd arguments exactly.
out="$(cd "$work/elsewhere" && "$launcher" -p "two words" --flag= "" 2>"$work/stderr")"
grep -qxF "cwd=$work/elsewhere" <<<"$out" || fail "launch dir not preserved: $out"
grep -qxF 'arg=[-p]' <<<"$out" || fail "flag not forwarded"
grep -qxF 'arg=[two words]' <<<"$out" || fail "spaced argument not forwarded"
grep -qxF 'arg=[--flag=]' <<<"$out" || fail "empty-valued flag not forwarded"
grep -qxF 'arg=[]' <<<"$out" || fail "empty argument not forwarded"
grep -qxF 'env=unset' <<<"$out" || fail "env leaked without env file"
grep -q '^coderdev: coder [0-9a-f]\{7,\} (clean\|dirty) ' "$work/stderr" \
  || fail "no build identity line: $(cat "$work/stderr")"
grep -q -- "--bin coder" "$STUB_LOG" || fail "binary not selected explicitly"
grep -q -- "-p coder " "$STUB_LOG" || fail "package not selected explicitly"
grep -q "^build $PWD " "$STUB_LOG" || fail "build did not run in the source tree"

# Every launch asks Cargo; Cargo decides freshness. Two launches, two calls.
"$launcher" >/dev/null 2>&1
test "$(wc -l <"$STUB_LOG")" -eq 2 || fail "expected two build invocations"

# A failed build stops the launch and leaves the old executable alone.
if STUB_FAIL=1 "$launcher" >"$work/out" 2>"$work/err"; then
  fail "failed build still launched"
fi
grep -q "not running the previous executable" "$work/err" || fail "no failure message"
test ! -s "$work/out" || fail "old executable ran after failed build"

# Credentials come from the env file into the child only.
printf 'CODERDEV_TEST_SECRET=loaded\n' >"$work/env"
out="$(CODERDEV_ENV_FILE="$work/env" "$launcher" 2>/dev/null)"
grep -qxF 'env=loaded' <<<"$out" || fail "env file not loaded"
test -z "${CODERDEV_TEST_SECRET:-}" || fail "env leaked into the parent shell"

# A relative CARGO_TARGET_DIR resolves against the launch directory.
out="$(cd "$work/elsewhere" && CARGO_TARGET_DIR=rel "$launcher" 2>"$work/stderr")"
test -x "$work/elsewhere/rel/debug/coder" || fail "relative target dir not honored"
grep -q "$work/elsewhere/rel/debug/coder" "$work/stderr" || fail "identity line lacks executable path"

echo "test-coderdev: ok"
