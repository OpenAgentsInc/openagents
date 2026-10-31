#!/usr/bin/env bash
# Offline fixed-source position IK only; never run SDK, NN, rendering or steps.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_ik_check.sh NEW_OUTPUT_DIRECTORY}"
python3 psionic/tests/robotics/panda_ik_reference.py --compiler "$coax" --out "$output"
