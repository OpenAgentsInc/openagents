#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_mass_solve_check.sh NEW_OUTPUT_DIRECTORY}"
python3 psionic/tests/robotics/panda_mass_solve_reference.py --compiler "$coax" --output "$output"
