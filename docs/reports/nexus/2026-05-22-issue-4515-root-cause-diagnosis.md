# Issue #4515 — Nexus control-API capacity exhaustion: root-cause diagnosis

Date: 2026-05-22
Status: root cause verified by local reproduction; fix direction is a
nexus-control architecture decision (see "Fixing it")

## Summary

Issue #4515 reports the public Nexus relay returning
`503 embedded Nexus control API capacity exhausted`, which blocks training-node
admission and provider presence heartbeats for the Pylon fleet.

The 503 is a **symptom**. The cause is in `nexus-control`, not the relay: every
mutating control-API request is serialized through one global lock, and inside
that lock the mutation persists the **entire** kernel state to disk. Per-mutation
cost is O(total accumulated state), so the control API has a throughput ceiling
that **degrades as the fleet runs**. When sustained fleet load exceeds that
ceiling, requests pile up at the relay until its fixed authority-slot budget
(256) is full, and the relay returns the 503.

## The symptom

The relay proxies control-API requests behind a semaphore:

- `apps/nexus-relay/src/durable.rs:458` — `proxy_authority_http_request`
  acquires an `authority_slots` permit with `try_acquire_owned()`.
- The permit is held for the **entire** proxied request, including the upstream
  HTTP call to the embedded control API (timeout
  `DEFAULT_AUTHORITY_HTTP_TIMEOUT_MS` = 180 s).
- Budget is `DEFAULT_AUTHORITY_MAX_IN_FLIGHT` = 256
  (`NEXUS_RELAY_AUTHORITY_MAX_IN_FLIGHT`).
- When all 256 permits are held, the next request gets
  `503 embedded Nexus control API capacity exhausted`.

For 256 permits to fill, 256 requests must be in flight at once. That happens
only when control-API requests take a long time — so the real question is why
control-API requests are slow.

## Root cause (in `nexus-control`)

### 1. One global lock serializes every mutation

`apps/nexus-control/src/lib.rs:1977` — `AppState.store` is a single
`Arc<RwLock<ControlStore>>` (a `std::sync::RwLock`). `ControlStore`
(`lib.rs:1991`) holds sessions, provider presence, the training scheduler,
treasury, economy, and the kernel — all behind that one lock.

Every mutating handler takes `state.store.write()`:

- `record_provider_presence_heartbeat` — `lib.rs:13519`
- `record_training_node_admission` — `lib.rs:13729`
- `record_training_node_heartbeat` — `lib.rs:13769`
- and the other training/treasury/session mutation handlers.

All mutations are therefore fully serialized. `std::sync::RwLock::write()` is a
blocking call, and the embedded control API runs on only 4 tokio worker threads
(`DEFAULT_AUTHORITY_TOKIO_WORKER_THREADS` = 4 in `durable.rs`). Under load all 4
worker threads block on `.write()`.

### 2. Each mutation persists the full kernel state under that lock

The admission path, under the write lock, reaches:

```
record_training_node_admission (handler, lib.rs:13722)
  -> kernel.record_training_node_admission (kernel.rs:3308)
    -> refresh_snapshot_for (kernel.rs:3457 / defined 15110)
      -> persist_compute_authority_state (kernel.rs:15118 / defined 4496)
```

`persist_compute_authority_state` (`kernel.rs:4496`):

- clones ~30 kernel collections (`admitted_training_nodes`,
  `compute_training_runs`, `receipt_store`, `snapshots`, ...),
- serializes the whole thing with `serde_json::to_vec_pretty`,
- writes it with `fs::write` to a temp file and `fs::rename`s it into place.

`refresh_snapshot_for` also calls `compute_snapshot_for` (`kernel.rs:15122`),
which runs `receipt_store.list_receipts()` and market-metrics passes over
**every** receipt.

Both are O(total accumulated state) and both run **inside the global write
lock**. The heartbeat handler additionally does a synchronous
`store.persist_training_scheduler_state()` disk write under the lock
(`lib.rs:13811`).

### 3. Consequence

Throughput is capped at one full-state persist at a time, and that cap **falls
as state grows** (more admitted nodes, more receipts, more snapshots → larger
clone + larger JSON + larger write). Each in-flight request holds its relay
authority-slot permit for its full duration. Once sustained fleet load pushes
in-flight count to 256, the relay returns the 503. This matches the observed
behavior: the failure appears under fleet load and worsens the longer Nexus
runs.

## Reproduction

`apps/nexus-control/tests/issue_4515_control_api_capacity.rs` —
`issue_4515_control_api_mutation_latency_scales_with_state`
(`#[tokio::test(flavor = "multi_thread", worker_threads = 4)]`, `#[ignore]`d).

It drives admissions through the real `nexus-control` router (`build_router`)
on a 4-worker runtime with a real on-disk `kernel_state_path`. Run:

```
cargo test -p nexus-control --test issue_4515_control_api_capacity \
    -- --ignored --nocapture
```

`ISSUE_4515_ADMISSIONS` overrides the admission count (default 800).

The reproduction sends minimal admission requests (no capability envelope), so
the kernel records each admission and then refuses it. The persistence path
exercised is identical to an accepted admission — receipt write, snapshot
recompute, full kernel-state rewrite — and because each stored object is
smaller than a real Pylon's, the measured growth below is a **conservative
lower bound**; production admissions carry full capability envelopes and
accumulate state faster.

### Phase A — per-admission latency vs. accumulated state (depth 800 run)

| admissions recorded | avg admission latency |
| ------------------: | --------------------: |
|                  80 |               0.65 ms |
|                 240 |               1.82 ms |
|                 400 |               2.92 ms |
|                 560 |               3.67 ms |
|                 800 |               5.42 ms |

Latency grows monotonically — 8.3x across this run. The kernel state file
reached 1.82 MB after 800 admissions, and per-mutation cost tracks that growth,
exactly as the full-state persist predicts. (An earlier run that sent full
fixture capability envelopes showed ~12-13x over the same depth.)

### Phase B — concurrency vs. sequential (at depth 800)

| measurement                           | result |
| -------------------------------------- | ------ |
| 160 admissions sequential              | 1.04 s |
| 160 admissions concurrent (4 workers)  | 1.25 s |
| concurrency speedup                    | 0.83x  |
| concurrent latency p50 / p99           | 30.6 ms / 47.6 ms |

4-way concurrency produces **no** throughput gain (0.83x — slightly worse,
because lock contention adds overhead). The control API is fully serialized:
a throughput ceiling of ~128 mutations/sec at this depth, and falling as state
grows. Each mutation also holds a relay authority-slot permit for its full
~31 ms.

### Phase C — validator-challenge requests stall on the same lock (depth 800)

Validator challenge claim/retry/finalize are `store.write()` mutations too, so
the validator backlog drains through the same global lock. Phase C times a
validator-challenge `claim` for an unadmitted node — it acquires the write
lock, fails the node lookup, and returns, so its latency is essentially the
time spent acquiring the lock:

| condition                            | claim latency avg / p99 |
| -------------------------------------- | ----------------------- |
| idle control plane                     | 5.6 µs / 62 µs |
| under a concurrent admission flood     | 7.9 ms / 36 ms |

Idle, a claim is microseconds. Under admission load it rises by three orders of
magnitude — every validator mutation queues behind the same serialized
O(total-state) persists. The idle baseline being microseconds inflates the raw
ratio; the figure that matters is the absolute under-load latency, and on
production scale (far more accumulated state, sustained real fleet load) it is
correspondingly worse. This is the mechanism behind a validator backlog that
does not drain: the work to clear it cannot get through the saturated control
API.

### Phase D — artifact resolver and signed-access stall on the same lock (depth 800)

The artifact resolver (`get_kernel_compute_training_artifact_resolver`) and
signed-access (`post_kernel_compute_training_artifact_signed_access`) endpoints
are read paths: each takes `store.read()` before fast work (an in-memory
lookup; for signed-access a local GCS v4 signing step with no network call).
Phase D probes both for a missing artifact — the handler acquires the read
lock, fails the lookup, and returns — idle and then under a concurrent
admission flood:

| endpoint          | idle  | under admission load (avg / p99) |
| ------------------ | ----- | -------------------------------- |
| artifact resolver  | ~8 µs | 4.5 ms / 87 ms |
| signed-access      | ~7 µs | 13.2 ms / 57 ms |

Idle, both are microseconds. Under admission write-load they stall to
milliseconds with a long p99 tail — `store.read()` cannot be acquired while
writers hold the lock for O(total-state) persists. This is the mechanism behind
the production "Artifact resolver latency" and "Signed access latency" health
alerts (each p95 ~4.5 s). As in the other phases, the raw idle→load ratio is
inflated by the microsecond baseline; the figure that matters is the absolute
under-load latency, and production scale is worse.

## Why relay-side changes do not fix this

The relay's semaphore is downstream of the bottleneck. Specifically:

- Raising `NEXUS_RELAY_AUTHORITY_MAX_IN_FLIGHT` above 256 only lets more
  requests pile up against the same serialized control API; it delays the 503
  and inflates latency instead of removing the failure.
- Replacing the relay's fail-fast `try_acquire_owned()` with a bounded wait
  queue cannot raise a serialized server's throughput ceiling; it changes when
  the 503 is returned, not whether the system keeps up.

The fix has to remove the per-mutation O(total-state) work from the control
API's critical path.

## Fixing it: options and the durability constraint

The per-mutation cost is O(total accumulated state) because every mutation
rewrites the entire kernel state file under the global lock. Removing that
requires one of:

- **Background / debounced persistence** — mutations update memory and mark
  state dirty; a background task writes periodically (and on shutdown). Removes
  the persist from the mutation hot path, but a crash loses the un-persisted
  window.
- **Incremental persistence** — append the per-mutation delta and replay it on
  load, compacting periodically. Keeps per-mutation cost flat as state grows
  and preserves today's write-through durability.

The durability constraint is decisive, and it was checked rather than assumed.
The kernel state file is the **authoritative** record of kernel authority
state, not a reconstructable cache:
`KernelState::new_with_persistence` -> `load_persisted_compute_authority_state`
(`kernel.rs:4407`) is the only loader, and it loads all ~30 collections
directly from the single JSON file. There is no receipt-log replay or other
reconstruction path. (`economy`'s receipt log is a separate, narrower record;
it does not rebuild kernel state.)

Consequently:

- Background / debounced persistence would lose authoritative kernel state on
  any crash within the un-persisted window. That conflicts with the repo
  guardrail that sync and state continuity must remain deterministic and
  replay-safe.
- Incremental persistence preserves durability but is a substantial change to
  the authority kernel's persistence layer.

A smaller, durability-preserving partial step is possible — keep the persist
write-through but move the serialize + `fs::write` off the global lock (clone
under the lock, write after releasing it, with writer ordering by sequence
number). That removes serialize + fsync from the lock's critical section, but
each mutation still does O(total-state) work, so it mitigates rather than
removes the scaling problem.

Which direction to take is an architecture decision for the nexus-control
maintainers. This report and the reproduction test provide the evidence to make
that decision; a fix is intentionally not included here so the decision is not
pre-empted.

## Artifacts

- Reproduction test:
  `apps/nexus-control/tests/issue_4515_control_api_capacity.rs`.
- Relay 503 site: `apps/nexus-relay/src/durable.rs:458`.
- Persist hot path: `apps/nexus-control/src/kernel.rs:4496`
  (`persist_compute_authority_state`), reached via
  `refresh_snapshot_for` (`kernel.rs:15110`).
