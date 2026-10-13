#!/usr/bin/env bash
# Kill any running bench on a VM and start a new detached job. usage: restart.sh NAME ZONE "cmd"
NAME=$1 ZONE=$2; shift 2; HERE=$(cd "$(dirname "$0")" && pwd)
"$HERE/vm.sh" run "$NAME" "$ZONE" "pkill -f 'CLEFJO[B]'; pkill -f '[p]ython -u'; pkill -f '[s]gl_jax'; sleep 3; true" >/dev/null 2>&1
"$HERE/start.sh" "$NAME" "$ZONE" "$@"
