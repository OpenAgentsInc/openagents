#!/bin/sh
# Offline, opt-in Darwin-arm64 scalar-F64 component proof.
set -eu
[ "$#" -le 1 ] || { echo "usage: $0 [NEW_OUTPUT_DIRECTORY]" >&2; exit 2; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
coax_pose_compiler=${COAX:-$root/coax/bin/coax}
if [ "$#" -eq 1 ]; then
    coax_pose_output=$1
else
    coax_pose_parent=$(mktemp -d "${TMPDIR:-/tmp}/panda-pose-ik.XXXXXX")
    coax_pose_output=$coax_pose_parent/native
fi
python3 "$root/psionic/tests/robotics/panda_pose_ik_native_reference.py" \
    --compiler "$coax_pose_compiler" --output "$coax_pose_output" \
    --oracle "$root/psionic/fixtures/robotics/panda-pose-ik"
