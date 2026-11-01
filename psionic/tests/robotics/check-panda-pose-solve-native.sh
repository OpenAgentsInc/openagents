#!/bin/sh
# New offline whole-pose endpoint/external-replay scalar-F64 profile.
set -eu
[ "$#" -le 1 ] || { echo "usage: $0 [NEW_OUTPUT_DIRECTORY]" >&2; exit 2; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
coax_pose_solver=${COAX:-$root/coax/bin/coax}
if [ "$#" -eq 1 ]; then
    coax_pose_solve_output=$1
else
    coax_pose_solve_parent=$(mktemp -d "${TMPDIR:-/tmp}/panda-pose-solve.XXXXXX")
    coax_pose_solve_output=$coax_pose_solve_parent/native
fi
python3 "$root/psionic/tests/robotics/panda_pose_solve_native_reference.py" \
    --compiler "$coax_pose_solver" --output "$coax_pose_solve_output" \
    --oracle "$root/psionic/fixtures/robotics/panda-pose-solve"
