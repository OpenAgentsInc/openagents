#!/usr/bin/env bash
# NEW offline Euler evidence only; never acquire SDK observations or run NN.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_euler_check.sh NEW_OUTPUT_DIRECTORY}"
shift
python3 psionic/tests/robotics/panda_euler_reference.py --compiler "$coax" --output "$output" "$@"
