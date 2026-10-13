#!/usr/bin/env bash
# Live delegation matrix and cache check (#11511), in an isolated HOME and
# --state so the person's ~/.openagents, ~/.claude and ~/.codex stay as they are.
#
#   OPENAGENTS=path/to/openagents \
#   CLAUDE_CODE_OAUTH_TOKEN=... CODEX_AUTH=path/to/a/codex/auth.json \
#   OPENROUTER_API_KEY=... scripts/coder/delegation-live.sh [WORK_DIR]
#
# CODEX_AUTH is copied into the isolated CODEX_HOME (read, never written
# back). Pairs: Claude Code -> Codex, Codex -> Claude Code, Coder loop ->
# Claude Code, Claude Code -> OpenCode, Claude Code -> OpenCode's ACP agent
# (read-only), OpenCode -> Codex over `openagents coder-tools-mcp`. Then the
# cache check: the same three-turn Claude Code chat with and without a
# delegation in turn 2; the last turn's prompt-cache read ratio, from Claude
# Code's own usage records, must match within 0.05. The cache check runs
# first, so its two chats are the only Claude Code sessions it reads.
set -euo pipefail
: "${OPENAGENTS:?set OPENAGENTS to the openagents binary}"
: "${CLAUDE_CODE_OAUTH_TOKEN:?}" "${CODEX_AUTH:?}" "${OPENROUTER_API_KEY:?}"
WORK=${1:-$(mktemp -d)}
export HOME=$WORK/home CODEX_HOME=$WORK/codex OPENCODE_DISABLE_AUTOUPDATE=1
export CODER_DELEGATIONS_DIR=$WORK/state/delegations
mkdir -p "$HOME" "$CODEX_HOME" "$WORK/state"
install -m 600 "$CODEX_AUTH" "$CODEX_HOME/auth.json"
REPO=$WORK/repo
rm -rf "$REPO" && mkdir -p "$REPO" && cd "$REPO"
git init -q -b main && git config user.email t@example.com && git config user.name T
echo "# demo" > README.md && git add . && git commit -qm init
coder() { "$OPENAGENTS" coder --state "$WORK/state" "$@"; }
python3 -c 'import json,os; print(json.dumps({"api_key":os.environ["OPENROUTER_API_KEY"],"enabled":True,"model":"openai/gpt-6.1-sol"}))' \
  | coder plugins configure openrouter-byok --stdin --json >/dev/null
ask() { # mode session prompt
  coder chat --mode "$1" --in-place --session "$2" -p "$3" --json | tail -1
}
pair() { # mode to model file
  local model=${3:+, model set to $3}
  ask "$1" "pair-${4%.txt}" "Use the delegate tool with to set to $2$model, and task set to: Run the shell command: echo hi > $4  (actually run it, then ls to confirm). Do not do it yourself. Then give the outcome and changed files in one line."
}
echo "--- cache check"
for s in with without; do ask claude "cache-$s" "Reply with just: ready" >/dev/null; done
ask claude cache-with "Use the delegate tool with to set to codex and task set to: Create a file named cache.txt containing hi. Then give the outcome in one line." >/dev/null
ask claude cache-without "Reply with just: ok" >/dev/null
for s in with without; do ask claude "cache-$s" "Reply with just: done" >/dev/null; done
python3 - "$HOME/.claude/projects" <<'PY'
import glob, json, os, sys
last = {}
for f in glob.glob(sys.argv[1] + "/*/*.jsonl"):
    calls, seen = [], set()
    for line in open(f):
        r = json.loads(line)
        if r.get("type") != "assistant" or r["message"].get("id") in seen:
            continue
        seen.add(r["message"]["id"])
        u = r["message"].get("usage", {})
        total = u.get("input_tokens", 0) + u.get("cache_creation_input_tokens", 0) + u.get("cache_read_input_tokens", 0)
        calls.append(u.get("cache_read_input_tokens", 0) / max(1, total))
        tools = [b.get("name") for b in r["message"].get("content", []) if b.get("type") == "tool_use"]
        if "mcp__coder__delegate" in tools:
            last["delegated"] = f
    last[f] = calls
delegated = last.pop("delegated", None)
ratios = {("with" if f == delegated else "without"): calls[-1] for f, calls in last.items() if len(calls) >= 3}
print(ratios)
assert len(ratios) == 2 and abs(ratios["with"] - ratios["without"]) < 0.05, ratios
print("cache read ratio unchanged across a delegation")
PY
pair claude codex "" claude-codex.txt
pair codex claude-code claude-haiku-5-5 codex-claude.txt
pair coder claude-code claude-haiku-5-5 loop-claude.txt
pair claude opencode opencode/big-pickle claude-opencode.txt
ask claude acp "Use the delegate tool with to set to opencode, read_only set to true, and task set to: Read README.md and give its first line. Then give the outcome in one line."
OPENCODE_CONFIG_CONTENT=$(python3 -c "import json,sys; print(json.dumps({'model':'opencode/big-pickle','permission':{'*':'allow'},'share':'disabled','autoupdate':False,'mcp':{'coder':{'type':'local','command':[sys.argv[1],'coder-tools-mcp','--harness','opencode','--cwd',sys.argv[2]],'enabled':True}}}))" "$OPENAGENTS" "$REPO") \
  opencode run --format json -m opencode/big-pickle --dir "$REPO" -- "Call the coder_delegate tool with to set to codex and task set to: Run the shell command: echo hi > opencode-codex.txt. Do not do it yourself. Then give the outcome and changed files in one line." </dev/null | tail -1
echo "--- delegation records"
cat "$CODER_DELEGATIONS_DIR/delegations.jsonl"
