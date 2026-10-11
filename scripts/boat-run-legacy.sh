#!/usr/bin/env bash
# The curl implementation of scripts/boat-run.sh, kept for `openagents`
# binaries released before `openagents boat run` existed. boat-run.sh picks
# this only when the installed `openagents` has no `boat` command.
#
#   scripts/boat-run-legacy.sh NAME -- COMMAND...
#
# NAME picks the sandbox (one per agent or task; its id is kept in
# ~/.openagents/boat/NAME). The first call creates a `large` sandbox and clones
# OpenAgentsInc/openagents; every call resets that clone to origin/main,
# applies this checkout's diff against origin/main (committed and uncommitted,
# new files included), then runs COMMAND in the clone with CARGO_INCREMENTAL=0.
# Output prints when the command ends; the exit code is the command's.
#   scripts/boat-run-legacy.sh NAME --stop    stops the sandbox (free while stopped)
#
# Needs BOAT_API_KEY, our oa-boat token (source ~/work/.secrets/boat.env, or
# Secret Manager oa-boat-api-key); BOAT_API_BASE overrides our service. Nothing is built on
# this machine. Owner, 2026-10-02: builds move off the Mac to Boat.
set -euo pipefail
name=${1:?usage: boat-run.sh NAME -- COMMAND... | NAME --stop}; shift
: "${BOAT_API_KEY:?source ~/work/.secrets/boat.env first}"
api=${BOAT_API_BASE:-https://oa-boat-157437760789.us-central1.run.app/api/v1}
state=~/.openagents/boat; mkdir -p "$state"; idfile=$state/$name
req() { curl -sS -H "Authorization: Bearer $BOAT_API_KEY" -H 'Content-Type: application/json' "$@"; }
field() { python3 -c "import json,sys;d=json.load(sys.stdin);s=d.get('sandbox',d);print(s.get('$1') or d.get('$1') or '')"; }
if [ "${1:-}" = --stop ]; then
  [ -f "$idfile" ] && req -X POST "$api/sandboxes/$(cat "$idfile")/stop" >/dev/null && echo "stopped $(cat "$idfile")"
  exit 0
fi
[ "${1:-}" = -- ] && shift
[ $# -gt 0 ] || { echo "no command" >&2; exit 2; }
id=$(cat "$idfile" 2>/dev/null || true)
if [ -n "$id" ]; then
  st=$(req "$api/sandboxes/$id" | field state)
  case $st in stopped|archived) req -X POST "$api/sandboxes/$id/resume" >/dev/null ;; ""|error) id= ;; esac
fi
if [ -z "$id" ]; then
  id=$(req -X POST "$api/sandboxes" -d '{"type":"large","ttlSeconds":14400,"noEnv":true}' | field id)
  [ -n "$id" ] || { echo "boat: could not create a sandbox" >&2; exit 1; }
  echo "$id" > "$idfile"
fi
for _ in $(seq 1 60); do
  st=$(req "$api/sandboxes/$id" | field state)
  case $st in ready|idle|running) break ;; esac; sleep 3
done
top=$(git rev-parse --show-toplevel)
git -C "$top" add -N . >/dev/null 2>&1 || true
# The patch goes up through the files API, not the command line: a large
# diff overflows the argument list (a 740 KB patch failed that way).
git -C "$top" diff --binary origin/main > "$state/$name.patch"
upload=$(python3 -c "import json,sys,base64;print(json.dumps({'path':'/tmp/oa-change.patch','content':base64.b64encode(open(sys.argv[1],'rb').read()).decode(),'encoding':'base64'}))" "$state/$name.patch")
written=$(printf '%s' "$upload" | req -X PUT "$api/sandboxes/$id/files" -d @- | field type)
[ "$written" = file.written ] || { echo "boat: the patch upload failed" >&2; exit 1; }
user_cmd=$(printf '%q ' "$@")
script="set -e; cd ~; [ -d openagents/.git ] || git clone -q https://github.com/OpenAgentsInc/openagents.git; cd openagents; git fetch -q origin; git reset -q --hard origin/main; git clean -qfd; [ -s /tmp/oa-change.patch ] && git apply /tmp/oa-change.patch; export CARGO_INCREMENTAL=0; $user_cmd"
body=$(python3 -c "import json,sys;print(json.dumps({'command':sys.argv[1],'detached':True}))" "$script")
pid=$(req -X POST "$api/sandboxes/$id/commands" -d "$body" | field processId)
[ -n "$pid" ] || { echo "boat: the command did not start" >&2; exit 1; }
echo "boat: $name on $id, process $pid" >&2
while :; do
  out=$(req "$api/sandboxes/$id/commands/$pid")
  st=$(printf '%s' "$out" | field status)
  case $st in
    running) sleep 5 ;;
    *) break ;;
  esac
done
# Upload only this run's selected artifacts when explicitly configured.
if [ -n "${OA_ARTIFACT_BUCKET:-}" ]; then
  : "${OA_ARTIFACT_ISSUE:?set the issue number for artifact links}"
  umask 077
  run_dir=$(mktemp -d "$state/run-XXXXXXXX")
  cp "$state/$name.patch" "$run_dir/change.patch"
  printf '%s' "$out" | python3 -c '
import json,pathlib,sys
d=json.load(sys.stdin); root=pathlib.Path(sys.argv[1])
for key in ("stdout", "stderr"):
    (root/(key+".log")).write_text(d.get(key) or "")
(root/"evidence.json").write_text(json.dumps({"sandbox":sys.argv[2], "process":sys.argv[3], "status":d.get("status"), "exit_code":d.get("exitCode")}))
' "$run_dir" "$id" "$pid"
  python3 "$top/scripts/cloud/publish-artifacts.py" "$run_dir" \
    --bucket "$OA_ARTIFACT_BUCKET" --issue "$OA_ARTIFACT_ISSUE" || \
    echo "boat: artifact publication failed; retained in $run_dir" >&2
fi
printf '%s' "$out" | python3 -c "
import json,sys
d=json.load(sys.stdin)
sys.stdout.write(d.get('stdout') or '');sys.stderr.write(d.get('stderr') or '')
status=d.get('status')
if status != 'exited':
    sys.stderr.write(f'boat: command $pid on sandbox $id failed with status {status or chr(34)+chr(34)}\\n')
    sys.exit(1)
code=d.get('exitCode')
sys.exit(1 if code is None else code)"
