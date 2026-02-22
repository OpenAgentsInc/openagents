#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/mixed-version-deploy-safety/${TIMESTAMP}}"
FIXTURE_DIR="${OUTPUT_DIR}/fixtures"
WORK_DIR="${OUTPUT_DIR}/work"
RESULTS_JSONL="${OUTPUT_DIR}/checks.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"
MANIFEST_PATH_FILE="${OUTPUT_DIR}/manifest_path.txt"

mkdir -p "${OUTPUT_DIR}" "${FIXTURE_DIR}" "${WORK_DIR}"
: >"${RESULTS_JSONL}"

AUTH_STORE_PATH="${FIXTURE_DIR}/auth-store.json"
CODEX_THREAD_STORE_PATH="${FIXTURE_DIR}/codex-thread-store.json"
DOMAIN_STORE_PATH="${FIXTURE_DIR}/domain-store.json"

cat >"${AUTH_STORE_PATH}" <<'JSON'
{
  "users_by_id": {
    "user-1": {
      "id": "user-1",
      "email": "migration@example.com"
    }
  },
  "sessions": {
    "session-1": {
      "id": "session-1",
      "user_id": "user-1"
    }
  },
  "personal_access_tokens": {
    "token-1": {
      "id": "token-1",
      "token": "hashed-token"
    }
  }
}
JSON

cat >"${CODEX_THREAD_STORE_PATH}" <<'JSON'
{
  "threads": {
    "thread-1": {
      "id": "thread-1",
      "title": "Parity Thread"
    }
  },
  "messages_by_thread": {
    "thread-1": [
      {
        "id": "message-1",
        "role": "user",
        "content": "hello"
      }
    ]
  }
}
JSON

cat >"${DOMAIN_STORE_PATH}" <<'JSON'
{
  "user_integrations": {
    "user-1::resend": {
      "id": 1,
      "provider": "resend"
    }
  },
  "user_integration_audits": [
    {
      "id": 2,
      "action": "upsert"
    }
  ],
  "comms_webhook_events": {
    "idempotency-1": {
      "id": 3,
      "status": "received"
    }
  },
  "comms_delivery_projections": {
    "scope-1": {
      "id": 4,
      "status": "queued"
    }
  },
  "shouts": [
    {
      "id": 5,
      "content": "hello world"
    }
  ],
  "whispers": [
    {
      "id": 6,
      "content": "private"
    }
  ]
}
JSON

sha256_file() {
  sha256sum "$1" | awk '{print $1}'
}

PRE_AUTH_SHA="$(sha256_file "${AUTH_STORE_PATH}")"
PRE_CODEX_SHA="$(sha256_file "${CODEX_THREAD_STORE_PATH}")"
PRE_DOMAIN_SHA="$(sha256_file "${DOMAIN_STORE_PATH}")"

overall_failed=0

run_step() {
  local check_id="$1"
  local description="$2"
  shift 2

  local log_path="${OUTPUT_DIR}/${check_id}.log"
  local started_at ended_at status reason
  started_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  if (
    cd "${ROOT_DIR}"
    "$@"
  ) >"${log_path}" 2>&1; then
    status="pass"
    reason=""
  else
    status="fail"
    reason="command_failed"
    overall_failed=1
  fi

  ended_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  jq -n \
    --arg check_id "${check_id}" \
    --arg description "${description}" \
    --arg command "$(printf '%q ' "$@")" \
    --arg status "${status}" \
    --arg reason "${reason}" \
    --arg started_at "${started_at}" \
    --arg ended_at "${ended_at}" \
    --arg log_path "${log_path}" \
    '{
      check_id: $check_id,
      description: $description,
      command: $command,
      status: $status,
      reason: (if $reason == "" then null else $reason end),
      started_at: $started_at,
      ended_at: $ended_at,
      log_path: $log_path
    }' >>"${RESULTS_JSONL}"

  echo "[mixed-version-harness] ${check_id}: ${status}"
}

run_step "backfill-script-syntax" "Backfill wrapper script syntax is valid" \
  bash -n apps/openagents.com/service/scripts/run-rust-ownership-backfill.sh

run_step "verify-script-syntax" "Backfill verification script syntax is valid" \
  bash -n apps/openagents.com/service/scripts/verify-rust-ownership-backfill.sh

run_step "rollback-script-syntax" "Backfill rollback script syntax is valid" \
  bash -n apps/openagents.com/service/scripts/rollback-rust-ownership-backfill.sh

run_step "rust-store-migrate-tests" "Rust store migration unit tests pass" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml --bin rust_store_migrate

run_step "run-backfill" "Backfill runs and writes migration manifest" \
  env WORK_DIR="${WORK_DIR}" \
    AUTH_STORE_PATH="${AUTH_STORE_PATH}" \
    CODEX_THREAD_STORE_PATH="${CODEX_THREAD_STORE_PATH}" \
    DOMAIN_STORE_PATH="${DOMAIN_STORE_PATH}" \
    ./apps/openagents.com/service/scripts/run-rust-ownership-backfill.sh

run_step "discover-manifest" "Backfill manifest can be discovered from working directory" \
  bash -c 'set -euo pipefail; manifest="$(ls -1 "$1"/manifests/*.json | sort | tail -n 1)"; printf "%s" "$manifest" > "$2"' _ \
  "${WORK_DIR}" "${MANIFEST_PATH_FILE}"

MANIFEST_PATH=""
if [[ -f "${MANIFEST_PATH_FILE}" ]]; then
  MANIFEST_PATH="$(cat "${MANIFEST_PATH_FILE}")"
fi

if [[ -z "${MANIFEST_PATH}" ]]; then
  MANIFEST_PATH="${WORK_DIR}/manifests/missing-manifest.json"
fi

run_step "verify-manifest-checksums" "Manifest checksums verify against migrated stores" \
  ./apps/openagents.com/service/scripts/verify-rust-ownership-backfill.sh "${MANIFEST_PATH}"

run_step "validate-manifest-invariants" "Manifest contains expected mixed-version checksum/count invariants" \
  jq -e '
    .schema == "openagents.rust_store_migration.v1" and
    (.stores | length == 3) and
    (.stores | all(.[]; (.after_sha256 | type == "string") and (.after_sha256 | length == 64))) and
    ((.stores | map(select(.store == "auth")) | length) == 1) and
    ((.stores | map(select(.store == "codex_threads")) | length) == 1) and
    ((.stores | map(select(.store == "domain")) | length) == 1) and
    ((.stores | map(select(.store == "auth")) | .[0].counts.users) == 1) and
    ((.stores | map(select(.store == "auth")) | .[0].counts.sessions) == 1) and
    ((.stores | map(select(.store == "auth")) | .[0].counts.personal_access_tokens) == 1) and
    ((.stores | map(select(.store == "codex_threads")) | .[0].counts.threads) == 1) and
    ((.stores | map(select(.store == "codex_threads")) | .[0].counts.messages) == 1) and
    ((.stores | map(select(.store == "domain")) | .[0].counts.autopilots) == 0) and
    ((.stores | map(select(.store == "domain")) | .[0].counts.l402_paywalls) == 0) and
    ((.stores | map(select(.store == "domain")) | .[0].counts.integrations) == 1) and
    ((.stores | map(select(.store == "domain")) | .[0].counts.shouts) == 1) and
    ((.stores | map(select(.store == "domain")) | .[0].counts.whispers) == 1)
  ' "${MANIFEST_PATH}"

run_step "run-rollback" "Rollback restores pre-backfill store snapshots" \
  ./apps/openagents.com/service/scripts/rollback-rust-ownership-backfill.sh "${MANIFEST_PATH}"

run_step "validate-rollback-hashes" "Rollback hashes match pre-migration snapshots" \
  bash -c 'set -euo pipefail
    auth_sha="$(sha256sum "$1" | cut -d " " -f 1)"
    codex_sha="$(sha256sum "$2" | cut -d " " -f 1)"
    domain_sha="$(sha256sum "$3" | cut -d " " -f 1)"
    [[ "$auth_sha" == "$4" ]]
    [[ "$codex_sha" == "$5" ]]
    [[ "$domain_sha" == "$6" ]]' _ \
  "${AUTH_STORE_PATH}" "${CODEX_THREAD_STORE_PATH}" "${DOMAIN_STORE_PATH}" \
  "${PRE_AUTH_SHA}" "${PRE_CODEX_SHA}" "${PRE_DOMAIN_SHA}"

run_step "mixed-version-runbook-present" "Mixed-version deploy safety runbook exists" \
  rg -q "Expand / Migrate / Contract" apps/openagents.com/service/docs/MIXED_VERSION_DEPLOY_SAFETY.md

jq -s \
  --arg generated_at "${TIMESTAMP}" \
  '{
    schema: "openagents.webparity.mixed_version_safety_harness.v1",
    generated_at: $generated_at,
    totals: {
      check_count: length,
      passed: (map(select(.status == "pass")) | length),
      failed: (map(select(.status == "fail")) | length)
    },
    overall_status: (if (map(select(.status == "fail")) | length) > 0 then "failed" else "passed" end),
    checks: .
  }' "${RESULTS_JSONL}" >"${SUMMARY_JSON}"

{
  echo "# Mixed-Version Deploy Safety Harness"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo
  echo "| Check | Status | Description | Log |"
  echo "| --- | --- | --- | --- |"
  jq -r '.checks[] | "| \(.check_id) | \(.status) | \(.description) | `\(.log_path)` |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[mixed-version-harness] summary: ${SUMMARY_JSON}"
echo "[mixed-version-harness] report: ${SUMMARY_MD}"

if [[ "${overall_failed}" -ne 0 ]]; then
  echo "error: mixed-version deploy safety harness failed" >&2
  exit 1
fi
