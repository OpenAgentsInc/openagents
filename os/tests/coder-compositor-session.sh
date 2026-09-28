#!/usr/bin/env bash
#
# Tests for `os/bin/coder-compositor-session`, the script a login runs when
# the desktop's compositor is the Coder one.
#
# Run it by hand:
#
#   os/tests/coder-compositor-session.sh
#
# There is no compositor here, so the compositor is a stub: it prints the
# announcement line the real one prints, waits for a file, and exits with the
# status the test asks for. What is tested is the whole of the decision the
# script makes: what it reads out of the grant, which backend it starts, which
# session a nested run opens its window in, how it reads the two sockets out
# of the announcement, that every row of the start list runs with those two
# sockets in its environment, that it exits with the compositor's own status,
# that the run stops what its start list opened before it returns, that a
# run that asks for it starts the compositor again after a clean exit and
# not after a failure, that a login on a TTY starts the hardware backend
# with no display in its environment, that a TTY session asks Xwayland for
# its TTY's number as its X11 display, that a nested run names none, and
# that a nested run with no session to open in says so.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
script="$root/os/bin/coder-compositor-session"

if ! command -v jq > /dev/null 2>&1; then
  echo "FAIL jq is missing, and these tests need it" >&2
  exit 1
fi

scratch=$(mktemp -d "${TMPDIR:-/tmp}/coder-compositor-session-test.XXXXXX")
trap 'rm -rf "$scratch"' EXIT

failures=0
pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; failures=$((failures + 1)); }

# The functions, with nothing started.
CODER_COMPOSITOR_SESSION_LIB=1
export CODER_COMPOSITOR_SESSION_LIB

# What the grant says.
grant="$scratch/compositor.json"
cat > "$grant" << JSON
{
  "compositor": "/nix/store/stub/bin/coder-compositor",
  "pane": "/nix/store/stub/bin/coder-pane",
  "terminal": "foot",
  "command": "coder",
  "start": ["/nix/store/stub/bin/coder-pane", "mako"],
  "launchers": ["deck", "camera"]
}
JSON

CODER_COMPOSITOR_CONFIG="$grant"
export CODER_COMPOSITOR_CONFIG
# shellcheck source=/dev/null
source "$script"

read_launchers=$(launchers)
[[ $read_launchers == "deck
camera" ]] \
  && pass "the launchers are read by the option that gates each one" \
  || fail "the launchers are read by the option that gates each one: got $read_launchers"

[[ $COMPOSITOR == "/nix/store/stub/bin/coder-compositor" ]] \
  && pass "the grant names the compositor to run" \
  || fail "the grant names the compositor to run: got $COMPOSITOR"

# A run names its log after the terminal it holds, so a session on a TTY
# always writes the same file and a test run over SSH writes one of its
# own. Until September 2026 every run wrote and emptied `coder-compositor.log`.
XDG_RUNTIME_DIR=/run/user/1000 log_on_tty=$(compositor_log /dev/tty2)
[[ $log_on_tty == "/run/user/1000/coder-compositor.tty2.log" ]] \
  && pass "a session on a TTY names its log after that TTY" \
  || fail "a session on a TTY names its log after that TTY: got $log_on_tty"

XDG_RUNTIME_DIR=/run/user/1000 log_off_tty=$(compositor_log "not a tty")
[[ $log_off_tty == "/run/user/1000/coder-compositor.$$.log" ]] \
  && pass "a run with no TTY names its log after itself" \
  || fail "a run with no TTY names its log after itself: got $log_off_tty"

[[ $log_on_tty != "$log_off_tty" ]] \
  && pass "a test run cannot write the session's log" \
  || fail "a test run cannot write the session's log: both are $log_on_tty"

# The landmark sockets the daemon and the compositor share, named for the
# session rather than left to each side's default.
(
  export XDG_RUNTIME_DIR=/run/user/1000
  camera_sockets
  [[ $CODEROS_CAMERA_DIR == "/run/user/1000/coderos-camera" ]] \
    && pass "the session names the camera daemon's directory" \
    || fail "the session names the camera daemon's directory: got $CODEROS_CAMERA_DIR"
  [[ $CODEROS_HANDS_SOCKET == "/run/user/1000/coderos-camera/hands.sock" ]] \
    && pass "the session names the landmark socket" \
    || fail "the session names the landmark socket: got $CODEROS_HANDS_SOCKET"
) || failures=$((failures + 1))

[[ $PANE == "/nix/store/stub/bin/coder-pane" ]] \
  && pass "the grant names the window a chord opens" \
  || fail "the grant names the window a chord opens: got $PANE"

[[ $TERMINAL == "foot" ]] \
  && pass "the grant names the terminal that window falls back to" \
  || fail "the grant names the terminal that window falls back to: got $TERMINAL"

read_rows=$(rows)
[[ $read_rows == "/nix/store/stub/bin/coder-pane
mako" ]] \
  && pass "the start list is read in the order the host granted" \
  || fail "the start list is read in the order the host granted: got $read_rows"

# A checkout with no grant runs on the defaults, and starts nothing.
CONFIG="$scratch/absent.json"
[[ $(setting compositor "$DEFAULT_COMPOSITOR") == "coder-compositor" ]] \
  && pass "a run with no grant runs the compositor on PATH" \
  || fail "a run with no grant runs the compositor on PATH"
[[ -z $(rows) ]] \
  && pass "a run with no grant opens no start list" \
  || fail "a run with no grant opens no start list"
[[ -z $(launchers) ]] \
  && pass "a run with no grant names no launchers, so the compositor answers every one" \
  || fail "a run with no grant names no launchers"
CODER_COMPOSITOR_CONFIG="$grant"

# The backend: the hardware on a TTY, a window inside a session, and the one
# the grant or the environment names when either names one.
unset WAYLAND_DISPLAY
[[ $(choose_backend) == "udev" ]] \
  && pass "a login on a TTY starts the hardware backend" \
  || fail "a login on a TTY starts the hardware backend: got $(choose_backend)"
WAYLAND_DISPLAY=wayland-4
[[ $(choose_backend) == "winit" ]] \
  && pass "a run inside a session starts the nested backend" \
  || fail "a run inside a session starts the nested backend: got $(choose_backend)"
[[ $(CODER_COMPOSITOR_BACKEND=udev choose_backend) == "udev" ]] \
  && pass "the environment names the backend over the session" \
  || fail "the environment names the backend over the session"
cat > "$scratch/winit.json" << JSON
{ "backend": "winit" }
JSON
unset WAYLAND_DISPLAY
[[ $(CONFIG="$scratch/winit.json" choose_backend) == "winit" ]] \
  && pass "the grant names the backend" \
  || fail "the grant names the backend: got $(CONFIG="$scratch/winit.json" choose_backend)"
if CODER_COMPOSITOR_BACKEND=x11 choose_backend > /dev/null 2>&1; then
  fail "a backend the compositor does not have is refused"
else
  pass "a backend the compositor does not have is refused"
fi

# The session the nested compositor draws its window in.
WAYLAND_DISPLAY=wayland-4
[[ $(host_display) == "wayland-4" ]] \
  && pass "a run inside a session draws in that session" \
  || fail "a run inside a session draws in that session"
unset WAYLAND_DISPLAY

# A login on the trial TTY has no session of its own, so the socket the
# session on tty1 bound is what it finds.
XDG_RUNTIME_DIR="$scratch/run"
export XDG_RUNTIME_DIR
mkdir -p "$XDG_RUNTIME_DIR"
if host_display > /dev/null 2>&1; then
  fail "a run with no session at all reports none"
else
  pass "a run with no session at all reports none"
fi

python3 - "$XDG_RUNTIME_DIR/wayland-1" << 'PY'
import socket
import sys

server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(sys.argv[1])
PY
touch "$XDG_RUNTIME_DIR/wayland-1.lock"
[[ $(host_display) == "wayland-1" ]] \
  && pass "a nested run from the trial TTY draws in the session on tty1" \
  || fail "a nested run from the trial TTY draws in the session on tty1: got $(host_display)"

# The announcement, which is the only place the two sockets are named.
log="$scratch/announcement.log"
: > "$log"
[[ -z $(announcement "$log") ]] \
  && pass "a compositor that has said nothing announces nothing" \
  || fail "a compositor that has said nothing announces nothing"

cat > "$log" << 'LOG'
[INFO] starting
[INFO] the compositor listens on wayland-2, and its desk socket is /run/user/1000/coder-desk/7.sock
LOG
[[ $(announcement "$log") == "wayland-2 /run/user/1000/coder-desk/7.sock" ]] \
  && pass "the two sockets are read out of the compositor's first line" \
  || fail "the two sockets are read out of the compositor's first line: got $(announcement "$log")"

unset CODER_COMPOSITOR_SESSION_LIB

# The whole run, against a compositor that announces its sockets, waits, and
# exits with the status the test asks for.
stub="$scratch/stub-compositor"
cat > "$stub" << 'STUB'
#!/usr/bin/env bash
printf '%s WAYLAND_DISPLAY=%s DISPLAY=%s X11=%s\n' "$*" "${WAYLAND_DISPLAY:-}" \
  "${DISPLAY:-}" "${CODER_COMPOSITOR_X11_DISPLAY:-}" > "$STUB_ARGS"
printf 'the compositor listens on %s, and its desk socket is %s\n' \
  "$STUB_DISPLAY" "$STUB_SOCKET"
for _ in $(seq 100); do
  [[ -e $STUB_STOP ]] && break
  sleep 0.1
done
exit "$STUB_STATUS"
STUB
chmod +x "$stub"

opened="$scratch/opened"
cat > "$scratch/run.json" << JSON
{
  "compositor": "$stub",
  "pane": "panestub",
  "terminal": "footstub",
  "command": "coderstub",
  "start": [
    "echo one \"\$WAYLAND_DISPLAY\" \"\$CODER_DESK_SOCKET\" >> $opened",
    "echo two \"\$CODER_COMPOSITOR_PANE\" \"\$CODER_COMPOSITOR_TERMINAL\" >> $opened",
    "echo three \"\$CODER_COMPOSITOR_LAUNCHERS\" >> $opened"
  ],
  "launchers": ["deck", "camera"]
}
JSON

: > "$opened"
export STUB_DISPLAY=wayland-9
export STUB_SOCKET="$scratch/desk.sock"
export STUB_STOP="$scratch/stop"
export STUB_STATUS=7
export STUB_ARGS="$scratch/args"
CODER_COMPOSITOR_CONFIG="$scratch/run.json" WAYLAND_DISPLAY=wayland-1 \
  bash "$script" > "$scratch/session.log" 2>&1 &
session=$!

for _ in $(seq 100); do
  [[ $(wc -l < "$opened") -ge 3 ]] && break
  sleep 0.1
done

if grep -qx "one wayland-9 $STUB_SOCKET" "$opened"; then
  pass "every row of the start list runs in the compositor the run started"
else
  fail "every row of the start list runs in the compositor the run started: $(cat "$opened")"
fi

if grep -qx "two panestub footstub" "$opened"; then
  pass "a row reads the window and the terminal the grant named"
else
  fail "a row reads the window and the terminal the grant named: $(cat "$opened")"
fi

if grep -qx "three deck camera" "$opened"; then
  pass "the compositor is told the launchers the grant named, on one line"
else
  fail "the compositor is told the launchers the grant named, on one line: $(cat "$opened")"
fi

touch "$STUB_STOP"
status=0
wait "$session" || status=$?
[[ $status -eq 7 ]] \
  && pass "the run ends with the compositor's own status" \
  || fail "the run ends with the compositor's own status: got $status"

if grep -qx -- "--backend winit WAYLAND_DISPLAY=wayland-1 DISPLAY=${DISPLAY:-} X11=" "$STUB_ARGS"; then
  pass "a run inside a session opens the compositor as a window in it"
else
  fail "a run inside a session opens the compositor as a window in it: $(cat "$STUB_ARGS")"
fi

# A login on a TTY starts the hardware backend, with no display left in its
# environment to send it to a window.
rm -f "$STUB_STOP" "$STUB_ARGS"
: > "$opened"
(
  unset WAYLAND_DISPLAY XDG_VTNR CODER_COMPOSITOR_X11_DISPLAY
  CODER_COMPOSITOR_CONFIG="$scratch/run.json" DISPLAY=:9 \
    bash "$script" > "$scratch/tty.log" 2>&1
) &
session=$!
for _ in $(seq 100); do
  [[ -s $STUB_ARGS ]] && break
  sleep 0.1
done
touch "$STUB_STOP"
wait "$session" || true
if grep -qx -- "--backend udev WAYLAND_DISPLAY= DISPLAY= X11=" "$STUB_ARGS"; then
  pass "a login on a TTY starts the hardware backend with no display in its environment"
else
  fail "a login on a TTY starts the hardware backend: $(cat "$STUB_ARGS" 2> /dev/null)"
fi

# A session asks Xwayland for its own TTY's number, whichever order the
# sessions started in, so the `game` grant can name the tty1 session's
# display. A login that names no VT asks for none.
rm -f "$STUB_STOP" "$STUB_ARGS"
(
  unset WAYLAND_DISPLAY
  CODER_COMPOSITOR_CONFIG="$scratch/run.json" XDG_VTNR=1 \
    bash "$script" > "$scratch/tty1.log" 2>&1
) &
session=$!
for _ in $(seq 100); do
  [[ -s $STUB_ARGS ]] && break
  sleep 0.1
done
touch "$STUB_STOP"
wait "$session" || true
if grep -qx -- "--backend udev WAYLAND_DISPLAY= DISPLAY= X11=:1" "$STUB_ARGS"; then
  pass "the tty1 session asks Xwayland for :1"
else
  fail "the tty1 session asks Xwayland for :1: $(cat "$STUB_ARGS" 2> /dev/null)"
fi

rm -f "$STUB_STOP" "$STUB_ARGS"
(
  unset WAYLAND_DISPLAY
  CODER_COMPOSITOR_CONFIG="$scratch/run.json" XDG_VTNR=2 \
    bash "$script" > "$scratch/tty2.log" 2>&1
) &
session=$!
for _ in $(seq 100); do
  [[ -s $STUB_ARGS ]] && break
  sleep 0.1
done
touch "$STUB_STOP"
wait "$session" || true
if grep -qx -- "--backend udev WAYLAND_DISPLAY= DISPLAY= X11=:2" "$STUB_ARGS"; then
  pass "a second TTY's session asks Xwayland for its own number"
else
  fail "a second TTY's session asks Xwayland for its own number: $(cat "$STUB_ARGS" 2> /dev/null)"
fi

# A nested run owns no TTY, so a display a session left in the environment
# must not reach it.
rm -f "$STUB_STOP" "$STUB_ARGS"
(
  CODER_COMPOSITOR_CONFIG="$scratch/run.json" WAYLAND_DISPLAY=wayland-1 \
    XDG_VTNR=1 CODER_COMPOSITOR_X11_DISPLAY=:1 \
    bash "$script" > "$scratch/nested.log" 2>&1
) &
session=$!
for _ in $(seq 100); do
  [[ -s $STUB_ARGS ]] && break
  sleep 0.1
done
touch "$STUB_STOP"
wait "$session" || true
if grep -qx -- "--backend winit WAYLAND_DISPLAY=wayland-1 DISPLAY=${DISPLAY:-} X11=" "$STUB_ARGS"; then
  pass "a nested run names no X11 display, so it cannot take a TTY's"
else
  fail "a nested run names no X11 display: $(cat "$STUB_ARGS" 2> /dev/null)"
fi

# A nested run with no session to open in is told what is missing rather
# than the event loop error the nested backend raises.
rm -f "$XDG_RUNTIME_DIR/wayland-1" "$XDG_RUNTIME_DIR/wayland-1.lock"
status=0
(
  unset WAYLAND_DISPLAY
  CODER_COMPOSITOR_CONFIG="$scratch/run.json" CODER_COMPOSITOR_BACKEND=winit \
    bash "$script" > "$scratch/alone.log" 2>&1
) || status=$?
[[ $status -ne 0 ]] \
  && pass "a nested run with no session to open in fails" \
  || fail "a nested run with no session to open in fails"
if grep -q "no Wayland session is running" "$scratch/alone.log"; then
  pass "a nested run with no session to open in says what is missing"
else
  fail "a nested run with no session to open in says what is missing: $(cat "$scratch/alone.log")"
fi

# The run stops what its start list opened before it returns — by pid, so a
# process this run never started is never reached — and the processes under
# a row go with it. `camera-overlay`'s `mpv` printed to the trial TTY's
# console for the whole time it took to die after the compositor quit.
rm -f "$STUB_STOP" "$STUB_ARGS"
cat > "$scratch/cleanup.json" << JSON
{
  "compositor": "$stub",
  "start": [
    "echo \$\$ > $scratch/child.pid; sleep 60",
    "sleep 60 & echo \$! > $scratch/grandchild.pid; wait"
  ]
}
JSON
CODER_COMPOSITOR_CONFIG="$scratch/cleanup.json" WAYLAND_DISPLAY=wayland-1 \
  STUB_STATUS=0 bash "$script" > "$scratch/cleanup.log" 2>&1 &
session=$!
for _ in $(seq 100); do
  [[ -s $scratch/child.pid && -s $scratch/grandchild.pid ]] && break
  sleep 0.1
done
if [[ -s $scratch/child.pid && -s $scratch/grandchild.pid ]]; then
  child=$(cat "$scratch/child.pid")
  grandchild=$(cat "$scratch/grandchild.pid")
else
  fail "the start list opened its rows"
  child=0
  grandchild=0
fi
touch "$STUB_STOP"
status=0
wait "$session" || status=$?
[[ $status -eq 0 ]] \
  && pass "a clean exit still returns the compositor's own status" \
  || fail "a clean exit still returns the compositor's own status: got $status"
alive "$child" \
  && fail "the run stops a process its start list opened" \
  || pass "the run stops a process its start list opened"
alive "$grandchild" \
  && fail "the run stops a process under a start-list row" \
  || pass "the run stops a process under a start-list row"

# A run that asks for it restarts after a clean exit: the trial TTY's login
# sets CODER_COMPOSITOR_RESTART, and the quit chord brings a fresh
# compositor rather than a prompt. A failure returns to the shell.
stub_restart="$scratch/stub-restart"
cat > "$stub_restart" << 'STUB2'
#!/usr/bin/env bash
printf 'run\n' >> "$STUB_RUNS"
printf 'the compositor listens on %s, and its desk socket is %s\n' \
  "$STUB_DISPLAY" "$STUB_SOCKET"
exit "$STUB_STATUS"
STUB2
chmod +x "$stub_restart"
cat > "$scratch/restart.json" << JSON
{
  "compositor": "$stub_restart",
  "start": ["echo \$\$ >> $scratch/row-pids; sleep 60"]
}
JSON
: > "$scratch/runs"
: > "$scratch/row-pids"
STUB_RUNS="$scratch/runs" STUB_STATUS=0 \
  CODER_COMPOSITOR_CONFIG="$scratch/restart.json" WAYLAND_DISPLAY=wayland-1 \
  CODER_COMPOSITOR_RESTART=1 CODER_COMPOSITOR_RESTART_PAUSE=0.2 \
  bash "$script" > "$scratch/restart.log" 2>&1 &
session=$!
sleep 4
runs=$(wc -l < "$scratch/runs" | tr -d ' ')
# A backgrounded shell ignores INT, so the test ends the loop with TERM.
kill -TERM "$session" 2> /dev/null || true
wait "$session" || true
[[ $runs -ge 2 ]] \
  && pass "a clean exit starts the compositor again when the run asks for it" \
  || fail "a clean exit starts the compositor again when the run asks for it: $runs run(s)"
if grep -q 'a fresh one starts' "$scratch/restart.log"; then
  pass "the run says on the console that the compositor is restarting"
else
  fail "the run says on the console that the compositor is restarting: $(cat "$scratch/restart.log")"
fi
if [[ -s $scratch/row-pids ]]; then
  first_row=$(head -1 "$scratch/row-pids")
  alive "$first_row" \
    && fail "a run stops the rows it opened before the next starts" \
    || pass "a run stops the rows it opened before the next starts"
else
  fail "a restarting run opened its start list"
fi

: > "$scratch/runs"
status=0
STUB_RUNS="$scratch/runs" STUB_STATUS=3 \
  CODER_COMPOSITOR_CONFIG="$scratch/restart.json" WAYLAND_DISPLAY=wayland-1 \
  CODER_COMPOSITOR_RESTART=1 CODER_COMPOSITOR_RESTART_PAUSE=0.2 \
  bash "$script" > "$scratch/fail-run.log" 2>&1 || status=$?
runs=$(wc -l < "$scratch/runs" | tr -d ' ')
[[ $status -eq 3 && $runs -eq 1 ]] \
  && pass "a failed compositor returns its status rather than looping" \
  || fail "a failed compositor returns its status rather than looping: status $status, $runs run(s)"

if ((failures > 0)); then
  printf '\n%d test(s) failed\n' "$failures" >&2
  exit 1
fi
printf '\nall tests passed\n'
