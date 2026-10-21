#!/bin/sh
# Start one DeepSeek-V4.1 stage on its cloud machine for the group head
# (#12402). The head runs this as the stage's managed worker:
#   v41-stage-ssh.sh MACHINE ZONE PROJECT REMOTE_PORT -- STAGE ARGS...
# The head appends --stage, --generation, --front, and --token. The stage's
# control connection to the front goes through an ssh reverse tunnel, so the
# front stays on loopback. Killing this process group ends the session and
# the remote stage; a lost machine ends the session within 15 seconds.
set -eu
[ "$#" -ge 5 ] && [ "$5" = -- ] || {
  echo "usage: $0 MACHINE ZONE PROJECT REMOTE_PORT -- STAGE ARGS..." >&2
  exit 2
}
machine=$1 zone=$2 project=$3 port=$4
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
exec "${OA_GCLOUD:-gcloud}" compute ssh "$machine" --zone "$zone" --project "$project" --quiet -- \
  -tt -o ExitOnForwardFailure=yes -o ServerAliveInterval=5 -o ServerAliveCountMax=3 \
  -R "127.0.0.1:$port:127.0.0.1:$front" "$remote"
