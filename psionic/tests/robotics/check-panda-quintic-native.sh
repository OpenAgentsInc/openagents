#!/bin/sh
# Offline rest-to-rest timing/sample qualification; no motion execution.
set -eu
[ "$#" -le 1 ] || { echo "usage: $0 [NEW_OUTPUT_DIRECTORY]" >&2; exit 2; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
coax_quintic_compiler=${COAX:-$root/coax/bin/coax}
if [ "$#" -eq 1 ]; then
    coax_quintic_output=$1
else
    coax_quintic_parent=$(mktemp -d "${TMPDIR:-/tmp}/panda-quintic.XXXXXX")
    coax_quintic_output=$coax_quintic_parent/native
fi
python3 "$root/psionic/tests/robotics/panda_quintic_native_reference.py" \
    --compiler "$coax_quintic_compiler" --output "$coax_quintic_output" \
    --oracle "$root/psionic/fixtures/robotics/panda-quintic"
