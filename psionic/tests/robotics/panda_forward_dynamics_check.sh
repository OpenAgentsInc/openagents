#!/usr/bin/env bash
# New unconstrained query corpus only. No SDK, simulator or earlier gate runs.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_forward_dynamics_check.sh NEW_OUTPUT_DIRECTORY}"
python3 psionic/tests/robotics/panda_forward_dynamics_reference.py --compiler "$coax" --output "$output"
