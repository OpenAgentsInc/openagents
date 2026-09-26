# Out-of-sample study tooling

Runs and reports the [pre-registered out-of-sample study](../../../../docs/terminal-bench/2026-09-26-out-of-sample-study.md)
(#9683). Results go in
[2026-09-26-out-of-sample-study-results.md](../../../../docs/terminal-bench/2026-09-26-out-of-sample-study-results.md).
Standard library only, Python 3.9 or later.

| File | What it does |
| --- | --- |
| `confirm.py` | Watches a round's outcomes and queues each passing held-out task's confirmation runs, once. |
| `report.py` | Builds a round's results tables (Markdown and JSON) under the win rules, for the TB4 study or, with `--study tb21`, the TB2.1 knowledge-off study. |
| `study.py` | Shared: whitelisted collection, pools, the Fable reference, parsing, and the rules. Run as a script, it prints a round's collection as JSON. |
| `test_study.py` | Fixture tests with a fake Microcoder binary. Nothing real runs. |

## What the tools read

Held-out runs may be read only through their outcome lines
(see the pre-registration's rules between rounds). `study.collect()`
enforces this. From the round directory it takes the outcome lines, the
`<round>.meta`, `queue-<round>.txt`, `burned.txt`, and `notes.txt`. From each
log it takes only the record path on a `Record:` line among the last 8 lines.
If there's no such line, it matches the record directory by name and start
time. From each `summary.json` it takes only reward, why the reward is
unknown, steps, seconds, cost fields, how the run ended, cost basis, provider,
model, kb, and the IDs and digests of the entries used. For a record whose
cost is partly unknown and that has no `usd_upper` (made before Microcoder
recorded the bound), it also reads `events.jsonl`, but keeps only the
numeric fields of each unpriced model step: step, prompt size in bytes,
tokens in and out, known dollars, and milliseconds. It parses only
`generated` lines and keeps no text from them. It never reads transcripts,
event text, verifier output, test results, or model reasoning. The fixture
tests plant secrets in all of those places and check that none come
through.

## Commands

On the execution host (coderos-4080), from an up-to-date checkout:

```sh
cd ~/openagents/bench/terminal-bench/studies/2026-09-26-out-of-sample

# What would be queued now. Starts nothing and writes nothing.
python3 confirm.py --round r2 --dry-run

# Watch until the screen driver is gone and every confirmation run is done.
# At most 3 confirmation runs at a time. Leave it in a tmux or screen session.
python3 confirm.py --round r2

# Queue what's eligible now, wait for those runs, then exit.
python3 confirm.py --round r2 --once

# The results tables: Markdown, then '---JSON---', then the JSON report.
python3 report.py --round r2
python3 report.py --round r2 --format md
python3 report.py --round r2 --format json --json-out /tmp/r2-report.json

# The TB2.1 knowledge-off study's tables (round t1).
python3 report.py --study tb21 --round t1 --format md
```

From another machine, `report.py` collects over ssh. It pipes `study.py` to
the host's `python3`, so the host needs no checkout:

```sh
python3 report.py --round r2 --host coderos-4080 --format md
python3 report.py --round r2 --host coderos-4080 --save-collected r2.json   # keep the collection
python3 report.py --collected r2.json                                      # rebuild offline
```

Tests:

```sh
python3 -m unittest -v test_study
```

## confirm.py

On each pass (every 60 s by default, `--poll`), `confirm.py` finds the tasks
that are due. A task is due when:

- it's in the held-out pool (parsed from the pre-registration and checked
  against its SHA-256);
- its screen passed, meaning its first knowledge-on run in the round with a
  graded result had reward ≥ 1;
- it isn't listed `BURNED <task>:` in `burned.txt`, and it isn't listed
  `UNGRADEABLE <task>:` in `notes.txt`;
- it hasn't been queued before. `<round>/confirm-state.json` records every
  queued task. As an extra guard, a task that already has knowledge-off logs
  or 3 knowledge-on logs in the round is skipped.

A due task gets 5 runs, queued in this order: off, on, off, on, off. With the
screen, that makes 3 knowledge-on runs and 3 knowledge-off runs. Each run
uses:

- the binary, `--max-steps`, `--max-usd`, and `OPENAGENTS_KNOWLEDGE` from the
  round's screen script;
- the task's `--max-minutes` from the round's queue;
- `--kb candidates` for on runs and `--kb off` for off runs, as the screen
  script sets them;
- the extra arguments from the running driver's command line (`ps`), for
  example `--provider openrouter --model openai/gpt-6-luna`.

The driver's command line is saved in the state file the first time a task is
queued. If the driver isn't running, the extra arguments come from, in order:
`--driver-args`; a `--script` that passes none (`screen.sh`); or the provider
and model in the round's records. They must agree with the provider in
`<round>.meta`.

Each run writes `<round>/<task>.<arm>.<epoch-ms>.log` and appends to
`<round>/outcomes.txt` in the screen script's format: the timestamp, task,
arm, `rc=`, and the log's last line matching the script's pattern, with
` log=<name>` added at the end. A lock file stops a second `confirm.py` from
running on the same round.

`confirm.py` doesn't replace a confirmation run that ends in a fault. The
report shows such a task as short of runs. It also doesn't queue anything for
the Fable-fails pool, where the pre-registration asks for a second pass to
confirm a first one.

## report.py and the rules it applies

- **Fable 5.1 low reference.** Taken from `reference/fable-5.1-replays.json`:
  trials with effort `low` and reward ≥ 1. "Cheapest" is the lowest
  `cost_usd`. "Fastest" is the shortest wall clock (`finished_at` minus
  `started_at`), the same measure `gym runs` uses. For 5 tasks this differs
  from the pre-registration's table (`atrx-vep-crispr`, `freecad-spring-clip`,
  `intrastat-meldung`, `retro-console-soc`, `wdm-design`). The report lists
  both values, and its tables use the replays.
- **Cost win.** A pass (reward ≥ 1) on a held-out task whose total cost
  (model, Jev, and embeddings) is below Fable's cheapest winning run, with
  no unknown part. A run whose cost is partly unknown (a non-empty
  `cost_unknown`) also counts when its upper bound is below the bar. The
  bound is `outcome.usd_upper` when Microcoder recorded it, labelled
  "(upper bound)". For an older record without it, the bound is rebuilt:
  the known dollars plus, for each failed attempt of each unpriced model
  step, the step's prompt bytes plus 16,384 bytes of instructions and tool
  declaration, counted as one token a byte plus 4,096 tokens, and the
  128,000-token output cap, at list price (long-context rates above
  272,000 input tokens). That is labelled "(upper bound, reconstructed)".
  A Jev call the API refused with an error status (such as HTTP 402, out
  of credit) cost nothing and adds $0. Any other unpriced Jev call in an
  older record, an unpriced embeddings call, an unpriced model, or a billed
  provider has no bound: the cost shows as "unknown" and never counts.
  (Newer Microcoder records bound Jev calls too: each of the client's 3
  attempts at one token per byte of state and questions plus 4,096 tokens,
  at $0.042 per million.)
- **TB2.1 study (`--study tb21`).** The [TB2.1 knowledge-off
  study](../../../../docs/terminal-bench/2026-09-26-tb21-oos-study.md):
  the 65 pre-registered tasks, checked against their digest. The bar is
  Fable 5 xhigh's `usd_per_trial` from `reference/tb21-dev-set.json`. A cost
  win is a pass whose cost, or upper bound, is under that bar, by the same
  cost rules. A task is confirmed when at least 2 of its first 3 graded runs
  are cost wins. The report also gives each run's cost as a multiple of
  the bar (marked `≤` when taken from an upper bound) and Fable's mean agent
  time. Sweep tasks show as outside the study. The collection is the same
  whitelisted one, so the round's logs need the same `<task>.<arm>.<ms>.log`
  names for records to be matched; an outcome line with no record has no
  bound.
- **Time win.** A cost win that also finishes faster than Fable's fastest
  winning run.
- **Confirmed out-of-sample win.** At least 2 of the task's first 3
  knowledge-on runs with a graded result are cost wins. The knowledge-off arm
  is shown next to it. When at least 2 of its 3 runs are also cost wins, the
  report says the loop gets the credit.
- **Burned tasks** are labelled in-sample and never count.
- **Earlier harnesses.** Tasks studied by them are marked † (the three with
  retained winning traces, and those in `tuned-lexicon.json`).
- **Fable-fails pool.** Reported separately. One pass shows as needing a
  second pass. Two passes show as beating Fable outright.
- **Faults.** Provider faults (bad replies with an HTTP 429 or 5xx, or a
  usage or rate limit), ungradeable runs (reward unknown), Compose refusals,
  crashes, and interruptions have their own table and are never results.
  Runs with no outcome line yet are listed as running.
- **Cost basis.** Taken from `summary.json`. Round 1 records don't have it,
  so it's inferred from the outcome line: `billed` if the line says billed,
  otherwise `list_price`.
