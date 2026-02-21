#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-changed}"

PROTO_TRIGGER_PATTERN='^(proto/|buf\.yaml$|buf\.gen\.yaml$|scripts/verify-proto-generate\.sh$|scripts/verify-rust-proto-crate\.sh$|crates/openagents-proto/)'
RUNTIME_TRIGGER_PATTERN='^(apps/runtime/|proto/|buf\.yaml$|buf\.gen\.yaml$)'
COMMS_TRIGGER_PATTERN='^(apps/openagents\.com/|apps/runtime/|docs/protocol/comms/|scripts/comms-security-replay-matrix\.sh$)'
OPENCLAW_TRIGGER_PATTERN='^(docs/plans/active/openclaw-intake/|apps/runtime/test/fixtures/openclaw/|scripts/openclaw-drift-report\.sh$)'

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing required command: $cmd" >&2
    exit 1
  fi
}

collect_changed_files() {
  local files
  files="$(git -C "$ROOT_DIR" diff --cached --name-only)"
  if [[ -z "$files" ]]; then
    files="$(git -C "$ROOT_DIR" diff --name-only HEAD)"
  fi
  printf '%s\n' "$files" | sed '/^$/d'
}

run_docs_check() {
  echo "==> docs-check"
  "$ROOT_DIR/scripts/docs-check.mjs"
}

run_proto_checks() {
  echo "==> proto checks"
  require_cmd buf
  require_cmd rg

  local against_ref
  against_ref="${OA_BUF_BREAKING_AGAINST:-}"

  if [[ -z "$against_ref" ]]; then
    if git -C "$ROOT_DIR" show-ref --verify --quiet refs/heads/main; then
      against_ref=".git#branch=main,subdir=proto"
    elif git -C "$ROOT_DIR" show-ref --verify --quiet refs/remotes/origin/main; then
      against_ref=".git#branch=origin/main,subdir=proto"
    fi
  fi

  if [[ -z "$against_ref" ]]; then
    echo "buf breaking failed: could not find local 'main' or 'origin/main'." >&2
    echo "Set OA_BUF_BREAKING_AGAINST to override baseline explicitly, for example:" >&2
    echo "  OA_BUF_BREAKING_AGAINST='.git#branch=origin/main,subdir=proto' ./scripts/local-ci.sh proto" >&2
    echo "Or fetch baseline refs:" >&2
    echo "  git fetch origin main" >&2
    exit 1
  fi

  (
    cd "$ROOT_DIR"
    buf lint
    buf breaking --against "$against_ref"

    ./scripts/verify-proto-generate.sh
  )
}

assert_trigger() {
  local label="$1"
  local pattern="$2"
  local file="$3"
  local expected="$4"
  local matched="false"

  if printf '%s\n' "$file" | rg -q "$pattern"; then
    matched="true"
  fi

  if [[ "$matched" != "$expected" ]]; then
    echo "trigger test failed: ${label} pattern mismatch for '${file}' (expected=${expected}, got=${matched})" >&2
    exit 1
  fi
}

run_trigger_tests() {
  echo "==> local-ci trigger tests"

  assert_trigger "proto" "$PROTO_TRIGGER_PATTERN" "proto/openagents/sync/v1/sync.proto" "true"
  assert_trigger "proto" "$PROTO_TRIGGER_PATTERN" "crates/openagents-proto/src/lib.rs" "true"
  assert_trigger "proto" "$PROTO_TRIGGER_PATTERN" "docs/README.md" "false"

  assert_trigger "runtime" "$RUNTIME_TRIGGER_PATTERN" "apps/runtime/lib/foo.ex" "true"
  assert_trigger "runtime" "$RUNTIME_TRIGGER_PATTERN" "crates/openagents-proto/src/lib.rs" "false"

  assert_trigger "comms" "$COMMS_TRIGGER_PATTERN" "apps/openagents.com/src/routes.tsx" "true"
  assert_trigger "comms" "$COMMS_TRIGGER_PATTERN" "docs/protocol/comms/README.md" "true"
  assert_trigger "comms" "$COMMS_TRIGGER_PATTERN" "proto/openagents/sync/v1/sync.proto" "false"

  assert_trigger "openclaw" "$OPENCLAW_TRIGGER_PATTERN" "scripts/openclaw-drift-report.sh" "true"
  assert_trigger "openclaw" "$OPENCLAW_TRIGGER_PATTERN" "scripts/local-ci.sh" "false"

  echo "local-ci trigger tests passed"
}

run_runtime_checks() {
  echo "==> runtime checks"
  (
    cd "$ROOT_DIR/apps/runtime"
    mix format --check-formatted
    mix compile --warnings-as-errors
    mix runtime.contract.check
    mix test --warnings-as-errors
  )
}

run_comms_matrix() {
  echo "==> comms security/replay matrix"
  (
    cd "$ROOT_DIR"
    ./scripts/comms-security-replay-matrix.sh all
  )
}

run_openclaw_drift() {
  echo "==> openclaw drift strict gate"
  (
    cd "$ROOT_DIR"
    OPENCLAW_DRIFT_FAIL_ON_ACTIONABLE=1 ./scripts/openclaw-drift-report.sh
  )
}

has_match() {
  local pattern="$1"
  local files="$2"
  if [[ -z "$files" ]]; then
    return 1
  fi
  printf '%s\n' "$files" | rg -q "$pattern"
}

run_all() {
  run_proto_checks
  run_runtime_checks
  run_comms_matrix
  run_openclaw_drift
}

run_changed() {
  local changed_files
  changed_files="$(collect_changed_files)"

  if [[ -z "$changed_files" ]]; then
    echo "No changed files detected; nothing to run."
    return 0
  fi

  if has_match "$PROTO_TRIGGER_PATTERN" "$changed_files"; then
    run_proto_checks
  fi

  if has_match "$RUNTIME_TRIGGER_PATTERN" "$changed_files"; then
    run_runtime_checks
  fi

  if has_match "$COMMS_TRIGGER_PATTERN" "$changed_files"; then
    run_comms_matrix
  fi

  if has_match "$OPENCLAW_TRIGGER_PATTERN" "$changed_files"; then
    run_openclaw_drift
  fi
}

case "$MODE" in
  docs)
    run_docs_check
    ;;
  proto)
    run_proto_checks
    ;;
  runtime)
    run_runtime_checks
    ;;
  comms)
    run_comms_matrix
    ;;
  openclaw)
    run_openclaw_drift
    ;;
  test-triggers)
    run_trigger_tests
    ;;
  all)
    run_all
    ;;
  changed)
    run_changed
    ;;
  *)
    echo "Usage: scripts/local-ci.sh [changed|all|docs|proto|runtime|comms|openclaw|test-triggers]" >&2
    exit 2
    ;;
esac
