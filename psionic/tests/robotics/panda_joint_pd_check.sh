#!/usr/bin/env bash
# New44 joint-PD queries only; no SDK/renderer/NN/step or old qualification.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_joint_pd_check.sh NEW_OUTPUT_DIRECTORY}"
shift
python3 psionic/tests/robotics/panda_joint_pd_reference.py --compiler "$coax" --output "$output" "$@"
