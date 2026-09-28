#!/usr/bin/env bash
#
# Tests for `os/bin/dictate-toggle`, the key that opens the microphone and
# transcribes what it heard into the focused window.
#
# Run it by hand:
#
#   os/tests/dictate-toggle.sh
#
# There is no microphone, no speech model, and no compositor here, so every
# tool the script reaches for is a stub: `ffmpeg` is both the recorder that
# holds a microphone until it is signalled and the tone generator, `pw-play`
# sounds the tone or refuses to, `whisper-cli` answers with a fixed line,
# `wtype` and `notify-send` log what they were asked to say, and `curl` fails,
# because a test that fetches a speech model is a test reaching the network.
#
# Two kinds of test run here. The dispatch tests run the script as a command
# and read what it left behind, which is how the toggle's phase is checked
# against a record whose recorder is gone. The unit tests source it with
# `DICTATE_TOGGLE_LIB` set, which defines its functions and stops before the
# dispatch, and call `recording_live`, `record_pid`, and `duration_text`
# directly.
#
# The tests the toggle exists for are its phase and its visibility: one press
# starts a recording when nothing is recording, whatever the record file says;
# a capture a dead recorder left is kept rather than deleted; the
# capture is bounded so the microphone cannot stay open for six minutes nobody
# asked for; `status` answers a person and a script; and the tone's failure
# reaches stderr rather than `/dev/null`.
#
# Every process this file starts is signalled by a pid it recorded itself.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
script="$root/os/bin/dictate-toggle"

scratch=$(mktemp -d "${TMPDIR:-/tmp}/coder-dictate-toggle-test.XXXXXX")
# The pids this file starts, so the trap ends what it started and nothing
# else. A recorder stub holds a loop until it is signalled, and a stand-in
# process stands for a live recorder in the unit tests.
started=()
trap 'for pid in ${started[@]+"${started[@]}"}; do kill "$pid" 2>/dev/null || true; done;
      rm -rf "$scratch"' EXIT

failures=0
pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; failures=$((failures + 1)); }

stubs="$scratch/stubs"
mkdir -p "$stubs"

# The stub recorder and tone generator. A call carrying `lavfi` is the tone,
# which prints a few bytes and leaves. Any other call is the capture: it
# writes the file it was given and holds until it is signalled, the way a
# recorder holds a microphone. With `TEST_HONOR_LIMIT` set it instead ends
# when the `-t` it was given runs out, which is the bound the toggle asks for.
cat > "$stubs/ffmpeg" << 'STUB'
#!/usr/bin/env bash
set -uo pipefail
printf '%s\n' "$*" >> "$TEST_FFMPEG_LOG"
case "$*" in
  *lavfi*)
    printf 'RIFFtone'
    exit 0
    ;;
esac
out=${*: -1}
limit=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "-t" ]; then limit=$arg; fi
  prev=$arg
done
printf 'RIFF-capture-%s' "$$" > "$out"
trap 'exit 0' INT TERM
if [ -n "$limit" ] && [ "${TEST_HONOR_LIMIT:-0}" = "1" ]; then
  waited=0
  while [ "$waited" -lt "$limit" ]; do
    sleep 0.2
    waited=$((waited + 1))
  done
  exit 0
fi
while true; do sleep 0.05; done
STUB
chmod +x "$stubs/ffmpeg"

# The stub speaker. It reads the tone and reports whether it sounded.
cat > "$stubs/pw-play" << 'STUB'
#!/usr/bin/env bash
set -uo pipefail
if [ "${TEST_CUE_STATUS:-0}" != "0" ]; then
  echo "failed to connect to the sink" >&2
  exit "$TEST_CUE_STATUS"
fi
exit 0
STUB
chmod +x "$stubs/pw-play"

cat > "$stubs/whisper-cli" << 'STUB'
#!/usr/bin/env bash
set -uo pipefail
printf '%s\n' "${TEST_TRANSCRIPT-the words the microphone heard}"
STUB
chmod +x "$stubs/whisper-cli"

cat > "$stubs/wtype" << 'STUB'
#!/usr/bin/env bash
set -uo pipefail
printf '%s\n' "$*" >> "$TEST_WTYPE_LOG"
STUB
chmod +x "$stubs/wtype"

cat > "$stubs/notify-send" << 'STUB'
#!/usr/bin/env bash
set -uo pipefail
printf '%s\n' "$*" >> "$TEST_NOTIFY_LOG"
STUB
chmod +x "$stubs/notify-send"

cat > "$stubs/curl" << 'STUB'
#!/usr/bin/env bash
echo "the test reached for the network" >&2
exit 1
STUB
chmod +x "$stubs/curl"

export PATH="$stubs:$PATH"
export TEST_FFMPEG_LOG="$scratch/ffmpeg.log"
export TEST_WTYPE_LOG="$scratch/wtype.log"
export TEST_NOTIFY_LOG="$scratch/notify.log"

# The paths and the bound the script reads, so a run touches this scratch
# directory and not /tmp, and so the bound can be reached inside a test.
export CODEROS_DICTATION_PID="$scratch/record.pid"
export CODEROS_DICTATION_AUDIO="$scratch/audio.wav"
export CODEROS_DICTATION_LIMIT=300
export CODEROS_WHISPER_MODEL="$scratch/model.bin"
printf 'a speech model that is present\n' > "$CODEROS_WHISPER_MODEL"

# The script as the module builds it. The wrapper is `writeShellApplication`'s
# preamble, so the tests run what a host runs.
runner="$scratch/dictate-toggle"
{
  printf '#!/usr/bin/env bash\nset -o errexit\nset -o nounset\nset -o pipefail\n'
  cat "$script"
} > "$runner"
chmod +x "$runner"

out="$scratch/out"
err="$scratch/err"

# Run one press. Output goes to files rather than through a pipe: the press
# leaves a recorder and a watcher behind, and a pipe would hold the test open
# until both of them ended.
press() {
  local status=0
  : > "$out"
  : > "$err"
  "$runner" "$@" > "$out" 2> "$err" || status=$?
  printf '%s' "$status"
}

# Clear what the stubs logged, so each test reads its own press.
reset_logs() {
  : > "$TEST_FFMPEG_LOG"
  : > "$TEST_WTYPE_LOG"
  : > "$TEST_NOTIFY_LOG"
}

# The pid the record names, or nothing.
held_pid() {
  head -n 1 "$CODEROS_DICTATION_PID" 2>/dev/null || true
}

# End the recording a test started, and leave no record for the watcher to
# act on. The record goes first so the watcher reads nothing and stays quiet.
end_recording() {
  local pid
  pid=$(held_pid)
  rm -f "$CODEROS_DICTATION_PID"
  if [ -n "$pid" ]; then
    kill "$pid" 2>/dev/null || true
  fi
}

# A process that is alive, for a record that names a live recorder. It sets
# `LIVE_PID` rather than printing the number: a background process started
# inside a command substitution does not outlive the substitution, so a caller
# that read the pid that way would read a pid that was already gone.
LIVE_PID=""
start_live_process() {
  # The stand-in is a grandchild: a subshell starts it and leaves, so the
  # process is reparented and reaped elsewhere. A direct child would stay a
  # zombie after it was signalled, and `kill -0` reads a zombie as alive.
  ( sleep 30 & printf '%s' "$!" > "$scratch/live.pid" ) &
  wait $! 2>/dev/null || true
  LIVE_PID=$(cat "$scratch/live.pid")
  started+=("$LIVE_PID")
}

# A pid that names nothing. A process started and reaped leaves its number
# free, which is the state the record was found in on 2026-09-11.
dead_pid() {
  local pid
  sleep 0 &
  pid=$!
  wait "$pid" 2>/dev/null || true
  printf '%s' "$pid"
}

# One press with no record starts a recording.
reset_logs
rm -f "$CODEROS_DICTATION_PID" "$CODEROS_DICTATION_AUDIO"
status=$(press)
if [ "$status" = "0" ] && [ -f "$CODEROS_DICTATION_PID" ] \
  && kill -0 "$(held_pid)" 2>/dev/null; then
  pass "a press with no record starts a recording"
else
  fail "a press with no record starts a recording (exit $status, record '$(held_pid)')"
fi
started+=("$(held_pid)")
if grep -q -- "-f pulse" "$TEST_FFMPEG_LOG"; then
  pass "the recording reads the microphone"
else
  fail "the recording reads the microphone"
fi
if [ ! -s "$TEST_WTYPE_LOG" ]; then
  pass "a press that starts a recording types nothing"
else
  fail "a press that starts a recording types nothing"
fi
# The bound: the capture ends on its own after the limit, so the microphone
# cannot stay open for a toggle nobody saw.
if grep -q -- "-t $CODEROS_DICTATION_LIMIT" "$TEST_FFMPEG_LOG"; then
  pass "the recording is bounded by the limit"
else
  fail "the recording is bounded by the limit"
fi
# The start says on screen that the microphone is open, through the
# notification the session draws.
if grep -qi "microphone is open" "$TEST_NOTIFY_LOG"; then
  pass "the start says on screen that the microphone is open"
else
  fail "the start says on screen that the microphone is open"
fi
end_recording

# `status` answers while a recording runs, and names how long it has run.
reset_logs
start_live_process
pid=$LIVE_PID
printf '%s\n%s\n' "$pid" "$(($(date +%s) - 390))" > "$CODEROS_DICTATION_PID"
status=$(press status)
if [ "$status" = "0" ] && grep -qi "recording" "$out" && grep -q "6m 30s" "$out"; then
  pass "status names the recording and how long it has run"
else
  fail "status names the recording and how long it has run (exit $status, said '$(cat "$out")')"
fi
rm -f "$CODEROS_DICTATION_PID"
kill "$pid" 2>/dev/null || true

# `status` answers when nothing is recording, and answers non-zero, so a
# script can read the answer without reading the words.
reset_logs
rm -f "$CODEROS_DICTATION_PID"
status=$(press status)
if [ "$status" != "0" ] && grep -qi "not recording" "$out"; then
  pass "status answers non-zero when nothing is recording"
else
  fail "status answers non-zero when nothing is recording (exit $status, said '$(cat "$out")')"
fi

# The defect this lane exists for. A record naming a process that is gone is
# not a recording, so the press that finds one starts a recording rather than
# taking the stop path.
reset_logs
rm -f "$CODEROS_DICTATION_AUDIO"
printf '%s\n' "$(dead_pid)" > "$CODEROS_DICTATION_PID"
status=$(press)
if [ "$status" = "0" ] && kill -0 "$(held_pid)" 2>/dev/null; then
  pass "a record naming a dead pid starts a recording"
else
  fail "a record naming a dead pid starts a recording (exit $status, record '$(held_pid)')"
fi
started+=("$(held_pid)")
if [ ! -s "$TEST_WTYPE_LOG" ]; then
  pass "a record naming a dead pid types nothing"
else
  fail "a record naming a dead pid types nothing"
fi
if grep -qi "gone" "$err"; then
  pass "the sweep says the recorder was gone"
else
  fail "the sweep says the recorder was gone (stderr '$(cat "$err")')"
fi
end_recording

# A capture a dead recorder left is a recording of the room the person was
# sitting in. The press that sweeps the record keeps it rather than deleting
# it, and the fresh capture does not write over it.
reset_logs
printf 'RIFF-what-the-dead-recorder-heard' > "$CODEROS_DICTATION_AUDIO"
printf '%s\n' "$(dead_pid)" > "$CODEROS_DICTATION_PID"
status=$(press)
started+=("$(held_pid)")
kept=""
for candidate in "$scratch"/audio-*.wav; do
  if [ -f "$candidate" ] && grep -q "what-the-dead-recorder-heard" "$candidate" 2>/dev/null; then
    kept=$candidate
  fi
done
if [ -n "$kept" ]; then
  pass "the capture a dead recorder left is kept"
else
  fail "the capture a dead recorder left is kept"
fi
if grep -q "audio-" "$err"; then
  pass "the sweep says where the capture was kept"
else
  fail "the sweep says where the capture was kept (stderr '$(cat "$err")')"
fi
end_recording
rm -f "$scratch"/audio-*.wav

# A press that finds a live recorder stops it, transcribes what it heard, and
# types the text. The signal goes to the pid the record names and to no other.
reset_logs
start_live_process
pid=$LIVE_PID
printf '%s\n%s\n' "$pid" "$(date +%s)" > "$CODEROS_DICTATION_PID"
printf 'RIFF-a-capture-with-speech' > "$CODEROS_DICTATION_AUDIO"
export TEST_TRANSCRIPT="the words the microphone heard"
status=$(press)
if [ "$status" = "0" ] && ! kill -0 "$pid" 2>/dev/null; then
  pass "a press that finds a live recorder stops it"
else
  fail "a press that finds a live recorder stops it (exit $status)"
fi
if grep -q "the words the microphone heard" "$TEST_WTYPE_LOG"; then
  pass "the stop types what was heard"
else
  fail "the stop types what was heard (typed '$(cat "$TEST_WTYPE_LOG")')"
fi
if [ ! -f "$CODEROS_DICTATION_PID" ]; then
  pass "the stop removes the record"
else
  fail "the stop removes the record"
fi

# The tone is a cue, not the indicator, and it fails in every case that
# matters. Its failure reaches stderr rather than /dev/null.
reset_logs
rm -f "$CODEROS_DICTATION_PID" "$CODEROS_DICTATION_AUDIO"
export TEST_CUE_STATUS=1
status=$(press)
unset TEST_CUE_STATUS
started+=("$(held_pid)")
# The cue runs beside the press, so give it a moment to report.
tries=0
while ! grep -qi "tone" "$err" && [ "$tries" -lt 40 ]; do
  sleep 0.1
  tries=$((tries + 1))
done
if grep -qi "tone" "$err"; then
  pass "a tone that cannot sound says so"
else
  fail "a tone that cannot sound says so (stderr '$(cat "$err")')"
fi
end_recording

# The bound, reached. The capture ends itself at the limit, and the watcher
# says the microphone closed, keeps what it heard, and clears the record so
# the next press starts a recording rather than stopping one that is over.
reset_logs
rm -f "$CODEROS_DICTATION_PID" "$CODEROS_DICTATION_AUDIO"
rm -f "$scratch"/audio-*.wav
CODEROS_DICTATION_LIMIT=1
export CODEROS_DICTATION_LIMIT
export TEST_HONOR_LIMIT=1
status=$(press)
unset TEST_HONOR_LIMIT
CODEROS_DICTATION_LIMIT=300
export CODEROS_DICTATION_LIMIT
tries=0
while [ -f "$CODEROS_DICTATION_PID" ] && [ "$tries" -lt 100 ]; do
  sleep 0.1
  tries=$((tries + 1))
done
if [ ! -f "$CODEROS_DICTATION_PID" ]; then
  pass "the bound clears the record when the capture ends itself"
else
  fail "the bound clears the record when the capture ends itself"
  end_recording
fi
if grep -qi "closed" "$err" || grep -qi "closed" "$TEST_NOTIFY_LOG"; then
  pass "the bound says the microphone closed"
else
  fail "the bound says the microphone closed (stderr '$(cat "$err")')"
fi
kept=""
for candidate in "$scratch"/audio-*.wav; do
  if [ -f "$candidate" ]; then kept=$candidate; fi
done
if [ -n "$kept" ]; then
  pass "the bound keeps what the microphone heard"
else
  fail "the bound keeps what the microphone heard"
fi
rm -f "$scratch"/audio-*.wav

# The unit tests. Sourcing with `DICTATE_TOGGLE_LIB` set defines the
# functions and stops before the dispatch.
export DICTATE_TOGGLE_LIB=1
# shellcheck disable=SC1090
source "$runner"

rm -f "$CODEROS_DICTATION_PID"
if recording_live; then
  fail "recording_live is true with no record"
else
  pass "recording_live is false with no record"
fi

printf '%s\n' "$(dead_pid)" > "$CODEROS_DICTATION_PID"
if recording_live; then
  fail "recording_live is true for a record naming a dead pid"
else
  pass "recording_live is false for a record naming a dead pid"
fi

start_live_process
pid=$LIVE_PID
printf '%s\n%s\n' "$pid" "$(date +%s)" > "$CODEROS_DICTATION_PID"
if recording_live; then
  pass "recording_live is true for a record naming a live process"
else
  fail "recording_live is false for a record naming a live process"
fi
if [ "$(record_pid)" = "$pid" ]; then
  pass "record_pid reads the pid the record names"
else
  fail "record_pid reads the pid the record names"
fi
kill "$pid" 2>/dev/null || true

printf 'not a pid\n' > "$CODEROS_DICTATION_PID"
if record_pid > /dev/null 2>&1; then
  fail "record_pid accepts a record that names no number"
else
  pass "record_pid refuses a record that names no number"
fi
if recording_live; then
  fail "recording_live is true for a record that names no number"
else
  pass "recording_live is false for a record that names no number"
fi
rm -f "$CODEROS_DICTATION_PID"

if [ "$(duration_text 390)" = "6m 30s" ]; then
  pass "duration_text reads 390 seconds as minutes and seconds"
else
  fail "duration_text reads 390 seconds as '$(duration_text 390)'"
fi
if [ "$(duration_text 42)" = "42s" ]; then
  pass "duration_text reads a short run as seconds"
else
  fail "duration_text reads a short run as '$(duration_text 42)'"
fi

if [ "$failures" -eq 0 ]; then
  printf '\nall dictate-toggle tests passed\n'
else
  printf '\n%s dictate-toggle test(s) failed\n' "$failures" >&2
  exit 1
fi
