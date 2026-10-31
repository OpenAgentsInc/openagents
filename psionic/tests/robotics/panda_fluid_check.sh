#!/usr/bin/env bash
# Offline fixed-model component proof. Never acquires SDK data or simulates.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_fluid_check.sh NEW_OUTPUT_DIRECTORY}"
python3 psionic/tests/robotics/panda_fluid_reference.py --compiler "$coax" --out "$output"
