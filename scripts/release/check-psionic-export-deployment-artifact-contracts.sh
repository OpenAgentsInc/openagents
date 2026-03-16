#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-export-deployment-artifact-contracts.sh

Runs the canonical Psionic export/deployment artifact harness.
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

run_cmd() {
  echo
  echo "==> $*"
  "$@"
}

run_cmd cargo test -p psionic-ir exportable_graph_contract_tracks_entry_signature_and_refuses_opaque_graphs -- --nocapture
run_cmd cargo test -p psionic-compiler deployment_artifact_contract_tracks_export_graph_digest_and_topology_attachment -- --nocapture
run_cmd cargo test -p psionic-compiler export_deployment_artifact_semantics_report_tracks_seeded_supported_and_refused_cases -- --nocapture
run_cmd cargo test -p psionic-compat semantics_claim_report_marks_seeded_evidence_and_future_compatibility_targets -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic export and deployment artifact contracts passed."
