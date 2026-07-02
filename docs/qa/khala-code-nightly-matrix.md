# Khala Code QA Nightly Matrix

Status: implementation note for ROADMAP_QA Q1.1 / issue #8012, Q1.3 /
issue #8014, Q1.4 / issue #8015, and Q1.5 / issue #8016.

`bun run qa:nightly` is the owned-runner Tier-2 loop for the fully automated
Khala Code QA cycle. It does not use GitHub-hosted CI. The committed systemd
unit and timer live in `ops/owned-runner/khala-code-qa-nightly.service` and
`ops/owned-runner/khala-code-qa-nightly.timer`.

The nightly matrix runs, in order:

1. `bun run --cwd packages/khala-qa-harness test`
2. `bun run --cwd clients/khala-code-desktop verify`
3. `bun run --cwd clients/khala-code-desktop smoke:part2-ui`
4. `bun run --cwd clients/khala-code-desktop smoke:cockpit-visual`
5. `bun run --cwd clients/khala-code-desktop smoke:composer-visual`
6. `bun src/monkey-night.ts --runs 16 --steps 64` from the harness package
7. `bun test src/model-based.test.ts` from the harness package
8. the desktop property tier: composer draft, ThreadItem projector, and
   transcript render properties

The default monkey settings produce 1024 deterministic fixture actions, meeting
the Q1.1 >=1000 action floor. Override with `OA_QA_NIGHTLY_MONKEY_RUNS` and
`OA_QA_NIGHTLY_MONKEY_STEPS` only on the owned runner.

The status surface emitted by the run includes the Q2 latency budget catalog
from `qaMetrics`. The harness scenario `perf` oracle evaluates budgeted samples;
sampleless catalog rows remain inconclusive until Q2.3/Q2.5 add real-run
samples, trends, and regression auto-issues.

Each run writes:

- `qa-nightly-report.json`
- `qa-nightly-report.md`
- `qa-status-surface.json`
- `qa-status-surface.md`
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
| `OA_QA_NIGHTLY_STEP_TIMEOUT_MS` | `1800000` | Per-step timeout. |
| `OA_QA_NIGHTLY_FILE_ISSUE` | unset | File a strict-form issue on matrix failure. |
| `OA_QA_NIGHTLY_FILE_COVERAGE_ISSUE` | unset | File a strict-form issue when a frontier class stays zero for seven consecutive coverage days. |
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
