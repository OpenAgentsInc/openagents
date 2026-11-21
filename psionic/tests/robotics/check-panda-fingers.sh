#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper; holds no logic)
set -euo pipefail
cd "$(dirname "$0")/../../.."
work="${1:?usage: check-panda-fingers.sh NEW_OUTPUT_DIRECTORY}"
compiler="${COAX:-$PWD/coax/bin/coax}"
"${PYTHON:-python3}" psionic/tests/robotics/panda_fingers_reference.py --compiler "$compiler" --output "$work"
