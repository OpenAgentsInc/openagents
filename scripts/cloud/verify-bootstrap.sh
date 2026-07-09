#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

required_paths=(
  "AGENTS.md"
  "INVARIANTS.md"
  "README.md"
  "docs/ARCHITECTURE.md"
  "docs/BENCHMARK_CLOUD.md"
  "docs/ISSUES.md"
  "docs/bootstrap/CND-032-container-packaging.md"
  "docs/bootstrap/CND-033-gcp-node-bootstrap.md"
  "docs/bootstrap/CND-034-gce-node-deploy.md"
  "docs/bootstrap/CND-035-gcp-workroom-smoke.md"
  "docs/bootstrap/CND-038-redacted-config-env.md"
  "docs/bootstrap/CND-039-mvp-observability.md"
  "docs/bootstrap/CND-045-gcp-benchmark-cloud-substrate.md"
  "docs/bootstrap/CND-046-cloud-batch-benchmark-backend.md"
  "docs/bootstrap/CND-047-terminal-bench-harbor-wrapper.md"
  "docs/bootstrap/CND-048-openagents-codex-benchmark-adapter.md"
  "docs/bootstrap/CND-049-swe-custom-repo-benchmark-adapter.md"
  "docs/bootstrap/CND-054-coding-agent-benchmark-improvement.md"
  "docs/bootstrap/CND-055-artanis-pylon-bootstrap.md"
  "docs/control/CODEX_CONTROL_API.md"
  "docs/contracts/openagents.artanis_bootstrap_assignment.v1.md"
  "docs/contracts/openagents.cloud_node.v1.md"
  "docs/contracts/openagents.codex_auth_grant.v1.md"
  "docs/contracts/openagents.codex_workroom_assignment.v1.md"
  "docs/contracts/openagents.forge_assignment.v1.md"
  "docs/contracts/openagents.psionic_worker_attachment.v1.md"
  "docs/contracts/openagents.probe_worker_attachment.v1.md"
  "docs/contracts/openagents.workroom.v1.md"
  "Cargo.toml"
  "crates/oa-codex-control/Cargo.toml"
  "crates/oa-codex-control/src/main.rs"
  "crates/oa-node/Cargo.toml"
  "crates/oa-node/src/main.rs"
  "docs/oa-node/CAPABILITY_BROKER_REDACTION.md"
  "docs/oa-node/QUARANTINE.md"
  "docs/oa-node/SERVICE_MANAGER.md"
  "docs/oa-node/SANDBOX_PROFILE_ENFORCEMENT.md"
  "docs/oa-node/SETTLEMENT_MODES.md"
  "docs/oa-node/SIGNED_UPDATES.md"
  "crates/oa-workroomd/Cargo.toml"
  "crates/oa-workroomd/src/main.rs"
  "docs/oa-workroomd/ARTIFACT_CLOSEOUT.md"
  "docs/oa-workroomd/CODEX_AUTH_GRANTS.md"
  "docs/oa-workroomd/CODEX_WORKROOM_RUNNER.md"
  "docs/oa-workroomd/LINK_LOCAL_GATEWAYS.md"
  "docs/oa-workroomd/MANAGED_PREVIEW_INGRESS.md"
  "docs/oa-workroomd/METADATA_ENDPOINT.md"
  "docs/oa-workroomd/WORKROOM_LIFECYCLE.md"
  "crates/openagents-cloud-contract/Cargo.toml"
  "crates/openagents-cloud-contract/src/lib.rs"
  "fixtures/cloud/artanis_bootstrap_assignment_v1/pylon-launch-bootstrap.json"
  "runners/py-bench-runner/openagents_bench/evaluate_signatures.py"
  "scripts/gcp-benchmark-bootstrap.sh"
  "scripts/gcp-benchmark-cleanup.sh"
  "scripts/gcp-benchmark-smoke.sh"
  "scripts/gcp-benchmark-submit-batch.sh"
  "scripts/build-cloud-images.sh"
  "scripts/gcp-node-bootstrap.sh"
  "scripts/gcp-node-cleanup.sh"
  "scripts/gcp-node-deploy-vm.sh"
  "scripts/gcp-node-destroy-vm.sh"
  "scripts/gcp-node-status.sh"
  "scripts/gcp-node-workroom-smoke.sh"
  "scripts/collect-mvp-observability.sh"
  "scripts/verify-redacted-config.sh"
  "config/oa-node.env.example"
  "config/oa-workroomd.env.example"
  "config/gcp-node.env.example"
  "docker/oa-node.Dockerfile"
  "docker/oa-workroomd.Dockerfile"
)

for path in "${required_paths[@]}"; do
  if [[ ! -e "$path" ]]; then
    echo "missing required bootstrap path: $path" >&2
    exit 1
  fi
done

cargo check
cargo test -p openagents-cloud-contract
bash -n \
  scripts/build-cloud-images.sh \
  scripts/gcp-node-bootstrap.sh \
  scripts/gcp-node-cleanup.sh \
  scripts/gcp-node-deploy-vm.sh \
  scripts/gcp-node-destroy-vm.sh \
  scripts/gcp-node-status.sh \
  scripts/gcp-node-workroom-smoke.sh \
  scripts/collect-mvp-observability.sh \
  scripts/verify-redacted-config.sh \
  scripts/gcp-benchmark-bootstrap.sh \
  scripts/gcp-benchmark-cleanup.sh \
  scripts/gcp-benchmark-smoke.sh \
  scripts/gcp-benchmark-submit-batch.sh

node_status="$(mktemp)"
workroom_status="$(mktemp)"
trap 'rm -f "$node_status" "$workroom_status"' EXIT

cargo run -p oa-node -- --help >/dev/null
cargo run -p oa-codex-control -- --help >/dev/null
cargo run -p oa-workroomd -- --help >/dev/null
cargo test -p oa-codex-control artanis_bootstrap >/dev/null
cargo run -p oa-node -- status --json >"$node_status"
cargo run -p oa-workroomd -- status --json >"$workroom_status"
scripts/verify-redacted-config.sh
observability_dir="$(mktemp -d)"
scripts/collect-mvp-observability.sh --local --output-dir "$observability_dir" >/dev/null
rm -rf "$observability_dir"
(
  cd runners/py-bench-runner
  python3 -m openagents_bench.evaluate_signatures --fixture-dir fixtures/cloud/signature-routing >/dev/null
)

grep -q '"contract_version": "openagents.cloud_node.v1"' "$node_status"
grep -q '"contract_version": "openagents.workroom.v1"' "$workroom_status"

if [[ "$(uname -s)" == "Linux" ]] && command -v codex >/dev/null 2>&1; then
  command -v bwrap >/dev/null 2>&1 || {
    echo "missing bubblewrap required by Codex Linux sandbox diagnostics" >&2
    exit 1
  }
  codex exec --help | grep -q -- "--skip-git-repo-check"
fi

echo "cloud bootstrap verification passed"
