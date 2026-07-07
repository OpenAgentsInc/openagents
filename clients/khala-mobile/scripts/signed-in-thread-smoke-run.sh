#!/usr/bin/env bash
# Owner-authorized repeatable runner for the Khala Mobile SignedInThreadSmoke
# Maestro flow (issue #8510). It:
#   1. resets the seeded thread's turn state (closes any active/queued runtime
#      turn) so the composer renders its lane picker deterministically — a
#      previous run's send leaves a `queued` turn behind (no Pylon processes the
#      hosted_khala lane for this public-safe test account), which otherwise
#      flips the composer to the active-turn (Steer/Queue) state and makes the
#      lane-picker assertion flake;
#   2. runs SignedInThreadSmoke.yaml against a booted iOS Simulator using the
#      seeded public-safe AgentFlampy credentials in
#      `~/work/.secrets/khala-maestro.env`.
#
# Preconditions:
#   - A Release-configuration KhalaCode.app is installed on the target booted
#     simulator with EXPO_PUBLIC_KHALA_SYNC_DEMO_* baked to the AgentFlampy
#     seeded creds (build with clients/khala-mobile/scripts/emulator-test-run.sh
#     pattern, pointing .env.local at KHALA_MAESTRO_OWNER_USER_ID/TOKEN).
#   - `maestro` is installed (curl -Ls https://get.maestro.mobile.dev | bash).
#   - A JDK 17 is available (Homebrew openjdk@17).
#   - The seeded thread (KHALA_MAESTRO_THREAD_ID) exists on the account.
#
# NEVER commit `~/work/.secrets/khala-maestro.env`; it holds the oa_agent_ token.
#
# Usage: bash scripts/signed-in-thread-smoke-run.sh [sim-udid]
#        default sim: iPhone 17 Pro 2E5DFC26-DB79-4EE2-BF8E-2EB486A1AFBA
set -euo pipefail

SIM_UDID="${1:-2E5DFC26-DB79-4EE2-BF8E-2EB486A1AFBA}"
SECRET="${HOME}/work/.secrets/khala-maestro.env"
BASE_URL="${OPENAGENTS_BASE_URL:-https://openagents.com}"
FLOW="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/.maestro/flows/SignedInThreadSmoke.yaml"

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

echo "==> resetting seeded thread turn state (close any active/queued turn)"
python3 - "$BASE_URL" "$KHALA_MAESTRO_TOKEN" "$KHALA_MAESTRO_THREAD_ID" <<'PY'
import json, sys, urllib.request, datetime
base, token, thread = sys.argv[1], sys.argv[2], sys.argv[3]
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
    "clientGroupId": "maestro-reset", "protocolVersion": 1, "schemaVersion": 1,
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
        "clientGroupId": "maestro-reset", "clientId": "maestro-reset-client",
        "mutations": [{"mutationId": i, "name": "runtime.closeTurn",
                       "argsJson": json.dumps(intent)}],
    })
    print("   closed", tid, "->", res.get("results"))
PY

echo "==> running SignedInThreadSmoke on ${SIM_UDID}"
maestro --device "${SIM_UDID}" test \
  -e MAESTRO_APP_ID=com.openagents.khala.mobile \
  -e KHALA_MAESTRO_OWNER_USER_ID="${KHALA_MAESTRO_OWNER_USER_ID}" \
  -e KHALA_MAESTRO_TOKEN="${KHALA_MAESTRO_TOKEN}" \
  -e KHALA_MAESTRO_THREAD_TITLE="${KHALA_MAESTRO_THREAD_TITLE}" \
  "${FLOW}" ${MAESTRO_EXTRA_ARGS:-}
