#!/bin/sh
# OFFLINE newtracking gate. Explicit SDK acquisition uses a different driver.
set -eu
[ "$#" -le 1 ] || { echo "usage: $0 [NEW_OUTPUT_DIRECTORY]" >&2; exit 2; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
coax_tracking_compiler=${COAX:-$root/coax/bin/coax}
if [ "$#" -eq 1 ]; then coax_tracking_output=$1
else
    coax_tracking_parent=$(mktemp -d "${TMPDIR:-/tmp}/panda-tracking.XXXXXX")
    coax_tracking_output=$coax_tracking_parent/native
fi
python3 "$root/psionic/tests/robotics/panda_joint_pd_tracking_offline.py" \
    --compiler "$coax_tracking_compiler" --output "$coax_tracking_output" \
    --reference "$root/psionic/fixtures/robotics/panda-joint-pd-tracking"
