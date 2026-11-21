# 272. Bounded training windows and Local-SGD / DiLoCo

[Chapter 235](ch235-can-gradient-descent-learn-exact-execution.md) and
[chapter 236](ch236-a-frozen-exact-core-with-a-learned-interface.md) established
what decentralized training should learn: **routing and interface adapters**
around frozen, digest-pinned compiled exact cores (`H2_ROUTING_ONLY`), never
replacing exact execution with gradient descent. [Chapter 238](ch238-pay-for-verified-work.md)
settled verified execution and replay on live Bitcoin mainnet. This chapter
opens Part XXVIII by defining the work unit that turns idle consumer Macs and
NVIDIA GPUs into a decentralized training network: a bounded
**`TrainingWindow`** backed by a strict **Dataset Shard Authority**.

Consumer machines on home internet cannot run synchronous all-reduce across
every step. Instead, Pylons run **Local-SGD / DiLoCo** (Distributed
Low-Communication) windows: each worker leases an immutable verified shard and
a pinned parent checkpoint, runs up to `TW_MAX_INNER_STEPS` (`500`) local inner
steps, and uploads a compact pseudo-gradient or LoRA delta capped at
`TW_MAX_BYTES_OUT` (`64 MiB`). Pylons are paid for **accepted windows**, never
raw GPU hours.

One `#authority` file holds the shard admission gate and bounded lease pool:
[`peer/training-window.coax`](../lib/peer/training-window.coax) (`119` mutants:
`117` killed, `0` survived, `2` marked equivalent).

```shell
coax/bin/coax run psionic/examples/training-window.coax
coax/bin/coax test psionic/lib/peer/training-window.coax
coax/bin/coax mutate psionic/lib/peer/training-window.coax --root-only
```

## By hand: shard admission and lease accounting

Before running the code, trace the two state machines by hand:

1. **Shard admission (`tw_admit_shard`):** A shard is admitted only if all four
   invariants hold in order: `quarantined_rows == 0` (never train on unverified
   traces or trajectories), `verified_rows >= min_rows` with `min_rows >= 1`,
   `redacted_pii_ok == true` (secret, PII, and BIP-39 redaction passed), and
   `exposure_overlap == 0` ([chapter 247](ch247-outcomes-become-training-rows.md)'s
   held-out exposure ledger has zero overlap).
   - `(128, 100, 0, true, 0)` passes (`ok`).
   - Adding a single quarantined row `(128, 100, 1, true, 0)` refuses
     immediately with `unverified_rows`.
   - Having `99 < 100` verified rows refuses with `insufficient_rows`.
   - Unredacted PII refuses with `unredacted_pii`; a single overlapping held-out
     item refuses with `eval_leakage`.
2. **Bounded leases (`tw_issue_lease`, `tw_complete_lease`, `tw_expire_leases`):**
   Starting from an empty pool (`active = 0, completed = 0, expired = 0`):
   - At `t = 1000`, worker `10` leases shard `888` on parent `777` for `50`
     inner steps, `4096` max output bytes, `ttl = 300` (`expires_at_s = 1300`) →
     `ok` (`active = 1`).
   - Issuing a second active lease for the same `(worker 10, shard 888)` →
     `duplicate_lease`.
   - Requesting `501 > TW_MAX_INNER_STEPS` → `invalid_bounds`.
   - Worker `11` leases shard `888` at `t = 1000` with `ttl = 120`
     (`expires_at_s = 1120`) → `ok` (`active = 2`).
   - At `t = 1100`, worker `10` tries to complete with `4097 > 4096` bytes →
     `invalid_bounds`; completing with `2048` bytes succeeds → `ok`
     (`active = 1, completed = 1`).
   - At `t = 1121 > 1120`, worker `11` tries to complete late → `lease_expired`.
   - Running `tw_expire_leases(p, 1121)` reclaims that `1` expired lease,
     leaving `active = 0, completed = 1, expired = 1`.

## The shard gate and bounded lease pool

Every bound is an explicit compile-time constant (`TW_MAX_LEASES = 32`,
`TW_MAX_INNER_STEPS = 500`, `TW_MAX_BYTES_OUT = 64 MiB`, `TW_MAX_TTL_S = 3600`):

<!-- source: psionic/lib/peer/training-window.coax types -->
```jai
TW_MAX_LEASES :: 32;
TW_MAX_INNER_STEPS :: 500;
TW_MAX_BYTES_OUT :: 67108864; // 64 MiB per window delta cap
TW_MAX_TTL_S :: 3600;

TwStatus :: enum u8 {
  ok;
  unverified_rows;
  insufficient_rows;
  unredacted_pii;
  eval_leakage;
  invalid_bounds;
  duplicate_lease;
  lease_not_found;
  lease_expired;
  capacity;
}

TwLease :: struct {
  active: bool;
  lease_id: int;
  worker_key: int;
  parent_sha: int;
  shard_sha: int;
  inner_steps: int;
  max_bytes_out: int;
  expires_at_s: int;
}

TwPool :: struct {
  leases: [TW_MAX_LEASES] TwLease;
  active_count: int;
  completed_count: int;
  expired_count: int;
}
```

The Dataset Shard Authority checks quarantine, row count, PII redaction, and
exposure-ledger decontamination before any shard can be leased:

<!-- source: psionic/lib/peer/training-window.coax admit -->
```jai
// Dataset Shard Authority: never train on quarantined/unverified rows, unredacted PII, or held-out eval leakage.
tw_admit_shard :: (
  verified_rows: int, min_rows: int, quarantined_rows: int,
  redacted_pii_ok: bool, exposure_overlap: int
) -> TwStatus #effects(all) {
  if quarantined_rows > 0 { return TwStatus.unverified_rows; }
  if min_rows < 1 || verified_rows < min_rows { return TwStatus.insufficient_rows; }
  if !redacted_pii_ok { return TwStatus.unredacted_pii; }
  if exposure_overlap > 0 { return TwStatus.eval_leakage; }
  return TwStatus.ok;
}
```

The lease pool enforces non-zero bindings, step/byte/TTL ceilings, uniqueness
per `lease_id` and per `(worker_key, shard_sha)`, bandwidth caps on completion,
and deterministic expiry reclamation:

<!-- source: psionic/lib/peer/training-window.coax leases -->
```jai
// Issue a bounded TrainingWindow lease for (worker_key, parent_sha, shard_sha).
tw_issue_lease :: (
  p: [] TwPool, lease_id: int, worker_key: int, parent_sha: int, shard_sha: int,
  inner_steps: int, max_bytes_out: int, now_s: int, ttl_s: int
) -> TwStatus #effects(all) {
  if lease_id == 0 || worker_key == 0 || parent_sha == 0 || shard_sha == 0 { return TwStatus.invalid_bounds; }
  if inner_steps < 1 || inner_steps > TW_MAX_INNER_STEPS { return TwStatus.invalid_bounds; }
  if max_bytes_out < 1 || max_bytes_out > TW_MAX_BYTES_OUT { return TwStatus.invalid_bounds; }
  if ttl_s < 1 || ttl_s > TW_MAX_TTL_S { return TwStatus.invalid_bounds; }

  free_slot := -1;
  for i: 0..<TW_MAX_LEASES {
    if p[0].leases[i].active {
      if p[0].leases[i].lease_id == lease_id ||
         (p[0].leases[i].worker_key == worker_key && p[0].leases[i].shard_sha == shard_sha) {
        return TwStatus.duplicate_lease;
      }
    } else if free_slot < 0 { // mutant: equivalent — any free slot yields identical pool semantics
      free_slot = i;
    }
  }
  if free_slot < 0 { return TwStatus.capacity; }
  p[0].leases[free_slot] = TwLease.{
    active = true,
    lease_id = lease_id,
    worker_key = worker_key,
    parent_sha = parent_sha,
    shard_sha = shard_sha,
    inner_steps = inner_steps,
    max_bytes_out = max_bytes_out,
    expires_at_s = now_s + ttl_s,
  };
  p[0].active_count += 1;
  return TwStatus.ok;
}

// Complete a leased window within its TTL and bandwidth budget.
tw_complete_lease :: (p: [] TwPool, lease_id: int, worker_key: int, bytes_out: int, now_s: int) -> TwStatus #effects(all) {
  for i: 0..<TW_MAX_LEASES {
    l := p[0].leases[i];
    if l.active && l.lease_id == lease_id && l.worker_key == worker_key {
      if now_s > l.expires_at_s { return TwStatus.lease_expired; }
      if bytes_out < 1 || bytes_out > l.max_bytes_out { return TwStatus.invalid_bounds; }
      p[0].leases[i].active = false;
      p[0].active_count -= 1;
      p[0].completed_count += 1;
      return TwStatus.ok;
    }
  }
  return TwStatus.lease_not_found;
}

// Reclaim all expired leases at `now_s`. Returns count reclaimed.
tw_expire_leases :: (p: [] TwPool, now_s: int) -> int #effects(all) {
  reclaimed := 0;
  for i: 0..<TW_MAX_LEASES {
    if p[0].leases[i].active && now_s > p[0].leases[i].expires_at_s {
      p[0].leases[i].active = false;
      p[0].active_count -= 1;
      p[0].expired_count += 1;
      reclaimed += 1;
    }
  }
  return reclaimed;
}
```

## Runnable example and expected transcript

[`examples/training-window.coax`](../examples/training-window.coax) drives every
shard refusal and lease transition and matches
[`fixtures/training-window.out`](../fixtures/training-window.out) byte for byte
in both debug and release builds:

```text
shard clean: ok
shard unverified: unverified_rows
shard short: insufficient_rows
shard pii: unredacted_pii
shard leak: eval_leakage
lease 1: ok
lease dup: duplicate_lease
lease over_steps: invalid_bounds
lease 2: ok
complete over_bytes: invalid_bounds
complete 1: ok
complete 2 late: lease_expired
reclaimed: 1 active: 0 completed: 1 expired: 1
```

## Checks and independent evidence

- **Unit & boundary checks:** `coax/bin/coax test psionic/lib/peer/training-window.coax`
  verifies every lower/upper boundary (`1` vs `TW_MAX_INNER_STEPS`, `TW_MAX_BYTES_OUT`,
  `TW_MAX_TTL_S`, `TW_MAX_LEASES`), duplicate `(worker_key, shard_sha)` refusal,
  wrong-worker completion refusal, and slot-0 expiry reclamation.
- **Mutation sharpness:** `#authority` with `117/119` mutants killed, `0`
  survivors, `2` marked equivalent.
- **End-to-end 4-Pylon Local-SGD / DiLoCo runner:**
  [`psionic/tools/diloco-round.coax`](../tools/diloco-round.coax) runs four
  independent Pylons (`30` Local-SGD inner steps each) over disjoint verified
  shards under `training-window.coax` leases and feeds their signed claims into
  [chapter 273](ch273-byzantine-verification-for-public-training.md).
- **Schema & CLI wiring:** Bound to `TrainingWindowClaim` in
  [`proto/openagents/executor/v1alpha1/training.proto`](../../proto/openagents/executor/v1alpha1/training.proto)
  and advertised over the `cj-training` lane in
  [NIP-PYLON](../../nostr/nips/openagents/NIP-PYLON.md).
