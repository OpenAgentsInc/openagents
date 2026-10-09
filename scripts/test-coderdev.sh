#!/usr/bin/env bash
# Exercise scripts/coderdev against a stub Cargo: reuse, rebuild, a failed
# build, argument forwarding, an unrelated launch directory, and a
# CARGO_TARGET_DIR override. No real build runs.
set -euo pipefail
cd "$(dirname "$0")/.."
launcher="$PWD/scripts/coderdev"

scratch="${OPENAGENTS_SCRATCH:-$HOME/.openagents/scratch/coderdev-tests}"
mkdir -p "$scratch"
work="$(mktemp -d "$scratch/coderdev-test.XXXXXX")"
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
cat >"$CARGO_TARGET_DIR/debug/coder-new" <<'BIN'
#!/usr/bin/env bash
printf 'cwd=%s\n' "$PWD"
for a in "$@"; do printf 'arg=[%s]\n' "$a"; done
printf 'env=%s\n' "${CODERDEV_TEST_SECRET:-unset}"
BIN
chmod +x "$CARGO_TARGET_DIR/debug/coder-new"
EOF
chmod +x "$work/bin/cargo"
cat >"$work/bin/openagents" <<'EOF'
#!/usr/bin/env bash
set -eu
printf '%s\n' "$*" >>"$STUB_LEASE_LOG"
echo "unexpected lease invocation" >&2
exit 99
EOF
chmod +x "$work/bin/openagents"
# Permit only the launcher's read-only Git queries. Any automatic sync,
# stash (including autostash), or index/worktree mutation fails the test.
export CODERDEV_TEST_REAL_GIT="$(command -v git)"
export CODERDEV_TEST_GIT_MUTATIONS="$work/git-mutations.log"
cat >"$work/bin/git" <<'EOF'
#!/usr/bin/env bash
set -eu
args=("$@")
if [[ "${1:-}" == -C ]]; then shift 2; fi
case "${1:-}" in
  rev-parse|status) exec "$CODERDEV_TEST_REAL_GIT" "${args[@]}" ;;
  *) printf '%s\n' "${args[*]}" >>"$CODERDEV_TEST_GIT_MUTATIONS"
     echo "unexpected Git mutation from coderdev" >&2
     exit 99 ;;
esac
EOF
chmod +x "$work/bin/git"
export PATH="$work/bin:$PATH"
export CODERDEV_LEASE_BIN="$work/bin/openagents" STUB_LEASE_LOG="$work/lease.log"
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
grep -q '^coderdev: building ' "$work/stderr" || fail "no immediate build banner"
if grep -q -- '--quiet' "$STUB_LOG"; then fail "Cargo progress suppressed"; fi
grep -q -- "--bin coder-new" "$STUB_LOG" || fail "current terminal binary not selected"
grep -q -- "-p coder-new " "$STUB_LOG" || fail "current terminal package not selected"
if grep -q -- "openagents-cli" "$STUB_LOG"; then fail "Verse-bearing CLI was built"; fi
grep -q -- "-p microcoder --bin microcoder" "$STUB_LOG" || fail "companion engine not built"
test ! -e "$STUB_LEASE_LOG" || fail "launcher contacted the lease broker"
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
test -x "$work/elsewhere/rel/debug/coder-new" || fail "relative target dir not honored"
grep -q "$work/elsewhere/rel/debug/coder-new" "$work/stderr" || fail "identity line lacks executable path"

test ! -e "$STUB_LEASE_LOG" || fail "launcher contacted the lease broker"

test ! -e "$CODERDEV_TEST_GIT_MUTATIONS" || fail "launcher tried to modify Git state"

echo "test-coderdev: ok"
