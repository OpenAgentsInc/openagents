# Gym leaderboard and trace viewer

Specified September 28, 2026. Status: the data contract and generator are
implemented in [`crates/gym-leaderboard`](../../crates/gym-leaderboard/); the
Grid user interface waits for the Gym port into the OpenAgents app.

The Gym building shows live run observations from a host the player
connected. It doesn't show what OpenAgents has measured and published:
which Terminal-Bench tasks Coder beat Fable on, for how much, with which
caveats, and what happened inside each attempt. This page specifies how
committed benchmark evidence becomes a typed, versioned leaderboard and a set
of scrubbed trace bundles, and how the Gym in the Grid shows them: boards,
per-task rows, a drill-in to one attempt, and a trace viewer that steps
through Jev's decision, the briefing, the agent's commands, and the
verifier's result.

## Goals and non-goals

Goals:

- Every number the Gym shows is computed from a committed evidence file by
  code, with the file's digest recorded beside it. No number is typed by
  hand.
- Every claim travels with its labels and caveats, so a screenshot can't
  separate a number from "in-sample" or "list price".
- A player can open any published attempt and step through it on a phone.
- A new study appears in the Gym by regenerating one publication, and a
  stale publication fails a test.

Non-goals:

- Live observation. The existing Gym connection ([Gym building](gym.md))
  keeps doing that, with its own grant and limits. The leaderboard is a
  second, read-only source that needs no connection code.
- Running, launching, or rerunning anything. Reading a leaderboard or a
  trace never executes a command or calls a model.
- A ranking across studies. Boards with different arms, references, or
  benchmarks are never pooled into one number.

## What exists today

| Piece | Where | What it does |
| --- | --- | --- |
| Gym building and boards | `crates/verse/src/gym.rs`, `crates/verse/src/hud/gym.rs` | Polls a connected host through `gym-bridge` every 5 s while the player is inside; shows `Run` rows (status, progress, cost, elapsed, up to 4 metric series). Being ported into the OpenAgents app's Grid now. |
| Bridge contract | `crates/gym-bridge/src/protocol.rs` | `openagents.gym-board.v1`: at most 128 KiB, 64 runs, 16 recipes. Encrypted Nostr transport, kind 3188. A board isn't an evaluation publication. |
| Measurement plane | `crates/gym` | `gym runs`, run cards, the `beats-winner` highlight rule, head-to-head replay. `gym publish` covers decision-model stores only; nothing is published for Terminal-Bench. |
| Verse replays | [Run replays](README.md#run-replays) | Plays a retained Microcoder run as visits to world landmarks, beside a ghost of Fable's cheapest winning run. Desktop only. |
| Evidence | `bench/terminal-bench/` | Experiment directories with `attempts.json`, study round reports, 631 retained trace directories (853 MB), and Microcoder run records. |
| Web | none in this repository | The openagents.com web app lives in its own repository. The retired Phoenix `/gym` pages are gone ([migration note](../history/2026-09-26-retired-gym-migration.md)). |

No Rust code reads an `attempts.json` before this work, and the retained
traces had no redaction step beyond `tbench retain`'s exact-value credential
scan, which only works on the host that ran the attempt.

## Result sets

| Board | Evidence | Traces | State |
| --- | --- | --- | --- |
| `tb4-fable-delegate-repro-9776`: Coder One's Jev-briefed Fable 5.1 low delegate on 14 TB4 tasks, two passes ([report](../terminal-bench/2026-09-27-fable-delegate-repro.md), #9776) | `experiments/2026-09-27-fable-delegate-repro/attempts.json`, `tasks.json` (frozen in e0414c3356) | 28 retained episodes, bundled | Generated |
| `tb21-oos-microcoder-9683`: Microcoder on 65 held-out TB2.1 tasks, knowledge off ([results](../terminal-bench/2026-09-26-tb21-oos-results.md), #9683) | `studies/2026-09-26-out-of-sample/t1-report.json`, 127 run records | 127 Microcoder run records, not yet bundled | Generated, without bundles |
| Fable delegate development on `fin-saccr-rwa` and three others, series 1 to 7 ([report](../terminal-bench/2026-09-27-fable-delegate.md), #9746) | `experiments/2026-09-27-fable-delegate/attempts*.json` (three row shapes) | 13 retained episodes | Next |
| TB4 Microcoder knowledge-assisted wins ([showcase](../coder/beat-fable-showcase.md)): the "one shared fact" result | `microcoder-runs/coderos-4080`, the `beats-winner` rule in `crates/gym` | Microcoder run records | Next |
| TB4 prospective out-of-sample study ([results](../terminal-bench/2026-09-26-out-of-sample-study-results.md)): no held-out pass yet | the study's round reports | Microcoder run records | Next; a negative board is still a board |
| Reference leaderboards | `reference/tb4-leaderboard.json`, `reference/tb21-leaderboard.json` | none | Next, labeled as a dated Harbor Hub snapshot |

The essay [Cheapest verified passes](../coder/cheapest-verified-passes.md)
cites the first two boards and the showcase. Each of its numbers is a
headline or a caveat that the generator computes: 30 confirmed wins, the
2.9% median pass cost, 31 of 65 first runs against Fable 5 xhigh's 299 of
325 trials, and 4 of 28 attempts in the reproduction.

## Data contract

The types are in
[`crates/gym-leaderboard/src/contract.rs`](../../crates/gym-leaderboard/src/contract.rs).
A publication is a directory, `bench/terminal-bench/published/`, holding
three kinds of file:

| File | Schema | Bound |
| --- | --- | --- |
| `leaderboard.v1.json` | `openagents.gym.leaderboard.v1` | 512 KiB (216 KiB today) |
| `traces/<board>/<attempt>.json` | `openagents.gym.trace-bundle.v1` | 256 KiB each (112 KiB largest today) |
| `index.json` | `openagents.gym.leaderboard-index.v1` | Append-only list of publications |

### Leaderboard

`Leaderboard { schema, generator, digest, boards }`. `digest` is the
ATIF-rule digest (object keys sorted at every depth, then SHA-256) of
`boards`, the same rule `atif::digest` and the Gym use.

Each `Board` has:

- **Identity:** `id`, `title`, `benchmark` (name and version), and `kind`,
  which names the beat rule: `beat_cheapest_and_fastest_win` or
  `cost_below_reference_per_trial`.
- **Claim:** `question` (from the report) and `headline`, one sentence the
  code builds from the tallies.
- **Provenance:** GitHub issues, the report path, the commit that froze the
  inputs when there was one, and `evidence`: every file read, with its
  SHA-256 and size.
- **Subject and reference:** the agent, arm, model, effort, and binary
  identity; the reference's name, its rule in words, and how its conditions
  differ.
- **Labels and caveats:** board-wide `labels` (see
  [presentation rules](#presentation-rules)) and `caveats`, each a code and
  a sentence whose numbers are computed.
- **Tallies:** `totals` and named `splits` (each pass; tasks with and
  without their own knowledge; first runs and confirmations), each with
  attempts, passes, beats, faults, and unknown-cost counts. Faults aren't
  results and aren't in `attempts`.
- **Spend:** reported dollars and, separately, the estimated lower or upper
  bound for attempts whose cost is unknown.
- **Tasks:** one `TaskRow` per task: the bar (cost, time, the reference
  trials that set them, the deadline, the reference's own pass count), the
  task's knowledge (`own`, `other_tasks_only`, or `off`), its attempt IDs,
  passes, beats, and a status: `beat`, `confirmed`, `not_confirmed`,
  `passed_without_beat`, or `never_passed`.
- **Attempts:** one `Attempt` per graded attempt: reward, whole-trial
  seconds and phases, `cost` (`reported` or `unknown` with bounds), the
  cost and time ratios against the bar, `beat`, `misses` (why it isn't a
  beat, in rule order: `failed`, `cost_unknown`, `cost`, `time`), labels,
  how it ended, a Jev summary, the verifier's summary and failed tests,
  and a `trace` reference (path, SHA-256, bytes) when a bundle exists.

### Trace bundle

`TraceBundle` is one attempt, readable without the leaderboard:

- `sources`: every retained file read, with its digest.
- `instruction`: the task as the agent received it.
- `jev`: the question set, thresholds, and every candidate (rank, ID,
  version, title, retrieval score, Jev's probability, kept or not, and
  whether it was written from this task) and every requirement (text,
  probability, flagged).
- `briefing`: the exact briefing the delegate received.
- `steps`: the episode on one clock, `at_ms` from its start: host steps,
  decision calls, the delegate starting, each thing it said, each command
  and its exit code and output, cumulative token usage, and the delegate's
  own end (absent when a deadline stopped it).
- `verifier`: reward, each test's status, and the output's tail.
- `outcome`: passed, beat, seconds, cost, the bar, and how it ended.
- `scrub`: redactions by rule, truncated fields, dropped steps, and the
  field bound used.

The steps come from the executor events in Coder One's
`trajectory.atif.json`, which carry timestamps, not from the raw delegate
stream, which has none and carries thinking signatures and the operator's
rate-limit windows.

### Versioning

A change that removes or renames a field, or changes a field's meaning, is a
new schema (`…v2`) and a new file name; readers keep reading `v1` until
they migrate. Adding an optional field is not a new version, and readers
ignore fields they don't know.

## Generation and provenance

```sh
cargo run -p gym-leaderboard -- build --commit "$(git rev-parse HEAD)"
cargo run -p gym-leaderboard -- check
```

`build` reads the committed evidence, writes the publication, and appends
the digest and commit to `index.json` when the digest is new. `check`
regenerates in memory and fails when a committed file differs or a bundle
matched a credential rule. Output is deterministic: the same evidence gives
the same bytes.

Each source adapter recomputes its study's verdict from each row's own
numbers and refuses to build when that disagrees with what the study
recorded:

- The #9776 adapter recomputes every beat (reward 1, known cost below the
  cheapest win, whole-trial time below the fastest win) and compares it
  with `beat_the_bar`; checks each row's bar against the frozen
  `tasks.json`; and checks its per-pass counts and known cost against the
  file's own `tallies`.
- The TB2.1 adapter recomputes every cost win and compares it with
  `cost_win`, and checks graded runs, passes, cost wins, and confirmed wins
  against the report's `totals` and each task's `cost_wins`.
- The bundler checks every file it reads against the digest `retention.json`
  recorded when the trace was retained.

A report's prose isn't parsed. The report is listed in `evidence` so a
reader can open it, and the tests pin the report's key numbers against the
generated board, so the two can't drift silently.

## Presentation rules

Every surface that shows a board (the Gym, a web page, a shared image)
follows these rules. The generator enforces the first five; the viewers
enforce the rest, and each viewer issue carries a test for them.

1. **Unknown is not zero.** An unknown cost is shown as "unknown", with its
   estimated bound labeled as a bound (`cost_bound`), and it can never beat.
2. **Denominators stay whole.** A rate is shown as "4 of 28", never "4
   beats". Failures, deadline-stopped attempts, and unknown costs stay in
   the denominator. Faults are shown separately.
3. **Labels travel with numbers.** A board's labels are shown on the board
   and on every attempt drawn from it: `pre_registered`,
   `knowledge_assisted` or `knowledge_off`, `in_sample` (a kept knowledge
   entry was written from earlier runs on the same task) or
   `out_of_sample`, `list_price`, `cost_bound`, `few_attempts`, and
   `reference_other_conditions`.
4. **Thin margins say so.** A beat whose cost or time margin is under 5% is
   labeled `thin_margin`, and the board's caveat names the margin (on
   #9776, `gsea-proteomics` by 1.9% and `sound-change-cascade` by 2.6%).
5. **Passes aren't pooled across runs that disagree.** Pass 1 and pass 2
   are separate splits, and a board states when they disagree ("0 of 14 in
   pass 1, 4 of 14 in pass 2").
6. **The headline is the board's.** A viewer shows `headline` verbatim and
   doesn't compose a stronger sentence from the numbers.
7. **Caveats are one tap away, never hidden.** The board panel shows the
   caveat count and at least the first caveat without scrolling; the
   `in_sample` and `thin_margin` caveats show on any beat's row.
8. **No cross-board ranking.** The boards list is ordered by publication,
   not by score, and no view sums or averages across boards.
9. **Reference conditions are shown beside the bar.** A bar is always shown
   with the reference's name and the `reference.conditions` sentence one
   tap away.
10. **Cost is list price.** Every dollar figure is labeled list price the
    first time it appears on a screen.

## Gym UX in the Grid

This section applies after the Gym port into the OpenAgents app's Grid
lands. It uses the port's building, board, and panel conventions and the
Grid's neutral white-on-black palette. It adds no new world geometry beyond
one board.

### Entry

A second board in the Gym, labeled **RESULTS**, stands beside the live
board. It needs no Gym connection: it works for every player. Tapping it
(within reach, unobstructed, the same rule as the live board) opens the
results panel. Entering the building starts the leaderboard fetch; leaving
cancels it. The live board's connection, grant, and polling are
unaffected.

### Screens

1. **Boards list.** One row per board: title, benchmark, the headline, and
   the board's labels as chips. Ordered as `leaderboard.v1.json` lists them.
   A footer shows the publication's digest (first 8 characters), its
   commit, and whether it's cached or current.
2. **Board.** The headline; the tallies as "passes / beats / attempts" with
   each split; spend (reported, then the bound, labeled); the caveats (first
   one shown, rest expandable); and the task table: task, bar (cost and
   time), each attempt as a compact cell (pass or fail, beat, cost or
   "unknown", time), and the task's status. Filters: all tasks, beats,
   never passed, own knowledge, no own knowledge.
3. **Attempt.** The attempt's numbers against its bar, with ratios and the
   misses in words ("reward 0, cost unknown, time"); phases (environment
   setup, agent, verifier); how it ended; Jev's summary (kept 2 of 12,
   flagged 2 of 4); the verifier summary and failed tests; labels; and
   **Open trace** when a bundle exists.
4. **Trace viewer.** A timeline scrubber over `steps` on the bundle's
   clock, with play, pause, step forward, and step back, and four tabs:
   - **Jev:** every candidate as a row with its probability as a bar,
     the keep threshold as a line, kept rows marked, and "own" marked; then
     every requirement with its probability and the flag threshold.
   - **Briefing:** the briefing text, scrollable.
   - **Agent:** the step list. A command shows its text and exit code; its
     output expands. Usage steps update a running token counter rather than
     taking rows. Truncated fields show "cut from N bytes".
   - **Verifier:** each test and its status, and the output tail.

   The header always shows the task, pass or fail, beat or not, cost (or
   "unknown"), time against the bar, and the labels.

A later step, not part of the first viewer, plays a bundle in the world as a
replay, using the landmark mapping in [Run replays](README.md#run-replays):
commands at the workbench, the Jev decision at the oracle, the verifier at
the proving ground.

### Mobile limits

- Fetch the leaderboard once per Gym entry, and a bundle only when the
  player opens its trace. At most one request in flight; a new one cancels
  the old.
- Never put a whole leaderboard or bundle in the per-frame render packet.
  The panel reads a view slice (one screen's rows or one page of steps)
  from Rust when the selection changes, well under the 1 MiB packet cap.
- The trace viewer pages steps (for example 50 per page) and renders
  expanded output lazily.
- Parsing, digest checks, filtering, and paging are Rust; native code draws
  lists and charts only, per the thin host boundary.
- VoiceOver and TalkBack read each row's numbers with their labels, and the
  **RESULTS** board has the same accessibility action as the live board.

## Serving, caching, and offline

The publication is static files. A host serves bytes; the app trusts only
digests.

- **Where from.** Version 1 reads the files from the public repository
  through `raw.githubusercontent.com`, pinned by commit: the app fetches
  `index.json` from `main`, takes the last publication's commit and digest,
  then fetches `leaderboard.v1.json` and bundles at that commit. The base
  URL is a build-time setting so a mirror (openagents.com, or a relay-hosted
  copy) can replace it without an app change. This adds no service and no
  deploy.
- **Verification.** The app recomputes the leaderboard's digest and refuses
  one that doesn't match the index; it checks each bundle's SHA-256 against
  its `TraceRef`. A mismatch shows "can't verify this publication" and
  keeps the cached one.
- **Caching.** Files are stored by digest in the app's cache directory. A
  cached leaderboard is shown at once, labeled with its age, while the
  index is checked. Bundles the player opened stay cached (a 16 MiB cap,
  least recently used first out).
- **Offline.** With no network, the cached publication is shown, labeled
  offline. Without a cache, the panel says the results need a connection
  once.
- **Later: signed publication.** A Nostr publication signed by the
  OpenAgents key, carrying the leaderboard digest and commit, lets a client
  check who published it, not only that the bytes match. NIP-EVAL's Gym
  profile says a live board isn't an evaluation publication; a results
  publication would need its own event shape, specified in NIP-EVAL before
  any code.

## Scrubbing and privacy

The retained traces are already public in this repository and passed
`tbench retain`'s credential scan. Publishing them to a phone adds a second,
host-independent layer in
[`scrub.rs`](../../crates/gym-leaderboard/src/scrub.rs):

- **Credential shapes** (private keys, Anthropic, OpenAI, OpenAgents,
  GitHub, Slack, AWS, and Google keys, Nostr secret keys, JWTs, and bearer
  tokens) are replaced with `[redacted:<rule>]`. Any credential match fails
  `check`, so a person inspects the retained trace before it's published.
- **Personal data** (home directories and email addresses) is redacted and
  counted but doesn't fail `check`. The #9776 bundles have 152 redacted
  emails, all synthetic customer data in `telecom-entity-resolution`, and
  two `/home/builder` container paths.
- **Left out entirely:** the raw delegate stream (thinking signatures, the
  operator's rate-limit utilization, tool and skill lists), produced files,
  setup logs, and credentials files, which the bundler never reads.
- **Bounds:** each text field is cut to a bound (4 KiB, halved until the
  bundle fits 256 KiB), keeping its head and tail; the briefing gets 24 KiB
  and the instruction 8 KiB. Every cut records its original size.

Task instructions and outputs include Terminal-Bench's canary line. The
tasks are public benchmark content already in this repository; the bundles
don't change who can read them.

## How new runs appear

1. A study lands its report and its machine-readable rows (for example an
   `attempts.json` from its `summarize.py`) and retains its traces.
2. If its rows have a shape an adapter already reads, add its descriptor
   (board ID, title, question, issues, paths); otherwise write an adapter
   with verdict and tally cross-checks.
3. Run `gym-leaderboard build --commit <rev>` and commit the publication
   with the report.
4. `the_committed_publication_matches_the_evidence` fails whenever evidence
   changes without regeneration, so a stale publication can't land quietly.
   The app picks up the new publication from the index on its next Gym
   entry.

To make step 2 data-only, experiments should emit a shared per-attempt row
schema (`openagents.gym.attempt-row.v1`) and a study descriptor, so a new
study needs a descriptor file rather than Rust.

## Web parity

An optional `/gym` page on openagents.com reads the same three files and
follows the same presentation rules. It belongs in the openagents.com web
app's repository, not this one. It can also serve as the mirror the app's
base URL points at.

## Verification

Implemented now (`cargo test -p gym-leaderboard`):

| Test | Checks |
| --- | --- |
| `the_delegate_board_says_what_the_9776_report_says` | 28 attempts, 13 passes, 4 beats, the per-pass and knowledge splits, the four beat IDs, the thin margins, unknown costs never beating, the never-passed tasks, and spend. |
| `the_tb21_board_says_what_the_essay_says` | 30 confirmed of 65, 83 of 127, 31 of 65 first runs, 2.9%, 48%, 92%, and the $4.45 bound. |
| `a_recorded_verdict_the_numbers_dont_support_refuses_to_build`, `tallies_that_disagree_with_the_rows_refuse_to_build` | A tampered verdict or tally refuses to build. |
| `a_trace_that_changed_after_retention_refuses_to_bundle` | A retained file that no longer matches its retention digest refuses to bundle. |
| `a_planted_credential_is_redacted_and_fails_the_check` | A planted token is redacted, counted, and fails `check`. |
| `every_bundle_fits_its_bound_and_matched_no_credential_rule`, `a_beat_bundle_steps_through_jev_the_delegate_and_the_verifier` | Bounds, and a beat's bundle has Jev, the briefing, the delegate's steps on a forward clock, and the verifier. |
| `generation_is_deterministic`, `the_committed_publication_matches_the_evidence` | Same bytes twice, and the committed files match. |

The results panel's view model is
[`crates/gym-leaderboard/src/view.rs`](../../crates/gym-leaderboard/src/view.rs):
a `Nav` (boards list, board with a filter, attempt, or trace with a tab,
a page of 50 steps, and a playhead) and `render`, which returns one
screen's `Page` with every figure formatted and labeled. Its tests
(`cargo test -p gym-leaderboard --lib view`) check presentation rules 1 to
10 one test each (`rule_1_unknown_cost_is_never_zero` through
`rule_10_first_dollar_figure_is_list_price`) on the committed publication,
and `every_page_fits_its_slice_bound` renders every board, filter,
attempt, bundle, tab, and page under 256 KiB, a quarter of the native
packet cap. Each Grid issue adds a capture of each screen on a simulator.

## Work breakdown

Epic [#9839](https://github.com/OpenAgentsInc/openagents/issues/9839).

| Issue | Piece | Depends on | When |
| --- | --- | --- | --- |
| #9840 | Contract and generator (this change) | none | Done |
| #9841 | Bundle TB2.1 Microcoder run records | #9840 | Now |
| #9842 | #9746 development board | #9840 | Now |
| #9843 | TB4 knowledge-assisted Microcoder board | #9840, #9841 | Now |
| #9844 | TB4 out-of-sample board and reference boards | #9840 | Now |
| #9845 | Shared attempt-row schema and study descriptor | #9840 | Now |
| #9846 | Rust client: fetch, verify, cache | #9840 | Now |
| #9848 | Rust view model with the presentation rules | #9840, #9846 | After the Gym port |
| #9849 | Grid **RESULTS** board and screens | #9846, #9848 | After the Gym port |
| #9850 | Grid trace viewer | #9849 | After the Gym port |
| #9851 | Play a trace as a world replay (optional) | #9850 | After the Gym port |
| #9852 | openagents.com `/gym` page (optional, other repository) | #9840 | Any time |
| #9853 | Signed results publication (optional) | #9840, #9846 | Later |
