#!/bin/sh
# Offline scalar-F64 isolated-rotor closed-form qualification.
set -eu
[ "$#" -le 1 ] || { echo "usage: $0 [NEW_OUTPUT_DIRECTORY]" >&2; exit 2; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
coax_rotor_compiler=${COAX:-$root/coax/bin/coax}
if [ "$#" -eq 1 ]; then
    coax_rotor_output=$1
else
    coax_rotor_parent=$(mktemp -d "${TMPDIR:-/tmp}/rotor-pd.XXXXXX")
    coax_rotor_output=$coax_rotor_parent/native
fi
python3 "$root/psionic/tests/robotics/rotor_pd_native_reference.py" \
    --compiler "$coax_rotor_compiler" --output "$coax_rotor_output" \
    --oracle "$root/psionic/fixtures/robotics/rotor-pd"
