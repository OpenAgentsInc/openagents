#!/usr/bin/env bash
# From the laptop: push scripts and start a detached job on worker 0. usage: start.sh NAME ZONE "ENV=.. cmd"
NAME=$1 ZONE=$2; shift 2
HERE=$(cd "$(dirname "$0")" && pwd)
"$HERE/vm.sh" push "$NAME" "$ZONE" >/dev/null 2>&1
"$HERE/vm.sh" run "$NAME" "$ZONE" "nohup bash -c ': CLEFJOB; $*' > ~/job-\$(date +%s).log 2>&1 < /dev/null & echo started"
