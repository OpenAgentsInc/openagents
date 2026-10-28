#!/usr/bin/env bash
# End-to-end tests for the voice host and the dev client.
#   verse/voice-host/test.sh            offline cases, then one live Google case if credentials work
#   verse/voice-host/test.sh --offline  offline cases only
# Offline cases run the host with --provider local --agent echo on a random
# port and stream the committed fixtures through voice-client in real time.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
offline_only=false
[[ "${1:-}" == "--offline" ]] && offline_only=true
"$here/build.sh"
bin="$here/build"
fx="$here/fixtures"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/voice-test.XXXXXX")"
host_pid=""
cleanup() { [[ -n "$host_pid" ]] && kill "$host_pid" 2>/dev/null || true; rm -rf "$tmp"; }
trap cleanup EXIT
failures=0

start_host() {  # start_host NAME ARGS...: sets $port
  local name="$1"; shift
  [[ -n "$host_pid" ]] && { kill "$host_pid" 2>/dev/null || true; wait "$host_pid" 2>/dev/null || true; }
  "$bin/voice-host" --listen 127.0.0.1:0 "$@" > "$tmp/$name.host.out" 2> "$tmp/$name.host.err" &
  host_pid=$!
  for _ in $(seq 1 100); do
    if grep -q '^listening on' "$tmp/$name.host.out" 2>/dev/null; then break; fi
    sleep 0.05
  done
  port="$(sed -n 's/^listening on 127\.0\.0\.1:\([0-9]*\).*/\1/p' "$tmp/$name.host.out")"
  [[ -n "$port" ]] || { echo "host did not start:"; cat "$tmp/$name.host.err"; exit 1; }
}

# The client's latency from its TURN_END, and the host's breakdown.
latency_lines() {  # latency_lines CASE HOSTNAME
  sed -n 's/^latency /    client: /p' "$tmp/$1.log" || true
  sed -n 's/^voice-host: [^ ]* latency /    host:   /p' "$tmp/$2.host.err" || true
}

field() { sed -n "s/^summary.* $2=\([0-9]*\).*/\1/p" "$1"; }

expect_field() {  # expect_field LOG NAME OP VALUE
  local got; got="$(field "$1" "$2")"
  if [[ -z "$got" ]] || ! [ "$got" "$3" "$4" ]; then
    echo "    expected $2 $3 $4, got '${got}'"
    return 1
  fi
}

run_case() {  # run_case NAME CHECKS -- CLIENT ARGS...
  local name="$1"; shift
  local checks=()
  while [[ "$1" != "--" ]]; do checks+=("$1"); shift; done
  shift
  local log="$tmp/$name.log" ok=true
  if ! "$bin/voice-client" --url "ws://127.0.0.1:$port/" "$@" > "$log" 2>&1; then ok=false; fi
  for c in ${checks[@]+"${checks[@]}"}; do
    IFS=: read -r f op val <<< "$c"
    expect_field "$log" "$f" "-$op" "$val" || ok=false
  done
  if $ok; then echo "ok   $name"; else
    echo "FAIL $name"; sed 's/^/    | /' "$log" | tail -40
    failures=$((failures + 1))
  fi
}

echo "offline: --provider local --agent echo"
start_host offline --provider local --agent echo

health="$(curl -s "http://127.0.0.1:$port/health" || true)"
if [[ "$health" == "ok" ]]; then echo "ok   health"; else echo "FAIL health ($health)"; failures=$((failures + 1)); fi

run_case hello finals:eq:1 speeches:eq:1 speech_ends:eq:1 audio_in:gt:0 downlink_samples:gt:0 meters:gt:0 errors:eq:0 -- \
  --out "$tmp/hello-reply.wav" --expect "I heard" "$fx/hello-foreman.wav"
if [[ -s "$tmp/hello-reply.wav" ]]; then echo "ok   hello reply WAV written"; else echo "FAIL hello reply WAV"; failures=$((failures + 1)); fi

# A 350 ms pause inside a sentence must not end the turn (end of turn waits
# 700 ms by default), and a ~3 s turn gets PARTIALs from re-recognition.
run_case pause-is-one-turn finals:eq:1 partials:ge:1 speech_ends:eq:1 -- --expect "heard" "$fx/garden-pause.wav"

run_case observe features:gt:50 speech_ends:eq:1 -- --observe "$fx/hello-foreman.wav"

# Talking over the reply: the client's engine raises BARGE_IN, the host stops,
# bumps the epoch and reports INTERRUPTED, then answers the new turn.
run_case barge-in finals:eq:2 interrupted:ge:1 speech_ends:ge:1 -- --barge "$fx/wait-stop.wav" "$fx/hello-foreman.wav"

# An observer's FrameDecision is applied by the host's engine: -12 dB of
# downlink gain makes the reply at most half as loud as the plain run.
plain_peak="$(field "$tmp/hello.log" peak)"
run_case decision-gain speech_ends:eq:1 -- --decide-gain -12 "$fx/hello-foreman.wav"
quiet_peak="$(field "$tmp/decision-gain.log" peak)"
if [[ -n "$plain_peak" && -n "$quiet_peak" && $((quiet_peak * 2)) -le "$plain_peak" ]]; then
  echo "ok   decision-gain peak $quiet_peak vs $plain_peak"
else
  echo "FAIL decision-gain peak '$quiet_peak' vs '$plain_peak'"; failures=$((failures + 1))
fi

# Two sessions at once.
"$bin/voice-client" --url "ws://127.0.0.1:$port/" --speaker a --expect "heard" "$fx/hello-foreman.wav" > "$tmp/conc-a.log" 2>&1 &
pa=$!
"$bin/voice-client" --url "ws://127.0.0.1:$port/" --speaker b --expect "heard" "$fx/garden-pause.wav" > "$tmp/conc-b.log" 2>&1 &
pb=$!
ca=0; cb=0
wait "$pa" || ca=$?
wait "$pb" || cb=$?
if [[ $ca -eq 0 && $cb -eq 0 && "$(field "$tmp/conc-a.log" speech_ends)" == 1 && "$(field "$tmp/conc-b.log" speech_ends)" == 1 ]]; then
  echo "ok   two-sessions"
else
  echo "FAIL two-sessions (exit $ca, $cb)"; tail -5 "$tmp/conc-a.log" "$tmp/conc-b.log" | sed 's/^/    | /'
  failures=$((failures + 1))
fi

if ! $offline_only; then
  gconf="${OA_VOICE_GCLOUD_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}"
  if CLOUDSDK_CONFIG="$gconf" gcloud auth print-access-token > /dev/null 2>&1; then
    echo "live: --provider google --agent echo"
    start_host live --provider google --agent echo
    run_case live-google finals:eq:1 speech_ends:eq:1 errors:eq:0 -- \
      --out "$tmp/live-reply.wav" --expect "what are we building today" "$fx/hello-foreman.wav"
    latency_lines live-google live
    echo "live: --provider google --agent gemini"
    start_host live-gemini --provider google --agent gemini
    run_case live-gemini finals:eq:1 speeches:eq:1 speech_ends:eq:1 errors:eq:0 -- \
      --expect "what are we building today" "$fx/hello-foreman.wav"
    latency_lines live-gemini live-gemini
    run_case live-gemini-barge finals:eq:2 interrupted:ge:1 errors:eq:0 -- \
      --barge "$fx/wait-stop.wav" "$fx/hello-foreman.wav"
  else
    echo "skip live: no Google access token from $gconf"
  fi
fi

if [[ $failures -ne 0 ]]; then echo "$failures failure(s)"; exit 1; fi
echo "all voice host tests passed"
