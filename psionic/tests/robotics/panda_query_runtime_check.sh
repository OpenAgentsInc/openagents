#!/usr/bin/env bash
# Explicit local fixed-model checks, never a neural or simulator execution.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}"
output="${1:?output directory}";mutate="${2:-0}";jobs="${3:-4}"
scene="${PSIONIC_PANDA_SCENE:?set PSIONIC_PANDA_SCENE to the exact qualified demo_000 scene.xml}"
[[ ! -e "$output" ]] || { echo 'preserve previous output' >&2;exit 2; }
mkdir -p "$output"
for root in panda-query-runtime panda-query-runtime-assets panda-query-runtime-hash;do
  "$coax" test "psionic/tests/robotics/$root.coax" --library
  "$coax" test "psionic/tests/robotics/$root.coax" --library --release
  if [[ $mutate == 1 ]];then "$coax" mutate "psionic/tests/robotics/$root.coax" --library --root-only --jobs "$jobs";fi
done
python3 psionic/tests/robotics/panda_query_runtime_reference.py --compiler "$coax" --scene "$scene" --output "$output/metadata"
python3 psionic/tests/robotics/panda_query_runtime_hash_reference.py --compiler "$coax" --output "$output/hash"
python3 psionic/tests/robotics/panda_query_runtime_mock.py --artifacts "$output/metadata" --output "$output/effects"
