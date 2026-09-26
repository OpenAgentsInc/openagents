# Verification gates and what they cost

`./scripts/verify-rust.sh` runs targeted development checks by default. The full
workspace matrix requires `--release` and is reserved for full releases. It
must never hold ordinary issue work, integration, commits, or pushes, and
independent issues continue while release checks run. Direct focused Cargo
checks are valid; choose coverage for the changed behavior.

There are no git hooks or GitHub workflows. Every recorded script invocation
leaves a `run.json` and phase logs under `.coder/verification/<run-id>/`.
A scoped record is labeled `partial` to describe its coverage, not to require
more checks before day-to-day work can ship. See the current
[verification policy](../../verification.md).

The measurements below are historical; they explain how full runs accumulated
cost. One September 26 standard full run took 1003.7 seconds, about 17 minutes.
The current development default runs only changed-package formatting, Clippy,
and tests. It does not include PostgreSQL or duplicate the feature matrix.

## The gate itself

- `b64585ce0c` (2026-09-20, AtlantisPleb) — "build: pin the Rust gate
  and centralize workspace package policy" created `verify-rust.sh` so
  verification is one pinned command instead of ad-hoc invocations.
- `db5c0badf0f` (2026-09-21, AtlantisPleb) — "Split the manual gate
  into scoped, recorded phases" gave it its current shape: every phase
  is individually selectable with `--phases`, skippable, and recorded.

## Phases, in order

| Phase | What it checks | Median | Worst recorded | Introduced |
|---|---|---|---|---|
| preflight | toolchain, disk, git | ~0s | ~0s | `b64585ce0c` |
| gate-tooling | the gate's own Python tests | ~0s | ~0s | `db5c0badf0f` |
| artifacts | kev artifact-fetch logic | ~0s | ~0s | `db5c0badf0f` |
| delegation | delegation evidence scripts | ~0s | ~0s | `db5c0badf0f` |
| backup | backup collection scripts | ~0s | ~0s | `db5c0badf0f` |
| fmt | `cargo fmt --all --check` | ~0s | ~0s | `b64585ce0c` |
| clippy | `cargo clippy --all-targets -D warnings` | 6s | 1.3m | `b64585ce0c` |
| clippy-features | the same under the feature matrix | 6s | 48s | `db5c0badf0f` |
| tests | `cargo test` for the workspace | 48s | 86m | `b64585ce0c` |
| tests-features | `cargo test` under the feature matrix | 24s | 108m | `db5c0badf0f` |
| rust-1.95 | `cargo +1.95.0 check` (workspace floor) | 36s | 36s | `db5c0badf0f` |
| rust-1.94 | `cargo +1.94.0 check -p kev` (kev floor) | 18s | 18s | `db5c0badf0f` |
| deps | `cargo deny` policy; skipped when cargo-deny is absent | ~0s | ~0s | `db5c0badf0f` |
| postgres | relay acceptance on a real PostgreSQL | 1m | 1m | `db5c0badf0f` |
| metal-clippy, metal-tests | kev `metal` feature on Apple silicon | skipped | — | `b64585ce0c` |
| soak | relay soak; runs only under `--phases soak` | opt-in | — | `b64585ce0c` |

## The slow part: kev conformance

The two `cargo test` phases carried the kev conformance battery,
`crates/kev/tests/conformance.rs`, introduced by `2d8f237291`
(2026-09-19, AtlantisPleb, "kev: packed encoding, block-causal mask,
candle backbone, LoRA, pointer head (#9340 #9341 #9342)"). It exists to
prove the Rust port reproduces the Python reference bit-for-bit: golden
probabilities plus packed-encoding, isolation, permutation, forgery,
and option-isolation probes, run once per committed variant
(`kev-0.5b`, `kev-4b`, `kev-8b`).

That proof loads each variant's weights and replays it on CPU: roughly
twenty minutes per variant, every core the machine has, inside both
test phases — the 86-minute and 108-minute worsts above.

As of `d2307565bb` (2026-09-22) the battery runs only under
`KEV_CONFORMANCE=1`; nothing automatic sets that variable, and
`KEV_VARIANT=<id>` narrows an enabled run to one variant. The gate
documented the opt-in in `docs/verification.md` in the same commit.

## Timing-sensitive tests

Four workspace tests fail under load rather than on logic. Each passes
in isolation and flakes when the machine is saturated — which a
concurrent gate guarantees. They exist to check real behavior:

- `delegate::tests::a_timed_out_delegation_ends_its_descendants`
  (`a96845c40d`, 2026-09-19) — a deadline kills a delegate's whole
  process group. Flakes on process-reaping timing under fd pressure.
- `delegate::tests::the_fan_out_is_concurrent_under_its_bound`
  (`f2a4bc79cb`, 2026-09-19) — six delegations overlap up to the bound.
  Flakes when scheduling delays shrink the measured overlap.
- `worktree::tests::simultaneous_additions_and_removals_keep_all_answers`
  (`6160c140ab`, 2026-09-19) — concurrent worktree metadata writes keep
  every answer. Flakes on fd pressure.
- `kev/tests/serve.rs::jev_client_round_trips_all_types`
  (`eb8e4894fe`, 2026-09-19) — the kev-serve HTTP contract round-trips
  every question type. Its internal 10-second deadline expires under
  concurrent load; standalone it finishes in 58 seconds.

## What actually costs time

1. Kev conformance, until `d2307565bb` made it opt-in — about 95% of a
   full gate's wall time.
2. Cold builds — the first `cargo test` or `cargo clippy` after a
   dependency change is minutes; incremental runs are seconds.
3. Flake retries — one timing-sensitive failure fails a whole test
   phase and the phase reruns from scratch.

Everything else in the gate is noise: formatting, clippy, the toolchain
floors, dependency policy, and the scripted checks complete in seconds
to about a minute each.
