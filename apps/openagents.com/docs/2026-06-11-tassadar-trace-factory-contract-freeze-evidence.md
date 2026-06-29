# Tassadar Verified Trace Factory — Day-0 Contract Freeze Evidence

Date: 2026-06-11
Issue: openagents #4748 (RESEARCH_PLAN.md W2, step 1)
Branch: `blocker-waves`, commit `4b957294c`
Spend: none — everything is local generation plus verification.

This document records the four frozen contracts, the local 1–5M-token
pilot corpus, the replay-from-clean-checkout proof, and the honest
acceptance numbers. Claim boundary unchanged: faithful re-execution of
digest-pinned compiled workloads only — no softmax, no learning, no
serving, no performance claim against conventional CPUs. The promise's
unsafeCopy governs all public copy about this lane.

## 1. The four frozen contracts (committed and versioned)

All under `apps/openagents.com/workers/api/src/tassadar-trace-factory/`,
each with vitest coverage (45 tests total, all passing).

### 1.1 `trace_record` v0.1 — `trace-record.ts`

- Version ids: `trace_record.v0.1`, token encoding `trace_token.v0.1`,
  profile `profile.tassadar_alm_numeric.v1`, binary container `TTRC`
  format 1.
- Fields exactly as scoped: `profileVersion`, `programHash` (model
  graph digest), `inputSeed` (u64 hex; with `familyId` it regenerates
  the workload), `compilerHash`, `executorHash`, compact
  `traceTokenIds` (uint16 or uint32), `stepOffsets`, 
  `finalOutputDigest`, `fullTraceDigest`, `validatorReceipts[]`.
- Token encoding: each executor step's i64 output row becomes
  little-endian uint16 limbs (4 per value; 2 per value at uint32
  width). The limb byte stream is byte-identical to the stream the Rust
  and TS executors hash, so the full trace digest is recomputable from
  the token stream alone (Tier 0) and from independent re-execution
  (Tier 1). The pinned test reproduces the committed fixture's
  Rust-parity digest
  `f2995c4e3c959b42bb1e4afbefffbcf7ba6104099621ccc0ac912862dc932a5b`
  from tokens.
- Compact binary in the hot path: exactly 2 bytes per uint16 token
  (1B tokens ≈ 2 GB); human-readable traces are sampled audit artifacts
  only. Every decode failure is typed (`bad_magic`, `truncated`,
  `unsupported_format_version`, `invalid_token_width`,
  `invalid_step_offsets`, `invalid_digest_field`, `invalid_receipt`).

### 1.2 Validator verdict v0.1 + tier ladder — `validation-policy.ts`

- Version ids: `validator_verdict.v0.1`, class
  `exact_trace_replay.trace_factory.v0_1`.
- The frozen ladder (`TASSADAR_VALIDATION_TIER_LADDER`):
  - Tier 0 `schema_hash` — every record; no re-execution; recomputes
    full-trace and final-output digests from the token stream.
  - Tier 1 `full_replay` — new workers/profiles/families; independent
    re-execution and token-level comparison.
  - Tier 2 `window_spot_check` — reputation-sampled.
  - Tier 3 `adversarial_replay` — random, deterministic seed-derived
    selection published only after generation closes.
- Quarantine-before-admission: `requiredAdmissionTier` returns Tier 1
  for any new worker/profile/family; `admissionDecision` keeps records
  quarantined until a verified receipt at the required tier exists.
- Iron rules as enforced invariants, not prose:
  - `trainingEligibility` — never train from unverified artifacts. A
    record without a verified Tier 0 receipt AND a verified full-replay
    receipt (Tier 1 or 3) is ineligible; there is no override
    parameter.
  - `generationAssignmentDigestViolations` — expected digests never
    ship in generation assignments; recursive typed scan for
    digest-bearing keys, asserted by the generation script on every
    assignment payload it would dispatch.

### 1.3 Training split policy v0.1 — `training-split-policy.ts`

- Version id: `training_split.v0.1`; split unit `program_family`.
- Held-out FAMILIES (never seeds): `family.application_state_machine.v1`
  (the economic ledger family — the demand shape) and
  `family.stack_loop_sum.compiled.v1` (the psionic-compiled anchor).
- Train-short / evaluate-long: train records ≤ 512 steps; eval factors
  2×/4×/8× (1024/2048/4096 step bound).
- Stress suites: `stress.branch.v1` (branch_gated_control),
  `stress.memory.v1` (memory_load_store).
- Adversaries: `family.near_miss_lookup.v1`, eval-only.
- `splitPolicyViolations` rejects policies that double-assign or
  orphan a family or fail to hold out the economic family.

### 1.4 Projection-rebuild rules v0.1 — `projection-rebuild.ts`

- Version id: `projection_rebuild.v0.1`. Case law: #4744, #4745,
  #4746 — four frozen-projection incidents in 24 hours.
- The rule as a type: `TassadarProjectionRebuildTrigger` admits only
  `'validation_transition'`; registration-triggered public rebuilds are
  unrepresentable in a compliant module declaration.
- The rule at runtime: `projectionRebuildCompliance` replays a
  registration-only log and a transition log through any module and
  emits typed violations for the known failure classes
  (`public_counter_moved_on_registration`,
  `rebuilt_at_frozen_on_transition`, etc.). Tests prove it catches both
  a projection frozen at registration time (#4744's class) and one that
  counts intake as verified work.
- `rebuildFactoryProjection` is the compliant reference fold: verified
  counters, family coverage, validation rate, and `rebuiltAtIso` move
  only on validation transitions; registrations move only
  pending/quarantine intake counts. Revocations
  (verified → rejected) are reflected, not frozen.

### 1.5 Tick closure as acceptance — `tick-closure.ts`

- Version id: `tick_closure.v0.1`. A factory work unit counts only when
  intent, execution, state delta, and evaluation ALL close (the
  tetrahedron criterion). `trainingRecordRefFromClosedTick` mints a
  training record reference only from a closed, verified,
  corpus-admitted tick — closed ticks ARE training records; the
  distillation dataset is the byproduct of operation.

## 2. The local pilot corpus

`corpus.tassadar_trace.v0_1.local_pilot`, generated 2026-06-11 by
`scripts/tassadar-trace-factory-generate.ts` with the REAL TS executor
(`@openagentsinc/tassadar-executor`, Rust-digest-parity), master seed
`4748c0de20260611`, executor hash
`d8ebab55e81979862651f41f69ff0bb483824e603f542a28ddaf3432302501fa`.

- Committed (tracked): the manifest with every record's digests —
  `apps/openagents.com/workers/api/corpus/tassadar-trace-corpus.v0_1.manifest.json`
  (sha256
  `387459e56a053008938e5cfb195bf4006d1f742e136b0a73db5f1fba86cbce86`).
- Untracked, reproducible: binary shards + verdict log under
  `apps/openagents.com/workers/api/corpus/tassadar-trace-corpus.v0_1/`
  (gitignored; ~7.5 MB for 3.48M tokens — the compact-binary arithmetic
  holding in practice).

| Family | Records | Tokens | Shard sha256 (prefix) |
|---|---|---|---|
| family.arithmetic_carry.v1 | 112 | 1,327,104 | 9ca294f38cdc3cdd |
| family.memory_load_store.v1 | 76 | 778,240 | bfd8062422434198 |
| family.branch_gated_control.v1 | 56 | 663,552 | aee3d2bbd0381d26 |
| family.application_state_machine.v1 (held out) | 40 | 450,560 | c4c6eafdb9edf7b0 |
| family.near_miss_lookup.v1 (adversarial) | 24 | 250,880 | 46d79bd6aee7e277 |
| family.stack_loop_sum.compiled.v1 (anchor, held out) | 6 | 6,240 | 1a6048ed159f7e96 |
| **Total** | **314** | **3,476,576** | |

Split distribution per `training_split.v0.1`: train 208, long-horizon
eval 36, held-out-family eval 46, adversarial eval 24. Train-split
tokens: 1,515,520. Closed ticks: 314/314; every admitted record carries
a `trainingRecordRef` minted through tick closure.

The anchor family's six records are genuinely executed prefixes
(80/72/64/48/32/16 steps) of the committed psionic-compiled loop-sum
fixture, pinning Rust↔TS digest parity inside the corpus itself.

## 3. Pilot acceptance numbers (honest)

Targets from #4748: ≥99.9% schema-valid, ≥99% full-replay pass on
accepted traces, zero unversioned artifacts, all failures typed.

| Criterion | Target | Measured |
|---|---|---|
| Schema-valid (Tier 0 / attempted) | ≥ 99.9% | **100.000%** (314/314) |
| Full-replay pass (Tier 1 / Tier 0-verified) | ≥ 99% | **100.000%** (314/314) |
| Unversioned artifacts | 0 | **0** — every record carries schema, profile, token-encoding, compiler, executor versions/hashes; shards are digest-pinned in the manifest |
| Untyped failures | 0 | **0** failures of any kind in the pilot; every failure path in the pipeline is a typed union (decode failures, verdict rejections, build failures, admission outcomes) and the negative paths are exercised in tests (tampered tokens, forged digests, wrong workloads, truncated bytes) |

No fudging was needed: the generator and validator share nothing but
the contracts — the validator regenerates workloads from
(familyId, inputSeed, stepCount) and re-executes. The honest limitation
is stated in §6: in the local pilot, generation and validation ran in
the same process on the same machine.

## 4. Replay-from-clean-checkout proof

A fresh detached git worktree of `blocker-waves` (commit `4b957294c`)
was created at `/tmp/tassadar-clean-4748`, `bun install` run, and
`scripts/tassadar-trace-factory-replay.ts` executed twice from
`apps/openagents.com/workers/api`:

Leg A — regeneration-only (the clean checkout contains the committed
manifest but NO shard bytes; every workload regenerates from the
committed contracts and re-executes):

```
schema-valid: 314/314 (100.000%)
tier0 verified: 314/314 (100.000%)
tier1 full-replay pass: 314/314 (100.000%)
projection: verified=314 tokens=3476576 families=6
typed failures: 0
REPLAY: PASSED (clean-checkout replay proof holds)
```

Leg B — against the generated binary artifacts
(`--corpus-dir` pointed at the shards): shard sha256s match the
manifest, every stored record decodes, passes Tier 0, and passes
Tier 1 full replay; identical 314/314 numbers, `REPLAY: PASSED`.

The replay also rebuilds the reference projection from validation
transitions only and asserts it agrees with the manifest totals, and
runs the projection-rebuild compliance check — the public counters of
this corpus are checkable by construction.

## 5. Gates

- `bunx vitest run src/tassadar-trace-factory/` — 45/45 passing.
- `bun run typecheck` (workers/api) — green.
- `bun run check:architecture` — green (the contracts live inside the
  scanned worker tree and comply: typed errors only, no raw
  time/random/JSON.parse, no HTTP helpers in domain modules).

## 6. Evolution-loop linkage: PROPOSED clear for the distillation-dataset blocker

The yellow promise `artanis.tassadar_evolution_loop.v1` carries
`blocker.product_promises.tassadar_distillation_dataset_receipt_missing`
("the first dataset_curation receipt converting verified traces into a
distillation dataset"). This work gives that blocker its shape and a
mintable artifact, and **proposes** the clear — it does not flip it.

What now exists: a versioned, digest-pinned, replayable corpus of
verified traces in which every record is a closed tick admitted through
the frozen tier ladder, with the training split already applied. A
curated-dataset receipt is now mintable. The operator should record:

> `receipt.dataset_curation.corpus.tassadar_trace.v0_1.local_pilot` —
> curated distillation dataset of 314 verified trace records /
> 3,476,576 trace tokens (train split: 208 records / 1,515,520 tokens)
> across six program families under `training_split.v0.1`; every record
> Tier 0 + Tier 1 verified (100%/100%, zero typed failures); manifest
> `apps/openagents.com/workers/api/corpus/tassadar-trace-corpus.v0_1.manifest.json`
> at commit `4b957294c` (manifest sha256
> `387459e56a053008938e5cfb195bf4006d1f742e136b0a73db5f1fba86cbce86`);
> replay recipe `bun scripts/tassadar-trace-factory-replay.ts`,
> clean-checkout proof recorded in this document.

Honest scope notes for the operator's decision:

- The pilot corpus is locally generated by the TS executor in one
  process; the network leg (Pylon-dispatched generation, separate-device
  validation through the Plane B verdict flow) is the factory-pilot
  stage, not this one. If the blocker is read as requiring
  network-produced traces, this receipt is the dataset-curation
  precondition rather than the full clear.
- The other evolution-loop blocker (sustained unattended tick streak)
  is untouched by this work.

## 7. Named remainders

1. **Factory pilot on the network rails** — dispatch generation
   assignments (which, per the frozen iron rule, carry no expected
   digests) through the Pylon assignment route and validate via the
   worker-as-validator flow; first 100–300M tokens across the four
   scaled families. W1 (psionic window ladder, psionic#1119) gates
   family diversity there, not the freeze.
2. **Public factory monitor** — a worker surface projecting the factory
   counters; it must declare and pass
   `projectionRebuildCompliance` (the contract is ready and tested;
   the surface is not built).
3. **Tier 2/3 in anger** — both tiers are implemented and tested, but
   the pilot exercised Tier 0 + Tier 1 on every record (correct for a
   new worker under quarantine-before-admission); reputation-sampled
   spot-checks and post-admission adversarial sweeps become meaningful
   only with multiple workers.
4. **uint32 token width at scale** — encoded, decoded, and tested, but
   the pilot corpus is all uint16.
5. **Operator decision** on the proposed `dataset_curation` receipt and
   blocker disposition (§6).
