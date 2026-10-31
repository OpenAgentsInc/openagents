#!/usr/bin/env bash
# NEW inverse-force gate only; ordinary references invoke no SDK or old gates.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_inverse_dynamics_check.sh NEW_OUTPUT_DIRECTORY}"
python3 psionic/tests/robotics/panda_inverse_dynamics_reference.py --compiler "$coax" --output "$output"
