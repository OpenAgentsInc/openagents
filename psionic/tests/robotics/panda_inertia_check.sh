#!/usr/bin/env bash
# NEW fixed16-body M9/zero-rate gravity comparison; no simulator or old gates.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_inertia_check.sh NEW_OUTPUT_DIRECTORY}"
python3 psionic/tests/robotics/panda_inertia_reference.py --compiler "$coax" --output "$output"
