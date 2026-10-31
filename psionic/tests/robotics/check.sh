#!/usr/bin/env bash
# Firm reason5: thin dispatcher. Default robotics/all uses Coax, C and Python stdlib;
# explicit opt-in gates require the documented device/reference/private cache.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}";jobs=4;mutate=0;list=0;requested=();chosen=();explicit=()
units=(sim evaluate learn singlearm groot-artifact groot-linear groot-flow groot-vision groot-embodiment groot-dit groot-request groot-image groot-text groot-metal-plan groot-metal-kernels groot-vision-device groot-vision-metal-plan groot-text-metal-plan groot-device-kernels groot-text-kernels groot-action-metal-plan groot-action-metal-kernels groot-action-wide-kernels groot-action-transcript)
optins=(groot-device-gpu groot-text-kernels-gpu groot-linear-original groot-metal-original groot-vision-metal-original groot-text-metal-original groot-action-metal-original groot-policy-metal-original)
while [[ $# -gt 0 ]];do
  case "$1" in
    --jobs) [[ $# -ge 2 && $2 =~ ^[1-9][0-9]*$ ]] || exit 2;jobs=$2;shift 2;;
    --mutate) mutate=1;shift;;
    --list) list=1;shift;;
    *) unit=${1%.coax};unit=${unit#psionic/lib/};unit=${unit#robotics/};known=0
       if [[ $unit == robotics || $unit == all ]];then known=1;unit=robotics;fi
       for candidate in "${units[@]}" "${optins[@]}";do if [[ $unit == "$candidate" ]];then known=1;break;fi;done
       [[ $known == 1 ]] || { echo "unknown robotics unit: $1" >&2;exit 2; };requested+=("$unit");shift;;
  esac
done
if [[ ${#requested[@]} == 0 ]];then requested=(robotics);fi
for unit in "${units[@]}";do for pick in "${requested[@]}";do if [[ $pick == robotics || $pick == "$unit" ]];then chosen+=("$unit");break;fi;done;done
for unit in "${optins[@]}";do for pick in "${requested[@]}";do if [[ $pick == "$unit" ]];then explicit+=("$unit");break;fi;done;done
if [[ $list == 1 ]];then
  for unit in "${chosen[@]}";do echo "portable: $unit";done
  for unit in "${explicit[@]}";do echo "opt-in: $unit";done
  exit 0
fi
work=$(mktemp -d "${TMPDIR:-/tmp}/psionic-robotics.XXXXXXXX");trap 'rm -rf "$work"' EXIT
core=0;example=0;metal=0;vision=0;text=0;action=0;action_plan=0;native_groups=0;optin_gates=0
for unit in "${chosen[@]}";do
  "$coax" test "psionic/lib/robotics/$unit.coax"
  "$coax" test "psionic/lib/robotics/$unit.coax" --release
  if [[ $mutate == 1 ]];then "$coax" mutate "psionic/lib/robotics/$unit.coax" --root-only --jobs "$jobs";fi
  case "$unit" in
    sim|evaluate) core=1;;learn) core=1;example=1;;
    singlearm) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-singlearm.sh "$work/singlearm";native_groups=$((native_groups+1));;
    groot-metal-plan|groot-metal-kernels) metal=1;;
    groot-vision-device|groot-vision-metal-plan) vision=1;;
    groot-text-metal-plan) text=1;;
    groot-action-metal-plan|groot-action-transcript) action_plan=1;;
    groot-action-metal-kernels|groot-action-wide-kernels) action=1;;
    groot-device-kernels) echo "$unit: portable VM/release checks; actual GPU/Torch gate is explicit groot-device-gpu";;
    groot-text-kernels) echo "$unit: portable VM/release checks; actual GPU/Torch gate is explicit groot-text-kernels-gpu";;
    groot-*) COAX="$coax" COAX_CC="${COAX_CC:-${CC:-cc}}" CC="${COAX_CC:-${CC:-cc}}" "psionic/tests/robotics/check-$unit.sh" "$work/$unit";native_groups=$((native_groups+1));;
  esac
done
if [[ $metal == 1 ]];then COAX="$coax" COAX_CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-metal-native.sh;native_groups=$((native_groups+1));fi
if [[ $vision == 1 ]];then COAX="$coax" COAX_CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-vision-metal-native.sh;native_groups=$((native_groups+1));fi
if [[ $text == 1 ]];then COAX="$coax" COAX_CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-text-metal-plan-native.sh "$work/text-plan";native_groups=$((native_groups+1));fi
if [[ $action == 1 ]];then COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-action-native.sh "$work/action";native_groups=$((native_groups+1));fi
if [[ $action_plan == 1 ]];then COAX="$coax" COAX_CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-action-plan-native.sh "$work/action-plan";native_groups=$((native_groups+1));fi
if [[ $core == 1 ]];then COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-native.sh "$work/core";native_groups=$((native_groups+1));fi
if [[ $example == 1 ]];then
  "$coax" run psionic/examples/imitation-before-robotics-scale.coax > "$work/example-debug.out"
  "$coax" run psionic/examples/imitation-before-robotics-scale.coax --release > "$work/example-release.out"
  cmp psionic/fixtures/imitation-before-robotics-scale.out "$work/example-debug.out"
  cmp psionic/fixtures/imitation-before-robotics-scale.out "$work/example-release.out"
fi
for unit in "${explicit[@]}";do
  if [[ $unit != groot-linear-original && $(uname -s) != Darwin ]];then echo "$unit requires a compatible Apple Metal GPU; it is not skipped" >&2;exit 2;fi
  case "$unit" in
    groot-device-gpu) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-device.sh "$work/device-gpu";;
    groot-text-kernels-gpu) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-text-kernels.sh "$work/text-gpu";;
    groot-linear-original) COAX="$coax" COAX_CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-original.sh "${PSIONIC_GROOT_STATE_WEIGHTS:?set PSIONIC_GROOT_STATE_WEIGHTS to the authenticated four category2 parts}" "$work/original-linear";;
    groot-metal-original) COAX="$coax" COAX_CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-metal.sh "${PSIONIC_GROOT_STATE_WEIGHTS:?set PSIONIC_GROOT_STATE_WEIGHTS to the authenticated four category2 parts}" "$work/original-state-metal";;
    groot-vision-metal-original) COAX="$coax" COAX_CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-vision-metal.sh "${PSIONIC_GROOT_SHARDS:?set PSIONIC_GROOT_SHARDS}" "${PSIONIC_GROOT_CAPSULE:?set PSIONIC_GROOT_CAPSULE}" "$work/original-vision";;
    groot-text-metal-original) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-text-metal.sh "${PSIONIC_GROOT_SHARDS:?set PSIONIC_GROOT_SHARDS}" "${PSIONIC_GROOT_CAPSULE:?set PSIONIC_GROOT_CAPSULE}" "$work/original-text" ${PSIONIC_GROOT_VISION_OUTPUTS:+"$PSIONIC_GROOT_VISION_OUTPUTS"};;
    groot-action-metal-original) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-action-metal.sh "${PSIONIC_GROOT_SHARDS:?set PSIONIC_GROOT_SHARDS}" "${PSIONIC_GROOT_CAPSULE:?set PSIONIC_GROOT_CAPSULE}" "$work/original-action" ${PSIONIC_GROOT_BACKBONE:+"$PSIONIC_GROOT_BACKBONE"};;
    groot-policy-metal-original) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-policy-metal.sh "${PSIONIC_GROOT_SHARDS:?set PSIONIC_GROOT_SHARDS}" "${PSIONIC_GROOT_CAPSULE:?set PSIONIC_GROOT_CAPSULE}" "$work/original-policy";;
  esac
  optin_gates=$((optin_gates+1))
done
echo "robotics: ${#chosen[@]} portable Coax units, $native_groups native groups and $optin_gates explicitly requested opt-in gates passed"
