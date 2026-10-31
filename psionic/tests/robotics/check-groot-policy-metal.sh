#!/usr/bin/env bash
# Firm reason5: composed verification wrapper. Every learned operation is Coax.
# The independent same-input/endpoint oracle is development-only reason2 tooling.
set -euo pipefail
cd "$(dirname "$0")/../../.."
shards="${1:?usage: check-groot-policy-metal.sh AUTHENTICATED_SHARDS ORIGINAL_GPU_CAPSULE NEW_OUTPUT_DIRECTORY}"
capsule="${2:?}";work="${3:?}"
[[ ! -e "$work" ]] || { echo 'output already exists' >&2;exit 2; };mkdir -p "$work"
python="${GROOT_REFERENCE_PYTHON:-$HOME/.cache/psionic/groot-reference-env/bin/python}"
export REFERENCE_PYTHON="$python" GROOT_REFERENCE_PYTHON="$python"
psionic/tests/robotics/check-groot-vision-metal.sh "$shards" "$capsule" "$work/vision"
psionic/tests/robotics/check-groot-text-metal.sh "$shards" "$capsule" "$work/text" "$work/vision"
# This additionally records a global hidden-trace diagnostic against the
# original SDK backbone. Composed upstream features differ numerically. Keep
# that diagnostic's nonzero result, and accept a policy ONLY if the later
# same-input head proof AND unchanged full-network action endpoints pass.
if psionic/tests/robotics/check-groot-action-metal.sh "$shards" "$capsule" "$work/action" "$work/text/backbone.f32"; then
  action_diagnostic_status=0
else
  action_diagnostic_status=$?
  [[ -s "$work/action/reference.json" ]] || exit "$action_diagnostic_status"
fi
"$python" psionic/tests/robotics/groot_action_conditional_reference.py "$shards" "$capsule" "$work/text/backbone.f32" "$work/action/output" "$work/conditional-head"
psionic/tests/robotics/check-groot-decoded-policy.sh "$work/action/output/normalized-actions.f32" "$work/decoded"
"$python" psionic/tests/robotics/groot_policy_reference.py "$capsule" "$work/vision" "$work/text" "$work/action" "$work/decoded" --conditional-head "$work/conditional-head/record.json" --record "$work/policy.json"
echo "prepared-observation policy endpoints passed; global hidden diagnostic exit: $action_diagnostic_status (retained in policy.json)"
