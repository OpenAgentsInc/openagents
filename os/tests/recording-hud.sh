#!/usr/bin/env bash
#
# Tests for `os/bin/recording-hud`, the strip that docks under the camera
# circle and starts or stops a recording.
#
# Run it by hand:
#
#   os/tests/recording-hud.sh
#
# There is no desk and no recorder here, so `coder-desk` and `screen-record`
# are stubs: `coder-desk` answers the camera and screen reads the strip makes
# and logs every change verb it asks for, and `screen-record` records its verb
# and keeps a pid file, the shared truth the strip reads. The script is sourced
# with `RECORDING_HUD_LIB` set, which defines its functions and stops before
# the run, so the test calls `is_recording`, `toggle_recording`,
# `resize_camera`, `place_hud`, and `mic_label` directly.
#
# The tests the strip exists for are the docking math, the record wiring, and
# the microphone label: a strip that lands under the circle, a button that
# starts audio and reflects the recorder's own state, and a label that turns
# red when the recording reads the wrong microphone, none, or one that
# delivers nothing.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
script="$root/os/bin/recording-hud"

if ! command -v jq > /dev/null 2>&1; then
  echo "FAIL jq is missing, and these tests need it" >&2
  exit 1
fi

scratch=$(mktemp -d "${TMPDIR:-/tmp}/coder-recording-hud-test.XXXXXX")
# The trap ends what the tests start on every exit path, including the one an
# unexpected failure takes under `set -e`: the stub recorder the pid file names
# and the stand-in process one test reads as a live recording.
trap 'kill "$(cat "$scratch/rec.pid" 2>/dev/null)" 2>/dev/null || true;
      kill "${live:-}" 2>/dev/null || true;
      kill "${dictating:-}" 2>/dev/null || true;
      rm -rf "$scratch"' EXIT

failures=0
pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; failures=$((failures + 1)); }

# The stub desk. It answers `list` and `screens` from the environment the
# test sets, and logs every change verb to a file the test reads back. A
# `list` counts itself in `TEST_LIST_COUNT`, and the camera stays out of the
# answer until the count reaches `TEST_CAM_AFTER`, which is a camera whose
# window maps after the strip's. `TEST_HUD=1` lists the strip's window too.
stubs="$scratch/stubs"
mkdir -p "$stubs"
log="$scratch/desk.log"
: > "$log"

cat > "$stubs/coder-desk" << 'STUB'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  list)
    count=0
    if [ -n "${TEST_LIST_COUNT:-}" ]; then
      count=$(cat "$TEST_LIST_COUNT" 2>/dev/null || echo 0)
      count=$((count + 1))
      echo "$count" > "$TEST_LIST_COUNT"
    fi
    rows=""
    if [ "${TEST_NO_CAM:-0}" != "1" ] && [ "$count" -ge "${TEST_CAM_AFTER:-0}" ]; then
      rows=$(printf '{"title":"selfie","at":{"x":%s,"y":%s},"size":{"width":%s,"height":%s}}' \
        "${TEST_CAM_X:-1637}" "${TEST_CAM_Y:-440}" \
        "${TEST_CAM_W:-560}" "${TEST_CAM_H:-560}")
    fi
    if [ "${TEST_HUD:-0}" = "1" ]; then
      hud='{"title":"recording-hud","at":{"x":1283,"y":6},"size":{"width":1274,"height":1428}}'
      rows="${rows:+$rows,}$hud"
    fi
    echo "[$rows]"
    ;;
  screens)
    printf '[{"name":"DP-1","at":{"x":0,"y":0},"size":{"width":2560,"height":%s},"scale":1.0,"desk":1}]\n' \
      "${TEST_MON_H:-1440}"
    ;;
  *)
    echo "$*" >> "$TEST_LOG"
    ;;
esac
STUB
chmod +x "$stubs/coder-desk"

# The stub recorder. `start` keeps a live pid the way screen-record does, so
# `is_recording` reads a real process; `stop` ends it; `microphone` answers
# with the JSON the real one prints. All three log their verb.
cat > "$stubs/screen-record" << 'STUB'
#!/usr/bin/env bash
set -euo pipefail
echo "$*" >> "$TEST_REC_LOG"
case "${1:-}" in
  microphone)
    echo '{"id":76,"name":"alsa_input.usb-Generic_Blue_Microphones-00.analog-stereo","description":"Blue Microphones Analog Stereo","nick":"Blue Microphones","preferred":true,"recording":false}'
    ;;
  start)
    sleep 300 &
    echo $! > "$CODEROS_SCREEN_RECORD_PID"
    disown 2>/dev/null || true
    ;;
  stop)
    pid=$(cat "$CODEROS_SCREEN_RECORD_PID" 2>/dev/null || true)
    [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
    rm -f "$CODEROS_SCREEN_RECORD_PID"
    ;;
esac
STUB
chmod +x "$stubs/screen-record"

# The stub terminal. These tests run with no terminal on standard input, so
# a real `stty size` would answer nothing. The stub answers the size the test
# asks for, and refuses when the test asks for none, which is the reading a
# strip with no terminal under it gets. The library the test sources asks
# `stty` for nothing else.
cat > "$stubs/stty" << 'STUB'
#!/usr/bin/env bash
set -euo pipefail
if [ "${1:-}" = "size" ]; then
  [ -n "${TEST_STTY_SIZE:-}" ] || exit 1
  echo "$TEST_STTY_SIZE"
fi
STUB
chmod +x "$stubs/stty"
export TEST_STTY_SIZE=""

export PATH="$stubs:$PATH"
export TEST_LOG="$log"
export TEST_REC_LOG="$scratch/rec.log"
: > "$TEST_REC_LOG"
export CODEROS_SCREEN_RECORD_PID="$scratch/rec.pid"
export CODEROS_DICTATION_PID="$scratch/dictation.pid"
export CODEROS_HUD_WATCH_PID="$scratch/watch.pid"
export CODEROS_HUD_HEIGHT=160
export CODEROS_HUD_GAP=8

# Source the script for its functions. The wrapper is `writeShellApplication`'s
# preamble, so the test runs what the module builds, and `RECORDING_HUD_LIB`
# stops it before the dispatch.
runner="$scratch/recording-hud"
{
  printf '#!/usr/bin/env bash\nset -o errexit\nset -o nounset\nset -o pipefail\n'
  cat "$script"
} > "$runner"
export RECORDING_HUD_LIB=1
# shellcheck disable=SC1090
source "$runner"

reset_log() { : > "$log"; }

# The stand-in processes these tests read as recorders. They are declared
# before the trap can name them, so an early failure ends what ran.
live=""
dictating=""

# is_recording reads the recorder's pid file, and nothing else.
rm -f "$CODEROS_SCREEN_RECORD_PID"
if is_recording; then fail "is_recording is true with no pid file"; else pass "is_recording is false with no pid file"; fi

echo 999999 > "$CODEROS_SCREEN_RECORD_PID"
if is_recording; then fail "is_recording is true for a dead pid"; else pass "is_recording is false for a dead pid"; fi
rm -f "$CODEROS_SCREEN_RECORD_PID"

# A live pid reads as recording.
sleep 300 &
live=$!
disown 2>/dev/null || true
echo "$live" > "$CODEROS_SCREEN_RECORD_PID"
if is_recording; then pass "is_recording is true for a live pid"; else fail "is_recording is false for a live pid"; fi
kill "$live" 2>/dev/null || true
rm -f "$CODEROS_SCREEN_RECORD_PID"

# The dictation indicator reads the record `dictate-toggle` writes, and reads
# it the same way: the process, not the file. Dictation draws nothing of its
# own, so a strip that trusted the file would say the microphone was open long
# after it closed, and say nothing when a record was never written.
rm -f "$CODEROS_DICTATION_PID"
if is_dictating; then fail "is_dictating is true with no record"; else pass "is_dictating is false with no record"; fi

echo 999999 > "$CODEROS_DICTATION_PID"
if is_dictating; then fail "is_dictating is true for a record naming a dead pid"; else pass "is_dictating is false for a record naming a dead pid"; fi

printf 'not a pid\n' > "$CODEROS_DICTATION_PID"
if is_dictating; then fail "is_dictating is true for a record naming no number"; else pass "is_dictating is false for a record naming no number"; fi

sleep 300 &
dictating=$!
disown 2>/dev/null || true
printf '%s\n%s\n' "$dictating" "$(date +%s)" > "$CODEROS_DICTATION_PID"
if is_dictating; then pass "is_dictating is true for a record naming a live recorder"; else fail "is_dictating is false for a record naming a live recorder"; fi
kill "$dictating" 2>/dev/null || true
rm -f "$CODEROS_DICTATION_PID"

# Toggling while idle starts a recording with audio and no duration.
: > "$TEST_REC_LOG"
toggle_recording
if grep -q -- "start --audio" "$TEST_REC_LOG"; then pass "toggle while idle starts audio"; else fail "toggle while idle did not start audio"; fi
if is_recording; then pass "toggle while idle leaves a recording running"; else fail "toggle while idle left no recording"; fi

# Toggling while recording stops it.
: > "$TEST_REC_LOG"
toggle_recording
if grep -qx "stop" "$TEST_REC_LOG"; then pass "toggle while recording stops it"; else fail "toggle while recording did not stop"; fi
if is_recording; then fail "toggle while recording left it running"; else pass "toggle while recording ends the recording"; fi

# Resize grows the circle by a step, as an exact square so it stays round.
reset_log
TEST_CAM_W=560 TEST_CAM_H=560 resize_camera 40
if grep -Fxq "shape title:selfie --size 600x600" "$log"; then pass "resize grows the circle to an exact square"; else fail "resize did not grow to an exact square: $(cat "$log")"; fi

# Resize clamps at the ceiling and the floor.
reset_log
TEST_CAM_W=980 TEST_CAM_H=980 resize_camera 40
if grep -Fxq "shape title:selfie --size 1000x1000" "$log"; then pass "resize clamps at the ceiling"; else fail "resize did not clamp at the ceiling: $(cat "$log")"; fi

reset_log
TEST_CAM_W=210 TEST_CAM_H=210 resize_camera -40
if grep -Fxq "shape title:selfie --size 200x200" "$log"; then pass "resize clamps at the floor"; else fail "resize did not clamp at the floor: $(cat "$log")"; fi

# The grid the strip draws in. The control row is the terminal's last row and
# a click reports the row it landed on, so `rows` has to be the row count the
# terminal reports. The strip is 104 pixels tall, which is three rows of the
# session's font, and a floor of four rows made every press on the control row
# report three against a `rows` of four, so no button fired.
grid_is() {
  local size=$1 want_rows=$2 want_wave=$3
  ring=0
  hist=()
  TEST_STTY_SIZE=$size
  read_size
  if [ "$rows" = "$want_rows" ] && [ "$wave_rows" = "$want_wave" ]; then
    pass "a terminal of '${size:-no size}' draws $want_wave meter rows under row $want_rows"
  else
    fail "a terminal of '${size:-no size}' read rows=$rows wave_rows=$wave_rows"
  fi
}
grid_is "3 52" 3 2
grid_is "6 80" 6 5
# Below two rows there is no room for a meter and a control row, so the floor
# holds.
grid_is "1 52" 2 1
# With no terminal to ask, the strip draws in the size a terminal defaults to.
grid_is "" 24 23

# Which control a press lands on. The arguments are the button, the column,
# the row, the press or release letter, and the terminal's last row.
control_is() {
  local want=$1; shift
  local got
  got=$(control_at "$@")
  if [ "$got" = "$want" ]; then pass "control_at $* -> ${want:-nothing}"; else fail "control_at $* -> ${got:-nothing}, wanted ${want:-nothing}"; fi
}
control_is record 0 8 3 M 3
control_is smaller 0 20 3 M 3
control_is bigger 0 26 3 M 3
# A press between two buttons, on another row, on the release, or on another
# button reaches no control.
control_is "" 0 17 3 M 3
control_is "" 0 26 2 M 3
control_is "" 0 26 3 m 3
control_is "" 1 26 3 M 3

# The two together, which is what a press on the live strip runs: the strip
# reads its own grid, and a press on the last row of it reaches the button
# under the column.
ring=0
hist=()
TEST_STTY_SIZE="3 52"
read_size
got=$(control_at 0 26 3 M "$rows")
if [ "$got" = "bigger" ]; then pass "a press on the last row of a three-row strip grows the circle"; else fail "a press on the last row of a three-row strip read '${got:-nothing}'"; fi

# The strip docks under the circle: same x and width, one gap below, its own
# height.
reset_log
TEST_CAM_X=1637 TEST_CAM_Y=440 TEST_CAM_W=560 TEST_CAM_H=560 TEST_MON_H=1440 place_hud
if grep -Fxq "shape title:recording-hud --size 560x160 --at 1637,1008" "$log"; then pass "the strip takes the circle's width and docks one gap under it"; else fail "the strip did not take the circle's width or dock under it: $(cat "$log")"; fi

# When the strip would run off the bottom, it docks above the circle instead.
reset_log
TEST_CAM_X=1637 TEST_CAM_Y=1300 TEST_CAM_W=560 TEST_CAM_H=560 TEST_MON_H=1440 place_hud
if grep -Fxq "shape title:recording-hud --size 560x160 --at 1637,1132" "$log"; then pass "the strip docks above the circle when below would overflow"; else fail "the strip did not dock above the circle: $(cat "$log")"; fi

# With no camera on screen, the strip places nothing.
reset_log
TEST_NO_CAM=1 place_hud || true
if [ -s "$log" ]; then fail "the strip placed itself with no camera: $(cat "$log")"; else pass "the strip places nothing with no camera"; fi

# The camera's geometry reads the desk's own shape, a place with `x` and `y`
# and a size with `width` and `height`, as one line the run loop compares.
got=$(TEST_CAM_X=1857 TEST_CAM_Y=142 TEST_CAM_W=561 TEST_CAM_H=561 camera_geometry)
if [ "$got" = "1857 142 561 561" ]; then pass "camera_geometry reads the desk's place and size"; else fail "camera_geometry read '$got'"; fi
got=$(TEST_NO_CAM=1 camera_geometry)
if [ -z "$got" ]; then pass "camera_geometry reads nothing with no camera"; else fail "camera_geometry read '$got' with no camera"; fi

# The strip started beside the camera: its window maps before the camera's,
# so the shaper's first reads find no `selfie`. The shaper floats the strip,
# waits for the camera, docks the strip under it, then pins and raises it
#.
reset_log
counter="$scratch/list.count"
rm -f "$counter"
TEST_HUD=1 TEST_LIST_COUNT="$counter" TEST_CAM_AFTER=4 \
  TEST_CAM_X=1857 TEST_CAM_Y=142 TEST_CAM_W=561 TEST_CAM_H=561 TEST_MON_H=1440 shape_hud
want=$(printf '%s\n' \
  "shape title:recording-hud --float --no-aspect --border 0 --rounding 0 --no-shadow" \
  "shape title:recording-hud --size 561x160 --at 1857,711" \
  "shape title:recording-hud --pin" \
  "raise title:recording-hud")
if [ "$(cat "$log")" = "$want" ]; then pass "the shaper waits for a camera that maps after the strip and docks under it"; else fail "the shaper did not wait for the camera: $(cat "$log")"; fi
if [ "$(cat "$counter")" -ge 4 ]; then pass "the shaper read the desk until the camera came"; else fail "the shaper read the desk $(cat "$counter") times"; fi

# A camera that never comes leaves the strip floating, pinned, and raised,
# and places nothing.
reset_log
TEST_HUD=1 TEST_NO_CAM=1 CAMERA_TRIES=2 shape_hud
want=$(printf '%s\n' \
  "shape title:recording-hud --float --no-aspect --border 0 --rounding 0 --no-shadow" \
  "shape title:recording-hud --pin" \
  "raise title:recording-hud")
if [ "$(cat "$log")" = "$want" ]; then pass "the shaper places nothing when no camera comes"; else fail "the shaper placed the strip with no camera: $(cat "$log")"; fi

# The microphone label. Its arguments are present, preferred, live, the
# short name, and whether the recording carries no audio track.
label_is() {
  local want=$1; shift
  local got
  got=$(mic_label "$@")
  if [ "$got" = "$want" ]; then pass "mic_label $* -> $want"; else fail "mic_label $* -> $got, wanted $want"; fi
}
MIC_NAME="Blue"
label_is "$(printf 'ok\tMIC · Blue Microphones')" 1 1 1 "Blue Microphones"
label_is "$(printf 'error\tNOT BLUE · Logitech Webcam C930e')" 1 0 1 "Logitech Webcam C930e"
label_is "$(printf 'error\tNO AUDIO · Blue Microphones')" 1 1 0 "Blue Microphones"
label_is "$(printf 'error\tNO MIC')" 0 0 0 ""
label_is "$(printf 'quiet\tVIDEO ONLY')" 1 1 1 "Blue Microphones" 1
# A dead microphone outranks a wrong one: no audio is the louder fault.
label_is "$(printf 'error\tNO AUDIO · Logitech Webcam C930e')" 1 0 0 "Logitech Webcam C930e"
# With no name for the wanted microphone, the label still says it is wrong.
MIC_NAME=""
label_is "$(printf 'error\tWRONG MIC · Logitech Webcam C930e')" 1 0 1 "Logitech Webcam C930e"

# The stub recorder answers the microphone question the strip asks.
if screen-record microphone | grep -q '"nick":"Blue Microphones"'; then pass "the recorder reports the microphone"; else fail "the recorder did not report the microphone"; fi

if [ "$failures" -ne 0 ]; then
  echo "$failures test(s) failed" >&2
  exit 1
fi
echo "all recording-hud tests passed"
