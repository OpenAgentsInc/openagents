#!/usr/bin/env bash
# Tests for agent-gate.sh: the cap, the park on a usage limit, and resume.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
gate=$here/agent-gate.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
export OA_AGENT_GATE_DIR=$tmp/gate OA_AGENT_MAX=2 OA_OPENAGENTS_BIN=$tmp/none

decision() {
  local out
  out=$(echo '{"tool_name":"Agent"}' | bash "$gate" pre)
  if [ -z "$out" ]; then echo allow; else printf '%s' "$out" | jq -r '.hookSpecificOutput.permissionDecision'; fi
}
fail() { echo "FAIL: $*"; exit 1; }

[ "$(decision)" = allow ] || fail "an idle gate allows"
echo '{"agent_id":"a1"}' | bash "$gate" start
echo '{"agent_id":"a2"}' | bash "$gate" start
[ "$(decision)" = deny ] || fail "two running agents at a cap of two deny a third"
echo '{"agent_id":"a1"}' | bash "$gate" stop
[ "$(decision)" = allow ] || fail "a finished agent frees its slot"

echo '{"error":"You have hit your session limit · resets 9:10am (America/Chicago)"}' | bash "$gate" limit
[ "$(decision)" = deny ] || fail "a usage limit parks new agents"
grep -q "resets 9:10am" "$OA_AGENT_GATE_DIR/parked" || fail "the park records the reset"
bash "$gate" resume >/dev/null
[ "$(decision)" = allow ] || fail "resume clears the park"

echo '{"error":"compile error in crate x"}' | bash "$gate" limit
[ "$(decision)" = allow ] || fail "an ordinary failure does not park"

# A capacity book that says claude is out denies.
cat >"$tmp/oa" <<'EOF'
#!/usr/bin/env bash
[ "$1 $2 $3" = "capacity check claude" ] && { echo "claude limited until 14:10 UTC"; exit 1; }
exit 0
EOF
chmod +x "$tmp/oa"
[ "$(OA_OPENAGENTS_BIN=$tmp/oa decision)" = deny ] || fail "an exhausted capacity book denies"

[ "$(OA_AGENT_GATE=off decision)" = allow ] || fail "OA_AGENT_GATE=off turns the gate off"
echo "agent-gate tests passed"
