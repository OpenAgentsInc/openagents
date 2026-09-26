# Gym

Gym is the measurement and evidence interface. `crates/gym` evaluates decision
doors against pinned suites, stores attributable records, and applies declared
comparison gates. Its terminal also reads Coder and Terminal-Bench evidence,
including saved head-to-head replay. The Terminal-Bench runner has its own
[status](../terminal-bench/README.md) and [runbook](../terminal-bench/runbook.md).

## Inspect coding runs

| Need | Guide |
| --- | --- |
| Browse recorded episodes | [Terminal interface](terminal-bench-tui.md), [CLI queries](terminal-bench-cli.md) |
| Compare full saved transcripts | [Head-to-head replay](head-to-head.md) |
| Understand ingestion and record limits | [Terminal-Bench data contract](terminal-bench.md) |
| Inspect raw retained files | [File viewer](retained-files.md) |
| Ask questions with checked citations | [Run analysis](run-analysis.md) |
| Publish a bounded evidence snapshot | [Published snapshots](published-snapshots.md) |

Replay follows recorded or explicitly estimated timestamps. Missing transcript
files and missing costs remain unavailable; selecting an indexed attempt does
not establish that all its artifacts are present. A replay does not execute
the recorded commands.

## Measure decision doors

| Need | Guide |
| --- | --- |
| Understand suite partitions and recorded results | [Measured records](measured-records.md), [run card](run-card.md) |
| Follow result provenance | [Ledger](ledger.md), [model identity](model-identity.md) |
| Interpret comparisons and acceptance | [Gate digests](gate-digests.md), [regression](regression.md) |
| Select a measured deployment | [Deployment from the store](deployment-from-store.md) |
| Read retained experiments | [Gym measurement index](measurements/README.md), [decision comparisons](../decision-models/measurements/README.md) |

These guides define what a metric, gate, or receipt proves. Model probabilities
are not automatically calibrated for a new workload, and a passing historical
gate does not admit a changed implementation. The
[optimization plan](../optimization/README.md) describes additional candidate
search and promotion machinery that is broader than this measurement core.

The [earlier migration survey](../history/2026-09-26-retired-gym-migration.md)
remains historical reference. The [master roadmap](../roadmap.md) owns future
direction; this index describes today's documentation boundaries.
