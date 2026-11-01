#!/usr/bin/env bash
# Offline own/recorded-command-shadow replay; never acquires another SDK prefix.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_joint_pd_prefix_check.sh NEW_OUTPUT_DIRECTORY}"
shift
python3 psionic/tests/robotics/panda_joint_pd_prefix_reference.py --coax "$coax" --output "$output" "$@"
