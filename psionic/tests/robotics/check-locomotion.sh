#!/bin/sh
# New CPU/stdlib numerical context; no model, environment, or private cache.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/../../.." && pwd)
out=${1:?usage: check-locomotion.sh NEW_OUTPUT_DIRECTORY [driver options]}
shift
exec "${PYTHON:-python3}" "$here/locomotion_native_reference.py" \
  --work-dir "$out" --coax "${COAX:-$root/coax/bin/coax}" "$@"
