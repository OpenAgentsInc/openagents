#!/usr/bin/env bash
# Only the NEW fixed Panda Jacobian native/independent-reference corpus.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
retained="${PANDA_RETAINED_CONTROLLER:-$PWD/psionic/fixtures/robotics/panda-fk/retained-controller}"
output="${1:?usage: panda_jacobian_check.sh NEW_OUTPUT_DIRECTORY}"
python3 -I psionic/tests/robotics/panda_jacobian_reference.py --compiler "$coax" --retained "$retained" --output "$output"
