# `openagents.resource_usage_receipt.v1`

Status: implementation scaffold for `CND-050`

This contract records the resource and model-usage facts that Cloud can prove
for a managed workroom, benchmark task, or Codex run. It is a receipt, not a
billing invoice and not a public benchmark claim.

## Purpose

Every Cloud-managed run should produce a resource-and-usage receipt bundle
when practical. The receipt lets Vortex, Probe, and later Treasury distinguish:

- real host/device facts from modeled capacity;
- allocated or observed run resources from high-level status;
- provider-reported token usage from unavailable subscription-backed usage;
- nullable costs from measured infrastructure facts.

If a provider does not expose token counts, the receipt must say so explicitly.
Silent missing token usage is not acceptable proof.

## Receipt Fields

| Field | Purpose |
| --- | --- |
| `run_ref` | Assignment, task, or workroom run id. |
| `workroom_id` | Workroom scope for the run. |
| `node_ref` | Redacted node id such as `oa-shc-katy-01`. |
| `provider_lane` | `local`, `gcp`, `shc`, `provider`, or `unknown`. |
| `host` | OS, arch, CPU, RAM, disk, accelerator, KVM, Firecracker, cgroup, and container facts when discoverable. |
| `run` | Sandbox/profile digest, workspace digest, wall time, exit state, workspace bytes, artifact bytes, and log bytes. |
| `model_usage` | One or more model/provider usage records. |
| `compute_usage` | Optional infra compute metering + billing input (`compute_usage` sub-record from `openagents.compute_quota_routing.v1`). Present for managed cloud-lane (GCE) sessions; absent for control-plane/local paths that meter no VM-seconds. |
| `receipt_digest` | Local `sha256:` digest over the receipt material. |

## Compute Usage Sub-Record (GCE lane)

For a managed GCE per-session lease (`openagents.gce_capacity_class.v1`), the
control plane records a `compute_usage` sub-record with genuinely **measured**
VM-seconds and a cost-plus-10% billing input. VM-seconds are the lease wall-time
(`release_at − acquire_at`, whole seconds, saturating at 0) — a real measured
dimension, so `metering_source = node_measured`.

The rate is the GCP published list-price catalog rate
(`GCE_RAW_PER_VM_SEC_NANOUSD`), not a live GCP Billing export, so:

```text
vm_seconds:          <measured lease wall-clock seconds>
metering_source:     node_measured
cost_input_microusd: floor(vm_seconds × cost-plus-10% catalog rate)
cost_input_basis:    cost_plus_10pct_gcp_catalog
```

`cost_input_basis = cost_plus_10pct_gcp` is reserved for the deeper follow-up
where the rate comes from a live GCP Billing export. See
`openagents.compute_quota_routing.v1` for the basis enum and cost rules and
cloud#92 for the measured-VM-seconds-vs-catalog-rate split.

## Token Usage Records

Each `model_usage` record includes:

- provider and backend;
- model and mode;
- redacted account ref when relevant;
- input, cached input, output, reasoning, and total tokens when exposed;
- `count_source`: `provider_reported`, `codex_reported`,
  `parsed_from_stream`, `estimated`, or `unavailable`;
- nullable `cost_microusd`;
- billing basis;
- `unavailable_reason` when `count_source` is `unavailable`.

For ChatGPT/Codex subscription-backed workrooms, the current runner records:

```text
provider: openai
backend: codex
model: codex_subscription
mode: codex_exec
count_source: unavailable
billing_basis: chatgpt_subscription
unavailable_reason: subscription_backed_codex_no_token_counts
```

Do not estimate token counts or per-token cost for this path unless Codex
exposes a trustworthy count source later. Vortex should still show the explicit
unavailable receipt so proof and credit surfaces know the gap is declared.

## Runner Behavior

`oa-workroomd` writes resource receipts to:

```text
resource-usage-receipts.jsonl
```

`runners/py-bench-runner` writes the same receipt family into each benchmark
artifact bundle as:

```text
resource_usage_receipt.json
```

and includes the digest as `resourceUsageReceiptDigest` in `proof_bundle.json`.

The Codex one-shot and session runners emit:

- an `openagents.codex_workroom_event.v1` `receipt` event citing the receipt
  digest;
- an `openagents.runner_event.v1` `resource.usage.captured` event;
- an `openagents.runner_event.v1` `turn.completed` or
  `ThreadTokenUsageUpdated` event when Codex exposes token usage for the turn;
- an `openagents.runner_event.v1` `opencode.step-finish` or
  `opencode.session.next.step.ended` event when OpenCode exposes token usage
  for the step/turn;
- an `openagents.runner_event.v1` `usage.unavailable` event only when no
  token-usage payload was observed, with the same receipt digest.

Artifact and closeout receipts remain separate. This receipt only records
resource and model-usage facts.

## Validation Rules

- Receipt refs must be bounded non-secret strings.
- Digests must be `sha256:` references.
- Host facts must not contain raw tokens, secrets, private keys, wallet
  material, or private topology markers.
- `model_usage` must contain at least one record.
- `count_source = unavailable` requires `unavailable_reason`.
- Costs are nullable; do not invent cost when the provider only supplies a
  subscription plan.
