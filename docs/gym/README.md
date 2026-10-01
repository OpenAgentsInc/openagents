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

## Test a plugin

Plugin tests (extension evals in the specifications) are built and live
([epic #9931](https://github.com/OpenAgentsInc/openagents/issues/9931)).
Start at [Plugins](../plugins/README.md) for what a plugin is and how to
write one.

| Need | Guide |
| --- | --- |
| Measure whether a plugin changes what Coder does, with and without it, and publish the result for others to check | [Extension evaluation](../extensions/evaluation.md) |
| Run, author, publish, or check a test set from a terminal | `openagents plugin test run \| init \| publish \| check` ([CLI guide](../cli/README.md#plugins-openagents-plugin), [live runner record](../extensions/measurements/2026-09-29-ext-eval-runner-live.md)) |
| Run a test set on our computers for the phone | [Hosted eval runner](../deployment/eval-runner.md) |
| Read the first live results and checks | [Hosted runner record](../extensions/measurements/2026-09-29-hosted-runner-live.md) |
| Make a plugin and its test set with the interview | [Authoring a suite](../extensions/evaluation.md#authoring-a-suite), [interview record](../extensions/measurements/2026-09-29-authoring-interview-live.md) |
| Earn and read XP for checks and adoptions | [Trainer XP: checks and adoption](../coder/guides/xp.md) |
| See results in the Verse | [The EVALS board](../verse/gym.md#the-evals-board-and-agents-comparing-notes) |

What is live:

- **In chat** (build 21): ask what's new in the Gym, test Project map, Code
  finder, or Test reader with and without the tool on the hosted runner,
  make a skill-shaped tool and its tests with the interview, **Add to the
  Gym**, check another trainer's result, and see your XP. The chat worker
  runs `chat-router-v2`
  ([measurement](../coder/measurements/2026-09-29-chat-router-v2.md)).
- **The hosted runner** on `coderos-4080` runs up to 8 tests, 3 runs, and 2
  sides per request, 3 runs per trainer per UTC day (checks don't count).
- **The first results**: each starter test set of six tests went from 2 of
  6 without its tool to 5 (Project map), 4 (Code finder), and 5 (Test
  reader) of 6 with it, **Better**, and another trainer's check confirmed
  each one.
- **Credit**: the XP referee signs `eval-check` awards (50 XP to the
  checker, 25 to the result's trainer, 25 to the test set's author) and
  `eval-adopt` awards. No tool has been adopted into Coder's defaults yet.
- **The Verse**: the Gym's EVALS board lists published results by test set
  with their checks.

An extension eval compares one extension's effect on Coder over a test
set; it is not a benchmark of Coder. Its verdict comes from the
`ext-eval-v2` gate (`crates/gym/gates/ext-eval-v2.json`), recorded like
every gate: **Better** only when more tests pass, the gain clears the
spread, and cost and time aren't materially worse. Results judged earlier
name `ext-eval-v1`.

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
