#!/bin/sh
# Offline local posture projection versus cached Gram/Gaussian targets.
set -eu
[ "$#" -le 1 ] || { echo "usage: $0 [NEW_OUTPUT_DIRECTORY]" >&2; exit 2; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
coax_nullspace_compiler=${COAX:-$root/coax/bin/coax}
if [ "$#" -eq 1 ]; then
    coax_nullspace_output=$1
else
    coax_nullspace_parent=$(mktemp -d "${TMPDIR:-/tmp}/panda-nullspace.XXXXXX")
    coax_nullspace_output=$coax_nullspace_parent/native
fi
python3 "$root/psionic/tests/robotics/panda_nullspace_native_reference.py" \
    --compiler "$coax_nullspace_compiler" --output "$coax_nullspace_output" \
    --oracle "$root/psionic/fixtures/robotics/panda-nullspace"
