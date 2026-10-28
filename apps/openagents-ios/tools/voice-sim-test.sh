#!/usr/bin/env bash
# Voice in the simulator, end to end: build the app, start the voice host,
# boot a simulator and run Tests/VoiceUITests.swift, which turns voice on in
# the Grid with a bundled recording and checks the transcript, the reply and
# the level indicator, then talks over the reply (wait-stop) and checks that
# playback is cut, frame decisions are applied and the new turn is answered.
#
#   apps/openagents-ios/tools/voice-sim-test.sh            # offline host, then live if credentials work
#   apps/openagents-ios/tools/voice-sim-test.sh --offline  # offline host only
#   apps/openagents-ios/tools/voice-sim-test.sh --live     # live host only
#
# Live runs twice with Google speech: with the echo agent (the reply is
# checked word for word) and with Gemini (a non-empty reply that isn't the
# echo, played until SPEECH_END).
#
# Environment: VOICE_SIM (simulator name, default "Voice iPhone"),
# VOICE_PORT (default 8797, so a host already on 8787 is left alone),
# VOICE_FIXTURE (default hello-foreman), VOICE_EXPECT (words the live
# transcript must contain; default "foreman"), VOICE_PLAYBACK_DELAY (seconds
# of simulated output latency; default 0.25).
#
# Every reply is also checked the way a listener hears it: audio starts within
# 1.5 s of the reply arriving, plays in about its own length (not stalled, not
# played twice), the player never runs dry mid-reply, and the caption never
# loses words.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
repo="$(cd "$here/../.." && pwd)"
mode="${1:-all}"
sim="${VOICE_SIM:-Voice iPhone}"
port="${VOICE_PORT:-8797}"
fixture="${VOICE_FIXTURE:-hello-foreman}"
expect="${VOICE_EXPECT:-foreman}"
# A phone's output latency (voice processing, Bluetooth), which the simulator
# lacks: played-back callbacks arrive this many seconds late, as on a device.
delay="${VOICE_PLAYBACK_DELAY:-0.25}"
logs="$here/build/voice-sim"
mkdir -p "$logs"

# The host agent's recordings are the app's test recordings.
if compgen -G "$repo/verse/voice-host/fixtures/*.wav" > /dev/null; then
  cp "$repo"/verse/voice-host/fixtures/*.wav "$here/OpenAgents/Voice/Fixtures/"
fi

echo "== build"
"$here/build.sh" -quiet > "$logs/build.log" 2>&1 || { tail -40 "$logs/build.log"; exit 1; }

# A simulator of our own, so other runs on the shared one aren't disturbed.
udid="$(xcrun simctl list devices available | sed -n "s/^ *$sim (\([0-9A-F-]*\)).*/\1/p" | head -1)"
if [[ -z "$udid" ]]; then
  type="$(xcrun simctl list devicetypes | sed -n 's/^iPhone [0-9][0-9]* (\(com.apple[^)]*\))$/\1/p' | sort -t- -k2 -n | tail -1)"
  runtime="$(xcrun simctl list runtimes available | sed -n 's/^iOS .* - \(com.apple.CoreSimulator.SimRuntime.iOS-[0-9-]*\).*/\1/p' | tail -1)"
  udid="$(xcrun simctl create "$sim" "$type" "$runtime")"
fi
xcrun simctl boot "$udid" 2>/dev/null || true
xcrun simctl bootstatus "$udid" -b > /dev/null

host_pid=""
stop_host() { if [[ -n "$host_pid" ]]; then kill "$host_pid" 2>/dev/null || true; wait "$host_pid" 2>/dev/null || true; fi; host_pid=""; }
trap stop_host EXIT

start_host() { # $1 = log name, rest = run.sh flags
  local name="$1"; shift
  "$repo/verse/voice-host/run.sh" --listen "127.0.0.1:$port" "$@" > "$logs/host-$name.log" 2>&1 &
  host_pid=$!
  for _ in $(seq 1 300); do
    if nc -z 127.0.0.1 "$port" 2>/dev/null; then return 0; fi
    if ! kill -0 "$host_pid" 2>/dev/null; then echo "voice host exited:"; tail -20 "$logs/host-$name.log"; return 1; fi
    sleep 0.2
  done
  echo "voice host never listened on $port"; return 1
}

run_test() { # $1 = name, $2 = expected words, $3 = agent (echo|gemini)
  local name="$1"
  echo "== test ($name)"
  rm -rf "$logs/$name.xcresult"
  set +e
  TEST_RUNNER_OA_VOICE_HOST="ws://127.0.0.1:$port" \
  TEST_RUNNER_OA_VOICE_FIXTURE="$fixture" \
  TEST_RUNNER_OA_VOICE_EXPECT="$2" \
  TEST_RUNNER_OA_VOICE_AGENT="${3:-echo}" \
  TEST_RUNNER_OA_VOICE_PLAYBACK_DELAY="$delay" \
  xcodebuild -project "$here/OpenAgents.xcodeproj" -scheme OpenAgents \
    -destination "platform=iOS Simulator,id=$udid" \
    -derivedDataPath "$here/build" CODE_SIGNING_ALLOWED=NO \
    -parallel-testing-enabled NO -resultBundlePath "$logs/$name.xcresult" \
    -only-testing:OpenAgentsUITests/VoiceUITests test > "$logs/test-$name.log" 2>&1
  local status=$?
  set -e
  grep -E "VoiceUITests (heard|reply|barge|playback)|Test Case|error:|failed|passed|\*\* TEST" "$logs/test-$name.log" | grep -v "^ *$" | tail -20
  return $status
}

status=0
if [[ "$mode" == all || "$mode" == --offline ]]; then
  start_host offline --offline
  run_test offline "" || status=1
  stop_host
fi
if [[ "$mode" == all || "$mode" == --live ]]; then
  if CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$repo/../.secrets/gcloud-sa-config}" gcloud auth print-access-token > /dev/null 2>&1; then
    start_host live --agent echo
    run_test live "$expect" echo || status=1
    stop_host
    # The real agent: any spoken reply that isn't an echo, played to its end.
    start_host live-gemini --agent gemini
    run_test live-gemini "$expect" gemini || status=1
    stop_host
  else
    echo "== live: skipped (no Google credentials on this Mac)"
  fi
fi
echo "logs: $logs"
exit $status
