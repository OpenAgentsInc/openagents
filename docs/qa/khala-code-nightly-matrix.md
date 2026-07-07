# Khala Code QA Nightly Matrix

Status: implementation note for ROADMAP_QA Q1.1 / issue #8012, Q1.3 /
issue #8014, Q1.4 / issue #8015, Q1.5 / issue #8016, and Q2.5 /
issue #8021.

`bun run qa:nightly` is the owned-runner Tier-2 loop for the fully automated
Khala Code QA cycle. It does not use GitHub-hosted CI. The committed systemd
unit and timer live in `ops/owned-runner/khala-code-qa-nightly.service` and
`ops/owned-runner/khala-code-qa-nightly.timer`.

The nightly matrix runs, in order:

1. `bun run --cwd packages/khala-qa-harness test`
2. `bun run --cwd packages/behavior-contracts test`
3. `bun run --cwd packages/khala-qa-harness smoke:real-bridge`
4. `bun run --cwd clients/khala-code-desktop verify`
5. `bun run --cwd clients/khala-code-desktop smoke:part2-ui`
6. `bun run --cwd clients/khala-code-desktop smoke:cockpit-visual`
7. `bun run --cwd clients/khala-code-desktop smoke:composer-visual`
8. `bun src/monkey-night.ts --runs 16 --steps 64` from the harness package
9. `bun test src/model-based.test.ts` from the harness package
10. the desktop property tier: composer draft, ThreadItem projector, and
   transcript render properties

The behavior-contract layer (ROADMAP_QA §9d, `packages/behavior-contracts`)
rides this matrix twice: the Khala Code UX contract oracles, registry
validation, coverage, and doc-sync run inside the desktop `verify` step, and
the registry/coverage machinery's own suite runs as the dedicated
`behavior-contracts` step. Per-contract receipts and deviation alerts
(#8184) derive from these nightly runs; the eventual trigger-engine carrier
is ROADMAP_BACKGROUND_AGENTS WS-B. The matrix now emits
`behavior-contracts/behavior-contract-receipts.json`: one receipt per contract
with `checks`, `evidenceRefs`, and `checkedAt`. Receipts record evidence only;
they never flip registry state. The latest sweep summary is also copied into
`qa-nightly-report.json` as `behaviorContractRun` and into
`qa-status-surface.json` as `behaviorContracts`, which is the QA Swarm evidence
board lookup for "contract X was green on date Y".

The default monkey settings produce 1024 deterministic fixture actions, meeting
the Q1.1 >=1000 action floor. Override with `OA_QA_NIGHTLY_MONKEY_RUNS` and
`OA_QA_NIGHTLY_MONKEY_STEPS` only on the owned runner.

## Mobile (opt-in, macOS runner only)

With `OA_QA_NIGHTLY_INCLUDE_MOBILE=1`, the matrix appends one more step after the
ten above:

11. `bash clients/khala-mobile/scripts/signed-in-thread-smoke-run.sh` — the Khala
    Mobile `mobile-signed-in-thread-smoke` step (step id
    `mobile-signed-in-thread-smoke`).

This is the launched-app-smoke tier oracle for the behavior contract
`khala_mobile.platform.launched_app_interaction_smoke.v1`. It drives the
`clients/khala-mobile/.maestro/flows/SignedInThreadSmoke.yaml` flow on a booted
iOS **Release**-configuration simulator: the runner first resets the seeded
thread's turn state (so the composer renders its lane picker deterministically),
then auto-signs-in as the seeded public-safe account, opens the seeded thread,
asserts the lane picker (`Send with Claude`), and sends a message that renders in
the transcript. Green receipt:
`docs/khala-mobile/2026-07-07-signed-in-thread-smoke-receipt.md`.

**Preconditions (why it is opt-in, not default):** a booted iOS simulator with an
installed Release `KhalaCode.app`, `maestro` + a JDK 17 on `PATH`, and the seeded
public-safe Maestro creds at `~/work/.secrets/khala-maestro.env` (gitignored,
never committed). The headless Linux owned runner has none of these, so the step
is only appended when the flag is set on a macOS runner. The paired
receipt-asserting bun-test
(`clients/khala-mobile/tests/signed-in-thread-smoke-receipt.test.ts`) still runs
in the normal `test:khala-mobile` sweep on every platform and keeps the contract
covered even where the simulator step cannot run.

The status surface emitted by the run includes the Q2 latency budget catalog
from `qaMetrics`. The harness scenario `perf` oracle evaluates budgeted samples;
sampleless catalog rows remain inconclusive, while `latencyBudgets.trends`
compares any current samples against persisted prior nightly samples.

Each run writes:

- `qa-nightly-report.json`
- `qa-nightly-report.md`
- `qa-status-surface.json`
- `qa-status-surface.md`
- the per-contract receipt set under
  `behavior-contracts/behavior-contract-receipts.json`
- `latencyBudgetRun` inside `qa-nightly-report.json`, derived from current
  `qaMetrics` snapshots found under the run artifact directory
- one log per step under `logs/`
- the flake quarantine ledger under
  `quarantine/flake-quarantine-ledger.json`
- the per-run monkey coverage ledger under
  `monkey-night/monkey-night-coverage-ledger.json`
- the monkey memory/zombie oracle artifact under
  `monkey-night/monkey-night-memory-oracle.json`
- the merged coverage union under `coverage/coverage-union-ledger.json`
- the frontier report under `coverage/coverage-frontier-report.json`
- the explorer steering input under `coverage/coverage-frontier-steering.json`

If the matrix fails and `OA_QA_NIGHTLY_FILE_ISSUE=1` is set, the runner files a
strict bug issue through `gh issue create` with public-safe report refs and the
failed step IDs. Raw command logs stay in the owned-runner artifact directory and
must be redaction-reviewed before external publication.

If any enforced behavior-contract receipt fails and
`OA_QA_NIGHTLY_FILE_CONTRACT_DEVIATION_ISSUE=1` is set, the runner files a
strict bug with the contract id, the statement, failed checks, and the receipt
artifact ref. The generated issue body is
`qa-nightly-behavior-contract-deviation-issue.md` under the current run
artifact directory.

## Perf Trends And Regressions

The runner scans the current dated artifact directory for public-safe
`qaMetrics` snapshots, evaluates every Q2.2 budget, and stores the result in
the nightly report as `latencyBudgetRun`. The Q1.5 surface compares each current
budget with the latest previous persisted value for the same budget and marks
it as `no_samples`, `first_sample`, `flat`, `improved`, or `regressed`.

With `OA_QA_NIGHTLY_FILE_PERF_ISSUE=1`, any `regressed` budget files a
strict-form issue with the previous value, latest value, delta, threshold, and
offending sample refs. The generated issue body is
`qa-nightly-latency-budget-regression-issue.md` under the current run artifact
directory. Details live in
[`khala-code-perf-trend-regressions.md`](./khala-code-perf-trend-regressions.md).

## Flake Policy And Quarantine

Every intermittent failure is treated as a product or harness bug. The nightly
runs each step once. If the first attempt fails or times out, it retries that
same step exactly once. A pass on retry is recorded as `flaky`, the nightly
status stays failed, and the run writes a quarantine entry with both attempt log
refs plus any step artifact refs. This is deliberately not a green retry.

With `OA_QA_NIGHTLY_FILE_QUARANTINE_ISSUE=1`, any pass-after-fail entry files a
strict-form issue with the quarantine ledger and logs. The static tracked
quarantine list lives in `docs/qa/khala-code-flake-quarantine-ledger.json`.
Its first entry is the 2026-07-02 one-in-three desktop-suite single-test error
from the QA design doc, tracked to fix issue #8044.

## Coverage Union And Frontier

Each run scans the artifact root for prior per-run `*coverage-ledger.json`
files, skips previously merged `coverage-union-ledger.json` files, and writes a
fresh union ledger for the current nightly. If the current monkey run failed
before producing a ledger, the union falls back to historical ledgers or an
empty baseline so the frontier remains explicit instead of silently missing.

The monkey-night step also writes memory and shutdown-oracle evidence. The
nightly fails the `monkey-night` step if RSS or JS heap exceeds the Q2.4
budgets, if RSS after-run samples increase monotonically by at least 64 MiB
across the night, or if any driver shutdown reports an orphan process.
Implementation details live in
[`khala-code-memory-zombie-oracle.md`](./khala-code-memory-zombie-oracle.md).

The frontier report compares the union ledger with the seed-corpus manifest and
lists unvisited RPC methods, unexercised slash commands, unopened hotbar
panels, unwritten settings, unrendered ThreadItem variants, unclicked
selectors, and missing approval decision kinds. The steering input flattens
that frontier into refs such as `hotbarPanels:fleet` and
`rpcMethods:fleetRunStatus` for the seeded monkey, live explorer, and GEPA
policy lanes.

When the same frontier ref is still missing for seven consecutive dated
frontier reports, the nightly marks it in `zeroForAWeekIssueCandidates`. With
`OA_QA_NIGHTLY_FILE_COVERAGE_ISSUE=1`, the runner files a strict-form issue
with public-safe refs to the report, union ledger, frontier report, and steering
input.

## Status Surface

The owner-facing summary lives in `qa-status-surface.json` and
`qa-status-surface.md`. See
[`khala-code-qa-status-surface.md`](./khala-code-qa-status-surface.md) for the
schema, public-safety contract, coverage counts, step-duration trend semantics,
and live-tier status rules.

Key environment switches:

| Variable | Default | Purpose |
| --- | --- | --- |
| `OA_QA_NIGHTLY_ARTIFACT_DIR` | `var/qa-nightly` | Artifact root scanned for prior ledgers and frontiers. |
| `OA_QA_NIGHTLY_MONKEY_RUNS` | `16` | Number of monkey runs. |
| `OA_QA_NIGHTLY_MONKEY_STEPS` | `64` | Steps per monkey run. |
| `OA_QA_NIGHTLY_INCLUDE_MOBILE` | unset | Append the opt-in Khala Mobile SignedInThreadSmoke step (macOS runner with a booted iOS simulator + installed Release build only). |
| `OA_QA_NIGHTLY_STEP_TIMEOUT_MS` | `1800000` | Per-step timeout. |
| `OA_QA_NIGHTLY_FILE_ISSUE` | unset | File a strict-form issue on matrix failure. |
| `OA_QA_NIGHTLY_FILE_COVERAGE_ISSUE` | unset | File a strict-form issue when a frontier class stays zero for seven consecutive coverage days. |
| `OA_QA_NIGHTLY_FILE_PERF_ISSUE` | unset | File a strict-form issue when any latency budget regresses against the latest persisted prior sample. |
| `OA_QA_NIGHTLY_FILE_QUARANTINE_ISSUE` | unset | File a strict-form issue when a failed step passes on the single retry. |

The timer is scheduled for 07:17 UTC with a small randomized delay. Install on
the owned runner by copying both unit files into the systemd unit directory and
enabling the timer:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now khala-code-qa-nightly.timer
systemctl list-timers khala-code-qa-nightly.timer
```

The owned checkout path in the unit is `/srv/openagents/openagents`; adjust it
only on the runner, not in Git, unless the fleet standard path changes.
