#!/usr/bin/env bash
# Check launcher argument boundaries and working directory without a relay or build.
set -euo pipefail
source_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$(mktemp -d "${TMPDIR:-/tmp}/coder-pair-test.XXXXXX")"
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/bin" "$fixture/unrelated directory"
cat >"$fixture/bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$PWD" "$CARGO_TARGET_DIR" "$@" >"$PAIR_TEST_LOG"
exit "${PAIR_TEST_EXIT:-0}"
SH
chmod +x "$fixture/bin/cargo"
export PATH="$fixture/bin:$PATH"
export PAIR_TEST_LOG="$fixture/arguments"
export CARGO_TARGET_DIR="$fixture/target with spaces"
cd "$fixture/unrelated directory"
"$source_dir/pair" --codex-root "$fixture/chats with spaces" --no-claude
printf '%s\n' "$source_dir" "$CARGO_TARGET_DIR" run --release --locked -p coder-connect -- connect --codex-root "$fixture/chats with spaces" --no-claude >"$fixture/expected"
cmp "$fixture/expected" "$PAIR_TEST_LOG"
unset CARGO_TARGET_DIR
export XDG_CACHE_HOME="$fixture/cache"
"$source_dir/pair" --no-browser
test "$(sed -n '2p' "$PAIR_TEST_LOG")" = "$XDG_CACHE_HOME/openagents/target-pair"
status=0
PAIR_TEST_EXIT=17 "$source_dir/pair" --no-browser || status=$?
test "$status" = 17
echo "test-pair: ok"
