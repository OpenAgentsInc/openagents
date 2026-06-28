#!/usr/bin/env bash
#
# supervisor-fresh-base.test.sh — focused tests for the fresh-base resolver.
#
# Stubs `git` (no network) and asserts that supervisor_resolve_fresh_origin_main_sha:
#   * returns the 40-char SHA from a well-formed `git ls-remote` line,
#   * queries the correct https URL + refs/heads/<branch> for the repo,
#   * lower-cases an upper-case SHA,
#   * fails (empty, rc!=0) when ls-remote errors,
#   * fails when ls-remote returns a non-SHA / truncated value,
#   * never invokes any mutating git verb (fetch/reset/checkout/clone/worktree).
#
# Run: bash apps/pylon/scripts/supervisor-fresh-base.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
ok()  { PASS=$((PASS+1)); printf 'ok   - %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf 'FAIL - %s\n' "$1"; }

# Stub git: log the full argv, then behave per SUP_TEST_MODE.
cat > "$WORK/git" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$GIT_CALL_LOG"
case "${SUP_TEST_MODE:-ok}" in
  fail)     exit 2 ;;
  garbage)  printf 'not-a-sha\trefs/heads/main\n'; exit 0 ;;
  short)    printf 'abc123\trefs/heads/main\n'; exit 0 ;;
  upper)    printf '%s\trefs/heads/main\n' "A1B2C3D4E5F60718293A4B5C6D7E8F9001122334"; exit 0 ;;
  ok|*)     printf '%s\trefs/heads/%s\n' "0123456789abcdef0123456789abcdef01234567" "${SUP_TEST_BRANCH:-main}"; exit 0 ;;
esac
STUB
chmod +x "$WORK/git"

export SUP_GIT_BIN="$WORK/git"
export GIT_CALL_LOG="$WORK/git-calls.log"
: > "$GIT_CALL_LOG"

# shellcheck source=supervisor-fresh-base.sh
source "$SCRIPT_DIR/supervisor-fresh-base.sh"

# 1. Well-formed ls-remote line -> SHA.
export SUP_TEST_MODE=ok; got="$(supervisor_resolve_fresh_origin_main_sha OpenAgentsInc/openagents)"; rc=$?
if [ "$rc" -eq 0 ] && [ "$got" = "0123456789abcdef0123456789abcdef01234567" ]; then
  ok "returns the SHA from a well-formed ls-remote line"
else
  bad "expected SHA, got '$got' rc=$rc"
fi

# 2. Queries the correct URL + ref.
if grep -q "ls-remote https://github.com/OpenAgentsInc/openagents.git refs/heads/main" "$GIT_CALL_LOG"; then
  ok "queries the correct https url and refs/heads/main"
else
  bad "did not query the expected url/ref: $(cat "$GIT_CALL_LOG")"
fi

# 3. Lower-cases an upper-case SHA.
export SUP_TEST_MODE=upper; got="$(supervisor_resolve_fresh_origin_main_sha OpenAgentsInc/openagents)"; rc=$?
if [ "$rc" -eq 0 ] && [ "$got" = "a1b2c3d4e5f60718293a4b5c6d7e8f9001122334" ]; then
  ok "lower-cases the resolved SHA"
else
  bad "expected lowercased SHA, got '$got' rc=$rc"
fi

# 4. ls-remote error -> empty + rc!=0.
export SUP_TEST_MODE=fail; got="$(supervisor_resolve_fresh_origin_main_sha OpenAgentsInc/openagents)"; rc=$?
if [ "$rc" -ne 0 ] && [ -z "$got" ]; then
  ok "fails closed when ls-remote errors"
else
  bad "expected failure on ls-remote error, got '$got' rc=$rc"
fi

# 5. Non-SHA output -> empty + rc!=0.
export SUP_TEST_MODE=garbage; got="$(supervisor_resolve_fresh_origin_main_sha OpenAgentsInc/openagents)"; rc=$?
if [ "$rc" -ne 0 ] && [ -z "$got" ]; then
  ok "fails closed when ls-remote returns a non-SHA"
else
  bad "expected failure on non-SHA, got '$got' rc=$rc"
fi

# 6. Truncated SHA -> empty + rc!=0.
export SUP_TEST_MODE=short; got="$(supervisor_resolve_fresh_origin_main_sha OpenAgentsInc/openagents)"; rc=$?
if [ "$rc" -ne 0 ] && [ -z "$got" ]; then
  ok "fails closed when ls-remote returns a truncated SHA"
else
  bad "expected failure on truncated SHA, got '$got' rc=$rc"
fi

# 7. Custom branch is honored.
: > "$GIT_CALL_LOG"
export SUP_TEST_MODE=ok SUP_TEST_BRANCH=release; supervisor_resolve_fresh_origin_main_sha OpenAgentsInc/openagents release >/dev/null; unset SUP_TEST_BRANCH
if grep -q "refs/heads/release" "$GIT_CALL_LOG"; then
  ok "honors a non-default base branch"
else
  bad "did not query the requested base branch: $(cat "$GIT_CALL_LOG")"
fi

# 8. Never invokes a mutating git verb.
if grep -Eq '(^| )(fetch|reset|checkout|clone|worktree|merge|pull|push|update-ref)( |$)' "$WORK/git-calls.log"; then
  bad "invoked a mutating git verb: $(cat "$WORK/git-calls.log")"
else
  ok "uses only the read-only ls-remote verb (no working-tree/ref mutation)"
fi

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
