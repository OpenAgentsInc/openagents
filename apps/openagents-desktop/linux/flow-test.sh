#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: makes a throwaway home and the mock worker, then runs the app's
# --flow-test; every check is in host-main.c run_flow)
# The OpenAgents Desktop chat flow test on Linux, headless: the app opens its page offscreen
# (software Vulkan), sends a message to a real worker process (evals/coder-desktop/mock_worker,
# never a model or a key), shows the reply, makes a new chat and sends another, and checks the
# chat list at the XDG data home: both chats, newest first, after a restart too; an index it
# can't read is never written over; a chat kept by another process is not dropped.
#   apps/openagents-desktop/linux/flow-test.sh          build, then test (on a Linux host)
#   coax/remote/remote.sh run -- apps/openagents-desktop/linux/flow-test.sh   from a Mac
# --no-build: test the binary build.sh made (build.sh --flow-test runs it so).
# OA_FLOW_SHOT=PATH keeps a PNG of the page at the end.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"

if [[ "${1:-}" != --no-build ]]; then
  exec "$here/build.sh" --no-worker --flow-test
fi

bin="$here/build/OpenAgents"
[[ -x "$bin" ]] || { echo "flow-test: no $bin (run build.sh first)" >&2; exit 2; }
command -v python3 >/dev/null || { echo "flow-test: python3 is needed for the mock worker" >&2; exit 2; }

tmp="$(mktemp -d "${TMPDIR:-/tmp}/oa-desktop-flow-XXXXXX")"
cleanup() { rm -rf -- "${tmp:?}"; }
trap cleanup EXIT
mkdir -p "$tmp/home" "$tmp/data" "$tmp/work"

# Two turns: one for each chat's message (a new chat's forget line is recorded and skipped).
cat > "$tmp/script.json" <<'JSON'
{"turns": [
  {"steps": [{"emit": {"kind": "thought", "text": "Reading the message."}},
             {"emit": {"kind": "reply", "text": "First reply "}},
             {"emit": {"kind": "reply", "text": "from the worker."}},
             {"emit": {"kind": "ended", "ending": "finished", "text": "First reply from the worker."}}]},
  {"steps": [{"emit": {"kind": "reply", "text": "Second reply from the worker."}},
             {"emit": {"kind": "ended", "ending": "finished", "text": "Second reply from the worker."}}]}
]}
JSON
python3="$(command -v python3)"
cat > "$tmp/worker" <<SH
#!/bin/sh
exec "$python3" "$repo/evals/coder-desktop/mock_worker/mock_worker.py" --script "$tmp/script.json" --record "$tmp/record.jsonl"
SH
chmod +x "$tmp/worker"

shot=()
[[ -n "${OA_FLOW_SHOT:-}" ]] && shot=(--shot "$OA_FLOW_SHOT")
status=0
env -u CODER_DESKTOP_DIR -u CODER_SETTINGS_FILE HOME="$tmp/home" XDG_DATA_HOME="$tmp/data" \
  "$bin" --flow-test --resources "$here/build/resources" --worker "$tmp/worker" --cwd "$tmp/work" ${shot[@]+"${shot[@]}"} || status=$?

# The worker got both messages, each as its own task.
if [[ "$status" == 0 ]]; then
  tasks="$(grep -c '"kind":"task"' "$tmp/record.jsonl" 2>/dev/null || true)"
  if [[ "$tasks" != 2 ]] || ! grep -q 'flow test first message' "$tmp/record.jsonl" || ! grep -q 'flow test second message' "$tmp/record.jsonl"; then
    echo "flow: FAIL worker: expected 2 tasks with both messages, the worker recorded ${tasks:-0}" >&2
    status=1
  else
    echo "flow: ok worker: 2 tasks recorded"
  fi
fi
exit "$status"
