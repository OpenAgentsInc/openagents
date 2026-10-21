#!/bin/sh
# Start a DeepSeek-V4.1 group head on a machine in the group's zone (#12399):
#   v41-head-ssh.sh MACHINE ZONE PROJECT
# `pylon cloud up --head stage0|vm` runs this on the budget host. The
# session's standard input carries the launch (one JSON line: the head's
# settings and the group's ssh key), then `down`; its standard output
# carries the head's status lines. The budget host keeps the spend guard and
# talks only to the head. When the session ends, the head drains and stops.
#
# A machine created moments ago refuses ssh until it boots, and its startup
# script then copies the head's own stages' weights, so this waits up to
# OA_HEAD_SSH_WAIT seconds (default 3600; 0 skips the wait) for head-run and
# the machine's ready mark before it starts the session, which must not
# lose the launch on its input. The stages it starts then only load.
#
# Ready means the machine's startup script finished in the current boot: its
# ready mark's first line names the boot ID (v41-stage-startup.sh, #12407).
# A mark left by an earlier boot, or by a startup that is still installing,
# never passes. OA_STAGE_ROOT, OA_STAGE_OPT, and OA_BOOT_ID replace the
# machine's paths, for tests.
set -eu
[ "$#" -eq 3 ] || { echo "usage: $0 MACHINE ZONE PROJECT" >&2; exit 2; }
gcloud=${OA_GCLOUD:-gcloud}
wait=${OA_HEAD_SSH_WAIT:-3600}
root=${OA_STAGE_ROOT:-/mnt/stage}
opt=${OA_STAGE_OPT:-/opt/openagents}
boot_id=${OA_BOOT_ID:-/proc/sys/kernel/random/boot_id}
probe="test -x $opt/head-run && test \"\$(head -n 1 $root/ready 2>/dev/null)\" = \"boot \$(cat $boot_id)\""
start=$(date +%s)
while [ "$wait" -gt 0 ]; do
  if "$gcloud" compute ssh "$1" --zone "$2" --project "$3" --quiet -- \
    -o ConnectTimeout=10 "$probe" </dev/null >/dev/null 2>&1; then
    break
  fi
  if [ $(($(date +%s) - start)) -ge "$wait" ]; then
    echo "$1 did not start its head within $wait s" >&2
    exit 1
  fi
  sleep 5
done
exec "$gcloud" compute ssh "$1" --zone "$2" --project "$3" --quiet -- \
  -T -o ServerAliveInterval=5 -o ServerAliveCountMax=3 "$opt/head-run"
