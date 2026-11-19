#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: runs the coder desktop verbs and a stand-in server, compares their output)
# coder desktop end to end (GitHub #13707): every offline verb against a temporary store, then the
# live verbs against desktop-mock.coax, a stand-in for Coder Desktop's Foreman control server.
# It never reads or writes ~/Library/Application Support/Coder.
#   coder/tui/desktop-smoke.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
unset CODER_FOREMAN_URL CODER_FOREMAN_TOKEN CODER_DESKTOP_DIR
tmp="$(mktemp -d "${TMPDIR:-/tmp}/coder-desktop-smoke.XXXXXX")"
mock_pid=""
cleanup() {
  [[ -z "$mock_pid" ]] || kill "$mock_pid" 2>/dev/null || true
  [[ -n "$tmp" && -d "$tmp" && "$tmp" == */coder-desktop-smoke.* ]] && rm -rf -- "$tmp"
}
trap cleanup EXIT
d="$tmp/store/chats"   # does not exist yet: create makes it
passed=0
c() { "$here/coder.sh" desktop "$@" --desktop-dir "$d"; }
has() { # NAME TEXT WANTED...
  local name="$1" text="$2"; shift 2
  for want in "$@"; do
    if [[ "$text" != *"$want"* ]]; then printf 'FAIL %s: wanted %s in:\n%s\n' "$name" "$want" "$text" >&2; exit 1; fi
  done
  passed=$((passed + 1)); printf 'ok   %s\n' "$name"
}
lacks() { # NAME TEXT UNWANTED
  if [[ "$2" == *"$3"* ]]; then printf 'FAIL %s: did not want %s in:\n%s\n' "$1" "$3" "$2" >&2; exit 1; fi
  passed=$((passed + 1)); printf 'ok   %s\n' "$1"
}
fails() { # NAME CODE COMMAND...
  local name="$1" want="$2"; shift 2
  set +e; "$@" >/dev/null 2>&1; local got=$?; set -e
  if [[ "$got" != "$want" ]]; then printf 'FAIL %s: exit %s, wanted %s\n' "$name" "$got" "$want" >&2; exit 1; fi
  passed=$((passed + 1)); printf 'ok   %s\n' "$name"
}
idof() { sed -n 's/.*"chat_id":"\([^"]*\)".*/\1/p' <<<"$1"; }

# ---- offline: the store ----
out="$(c status --json)"; has "status on an empty store" "$out" '"chat_count":0' '"store_exists":false' '"foreman":null'
out="$(c ls --json)"; has "ls on an empty store" "$out" '"chats":[]'
out="$(c create --title "Alpha  one" --folder /tmp/a --budget 3 --json)"
has "create with a title and budget" "$out" '"action":"create"' '"title":"Alpha one"' '"renamed":true' '"budget_usd_micros":3000000' '"turn_count":0'
a="$(idof "$out")"; [[ -n "$a" && -f "$d/$a.json" && -f "$d/index.json" ]] || { echo "FAIL create wrote no files" >&2; exit 1; }
b="$(c create --folder /tmp/b)"; has "create prints the new ID" "$b" "-"
out="$(c ls --json)"; has "ls lists both, newest first" "$out" "{\"id\":\"$b\",\"title\":\"New chat\"" "\"id\":\"$a\""
[[ "$(sed -n 's/.*"chats":\[{"id":"\([^"]*\)".*/\1/p' <<<"$out")" == "$b" ]] || { echo "FAIL newest is not first" >&2; exit 1; }
out="$(c pin "$a" --json)"; has "pin" "$out" '"action":"pin"' '"pinned":true' '"pin_order":0'
out="$(c ls --pinned --json)"; has "ls --pinned" "$out" "\"id\":\"$a\""; lacks "ls --pinned leaves out the unpinned" "$out" "\"id\":\"$b\""
out="$(c ls)"; has "ls for people" "$out" "pinned  $a  Alpha one  (/tmp/a)  \$0.00 of \$3.00" "        $b  New chat"
out="$(c rename "$b" "Beta" "  chat" --json)"; has "rename" "$out" '"title":"Beta chat"' '"renamed":true'
out="$(c folder "$b" /tmp/c --json)"; has "folder" "$out" '"folder":"/tmp/c"'

# a chat with history: cost $2.00, one turn with a tool call and a sub-agent
printf '%s' '{"v":1,"cost_usd_micros":2000000,"budget_usd_micros":3000000,"turns":[{"ask":"hi","reply":"hello there","cut":false,"state":2,"note":"","more":0,"earlier":0,"cmds":[{"text":"ls -la","out":"a\nb","read":false,"failed":true,"took":3}],"agents":[{"level":"low","task":"look around","state":2,"reply":"looked","more":0,"earlier":0,"after":0,"rows":[],"calls":4,"took":1}]}]}' > "$d/$a.json"
out="$(c read "$a" --json)"; has "read" "$out" '"total_turns":1' '"ask":"hi"' '"reply":"hello there"' '"tool_count":1' '"task":"look around"' '"cost_usd_micros":2000000'
lacks "read leaves tool output out" "$out" '"tools":'
out="$(c read "$a" --full --json)"; has "read --full" "$out" '"tools":[{"text":"ls -la","out":"a\nb","failed":true,"took":3}]' '"reply":"looked"'
out="$(c read "$a")"; has "read for people" "$out" "Alpha one  ($a)" "spent \$2.00 of \$3.00" "> hi" "  1 tool call (--full shows it)" "sub-agent low: look around" "hello there"
out="$(c read "$a" --full)"; has "read --full for people" "$out" '  $ ls -la  (failed)'
fails "read of a missing chat" 1 c read no-such-chat

out="$(c budget "$a" --json)"; has "budget shows" "$out" '"budget_usd_micros":3000000' '"budget_exceeded":false'
out="$(c budget "$a" --set-usd 1 --json)"; has "budget below the cost" "$out" '"budget_usd_micros":1000000' '"budget_exceeded":true'
out="$(c goal "$a" set ship the release --json)"; has "goal over budget waits" "$out" '"goal_status":"budget_limited"' '"goal_objective":"ship the release"'
out="$(c budget "$a" --override --json)"; has "override resumes the goal" "$out" '"budget_override":true' '"goal_status":"active"' '"budget_exceeded":false'
out="$(c budget "$a" --override off --json)"; has "override off" "$out" '"budget_override":false' '"goal_status":"budget_limited"'
out="$(c budget "$a" --set-usd 9.5 --json)"; has "budget raised" "$out" '"budget_usd_micros":9500000' '"budget_exceeded":false'
out="$(c goal "$a" pause --json)"; has "goal pause" "$out" '"goal_status":"paused"'
out="$(c goal "$a" resume --json)"; has "goal resume" "$out" '"goal_status":"active"'
out="$(c goal "$a" --json)"; has "goal status" "$out" '"action":"goal"' '"goal_status":"active"' '"goal_turns":0'
out="$(c goal "$a" complete)"; has "goal complete for people" "$out" "$a goal complete: ship the release"
out="$(c goal "$a" clear --json)"; has "goal clear" "$out" '"goal_status":"none"'
fails "goal pause with no goal" 1 c goal "$a" pause
fails "goal with a bad step" 2 c goal "$a" sideways
fails "budget of nothing" 2 c budget "$a" --set-usd 0
grep -q '"ask":"hi"' "$d/$a.json" && grep -q '"budget_usd_micros":9500000' "$d/$a.json" && passed=$((passed + 1)) && echo "ok   the chat file keeps its turns through every change"

out="$(c search hello --json)"; has "search finds text" "$out" "\"id\":\"$a\"" '"matched":"text"'
out="$(c search beta --json)"; has "search finds a title" "$out" "\"id\":\"$b\"" '"matched":"title"'
out="$(c search nothing-here)"; has "search with no match" "$out" "no chats match"
out="$(c status --json)"; has "status counts" "$out" '"chat_count":2' '"pinned_count":1' '"store_exists":true'
out="$(c unpin "$a" --json)"; has "unpin" "$out" '"pinned":false'
out="$(c delete "$b" --json)"; has "delete" "$out" '"deleted":true'
[[ ! -e "$d/$b.json" ]] && passed=$((passed + 1)) && echo "ok   delete removes the chat file"
out="$(c ls --json)"; lacks "delete leaves the list" "$out" "\"id\":\"$b\""
fails "unknown verb" 2 c sideways
fails "live-only verb offline" 2 c send "$a" hello
fails "create with a prompt offline" 2 c create --prompt hi --folder /tmp

# ---- live: a stand-in for the app's Foreman server ----
token="smoke-token-$$"
"$repo/coax/web/tool.sh" "$here/desktop-mock.coax" --port-file "$tmp/port" --log "$tmp/requests.log" --token "$token" --idle-ms 120000 &
mock_pid=$!
for _ in $(seq 1 600); do [[ -s "$tmp/port" ]] && break; sleep 0.2; done
[[ -s "$tmp/port" ]] || { echo "FAIL the stand-in server did not start" >&2; exit 1; }
url="http://127.0.0.1:$(cat "$tmp/port")"
l() { "$here/coder.sh" desktop "$@" --desktop-dir "$d" --foreman-url "$url" --foreman-token "$token"; }
out="$(l status --json)"; has "live status" "$out" '"foreman_url":"http://127.0.0.1:' '"tier_label":"Managed"' '"chat_count":1'
out="$(l status)"; has "live status for people" "$out" "Foreman: Managed, running 0, workspace /mock"
out="$(l tier --json)"; has "tier" "$out" '"tier":"managed"'
out="$(l ls)"; has "live ls for people" "$out" "pinned  live-1  Live chat  (/mock)  \$1.00 of \$50.00  goal active"
out="$(l ls --pinned --json)"; has "live ls --json is the app's answer" "$out" '"id":"live-1"'
out="$(l read live-1)"; has "live read" "$out" "Live chat  (live-1)" "> hi live" "hello live"
fails "live read of a missing chat" 1 l read nope
out="$(l create --title T --prompt "do it" --folder /mock --budget 2.5 --json)"; has "live create" "$out" '"chat_id":"live-2"'
fails "live create without a prompt" 2 l create
out="$(l send live-1 keep going --json)"; has "send" "$out" '"status":"running"'
out="$(l stop live-1)"; has "stop" "$out" "stopped live-1"
out="$(l stop-all --json)"; has "stop-all" "$out" '"stopped":["live-1"]'
out="$(l pin live-1)"; has "live pin" "$out" "pin live-1"
out="$(l rename live-1 New name --json)"; has "live rename" "$out" '"action":"rename","title":"New name"'
out="$(l delete live-1 --json)"; has "live delete" "$out" '"action":"delete"'
out="$(l budget live-1 --set-usd 7 --json)"; has "live budget" "$out" '"action":"set_budget","budget_usd":7'
out="$(l goal live-1 set ship it --json)"; has "live goal set" "$out" '"action":"set_goal","objective":"ship it"'
out="$(l goal live-1 pause --json)"; has "live goal pause" "$out" '"action":"pause_goal"'
out="$(l goal live-1 --json)"; has "live goal status reads the chat" "$out" '"goal_status":"active"'
out="$(l search-chats live --json)"; has "search-chats" "$out" '"results":[{"chat_id":"live-1"'
out="$(l search readme --repo --json)"; has "search --repo" "$out" '"matches":'
fails "live goal complete has no endpoint" 2 l goal live-1 complete
fails "live folder has no endpoint" 2 l folder live-1 /x
fails "live budget override has no endpoint" 2 l budget live-1 --override
fails "a wrong token" 1 "$here/coder.sh" desktop status --desktop-dir "$d" --foreman-url "$url" --foreman-token wrong
fails "a URL without a token" 2 "$here/coder.sh" desktop ls --desktop-dir "$d" --foreman-url "$url"
out="$(CODER_FOREMAN_URL="$url" CODER_FOREMAN_TOKEN="$token" "$here/coder.sh" desktop ls --json --desktop-dir "$d")"; has "the Foreman's variables" "$out" '"id":"live-1"'
out="$(CODER_FOREMAN_URL="$url" CODER_FOREMAN_TOKEN="$token" "$here/coder.sh" desktop ls --json --offline --desktop-dir "$d")"; has "--offline uses the store" "$out" "\"id\":\"$a\""
log="$(cat "$tmp/requests.log")"
has "the requests the app got" "$log" 'GET /v1/status' 'GET /v1/tier' 'POST /v1/chats/list {"limit":200,"filter":"all"}' 'POST /v1/chats/list {"limit":200,"filter":"pinned"}' \
  'POST /v1/chats/read {"chat_id":"live-1","tail":8,"include_tools":false}' 'POST /v1/chats/create {"title":"T","prompt":"do it","folder":"/mock","budget_usd":2.5}' \
  'POST /v1/chats/send {"chat_id":"live-1","prompt":"keep going","mode":"follow_up"}' 'POST /v1/chats/control {"chat_id":"live-1","action":"stop"}' \
  'POST /v1/chats/stop-all {"all":false}' 'POST /v1/search/chats {"query":"live","limit":20,"max":20}' 'POST /v1/search/repo {"query":"readme","limit":20,"max":20}'
echo "$passed checks passed"
