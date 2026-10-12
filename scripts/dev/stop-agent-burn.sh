#!/usr/bin/env bash
# Stop everything on this machine that can burn a provider subscription,
# after the 2026-10-12 incident where an unattended `claude bg` workflow
# drained the owner's Claude login (docs/audits/2026-10-12-claude-usage-burn).
#
#   scripts/dev/stop-agent-burn.sh            report what would be stopped
#   scripts/dev/stop-agent-burn.sh --kill     stop it (TERM, then KILL)
#   scripts/dev/stop-agent-burn.sh --gcp      also list matching GCP instances
#   scripts/dev/stop-agent-burn.sh --kill --gcp   and stop those too
#
# Local consumers matched: detached `claude` daemons, pty/spare/headless
# workers and auth stubs; `coder-new` processes orphaned to launchd;
# `openagents mac serve`; and bench batch loops under a scripts/bench path.
# Interactive claude/coder-new sessions on a real tty are never touched.
#
# GCP consumers matched: RUNNING instances named oa-dev-env-*, clef-*-h100,
# or oa-att-* (bench/run shapes agents create).
set -uo pipefail

kill_mode=0
gcp_mode=0
for arg in "$@"; do
  case "$arg" in
    --kill) kill_mode=1 ;;
    --gcp) gcp_mode=1 ;;
    *) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
  esac
done

found=0

report() {
  found=1
  printf '%8s  %6s%%  %-12s  %s\n' "$1" "$2" "$3" "${4:0:110}"
}

stop_pid() {
  kill -TERM "$1" 2>/dev/null
  (sleep 3; kill -KILL "$1" 2>/dev/null) &
}

# Detached claude consumers: daemon supervisors, pty hosts, spares, headless
# runs, and dangling auth stubs. TTY field `??` means no controlling terminal.
while read -r pid tty etime pcpu cmd; do
  case "$cmd" in
    *claude*daemon*run*|*bg-pty-host*|*bg-spare*|*claude\ -p*|*claude\ auth\ login*)
      report "$pid" "$pcpu" "$etime" "$cmd"
      [[ $kill_mode -eq 1 ]] && stop_pid "$pid"
      ;;
  esac
done < <(ps -eo pid=,tty=,etime=,pcpu=,command= | awk '$2=="??"' | grep -E 'claude' | grep -v grep)

# coder-new (and coder_new test binaries) orphaned to launchd.
while read -r pid ppid etime pcpu cmd; do
  [[ $ppid -eq 1 ]] || continue
  case "$cmd" in
    *coder-new*|*coder_new-*)
      report "$pid" "$pcpu" "$etime" "$cmd"
      [[ $kill_mode -eq 1 ]] && stop_pid "$pid"
      ;;
  esac
done < <(ps -eo pid=,ppid=,etime=,pcpu=,command= | grep -E 'coder.new|coder_new' | grep -v grep)

# The cloud-pool door an unattended run must not hold open.
while read -r pid etime pcpu cmd; do
  report "$pid" "$pcpu" "$etime" "$cmd"
  [[ $kill_mode -eq 1 ]] && stop_pid "$pid"
done < <(ps -eo pid=,etime=,pcpu=,command= | grep -E 'openagents mac serve' | grep -v grep)

# Bench batch loops (ab.py and friends) under a bench directory.
while read -r pid etime pcpu cmd; do
  report "$pid" "$pcpu" "$etime" "$cmd"
  [[ $kill_mode -eq 1 ]] && stop_pid "$pid"
done < <(ps -eo pid=,etime=,pcpu=,command= | grep -E 'bench.*\.py (batch|run)|ab\.py' | grep -v grep)

if [[ $found -eq 0 ]]; then
  echo "No local consumers found."
elif [[ $kill_mode -eq 0 ]]; then
  echo
  echo "Report only. Re-run with --kill to stop these processes."
else
  wait
  echo "Stopped."
fi

if [[ $gcp_mode -eq 1 ]]; then
  echo
  echo "GCP instances matching bench/run shapes:"
  gcloud compute instances list \
    --filter='status=RUNNING AND name~^(oa-dev-env-|clef-.*-h100|oa-att-)' \
    --format='value(name,zone)' | while read -r name zone; do
    echo "  $name  ($zone)"
    if [[ $kill_mode -eq 1 ]]; then
      gcloud compute instances stop "$name" --zone="$zone" \
        --discard-local-ssd=false -q >/dev/null 2>&1 &
    fi
  done
  [[ $kill_mode -eq 1 ]] && { wait; echo "GCP stop issued."; }
fi
