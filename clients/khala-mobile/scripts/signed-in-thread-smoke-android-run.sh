#!/usr/bin/env bash
# Owner-authorized repeatable runner for the Khala Mobile SignedInThreadSmoke
# Maestro flow on a real Android emulator (QAM-6, issue #8541). It is the Android
# analog of scripts/signed-in-thread-smoke-run.sh (iOS).
#
# WHY A DEDICATED SCRIPT: the app has no manual-sign-in UI, so the flow's
# "Sign in manually instead" block is a dead fallback on Android too. The honest
# way to reach the signed-in thread list is to AUTO-SIGN-IN from a build baked
# with the seeded public-safe account via EXPO_PUBLIC_KHALA_SYNC_DEMO_* (same
# devEnvCredentials path the iOS emulator-test-run.sh uses). This runner:
#   1. writes a TEMPORARY .env.local baking the AgentFlampy seeded creds from
#      ~/work/.secrets/khala-maestro.env, then builds a Release APK with a fresh
#      JS bundle (metro cache reset) so the creds actually inline;
#   2. ALWAYS removes .env.local on exit (trap) so no shippable/AAB build can
#      ever carry the token;
#   3. installs the baked APK on the booted emulator;
#   4. resets the seeded thread's turn state (closes any active/queued turn) so
#      the composer renders its lane picker deterministically;
#   5. runs SignedInThreadSmoke.yaml and captures an adb screencap.
#
# SAFETY: NEVER commit ~/work/.secrets/khala-maestro.env or .env.local; they hold
# the oa_agent_ token. This script prints neither.
#
# Preconditions:
#   - A booted Android emulator (see scripts/android-emulator-test-run.sh for AVD
#     bring-up), `adb`, a JDK 17 (Homebrew openjdk@17), and `maestro` on PATH.
#   - The Homebrew Android SDK at /opt/homebrew/share/android-commandlinetools.
#   - The seeded thread (KHALA_MAESTRO_THREAD_TITLE) exists on the account.
#
# Usage: bash scripts/signed-in-thread-smoke-android-run.sh [adb-serial]
#        default serial: emulator-5554
set -euo pipefail

SERIAL="${1:-emulator-5554}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECRET="${HOME}/work/.secrets/khala-maestro.env"
BASE_URL="${OPENAGENTS_BASE_URL:-https://openagents.com}"
ENV_LOCAL="${HERE}/.env.local"
SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}}"
JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home}"
APP_ID="com.openagents.khala.mobile"
APK="${HERE}/android/app/build/outputs/apk/release/app-release.apk"
ARTIFACT_DIR="${KHALA_ANDROID_ARTIFACT_DIR:-${HERE}/var/android-emulator-qam-6}"

export ANDROID_HOME="$SDK_ROOT"
export ANDROID_SDK_ROOT="$SDK_ROOT"
export JAVA_HOME
export MAESTRO_APP_ID="$APP_ID"
export MAESTRO_CLI_NO_ANALYTICS=1
export PATH="$JAVA_HOME/bin:$SDK_ROOT/platform-tools:$HOME/.maestro/bin:$PATH"

if [[ ! -f "$SECRET" ]]; then
  echo "ERROR: missing $SECRET (seeded AgentFlampy Maestro creds)." >&2
  exit 1
fi

cleanup() {
  # ALWAYS remove .env.local so a later archive/AAB can never bake test creds.
  rm -f "$ENV_LOCAL"
  echo "cleaned up .env.local"
}
trap cleanup EXIT

# shellcheck disable=SC1090
set -a; source "$SECRET"; set +a

mkdir -p "$ARTIFACT_DIR"

echo "==> baking seeded AgentFlampy creds into a Release APK (.env.local, removed on exit)"
export EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID="${KHALA_MAESTRO_OWNER_USER_ID}"
export EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN="${KHALA_MAESTRO_TOKEN}"
cat > "$ENV_LOCAL" <<EOF
# TEMPORARY — written by signed-in-thread-smoke-android-run.sh, removed on exit.
EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID=${KHALA_MAESTRO_OWNER_USER_ID}
EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN=${KHALA_MAESTRO_TOKEN}
EOF

# Reset metro transform cache + generated bundle so the baked env actually inlines.
rm -rf "${TMPDIR}/metro-cache" "${TMPDIR}"/metro-file-map-expo-* 2>/dev/null || true
rm -rf "${HERE}/../../node_modules/.cache" 2>/dev/null || true
rm -f "${HERE}/android/app/build/generated/assets/react/release/index.android.bundle" 2>/dev/null || true

( cd "$HERE" && ./android/gradlew -p android :app:createBundleReleaseJsAndAssets :app:assembleRelease --no-daemon --rerun-tasks )

if [[ ! -f "$APK" ]]; then
  echo "ERROR: missing Release APK at $APK" >&2
  exit 1
fi

echo "==> verifying baked creds landed in the JS bundle (no token printed)"
# NOTE: pipe through `strings` — raw BSD grep on Hermes bytecode is
# unreliable (a contiguous string the `strings` scan finds can still fail a
# raw binary grep), which produced a false "did not inline" failure on
# 2026-07-09 even though the bundle was correctly baked.
if ! unzip -p "$APK" assets/index.android.bundle 2>/dev/null | strings | grep -qF "${KHALA_MAESTRO_OWNER_USER_ID}"; then
  echo "ERROR: baked creds did not inline into the bundle (metro cache?)." >&2
  exit 1
fi

echo "==> installing baked Release APK on ${SERIAL}"
adb -s "$SERIAL" uninstall "$APP_ID" >/dev/null 2>&1 || true
adb -s "$SERIAL" install -r "$APK"

echo "==> resetting seeded thread turn state (close any active/queued turn)"
python3 - "$BASE_URL" "$KHALA_MAESTRO_TOKEN" "$KHALA_MAESTRO_THREAD_ID" <<'PY'
import json, sys, time, urllib.request, datetime
base, token, thread = sys.argv[1], sys.argv[2], sys.argv[3]
reset_client = "maestro-android-reset-" + str(int(time.time() * 1000))
def post(path, body):
    req = urllib.request.Request(
        base + path, data=json.dumps(body).encode(),
        headers={"authorization": "Bearer " + token, "content-type": "application/json"},
        method="POST")
    with urllib.request.urlopen(req) as r:
        return json.load(r)
snap = post("/api/sync/bootstrap", {
    "clientGroupId": "maestro-reset", "protocolVersion": 1, "schemaVersion": 1,
    "scope": "scope.thread." + thread})
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
        "idempotencyKey": "idem.close." + tid, "causalityRefs": []}
    res = post("/api/sync/push", {
        "protocolVersion": 1, "schemaVersion": 1,
        "clientGroupId": "maestro-reset", "clientId": reset_client,
        "mutations": [{"mutationId": i, "name": "runtime.closeTurn",
                       "argsJson": json.dumps(intent)}]})
    print("   closed", tid, "->", res.get("results"))
PY

# Fresh per-run thread (same fix as scripts/straight-line-e2e-run.sh, issue
# #8543): the long-lived seeded thread accumulates turn-status transcript rows
# from every prior run/probe, which pushes the freshly sent bubble off-screen
# and flakes the final visibility assert. Each run drives a brand-new thread.
RUN_THREAD_TITLE="SL android $(date -u +%H%M%S)"
echo "==> creating fresh run thread '${RUN_THREAD_TITLE}'"
python3 - "$BASE_URL" "$KHALA_MAESTRO_TOKEN" "thread.slandroid$(date -u +%s)" "$RUN_THREAD_TITLE" <<'PY'
import json, sys, time, urllib.request
base, token, thread_id, title = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
req = urllib.request.Request(
    base + "/api/sync/push",
    data=json.dumps({
        "protocolVersion": 1, "schemaVersion": 1,
        "clientGroupId": "straightline-seed",
        "clientId": "android-seed-" + str(int(time.time() * 1000)),
        "mutations": [{"mutationId": 1, "name": "chat.createThread",
                       "argsJson": json.dumps({"threadId": thread_id, "title": title})}],
    }).encode(),
    headers={"authorization": "Bearer " + token, "content-type": "application/json"},
    method="POST")
with urllib.request.urlopen(req) as r:
    res = json.load(r)
status = res.get("results", [{}])[0].get("status")
if status != "applied":
    raise SystemExit(f"fresh thread create failed: {res}")
print("   created ->", status)
PY

echo "==> running SignedInThreadSmoke on ${SERIAL}"
maestro --device "${SERIAL}" test \
  -e MAESTRO_APP_ID="${APP_ID}" \
  -e KHALA_MAESTRO_OWNER_USER_ID="${KHALA_MAESTRO_OWNER_USER_ID}" \
  -e KHALA_MAESTRO_TOKEN="${KHALA_MAESTRO_TOKEN}" \
  -e KHALA_MAESTRO_THREAD_TITLE="${RUN_THREAD_TITLE}" \
  "${HERE}/.maestro/flows/SignedInThreadSmoke.yaml" ${MAESTRO_EXTRA_ARGS:-}

adb -s "$SERIAL" exec-out screencap -p > "${ARTIFACT_DIR}/signed-in-thread-smoke.png"
echo "==> captured ${ARTIFACT_DIR}/signed-in-thread-smoke.png"
