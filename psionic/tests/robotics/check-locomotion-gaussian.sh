#!/bin/sh
# Scalar CPU/stdlib context only; no model, SDK, environment or private data.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/../../.." && pwd)
out=${1:?usage: check-locomotion-gaussian.sh NEW_OUTPUT_DIRECTORY [driver options]}
shift
exec "${PYTHON:-python3}" "$here/locomotion_gaussian_native_reference.py" \
  --work-dir "$out" --coax "${COAX:-$root/coax/bin/coax}" "$@"
