#!/usr/bin/env bash
# Firm reason: 5 (thin compiler wrapper). All admissions/numerics are Coax.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}";jobs=4;mutate=0;sim=0;evaluate=0;learn=0;singlearm=0;selected=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --jobs) [[ $# -ge 2 && $2 =~ ^[1-9][0-9]*$ ]] || exit 2;jobs=$2;shift 2 ;;
    --mutate) mutate=1;shift ;;
    *) unit=${1%.coax};unit=${unit#psionic/lib/};unit=${unit#robotics/};selected=1
       case "$unit" in robotics) sim=1;evaluate=1;learn=1;singlearm=1;;sim) sim=1;;evaluate) evaluate=1;;learn) learn=1;;singlearm) singlearm=1;;*) echo "unknown robotics unit: $1" >&2;exit 2;;esac;shift ;;
  esac
done
if [[ $selected == 0 ]]; then sim=1;evaluate=1;learn=1;singlearm=1;fi
work=$(mktemp -d "${TMPDIR:-/tmp}/psionic-robotics.XXXXXXXX");trap 'rm -rf "$work"' EXIT
for unit in sim evaluate learn singlearm; do
  enabled=0;case "$unit" in sim) enabled=$sim;;evaluate) enabled=$evaluate;;learn) enabled=$learn;;singlearm) enabled=$singlearm;;esac
  [[ $enabled == 1 ]] || continue
  "$coax" test "psionic/lib/robotics/$unit.coax"
  "$coax" test "psionic/lib/robotics/$unit.coax" --release
  if [[ $mutate == 1 ]]; then "$coax" mutate "psionic/lib/robotics/$unit.coax" --root-only --jobs "$jobs";fi
done
if [[ $sim == 1 || $evaluate == 1 || $learn == 1 ]]; then
  COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-native.sh "$work/core"
fi
if [[ $singlearm == 1 ]]; then
  COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-singlearm.sh "$work/singlearm"
fi
if [[ $learn == 1 ]]; then
  "$coax" run psionic/examples/imitation-before-robotics-scale.coax > "$work/example-debug.out"
  "$coax" run psionic/examples/imitation-before-robotics-scale.coax --release > "$work/example-release.out"
  cmp psionic/fixtures/imitation-before-robotics-scale.out "$work/example-debug.out"
  cmp psionic/fixtures/imitation-before-robotics-scale.out "$work/example-release.out"
fi
echo 'robotics: selected Coax units, focused mutation if requested, native debug/release and ASan/UBSan passed'
