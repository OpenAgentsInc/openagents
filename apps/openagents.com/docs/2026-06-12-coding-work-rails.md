# Coding-Agent Work Classes on the Pluralis Rails

Date: 2026-06-12
Issue of record: openagents#4861 (Pluralis adaptation tracker
openagents#4862, master tracking issue openagents#4855)
Rails consumed: #4854 (presence/compute receipt tiers, P2.3), #4848
(join-lifecycle ladder, P0.1), #4852 (reasoned device admission gates,
P1.4)
Roadmap source: `docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md`
(workspace `openagents` repo)
Promises referenced read-only: `autopilot.codex_probe_pylon_successor.v1`,
`pylon.local_claude_agent_bridge.v1`

## Claim discipline

Everything in `workers/api/src/pylon-coding-work-rails.ts` is a
contract definition. No live admission, payout, settlement, or promise
state change is claimed or made: the exported contract carries
`contractDefinitionOnly: true`, `liveAdmissionClaim: false`, and
`livePayoutClaim: false` in the payload itself, and the two bridge
promises above are cited as the lineage of the capability refs, not
mutated.

## Why coding work goes first

The Pluralis-campaign rails (#4855) were built for training devices,
but nothing in them is hardware-gated at the contract layer. Coding
-agent Pylons (`claude_agent_task` from the #4755/#4756 lineage,
`codex_agent_task` from the CX executor lane #4788-#4792) already have
live capability refs, readiness probes, bounded no-spend smokes, and
accepted closeouts — so they become the first work classes classified
onto the rails, giving coding-capable Pylons honest funnel positions
and priced availability without waiting for training hardware.

## The catalog (real refs only)

`CODING_WORK_CLASS_CATALOG` carries one entry per adapter, built from
the live contract constants rather than invented strings:

| Field | Claude lane | Codex lane |
| --- | --- | --- |
| `workClassRef` | `work_class.coding.claude_agent_task` | `work_class.coding.codex_agent_task` |
| `jobKind` (live `PylonApiAssignmentJobKind`) | `claude_agent_task` | `codex_agent_task` |
| `capabilityRef` (adapter-selection policy, CX5 #4792) | `capability.pylon.local_claude_agent` | `capability.pylon.local_codex` |
| `readinessSchemaRef` (Pylon bridge probes) | `openagents.pylon.claude_agent_readiness.v0.3` | `openagents.pylon.codex_agent_readiness.v0.3` |
| `promiseRef` | `pylon.local_claude_agent_bridge.v1` | `autopilot.codex_probe_pylon_successor.v1` |

The `capabilityRef` and `jobKind` values are imported from
`autopilot-work-adapter-selection.ts` (`adapterCapabilityRefs`,
`adapterJobKinds`), so the catalog cannot drift from the placement
policy without a type error.

## Rail 1: receipt tiers (reuses `classifyReceiptTier`)

`classifyCodingWorkReceiptTier` maps coding evidence kinds onto the
existing classifier's closed work-kind set and delegates — no parallel
tier policy:

| Coding evidence kind | Mapped work kind | Tier outcome |
| --- | --- | --- |
| `byok_readiness_probe` | `qualification_probe` | presence-tier (the probe IS the evidence) |
| `bounded_no_spend_smoke` | `shadow_window_work` | presence-tier by construction (warmup-state shadow work) |
| `accepted_closeout` | `verified_closeout` | compute-tier ONLY from an `active` ladder state with verification outcome refs; presence-tier (`presence_unmerged_work_not_active`) anywhere earlier |

An accepted closeout without verification outcome refs has no tier at
all — the existing refusal in `classifyReceiptTier` holds unchanged.

## Rail 2: join ladder (evidence projection, no new reason codes)

`codingJoinLifecycleStateForEvidence` claims only the rung the refs
prove, in the same spirit as `joinLifecycleStateForFunnel`:

- in the registry (an evidence record exists) -> `registered`
- readiness probe passed -> `qualified`
- bounded no-spend smoke verified -> `warmup`
- live accepted closeout -> `active`

This is a projection, not a transition: the reason-coded transition
machine in `pylon-join-lifecycle.ts` is untouched and its closed
taxonomy needed no new codes. `pylonCodingLadderProjection` renders the
funnel-ladder-block view — how many coding-capable Pylons sit at each
rung, overall and per coding capability ref — with bounded inputs, the
house privacy ref scan, and Pylons lacking a coding capability surfaced
honestly as `nonCodingPylonCount` rather than counted as rung zero.

## Rail 3: admission gates (definitions only)

`CODING_WORK_DEVICE_ADMISSION_GATE_SET` reuses
`DeviceAdmissionGateDefinition` and the `definitionsOnly: true` /
`liveAdmissionClaim: false` posture of the #4852 seeded set. Both
adapters execute inside the same Pylon host process, so the gates bind
to the shared `work_class.coding.local_coding_agent_session` shape:

| Gate | Requirement | Stated rationale |
| --- | --- | --- |
| `gate.device_admission.coding.node_or_bun_runtime_present.v1` | `node_or_bun_runtime_present` at_least 1 | both adapters load their SDKs lazily in the Pylon host process; no runtime, no session |
| `gate.device_admission.coding.workspace_write_sandbox_supported.v1` | `workspace_write_sandbox_supported` at_least 1 | coding assignments run under a workspace-write sandbox pinned to the bounded working directory (Codex lane: read-only anywhere narrows, never expands); a host that cannot enforce it is excluded, not run unsandboxed |
| `gate.device_admission.coding.sdk_session_host_ram_headroom_floor.v1` | `host_ram_headroom_gb` at_least 8 | an SDK session holds a conversation, workspace materialization, and verification run in host memory at once |

Two coding-host probe kinds (`node_or_bun_runtime_present`,
`workspace_write_sandbox_supported`) were added to the gate
measurement-kind union (`DeviceAdmissionGateMeasurementKinds` in
`training-device-admission-gates.ts`) without touching the CS336 A2
qualification payload: the a2 benchmark suite does not measure them,
so `Cs336A2QualificationProbeMeasurements` and the benchmark payload
are unchanged. Presence kinds measure 1 or 0 and gate `at_least 1`.
Live values are hardware-gated and arrive only as receipted evidence,
exactly like the #4852 host probes.

## Exported contract

`exportPylonCodingWorkRailsContract()` returns the versioned, deeply
frozen, JSON-able contract:

```jsonc
{
  "schemaVersion": "openagents.pylon.coding_work_rails.v1",
  "contractDefinitionOnly": true,
  "liveAdmissionClaim": false,
  "livePayoutClaim": false,
  "workClasses": [ /* CODING_WORK_CLASS_CATALOG */ ],
  "evidenceKinds": ["accepted_closeout", "bounded_no_spend_smoke", "byok_readiness_probe"],
  "admissionGates": { /* exportDeviceAdmissionGateContract(CODING_WORK_DEVICE_ADMISSION_GATE_SET) */ },
  "promiseRefs": ["autopilot.codex_probe_pylon_successor.v1", "pylon.local_claude_agent_bridge.v1"]
}
```

## Verified vs remaining

Landed and test-covered (`src/pylon-coding-work-rails.test.ts` plus
the three neighbor suites):

- the catalog carries the live adapter capability refs, job kinds,
  readiness schema refs, and promise refs — cross-checked against the
  adapter-selection constants in the tests,
- receipt-tier classification flows through the unchanged
  `classifyReceiptTier`, including the compute-tier-only-from-active
  rule and the unverified-closeout refusal,
- the ladder projection counts coding-capability rungs with bounded
  inputs and the privacy ref scan,
- every coding gate evaluates on both branches through the existing
  `evaluateDeviceAdmissionGate` with stated, measured reasons,
- the exported contract is frozen, JSON-round-trip stable, versioned,
  and marked definitions-only on every claim axis.

Remaining, **not** claimed here:

- any live coding-Pylon admission, exclusion, presence accrual, or
  compute settlement against these definitions,
- wiring the coding ladder projection into the live
  `/api/public/pylon-capacity-funnel` response (the funnel records do
  not yet carry per-Pylon capability refs at that boundary),
- live measurement of the coding-host probe kinds (runtime presence,
  sandbox support) — these arrive only as receipted device evidence,
- any promise state change for the two cited bridge promises.
