#!/usr/bin/env bash
# NEW offline smooth composition only; never acquire SDK facts or run episodes.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_smooth_check.sh NEW_OUTPUT_DIRECTORY}"
python3 psionic/tests/robotics/panda_smooth_reference.py --compiler "$coax" --output "$output"
