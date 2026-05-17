#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd rg

BUILD_CONTEXT_SCRIPT="${SCRIPT_DIR}/stage-build-context.sh"
[[ -f "$BUILD_CONTEXT_SCRIPT" ]] || die "Missing build context helper: ${BUILD_CONTEXT_SCRIPT}"

TMP_CONTEXT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/openagents-nexus-build-context.XXXXXX")"
trap 'rm -rf "$TMP_CONTEXT_DIR"' EXIT

bash "$BUILD_CONTEXT_SCRIPT" "$TMP_CONTEXT_DIR" >/dev/null

FORBIDDEN_PACKAGE_PATTERN='openagents-s[p]ark|breez-sdk-s[p]ark|s[p]ark-wallet|name = "s[p]ark"|breez/s[p]ark-sdk'
FORBIDDEN_DRAIN_PATTERN='S[p]arkFinal[D]rain|s[p]ark_final_[d]rain|NEXUS_S[P]ARK_FINAL_[D]RAIN_ENABLED'
FORBIDDEN_PROVIDER_PATTERN='NEXUS_TREASURY_PROVIDER=s[p]ark|provider=s[p]ark'

if rg -n "$FORBIDDEN_PACKAGE_PATTERN" "$TMP_CONTEXT_DIR" -S; then
  die "Spark package dependency found in staged Nexus build context"
fi

ACTIVE_PATHS=(
  "apps/nexus-control/src"
  "apps/nexus-relay/src"
  "apps/nexus-relay/deploy"
  "apps/pylon/src"
  "crates/openagents-provider-substrate/src"
  "scripts/deploy/nexus"
)

if rg -n "$FORBIDDEN_DRAIN_PATTERN" \
  "${ACTIVE_PATHS[@]/#/${ROOT_DIR}/}" \
  --glob '!**/test-ldk-deploy-invariants.sh' \
  -S; then
  die "Spark drain/runtime provider symbol found in active Nexus/Pylon paths"
fi

if rg -n "$FORBIDDEN_PROVIDER_PATTERN" \
  "${ACTIVE_PATHS[@]/#/${ROOT_DIR}/}" \
  --glob '!**/test-ldk-deploy-invariants.sh' \
  -S; then
  die "Spark provider selector found in active Nexus/Pylon paths"
fi

printf 'Nexus/Pylon active paths are LDK-only; no Spark runtime/deploy dependency found.\n'
