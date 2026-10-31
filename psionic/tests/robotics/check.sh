#!/usr/bin/env bash
# Firm reason5: thin dispatcher. Default robotics/all uses Coax, C and Python stdlib;
# explicit opt-in gates require the documented device/reference/private cache.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}";jobs=4;mutate=0;list=0;requested=();chosen=();explicit=()
units=(sim evaluate learn singlearm groot-artifact groot-linear groot-flow groot-vision groot-embodiment groot-dit groot-request groot-image groot-text groot-metal-plan groot-metal-kernels groot-vision-device groot-vision-metal-plan groot-text-metal-plan groot-device-kernels groot-text-kernels groot-action-metal-plan groot-action-metal-kernels groot-action-wide-kernels groot-action-transcript groot-action-dynamic-plan panda-matrix panda-control panda-env panda-loop panda-trace panda-policy panda-clock-adapter panda-fk-profile panda-fk-math panda-fk-chain panda-jacobian-math panda-jacobian panda-fingers-profile panda-fingers-chain panda-inertia-profile panda-inertia-math panda-inertia panda-inverse-dynamics-math panda-inverse-dynamics-motion panda-inverse-dynamics-force panda-inverse-dynamics panda-mass-solve-math panda-mass-solve-basis panda-mass-solve panda-forward-dynamics-net panda-forward-dynamics panda-actuation-math panda-actuation panda-owned-sample panda-query-source)
optins=(groot-device-gpu groot-text-kernels-gpu groot-linear-original groot-metal-original groot-vision-metal-original groot-text-metal-original groot-action-metal-original groot-policy-metal-original panda-control-original panda-env-sdk panda-query-source-sdk panda-loop-metal)
while [[ $# -gt 0 ]];do
  case "$1" in
    --jobs) [[ $# -ge 2 && $2 =~ ^[1-9][0-9]*$ ]] || exit 2;jobs=$2;shift 2;;
    --mutate) mutate=1;shift;;
    --list) list=1;shift;;
    *) unit=${1%.coax};unit=${unit#psionic/lib/};unit=${unit#robotics/};known=0
       if [[ $unit == panda-clock ]];then unit=panda-clock-adapter;fi
       if [[ $unit == panda-jacobian ]];then requested+=(panda-jacobian-math panda-jacobian);shift;continue;fi
       if [[ $unit == panda-fingers ]];then requested+=(panda-fingers-profile panda-fingers-chain);shift;continue;fi
       if [[ $unit == panda-inverse-dynamics ]];then requested+=(panda-inverse-dynamics-math panda-inverse-dynamics-motion panda-inverse-dynamics-force panda-inverse-dynamics);shift;continue;fi
       if [[ $unit == panda-actuation ]];then requested+=(panda-actuation-math panda-actuation);shift;continue;fi
       if [[ $unit == panda-forward-dynamics ]];then requested+=(panda-forward-dynamics-net panda-forward-dynamics);shift;continue;fi
       if [[ $unit == panda-mass-solve ]];then requested+=(panda-mass-solve-math panda-mass-solve-basis panda-mass-solve);shift;continue;fi
       if [[ $unit == panda-inertia ]];then requested+=(panda-inertia-profile panda-inertia-math panda-inertia);shift;continue;fi
       if [[ $unit == panda-fk ]];then requested+=(panda-fk-profile panda-fk-math panda-fk-chain panda-jacobian-math panda-jacobian);shift;continue;fi
       if [[ $unit == robotics || $unit == all ]];then known=1;unit=robotics;fi
       for candidate in "${units[@]}" "${optins[@]}";do if [[ $unit == "$candidate" ]];then known=1;break;fi;done
       [[ $known == 1 ]] || { echo "unknown robotics unit: $1" >&2;exit 2; };requested+=("$unit");shift;;
  esac
done
if [[ ${#requested[@]} == 0 ]];then requested=(robotics);fi
for unit in "${units[@]}";do for pick in "${requested[@]}";do if [[ $pick == robotics || $pick == "$unit" ]];then chosen+=("$unit");break;fi;done;done
for unit in "${optins[@]}";do for pick in "${requested[@]}";do if [[ $pick == "$unit" ]];then explicit+=("$unit");break;fi;done;done
if [[ $list == 1 ]];then
  for unit in "${chosen[@]}";do if [[ $unit == panda-forward-dynamics || $unit == panda-actuation || $unit == panda-query-source ]];then echo "Darwin-arm64-native: $unit";else echo "portable: $unit";fi;done
  for unit in "${explicit[@]}";do echo "opt-in: $unit";done
  exit 0
fi
work=$(mktemp -d "${TMPDIR:-/tmp}/psionic-robotics.XXXXXXXX");trap 'rm -rf "$work"' EXIT
core=0;example=0;metal=0;vision=0;text=0;action=0;action_plan=0;native_groups=0;optin_gates=0
for unit in "${chosen[@]}";do
  unit_flags=();if [[ $unit == panda-owned-sample || $unit == panda-actuation* || $unit == panda-query-source || $unit == panda-forward-dynamics* || $unit == panda-mass-solve* || $unit == panda-inverse-dynamics* || $unit == panda-jacobian-math || $unit == panda-inertia || $unit == panda-inertia-math || $unit == panda-inertia-profile ]];then unit_flags=(--library);fi
  "$coax" test "psionic/lib/robotics/$unit.coax" "${unit_flags[@]}"
  "$coax" test "psionic/lib/robotics/$unit.coax" --release "${unit_flags[@]}"
  if [[ $mutate == 1 ]];then
    if [[ $unit == panda-owned-sample ]];then
      pcs_scope=$(python3 psionic/tests/robotics/panda_owned_sample_mutation_scope.py --output "$work/panda-owned-sample-scope")
      "$coax" mutate "$pcs_scope" --library --root-only --jobs "$jobs"
    else "$coax" mutate "psionic/lib/robotics/$unit.coax" --root-only --jobs "$jobs" "${unit_flags[@]}";fi
  fi
  case "$unit" in
    sim|evaluate) core=1;;learn) core=1;example=1;;
    singlearm) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-singlearm.sh "$work/singlearm";native_groups=$((native_groups+1));;
    groot-metal-plan|groot-metal-kernels) metal=1;;
    groot-vision-device|groot-vision-metal-plan) vision=1;;
    groot-text-metal-plan) text=1;;
    groot-action-metal-plan|groot-action-transcript) action_plan=1;;
    groot-action-dynamic-plan) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-groot-action-dynamic-native.sh "$work/action-dynamic";native_groups=$((native_groups+1));;
    panda-matrix|panda-control) echo "$unit: portable VM/release checks; independent source native gate is explicit panda-control-original";;
    panda-env|panda-loop|panda-trace) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" "psionic/tests/robotics/check-$unit.sh" "$work/$unit";native_groups=$((native_groups+1));;
    panda-clock-adapter) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-panda-clock.sh "$work/panda-clock";COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-panda-clock-host.sh "$work/panda-clock-host";native_groups=$((native_groups+2));;
    panda-fk-chain) python3 psionic/tests/robotics/panda_fk_bindings_test.py;COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-panda-fk.sh "$PWD/psionic/fixtures/robotics/panda-fk/retained-controller" "$work/panda-fk";native_groups=$((native_groups+1));;
    panda-owned-sample)
      COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/panda_owned_sample_check.sh "$work/panda-owned-sample"
      python3 psionic/tests/robotics/panda_owned_sample_cache_alias_checks.py --qualified "$work/panda-owned-sample" --output "$work/panda-owned-sample-alias"
      CC="${COAX_CC:-${CC:-cc}}" python3 psionic/tests/robotics/panda_owned_controller_reference.py --qualified-samples "$work/panda-owned-sample-alias/alias" --output "$work/panda-owned-controller"
      native_groups=$((native_groups+2));;
    panda-actuation) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/panda_actuation_check.sh "$work/panda-actuation";native_groups=$((native_groups+1));;
    panda-query-source) CC="${COAX_CC:-${CC:-cc}}" python3 psionic/tests/robotics/panda_query_source_native_reference.py --compiler "$coax" --output "$work/panda-query-source";native_groups=$((native_groups+1));;
    panda-forward-dynamics) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/panda_forward_dynamics_check.sh "$work/panda-forward-dynamics";native_groups=$((native_groups+1));;
    panda-mass-solve) python3 psionic/tests/robotics/panda_mass_solve_guard_checks.py;COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/panda_mass_solve_check.sh "$work/panda-mass-solve";native_groups=$((native_groups+1));;
    panda-inverse-dynamics) "$coax" test psionic/tests/robotics/panda_inverse_dynamics_unit_checks.coax --only "Panda inverse force dimensions";COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/panda_inverse_dynamics_check.sh "$work/panda-inverse-dynamics";native_groups=$((native_groups+1));;
    panda-inertia) "$coax" test psionic/tests/robotics/panda_inertia_unit_checks.coax --only "Panda inertia typed blocks";COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/panda_inertia_check.sh "$work/panda-inertia";native_groups=$((native_groups+1));;
    panda-fingers-chain) python3 psionic/tests/robotics/panda_fingers_bindings_test.py;COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-panda-fingers.sh "$work/panda-fingers";native_groups=$((native_groups+1));;
    panda-jacobian) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/panda_jacobian_check.sh "$work/panda-jacobian";native_groups=$((native_groups+1));;
    panda-policy) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-panda-policy-native.sh "$work/panda-policy";native_groups=$((native_groups+1));;
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
  if [[ $unit == panda-query-source-sdk && ( $(uname -s) != Darwin || $(uname -m) != arm64 ) ]];then echo "$unit requires its qualified Darwin-arm64 SDK profile; it is not skipped" >&2;exit 2;fi
  if [[ $unit != groot-linear-original && $unit != panda-control-original && $(uname -s) != Darwin ]];then echo "$unit requires a compatible Apple Metal GPU; it is not skipped" >&2;exit 2;fi
  case "$unit" in
    panda-control-original) COAX="$coax" COAX_CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-panda-control.sh "${PSIONIC_PANDA_CONTROLLER_FIXTURES:?set PSIONIC_PANDA_CONTROLLER_FIXTURES to source PcSample captures}" "$work/panda-control-original";;
    panda-query-source-sdk)
      query_args=(--compiler "$coax" --xml "${PSIONIC_PANDA_SCENE:?set PSIONIC_PANDA_SCENE to the authenticated scene XML}" --initial "${PSIONIC_PANDA_INITIAL_STATE:?set PSIONIC_PANDA_INITIAL_STATE to the matching45F64 reset}" --trace "${PSIONIC_PANDA_QUERY_TRACE:?set PSIONIC_PANDA_QUERY_TRACE to the authenticated original action trace}" --output "${PSIONIC_PANDA_QUERY_OUTPUT:?set PSIONIC_PANDA_QUERY_OUTPUT to a NEW persistent output directory}")
      if [[ -n ${PSIONIC_PANDA_QUERY_SDK:-} ]];then query_args+=(--SDK "$PSIONIC_PANDA_QUERY_SDK");fi
      CC="${COAX_CC:-${CC:-cc}}" python3 psionic/tests/robotics/panda_query_source_prefix_reference.py "${query_args[@]}" --prepare-only
      CC="${COAX_CC:-${CC:-cc}}" python3 psionic/tests/robotics/panda_query_source_prefix_reference.py "${query_args[@]}" --execute-prepared;;
    panda-env-sdk) COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/check-panda-env-sdk.sh "${PSIONIC_PANDA_SCENE:?set PSIONIC_PANDA_SCENE to an audited original XML}" "${PSIONIC_PANDA_INITIAL_STATE:?set PSIONIC_PANDA_INITIAL_STATE to its matching45F64 state}" "$work/panda-env-sdk";;
    panda-loop-metal)
      read -r -a panda_demo_ids <<< "${PSIONIC_PANDA_DEMOS:-001}"
      COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/robotics/run-panda-loop-metal.sh "${PSIONIC_GROOT_SHARDS:?set PSIONIC_GROOT_SHARDS}" "${PSIONIC_PANDA_PST:?set PSIONIC_PANDA_PST to the authenticated original tokenizer}" "${PSIONIC_PANDA_NOISE:?set PSIONIC_PANDA_NOISE to the explicit noise artifact}" "${PSIONIC_PANDA_PAIRS:?set PSIONIC_PANDA_PAIRS to the audited per-demo pair directory}" "${PSIONIC_PANDA_OUTPUT:?set PSIONIC_PANDA_OUTPUT to a new persistent output directory}" "${PSIONIC_PANDA_TICKS:-720}" "${PSIONIC_PANDA_SCENARIO:-0}" "${panda_demo_ids[@]}";;
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
echo "robotics: ${#chosen[@]} selected Coax units, $native_groups native groups and $optin_gates explicitly requested opt-in gates passed"
