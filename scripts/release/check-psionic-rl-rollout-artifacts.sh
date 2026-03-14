#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-rl-rollout-artifacts.sh

Runs the canonical rollout-artifact and trainer-batch harness for #3565.
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

run_cmd cargo test -p psionic-train --lib rollout_artifacts_and_trainer_batch_are_machine_legible -- --nocapture
run_cmd cargo test -p psionic-train --lib trainer_batch_refuses_cross_family_rollouts -- --nocapture
run_cmd cargo test -p psionic-train --lib rollout_artifact_requires_samples -- --nocapture
run_cmd cargo test -p psionic-train --lib -- --nocapture

echo
echo "Psionic rollout artifact contracts passed."
