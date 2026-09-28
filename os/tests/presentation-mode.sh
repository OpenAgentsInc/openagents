#!/usr/bin/env bash
#
# Tests for `os/bin/presentation-mode`, the mode that sets a CoderOS desktop
# up to be recorded and puts it back.
#
# Run it by hand:
#
#   os/tests/presentation-mode.sh
#
# There is no desk here, so `coder-desk` is a stub backed by a file that
# stands for the desktop: a screen with a scale, a camera view with a corner,
# and a focused window. The stub answers the same verbs a desk answers and
# applies the same changes, so the script under test does its real work and
# every assertion reads the fake desktop rather than the script's output. A
# scale change moves and resizes the camera view with the logical screen,
# the way the Coder compositor's does, so leaving has to place the view
# after it puts the scale back.
#
# The test the mode exists for is `entering twice keeps the first capture`.
# A second capture would write the presentation scale down as the scale to
# return to, and no command could undo that.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
script="$root/os/bin/presentation-mode"

if ! command -v jq > /dev/null 2>&1; then
  echo "FAIL jq is missing, and these tests need it" >&2
  exit 1
fi

scratch=$(mktemp -d "${TMPDIR:-/tmp}/coder-presentation-test.XXXXXX")
trap 'rm -rf "$scratch"' EXIT

failures=0
pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; failures=$((failures + 1)); }

# The script has no shebang: `pkgs.writeShellApplication` supplies one and the
# strict flags. This wrapper is that preamble, so the test runs what the module
# builds.
runner="$scratch/presentation-mode"
{
  printf '#!/usr/bin/env bash\nset -o errexit\nset -o nounset\nset -o pipefail\n'
  cat "$script"
} > "$runner"
chmod +x "$runner"

# The fake desktop.
desktop="$scratch/desktop"
stubs="$scratch/stubs"
mkdir -p "$stubs"

reset_desktop() {
  cat > "$desktop" << 'STATE'
scale=1.00
camera=yes
camera_x=1860
camera_y=140
camera_w=560
camera_h=560
STATE
}

# `stuck=yes` makes the stub ignore every change, which is how the test drives
# a restore that does not take.
stick_desktop() { echo "stuck=yes" >> "$desktop"; }

field() { grep "^$1=" "$desktop" | tail -1 | cut -d= -f2; }
set_field() {
  local name="$1" value="$2"
  if [ "$(field stuck)" = "yes" ]; then
    return 0
  fi
  printf '%s=%s\n' "$name" "$value" >> "$desktop"
}

cat > "$stubs/coder-desk" << 'STUB'
#!/usr/bin/env bash
set -euo pipefail
desktop="$CODER_TEST_DESKTOP"
field() { grep "^$1=" "$desktop" | tail -1 | cut -d= -f2; }

# What a real client answers a session with: a socket the request names
# has to be there and answer, a file holding "dead" stands for one a dead
# session left, and no socket and no Hyprland signature is no session.
socket="${CODER_DESK_SOCKET:-}"
if [ -n "$socket" ]; then
  if [ ! -e "$socket" ] || grep -q dead "$socket" 2>/dev/null; then
    exit 1
  fi
  put_socket() { printf 'via=%s\n' "$socket" >> "$desktop"; }
  put_socket
elif [ -z "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]; then
  exit 1
fi
put() {
  if [ "$(field stuck)" = "yes" ]; then return 0; fi
  printf '%s=%s\n' "$1" "$2" >> "$desktop"
}
screens() {
  printf '[{"name":"DP-2","at":{"x":0,"y":0},'
  printf '"size":{"width":2560,"height":1440},"scale":%s,"desk":1}]' "$(field scale)"
}

case "${1:-}" in
  screens)
    screens
    printf '\n'
    ;;
  reading)
    printf '{"screens":%s,"focused":"DP-2",' "$(screens)"
    printf '"desks":[{"id":1,"screen":"DP-2","windows":2}]}\n'
    ;;
  list)
    printf '['
    printf '{"handle":"0xfoot","title":"foot","app_id":"foot",'
    printf '"at":{"x":7,"y":7},"size":{"width":100,"height":100}}'
    if [ "$(field camera)" = "yes" ]; then
      printf ',{"handle":"0xcam","title":"selfie","app_id":"mpv",'
      printf '"at":{"x":%s,"y":%s},"size":{"width":%s,"height":%s}}' \
        "$(field camera_x)" "$(field camera_y)" "$(field camera_w)" "$(field camera_h)"
    fi
    printf ']\n'
    ;;
  focused)
    printf '{"handle":"0xfoot","title":"foot","app_id":"foot"}\n'
    ;;
  scale)
    # A float is a fraction of the screen on the compositor, so a scale
    # change carries the camera view with the logical screen.
    old=$(field scale)
    if [ "$(field camera)" = "yes" ]; then
      for name in camera_x camera_y camera_w camera_h; do
        put "$name" "$(jq -n --argjson v "$(field "$name")" --argjson o "$old" \
          --argjson n "$3" '($v * $o / $n) | round')"
      done
    fi
    put scale "$3"
    ;;
  shape)
    # The selector and then the flags, of which this fake reads two.
    shift 2
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --size)
          put camera_w "${2%x*}"
          put camera_h "${2#*x}"
          shift 2
          ;;
        --at)
          put camera_x "${2%,*}"
          put camera_y "${2#*,}"
          shift 2
          ;;
        *) shift ;;
      esac
    done
    ;;
  open)
    if [ "${3:-}" = "camera-overlay start" ]; then
      put camera yes
      put camera_x 1860
      put camera_y 140
      put camera_w 560
      put camera_h 560
    fi
    ;;
  focus) ;;
  *)
    echo "unhandled coder-desk call: $*" >&2
    exit 1
    ;;
esac
STUB

cat > "$stubs/camera-overlay" << 'STUB'
#!/usr/bin/env bash
set -euo pipefail
desktop="$CODER_TEST_DESKTOP"
field() { grep "^$1=" "$desktop" | tail -1 | cut -d= -f2; }
if [ "${1:-}" = "stop" ] && [ "$(field stuck)" != "yes" ]; then
  echo "camera=no" >> "$desktop"
fi
STUB

chmod +x "$stubs/coder-desk" "$stubs/camera-overlay"

home="$scratch/home"
state="$home/.openagents/presentation-mode.json"
mkdir -p "$home"

mode() {
  env HOME="$home" \
      PATH="$stubs:$PATH" \
      CODER_TEST_DESKTOP="$desktop" \
      XDG_RUNTIME_DIR="$scratch/run" \
      HYPRLAND_INSTANCE_SIGNATURE=test \
      CODEROS_PRESENTATION_SCALE=1.25 \
      "$runner" "$@"
}

reset_desktop

# Status before anything.
if mode status | grep -q 'Presentation mode is off'; then
  pass "status says the mode is off before it is entered"
else
  fail "status says the mode is off before it is entered"
fi

# Entering: the state is written, the scale rises, and the camera view moves
# to the top right of the logical screen 1.25 makes of 2560 pixels.
mode on > "$scratch/on.log" 2>&1 || fail "entering the mode exits zero"
if [ -f "$state" ]; then
  pass "entering writes the state under ~/.openagents"
else
  fail "entering writes the state under ~/.openagents"
fi
if [ "$(field scale)" = "1.25" ]; then
  pass "entering raises the screen scale"
else
  fail "entering raises the screen scale (scale is $(field scale))"
fi
if [ "$(field camera_x)" = "1348" ] && [ "$(field camera_y)" = "140" ]; then
  pass "entering puts the camera view in the top right of the logical screen"
else
  fail "entering puts the camera view in the top right (at $(field camera_x),$(field camera_y))"
fi
# jq 1.6 prints 1.00 as 1 and jq 1.7 keeps the literal, so this compares the
# number rather than the text.
if [ "$(printf '%.2f' "$(jq -r '.monitors[0].scale' "$state")")" = "1.00" ]; then
  pass "the state holds the scale the desktop had"
else
  fail "the state holds the scale the desktop had"
fi

# The item this mode exists for. A second entry captures nothing, so the
# desktop it returns to is still the one it started from.
captured=$(cat "$state")
mode on > "$scratch/on-again.log" 2>&1 || fail "entering twice exits zero"
if [ "$(cat "$state")" = "$captured" ]; then
  pass "entering twice keeps the first capture"
else
  fail "entering twice keeps the first capture"
fi
if grep -q 'already on' "$scratch/on-again.log"; then
  pass "entering twice says the mode is already on"
else
  fail "entering twice says the mode is already on"
fi

# Leaving: every item comes back, and the state file goes.
mode off > "$scratch/off.log" 2>&1 || fail "leaving the mode exits zero"
if [ "$(field scale)" = "1.00" ]; then
  pass "leaving puts the screen scale back"
else
  fail "leaving puts the screen scale back (scale is $(field scale))"
fi
if [ "$(field camera_x)" = "1860" ] && [ "$(field camera_y)" = "140" ]; then
  pass "leaving puts the camera view back in its corner"
else
  fail "leaving puts the camera view back (at $(field camera_x),$(field camera_y))"
fi
if [ "$(field camera_w)" = "560" ] && [ "$(field camera_h)" = "560" ]; then
  pass "leaving puts the camera view back at its size after the scale moved it"
else
  fail "leaving puts the camera view back at its size (is $(field camera_w)x$(field camera_h))"
fi
if [ ! -e "$state" ]; then
  pass "a restore that verified removes the state"
else
  fail "a restore that verified removes the state"
fi
if mode off | grep -q 'Nothing to put back'; then
  pass "leaving a mode that is off changes nothing"
else
  fail "leaving a mode that is off changes nothing"
fi

# A person stranded by a run that died leaves from a fresh process. Entering
# writes the state before it changes anything, so the file describes the
# desktop even when the run never reached the desktop at all.
reset_desktop
mode on > /dev/null 2>&1
# The desktop is in presentation state and nothing of the entering run
# survives. This process has never seen it.
if [ "$(field scale)" = "1.25" ]; then
  pass "the desktop is in presentation state before the escape"
else
  fail "the desktop is in presentation state before the escape"
fi
mode off > /dev/null 2>&1
if [ "$(field scale)" = "1.00" ] && [ "$(field camera_x)" = "1860" ]; then
  pass "a fresh process leaves the mode from the saved state"
else
  fail "a fresh process leaves the mode from the saved state"
fi

# The camera view was not up before, so leaving closes the one the mode
# opened rather than leaving it on the screen.
reset_desktop
echo "camera=no" >> "$desktop"
mode on > /dev/null 2>&1
if [ "$(field camera)" = "yes" ]; then
  pass "entering starts a camera view when there was none"
else
  fail "entering starts a camera view when there was none"
fi
mode off > /dev/null 2>&1
if [ "$(field camera)" = "no" ]; then
  pass "leaving closes a camera view the mode opened"
else
  fail "leaving closes a camera view the mode opened"
fi

# A restore that does not take keeps the state file, so leaving can be run
# again. Removing it here is what strands a person for good.
reset_desktop
mode on > /dev/null 2>&1
stick_desktop
if mode off > "$scratch/stuck.log" 2>&1; then
  fail "a restore that did not take reports failure"
else
  pass "a restore that did not take reports failure"
fi
if [ -f "$state" ]; then
  pass "a restore that did not take keeps the state"
else
  fail "a restore that did not take keeps the state"
fi
if grep -q 'run .presentation-mode off. again' "$scratch/stuck.log"; then
  pass "a restore that did not take says to run it again"
else
  fail "a restore that did not take says to run it again"
fi

# A run over ssh carries no session variables, so the script finds the
# session itself: the desk socket it can reach, the way the desk client
# reads it, before the Hyprland signature a session on Hyprland still
# announces.
desks="$scratch/run/coder-desk"
mkdir -p "$desks"
ssh_mode() {
  env -u HYPRLAND_INSTANCE_SIGNATURE -u CODER_DESK_SOCKET "$@" \
      HOME="$home" \
      PATH="$stubs:$PATH" \
      CODER_TEST_DESKTOP="$desktop" \
      XDG_RUNTIME_DIR="$scratch/run" \
      CODEROS_PRESENTATION_SCALE=1.25 \
      "$runner" on
}

# Two sockets, and the newest is one a dead session left: the script names
# the newest one that answers, not the newest file.
reset_desktop
rm -f "$state"
echo live > "$desks/100.sock"
sleep 0.1
echo dead > "$desks/200.sock"
if ssh_mode > /dev/null 2>&1 && [ "$(field via)" = "$desks/100.sock" ]; then
  pass "a run outside the session names the newest desk socket that answers"
else
  fail "a run outside the session names the newest desk socket that answers (via $(field via))"
fi
rm -f "$desks"/*.sock "$state"
reset_desktop

# A socket the environment names is used as given, even beside a newer one
# that answers.
echo live > "$desks/100.sock"
echo live > "$desks/named.sock"
if ssh_mode CODER_DESK_SOCKET="$desks/named.sock" > /dev/null 2>&1 \
    && [ "$(field via)" = "$desks/named.sock" ]; then
  pass "a named socket is used as given"
else
  fail "a named socket is used as given (via $(field via))"
fi
rm -f "$desks"/*.sock "$state"
reset_desktop

# No socket and no signature is no session, and the run says so rather
# than acting on a guess.
if ssh_mode > "$scratch/none.log" 2>&1; then
  fail "a run with no session exits non-zero"
else
  pass "a run with no session exits non-zero"
fi
if grep -q 'No desktop session to change' "$scratch/none.log"; then
  pass "a run with no session says so"
else
  fail "a run with no session says so"
fi

if [ "$failures" -eq 0 ]; then
  echo "presentation-mode: all checks passed"
else
  echo "presentation-mode: $failures check(s) failed" >&2
  exit 1
fi
