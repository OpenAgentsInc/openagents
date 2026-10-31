#!/usr/bin/env bash
# release.sh's smoke comparison: a one-time failure is retried away, a
# persistent new one still stops the release, a baseline failure never does.
set -euo pipefail
cd "$(dirname "$0")/.."
source ./smoke-compare.sh
export SMOKE_RETRY_WAITS="0 0"
dir=$(mktemp -d); trap 'rm -rf "$dir"' EXIT
# A fake smoke.sh: card fails on its first run only; homepage always fails.
cat > "$dir/smoke" <<'S'
#!/usr/bin/env bash
n=$(cat "$STATE" 2>/dev/null || echo 0); echo $((n + 1)) > "$STATE"
echo "  ok    /chat is the chat page (200)"
if [[ $n -eq 0 ]]; then echo "  FAIL  / has a whole link preview with a JPEG card (none)"; else echo "  ok    / has a whole link preview with a JPEG card (https://x/og.jpg)"; fi
[[ -z "${ALWAYS:-}" ]] || echo "  FAIL  / runs the live network homepage"
S
chmod +x "$dir/smoke"
fail=0
expect() { [[ "$2" == "$3" ]] && echo "ok   $1" || { echo "FAIL $1: got '$2', want '$3'"; fail=1; }; }

export STATE="$dir/s1"; first=$("$dir/smoke")
expect "a one-time failure is not broken by the release" "$(broken_by_release "$first" "" "$dir/smoke")" ""
expect "it was run again once" "$(cat "$STATE")" "2"

export STATE="$dir/s2" ALWAYS=1; first=$("$dir/smoke")
expect "a persistent new failure still counts" "$(broken_by_release "$first" "" "$dir/smoke")" "/ runs the live network homepage"
expect "it was retried twice" "$(cat "$STATE")" "3"

export STATE="$dir/s3"; first=$("$dir/smoke")
expect "a failure already live (the baseline) is not retried or counted" "$(broken_by_release "$first" "$(printf '%s\n' "/ runs the live network homepage" "/ has a whole link preview with a JPEG card")" "$dir/smoke")" ""
expect "nothing was rerun" "$(cat "$STATE")" "1"
unset ALWAYS
export STATE="$dir/s4"; first=$("$dir/smoke"); "$dir/smoke" >/dev/null
expect "a clean run needs no retry" "$(broken_by_release "$("$dir/smoke")" "" "$dir/smoke")" ""
expect "no extra run" "$(cat "$STATE")" "3"
# A whole check (the --starts walk) that fails once passes on its retry; one
# that always fails does not.
cat > "$dir/walk" <<'S'
#!/usr/bin/env bash
n=$(cat "$STATE" 2>/dev/null || echo 0); echo $((n + 1)) > "$STATE"
[[ -z "${ALWAYS:-}" && $n -gt 0 ]]
S
chmod +x "$dir/walk"
export STATE="$dir/w1"; "$dir/walk" || true
passes_on_retry "$dir/walk" 2>/dev/null && r=pass || r=fail
expect "a walk that failed once passes on retry" "$r" "pass"
expect "it ran twice" "$(cat "$STATE")" "2"
export STATE="$dir/w2" ALWAYS=1; "$dir/walk" || true
passes_on_retry "$dir/walk" 2>/dev/null && r=pass || r=fail
expect "a walk that keeps failing still fails" "$r" "fail"
expect "it ran three times" "$(cat "$STATE")" "3"
exit $fail
