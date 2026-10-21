#!/bin/sh
# Start one DeepSeek-V4.1 stage on its cloud machine for the group head
# (#12402, #12399). The head runs this as the stage's managed worker:
#   v41-stage-ssh.sh MACHINE ZONE PROJECT REMOTE_PORT -- STAGE ARGS...
#   v41-stage-ssh.sh --ssh KEY USER@HOST REMOTE_PORT -- STAGE ARGS...
# The first form runs `gcloud compute ssh` from the budget host. The second
# runs plain ssh with the group's own key from a head inside the zone (the
# stage-0 machine or a small head machine), to the stage machine's internal
# name. The head appends --stage, --generation, --front, and --token. The
# stage's control connection to the front goes through an ssh reverse
# tunnel, so the front stays on loopback. Killing this process group ends the
# session and the remote stage; a lost machine ends the session within 15
# seconds.
set -eu
usage() {
  echo "usage: $0 MACHINE ZONE PROJECT REMOTE_PORT -- STAGE ARGS..." >&2
  echo "       $0 --ssh KEY USER@HOST REMOTE_PORT -- STAGE ARGS..." >&2
  exit 2
}
[ "$#" -ge 5 ] && [ "$5" = -- ] || usage
if [ "$1" = --ssh ]; then
  key=$2 target=$3 port=$4 mode=ssh
else
  machine=$1 zone=$2 project=$3 port=$4 mode=gcloud
fi
shift 5
quote() { printf "'%s'" "$(printf %s "$1" | sed "s/'/'\\\\''/g")"; }
remote=/opt/openagents/stage-run front= prev=
for arg; do
  if [ "$prev" = --front ]; then
    front=${arg##*:}
    arg=127.0.0.1:$port
  fi
  remote="$remote $(quote "$arg")"
  prev=$arg
done
[ -n "$front" ] || { echo "no --front address" >&2; exit 2; }
keepalive="-o ExitOnForwardFailure=yes -o ServerAliveInterval=5 -o ServerAliveCountMax=3"
if [ "$mode" = ssh ]; then
  # shellcheck disable=SC2086
  exec "${OA_SSH:-ssh}" -i "$key" -tt -o BatchMode=yes \
    -o StrictHostKeyChecking=accept-new -o "UserKnownHostsFile=$key.known_hosts" \
    $keepalive -R "127.0.0.1:$port:127.0.0.1:$front" "$target" "$remote"
fi
# shellcheck disable=SC2086
exec "${OA_GCLOUD:-gcloud}" compute ssh "$machine" --zone "$zone" --project "$project" --quiet -- \
  -tt $keepalive -R "127.0.0.1:$port:127.0.0.1:$front" "$remote"
