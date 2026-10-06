#!/usr/bin/env bash
# Exercise scripts/install-coder.sh against a stub Cargo and a temporary
# OPENAGENTS_HOME: install Coder and the CLI over older builds, roll back,
# roll forward, and refuse failed or incomplete builds. No real build runs.
set -euo pipefail
cd "$(dirname "$0")/.."
script="$PWD/scripts/install-coder.sh"

work="$(mktemp -d "${TMPDIR:-/tmp}/install-coder-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/home/versions" "$work/home/bin"

# The stub writes both commands, whose --version names the commit and tree state
# the script passed to the build, and fails when asked.
cat >"$work/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -eu
echo "build $PWD $*" >>"$STUB_LOG"
test -z "${STUB_FAIL:-}" || { echo "error: could not compile" >&2; exit 101; }
mkdir -p "$CARGO_TARGET_DIR/release"
cat >"$CARGO_TARGET_DIR/release/coder" <<BIN
#!/usr/bin/env bash
echo "coder stub ($CODER_BUILD_COMMIT dirty=$CODER_BUILD_DIRTY)"
BIN
chmod +x "$CARGO_TARGET_DIR/release/coder"
rm -f "$CARGO_TARGET_DIR/release/openagents"
if test -z "${STUB_MISSING_CLI:-}"; then
  cat >"$CARGO_TARGET_DIR/release/openagents" <<BIN
#!/usr/bin/env bash
test -z "$STUB_BAD_CLI" || exit 1
echo "openagents stub ($CODER_BUILD_COMMIT dirty=$CODER_BUILD_DIRTY)"
BIN
  chmod +x "$CARGO_TARGET_DIR/release/openagents"
fi
EOF
chmod +x "$work/bin/cargo"

# The build the install replaces, from somewhere else.
old="$work/home/versions/coder-terminal-old"
printf '#!/usr/bin/env bash\necho "coder old"\n' >"$old"
chmod +x "$old"
ln -s "$old" "$work/home/bin/coder"
old_cli="$work/home/versions/openagents-old"
printf '#!/usr/bin/env bash\necho "openagents old"\n' >"$old_cli"
chmod +x "$old_cli"
ln -s "$old_cli" "$work/home/bin/openagents"

export STUB_LOG="$work/build.log"
export CODER_INSTALL_CARGO="$work/bin/cargo"
export CODER_INSTALL_TARGET_DIR="$work/target"
export OPENAGENTS_HOME="$work/home"
export STUB_BAD_CLI=""
link="$work/home/bin/coder"
cli_link="$work/home/bin/openagents"

fail() {
  echo "test-install-coder: $*" >&2
  exit 1
}

"$script" 2>"$work/err" >"$work/out" || fail "install failed: $(cat "$work/err")"
new="$(readlink "$link")"
new_cli="$(readlink "$cli_link")"
case "$new" in
  "$work/home/versions/coder-openagents-"*) ;;
  *) fail "the link points at $new" ;;
esac
grep -q -- "--release" "$STUB_LOG" || fail "not a release build"
grep -q -- "-p coder -p openagents-cli --bin coder --bin openagents" "$STUB_LOG" || fail "both packages and binaries were not selected"
case "$new_cli" in
  "$work/home/versions/openagents-openagents-"*) ;;
  *) fail "the CLI link points at $new_cli" ;;
esac
test "${new#*coder-openagents-}" = "${new_cli#*openagents-openagents-}" || fail "commands came from different checkouts"
grep -q "previous:  $old" "$work/err" || fail "previous target not printed: $(cat "$work/err")"
test "$(cat "$work/home/versions/coder.previous")" = "$old" || fail "previous target not recorded"
test "$(cat "$work/home/versions/openagents.previous")" = "$old_cli" || fail "previous CLI target not recorded"
grep -q "^coder stub ($(git rev-parse HEAD) dirty=[01])$" "$work/out" \
  || fail "the installed build does not name its commit: $(cat "$work/out")"
grep -q "^openagents stub ($(git rev-parse HEAD) dirty=[01])$" "$work/out" \
  || fail "the installed CLI does not name its commit: $(cat "$work/out")"

# Rollback returns to the old build and records the new one.
"$script" --rollback 2>"$work/err" >"$work/out" || fail "rollback failed: $(cat "$work/err")"
test "$(readlink "$link")" = "$old" || fail "rollback did not restore $old"
test "$(readlink "$cli_link")" = "$old_cli" || fail "rollback did not restore the CLI"
grep -qx "coder old" "$work/out" || fail "the restored build did not run"
test "$(cat "$work/home/versions/coder.previous")" = "$new" || fail "rollback did not record $new"
test "$(cat "$work/home/versions/openagents.previous")" = "$new_cli" || fail "rollback did not record the CLI"

# A second rollback undoes the first.
"$script" --rollback 2>"$work/err" >/dev/null || fail "second rollback failed"
test "$(readlink "$link")" = "$new" || fail "second rollback did not return to $new"
test "$(readlink "$cli_link")" = "$new_cli" || fail "second rollback did not restore the new CLI"

# A failed build leaves the link alone.
if STUB_FAIL=1 "$script" 2>"$work/err" >/dev/null; then
  fail "a failed build installed"
fi
grep -q "is unchanged" "$work/err" || fail "no failure message: $(cat "$work/err")"
test "$(readlink "$link")" = "$new" || fail "a failed build moved the link"
test "$(readlink "$cli_link")" = "$new_cli" || fail "a failed build moved the CLI link"

# Missing or unrunnable CLI builds leave both installed commands unchanged.
for failure in STUB_MISSING_CLI STUB_BAD_CLI; do
  if env "$failure=1" "$script" 2>"$work/err" >/dev/null; then
    fail "$failure installed"
  fi
  test "$(readlink "$link")" = "$new" || fail "$failure moved the Coder link"
  test "$(readlink "$cli_link")" = "$new_cli" || fail "$failure moved the CLI link"
done

# Every switch is in the history, the build before the first install
# included; the failed build added nothing.
history="$work/home/versions/coder.history"
test "$(wc -l <"$history")" -eq 3 || fail "expected three switches: $(cat "$history")"
head -1 "$history" | grep -q " $old -> $new$" || fail "the first switch is not recorded: $(cat "$history")"
cli_history="$work/home/versions/openagents.history"
test "$(wc -l <"$cli_history")" -eq 3 || fail "expected three CLI switches"
head -1 "$cli_history" | grep -q " $old_cli -> $new_cli$" || fail "the first CLI switch is not recorded"

# Validate both rollback targets before switching either command.
printf '%s\n' "$work/missing-cli" >"$work/home/versions/openagents.previous"
if "$script" --rollback 2>"$work/err" >/dev/null; then
  fail "rollback accepted a missing CLI build"
fi
test "$(readlink "$link")" = "$new" || fail "invalid CLI rollback moved Coder"
test "$(readlink "$cli_link")" = "$new_cli" || fail "invalid CLI rollback moved the CLI"

# A legacy Coder-only record still rolls back without touching the CLI.
rm "$work/home/versions/openagents.previous"
"$script" --rollback 2>"$work/err" >/dev/null || fail "legacy rollback failed"
test "$(readlink "$link")" = "$old" || fail "legacy rollback did not restore Coder"
test "$(readlink "$cli_link")" = "$new_cli" || fail "legacy rollback changed the CLI"

# If installation introduced the CLI, rollback removes its link and can restore it.
fresh="$work/fresh"
mkdir -p "$fresh/bin" "$fresh/versions"
ln -s "$old" "$fresh/bin/coder"
OPENAGENTS_HOME="$fresh" "$script" 2>"$work/err" >/dev/null || fail "fresh CLI installation failed"
test "$(cat "$fresh/versions/openagents.previous")" = none || fail "fresh CLI previous state is not none"
fresh_cli="$(readlink "$fresh/bin/openagents")"
OPENAGENTS_HOME="$fresh" "$script" --rollback 2>"$work/err" >/dev/null || fail "fresh CLI rollback failed"
test ! -e "$fresh/bin/openagents" && test ! -L "$fresh/bin/openagents" || fail "fresh CLI rollback kept the link"
OPENAGENTS_HOME="$fresh" "$script" --rollback 2>"$work/err" >/dev/null || fail "fresh CLI second rollback failed"
test "$(readlink "$fresh/bin/openagents")" = "$fresh_cli" || fail "fresh CLI second rollback did not restore the CLI"

# Help names both shipped commands and performs no build.
builds_before="$(wc -l <"$STUB_LOG")"
"$script" --help >"$work/out" || fail "help failed"
grep -q 'bundled OpenAgents CLI' "$work/out" || fail "help does not name the CLI"
test "$(wc -l <"$STUB_LOG")" = "$builds_before" || fail "help ran a build"

# An unknown argument is a usage error.
if "$script" --bogus 2>/dev/null; then
  fail "an unknown argument was accepted"
fi

echo "test-install-coder: ok"
