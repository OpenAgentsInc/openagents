#!/usr/bin/env bash
# New fixed Panda FK only; all SDK inputs are retained reference data.
set -euo pipefail
cd "$(dirname "$0")/../../.."
retained="${1:?usage: check-panda-fk.sh RETAINED_PCSAMPLE_DIRECTORY NEW_OUTPUT_DIRECTORY}"
work="${2:?new output directory required}"
compiler="${COAX:-$PWD/coax/bin/coax}"
"${PYTHON:-python3}" psionic/tests/robotics/panda_fk_reference.py --compiler "$compiler" --retained "$retained" --output "$work"
