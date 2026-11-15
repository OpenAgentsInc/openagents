#!/usr/bin/env bash
# Firm reason: 5. The nightly mutation sweep's entry (#13583, #13586): git, coax/build.sh,
# coax/web/tool.sh mutation-sweep, which holds every rule (coax/web/mutation-sweep.coax), and
# systemctl for its timer. It keeps its own checkout of origin/main (OA_SWEEP_DIR, default
# ~/.openagents/mutation-sweep/checkout), moves it to origin/main's tip, builds the compiler
# there and runs the sweep from it. The sweep's state (each part's killers file between
# nights, the report, the tracking issue's number) is that checkout's untracked
# mutation-sweep/ folder, which the move keeps. The parts run on the build pool through
# coax/remote/remote.sh, inside its spend cap (OA_REMOTE_DAILY_USD) and host count;
# OA_REMOTE_HOST keeps them on one host. One sweep runs at a time: a second one stops.
#   coax/remote/mutation-sweep.sh [ARGS...]   ARGS pass to the sweep (--dry-run, --only, --parts, --jobs, --part-minutes, --hours)
#   coax/remote/mutation-sweep.sh install     run it each night at 02:00 Chicago time: on Linux the systemd user
#                                             timer com.openagents.mutation-sweep (OA_SWEEP_ARGS are its ARGS;
#                                             OA_REMOTE_STATE, OA_REMOTE_HOST and CLOUDSDK_CONFIG are kept when set);
#                                             on macOS it prints the crontab line
#   coax/remote/mutation-sweep.sh uninstall   stop the timer
#   coax/remote/mutation-sweep.sh timer [ARGS...]  the timer's start: when no sweep is running, move the
#                                             checkout to origin/main and run that commit's script with ARGS
# docs/metaverse/remote-builds.md, "The nightly mutation sweep".
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd -P)"
dir="${OA_SWEEP_DIR:-$HOME/.openagents/mutation-sweep/checkout}"
log="${dir%/*}/sweep.log"
mkdir -p "${dir%/*}"
make_checkout() {
  if [[ ! -d "$dir/.git" && ! -f "$dir/.git" ]]; then
    git -C "$here" fetch -q origin main
    git -C "$here" worktree add -q --detach "$dir" origin/main
  fi
}
unit=com.openagents.mutation-sweep
case "${1:-}" in
  install)
    make_checkout
    if [[ "$(uname -s)" == Darwin ]]; then
      echo "( crontab -l 2> /dev/null; echo '0 2 * * * cd $dir && coax/remote/mutation-sweep.sh > /dev/null 2>&1' ) | crontab -"
      exit 0
    fi
    units="$HOME/.config/systemd/user"; mkdir -p "$units"
    env_line="\"PATH=$PATH\" \"HOME=$HOME\""
    for v in OA_REMOTE_STATE OA_REMOTE_HOST OA_REMOTE_DAILY_USD CLOUDSDK_CONFIG; do
      [[ -z "${!v:-}" ]] || env_line+=" \"$v=${!v}\""
    done
    cat > "$units/$unit.service" <<EOF
[Unit]
Description=Nightly mutation sweep of every #authority file (coax/remote/mutation-sweep.sh, docs/metaverse/remote-builds.md)
After=network-online.target

[Service]
Type=oneshot
TimeoutStartSec=infinity
Environment=$env_line
ExecStart=$(command -v bash) -c "cd '$dir' && exec coax/remote/mutation-sweep.sh timer ${OA_SWEEP_ARGS:-}"
StandardOutput=append:${dir%/*}/timer.out
StandardError=append:${dir%/*}/timer.out
EOF
    cat > "$units/$unit.timer" <<EOF
[Unit]
Description=Start the nightly mutation sweep at 02:00 Chicago time

[Timer]
OnCalendar=*-*-* 02:00:00 America/Chicago
Persistent=true

[Install]
WantedBy=timers.target
EOF
    systemctl --user daemon-reload
    systemctl --user enable --now "$unit.timer"
    systemctl --user list-timers "$unit.timer" --no-pager
    exit 0 ;;
  uninstall)
    if [[ "$(uname -s)" == Darwin ]]; then echo "crontab -e, and delete the mutation-sweep.sh line"; exit 0; fi
    systemctl --user disable --now "$unit.timer" 2> /dev/null || true
    rm -f "$HOME/.config/systemd/user/$unit.timer" "$HOME/.config/systemd/user/$unit.service"
    systemctl --user daemon-reload
    exit 0 ;;
esac
# The sweep yields to the pipeline: its jobs take only slots 4 to 6 of a host, never the
# after-landing jobs' slots 1 to 3, so an after-landing job never waits for a slot the sweep holds.
export OA_REMOTE_SLOT_FIRST="${OA_REMOTE_SLOT_FIRST:-4}" OA_REMOTE_SLOTS="${OA_REMOTE_SLOTS:-6}"
lock="${dir%/*}/sweep.lock"
running() { local pid; pid="$(cat "$lock/pid" 2> /dev/null || true)"; [[ -n "$pid" ]] && kill -0 "$pid" 2> /dev/null; }
if [[ "${1:-}" == timer ]]; then
  shift
  # The checkout moves only between sweeps: a running one reads it.
  if running; then echo "mutation-sweep.sh: a sweep is running (pid $(cat "$lock/pid")); this night's start does nothing" >&2; exit 0; fi
  make_checkout
  git -C "$dir" fetch -q origin main && git -C "$dir" checkout -q --detach origin/main
  exec "$dir/coax/remote/mutation-sweep.sh" "$@"
fi
make_checkout
# One sweep at a time: the lock is a folder holding the running sweep's pid.
if ! mkdir "$lock" 2> /dev/null; then
  if running; then echo "mutation-sweep.sh: a sweep is running (pid $(cat "$lock/pid")); not starting another" >&2; exit 1; fi
  rm -f "$lock/pid"; rmdir "$lock" 2> /dev/null || true
  mkdir "$lock"
fi
echo $$ > "$lock/pid"
trap 'rm -f "$lock/pid"; rmdir "$lock" 2> /dev/null || true' EXIT
cd "$dir"
git fetch -q origin main
git checkout -q --detach origin/main
{ echo "== $(date '+%Y-%m-%d %H:%M %Z') sweep at $(git rev-parse --short HEAD)"; } >> "$log"
coax/build.sh >> "$log" 2>&1
coax/web/tool.sh mutation-sweep "$@" 2>&1 | tee -a "$log"
