# `openagents.codex_placement_assignment.v1`

Status: implementation scaffold for cloud#86 / cloud#88

This contract is the lane-agnostic placement input for managed coding runs. It
lets a generic, Vortex-codebase-independent control front door (Pylon) ask
Cloud to place one bounded coding run on a managed runner without hard-wiring a
specific node. It is the coordinator/placement layer described in Leg B / Phase
1 of the "code on the go" plan.

It is paired with `openagents.compute_quota_routing.v1` for caps and metering,
and with `openagents.gce_capacity_class.v1` for the GCE ephemeral VM lane.

## Placement Assignment Fields

| Field | Purpose |
| --- | --- |
| `contract_version` | Must be `openagents.codex_placement_assignment.v1`. |
| `run_id` | Stable run id assigned by the caller. |
| `owner_ref` | Redacted owner/org ref used for per-owner quota evaluation. |
| `provider_account_ref` | Sanitized ChatGPT/Codex provider-account ref. |
| `auth_grant_ref` | Session grant ref from `openagents.codex_auth_grant.v1`. |
| `goal` | Bounded instruction for the coding run. |
| `lane` | `auto` (default), `local`, `cloud-gcp`, or `cloud-shc`. |
| `repository` | Optional non-secret repo/project context. |
| `sandbox_mode` | Optional; defaults to `danger_full_access` inside the no-wallet VM boundary. |
| `wallet_authority` | Must be `false`. |
| `created_at_ms` | Caller timestamp; must not be from the future. |

## Lane Policy

Owner direction (2026-06-14): the cloud lane priority is **Google GCE first,
SHC second.**

- `auto` — cost-driven (CND-042). Own-Pylon-first-and-free is resolved upstream
  by the caller; at the cloud placement endpoint `auto` compares the eligible
  cloud lanes on measured cost-plus-10% and binds the cheaper lane, with Google
  GCE winning ties/near-ties and SHC chosen only when materially cheaper AND the
  pilot recommendation is "expand". When GCE is unavailable it falls back to SHC
  (`oa-shc-katy-01`); when cost-driven placement is disabled it binds the GCE
  ephemeral per-session VM lane (`gce.ephemeral.standard.v1`) by policy default.
- `cloud-gcp` — pin GCE; falls back to SHC only if GCE is unavailable.
- `cloud-shc` — pin SHC (`oa-shc-katy-01`).
- `local` — resolved by the caller's own Pylon; rejected by cloud placement.

Placement is **cost-driven (CND-042).** See the cost split and lane rates in
`docs/contracts/openagents.compute_quota_routing.v1.md` ("Cost-Driven
Placement") and the comparison report
`docs/benchmarks/2026-06-14-cnd-042-gce-shc-receipt-comparison.md`. A cost-driven
binding records `cost_driven = true` and a refs-only `cost_basis`. Policy-driven
fallback, caller pins, and the disabled-cost-driven path record
`cost_driven = false` with no `cost_basis`.

## Runner Binding

The endpoint returns an `openagents.codex_placement_assignment.v1` runner
binding:

| Field | Purpose |
| --- | --- |
| `run_id` | The placed run id. |
| `external_run_id` | `shc-codex:<runner_id>:<run_id>`. |
| `lane` | Resolved lane (`cloud-gcp` or `cloud-shc`). |
| `provider_lane` | `gcp` or `shc`. |
| `runner_id` | Bound runner id (ephemeral GCE label, or SHC node id). |
| `capacity_class_id` | `gce.ephemeral.standard.v1` for GCE; null for SHC. |
| `sandbox_mode` | Sandbox profile; `danger_full_access` default. |
| `reason` | `lane_pinned`, `policy_default_gce`, `gce_unavailable_shc_fallback`, or `cost_driven`. |
| `cost_driven` | `true` when the lane was chosen by the CND-042 cost comparison; `false` otherwise. |
| `cost_basis` | Refs-only `PlacementCostBasis` (lane cost-plus-10% estimates + modeled per-session figure) when `cost_driven`; omitted otherwise. Never raw customer cost. |
| `caps` | Session/idle/lease/pause TTLs and per-owner caps from quota routing. |

## Quota Caps

Caps default to `openagents.compute_quota_routing.v1`:

- `session_ttl_ms` = 8h, `idle_timeout_ms` = 30m, `lease_ttl_ms` = 12h,
  `pause_ttl_ms` = 2h;
- per-owner active-session cap = 4, per-owner remote-lease cap = 2.

Fleet policy may override these; placement must honor whatever caps it binds
rather than minting unbounded leases.

## GCE Default Owner-Session Lane (cloud#88)

The GCE ephemeral-per-session VM lane is the default owner-session runner. The
lane path is: provision (`openagents.gce_capacity_class.v1`) ->
`oa-workroomd` Codex runner -> artifact closeout ->
`openagents.resource_usage_receipt.v1` -> cleanup. `danger_full_access` is used
inside the no-wallet VM boundary (CND-041/CND-055).

Full warm-pool/density optimization is **deferred to the density phase**. The
current scaffold selects GCE by default and keeps cold-start reasonable; it does
not yet maintain a pre-warmed pool.

## Validation Rules

- `contract_version` must be `openagents.codex_placement_assignment.v1`.
- `wallet_authority` must be `false`.
- `run_id`, `owner_ref`, `provider_account_ref`, `auth_grant_ref`, and `goal`
  are bounded non-secret strings.
- `created_at_ms` must not be in the future.
- `local` lane is not cloud-placeable and is rejected at the endpoint.
- Bindings and caps are refs-and-limits only: no raw owner identity, cost,
  GCP project id, instance name, IP, credentials, wallet material, bearer
  tokens, or private topology markers.

See `docs/control/CODEX_CONTROL_API.md`,
`docs/contracts/openagents.compute_quota_routing.v1.md`, and
`docs/contracts/openagents.gce_capacity_class.v1.md`.
