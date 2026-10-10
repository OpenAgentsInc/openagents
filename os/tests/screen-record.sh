#!/usr/bin/env bash
#
# Tests for `os/bin/screen-record`'s choice of microphone.
#
# Run it by hand:
#
#   os/tests/screen-record.sh
#
# There is no PipeWire here, so `pw-dump` and `wpctl` are stubs: `pw-dump`
# prints the capture nodes the test declares, in the shape the real one
# prints, and `wpctl inspect @DEFAULT_AUDIO_SOURCE@` names one of them as the
# default. The script is sourced with `SCREEN_RECORD_LIB` set, which defines
# its functions and stops before the dispatch, so the test calls
# `pick_microphone` and `microphone_report` directly.
#
# The choice is what this file exists for: the named microphone when it is
# present, the default source when it is not, and an honest answer when there
# is no capture node at all. On 2026-09-07 a recording read the webcam's
# microphone because nothing made that choice.
#
# Three more things are checked here, and each of them cost a real take on
# 2026-09-09.
#
# The choice survives a re-enumeration. A node id dies with its node, so a
# microphone that is unplugged and plugged back in comes back as a different
# id under the same name. `pick_microphone` is asked for the name and finds
# it either way, including under the `.2` suffix PipeWire adds when the new
# node arrives before the old one has gone.
#
# The capture is reattached. `supervise_audio` runs against a `pw-record`
# that dies the way a capture on a dead node does, and the next segment has
# to open by itself against whatever id the microphone came back as.
#
# The pieces are placed rather than concatenated, and the picture is never
# shortened by the sound. Those two need a real muxer, so they run only where
# ffmpeg and ffprobe exist and say so when they are skipped. They are the
# highest-value tests in the file: the mux carried `-shortest` until
# 2026-09-09 and delivered a 187.9-second take as 162.0 seconds of picture,
# throwing away 777 captured frames that could not be recovered.
#
# Two more are about a hole nobody watched open.
# A capture that stops delivering without ending is cut and the next segment
# opens where the sound came back, so the ledger carries the hole; and a take
# that comes back short with nothing to say where names no window at all,
# because the take this came from named the last 12.695s of a four-minute
# recording and the hole was at 82.31s.
#
# Everything this file starts is reaped on the way out. A stub here is a
# shell that waits, a shell that is ended does not end the `sleep` under it,
# and on 2026-09-11 this machine held around 1,500 orphaned stand-ins from
# earlier runs of these tests. So every stub waits with `exec`, which leaves
# nothing under it to orphan, and the trap ends every process this file
# started by the identifier it captured when it started it.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
script="$root/os/bin/screen-record"

if ! command -v jq > /dev/null 2>&1; then
  echo "FAIL jq is missing, and these tests need it" >&2
  exit 1
fi

scratch=$(mktemp -d "${TMPDIR:-/tmp}/coder-screen-record-test.XXXXXX")

# Every process this file started, youngest last, and what to do with them.
# `remember` is called with the identifier of each background command as it
# is started, and nothing is ever selected by name: `reap` signals the
# identifiers this file captured itself and the children those identifiers
# still have, and a child is found through its parent rather than through
# what it is called.
spawned=()
remember() { spawned+=("$1"); }

children() {
  local parent=$1 child
  command -v pgrep > /dev/null 2>&1 || return 0
  for child in $(pgrep -P "$parent" 2>/dev/null || true); do
    children "$child"
    printf '%s\n' "$child"
  done
}

reap() {
  local pid child
  for pid in ${spawned[@]+"${spawned[@]}"}; do
    [ -n "$pid" ] || continue
    for child in $(children "$pid"); do
      kill -TERM "$child" 2>/dev/null || true
    done
    kill -TERM "$pid" 2>/dev/null || true
  done
}

trap 'reap; rm -rf "$scratch"' EXIT

failures=0
pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; failures=$((failures + 1)); }

stubs="$scratch/stubs"
mkdir -p "$stubs"

# The stub graph. Each node the test declares is one line of
# `id<TAB>name<TAB>description<TAB>nick` in the file `TEST_NODES` names.
cat > "$stubs/pw-dump" << 'STUB'
#!/usr/bin/env bash
set -euo pipefail
jq -R -s '
  split("\n") | map(select(length > 0) | split("\t")) | map({
    type: "PipeWire:Interface:Node",
    id: (.[0] | tonumber),
    info: { props: {
      "media.class": "Audio/Source",
      "node.name": .[1],
      "node.description": .[2],
      "node.nick": .[3]
    } }
  })
' "$TEST_NODES"
STUB
chmod +x "$stubs/pw-dump"

# The stub session manager answers the one question the script asks, with
# the id `TEST_DEFAULT` holds, or with an error when it holds nothing.
cat > "$stubs/wpctl" << 'STUB'
#!/usr/bin/env bash
set -euo pipefail
if [ "${1:-}" = "inspect" ] && [ "${2:-}" = "@DEFAULT_AUDIO_SOURCE@" ]; then
  if [ -n "${TEST_DEFAULT:-}" ]; then
    printf 'id %s, type PipeWire:Interface:Node\n' "$TEST_DEFAULT"
    exit 0
  fi
  echo "Error: no default source" >&2
  exit 1
fi
exit 1
STUB
chmod +x "$stubs/wpctl"

export PATH="$stubs:$PATH"
export TEST_NODES="$scratch/nodes"
export CODEROS_SCREEN_RECORD_PID="$scratch/rec.pid"
export CODEROS_SCREEN_RECORD_META="$scratch/rec.meta"
rm -f "$CODEROS_SCREEN_RECORD_PID"

blue='alsa_input.usb-Generic_Blue_Microphones_SERIAL000001-00.analog-stereo'
webcam='alsa_input.usb-046d_Logitech_Webcam_C930e_BC7ACD8E-02.analog-stereo'
builtin='alsa_input.pci-0000_00_1f.3.analog-stereo'

# Source the script for its functions, under the wrapper the module builds.
runner="$scratch/screen-record"
{
  printf '#!/usr/bin/env bash\nset -o errexit\nset -o nounset\nset -o pipefail\n'
  cat "$script"
} > "$runner"
export SCREEN_RECORD_LIB=1
export CODEROS_MICROPHONE_NODE='alsa_input\.usb-Generic_Blue_Microphones.*'
export CODEROS_MICROPHONE_NAME="Blue"
# shellcheck disable=SC1090
source "$runner"

field() { printf '%s' "$1" | jq -r "$2"; }

# The named microphone wins whatever the default is.
printf '46\t%s\tWebcam C930e Analog Stereo\tLogitech Webcam C930e\n76\t%s\tBlue Microphones Analog Stereo\tBlue Microphones\n' "$webcam" "$blue" > "$TEST_NODES"
export TEST_DEFAULT=46
pick=$(pick_microphone)
if [ "$(field "$pick" .id)" = 76 ]; then pass "the named microphone is chosen over the default"; else fail "the named microphone was not chosen: $pick"; fi
if [ "$(field "$pick" .preferred)" = true ]; then pass "the named microphone counts as preferred"; else fail "the named microphone is not preferred: $pick"; fi
if [ "$(field "$pick" .nick)" = "Blue Microphones" ]; then pass "the choice carries the short name"; else fail "the choice lacks the short name: $pick"; fi

# With the named microphone absent, the default source is the fallback, and
# it is not preferred.
printf '46\t%s\tWebcam C930e Analog Stereo\tLogitech Webcam C930e\n60\t%s\tBuilt-in Audio Analog Stereo\tBuilt-in Audio\n' "$webcam" "$builtin" > "$TEST_NODES"
export TEST_DEFAULT=46
pick=$(pick_microphone)
if [ "$(field "$pick" .id)" = 46 ]; then pass "the default source is the fallback"; else fail "the fallback is not the default source: $pick"; fi
if [ "$(field "$pick" .preferred)" = false ]; then pass "the fallback is not preferred"; else fail "the fallback counts as preferred: $pick"; fi

# With no capture node at all, the answer is an empty id.
: > "$TEST_NODES"
export TEST_DEFAULT=""
pick=$(pick_microphone)
if [ "$(field "$pick" .id)" = null ]; then pass "no capture node gives a null id"; else fail "no capture node gave an id: $pick"; fi
if [ "$(field "$pick" .preferred)" = false ]; then pass "no capture node is not preferred"; else fail "no capture node counts as preferred: $pick"; fi

# With no pattern configured, the default source is the microphone and it
# counts as preferred: there is nothing else to want.
printf '46\t%s\tWebcam C930e Analog Stereo\tLogitech Webcam C930e\n' "$webcam" > "$TEST_NODES"
export TEST_DEFAULT=46
pick=$(MIC_NODE="" pick_microphone)
if [ "$(field "$pick" .preferred)" = true ]; then pass "with no pattern the default is preferred"; else fail "with no pattern the default is not preferred: $pick"; fi

# An unusable pattern falls through to the default rather than failing.
printf '76\t%s\tBlue Microphones Analog Stereo\tBlue Microphones\n' "$blue" > "$TEST_NODES"
export TEST_DEFAULT=76
pick=$(MIC_NODE='(' pick_microphone)
if [ "$(field "$pick" .id)" = 76 ]; then pass "a broken pattern falls through to the default"; else fail "a broken pattern broke the choice: $pick"; fi

# The report while no recording runs is the choice a start would make.
report=$(microphone_report)
if [ "$(field "$report" .recording)" = false ]; then pass "the idle report says no recording runs"; else fail "the idle report claims a recording: $report"; fi
if [ "$(field "$report" .id)" = 76 ]; then pass "the idle report names the microphone a start would read"; else fail "the idle report names the wrong microphone: $report"; fi

# The report while a recording runs is what the start wrote down.
sleep 300 &
live=$!
remember "$live"
disown 2>/dev/null || true
echo "$live" > "$CODEROS_SCREEN_RECORD_PID"
echo '{"id":46,"nick":"Logitech Webcam C930e","preferred":false,"recording":true,"live":true,"warning":"The microphone is the Logitech Webcam C930e; the Blue is absent."}' > "$CODEROS_SCREEN_RECORD_META.mic"
report=$(microphone_report)
if [ "$(field "$report" .recording)" = true ]; then pass "the recording report says a recording runs"; else fail "the recording report denies the recording: $report"; fi
if [ "$(field "$report" .id)" = 46 ]; then pass "the recording report names the recorded microphone"; else fail "the recording report names the wrong microphone: $report"; fi
kill "$live" 2>/dev/null || true
rm -f "$CODEROS_SCREEN_RECORD_PID" "$CODEROS_SCREEN_RECORD_META.mic"

# ---------------------------------------------------------------------------
# The choice survives a re-enumeration.
#
# The Blue on coderos-4080 came back as USB device 16, then 17, then 18, then
# 19 inside fifteen seconds, and PipeWire minted a node for each. Nothing
# about the id is stable; the name, built from the vendor, product, and
# serial, is the same string every time.
printf '76\t%s\tBlue Microphones Analog Stereo\tBlue Microphones\n' "$blue" > "$TEST_NODES"
export TEST_DEFAULT=76
pick=$(pick_microphone "$blue")
if [ "$(field "$pick" .id)" = 76 ]; then pass "the wanted name is found before a re-enumeration"; else fail "the wanted name was not found: $pick"; fi

# The same microphone, back on a new node id, with the webcam holding the
# default in the meantime.
printf '46\t%s\tWebcam C930e Analog Stereo\tLogitech Webcam C930e\n91\t%s\tBlue Microphones Analog Stereo\tBlue Microphones\n' "$webcam" "$blue" > "$TEST_NODES"
export TEST_DEFAULT=46
pick=$(pick_microphone "$blue")
if [ "$(field "$pick" .id)" = 91 ]; then pass "the wanted name is found again on its new node id"; else fail "the reattach picked the wrong node: $pick"; fi
if [ "$(field "$pick" .preferred)" = true ]; then pass "the reattached node still counts as preferred"; else fail "the reattached node is not preferred: $pick"; fi

# PipeWire numbers a duplicate when the new node arrives before the old one
# is gone, which is exactly what a replug during a recording produces.
printf '91\t%s.2\tBlue Microphones Analog Stereo\tBlue Microphones\n' "$blue" > "$TEST_NODES"
export TEST_DEFAULT=""
pick=$(pick_microphone "$blue")
if [ "$(field "$pick" .id)" = 91 ]; then pass "the wanted name is found under PipeWire's duplicate suffix"; else fail "the duplicate suffix hid the microphone: $pick"; fi

# With the wanted microphone genuinely gone, the pattern and then the default
# still answer, so a reattach never stalls waiting for a name that is not
# coming back.
printf '46\t%s\tWebcam C930e Analog Stereo\tLogitech Webcam C930e\n' "$webcam" > "$TEST_NODES"
export TEST_DEFAULT=46
pick=$(pick_microphone "$blue")
if [ "$(field "$pick" .id)" = 46 ]; then pass "a name that is gone falls through to the default"; else fail "a missing name did not fall through: $pick"; fi
if [ "$(field "$pick" .preferred)" = false ]; then pass "the fall-through is not preferred"; else fail "the fall-through counts as preferred: $pick"; fi

# ---------------------------------------------------------------------------
# The capture is reattached when its node dies.
#
# `pw-record` here writes a plausible capture and then exits, which is what
# the real one does when the node behind its target goes away. Nothing
# restarted it before 2026-09-09, and the rest of the take ran silently.
cat > "$stubs/pw-record" << 'STUB'
#!/usr/bin/env bash
set -euo pipefail
target=""
out=""
for arg in "$@"; do
  case "$arg" in
    --target=*) target="${arg#--target=}" ;;
    *.wav) out="$arg" ;;
  esac
done
printf '%s\n' "$target" >> "$TEST_TARGETS"
# A capture large enough to clear the script's 4 KB floor for real sound.
head -c 8192 /dev/zero > "$out"
n=$(wc -l < "$TEST_TARGETS" | tr -d ' ')
if [ "$n" = 1 ]; then
  # The first capture delivers until its node goes away, and then dies the
  # way a capture on a re-enumerated node does. It has to keep writing while
  # it lives, because a capture that stops writing is now cut where it
  # stopped rather than waited on, and this case is about the exit.
  for _ in 1 2 3 4 5 6 7 8; do
    head -c 4800 /dev/zero >> "$out"
    sleep 0.05
  done
  exit 1
fi
# The second delivers until it is asked to stop. It keeps writing for the
# same reason the first does: a capture that goes quiet is cut.
while : ; do
  head -c 4800 /dev/zero >> "$out"
  sleep 0.05
done
STUB
chmod +x "$stubs/pw-record"

export TEST_TARGETS="$scratch/targets"
: > "$TEST_TARGETS"
rm -f "$CODEROS_SCREEN_RECORD_META.audio.segments" "$CODEROS_SCREEN_RECORD_META.audio.outage"
rm -f "$scratch/take.mp4".audio*.wav

# The microphone is on node 76 when the take starts and on node 91 when it
# comes back, the same name both times.
printf '76\t%s\tBlue Microphones Analog Stereo\tBlue Microphones\n' "$blue" > "$TEST_NODES"
export TEST_DEFAULT=76
# shellcheck disable=SC2034  # supervise_audio reads both through resolve_audio_target
AUDIO_KIND=microphone
# shellcheck disable=SC2034
AUDIO_NAME=$blue
echo '{"id":76,"name":"'"$blue"'","description":"Blue Microphones Analog Stereo","nick":"Blue Microphones","preferred":true,"recording":true,"live":true,"warning":""}' > "$CODEROS_SCREEN_RECORD_META.mic"

supervise_audio "$scratch/take.mp4" "$(epoch_now)" 76 '{"id":76,"name":"'"$blue"'","description":"Blue Microphones Analog Stereo","nick":"Blue Microphones","preferred":true}' > /dev/null 2>&1 &
supervisor=$!
remember "$supervisor"
disown 2>/dev/null || true

# The outage file is there only while the outage is open, so watch for it
# from the moment the supervisor starts rather than through a window that a
# loaded machine can open and close between two polls.
outage_seen_file="$scratch/outage-seen"
rm -f "$outage_seen_file"
(
  for _ in $(seq 1 400); do
    [ -f "$CODEROS_SCREEN_RECORD_META.audio.outage" ] && { : > "$outage_seen_file"; break; }
    [ -f "$scratch/take.mp4.audio.001.wav" ] && break
    sleep 0.05
  done
) &
watcher=$!
remember "$watcher"

# The node comes back on a new id while the supervisor is looking for it.
sleep 0.8
printf '91\t%s\tBlue Microphones Analog Stereo\tBlue Microphones\n' "$blue" > "$TEST_NODES"
export TEST_DEFAULT=91

# The outage is visible while it is open.
wait "$watcher" 2>/dev/null || true
if [ -f "$outage_seen_file" ]; then pass "an open outage is written down while it is open"; else fail "no outage was recorded while the capture was dead"; fi

# And the capture comes back by itself.
reattached=0
for _ in $(seq 1 40); do
  if [ -f "$scratch/take.mp4.audio.001.wav" ]; then reattached=1; break; fi
  sleep 0.25
done
# The capture file lands before the ledger line, the outage close, and the
# report that follow it in the supervisor's loop. A stop inside that window
# cuts the bookkeeping short, so wait on its last write instead.
for _ in $(seq 1 40); do
  if jq -e '.id == 91 and .live == true' "$CODEROS_SCREEN_RECORD_META.mic" > /dev/null 2>&1; then break; fi
  sleep 0.25
done
: > "$CODEROS_SCREEN_RECORD_PID.audio.stop"
kill "$supervisor" 2>/dev/null || true
apid=$(cat "$CODEROS_SCREEN_RECORD_PID.audio" 2>/dev/null || true)
[ -n "$apid" ] && kill "$apid" 2>/dev/null || true
wait "$supervisor" 2>/dev/null || true

if [ "$reattached" = 1 ]; then pass "a capture that dies mid-take is reattached into a new segment"; else fail "the capture was never reattached"; fi
# `pw-record` is handed the node's name, not its id, since 2026-10-01 (an id
# there is read as an object serial); the name is the one that survives the
# re-enumeration, and the report below carries the new id.
if [ "$(sed -n '2p' "$TEST_TARGETS")" = "$blue" ]; then pass "the reattach targets the microphone by the name it came back under"; else fail "the reattach targeted $(sed -n '2p' "$TEST_TARGETS") rather than $blue"; fi
if [ "$(wc -l < "$CODEROS_SCREEN_RECORD_META.audio.segments" | tr -d ' ')" = 2 ]; then pass "the ledger carries a line per segment"; else fail "the ledger does not carry two segments: $(cat "$CODEROS_SCREEN_RECORD_META.audio.segments")"; fi
if [ "$(head -n 1 "$CODEROS_SCREEN_RECORD_META.audio.segments" | cut -f2)" = "0.000" ]; then pass "the first segment opens at the take's own zero"; else fail "the first segment does not open at zero"; fi
if awk -F'\t' 'BEGIN { ok = 0 } NR == 2 && $2 > 0.3 { ok = 1 } END { exit !ok }' "$CODEROS_SCREEN_RECORD_META.audio.segments"; then pass "the second segment carries the offset it opened at"; else fail "the second segment has no offset"; fi
if jq -e '.id == 91 and .live == true' "$CODEROS_SCREEN_RECORD_META.mic" > /dev/null 2>&1; then pass "the report the HUD reads moves to the node that is live"; else fail "the report still names the dead node: $(cat "$CODEROS_SCREEN_RECORD_META.mic")"; fi
if [ ! -f "$CODEROS_SCREEN_RECORD_META.audio.outage" ]; then pass "the outage is closed when the capture comes back"; else fail "the outage stayed open after the reattach"; fi

rm -f "$stubs/pw-record" "$CODEROS_SCREEN_RECORD_PID.audio" "$CODEROS_SCREEN_RECORD_PID.audio.stop"
rm -f "$CODEROS_SCREEN_RECORD_META.mic" "$CODEROS_SCREEN_RECORD_META.audio.outage"

# ---------------------------------------------------------------------------
# A capture that stops delivering is cut, whether or not it returns.
#
# The stub below writes half a second of sound, stops writing, and stays
# alive. That is what happened on 2026-09-11: a microphone left the USB bus
# 82.31s into a four-minute take, PipeWire held the node across the
# re-enumeration, `pw-record` never returned, and 12.695s of sound went
# missing inside one segment. Measuring the same file afterwards found five
# more losses of 0.29s to 2.56s, each one against a freeze in the recorder's
# own drawing and none of them against a device event, so the capture's own
# file is the only thing that says the sound stopped every time.
cat > "$stubs/pw-record" << 'STUB'
#!/usr/bin/env bash
set -euo pipefail
target=""
out=""
for arg in "$@"; do
  case "$arg" in
    --target=*) target="${arg#--target=}" ;;
    *.wav) out="$arg" ;;
  esac
done
printf '%s\n' "$target" >> "$TEST_TARGETS"
n=$(wc -l < "$TEST_TARGETS" | tr -d ' ')
head -c 44 /dev/zero > "$out"
if [ "$n" = 1 ]; then
  # Half a second of sound at 48 kHz mono, and then a process that is still
  # here and writing nothing.
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    head -c 4800 /dev/zero >> "$out"
    sleep 0.05
  done
  exec sleep 120
fi
# What opens after the cut keeps delivering until it is asked to stop.
while : ; do
  head -c 4800 /dev/zero >> "$out"
  sleep 0.05
done
STUB
chmod +x "$stubs/pw-record"

export TEST_TARGETS="$scratch/stall-targets"
: > "$TEST_TARGETS"
rm -f "$CODEROS_SCREEN_RECORD_META.audio.segments" "$CODEROS_SCREEN_RECORD_META.audio.outage"
rm -f "$CODEROS_SCREEN_RECORD_PID.audio" "$CODEROS_SCREEN_RECORD_PID.audio.stop"
rm -f "$scratch/stall.mp4".audio*.wav
printf '76\t%s\tBlue Microphones Analog Stereo\tBlue Microphones\n' "$blue" > "$TEST_NODES"
export TEST_DEFAULT=76
# shellcheck disable=SC2034  # supervise_audio reads both through resolve_audio_target
AUDIO_KIND=microphone
# shellcheck disable=SC2034
AUDIO_NAME=$blue

supervise_audio "$scratch/stall.mp4" "$(epoch_now)" 76 '{"id":76,"name":"'"$blue"'","description":"Blue Microphones Analog Stereo","nick":"Blue Microphones","preferred":true}' > /dev/null 2>&1 &
supervisor=$!
remember "$supervisor"
disown 2>/dev/null || true

cut=0
for _ in $(seq 1 100); do
  if [ -f "$scratch/stall.mp4.audio.001.wav" ]; then cut=1; break; fi
  sleep 0.1
done
# The capture file lands before the ledger line that follows it in the
# supervisor's loop; wait for the line so the stop cannot cut it short.
for _ in $(seq 1 100); do
  if [ "$(wc -l < "$CODEROS_SCREEN_RECORD_META.audio.segments" 2>/dev/null | tr -d ' ')" -ge 2 ]; then break; fi
  sleep 0.1
done
: > "$CODEROS_SCREEN_RECORD_PID.audio.stop"
kill "$supervisor" 2>/dev/null || true
apid=$(cat "$CODEROS_SCREEN_RECORD_PID.audio" 2>/dev/null || true)
[ -n "$apid" ] && kill "$apid" 2>/dev/null || true
wait "$supervisor" 2>/dev/null || true

if [ "$cut" = 1 ]; then pass "a capture that stops delivering is cut into a new segment"; else fail "a capture that stopped delivering was never cut"; fi
if [ "$(wc -l < "$CODEROS_SCREEN_RECORD_META.audio.segments" | tr -d ' ')" = 2 ]; then pass "the cut is a line in the ledger"; else fail "the cut left no second segment: $(cat "$CODEROS_SCREEN_RECORD_META.audio.segments")"; fi
# The watcher times the capture as it grows, which is what the clock
# measurement at `stop` reads (#11151): a line per growth it saw, the wall
# clock and the size, and nothing from before the first sample.
clock_log="$scratch/stall.mp4.audio.wav.clock"
if [ -f "$clock_log" ] && [ "$(wc -l < "$clock_log" | tr -d ' ')" -ge 3 ]; then pass "a running capture is timed against the wall as it grows"; else fail "the capture left no clock log: $(cat "$clock_log" 2>/dev/null)"; fi
if [ -f "$clock_log" ] && awk -F'\t' 'BEGIN { ok = 1 } $2 <= 44 { ok = 0 } END { exit !ok }' "$clock_log"; then pass "the clock log starts at the first sample, not the header"; else fail "the clock log carries the header: $(head -n 2 "$clock_log" 2>/dev/null)"; fi
if awk -F'\t' 'BEGIN { ok = 0 } NR == 2 && $2 > 0.4 { ok = 1 } END { exit !ok }' "$CODEROS_SCREEN_RECORD_META.audio.segments"; then pass "the segment after a cut opens where the sound came back"; else fail "the segment after a cut opens at $(awk -F'\t' 'NR == 2 { print $2 }' "$CODEROS_SCREEN_RECORD_META.audio.segments")"; fi

rm -f "$CODEROS_SCREEN_RECORD_PID.audio" "$CODEROS_SCREEN_RECORD_PID.audio.stop"
rm -f "$CODEROS_SCREEN_RECORD_META.audio.segments" "$CODEROS_SCREEN_RECORD_META.audio.outage"
rm -f "$CODEROS_SCREEN_RECORD_META.mic"

# ---------------------------------------------------------------------------
# A capture node that goes away while the process lives is a hole too.
#
# This is the kernel's `USB disconnect` seen one level above it. The stub
# here never stops delivering, so nothing but the node check can end the
# first segment, and the node the capture was opened on is not in the graph
# when the supervisor looks.
cat > "$stubs/pw-record" << 'STUB'
#!/usr/bin/env bash
set -euo pipefail
target=""
out=""
for arg in "$@"; do
  case "$arg" in
    --target=*) target="${arg#--target=}" ;;
    *.wav) out="$arg" ;;
  esac
done
printf '%s\n' "$target" >> "$TEST_TARGETS"
head -c 44 /dev/zero > "$out"
while : ; do
  head -c 4800 /dev/zero >> "$out"
  sleep 0.05
done
STUB
chmod +x "$stubs/pw-record"

export TEST_TARGETS="$scratch/device-targets"
: > "$TEST_TARGETS"
rm -f "$scratch/device.mp4".audio*.wav
# The microphone is back on node 91 by the time the supervisor looks, and
# node 76, which the capture was opened on, is gone.
printf '91\t%s\tBlue Microphones Analog Stereo\tBlue Microphones\n' "$blue" > "$TEST_NODES"
export TEST_DEFAULT=91
DEVICE_POLL=0.3

supervise_audio "$scratch/device.mp4" "$(epoch_now)" 76 '{"id":76,"name":"'"$blue"'","description":"Blue Microphones Analog Stereo","nick":"Blue Microphones","preferred":true}' > /dev/null 2>&1 &
supervisor=$!
remember "$supervisor"
disown 2>/dev/null || true

moved=0
for _ in $(seq 1 100); do
  if [ -f "$scratch/device.mp4.audio.001.wav" ]; then moved=1; break; fi
  sleep 0.1
done
: > "$CODEROS_SCREEN_RECORD_PID.audio.stop"
kill "$supervisor" 2>/dev/null || true
apid=$(cat "$CODEROS_SCREEN_RECORD_PID.audio" 2>/dev/null || true)
[ -n "$apid" ] && kill "$apid" 2>/dev/null || true
wait "$supervisor" 2>/dev/null || true

if [ "$moved" = 1 ]; then pass "a node that went away ends the segment it was read from"; else fail "a capture on a node that went away was never ended"; fi
if [ "$(sed -n '2p' "$TEST_TARGETS")" = "$blue" ]; then pass "the segment after a lost node opens on the microphone by name"; else fail "the segment after a lost node opened on $(sed -n '2p' "$TEST_TARGETS")"; fi

# shellcheck disable=SC2034  # watch_capture reads it; the tests below do not
DEVICE_POLL="${CODEROS_RECORD_AUDIO_DEVICE_POLL:-2}"
rm -f "$stubs/pw-record" "$CODEROS_SCREEN_RECORD_PID.audio" "$CODEROS_SCREEN_RECORD_PID.audio.stop"
rm -f "$CODEROS_SCREEN_RECORD_META.mic" "$CODEROS_SCREEN_RECORD_META.audio.outage"
rm -f "$CODEROS_SCREEN_RECORD_META.audio.segments"

# ---------------------------------------------------------------------------
# A hole is named where it was found, and nowhere else.
#
# The ledger records when a capture opened, not whether it delivered. The
# 2026-09-11 take came back 12.695s short with one line in its ledger, and
# every piece agreed on the same wrong answer: the tag read `audio missing
# 227.6s-240.3s`, which says the last 12.695s is silent and the rest is
# sound. The hole was at 82.31s and everything after it sat 12.695s early,
# so both halves of the tag were false and the take read as one that needed
# no repair.
#
# A window at the end of the take needs proof that the sound stopped there.
# The proof is the offset the supervisor wrote down when a capture it was
# watching stopped and no other opened. Without it the take is short, the
# place is unknown, and saying that is the answer.
short_place="$scratch/short-place"
printf '0.000\t2.000\t%s\n' "$scratch/one.wav" > "$short_place"

named=$(audio_outages "$short_place" 6.000000 "")
if [ -z "$named" ]; then pass "a shortfall nothing watched names no window"; else fail "an unwatched shortfall was named '$named'"; fi
short=$(audio_shortfall "$short_place" 6.000000 "")
if [ "$short" = "4.000" ]; then pass "a shortfall nothing watched is measured"; else fail "the shortfall was measured as '$short'"; fi

named=$(audio_outages "$short_place" 6.000000 "2.000")
if [ "$named" = "2.0s-6.0s" ]; then pass "a shortfall the supervisor watched is named"; else fail "a watched shortfall was named '$named'"; fi
if [ -z "$(audio_shortfall "$short_place" 6.000000 "2.000")" ]; then pass "a watched shortfall is not counted twice"; else fail "a watched shortfall was reported as unplaced as well"; fi

# A hole between two pieces is located by the ledger itself, and needs no
# proof: the next piece's offset is the clock at the moment it opened.
two_place="$scratch/two-place"
{
  printf '0.000\t2.000\t%s\n' "$scratch/one.wav"
  printf '4.000\t2.000\t%s\n' "$scratch/two.wav"
} > "$two_place"
named=$(audio_outages "$two_place" 6.000000 "")
if [ "$named" = "2.0s-4.0s" ]; then pass "a hole between two pieces needs no proof"; else fail "the hole between two pieces was named '$named'"; fi
if [ -z "$(audio_shortfall "$two_place" 6.000000 "")" ]; then pass "a take whose sound reaches the picture is not short"; else fail "a take whose sound reaches the picture was called short"; fi

# What counts as proof: the marker a supervisor leaves while a hole is open,
# and a last capture that opened and delivered nothing.
rm -f "$CODEROS_SCREEN_RECORD_META.audio.outage" "$CODEROS_SCREEN_RECORD_META.audio.segments"
if [ -z "$(trailing_outage)" ]; then pass "a take with no ledger proves no hole at the end"; else fail "a take with no ledger proved a hole at the end"; fi
printf '12.500\n' > "$CODEROS_SCREEN_RECORD_META.audio.outage"
if [ "$(trailing_outage)" = "12.500" ]; then pass "a hole still open at stop is the proof"; else fail "an open hole is not read as proof: $(trailing_outage)"; fi
rm -f "$CODEROS_SCREEN_RECORD_META.audio.outage"
head -c 8192 /dev/zero > "$scratch/proof.wav"
head -c 44 /dev/zero > "$scratch/proof.001.wav"
{
  printf '0\t0.000\t%s\n' "$scratch/proof.wav"
  printf '1\t3.000\t%s\n' "$scratch/proof.001.wav"
} > "$CODEROS_SCREEN_RECORD_META.audio.segments"
if [ "$(trailing_outage)" = "3.000" ]; then pass "a last capture that delivered nothing is the proof"; else fail "an empty last capture is not read as proof: $(trailing_outage)"; fi
rm -f "$CODEROS_SCREEN_RECORD_META.audio.segments" "$scratch/proof.wav" "$scratch/proof.001.wav"

# ---------------------------------------------------------------------------
# The microphone's clock against the wall (#11151).
#
# On 2026-10-09 the Blue delivered its samples 0.137% off the wall clock the
# picture is held to, and a 281-second take ended with its sound 0.39s out of
# step, the gap growing from the first second. The clock log is what the
# watcher wrote while the capture ran; the ratio is what `stop` stretches the
# piece by. The log here is a capture that delivered 0.137% fewer bytes than
# 96,000 a second, seen ten times a second with the jitter a poll has.
clock_case="$scratch/clock"
mkdir -p "$clock_case"
make_clock() {
  awk -v rate="$2" -v secs="$3" 'BEGIN {
    srand(11)
    t0 = 1760000000000000
    for (i = 0; i < secs * 10; i++) {
      t = i / 10 + rand() * 0.08
      printf "%d\t%d\n", t0 + t * 1000000, 44 + 4096 + int(96000 * rate * t / 2048) * 2048
    }
  }' > "$1"
}
make_clock "$clock_case/slow.clock" 0.99863 281
ratio=$(audio_clock_ratio "$clock_case/slow.clock")
if awk -v r="$ratio" 'BEGIN { exit !(r > 0.99853 && r < 0.99873) }'; then pass "a microphone 0.137% slow is measured to within 0.01%"; else fail "a microphone 0.137% slow measured '$ratio'"; fi
make_clock "$clock_case/fast.clock" 1.0025 312
ratio=$(audio_clock_ratio "$clock_case/fast.clock")
if awk -v r="$ratio" 'BEGIN { exit !(r > 1.0024 && r < 1.0026) }'; then pass "a microphone 0.25% fast is measured"; else fail "a microphone 0.25% fast measured '$ratio'"; fi
make_clock "$clock_case/true.clock" 1.0 200
if [ -z "$(audio_clock_ratio "$clock_case/true.clock")" ]; then pass "a microphone on time is not stretched"; else fail "a microphone on time was given a ratio: $(audio_clock_ratio "$clock_case/true.clock")"; fi
make_clock "$clock_case/short.clock" 0.99863 8
if [ -z "$(audio_clock_ratio "$clock_case/short.clock")" ]; then pass "a capture too short to measure is not stretched"; else fail "a short capture was given a ratio"; fi
make_clock "$clock_case/wrong.clock" 2.0 120
if [ -z "$(audio_clock_ratio "$clock_case/wrong.clock")" ]; then pass "a rate far off nominal is a wrong format, not a clock, and is not stretched"; else fail "a doubled rate was taken for a clock"; fi
if [ -z "$(audio_clock_ratio "$clock_case/absent.clock")" ]; then pass "a capture with no clock log is not stretched"; else fail "a missing clock log gave a ratio"; fi

# The tag names the measured rate of every piece that was stretched, and
# nothing when none was.
printf '0.000\t281.385\t%s\t0.998630\n' "$scratch/one.wav" > "$clock_case/place"
case "$(audio_clock_note "$clock_case/place")" in
  *"-0.137%"*"stretched"*) pass "the tag names the microphone's clock and the stretch" ;;
  *) fail "the clock note reads '$(audio_clock_note "$clock_case/place")'" ;;
esac
printf '0.000\t281.000\t%s\t\n' "$scratch/one.wav" > "$clock_case/place"
if [ -z "$(audio_clock_note "$clock_case/place")" ]; then pass "a take with no stretch says nothing about the clock"; else fail "an unstretched take named a clock"; fi

# ---------------------------------------------------------------------------
# The check after the mux: the HUD meter against the sound (#11151).
#
# The meter draws a fixed latency after the sound (0.3s-0.47s on the owner's
# takes), and that constant is not a defect. What is checked is whether the
# lag changes across the take. These are the two series the check extracts,
# made directly: a speech-like level, and a meter that follows it 12 frames
# late, either steadily or sliding a further 30 frames over the take.
make_series() {
  awk -v drift="$2" -v sound="$1.sound" -v meter="$1.meter" 'BEGIN {
    srand(5)
    n = 30 * 200; level = 0; target = 0
    for (i = 0; i < n; i++) {
      if (rand() < 0.08) target = (rand() < 0.35) ? 0 : rand()
      level = 0.6 * level + 0.4 * target
      a[i] = level
    }
    for (i = 0; i < n; i++) {
      d = 12 + int(drift * i / n)
      j = i - d; v = (j >= 0) ? a[j] : 0
      printf "%.6f\n", a[i] > sound
      printf "%.3f\n", 20 + 180 * v + rand() * 3 > meter
    }
  }'
}
make_series "$clock_case/steady" 0
IFS=$'\t' read -r verdict first final used total < <(meter_lag_trend "$clock_case/steady.meter" "$clock_case/steady.sound" 30)
if [ "$verdict" = ok ]; then pass "a meter that trails the sound steadily passes the check"; else fail "a steady meter got '$verdict' ($first -> $final, $used of $total)"; fi
if awk -v f="$first" 'BEGIN { exit !(f > 0.35 && f < 0.45) }'; then pass "the meter's own latency is measured, not judged"; else fail "the steady meter's lag measured '$first'"; fi
make_series "$clock_case/drift" 30
IFS=$'\t' read -r verdict first final used total < <(meter_lag_trend "$clock_case/drift.meter" "$clock_case/drift.sound" 30)
if [ "$verdict" = drift ]; then pass "a meter whose lag grows a second over the take fails the check"; else fail "a drifting meter got '$verdict' ($first -> $final, $used of $total)"; fi
if awk -v a="$first" -v b="$final" 'BEGIN { d = b - a; exit !(d > 0.8 && d < 1.2) }'; then pass "the drift is measured across the take"; else fail "the drift measured $first -> $final"; fi
awk 'BEGIN { for (i = 0; i < 6000; i++) print 0 }' > "$clock_case/flat.sound"
IFS=$'\t' read -r verdict first final used total < <(meter_lag_trend "$clock_case/steady.meter" "$clock_case/flat.sound" 30)
if [ "$verdict" = unknown ]; then pass "a silent take is not judged"; else fail "a silent take got '$verdict'"; fi
# Off, or with nowhere to look, the check does not run and says nothing.
checked=0
out=$(SYNC_CHECK=0 sync_check "$scratch/none.mp4" 30) || checked=$?
if [ "$checked" = 2 ] && [ -z "$out" ]; then pass "a disabled check does not run"; else fail "a disabled check answered $checked and said '$out'"; fi

# ---------------------------------------------------------------------------
# The mux: what the file is when the take is put together.
#
# These need a real ffmpeg, because what is being checked is where the
# samples land and how many frames survive. A host without one is told which
# tests it did not run rather than passing them quietly.
if ! command -v ffmpeg > /dev/null 2>&1 || ! command -v ffprobe > /dev/null 2>&1; then
  echo "skip the mux tests: this host has no ffmpeg" >&2
else
  mux="$scratch/mux"
  mkdir -p "$mux"

  # A six-second picture at 30 frames a second: 180 frames, and every one of
  # them has to be in the file at the end of every case below. mpeg4 is
  # ffmpeg's own encoder, so this runs on a build with no external ones.
  make_video() {
    ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc=size=160x120:rate=30:duration=6" \
      -pix_fmt yuv420p -c:v mpeg4 -q:v 5 "$1" 2>/dev/null
  }
  # A tone, which is loud where it is and silent where it is not.
  make_tone() {
    ffmpeg -hide_banner -loglevel error -y -f lavfi -i "sine=frequency=1000:duration=$2:sample_rate=48000" \
      -ac 1 -c:a pcm_s16le "$1" 2>/dev/null
  }
  frames() { ffprobe -v error -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of csv=p=0 "$1" 2>/dev/null | tr -d '\r\n '; }
  aduration() { ffprobe -v error -select_streams a:0 -show_entries stream=duration -of csv=p=0 "$1" 2>/dev/null | tr -d '\r\n '; }
  comment() { ffprobe -v error -show_entries format_tags=comment -of csv=p=0 "$1" 2>/dev/null; }
  # The mean level of one window of the sound, in dB. Digital silence reads
  # about -91 dB; a tone reads better than -20.
  window_db() {
    ffmpeg -hide_banner -nostats -y -ss "$2" -t "$3" -i "$1" -map 0:a:0 -af volumedetect -f null - 2>&1 \
      | sed -n 's/.*mean_volume: \(-*[0-9.]*\) dB.*/\1/p' | head -n 1
  }
  loud() { awk -v d="$1" 'BEGIN { exit !(d + 0 > -40) }'; }
  quiet() { awk -v d="$1" 'BEGIN { exit !(d + 0 < -60) }'; }

  # A take that never lost its microphone: one piece, no holes, and the tag
  # it has always carried.
  make_video "$mux/whole.mp4"
  make_tone "$mux/whole.mp4.audio.wav" 6
  printf '0\t0.000\t%s\n' "$mux/whole.mp4.audio.wav" > "$CODEROS_SCREEN_RECORD_META.audio.segments"
  audio_segments "$mux/whole.mp4" > "$mux/place"
  note="coderos: video 6.000000s, 180 frames at 30 fps constant; audio $(audio_end "$mux/place")s"
  if [ -z "$(audio_outages "$mux/place" 6.0 "")" ]; then pass "a take with no hole names no window"; else fail "a whole take named a window: $(audio_outages "$mux/place" 6.0 "")"; fi
  if mux_recording "$mux/whole.mp4" 6.0 "$note" "$mux/place" ""; then pass "a whole take muxes"; else fail "a whole take did not mux"; fi
  if [ "$(frames "$mux/whole.mp4")" = 180 ]; then pass "a whole take keeps every frame"; else fail "a whole take lost frames: $(frames "$mux/whole.mp4")"; fi
  case "$(comment "$mux/whole.mp4")" in
    *"audio missing"*) fail "a whole take claims audio is missing" ;;
    *coderos:*) pass "a whole take carries the tag it always did" ;;
    *) fail "a whole take carries no tag: $(comment "$mux/whole.mp4")" ;;
  esac

  # A piece whose microphone ran slow is stretched to the wall before it is
  # placed (#11151). The rate here is far past what a clock does, so the
  # stretch is plain to hear: two seconds of tone at half rate fill four, and
  # the tone is still sounding at three seconds where the unstretched piece
  # would be silence.
  make_video "$mux/clock.mp4"
  make_tone "$mux/clock.mp4.audio.wav" 2
  printf '0.000\t4.000000\t%s\t0.500000\n' "$mux/clock.mp4.audio.wav" > "$mux/place"
  if mux_recording "$mux/clock.mp4" 6.0 "coderos: clock" "$mux/place" ""; then pass "a stretched piece muxes"; else fail "a stretched piece did not mux"; fi
  if [ "$(frames "$mux/clock.mp4")" = 180 ]; then pass "a stretched piece keeps every frame"; else fail "a stretched piece lost frames: $(frames "$mux/clock.mp4")"; fi
  if loud "$(window_db "$mux/clock.mp4" 2.3 1.4)"; then pass "a slow microphone's sound is stretched over the seconds it was said in"; else fail "the stretched sound is missing at 2.3s-3.7s: $(window_db "$mux/clock.mp4" 2.3 1.4) dB"; fi
  if quiet "$(window_db "$mux/clock.mp4" 4.4 1.4)"; then pass "the stretch ends where the stretched piece does"; else fail "the stretched piece runs past its end: $(window_db "$mux/clock.mp4" 4.4 1.4) dB"; fi

  # The placement carries the measured rate and the stretched length, from
  # the clock log the watcher left beside the capture.
  make_tone "$mux/measured.mp4.audio.wav" 2
  make_clock "$mux/measured.mp4.audio.wav.clock" 0.99863 60
  printf '0\t0.000\t%s\n' "$mux/measured.mp4.audio.wav" > "$CODEROS_SCREEN_RECORD_META.audio.segments"
  audio_segments "$mux/measured.mp4" > "$mux/place"
  if awk -F'\t' '{ exit !($4 > 0.9985 && $4 < 0.9988 && $2 > 2.002 && $2 < 2.004) }' "$mux/place"; then pass "a measured piece is placed at its stretched length with its rate"; else fail "the measured piece was placed as '$(cat "$mux/place")'"; fi
  rm -f "$mux/measured.mp4.audio.wav" "$mux/measured.mp4.audio.wav.clock"

  # The take these tests are about: the microphone died at two seconds and came
  # back at four, and the sound after the hole has to stay where it was said.
  # Concatenating the two pieces would put the second tone at 2s-4s, which is
  # exactly what the last window below rules out.
  make_video "$mux/gap.mp4"
  make_tone "$mux/gap.mp4.audio.wav" 2
  make_tone "$mux/gap.mp4.audio.001.wav" 2
  {
    printf '0\t0.000\t%s\n' "$mux/gap.mp4.audio.wav"
    printf '1\t4.000\t%s\n' "$mux/gap.mp4.audio.001.wav"
  } > "$CODEROS_SCREEN_RECORD_META.audio.segments"
  audio_segments "$mux/gap.mp4" > "$mux/place"
  missing=$(audio_outages "$mux/place" 6.000000 "")
  if [ "$missing" = "2.0s-4.0s" ]; then pass "the hole between two pieces is named"; else fail "the hole was named '$missing'"; fi
  note="coderos: video 6.000000s, 180 frames at 30 fps constant; audio $(audio_end "$mux/place")s; audio missing ${missing}"
  if mux_recording "$mux/gap.mp4" 6.000000 "$note" "$mux/place" ""; then pass "a take with a hole muxes"; else fail "a take with a hole did not mux"; fi
  if [ "$(frames "$mux/gap.mp4")" = 180 ]; then pass "a take with a hole keeps every frame"; else fail "a take with a hole lost frames: $(frames "$mux/gap.mp4")"; fi
  if awk -v a="$(aduration "$mux/gap.mp4")" 'BEGIN { exit !(a > 5.8 && a < 6.3) }'; then pass "the audio track is as long as the picture"; else fail "the audio track is $(aduration "$mux/gap.mp4")s against 6s of picture"; fi
  if loud "$(window_db "$mux/gap.mp4" 0.2 1.5)"; then pass "the sound before the hole is where it was said"; else fail "the sound before the hole is missing: $(window_db "$mux/gap.mp4" 0.2 1.5) dB"; fi
  if quiet "$(window_db "$mux/gap.mp4" 2.3 1.4)"; then pass "the hole is silence at its true offset"; else fail "the hole is not silent: $(window_db "$mux/gap.mp4" 2.3 1.4) dB"; fi
  if loud "$(window_db "$mux/gap.mp4" 4.3 1.5)"; then pass "the sound after the hole did not slide earlier"; else fail "the sound after the hole slid: $(window_db "$mux/gap.mp4" 4.3 1.5) dB"; fi
  case "$(comment "$mux/gap.mp4")" in
    *"audio missing 2.0s-4.0s"*) pass "the file names the window its sound is missing from" ;;
    *) fail "the file does not name the window: $(comment "$mux/gap.mp4")" ;;
  esac

  # The other half: a take whose microphone died and never came back.
  # The mux carried `-shortest`, so the video was cut to the audio's length
  # and the frames past it were deleted with the pre-mux halves. The picture
  # is the length of the take; the sound is padded to it.
  make_video "$mux/short.mp4"
  make_tone "$mux/short.mp4.audio.wav" 2
  printf '0\t0.000\t%s\n' "$mux/short.mp4.audio.wav" > "$CODEROS_SCREEN_RECORD_META.audio.segments"
  audio_segments "$mux/short.mp4" > "$mux/place"
  missing=$(audio_outages "$mux/place" 6.000000 "2.000")
  if [ "$missing" = "2.0s-6.0s" ]; then pass "a take that lost its microphone names the window to the end"; else fail "the trailing hole was named '$missing'"; fi
  note="coderos: video 6.000000s, 180 frames at 30 fps constant; audio missing ${missing}"
  if mux_recording "$mux/short.mp4" 6.000000 "$note" "$mux/place" ""; then pass "a take with short audio muxes"; else fail "a take with short audio did not mux"; fi
  if [ "$(frames "$mux/short.mp4")" = 180 ]; then pass "short audio does not shorten the picture"; else fail "short audio cut the picture to $(frames "$mux/short.mp4") frames"; fi
  if awk -v a="$(aduration "$mux/short.mp4")" 'BEGIN { exit !(a > 5.8 && a < 6.3) }'; then pass "short audio is padded to the picture"; else fail "the audio track is $(aduration "$mux/short.mp4")s against 6s of picture"; fi

  # A microphone that delivered nothing at all leaves a 44-byte header. It is
  # dropped rather than muxed, the video is not touched, and the file still
  # says the whole take is silent.
  make_video "$mux/silent.mp4"
  head -c 44 /dev/zero > "$mux/silent.mp4.audio.wav"
  printf '0\t0.000\t%s\n' "$mux/silent.mp4.audio.wav" > "$CODEROS_SCREEN_RECORD_META.audio.segments"
  audio_segments "$mux/silent.mp4" > "$mux/place"
  if [ ! -s "$mux/place" ]; then pass "a header with no samples is not a piece of audio"; else fail "an empty capture was taken for sound"; fi
  if [ ! -f "$mux/silent.mp4.audio.wav" ]; then pass "the empty capture is dropped"; else fail "the empty capture was kept"; fi
  if tag_recording "$mux/silent.mp4" "coderos: video 6.000000s, 180 frames at 30 fps constant; audio missing 0.0s-6.000000s"; then pass "a silent take is tagged"; else fail "a silent take could not be tagged"; fi
  if [ "$(frames "$mux/silent.mp4")" = 180 ]; then pass "a silent take keeps every frame"; else fail "a silent take lost frames: $(frames "$mux/silent.mp4")"; fi
  case "$(comment "$mux/silent.mp4")" in
    *"audio missing 0.0s-6.000000s"*) pass "a silent take says so in its own metadata" ;;
    *) fail "a silent take says nothing: $(comment "$mux/silent.mp4")" ;;
  esac

  # `stop` end to end over the same state a take leaves behind: the ledger,
  # the pieces, and a video the recorder already finalised. What is checked
  # is the wiring — that the pieces are found, the windows are worked out,
  # the tag is written, and the pieces are cleaned up — rather than the
  # placement, which the cases above own.
  make_video "$mux/take.mp4"
  make_tone "$mux/take.mp4.audio.wav" 2
  make_tone "$mux/take.mp4.audio.001.wav" 2
  {
    printf '0\t0.000\t%s\n' "$mux/take.mp4.audio.wav"
    printf '1\t4.000\t%s\n' "$mux/take.mp4.audio.001.wav"
  } > "$CODEROS_SCREEN_RECORD_META.audio.segments"
  echo "$mux/take.mp4" > "$CODEROS_SCREEN_RECORD_META"
  echo 30 > "$CODEROS_SCREEN_RECORD_META.fps"
  sleep 300 &
  live=$!
  remember "$live"
  disown 2>/dev/null || true
  echo "$live" > "$CODEROS_SCREEN_RECORD_PID"
  said=$(stop_recording 2>&1) || true
  kill "$live" 2>/dev/null || true
  case "$said" in
    *"The audio is missing 2.0s-4.0s"*) pass "stop says where the sound is missing" ;;
    *) fail "stop did not say where the sound is missing: $said" ;;
  esac
  case "$said" in
    *"reattached 1 time(s)"*) pass "stop says the microphone was reattached" ;;
    *) fail "stop did not report the reattach: $said" ;;
  esac
  if [ "$(frames "$mux/take.mp4")" = 180 ]; then pass "stop keeps every frame of the take"; else fail "stop lost frames: $(frames "$mux/take.mp4")"; fi
  case "$(comment "$mux/take.mp4")" in
    *"audio missing 2.0s-4.0s"*) pass "stop writes the window into the file" ;;
    *) fail "stop wrote no window: $(comment "$mux/take.mp4")" ;;
  esac
  if [ ! -f "$mux/take.mp4.audio.wav" ] && [ ! -f "$mux/take.mp4.audio.001.wav" ]; then pass "stop clears the pieces it muxed"; else fail "stop left the pieces behind"; fi
  if [ ! -f "$CODEROS_SCREEN_RECORD_META.audio.segments" ]; then pass "stop clears the ledger"; else fail "stop left the ledger behind"; fi

  # `stop` over the state the 2026-09-11 take left: one capture that never
  # returned, a ledger with one line at offset 0, and a take four seconds
  # short of its picture with nothing to say where those seconds went. The
  # tag may not name the end, because a hole named at the end says the tail
  # is silent and the rest is sound, and neither is true.
  make_video "$mux/unplaced.mp4"
  make_tone "$mux/unplaced.mp4.audio.wav" 2
  printf '0\t0.000\t%s\n' "$mux/unplaced.mp4.audio.wav" > "$CODEROS_SCREEN_RECORD_META.audio.segments"
  rm -f "$CODEROS_SCREEN_RECORD_META.audio.outage"
  echo "$mux/unplaced.mp4" > "$CODEROS_SCREEN_RECORD_META"
  echo 30 > "$CODEROS_SCREEN_RECORD_META.fps"
  sleep 300 &
  live=$!
  remember "$live"
  disown 2>/dev/null || true
  echo "$live" > "$CODEROS_SCREEN_RECORD_PID"
  said=$(stop_recording 2>&1) || true
  kill "$live" 2>/dev/null || true
  case "$(comment "$mux/unplaced.mp4")" in
    *"audio missing"*) fail "a hole nothing located was named at the end: $(comment "$mux/unplaced.mp4")" ;;
    *"short"*) pass "a hole nothing located is called short rather than placed at the end" ;;
    *) fail "a hole nothing located says nothing: $(comment "$mux/unplaced.mp4")" ;;
  esac
  case "$said" in
    *"does not say where"*) pass "stop says the sound is short and the place unknown" ;;
    *) fail "stop did not say the place is unknown: $said" ;;
  esac
  if [ "$(frames "$mux/unplaced.mp4")" = 180 ]; then pass "a hole nothing located keeps every frame"; else fail "a hole nothing located lost frames: $(frames "$mux/unplaced.mp4")"; fi

  rm -f "$CODEROS_SCREEN_RECORD_PID" "$CODEROS_SCREEN_RECORD_META" \
    "$CODEROS_SCREEN_RECORD_META.fps" "$CODEROS_SCREEN_RECORD_META.audio.segments"
fi

if [ "$failures" -ne 0 ]; then
  echo "$failures test(s) failed" >&2
  exit 1
fi
echo "all screen-record tests passed"
