#!/bin/sh
# OFFLINE committed evidence only. Actual execution is a separate one-shot CLI.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ "$#" -gt 0 ] && [ "${1#-}" = "$1" ]; then
  out=$1
  shift
  set -- --output "$out" "$@"
fi
exec "${PYTHON:-python3}" "$here/panda_owned_query_neural_cached.py" "$@"
