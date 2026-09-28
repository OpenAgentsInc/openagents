#!/usr/bin/env bash
#
# Tests for the camera on a CoderOS desktop: the circle `os/bin/camera-overlay`
# reading the node the camera daemon serves, the key `os/bin/camera-toggle`
# asking the daemon before it opens the circle, and the rendering of
# `coderos.desktop.camera` in `os/modules/coderos/camera-grant.nix`.
#
# Run it by hand:
#
#   os/tests/coderos-camera.sh
#
# There is no desk, no player, and no daemon here, so `coder-desk`, `mpv`,
# `recording-hud`, and `coderos-camera` are files under a scratch directory
# that record what they were given, and every assertion reads an exit code
# or those records. One case needs a player that stays up and reaps nothing,
# the way mpv does, so that stub becomes `sleep` and the assertion reads the
# process table. The rendering half evaluates the pure function the
# module reads its grant from with `nix-instantiate`, which needs no
# nixpkgs, and is skipped on a host without `nix`.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
overlay_src="$root/os/bin/camera-overlay"
toggle_src="$root/os/bin/camera-toggle"
grant_src="$root/os/modules/coderos/camera-grant.nix"

if ! command -v jq > /dev/null 2>&1; then
  echo "FAIL jq is missing, and these tests need it" >&2
  exit 1
fi

# A CoderOS host exports the camera's session variables, and a case that
# means to have no loopback reads the host's one through them. Clear them
# here, so every case below reads only what it sets: on coderos-4080 the
# host's `CODEROS_CAMERA_LOOPBACK` failed the case that reads the camera
# itself.
unset CODEROS_CAMERA_DEVICE CODEROS_CAMERA_LOOPBACK CODEROS_CAMERA_CAPTURE \
  CODEROS_CAMERA_FORMAT CODEROS_CAMERA_SIZE CODEROS_CAMERA_MARGIN \
  CODEROS_CAMERA_LOOPBACK_WAIT CODEROS_CAMERA_SHAPE_WAIT

scratch=$(mktemp -d "${TMPDIR:-/tmp}/coderos-camera-test.XXXXXX")
trap 'rm -rf "$scratch"' EXIT

failures=0
pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; failures=$((failures + 1)); }

# The scripts have no shebang: `pkgs.writeShellApplication` supplies one and
# the strict flags. These wrappers are that preamble, so the tests run what
# the module builds.
wrap() {
  {
    printf '#!/usr/bin/env bash\nset -o errexit\nset -o nounset\nset -o pipefail\n'
    cat "$2"
  } > "$1"
  chmod +x "$1"
}
bin="$scratch/bin"
mkdir -p "$bin"
wrap "$bin/camera-overlay" "$overlay_src"
wrap "$bin/camera-toggle" "$toggle_src"

mpv_args="$scratch/mpv-args"
daemon_calls="$scratch/daemon-calls"
daemon_log="$scratch/daemon-log"
clients="$scratch/clients.json"
overlay_calls="$scratch/overlay-calls"
player_pid="$scratch/player-pid"

# The stubs. `mpv` records its arguments and exits; `coder-desk` answers
# `list` from a file and takes every other verb; `coderos-camera` counts its
# calls and answers `outputs` with the loopback up from the `TEST_UP_AFTER`th
# call on, and `status` with the exit code in `TEST_STATUS_EXIT`; `pgrep`
# finds no player, so `start` starts one.
reset_host() {
  rm -f "$mpv_args" "$daemon_calls" "$daemon_log" "$overlay_calls" "$player_pid"
  printf '[]\n' > "$clients"
  cat > "$bin/mpv" << STUB
#!/usr/bin/env bash
printf '%s\n' "\$@" > "$mpv_args"
exit 0
STUB
  cat > "$bin/coder-desk" << STUB
#!/usr/bin/env bash
case "\${1:-}" in
  list) cat "$clients" ;;
  screens) printf '[{"size":{"width":1920,"height":1080}}]\n' ;;
  *) exit 0 ;;
esac
STUB
  cat > "$bin/pgrep" << 'STUB'
#!/usr/bin/env bash
exit 1
STUB
  cat > "$bin/coderos-camera" << STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >> "$daemon_log"
count=0
[ -f "$daemon_calls" ] && count=\$(cat "$daemon_calls")
count=\$((count + 1))
echo "\$count" > "$daemon_calls"
case "\$*" in
  *outputs*)
    if [ "\$count" -ge "\${TEST_UP_AFTER:-1}" ]; then
      printf '{"generation":1,"type":"outputs","outputs":[{"name":"loopback","target":"/dev/video10","state":"up","frames":1,"dropped":0}]}\n'
    else
      printf '{"generation":1,"type":"outputs","outputs":[{"name":"loopback","target":"/dev/video10","state":"absent: not opened yet","frames":0,"dropped":0}]}\n'
    fi
    ;;
  status) exit "\${TEST_STATUS_EXIT:-0}" ;;
  serve) sleep 0.2 ;;
esac
exit 0
STUB
  chmod +x "$bin/mpv" "$bin/coder-desk" "$bin/pgrep" "$bin/coderos-camera"
}

# The overlay reads the loopback node as YUYV once the daemon reports it up.
reset_host
if PATH="$bin:$PATH" CODEROS_CAMERA_DEVICE=/dev/video0 CODEROS_CAMERA_LOOPBACK=/dev/video10 \
  "$bin/camera-overlay" start > /dev/null 2>&1; then
  if grep -q '^av://v4l2:/dev/video10$' "$mpv_args" \
    && grep -q '^--demuxer-lavf-o=input_format=yuyv422,' "$mpv_args"; then
    pass "the overlay opens the loopback node as yuyv422"
  else
    fail "the overlay opened something else: $(tr '\n' ' ' < "$mpv_args")"
  fi
else
  fail "the overlay exited non-zero with the loopback up"
fi

# The overlay keeps the circle's own look: the mask graph, the clear
# background, and the selfie title.
if grep -q '^--title=selfie$' "$mpv_args" \
  && grep -q '^--background=color$' "$mpv_args" \
  && grep -q 'alphamerge\[vo\]$' "$mpv_args"; then
  pass "the overlay keeps the selfie title, the clear background, and the mask"
else
  fail "the overlay lost a row of the circle's look: $(tr '\n' ' ' < "$mpv_args")"
fi

# Without a loopback the overlay reads the camera itself as MJPEG and asks
# the daemon nothing.
reset_host
if PATH="$bin:$PATH" CODEROS_CAMERA_DEVICE=/dev/null "$bin/camera-overlay" start > /dev/null 2>&1; then
  if grep -q '^av://v4l2:/dev/null$' "$mpv_args" \
    && grep -q '^--demuxer-lavf-o=input_format=mjpeg,' "$mpv_args" \
    && [ ! -e "$daemon_log" ]; then
    pass "without a loopback the overlay reads the camera as mjpeg and asks no daemon"
  else
    fail "without a loopback the overlay did something else: $(tr '\n' ' ' < "$mpv_args") $(cat "$daemon_log" 2>/dev/null)"
  fi
else
  fail "the overlay exited non-zero reading the camera itself"
fi

# The overlay waits for the daemon: the loopback comes up on the third
# answer, and the player starts after it.
reset_host
if PATH="$bin:$PATH" CODEROS_CAMERA_LOOPBACK=/dev/video10 TEST_UP_AFTER=3 \
  "$bin/camera-overlay" start > /dev/null 2>&1; then
  if [ "$(cat "$daemon_calls")" -eq 3 ] && grep -q '^av://v4l2:/dev/video10$' "$mpv_args"; then
    pass "the overlay waits for the daemon to report the loopback up"
  else
    fail "the overlay did not wait for the loopback: $(cat "$daemon_calls") calls"
  fi
else
  fail "the overlay exited non-zero while the loopback came up"
fi

# A loopback that never comes up names the daemon and starts no player.
reset_host
if PATH="$bin:$PATH" CODEROS_CAMERA_LOOPBACK=/dev/video10 TEST_UP_AFTER=999 CODEROS_CAMERA_LOOPBACK_WAIT=3 \
  "$bin/camera-overlay" start > "$scratch/out" 2>&1; then
  fail "the overlay exited zero with no loopback"
else
  if grep -q 'coderos-camera serve' "$scratch/out" && [ ! -e "$mpv_args" ]; then
    pass "a loopback that never comes up names the daemon and opens no player"
  else
    fail "the overlay's failure said something else: $(cat "$scratch/out")"
  fi
fi

# `status` names the node the circle reads.
reset_host
cat > "$bin/pgrep" << 'STUB'
#!/usr/bin/env bash
exit 0
STUB
chmod +x "$bin/pgrep"
out=$(PATH="$bin:$PATH" CODEROS_CAMERA_LOOPBACK=/dev/video10 "$bin/camera-overlay" status)
if [ "$out" = "The camera view is up on /dev/video10." ]; then
  pass "status names the loopback node"
else
  fail "status said: $out"
fi

# A player that stays up and reaps nothing, which is what mpv is: it never
# waits for a process it inherits, so a child left running across the `exec`
# stays in the table as a zombie under it. The stub records its own pid and
# becomes `sleep`, so the process it leaves behind is one that waits for
# nothing at all.
stub_player() {
  cat > "$bin/mpv" << STUB
#!/usr/bin/env bash
printf '%s\n' "\$@" > "$mpv_args"
echo \$\$ > "$player_pid"
exec sleep "\${TEST_PLAYER_SECONDS:-3}"
STUB
  chmod +x "$bin/mpv"
}

# The overlay leaves no defunct shaper under the player. It used to: the
# shaper was backgrounded and the shell then became mpv, which never waited
# for it, so `ps` carried one `[camera-overlay] <defunct>` for the life of
# the session.
reset_host
stub_player
printf '[{"title":"selfie"}]\n' > "$clients"
PATH="$bin:$PATH" CODEROS_CAMERA_DEVICE=/dev/null "$bin/camera-overlay" start > /dev/null 2>&1 &
overlay=$!
tries=0
while [ ! -s "$player_pid" ] && [ "$tries" -lt 50 ]; do
  sleep 0.1
  tries=$((tries + 1))
done
if [ -s "$player_pid" ]; then
  # Long enough for the shaper to finish its work and exit: it finds the
  # window at once and sleeps 0.3 between two of its calls.
  sleep 1.5
  player=$(cat "$player_pid")
  defunct=$(ps -Ao ppid=,stat= | awk -v p="$player" '$1 == p && $2 ~ /Z/' | wc -l | tr -d ' ')
  if [ "$defunct" -eq 0 ]; then
    pass "the overlay leaves no defunct shaper under the player"
  else
    fail "the overlay left $defunct defunct process(es) under the player"
  fi
else
  fail "the player stub never started"
fi
wait "$overlay" 2>/dev/null || true

# A shaper the session refuses says which step failed and what the session
# answered. Nothing reads the shaper's exit status, so a circle that never
# floats has only stderr to say so on.
reset_host
stub_player
printf '[{"title":"selfie"}]\n' > "$clients"
cat > "$bin/coder-desk" << STUB
#!/usr/bin/env bash
case "\${1:-}" in
  list) cat "$clients" ;;
  screens) printf '[{"size":{"width":1920,"height":1080}}]\n' ;;
  shape) echo "no window matches title:selfie" >&2; exit 1 ;;
  *) exit 0 ;;
esac
STUB
chmod +x "$bin/coder-desk"
PATH="$bin:$PATH" CODEROS_CAMERA_DEVICE=/dev/null TEST_PLAYER_SECONDS=2 \
  "$bin/camera-overlay" start > "$scratch/out" 2>&1
if grep -q 'did not float the camera window: no window matches title:selfie' "$scratch/out"; then
  pass "a shaper the session refuses names the step and the reason"
else
  fail "the refused shaper said: $(cat "$scratch/out")"
fi

# A window that never appears is reported too, rather than leaving a circle
# nobody can explain.
reset_host
stub_player
PATH="$bin:$PATH" CODEROS_CAMERA_DEVICE=/dev/null CODEROS_CAMERA_SHAPE_WAIT=3 TEST_PLAYER_SECONDS=2 \
  "$bin/camera-overlay" start > "$scratch/out" 2>&1
if grep -q 'The camera window never appeared' "$scratch/out"; then
  pass "a window that never appears is reported on stderr"
else
  fail "the waiting shaper said: $(cat "$scratch/out")"
fi

# The toggle, with no circle up, checks the daemon and starts the circle;
# with the daemon answering, it starts no second daemon.
stub_overlay() {
  cat > "$bin/camera-overlay" << STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >> "$overlay_calls"
exit 0
STUB
  chmod +x "$bin/camera-overlay"
}
reset_host
stub_overlay
PATH="$bin:$PATH" CODEROS_CAMERA_LOOPBACK=/dev/video10 "$bin/camera-toggle" > /dev/null 2>&1
sleep 0.3
if grep -qx 'status' "$daemon_log" && ! grep -qx 'serve' "$daemon_log" && grep -qx 'start' "$overlay_calls"; then
  pass "the toggle asks the daemon for status and starts the circle"
else
  fail "the toggle did something else: daemon=$(tr '\n' ' ' < "$daemon_log" 2>/dev/null) overlay=$(tr '\n' ' ' < "$overlay_calls" 2>/dev/null)"
fi

# With no daemon answering, the toggle starts one before the circle.
reset_host
stub_overlay
PATH="$bin:$PATH" CODEROS_CAMERA_LOOPBACK=/dev/video10 TEST_STATUS_EXIT=1 "$bin/camera-toggle" > /dev/null 2>&1
sleep 0.5
if grep -qx 'serve' "$daemon_log" && grep -qx 'start' "$overlay_calls"; then
  pass "the toggle starts the daemon when none answers"
else
  fail "the toggle started no daemon: daemon=$(tr '\n' ' ' < "$daemon_log" 2>/dev/null)"
fi

# With the circle up, the toggle stops it and asks the daemon nothing.
reset_host
stub_overlay
printf '[{"title":"selfie"}]\n' > "$clients"
PATH="$bin:$PATH" CODEROS_CAMERA_LOOPBACK=/dev/video10 "$bin/camera-toggle" > /dev/null 2>&1
if grep -qx 'stop' "$overlay_calls" && [ ! -e "$daemon_log" ]; then
  pass "the toggle stops a circle that is up and leaves the daemon alone"
else
  fail "the toggle did something else with the circle up: overlay=$(tr '\n' ' ' < "$overlay_calls" 2>/dev/null) daemon=$(cat "$daemon_log" 2>/dev/null)"
fi

# The option rendering: the grant, the session's variables, and the modprobe
# line, from the option values alone.
if command -v nix-instantiate > /dev/null 2>&1; then
  render() {
    nix-instantiate --eval --strict --json --expr \
      "(import $grant_src { camera = $1; })" 2>&1
  }
  on='{ device = "/dev/video0"; width = 1280; height = 720; framerate = 30; loopback = { enable = true; number = 10; label = "CoderOS camera"; }; }'
  off='{ device = "/dev/video2"; width = 640; height = 480; framerate = 15; loopback = { enable = false; number = 10; label = "CoderOS camera"; }; }'
  if rendered=$(render "$on"); then
    if [ "$(printf '%s' "$rendered" | jq -c '.grant')" = '{"device":"/dev/video0","framerate":30,"height":720,"loopback":"/dev/video10","width":1280}' ] \
      && [ "$(printf '%s' "$rendered" | jq -r '.sessionVariables.CODEROS_CAMERA_LOOPBACK')" = "/dev/video10" ] \
      && [ "$(printf '%s' "$rendered" | jq -r '.sessionVariables.CODEROS_CAMERA_CAPTURE')" = "1280x720" ] \
      && [ "$(printf '%s' "$rendered" | jq -r '.modprobe')" = 'options v4l2loopback exclusive_caps=1 video_nr=10 card_label="CoderOS camera"' ]; then
      pass "the option renders the grant, the loopback variable, and the modprobe line"
    else
      fail "the option rendered something else: $rendered"
    fi
  else
    fail "the rendering did not evaluate: $rendered"
  fi
  if rendered=$(render "$off"); then
    if [ "$(printf '%s' "$rendered" | jq -r '.grant.loopback')" = "null" ] \
      && [ "$(printf '%s' "$rendered" | jq -r '.sessionVariables.CODEROS_CAMERA_LOOPBACK // "unset"')" = "unset" ] \
      && [ "$(printf '%s' "$rendered" | jq -r '.modprobe')" = "" ]; then
      pass "with the loopback off the grant names none and the session sees none"
    else
      fail "with the loopback off the option rendered something else: $rendered"
    fi
  else
    fail "the rendering with the loopback off did not evaluate: $rendered"
  fi
else
  printf 'skip the option rendering: nix-instantiate is not on this host\n'
fi

if [ "$failures" -gt 0 ]; then
  echo "$failures failure(s)" >&2
  exit 1
fi
echo "all passed"
