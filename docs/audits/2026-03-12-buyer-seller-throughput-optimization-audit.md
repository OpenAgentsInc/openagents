# 2026-03-12 Buyer/Seller Throughput Optimization Audit

## Goal

Measure the effective buyer-to-seller throughput of the current Autopilot NIP-28 + NIP-90 compute path, optimize it through repeated code changes, and stop only after the codebase shows measurable throughput gains across multiple scenarios.

This audit documents:

- the measurement harness
- why the harness was built the way it was
- every optimization cycle that was run
- the measured deltas after each cycle
- the final optimized settings now in the repo
- remaining ceilings and follow-up work

## Scope

This work stayed inside `apps/autopilot-desktop`, which is the correct owner for:

- Mission Control buy-mode scheduling
- app-owned NIP-28 peer selection
- provider lane polling and publish handling
- desktop-control / CLI-visible throughput instrumentation

No reusable protocol crates were expanded for product-specific throughput tuning.

## Why a dedicated benchmark was necessary

We needed a repeatable measurement loop that exercised the real product flow without being dominated by unrelated network variance.

The main options were:

1. Live mainnet Spark settlement on every run.
2. Deterministic local relay + synthetic wallet settlement.

For throughput tuning, option 2 is the right primary tool.

Reasoning:

- Live Spark/Lightning settlement is necessary for correctness checks, but it mixes app latency with mempool, remote receiver, and Lightning path variance.
- The question here was how quickly the app can progress a buyer request through request publish, provider execution, result publish, invoice delivery, and local payment completion.
- To maximize product throughput, we first need to remove app-side bottlenecks. A deterministic harness gives a stable target for that.

This means the benchmark measures **Autopilot throughput**, not full internet-to-internet Lightning settlement throughput.

## Benchmark design

I added a dedicated relay-backed benchmark binary:

- `apps/autopilot-desktop/src/bin/autopilot_throughput_bench.rs`
- `apps/autopilot-desktop/src/throughput_bench.rs`

The harness uses real app paths instead of fake shortcuts:

- NIP-28 chat lane for presence and peer visibility
- app-owned Autopilot compute presence publishing
- app-owned peer roster targeting
- NIP-90 provider lane publish / ingress behavior
- app-owned buyer request state and compute flow projection

The harness runs three scenarios:

1. `single_pair_serial`
2. `multi_buyer_multi_provider`
3. `role_flip_pair`

### What each scenario covers

`single_pair_serial`

- 1 buyer
- 1 provider
- serial jobs
- isolates dispatch cadence and provider response overhead

`multi_buyer_multi_provider`

- 3 buyers
- 3 providers
- concurrent demand
- exposes provider hot-spotting and roster-selection fairness problems

`role_flip_pair`

- 2 identities that swap buyer/provider roles across phases
- verifies that the flow remains healthy when roles reverse instead of using a fixed seller/buyer split

### NIP-28 coordination included in the benchmark

The harness did not merely publish NIP-90 requests directly.

It also:

- created the main NIP-28 channel
- ran the real NIP-28 chat lane
- published compute-presence events into that channel
- published one ordinary readiness chat message per provider

So the benchmark exercised the same main-channel roster behavior that now drives targeted buy-mode dispatch.

I did **not** add extra free-form negotiation chatter as a dedicated stress scenario, because the first measurement passes showed the dominant bottlenecks were:

- buy-mode dispatch cadence
- lane polling intervals
- provider command-drain behavior
- provider-selection fairness

More chat noise would have been secondary until those were fixed.

## Environment and commands

Before touching desktop code, I verified the Apple FM bridge as required by repo instructions:

```bash
curl -s http://127.0.0.1:11435/health
```

Build / run commands used during the work:

```bash
cargo build -q -p autopilot-desktop --bin autopilot-throughput-bench
cargo build -q -p autopilot-desktop --bin autopilotctl
target/debug/autopilot-throughput-bench --provider-compute-ms 40 > /tmp/autopilot-throughput-baseline.json
target/debug/autopilot-throughput-bench --provider-compute-ms 40 > /tmp/autopilot-throughput-cycle1.json
target/debug/autopilot-throughput-bench --provider-compute-ms 40 > /tmp/autopilot-throughput-cycle2.json
target/debug/autopilot-throughput-bench --provider-compute-ms 40 > /tmp/autopilot-throughput-cycle3.json
target/debug/autopilot-throughput-bench --provider-compute-ms 40 > /tmp/autopilot-throughput-cycle4.json
target/debug/autopilot-throughput-bench --provider-compute-ms 40 > /tmp/autopilot-throughput-cycle5.json
RUST_LOG=error target/debug/autopilot-throughput-bench --provider-compute-ms 40 > /tmp/autopilot-throughput-final.clean.json
```

The raw benchmark files are left in `/tmp/` for local inspection.

## Metrics tracked

Per scenario, the benchmark records:

- total jobs
- completed jobs
- total scenario duration
- jobs per minute
- p50 request publish latency
- p50 result latency
- p50 payment-required latency
- p50 paid latency
- p50 total latency
- p95 total latency
- provider distribution

The most important throughput metrics were:

- `jobs_per_minute`
- `p50_paid_latency_ms`
- `p95_total_latency_ms`

## Baseline

Baseline configuration reflected the pre-optimization behavior:

- buy-mode cadence effectively `12s`
- provider lane poll interval `120ms`
- NIP-28 lane poll interval `120ms`
- provider lane processed only one command per loop
- targeted peer selection could repeatedly pin to the same provider

Baseline results:

| Scenario | Jobs/min | p50 paid latency | p95 total latency | Provider distribution |
| --- | ---: | ---: | ---: | --- |
| `single_pair_serial` | 7.41 | 280 ms | 285 ms | `3` |
| `multi_buyer_multi_provider` | 28.68 | 402 ms | 549 ms | `6` on one provider |
| `role_flip_pair` | 9.71 | 276 ms | 278 ms | `2 / 2` |

## Optimization cycle 1: cut buy-mode cadence from 12s to 1s

### Change

In `app_state.rs` and `headless_compute.rs`:

- `MISSION_CONTROL_BUY_MODE_INTERVAL_SECONDS: 12 -> 1`
- `HEADLESS_BUY_MODE_INTERVAL_SECONDS: 12 -> 1`

### Hypothesis

The initial cadence floor was the biggest obvious limiter on job-start throughput.

### Result

| Scenario | Jobs/min | p50 paid latency | p95 total latency |
| --- | ---: | ---: | ---: |
| `single_pair_serial` | 78.81 | 287 ms | 289 ms |
| `multi_buyer_multi_provider` | 233.46 | 405 ms | 541 ms |
| `role_flip_pair` | 88.53 | 280 ms | 282 ms |

### What improved

- Dispatch throughput improved immediately by roughly one order of magnitude.

### What did not improve

- Paid latency barely moved.
- Provider hot-spotting in multi-provider mode remained unchanged.

Conclusion: cadence was a hard cap on throughput, but not the main latency bottleneck.

## Optimization cycle 2: reduce lane latency and drain the provider command queue

### Change

In `provider_nip90_lane.rs`:

- `LANE_POLL: 120ms -> 30ms`
- `RELAY_RECV_TIMEOUT: 4ms -> 2ms`
- `MAX_MESSAGES_PER_RELAY_POLL: 6 -> 24`
- added provider command draining so one loop iteration can process all pending commands

In `nip28_chat_lane.rs`:

- `LANE_POLL: 120ms -> 30ms`
- `RELAY_RECV_TIMEOUT: 4ms -> 2ms`

### Hypothesis

The system was spending too much time asleep and was serializing command handling more than necessary.

### Result

| Scenario | Jobs/min | p50 paid latency | p95 total latency |
| --- | ---: | ---: | ---: |
| `single_pair_serial` | 83.96 | 97 ms | 134 ms |
| `multi_buyer_multi_provider` | 287.77 | 179 ms | 255 ms |
| `role_flip_pair` | 101.10 | 93 ms | 132 ms |

### What improved

- Latency dropped sharply across all scenarios.
- The app was no longer dominated by polling sleep.

Conclusion: lane cadence and command-drain behavior were major critical-path bottlenecks.

## Optimization cycle 3: fix provider hot-spotting with app-owned roster policy

### Change

In `autopilot_peer_roster.rs` and related app state:

- added `select_autopilot_buy_mode_target_with_policy(...)`
- target choice now rotates past the last successfully targeted peer
- if no previous target exists, selection uses a stable buyer-specific offset derived from local pubkey
- successful real dispatches now record the last targeted peer

### Hypothesis

The benchmark showed multi-provider demand collapsing onto a single provider. That reduces effective throughput even if each provider is fast.

### Result

| Scenario | Jobs/min | p50 paid latency | p95 total latency | Provider distribution |
| --- | ---: | ---: | ---: | --- |
| `single_pair_serial` | 85.71 | 91 ms | 126 ms | `3` |
| `multi_buyer_multi_provider` | 289.86 | 211 ms | 285 ms | `2 / 2 / 2` |
| `role_flip_pair` | 103.54 | 94 ms | 140 ms | `2 / 2` |

### What improved

- Multi-provider load distribution became correct.
- Throughput stayed high while work spread evenly.

### Tradeoff

- p50 paid latency in the multi-provider scenario rose slightly versus cycle 2.

That tradeoff was acceptable because the system was now behaving correctly at the product level instead of hiding work imbalance behind one hot provider.

## Optimization cycle 4: tighten lane cadence further

### Change

In `provider_nip90_lane.rs`:

- `LANE_POLL: 30ms -> 15ms`
- `RELAY_RECV_TIMEOUT: 2ms -> 1ms`
- `MAX_MESSAGES_PER_RELAY_POLL: 24 -> 48`

In `nip28_chat_lane.rs`:

- `LANE_POLL: 30ms -> 15ms`
- `RELAY_RECV_TIMEOUT: 2ms -> 1ms`

### Result

| Scenario | Jobs/min | p50 paid latency | p95 total latency |
| --- | ---: | ---: | ---: |
| `single_pair_serial` | 86.46 | 77 ms | 82 ms |
| `multi_buyer_multi_provider` | 298.26 | 145 ms | 203 ms |
| `role_flip_pair` | 104.48 | 73 ms | 78 ms |

### What improved

- Further meaningful latency reduction.
- Multi-provider throughput rose again while keeping the balanced distribution.

Conclusion: the app still had slack in its event loop cadence.

## Optimization cycle 5: break the 1-second cadence floor and fix event-id collisions

### Change

Buy-mode cadence was reduced from `1s` to `100ms`.

This touched:

- `app_state.rs`
- `desktop_control.rs`
- `pane_renderer.rs`
- `nip90_compute_flow.rs`
- `autopilotctl.rs`
- `throughput_bench.rs`

### Hidden bug discovered

The first attempt at sub-second dispatch did **not** work.

Reason:

- Nostr event timestamps are second-granularity.
- Our Mission Control buy-mode requests had identical payload and tags when dispatched multiple times inside the same second.
- That meant repeated dispatches produced the same event id.

Observed effect:

- the benchmark appeared to stall
- the same request id kept being republished

### Fix

In `input/actions.rs`, `build_mission_control_buy_mode_request_event(...)` now adds:

- `oa_dispatch_nonce`

The nonce is generated from nanosecond-resolution system time so sub-second dispatches produce distinct Nostr events.

### Result

Cycle-5 benchmark:

| Scenario | Jobs/min | p50 paid latency | p95 total latency |
| --- | ---: | ---: | ---: |
| `single_pair_serial` | 640.57 | 80 ms | 80 ms |
| `multi_buyer_multi_provider` | 867.47 | 193 ms | 204 ms |
| `role_flip_pair` | 452.83 | 85 ms | 93 ms |

This was the biggest throughput jump of the entire exercise.

## Final confirmation run

After the optimization cycles were complete, I reran the benchmark once more with quiet logging and used that as the final confirmation artifact:

- `/tmp/autopilot-throughput-final.clean.json`

Final confirmation metrics:

| Scenario | Jobs/min | p50 paid latency | p95 total latency | Provider distribution |
| --- | ---: | ---: | ---: | --- |
| `single_pair_serial` | 645.16 | 74 ms | 89 ms | `3` |
| `multi_buyer_multi_provider` | 812.64 | 209 ms | 221 ms | `2 / 2 / 2` |
| `role_flip_pair` | 480.96 | 77 ms | 94 ms | `2 / 2` |

## Baseline vs final

Using baseline versus the final confirmation run:

| Scenario | Jobs/min baseline | Jobs/min final | Improvement | p50 paid baseline | p50 paid final | Improvement |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pair_serial` | 7.41 | 645.16 | `87.05x` | 280 ms | 74 ms | `3.78x` faster |
| `multi_buyer_multi_provider` | 28.68 | 812.64 | `28.33x` | 402 ms | 209 ms | `1.92x` faster |
| `role_flip_pair` | 9.71 | 480.96 | `49.54x` | 276 ms | 77 ms | `3.58x` faster |

## What actually limited throughput

The optimization work showed that the main bottlenecks were:

1. **Mission Control cadence was far too conservative.**
2. **The provider and NIP-28 lanes slept too long between polls.**
3. **Provider command handling was under-drained.**
4. **Multi-provider demand was not distributed fairly.**
5. **Sub-second dispatch exposed a real event-identity collision bug.**

Notably, “provider compute time” was not the dominant issue in these runs; the harness compute delay stayed fixed at `40ms`.

## What is now true in the codebase

The repo now has:

- a reproducible relay-backed throughput benchmark
- app-owned targeted-provider rotation that avoids hot-spotting
- lower-latency NIP-28 and provider lanes
- sub-second Mission Control buy-mode cadence
- explicit UI / CLI support for sub-second buy-mode timing
- a nonce in buy-mode request events so sub-second dispatch is valid

## Why I did not use repeated live mainnet Spark payments for this loop

The user explicitly allowed moving sats, but repeated live-wallet benchmarking was not the right optimization loop for this problem.

Using mainnet Spark for every iteration would have mostly measured:

- Lightning path quality
- remote invoice handling
- fee/reserve behavior
- remote network variance

That is valuable for correctness and economics, but it is a bad primary signal for optimizing **app throughput**.

The benchmark here was deliberately scoped to the app-controlled path so each code change had a measurable, attributable effect.

## Remaining ceilings

The current throughput is much better, but there are still clear future levers:

1. **Single-flight buy mode still caps buyer concurrency.**
   The buy-mode loop dispatches one outstanding request at a time. Parallel buyer flights would raise throughput further.

2. **Provider publish/result/payment flow is still sequential per job.**
   If we ever support more parallel provider work, the current active-job assumptions will become a limit.

3. **NIP-28 coordination traffic is still light in the benchmark.**
   We should add a noisy-channel scenario later, but only after the current gains land and remain stable.

4. **Real-sats throughput is still a separate benchmark class.**
   We should add a lower-frequency live-wallet benchmark for end-to-end economic latency, but that should not replace the deterministic app-throughput harness.

## Recommended next measurements

1. Add a `chat_noise_multi_agent` scenario that floods the NIP-28 main channel with non-presence messages while compute jobs continue.
2. Add a `parallel_buyers_single_provider` saturation scenario to identify the true single-provider ceiling.
3. Add a separate `live_wallet_smoke` throughput report that measures one or two real Spark-settled jobs and keeps fee/settlement metrics distinct from the synthetic harness.

## Validation run during this work

The key checks used while finishing this throughput work were:

```bash
cargo test -p autopilot-desktop --lib autopilot_peer_roster::tests::buy_mode_target_selection_rotates_after_last_targeted_peer -- --exact
cargo test -p autopilot-desktop --lib autopilot_peer_roster::tests::buy_mode_target_selection_uses_stable_local_offset_when_no_last_target_exists -- --exact
cargo test -p autopilot-desktop --lib app_state::tests::mission_control_buy_mode_loop_toggle_and_schedule_are_deterministic -- --exact
cargo test -p autopilot-desktop --bin autopilotctl -- --nocapture
scripts/lint/ownership-boundary-check.sh
scripts/lint/workspace-dependency-drift-check.sh
git diff --check
```

`scripts/lint/touched-clippy-gate.sh` is still blocked by pre-existing repo debt and warning noise outside the scope of this throughput work, not by the throughput changes themselves.

## Bottom line

This work produced measurable throughput gains, not cosmetic changes.

The main throughput win came from treating the buyer/seller path like a real low-latency system instead of a slow background loop:

- dispatch faster
- poll less lazily
- drain commands fully
- distribute work fairly
- make sub-second dispatch produce unique events

That combination moved the app from “single-digit to double-digit jobs per minute” in the baseline cases to “hundreds of jobs per minute” in the optimized cases, while preserving the NIP-28-targeted buyer/seller model.
