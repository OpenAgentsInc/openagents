#!/usr/bin/env bash
# New constructor gate; cached references only, no SDK or episode execution.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?usage: panda_owned_sample_check.sh NEW_OUTPUT_DIRECTORY}"
[[ ! -e "$output" ]] || { echo "choose a new output directory" >&2;exit 1; }
# Logical types are erased at the C ABI; refuse the three Coax misuses before
# native execution. Existing evidence records these exact compiler cases.
mkdir "$output.units"
for kind in angle rate slide_rate;do
  diagnostic="$output.units/$kind.stderr"
  if "$coax" c "psionic/tests/robotics/panda_owned_sample_bad_$kind.coax" --library > "$output.units/$kind.c" 2> "$diagnostic";then
    echo "owned sample accepted a wrong $kind unit" >&2;exit 1
  fi
  rg -q 'error\[' "$diagnostic" && rg -q 'Quantity' "$diagnostic"
done
python3 psionic/tests/robotics/panda_owned_sample_reference.py --compiler "$coax" --output "$output"
