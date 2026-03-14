#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

invocation="$tmpdir/invocation.json"
result="$tmpdir/result.json"

cat >"$invocation" <<'JSON'
{
  "run_id": "run-check",
  "started_at_ms": 1000,
  "spec": {
    "schema_version": 1,
    "experiment_id": "exp.serve.check",
    "candidate_id": "candidate-check",
    "family": {
      "family": "serving_scheduler",
      "model_id": "gpt-oss-20b",
      "benchmark_suite_ref": "benchmark://serve/local-weather",
      "policy": {
        "max_batch_tokens": 8192,
        "max_active_sequences": 8,
        "prefill_share_bps": 4500,
        "decode_share_bps": 5500,
        "queue_slack_ms": 25
      }
    },
    "base_artifacts": [
      {
        "kind": "served_artifact",
        "reference": "served://gpt-oss-20b",
        "digest": "served-digest-check"
      }
    ],
    "mutation": {
      "mutation_id": "mutation-check",
      "parent_candidate_id": "baseline",
      "family": "serving_scheduler",
      "changed_surfaces": [
        "serve.scheduler.prefill_share_bps"
      ],
      "mutation_digest": "b4fd33181ee3b18c83b82308fe8fbf597963138d862c3a0d0b6b8424eb6c749a"
    },
    "runtime_profile": {
      "runner_binary_digest": "runner-digest-check",
      "sandbox_profile_ref": "sandbox://research/local",
      "requested_backend": "cuda",
      "requested_visible_devices": []
    },
    "budget": {
      "max_wall_time_ms": 30000,
      "max_steps": 2000,
      "max_samples": 32,
      "output_root": "runs/serve.check"
    },
    "score_contract": {
      "contract_id": "serve.score.v1",
      "family": "serving_scheduler",
      "metrics": [
        {
          "metric_id": "p95_latency_ms",
          "unit": "milliseconds",
          "direction": "minimize",
          "weight_bps": 3000,
          "required": true,
          "hard_gate": {
            "comparison": "at_most",
            "value_micros": 65000
          }
        },
        {
          "metric_id": "throughput_tokens_per_second",
          "unit": "tokens_per_second",
          "direction": "maximize",
          "weight_bps": 7000,
          "required": true,
          "hard_gate": {
            "comparison": "at_least",
            "value_micros": 150000000
          }
        }
      ],
      "contract_digest": "df4a141de4a849d168c9fdcb7836312c9f01703437091cdbff8e4631d86a74f1"
    },
    "spec_digest": "163f2a8ffbd3f37ad88ddd4fd8ad1e6b5fe4d7748094f8bb023dd4e11bf777cb"
  }
}
JSON

cd "$repo_root"

echo "==> cargo test -p psionic-research --lib -- --nocapture"
cargo test -p psionic-research --lib -- --nocapture

echo
echo "==> cargo run -p psionic-research --bin psionic-research-runner -- --invocation $invocation --result $result"
cargo run -p psionic-research --bin psionic-research-runner -- --invocation "$invocation" --result "$result"

echo
echo "==> verify typed outputs"
rg -n '"status": "succeeded"' "$result"
rg -n '"family": "serving_scheduler"' "$result"
test -f "${result%.json}.stdout.log"
test -f "${result%.json}.stderr.log"

echo
echo "Psionic research runner passed."
