#!/usr/bin/env bash
# New constructor gate; cached references only, no SDK or episode execution.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_owned_sample_check.sh NEW_OUTPUT_DIRECTORY}"
python3 psionic/tests/robotics/panda_owned_sample_reference.py --compiler "$coax" --output "$output"
