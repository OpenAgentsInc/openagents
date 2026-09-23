#!/usr/bin/env bash
# Exercise scripts/install-coder.sh against a stub Cargo and a temporary
# OPENAGENTS_HOME: install over an older build, roll back, roll forward, and
# refuse a failed build. No real build runs.
set -euo pipefail
cd "$(dirname "$0")/.."
script="$PWD/scripts/install-coder.sh"

work="$(mktemp -d "${TMPDIR:-/tmp}/install-coder-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/home/versions" "$work/home/bin"

# The stub writes a "coder" whose --version names the commit and tree state
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
EOF
chmod +x "$work/bin/cargo"

# The build the install replaces, from somewhere else.
old="$work/home/versions/coder-terminal-old"
printf '#!/usr/bin/env bash\necho "coder old"\n' >"$old"
chmod +x "$old"
ln -s "$old" "$work/home/bin/coder"

export STUB_LOG="$work/build.log"
export CODER_INSTALL_CARGO="$work/bin/cargo"
export CODER_INSTALL_TARGET_DIR="$work/target"
export OPENAGENTS_HOME="$work/home"
link="$work/home/bin/coder"

fail() {
  echo "test-install-coder: $*" >&2
  exit 1
}

"$script" 2>"$work/err" >"$work/out" || fail "install failed: $(cat "$work/err")"
new="$(readlink "$link")"
case "$new" in
  "$work/home/versions/coder-openagents-"*) ;;
  *) fail "the link points at $new" ;;
esac
grep -q -- "--release" "$STUB_LOG" || fail "not a release build"
grep -q -- "-p coder --bin coder" "$STUB_LOG" || fail "package or binary not selected"
grep -q "previous:  $old" "$work/err" || fail "previous target not printed: $(cat "$work/err")"
test "$(cat "$work/home/versions/coder.previous")" = "$old" || fail "previous target not recorded"
grep -q "^coder stub ($(git rev-parse HEAD) dirty=[01])$" "$work/out" \
  || fail "the installed build does not name its commit: $(cat "$work/out")"

# Rollback returns to the old build and records the new one.
"$script" --rollback 2>"$work/err" >"$work/out" || fail "rollback failed: $(cat "$work/err")"
test "$(readlink "$link")" = "$old" || fail "rollback did not restore $old"
grep -qx "coder old" "$work/out" || fail "the restored build did not run"
test "$(cat "$work/home/versions/coder.previous")" = "$new" || fail "rollback did not record $new"

# A second rollback undoes the first.
"$script" --rollback 2>"$work/err" >/dev/null || fail "second rollback failed"
test "$(readlink "$link")" = "$new" || fail "second rollback did not return to $new"

# A failed build leaves the link alone.
if STUB_FAIL=1 "$script" 2>"$work/err" >/dev/null; then
  fail "a failed build installed"
fi
grep -q "is unchanged" "$work/err" || fail "no failure message: $(cat "$work/err")"
test "$(readlink "$link")" = "$new" || fail "a failed build moved the link"

# An unknown argument is a usage error.
if "$script" --bogus 2>/dev/null; then
  fail "an unknown argument was accepted"
fi

echo "test-install-coder: ok"
