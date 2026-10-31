#!/usr/bin/env bash
# Offline component gate. SDK acquisition is separate and is never invoked here.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_actuation_check.sh NEW_OUTPUT_DIRECTORY}"
python3 psionic/tests/robotics/panda_actuation_reference.py --compiler "$coax" --output "$output"
