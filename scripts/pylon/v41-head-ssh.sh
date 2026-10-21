#!/bin/sh
# Start a DeepSeek-V4.1 group head on a machine in the group's zone (#12399):
#   v41-head-ssh.sh MACHINE ZONE PROJECT
# `pylon cloud up --head stage0|vm` runs this on the budget host. The
# session's standard input carries the launch (one JSON line: the head's
# settings and the group's ssh key), then `down`; its standard output
# carries the head's status lines. The budget host keeps the spend guard and
# talks only to the head. When the session ends, the head drains and stops.
set -eu
[ "$#" -eq 3 ] || { echo "usage: $0 MACHINE ZONE PROJECT" >&2; exit 2; }
exec "${OA_GCLOUD:-gcloud}" compute ssh "$1" --zone "$2" --project "$3" --quiet -- \
  -T -o ServerAliveInterval=5 -o ServerAliveCountMax=3 /opt/openagents/head-run
