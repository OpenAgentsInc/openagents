#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-distributed-data-feed-semantics.sh

Runs the canonical Psionic distributed data-feed harness.
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

run_cmd cargo test -p psionic-data distributed_data_feed_plan_partitions_shards_without_overlap_and_with_stable_replay_order -- --nocapture
run_cmd cargo test -p psionic-data distributed_data_feed_semantics_report_tracks_partitioning_coordination_and_replay_cases -- --nocapture
run_cmd cargo test -p psionic-compat semantics_claim_report_marks_seeded_evidence_and_future_compatibility_targets -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic distributed data-feed semantics passed."
