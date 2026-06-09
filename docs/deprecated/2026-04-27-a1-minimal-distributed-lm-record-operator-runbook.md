# A1 Minimal Distributed LM Participant-Record Operator Runbook

Status: active  
Date: 2026-04-27  
Owner repo: `openagents`

This runbook defines the operator gates for
`a1_minimal_distributed_lm_001`, the first A1-derived distributed language
model run intended to support a public participant-count claim.

The target claim is:

```text
OpenAgents ran what we believe is the world's largest distributed language-model training run by number of participants: N distinct Pylons contributed real compute through Psionic and completed accepted work for the same run, with run/window/checkpoint lineage published publicly.
```

The stronger model-progress claim is:

```text
OpenAgents ran what we believe is the world's largest distributed language-model training run by number of model-progress participants: N distinct Pylons contributed accepted local-update work that advanced promoted checkpoint X for run Y.
```

The phrase "by number of participants" is allowed only when "participant"
means accepted real compute work under one run id. It must never be inferred
from online Pylons, seen-in-24h Pylons, sellable Pylons, generic payout totals,
Discord members, downloads, or app sessions.

## Claim Contract

Run id:

- `a1_minimal_distributed_lm_001`

Record bar:

- public target: `201+` participants
- internal source of truth for `Participants`:
  `training_accepted_contributors`
- internal source of truth for `Model-progress participants`:
  `training_model_progress_contributors`

Required run evidence:

- fixed tokenizer digest
- fixed tokenized dataset digest
- model config
- optimizer config
- validation set digest
- run-definition ref
- assignment/window/checkpoint lineage
- closeout receipts accepted by Nexus
- payout projection or payout ledger rows for accepted work
- public run detail and claim-gate JSON

The guarded fanout/control-plane closure receipt for `#4456` is:

```text
docs/reports/nexus/20260427-issue-4456-closure.md
```

That receipt closes the implementation issue. It does not waive the live
launch-health gates below.

## Local Proof Before Production

Run these from the `openagents` repo before any production canary or beta:

```bash
cargo test -p nexus-control a1_minimal --lib
cargo test -p nexus-control a1_minimal_claim_gate --lib
cargo test -p nexus-control kernel_compute_training_run_definition_route_projects_a1_minimal_fixed_refs --lib
cargo test -p nexus-control kernel_compute_training_artifact_resolver_route_returns_logical_contract --lib
cargo test -p nexus-control kernel_compute_training_artifact_signed_access_route_issues_google_v4_urls --lib
```

Stop if any command fails. Do not substitute a demo-only run for this lane.

## Production Verification Gates

Baseline Nexus deploy verification remains:

```bash
scripts/deploy/nexus/04-verify-gates.sh
```

For the A1 minimal participant-record attempt, enable the run-specific gates:

```bash
VERIFY_A1_MINIMAL_RECORD_GATES_ENABLED=true \
VERIFY_A1_MINIMAL_RUN_ID=a1_minimal_distributed_lm_001 \
scripts/deploy/nexus/04-verify-gates.sh
```

Before making the participant-count public claim, require the participant gate:

```bash
VERIFY_A1_MINIMAL_RECORD_GATES_ENABLED=true \
VERIFY_A1_MINIMAL_REQUIRE_LAUNCH_HEALTH_GOOD=true \
VERIFY_A1_MINIMAL_REQUIRE_PARTICIPANT_GATE=true \
VERIFY_A1_MINIMAL_RUN_ID=a1_minimal_distributed_lm_001 \
scripts/deploy/nexus/04-verify-gates.sh
```

Before making the model-progress participant claim, require both gates:

```bash
VERIFY_A1_MINIMAL_RECORD_GATES_ENABLED=true \
VERIFY_A1_MINIMAL_REQUIRE_LAUNCH_HEALTH_GOOD=true \
VERIFY_A1_MINIMAL_REQUIRE_PARTICIPANT_GATE=true \
VERIFY_A1_MINIMAL_REQUIRE_MODEL_PROGRESS_GATE=true \
VERIFY_A1_MINIMAL_RUN_ID=a1_minimal_distributed_lm_001 \
scripts/deploy/nexus/04-verify-gates.sh
```

The A1 record mode writes these extra fields into the deploy receipt:

- `a1_minimal_record_gate_policy`
- `training_summary`
- `a1_minimal_claim_gates`
- `endpoint_latency_ms.training_summary`
- `endpoint_latency_ms.a1_minimal_claim_gates`
- `gates[].gate_id == "a1_minimal_claim_gate_endpoint"`
- `gates[].gate_id == "a1_minimal_updated_admitted_workers"`
- `gates[].gate_id == "a1_minimal_launch_health"`
- `gates[].gate_id == "a1_minimal_participant_claim_gate"`
- `gates[].gate_id == "a1_minimal_model_progress_participant_claim_gate"`

## Exact Endpoint Checks

Set the base URL without printing secrets:

```bash
NEXUS_BASE_URL="${NEXUS_BASE_URL:-https://nexus.openagents.com}"
RUN_ID="a1_minimal_distributed_lm_001"
```

Health and public state:

```bash
curl -fsS "$NEXUS_BASE_URL/healthz" | jq .
curl -fsS "$NEXUS_BASE_URL/api/stats" | jq '.training_public_state'
curl -fsS "$NEXUS_BASE_URL/api/training/summary" | jq '.runs[] | select(.training_run_id == "'"$RUN_ID"'")'
curl -fsS "$NEXUS_BASE_URL/api/training/rollout" | jq .
curl -fsS "$NEXUS_BASE_URL/v1/treasury/status" | jq .
```

Run detail and claim gate:

```bash
curl -fsS "$NEXUS_BASE_URL/api/training/runs/$RUN_ID?refresh=true" | jq .
curl -fsS "$NEXUS_BASE_URL/api/training/runs/$RUN_ID/claim-gates" | jq .
```

The participant claim is allowed only when:

```bash
curl -fsS "$NEXUS_BASE_URL/api/training/runs/$RUN_ID/claim-gates" \
  | jq -e '
      .unqualified_largest_claim_allowed == false
      and .participant_gate.passed == true
      and .participant_gate.internal_source_of_truth == "training_accepted_contributors"
      and .participant_gate.evidence.real_compute_accepted_participants >= 201
    '
```

The model-progress participant claim is allowed only when:

```bash
curl -fsS "$NEXUS_BASE_URL/api/training/runs/$RUN_ID/claim-gates" \
  | jq -e '
      .unqualified_largest_claim_allowed == false
      and .participant_gate.passed == true
      and .model_progress_participant_gate.passed == true
      and .model_progress_participant_gate.internal_source_of_truth == "training_model_progress_contributors"
      and .model_progress_participant_gate.evidence.real_compute_model_progress_participants >= 201
      and (.model_progress_participant_gate.evidence.latest_promoted_checkpoint_ref // "") != ""
    '
```

Artifact resolver and signed access use the versioned kernel routes:

```bash
ARTIFACT_ID="..."
curl -fsS "$NEXUS_BASE_URL/v1/kernel/compute/training/artifacts/$ARTIFACT_ID" | jq .
curl -fsS -X POST \
  -H 'content-type: application/json' \
  --data '{"purpose":"download","requested_ttl_seconds":3600}' \
  "$NEXUS_BASE_URL/v1/kernel/compute/training/artifacts/$ARTIFACT_ID/signed-access" \
  | jq .
```

## Canary, Beta, Broad Fanout

Canary is allowed only when:

- local proof tests pass;
- `scripts/deploy/nexus/04-verify-gates.sh` passes;
- `VERIFY_A1_MINIMAL_RECORD_GATES_ENABLED=true` passes for the run;
- `/api/training/rollout` is not paused for the target cohort;
- the target release id and build digest are not blocked;
- at least two updated admitted workers exist for the run before any
  multi-Pylon language is used;
- artifact resolver and signed-access samples are present and inside budget;
- validator closeout can accept at least one support/verifier unit and one
  local-update unit.

Beta is allowed only when all canary gates remain true and:

- rolling assignment success stays within the SLO package;
- retained assignment/materialization receipts show no systemic artifact-fetch
  failure;
- validator backlog clears inside budget;
- accepted-work payout ledger rows are projected or reconciled for accepted
  work;
- public stats are fresh and drift-free.

Broad fanout is allowed only when all beta gates remain true and:

- `training_public_state.launch_health.overall_status == "good"`;
- `VERIFY_A1_MINIMAL_REQUIRE_LAUNCH_HEALTH_GOOD=true` passes in the deploy
  verifier;
- there are no `run_backlog`, `validator_backlog`, `payout_lag`,
  `resolver_latency`, `signed_access_latency`, `stale_snapshot`, or
  `public_state_drift` alerts;
- `VERIFY_A1_MINIMAL_REQUIRE_PARTICIPANT_GATE=true` passes before Launch A;
- `VERIFY_A1_MINIMAL_REQUIRE_MODEL_PROGRESS_GATE=true` passes before Launch B;
- no issue remains open that blocks real assignment, artifact materialization,
  validation, payout, or public stats freshness for this run.

## Pause and Rollback Criteria

Pause new leases immediately when any of these happen:

- `/healthz`, `/api/stats`, `/api/training/rollout`, or `/v1/treasury/status`
  fails or breaches latency gates;
- public Nexus returns Cloudflare `530` or `1033`;
- `/api/training/rollout` reports `pause_new_leases=true`;
- artifact resolver or signed-access p95 exceeds the frozen SLO;
- retained receipts show repeated assignment materialization failures;
- validator closeout backlog does not clear inside budget;
- accepted-work payouts enter `attention_required`, fail, or skip for an
  accepted-work reason;
- public stats age exceeds the freshness budget;
- claim-gate JSON is missing, stale, or permits unqualified "largest" language;
- an operator or website surface starts using online/presence/session/payout
  counters as participant-count evidence.

Rollback the production release rather than widening cohorts when the failing
gate began after a deploy and the previous Nexus release had clean receipts.
Retain the failed deploy receipt and the run-specific claim-gate JSON before
rolling back.

## Evidence Retention

Retain these files or endpoint captures for Launch A:

- `docs/reports/nexus/<stamp>-deploy-receipt.json`
- local proof command output
- `/api/stats`
- `/api/training/summary`
- `/api/training/rollout`
- `/api/training/runs/a1_minimal_distributed_lm_001?refresh=true`
- `/api/training/runs/a1_minimal_distributed_lm_001/claim-gates`
- redacted `/v1/treasury/status`
- sample artifact resolver and signed-access responses
- accepted closeout receipts for support/verifier and local-update work
- payout projection or ledger rows for accepted work

Retain these additional files or endpoint captures for Launch B:

- aggregate checkpoint receipt
- promoted checkpoint ref
- validation loss before and after promotion
- local-update contribution receipts for all model-progress participants
- promotion receipt digest or equivalent closeout artifact

## Model-Scale Caveat

This lane does not claim largest by model size, token budget, total FLOPs,
model quality, or permissionless model-progress training. The allowed public
claim is largest by number of participants, where participants means distinct
Pylons/providers with accepted real compute work under one run id.
