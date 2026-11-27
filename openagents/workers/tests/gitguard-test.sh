#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: builds the hook with tool.sh, makes scratch repositories with git, feeds it JSON)
# Checks the Claude Code PreToolUse hook (coder/tui/gitguard.coax, GitHub #13771) against a real main
# checkout and a worktree of it, then the Claude worker (coder/tui/claudeworker.coax) handing it to a
# stand-in claude (claude_stub.py, CLAUDE_STUB_MODE=settings), under a throwaway folder:
#   openagents/workers/tests/gitguard-test.sh
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../.." && pwd)"
t="${TMPDIR:-/tmp}"; tmp="$(mktemp -d "${t%/}/gg-test.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
"$repo/coax/web/tool.sh" "$repo/openagents/workers/gitguard.coax" < /dev/null > /dev/null 2>&1 || true
hook="$repo/coax/web/build/tools/gitguard-$(shasum -a 256 "${COAX:-$repo/coax/bin/coax}" | cut -d ' ' -f 1)"
[[ -x "$hook" ]] || { echo "FAIL the hook did not build"; exit 1; }

main="$tmp/main repo"; wt="$tmp/wt"
mkdir -p "$main/sub"
git -C "$main" init -q -b main
git -C "$main" -c user.name=t -c user.email=t@example.invalid commit -q --allow-empty -m start
git -C "$main" worktree add -q -b coder/x "$wt"

failed=0
# expect EXIT TOOL COMMAND CWD: the hook's exit for that call
expect() {
  local want="$1" tool="$2" cmd="$3" cwd="$4" got=0 err
  local json
  json="$(TOOL="$tool" CMD="$cmd" CWD="$cwd" /usr/bin/python3 -I -c 'import json, os; print(json.dumps({"hook_event_name": "PreToolUse", "tool_name": os.environ["TOOL"], "tool_input": {"command": os.environ["CMD"]}, "cwd": os.environ["CWD"]}))')"
  err="$(HOME="$tmp" "$hook" <<< "$json" 2>&1 >/dev/null)" || got=$?
  if [[ "$got" == "$want" ]]; then echo "ok   $tool \`$cmd\` in ${cwd#$tmp/} -> $got"
  else echo "FAIL $tool \`$cmd\` in ${cwd#$tmp/} -> $got (wanted $want) $err"; failed=$((failed + 1)); fi
}
expect 2 Bash "git stash" "$main"
expect 2 Bash "git stash" "$main/sub"
expect 2 Bash "cd sub && git reset --hard" "$main"
expect 2 Bash "git -C '$main' clean -fd" "$wt"
expect 2 Bash "cd \$X && git stash" "$main"
expect 0 Bash "git stash list" "$main"
expect 0 Bash "git status && git diff" "$main"
expect 0 Bash "git stash" "$wt"
expect 0 Bash "git reset --hard origin/main" "$wt"
expect 0 Bash "git stash" "$tmp"
expect 0 Read "git stash" "$main"
# the refusal Claude is handed names the form and the folder
why="$(HOME="$tmp" "$hook" <<< "{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"git stash\"},\"cwd\":\"$main\"}" 2>&1 >/dev/null || true)"
if [[ "$why" == "Refused: \`git stash\` would change the files of a shared checkout ($main)"* ]]; then echo "ok   the refusal names the form and the folder"
else echo "FAIL refusal text: $why"; failed=$((failed + 1)); fi
[[ "$(git -C "$main" stash list | wc -l | tr -d ' ')" == 0 ]] || { echo "FAIL a stash was made"; failed=$((failed + 1)); }

# the Claude worker gives Claude Code the hook: a stand-in claude runs it as Claude Code would
"$repo/coax/web/tool.sh" "$repo/openagents/workers/claudeworker.coax" < /dev/null > /dev/null 2>&1 || true
worker="$(dirname "$hook")/claudeworker-${hook##*/gitguard-}"
[[ -x "$worker" ]] || { echo "FAIL the Claude worker did not build"; exit 1; }
mkdir -p "$tmp/bin"
printf '#!/bin/sh\nexec /usr/bin/python3 -I "%s" "$@"\n' "$repo/openagents/workers/tests/claude_stub.py" > "$tmp/bin/claude"; chmod +x "$tmp/bin/claude"
drive() {   # drive WORKER CWD: one task, its lines on stdout until it ends
  /usr/bin/python3 -I - "$1" "$2" <<'PY'
import json, subprocess, sys
p = subprocess.Popen([sys.argv[1]], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
p.stdin.write(json.dumps({"task": "t", "cwd": sys.argv[2], "model": "sonnet"}) + "\n"); p.stdin.flush()
for line in p.stdout:
    print(line.rstrip())
    if json.loads(line).get("kind") == "ended": break
p.stdin.close(); p.wait(timeout=30)
PY
}
answers="$tmp/answers.jsonl"
lines="$(HOME="$tmp" PATH="$tmp/bin:$PATH" CLAUDE_STUB_OUT="$answers" CLAUDE_STUB_MODE=settings \
  CLAUDE_STUB_BASH='["git stash", "git stash list", "cd sub && git reset --hard"]' drive "$worker" "$main")"
got="$(/usr/bin/python3 -I -c 'import json, sys; print(" ".join("%d:%d" % (r["hooks"], r["exit"]) for r in map(json.loads, open(sys.argv[1]))))' "$answers" 2>/dev/null || true)"
if [[ "$got" == "1:2 1:0 1:2" && "$lines" == *'"ending":"finished"'* ]]; then echo "ok   the Claude worker's Bash calls go through the guard: git stash refused, git stash list runs"
else echo "FAIL Claude worker hook: $got / $lines"; failed=$((failed + 1)); fi
grep -q 'Refused: `git stash` would change the files of a shared checkout' "$answers" && echo "ok   Claude is handed the refusal" || { echo "FAIL no refusal for Claude"; failed=$((failed + 1)); }
# a worker with no guard beside it does not start Claude
cp "$worker" "$tmp/bin/claudeworker"
lines="$(HOME="$tmp" PATH="$tmp/bin:$PATH" CLAUDE_STUB_OUT="$tmp/none.jsonl" drive "$tmp/bin/claudeworker" "$main")"
if [[ "$lines" == *'"ending":"failed"'*"git guard"* && ! -e "$tmp/none.jsonl" ]]; then echo "ok   no guard, no run"
else echo "FAIL a worker with no guard: $lines"; failed=$((failed + 1)); fi

echo "$( [[ $failed == 0 ]] && echo "gitguard checks: ok" || echo "gitguard checks: $failed FAILED")"
exit $(( failed == 0 ? 0 : 1 ))
