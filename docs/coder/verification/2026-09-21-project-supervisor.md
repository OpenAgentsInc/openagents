# Project supervisor verification, 2026-09-21

The project supervisor's first integrated revision is
`b5a70217515a33bb91771fea7aaade4b72209551`. It includes the separately completed
#9476 work from main. The supervisor excluded #9476 from every assignment.

The full `./scripts/verify-rust.sh` gate passed: formatting, default and runtime
feature Clippy/tests, Rust 1.95 workspace compilation, Rust 1.94 Kev compilation,
dependency policy, and disposable PostgreSQL acceptance. Metal and the long relay
soak were not run. [Verification data](2026-09-21-project-supervisor/verification.json)
records each phase and the retained log digest. The host used a separate Cargo
target directory, four build jobs, and two test threads.

## Scheduling comparisons

The [deterministic simulation](2026-09-21-project-supervisor/simulation.json)
compares the same 30-task catalog with dependencies, path conflicts, resource
limits, and quiet-host work. Every task completed in both policies. At widths
1, 2, 4, 6, 8, and 10, fixed waves took 180, 121, 80, 66, 65, and 61 simulated
ticks; refill took 180, 101, 61, 51, 49, and 49. These are declared estimates,
not measured Devin speedups.

The [process benchmark](2026-09-21-project-supervisor/process-benchmark.json)
runs bounded fixture subprocesses through the real delegator and filesystem
boundary. Each run contains one 400 ms sleeper and eleven 35 ms sleepers. Three
trials per policy and width produced 432 accepted fixture results across 36
runs. Observed process counts stayed within each configured bound.

| Width | Fixed-wave median | Refill median |
| --- | ---: | ---: |
| 1 | 1,049 ms | 1,043 ms |
| 2 | 728 ms | 528 ms |
| 4 | 550 ms | 428 ms |
| 6 | 492 ms | 429 ms |
| 8 | 495 ms | 431 ms |
| 10 | 493 ms | 434 ms |

This workload has little additional benefit above four slots. These measurements
come from a shared workstation and fixture processes; they establish neither
Devin account limits nor an optimal coding-agent count. The approved local Devin
manifest remains capped at six. Reproduce with:

```sh
cargo run -p coder-scheduler --bin scheduler-sim
cargo run -p coder-project --example refill-benchmark
```

## Live project queue

The approved GitHub adapter read all 29 Project 16 items. A protected queue used
four total executor slots, two reserved externally, and two available to managed
read-only Devins. Three prepared tasks exercised program authority, effect
metadata, and tracker inspection. Native prerequisites were checked before
admission. The third task started while an earlier task was still active.

The first run answered all three tasks, but two expected-text checks failed:
the prompts requested facts plus a marker while the checker expected only the
marker. Those results were not accepted. The corrected prompts put facts before
an exact `Final answer:` line. All three tasks then answered and passed their
text checks, with delegation durations of 23,885, 26,952, and 24,979 ms. These are
functional checks performed while the manual gate was running, not quiet-host
throughput measurements.

The host inspected the evidence and accepted the three smoke tasks through
protected control records. Restart retained exactly three attempts and three
completed tasks; it did not dispatch them again. Acceptance applied to these
smoke tasks, not to their containing GitHub issues. The raw ATIF logs, assignments,
and ledgers remain in protected operator state; the published record contains
identities, outcomes, durations, and trace digests.

## Implementation delegation and limits

A live writing Devin produced and committed the scheduler core. Two initial
implementation jobs timed out after 30 minutes, leaving authority and tracker
drafts. Two narrower follow-ups also timed out after 15 minutes; one left useful
tracker hardening and the other left no patch. The supervisor preserved, reviewed,
repaired, and tested the drafts before integration. None of those timeouts counted
as successful task completion.

This establishes bounded execution, completion-driven refill, durable review,
and safe refusal/recovery foundations. It does not establish unattended
implementation success. Keep implementation tasks small, retain partial work,
limit review backlog, and require independent verification before accepting an
issue. Full program recovery, measured semantic admission, automatic integration,
and whole-run spending guarantees remain open work.
