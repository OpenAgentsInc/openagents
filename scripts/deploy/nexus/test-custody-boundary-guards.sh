#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
BOUNDARY_DOC="${REPO_ROOT}/docs/deploy/NEXUS_CLOUDFLARE_BOUNDARY.md"
GCP_RUNBOOK="${REPO_ROOT}/docs/deploy/NEXUS_LDK_GCP_RUNBOOK.md"
TREASURY_DOC="${REPO_ROOT}/docs/nexus-treasury.md"
NEXUS_LIB="${REPO_ROOT}/apps/nexus-control/src/lib.rs"
NEXUS_TREASURY="${REPO_ROOT}/apps/nexus-control/src/treasury.rs"

assert_file_contains() {
  local needle="$1"
  local file="$2"
  if ! grep -Fq -- "$needle" "$file"; then
    printf 'missing expected content in %s: %s\n' "$file" "$needle" >&2
    exit 1
  fi
}

assert_file_not_contains() {
  local needle="$1"
  local file="$2"
  if grep -Fq -- "$needle" "$file"; then
    printf 'unexpected content in %s: %s\n' "$file" "$needle" >&2
    exit 1
  fi
}

assert_file_contains 'Cloudflare Workers must not store or log:' "$BOUNDARY_DOC"
assert_file_contains 'Browser code must never instantiate the production LDK node' "$BOUNDARY_DOC"
assert_file_contains 'Admin routes must require WorkOS/API-token authorization before calling' "$BOUNDARY_DOC"
assert_file_contains 'Admin writes must pass an idempotency key' "$BOUNDARY_DOC"
assert_file_contains 'GET /v1/treasury/projections' "$BOUNDARY_DOC"
assert_file_contains 'POST /v1/admin/treasury/operations' "$BOUNDARY_DOC"

assert_file_contains 'NEXUS_CLOUDFLARE_BOUNDARY.md' "$GCP_RUNBOOK"
assert_file_contains 'deploy/NEXUS_CLOUDFLARE_BOUNDARY.md' "$TREASURY_DOC"

assert_file_contains '"/v1/admin/treasury/operations"' "$NEXUS_LIB"
assert_file_contains '"/api/admin/treasury/operations"' "$NEXUS_LIB"
assert_file_contains 'authenticate_admin_bearer_token(&state, &headers)?;' "$NEXUS_LIB"
assert_file_contains 'treasury_admin_idempotency_key_required' "$NEXUS_LIB"
assert_file_contains 'record_treasury_admin_command' "$NEXUS_LIB"
assert_file_contains 'safe_treasury_operation_metadata' "$NEXUS_LIB"
assert_file_contains 'TreasuryOperationKind::LightningAdminCommand' "$NEXUS_TREASURY"

if grep -RIn --include='*.ts' --include='*.tsx' --include='*.js' --include='*.jsx' \
  -E 'NEXUS_(LDK|BITCOIND).*(SEED|API_KEY|TLS|RPC)|keys_seed|ldk_node_data.sqlite|treasury spend authority' \
  "${REPO_ROOT}/apps/deck" "${REPO_ROOT}/apps/autopilot" 2>/dev/null; then
  printf 'web/client code appears to reference custody material\n' >&2
  exit 1
fi

assert_file_not_contains 'Spark provider' "$BOUNDARY_DOC"

printf 'ok: Nexus Cloudflare boundary docs and admin custody guards are present\n'
