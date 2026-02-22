#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-changed}"
ENABLE_LEGACY_LANES="${OA_LOCAL_CI_ENABLE_LEGACY:-0}"

PROTO_TRIGGER_PATTERN='^(proto/|buf\.yaml$|buf\.gen\.yaml$|scripts/verify-proto-generate\.sh$|scripts/verify-rust-proto-crate\.sh$|crates/openagents-proto/)'
RUNTIME_TRIGGER_PATTERN='^(apps/runtime/|proto/|buf\.yaml$|buf\.gen\.yaml$)'
RUNTIME_HISTORY_TRIGGER_PATTERN='^(apps/runtime/src/|apps/runtime/fixtures/history_compat/|apps/runtime/Cargo\.toml$|Cargo\.lock$)'
LEGACY_COMMS_TRIGGER_PATTERN='^(apps/openagents\.com/(app/|bootstrap/|config/|database/|resources/|routes/|tests/|artisan$|composer\.json$|composer\.lock$|phpunit\.xml$)|docs/protocol/comms/|scripts/comms-security-replay-matrix\.sh$)'
LEGACY_LEGACYPARITY_TRIGGER_PATTERN='^(docs/plans/active/legacyparity-intake/|apps/runtime/test/fixtures/legacyparity/|scripts/legacyparity-drift-report\.sh$)'
WEB_SHELL_TRIGGER_PATTERN='^(apps/openagents\.com/web-shell/)'
CROSS_SURFACE_TRIGGER_PATTERN='^(apps/openagents\.com/web-shell/|apps/autopilot-desktop/|apps/autopilot-ios/|docs/autopilot/testing/CROSS_SURFACE_CONTRACT_HARNESS\.md$|docs/autopilot/testing/cross-surface-contract-scenarios\.json$|scripts/run-cross-surface-contract-harness\.sh$)'
IOS_RUST_CORE_TRIGGER_PATTERN='^(apps/autopilot-ios/|crates/openagents-client-core/|apps/autopilot-ios/scripts/build-rust-client-core\.sh$|apps/autopilot-ios/scripts/verify-rust-client-core-reproducibility\.sh$)'
RUST_WORKSPACE_COMPILE_TRIGGER_PATTERN='^(Cargo\.toml$|Cargo\.lock$|crates/|apps/openagents\.com/service/|apps/openagents\.com/web-shell/|apps/autopilot-desktop/|apps/autopilot-ios/|apps/runtime/src/|apps/runtime/Cargo\.toml$|apps/runtime/tests?/|apps/lightning-ops/|apps/lightning-wallet-executor/|apps/onyx/|scripts/local-ci\.sh$)'
RUNTIME_CODEX_WORKERS_PHP_TRIGGER_PATTERN='^(apps/openagents\.com/(app/Http/Controllers/Api/RuntimeCodexWorkersController\.php|app/AI/Runtime/RuntimeCodexClient\.php|config/runtime\.php|routes/api\.php|tests/Feature/Api/RuntimeCodexWorkersApiTest\.php))'

is_truthy() {
  local value="${1:-}"
  local lowered
  lowered="$(printf '%s' "$value" | tr '[:upper:]' '[:lower:]')"
  case "$lowered" in
    1|true|yes|on)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

legacy_lanes_enabled() {
  is_truthy "$ENABLE_LEGACY_LANES"
}

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

  local mode
  mode="${OA_BUF_BREAKING_MODE:-auto}"
  local timeout
  timeout="${OA_BUF_BREAKING_TIMEOUT:-8s}"
  local against_ref
  against_ref="${OA_BUF_BREAKING_AGAINST:-}"

  if [[ -z "$against_ref" ]]; then
    if git -C "$ROOT_DIR" show-ref --verify --quiet refs/heads/main; then
      against_ref=".git#branch=main,subdir=proto"
    elif git -C "$ROOT_DIR" show-ref --verify --quiet refs/remotes/origin/main; then
      against_ref=".git#branch=origin/main,subdir=proto"
    fi
  fi

  (
    cd "$ROOT_DIR"
    buf lint
    ./scripts/verify-proto-generate.sh
  )

  if [[ "$mode" == "off" ]]; then
    echo "==> buf breaking skipped (OA_BUF_BREAKING_MODE=off)"
    return 0
  fi

  if [[ -z "$against_ref" ]]; then
    if [[ "$mode" == "strict" ]]; then
      echo "buf breaking failed: could not find local 'main' or 'origin/main'." >&2
      echo "Set OA_BUF_BREAKING_AGAINST to override baseline explicitly, for example:" >&2
      echo "  OA_BUF_BREAKING_AGAINST='.git#branch=origin/main,subdir=proto' OA_BUF_BREAKING_MODE=strict ./scripts/local-ci.sh proto" >&2
      echo "Or fetch baseline refs:" >&2
      echo "  git fetch origin main" >&2
      exit 1
    fi

    echo "==> buf breaking skipped in auto mode (no baseline ref found)"
    return 0
  fi

  echo "==> buf breaking (mode=${mode}, timeout=${timeout})"
  local output
  local status
  set +e
  output="$(
    cd "$ROOT_DIR" &&
      buf breaking --timeout "$timeout" --against "$against_ref" 2>&1
  )"
  status=$?
  set -e

  if [[ $status -eq 0 ]]; then
    return 0
  fi

  if [[ "$mode" == "strict" ]]; then
    printf '%s\n' "$output" >&2
    exit $status
  fi

  if printf '%s' "$output" | rg -qi '(too many requests|rate.?limit|timed out|timeout|deadline exceeded|temporar|transport|connection|unavailable|i/o timeout|tls|eof)'; then
    echo "==> buf breaking skipped in auto mode due transient/remote failure:"
    printf '%s\n' "$output"
    return 0
  fi

  printf '%s\n' "$output" >&2
  exit $status
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

  assert_trigger "runtime" "$RUNTIME_TRIGGER_PATTERN" "apps/runtime/src/main.rs" "true"
  assert_trigger "runtime" "$RUNTIME_TRIGGER_PATTERN" "crates/openagents-proto/src/lib.rs" "false"
  assert_trigger "runtime-history" "$RUNTIME_HISTORY_TRIGGER_PATTERN" "apps/runtime/src/history_compat.rs" "true"
  assert_trigger "runtime-history" "$RUNTIME_HISTORY_TRIGGER_PATTERN" "apps/runtime/docs/DEPLOY_GCP.md" "false"

  assert_trigger "legacy-comms" "$LEGACY_COMMS_TRIGGER_PATTERN" "apps/openagents.com/routes/web.php" "true"
  assert_trigger "legacy-comms" "$LEGACY_COMMS_TRIGGER_PATTERN" "apps/openagents.com/service/src/lib.rs" "false"
  assert_trigger "legacy-comms" "$LEGACY_COMMS_TRIGGER_PATTERN" "docs/protocol/comms/README.md" "true"
  assert_trigger "legacy-comms" "$LEGACY_COMMS_TRIGGER_PATTERN" "proto/openagents/sync/v1/sync.proto" "false"

  assert_trigger "legacy-legacyparity" "$LEGACY_LEGACYPARITY_TRIGGER_PATTERN" "scripts/legacyparity-drift-report.sh" "true"
  assert_trigger "legacy-legacyparity" "$LEGACY_LEGACYPARITY_TRIGGER_PATTERN" "scripts/local-ci.sh" "false"

  assert_trigger "web-shell" "$WEB_SHELL_TRIGGER_PATTERN" "apps/openagents.com/web-shell/src/lib.rs" "true"
  assert_trigger "web-shell" "$WEB_SHELL_TRIGGER_PATTERN" "apps/openagents.com/service/src/lib.rs" "false"

  assert_trigger "cross-surface" "$CROSS_SURFACE_TRIGGER_PATTERN" "apps/autopilot-ios/Autopilot/AutopilotTests/AutopilotTests.swift" "true"
  assert_trigger "cross-surface" "$CROSS_SURFACE_TRIGGER_PATTERN" "apps/autopilot-desktop/src/main.rs" "true"
  assert_trigger "cross-surface" "$CROSS_SURFACE_TRIGGER_PATTERN" "scripts/local-ci.sh" "false"

  assert_trigger "ios-rust-core" "$IOS_RUST_CORE_TRIGGER_PATTERN" "apps/autopilot-ios/scripts/build-rust-client-core.sh" "true"
  assert_trigger "ios-rust-core" "$IOS_RUST_CORE_TRIGGER_PATTERN" "crates/openagents-client-core/src/ffi.rs" "true"
  assert_trigger "ios-rust-core" "$IOS_RUST_CORE_TRIGGER_PATTERN" "apps/runtime/src/main.rs" "false"

  assert_trigger "workspace-compile" "$RUST_WORKSPACE_COMPILE_TRIGGER_PATTERN" "apps/openagents.com/service/src/lib.rs" "true"
  assert_trigger "workspace-compile" "$RUST_WORKSPACE_COMPILE_TRIGGER_PATTERN" "crates/openagents-proto/src/lib.rs" "true"
  assert_trigger "workspace-compile" "$RUST_WORKSPACE_COMPILE_TRIGGER_PATTERN" "docs/README.md" "false"

  assert_trigger "runtime-codex-workers-php" "$RUNTIME_CODEX_WORKERS_PHP_TRIGGER_PATTERN" "apps/openagents.com/tests/Feature/Api/RuntimeCodexWorkersApiTest.php" "true"
  assert_trigger "runtime-codex-workers-php" "$RUNTIME_CODEX_WORKERS_PHP_TRIGGER_PATTERN" "apps/openagents.com/tests/Feature/ChatStreamingTest.php" "false"

  echo "local-ci trigger tests passed"
}

run_runtime_checks() {
  echo "==> runtime checks"
  (
    cd "$ROOT_DIR"
    cargo fmt --manifest-path apps/runtime/Cargo.toml -- --check
    cargo check --manifest-path apps/runtime/Cargo.toml
    cargo test --manifest-path apps/runtime/Cargo.toml
  )
}

run_runtime_history_checks() {
  echo "==> runtime history compatibility checks"
  (
    cd "$ROOT_DIR"
    cargo test -p openagents-runtime-service history_compat::tests
  )
}

run_legacy_comms_matrix() {
  echo "==> legacy comms security/replay matrix"
  (
    cd "$ROOT_DIR"
    ./scripts/comms-security-replay-matrix.sh all
  )
}

run_legacy_legacyparity_drift() {
  echo "==> legacy legacyparity drift strict gate"
  (
    cd "$ROOT_DIR"
    LEGACYPARITY_DRIFT_FAIL_ON_ACTIONABLE=1 ./scripts/legacyparity-drift-report.sh
  )
}

run_web_shell_checks() {
  echo "==> web-shell host boundary checks"
  (
    cd "$ROOT_DIR"
    ./apps/openagents.com/web-shell/check-host-shim.sh
    ./apps/openagents.com/web-shell/scripts/sw-policy-verify.sh
    ./apps/openagents.com/web-shell/scripts/perf-budget-gate.sh
  )
}

run_cross_surface_harness() {
  echo "==> cross-surface contract harness"
  (
    cd "$ROOT_DIR"
    ./scripts/run-cross-surface-contract-harness.sh
  )
}

run_ios_rust_core_checks() {
  echo "==> iOS rust client-core deterministic packaging checks"
  (
    cd "$ROOT_DIR"
    ./apps/autopilot-ios/scripts/build-rust-client-core.sh --clean
    cargo test -p openagents-client-core
  )
}

run_ios_rust_core_repro_checks() {
  echo "==> iOS rust client-core reproducibility checks"
  (
    cd "$ROOT_DIR"
    ./apps/autopilot-ios/scripts/verify-rust-client-core-reproducibility.sh
  )
}

run_runtime_codex_workers_php_tests() {
  echo "==> runtime codex workers php contract tests"
  (
    cd "$ROOT_DIR/apps/openagents.com"
    ./vendor/bin/pest tests/Feature/Api/RuntimeCodexWorkersApiTest.php
  )
}

run_workspace_compile() {
  echo "==> workspace rust compile baseline"
  (
    cd "$ROOT_DIR"
    cargo check --workspace --all-targets
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

run_all_rust() {
  run_workspace_compile
  run_runtime_checks
  run_proto_checks
  run_runtime_history_checks
  run_web_shell_checks
  run_ios_rust_core_checks
}

run_all() {
  run_all_rust
  if legacy_lanes_enabled; then
    run_legacy_comms_matrix
    run_legacy_legacyparity_drift
  else
    echo "==> skipping legacy compatibility lanes (set OA_LOCAL_CI_ENABLE_LEGACY=1 to run legacy-comms/legacy-legacyparity)"
  fi
}

run_changed() {
  local changed_files
  changed_files="$(collect_changed_files)"
  local legacy_lanes_detected=0

  if [[ -z "$changed_files" ]]; then
    echo "No changed files detected; nothing to run."
    return 0
  fi

  if has_match "$PROTO_TRIGGER_PATTERN" "$changed_files"; then
    run_proto_checks
  fi

  if has_match "$RUNTIME_HISTORY_TRIGGER_PATTERN" "$changed_files"; then
    run_runtime_history_checks
  fi

  if has_match "$RUNTIME_TRIGGER_PATTERN" "$changed_files"; then
    run_runtime_checks
  fi

  if has_match "$LEGACY_COMMS_TRIGGER_PATTERN" "$changed_files"; then
    if legacy_lanes_enabled; then
      run_legacy_comms_matrix
    else
      legacy_lanes_detected=1
    fi
  fi

  if has_match "$LEGACY_LEGACYPARITY_TRIGGER_PATTERN" "$changed_files"; then
    if legacy_lanes_enabled; then
      run_legacy_legacyparity_drift
    else
      legacy_lanes_detected=1
    fi
  fi

  if has_match "$WEB_SHELL_TRIGGER_PATTERN" "$changed_files"; then
    run_web_shell_checks
  fi

  if has_match "$CROSS_SURFACE_TRIGGER_PATTERN" "$changed_files"; then
    if is_truthy "${OA_LOCAL_CI_ENABLE_CROSS_SURFACE:-0}"; then
      run_cross_surface_harness
    else
      echo "==> cross-surface-triggered paths detected; harness skipped (set OA_LOCAL_CI_ENABLE_CROSS_SURFACE=1 to enable)"
    fi
  fi

  if has_match "$IOS_RUST_CORE_TRIGGER_PATTERN" "$changed_files"; then
    run_ios_rust_core_checks
    run_ios_rust_core_repro_checks
  fi

  if has_match "$RUNTIME_CODEX_WORKERS_PHP_TRIGGER_PATTERN" "$changed_files"; then
    run_runtime_codex_workers_php_tests
  fi

  if has_match "$RUST_WORKSPACE_COMPILE_TRIGGER_PATTERN" "$changed_files"; then
    run_workspace_compile
  fi

  if [[ "$legacy_lanes_detected" -eq 1 ]]; then
    echo "==> legacy-triggered paths detected; compatibility lanes skipped (set OA_LOCAL_CI_ENABLE_LEGACY=1 to enable)"
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
  runtime-history)
    run_runtime_history_checks
    ;;
  legacy-comms)
    run_legacy_comms_matrix
    ;;
  legacy-legacyparity)
    run_legacy_legacyparity_drift
    ;;
  web-shell)
    run_web_shell_checks
    ;;
  workspace-compile)
    run_workspace_compile
    ;;
  cross-surface)
    run_cross_surface_harness
    ;;
  ios-rust-core)
    run_ios_rust_core_checks
    run_ios_rust_core_repro_checks
    ;;
  runtime-codex-workers-php)
    run_runtime_codex_workers_php_tests
    ;;
  test-triggers)
    run_trigger_tests
    ;;
  all)
    run_all
    ;;
  all-rust)
    run_all_rust
    ;;
  comms)
    echo "lane 'comms' has been renamed to 'legacy-comms'" >&2
    run_legacy_comms_matrix
    ;;
  legacyparity)
    echo "lane 'legacyparity' has been renamed to 'legacy-legacyparity'" >&2
    run_legacy_legacyparity_drift
    ;;
  changed)
    run_changed
    ;;
  *)
    echo "Usage: scripts/local-ci.sh [changed|all|all-rust|docs|proto|runtime|runtime-history|legacy-comms|legacy-legacyparity|web-shell|workspace-compile|cross-surface|ios-rust-core|runtime-codex-workers-php|test-triggers]" >&2
    exit 2
    ;;
esac
