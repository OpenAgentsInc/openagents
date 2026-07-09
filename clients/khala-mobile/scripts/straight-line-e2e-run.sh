#!/usr/bin/env bash
# Owner-authorized unattended straight-line E2E runner for Khala Mobile
# (issue #8543), wired to the owner-approved seeded public-safe account
# AgentFlampy and its fork AgentFlampy/openagents (recorded on #8543).
#
# Runs every leg of the launch straight line that is genuinely runnable
# headlessly on the booted iOS simulator, and records the legs that are NOT
# runnable as TYPED BLOCKED skips (never fake passes) in a receipt JSON:
#
#   leg ios_signed_in_thread_smoke   — SignedInThreadSmoke.yaml (sign-in
#                                      resolves, seeded thread opens, lane
#                                      picker visible, message sends+renders)
#   leg ios_repo_picker_reachable    — RepoPickerReachable.yaml (repo chip
#                                      opens the real RepoPickerScreen)
#   leg ios_dispatch_reply           — SignedInThreadReply.yaml (send →
#                                      hosted_khala answers on-screen)
#   leg ios_repo_pick_fork_bind      — StraightLineRepoPick.yaml, ONLY when
#                                      the mobile-user-session gate is open
#                                      (probed live against
#                                      GET /api/mobile/repos); otherwise
#                                      BLOCKED: the repo list is
#                                      mobile-OpenAuth-USER-session-only by
#                                      documented invariant and the seeded
#                                      agent token 401s it.
#   leg push_writeback               — ALWAYS BLOCKED until CX-3's in-VM
#                                      cloud-execution lane exists (#8547).
#                                      Recorded, never run, never faked.
#   leg credits_grant_visible_drain  — BLOCKED with the same mobile-user-
#                                      session gate as the repo list
#                                      (GET /api/mobile/credits/balance).
#
# The typed leg registry lives in src/qa/straight-line-e2e.ts and is guarded
# by tests/straight-line-e2e.test.ts; this runner emits receipts against that
# registry's leg ids.
#
# Preconditions:
#   - A Release KhalaCode.app baked with the CURRENT AgentFlampy seeded creds
#     is installed on the target booted simulator (scripts/build-seeded-ios.sh).
#   - `maestro`, JDK 17, and the seeded env `~/work/.secrets/khala-maestro.env`.
#
# NEVER commit `~/work/.secrets/khala-maestro.env`; it holds the oa_agent_
# token. This script never prints token material.
#
# Usage: bash scripts/straight-line-e2e-run.sh [sim-udid]
set -euo pipefail

SIM_UDID="${1:-2E5DFC26-DB79-4EE2-BF8E-2EB486A1AFBA}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECRET="${HOME}/work/.secrets/khala-maestro.env"
SESSION_SECRET="${HOME}/work/.secrets/khala-mobile-session.env"
BASE_URL="${OPENAGENTS_BASE_URL:-https://openagents.com}"
REPO_FULL_NAME="${KHALA_MAESTRO_REPO_FULL_NAME:-AgentFlampy/openagents}"
RECEIPT_DIR="${KHALA_STRAIGHT_LINE_RECEIPT_DIR:-${HERE}/var/straight-line-e2e}"
RECEIPT="${RECEIPT_DIR}/straight-line-e2e.latest.json"

if [[ ! -f "$SECRET" ]]; then
  echo "ERROR: missing $SECRET (seeded AgentFlampy Maestro creds)." >&2
  exit 1
fi
# shellcheck disable=SC1090
set -a; source "$SECRET"; set +a

: "${JAVA_HOME:=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home}"
export JAVA_HOME
export PATH="${JAVA_HOME}/bin:${HOME}/.maestro/bin:${PATH}"
export MAESTRO_CLI_NO_ANALYTICS=1

mkdir -p "$RECEIPT_DIR"

# ---------------------------------------------------------------------------
# Gate probe: is a real mobile OpenAuth USER session available? The repo-list
# and credits legs are mobile-user-session-only by documented invariant
# (docs/khala-code/receipts/2026-07-07-qam-4-populated-happy-path.md); the
# seeded agent token 401s them BY DESIGN. A real session token, if the
# one-time owner GitHub OAuth has been captured, lives in $SESSION_SECRET.
# ---------------------------------------------------------------------------
USER_SESSION_GATE="blocked"
if [[ -f "$SESSION_SECRET" ]]; then
  # shellcheck disable=SC1090
  set -a; source "$SESSION_SECRET"; set +a
  status=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "authorization: Bearer ${KHALA_MOBILE_SESSION_TOKEN:-}" \
    "${BASE_URL}/api/mobile/repos?page=1&perPage=1" || echo "000")
  if [[ "$status" == "200" ]]; then USER_SESSION_GATE="open"; fi
fi
echo "==> mobile user-session gate: ${USER_SESSION_GATE}"

# ---------------------------------------------------------------------------
# Seeded-thread turn-state reset (same fix as the smoke runner: a leftover
# queued/active turn hides the lane picker; see issue #8539).
# ---------------------------------------------------------------------------
echo "==> resetting seeded thread turn state"
python3 - "$BASE_URL" "$KHALA_MAESTRO_TOKEN" "$KHALA_MAESTRO_THREAD_ID" <<'PY'
import json, sys, time, urllib.request, datetime
base, token, thread = sys.argv[1], sys.argv[2], sys.argv[3]
reset_client = "straightline-reset-" + str(int(time.time() * 1000))
def post(path, body):
    req = urllib.request.Request(
        base + path,
        data=json.dumps(body).encode(),
        headers={"authorization": "Bearer " + token, "content-type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req) as r:
        return json.load(r)
snap = post("/api/sync/bootstrap", {
    "clientGroupId": "straightline-reset", "protocolVersion": 1, "schemaVersion": 1,
    "scope": "scope.thread." + thread,
})
active = [json.loads(e["postImageJson"]) for e in snap.get("entities", [])
          if e["entityType"] == "runtime_turn"
          and json.loads(e["postImageJson"]).get("status") in ("queued", "running")]
if not active:
    print("   no active turns to reset")
now = datetime.datetime.utcnow().isoformat() + "Z"
for i, turn in enumerate(active, start=1):
    tid = turn["turnId"]
    intent = {
        "schema": "openagents.khala_runtime_control_intent.v1",
        "intentId": "intent.close." + tid, "kind": "turn.close",
        "threadId": thread, "turnId": tid, "createdAt": now,
        "origin": {"lane": "khala_sync_mobile_control", "surface": "mobile"},
        "target": {"lane": turn.get("lane", "hosted_khala")},
        "visibility": "private", "redactionClass": "private_ref",
        "idempotencyKey": "idem.close." + tid, "causalityRefs": [],
    }
    res = post("/api/sync/push", {
        "protocolVersion": 1, "schemaVersion": 1,
        "clientGroupId": "straightline-reset", "clientId": reset_client,
        "mutations": [{"mutationId": i, "name": "runtime.closeTurn",
                       "argsJson": json.dumps(intent)}],
    })
    print("   closed", tid, "->", res.get("results"))
PY

# ---------------------------------------------------------------------------
# Fresh per-run thread: the long-lived seeded thread accumulates transcript
# rows (turn-status chips from every prior run and headless probe), which
# pushes a freshly sent bubble off-screen and flakes the visibility oracles.
# chat.createThread is cheap and owner-scoped, so each run drives a brand-new
# thread with a unique short title; the flows receive it via
# KHALA_MAESTRO_THREAD_TITLE exactly as before.
# ---------------------------------------------------------------------------
RUN_STAMP="$(date -u +%H%M%S)"
RUN_EPOCH="$(date -u +%s)"
# Two fresh threads: the smoke leg SENDS (leaving an active hosted_khala turn
# behind for up to ~1 min), so the reply leg gets its own thread — otherwise
# its composer would render the active-turn Follow up/Steer state instead of
# the idle "Message" input the flow drives. The reachable leg only opens the
# repo picker and shares the smoke thread.
SMOKE_THREAD_TITLE="SL smoke ${RUN_STAMP}"
REPLY_THREAD_TITLE="SL reply ${RUN_STAMP}"
create_thread() {
  local thread_id="$1" title="$2"
  python3 - "$BASE_URL" "$KHALA_MAESTRO_TOKEN" "$thread_id" "$title" <<'PY'
import json, sys, time, urllib.request
base, token, thread_id, title = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
req = urllib.request.Request(
    base + "/api/sync/push",
    data=json.dumps({
        "protocolVersion": 1, "schemaVersion": 1,
        "clientGroupId": "straightline-seed",
        "clientId": "straightline-seed-" + str(int(time.time() * 1000)),
        "mutations": [{"mutationId": 1, "name": "chat.createThread",
                       "argsJson": json.dumps({"threadId": thread_id, "title": title})}],
    }).encode(),
    headers={"authorization": "Bearer " + token, "content-type": "application/json"},
    method="POST",
)
with urllib.request.urlopen(req) as r:
    res = json.load(r)
status = res.get("results", [{}])[0].get("status")
if status != "applied":
    raise SystemExit(f"fresh thread create failed: {res}")
print("   created", thread_id, "->", status)
PY
}
echo "==> creating fresh run threads"
create_thread "thread.sle2esmoke${RUN_EPOCH}" "$SMOKE_THREAD_TITLE"
create_thread "thread.sle2ereply${RUN_EPOCH}" "$REPLY_THREAD_TITLE"

FLOWS_DIR="${HERE}/.maestro/flows"
declare -a LEG_IDS=() LEG_STATUSES=() LEG_DETAILS=()

run_leg() {
  local leg_id="$1" flow="$2" thread_title="$3"
  echo "==> leg ${leg_id}: running ${flow} against '${thread_title}'"
  if maestro --device "${SIM_UDID}" test \
      -e MAESTRO_APP_ID=com.openagents.khala.mobile \
      -e KHALA_MAESTRO_OWNER_USER_ID="${KHALA_MAESTRO_OWNER_USER_ID}" \
      -e KHALA_MAESTRO_TOKEN="${KHALA_MAESTRO_TOKEN}" \
      -e KHALA_MAESTRO_THREAD_TITLE="${thread_title}" \
      -e KHALA_MAESTRO_REPO_FULL_NAME="${REPO_FULL_NAME}" \
      "${FLOWS_DIR}/${flow}"; then
    LEG_IDS+=("$leg_id"); LEG_STATUSES+=("PASS"); LEG_DETAILS+=("${flow}")
  else
    LEG_IDS+=("$leg_id"); LEG_STATUSES+=("FAIL"); LEG_DETAILS+=("${flow}")
    OVERALL_FAILED=1
  fi
}

skip_leg() {
  local leg_id="$1" blocker="$2"
  echo "==> leg ${leg_id}: BLOCKED (${blocker})"
  LEG_IDS+=("$leg_id"); LEG_STATUSES+=("BLOCKED"); LEG_DETAILS+=("$blocker")
}

OVERALL_FAILED=0

run_leg "ios_signed_in_thread_smoke" "SignedInThreadSmoke.yaml" "$SMOKE_THREAD_TITLE"
run_leg "ios_repo_picker_reachable" "RepoPickerReachable.yaml" "$SMOKE_THREAD_TITLE"
run_leg "ios_dispatch_reply" "SignedInThreadReply.yaml" "$REPLY_THREAD_TITLE"

if [[ "$USER_SESSION_GATE" == "open" ]]; then
  run_leg "ios_repo_pick_fork_bind" "StraightLineRepoPick.yaml" "$SMOKE_THREAD_TITLE"
  # The grant-visible half is provable through the QAM-4 populated visual lane
  # once a session exists; the DRAIN assertion still has no in-app flow.
  skip_leg "credits_grant_visible_drain" \
    "blocker.khala_mobile.credits_drain_assertion_flow_not_built"
else
  skip_leg "ios_repo_pick_fork_bind" \
    "blocker.khala_mobile.repo_list_requires_github_backed_mobile_session"
  skip_leg "credits_grant_visible_drain" \
    "blocker.khala_mobile.credits_routes_require_github_backed_mobile_session"
fi

skip_leg "push_writeback" "blocker.cx3.in_vm_cloud_execution_lane_missing.openagents#8547"

# ---------------------------------------------------------------------------
# Typed receipt (public-safe: leg ids, statuses, blocker refs, flow names —
# no tokens, no chat bodies, no machine identifiers beyond the sim UDID).
# ---------------------------------------------------------------------------
{
  printf '{\n'
  printf '  "schema": "openagents.khala_mobile.straight_line_e2e_receipt.v1",\n'
  printf '  "issue": 8543,\n'
  printf '  "generatedAt": "%s",\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf '  "platform": "ios-simulator",\n'
  printf '  "simUdid": "%s",\n' "$SIM_UDID"
  printf '  "seedAccount": "AgentFlampy",\n'
  printf '  "seedRepo": "%s",\n' "$REPO_FULL_NAME"
  printf '  "mobileUserSessionGate": "%s",\n' "$USER_SESSION_GATE"
  printf '  "legs": [\n'
  local_n=${#LEG_IDS[@]}
  for i in "${!LEG_IDS[@]}"; do
    sep=","; [[ $((i + 1)) -eq $local_n ]] && sep=""
    printf '    {"id": "%s", "status": "%s", "detail": "%s"}%s\n' \
      "${LEG_IDS[$i]}" "${LEG_STATUSES[$i]}" "${LEG_DETAILS[$i]}" "$sep"
  done
  printf '  ]\n'
  printf '}\n'
} > "$RECEIPT"

echo "==> receipt written: ${RECEIPT}"
cat "$RECEIPT"

exit "$OVERALL_FAILED"
