#!/usr/bin/env bash
# Owner-authorized repeatable runner for the Khala Mobile iOS visual tier
# (QAM-4, #8539). It drives a Maestro flow that reaches signed-in product
# screens, captures a `takeScreenshot` checkpoint per screen, and then either
# blesses those captures into the owned `openagents.khala_visual_baselines.v1`
# manifest (default) or verifies them against the committed baseline
# (`--verify`, the nightly regression check — fails on any changed/missing).
#
# Default flow (SignedInScreensVisual) captures Settings, Credit history, and
# the repo picker, and REQUIRES an installed Release KhalaCode.app baked with
# the seeded public-safe AgentFlampy creds (build with the
# emulator-test-run.sh pattern pointing .env.local at KHALA_MAESTRO_*). It first
# resets the seeded thread's turn state, exactly like
# signed-in-thread-smoke-run.sh, so opening the thread is deterministic.
#
# The onboarding first-run screen lives only on an empty thread list, so it is
# captured by pointing MOBILE_VISUAL_FLOW at OnboardingFirstRunVisual against a
# Release build baked with a public-safe ZERO-THREAD account instead
# (KHALA_MOBILE_TEST_* — see scripts/emulator-test-run.sh).
#
# Usage:
#   bash scripts/mobile-visual-tier-run.sh [--verify] [sim-udid]
# Env:
#   MOBILE_VISUAL_FLOW   flow basename under .maestro/flows (default SignedInScreensVisual)
#   MOBILE_VISUAL_REPORT report path (default docs/.../2026-07-07-qam-4-ios-signed-in-screens.json)
#   MOBILE_VISUAL_RESET  set to 0 to skip the seeded-thread turn reset
#
# NEVER commit ~/work/.secrets/khala-maestro.env (holds the oa_agent_ token).
set -euo pipefail

VERIFY=""
if [[ "${1:-}" == "--verify" ]]; then VERIFY="--verify"; shift; fi
SIM_UDID="${1:-2E5DFC26-DB79-4EE2-BF8E-2EB486A1AFBA}"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "${HERE}/../.." && pwd)"
SECRET="${HOME}/work/.secrets/khala-maestro.env"
BASE_URL="${OPENAGENTS_BASE_URL:-https://openagents.com}"
FLOW_NAME="${MOBILE_VISUAL_FLOW:-SignedInScreensVisual}"
FLOW="${HERE}/.maestro/flows/${FLOW_NAME}.yaml"
REPORT="${MOBILE_VISUAL_REPORT:-${REPO_ROOT}/docs/khala-code/receipts/2026-07-07-qam-4-ios-signed-in-screens.json}"

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

if [[ "${MOBILE_VISUAL_RESET:-1}" == "1" && "$FLOW_NAME" == "SignedInScreensVisual" ]]; then
  echo "==> resetting seeded thread turn state"
  python3 - "$BASE_URL" "$KHALA_MAESTRO_TOKEN" "$KHALA_MAESTRO_THREAD_ID" <<'PY'
import json, sys, time, urllib.request, datetime
base, token, thread = sys.argv[1], sys.argv[2], sys.argv[3]
def post(path, body):
    req = urllib.request.Request(base + path, data=json.dumps(body).encode(),
        headers={"authorization": "Bearer " + token, "content-type": "application/json"}, method="POST")
    with urllib.request.urlopen(req) as r:
        return json.load(r)
snap = post("/api/sync/bootstrap", {"clientGroupId": "maestro-reset", "protocolVersion": 1,
    "schemaVersion": 1, "scope": "scope.thread." + thread})
active = [json.loads(e["postImageJson"]) for e in snap.get("entities", [])
          if e["entityType"] == "runtime_turn"
          and json.loads(e["postImageJson"]).get("status") in ("queued", "running")]
now = datetime.datetime.utcnow().isoformat() + "Z"
reset_client = "maestro-reset-client-" + str(int(time.time() * 1000))
for i, turn in enumerate(active, start=1):
    tid = turn["turnId"]
    intent = {"schema": "openagents.khala_runtime_control_intent.v1",
        "intentId": "intent.close." + tid, "kind": "turn.close", "threadId": thread,
        "turnId": tid, "createdAt": now,
        "origin": {"lane": "khala_sync_mobile_control", "surface": "mobile"},
        "target": {"lane": turn.get("lane", "hosted_khala")}, "visibility": "private",
        "redactionClass": "private_ref", "idempotencyKey": "idem.close." + tid, "causalityRefs": []}
    res = post("/api/sync/push", {"protocolVersion": 1, "schemaVersion": 1,
        "clientGroupId": "maestro-reset", "clientId": reset_client,
        "mutations": [{"mutationId": i, "name": "runtime.closeTurn", "argsJson": json.dumps(intent)}]})
    print("   closed", tid, "->", res.get("results"))
PY
fi

CAND="$(mktemp -d)/khala-visual-candidates"
mkdir -p "$CAND"

echo "==> running ${FLOW_NAME} on ${SIM_UDID} (captures -> ${CAND})"
( cd "$CAND" && maestro --device "${SIM_UDID}" test \
  -e MAESTRO_APP_ID=com.openagents.khala.mobile \
  -e KHALA_MAESTRO_OWNER_USER_ID="${KHALA_MAESTRO_OWNER_USER_ID}" \
  -e KHALA_MAESTRO_TOKEN="${KHALA_MAESTRO_TOKEN}" \
  -e KHALA_MAESTRO_THREAD_TITLE="${KHALA_MAESTRO_THREAD_TITLE}" \
  "${FLOW}" )

echo "==> ${VERIFY:-bless} captures into the baseline engine"
( cd "$REPO_ROOT" && bun packages/khala-qa-harness/src/bless-ios-mobile-visual-baselines.ts "$CAND" "$REPORT" ${VERIFY} )
