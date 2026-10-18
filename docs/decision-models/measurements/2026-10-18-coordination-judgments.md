# Coordination judgments: four question sets, first baseline

Question sets, each asked through the Decisions API (`POST /v1/decisions`,
model `openagents/decide`, Clef first) by the `judge` app tool and by the
landing queue (#12042, roadmap item 10,
[sdk-additions.md, S11](../../foreman/sdk-additions.md#s11-typed-judgments-for-coordination)):

| Set | Asks | Consumer |
| --- | --- | --- |
| [`run-progress`](../../../questions/run-progress.json) | Has this run moved its work forward since the last check-in? (Noul `progressed`, gate; Choice `activity`) | `judge`, the stall ladder |
| [`failure-on-main`](../../../questions/failure-on-main.json) | Does this branch's failing test also fail on `main`? (Noul `also_on_main`) | `judge`, landing checks |
| [`change-overlap`](../../../questions/change-overlap.json) | Do two changes waiting to land overlap? (Noul `overlap`) | `land_plan::overlap_judged`, behind `OPENAGENTS_LAND_OVERLAP_JUDGE=1` |
| [`ready-to-close`](../../../questions/ready-to-close.json) | Does what landed do what the issue's acceptance text asks? (Noul `code_complete`) | `judge` |

The sets are named for what they judge, and any agent may ask them; the
SDK reads them through `jev::question_set::coordination`.

## Method

- **Examples.** 90 labelled snapshots from 2026-10-16 and 2026-10-17, in
  [`2026-10-18-coordination/`](2026-10-18-coordination/): one JSONL file
  per set, each line `{id, date, source, state, label, why}`. They come
  from the coordinator's and Foreman's transcripts, #12012 and the other
  issues of those days, the
  [nostr-identity close timeline](../../audits/2026-10-17-nostr-identity-close-timeline.md),
  and commits on `main`. Each label is what actually happened afterwards,
  not a guess; `source` says where. Where a recorded run printed only
  `FAILED` lines, the failure message was rebuilt from the test's
  assertion at that commit, and `source` says so.
- **Held out.** The wording of every question was written before any
  example was collected and was not changed after the run. These are
  the first measurements; nothing was tuned on them.
- **The call.** `cargo run -p jev --example judge_baseline -- SET FILE`,
  one call per example against `https://openagents.com/api` with no key.
  A Noul is read at 0.5. Every answer and its receipt is in
  `SET.results.jsonl` beside the examples.
- **Who answered.** Clef (`clef-flash`, door `clef`) answered all 90
  calls; nothing fell back to Jev.
- **Run.** 2026-10-18, about 03:45 to 04:20 CDT.

## Results

| Set | Question | Right | False yes | False no | p50 latency |
| --- | --- | --- | --- | --- | --- |
| run-progress | `progressed` | 24/25 (96%) | 0/13 | 1/12 | 106 ms |
| run-progress | `activity` | 21/25 (84%) | | | |
| failure-on-main | `also_on_main` | 23/24 (96%) | 1/12 | 0/12 | 167 ms |
| change-overlap | `overlap`, all pairs | 18/25 (72%) | 0/12 | 7/13 | 89 ms |
| change-overlap | `overlap`, pairs the rule finds apart | 9/15 | 0/9 | 6/6 | |
| ready-to-close | `code_complete` | 13/16 (81%) | 3/7 | 0/9 | 113 ms |

"False yes" counts examples labelled no that were answered yes, over all
examples labelled no; "false no" is the reverse.

### run-progress

- The one gate error, rp-06, is a chat that ran tests, isolated a
  failure, and fixed it in the window: 0.48, just under 0.5. So it errs
  toward "stalled", whose cost is one message to a chat that was
  working. No run that had stalled was called progressing at any
  threshold from 0.5 up.
- At 0.3: 1 false yes and 0 false no; at 0.7: 0 and 2. The stall ladder
  reads the gate at 0.5 (provisional).
- `activity` errors are between neighbouring options: two loops (the same
  heartbeat posted twice, the same build-slot poll again and again)
  called `waiting_on_work`, one progressing chat called
  `waiting_on_work`, and rp-12, a coordinator that took new steps while
  waiting on its chats, called `progressing`. The labeller already marked
  rp-12 and the poll loops as ambiguous. Treat `activity` as a hint for
  which rung to use, never as the gate.

### failure-on-main

- The one error, fm-21, is the hardest case in the set: the same test
  name (`screens::every_live_page_is_a_screen`) failing on both, for
  different pages (main misses `/chat` and `/labs`, the branch its own
  new `/login/key` pages). It was answered 0.92, so a branch can hide a
  new failure behind a test that is already red on main.
- Code should therefore still compare the failure messages for any test
  the judgment calls "also on main", and show both messages when they
  differ. That check is deterministic over the test's name, which code has
  already picked.

### change-overlap

- On pairs where the landing plan's rule already finds an overlap, the
  judgment isn't asked. On the 15 pairs it finds apart, which are the only
  ones the flag sends, it caught none of the 6 real overlaps (highest
  0.35) and raised no false alarms (highest 0.17 on the 9 non-overlaps).
- All six misses are dependency edges the state doesn't show:
  - an `apps/openagents` crate that depends on another one in that
    workspace (ui on engine and rpc, mobile on sync);
  - an app crate with a path dependency on a root crate (engine on
    coder-access, mobile on lab-kit, ui on lab-ui and account).
  The state lists each side's packages, not what depends on them, so
  nothing in it shows the coupling.
- The gap is in the rule, not in the judgment: `land_plan` treats each
  `apps/openagents` crate as its own `dir:` folder with no edges. The fix
  is to read the app workspace's graph and app crates' path dependencies
  into `Graph`. Done in #12121: `land_plan::path_edges` adds every
  nested workspace's packages by `dir:` folder, with their path
  dependencies, and its regression tests put the six pairs above in order
  by rule. Of the 15 pairs the rule found apart, the 9 left are all real
  non-overlaps, so the next measurement needs new pairs coupled only
  through contracts.
- So `OPENAGENTS_LAND_OVERLAP_JUDGE` stays off. Turning it on today costs
  one call per new pair of code entries and changes nothing; it can only
  ever make two entries land in order, never side by side. Measure it
  again once the graph covers the app workspace, on pairs coupled only
  through contracts (a route and its caller, a fixture, a generated file).

### ready-to-close

- All three errors are "complete" answers on snapshots where follow-up
  fixes were still to come: #11965 at its first close (reopened three
  minutes later for the 10-minute status rule), and #12058 and #12056
  after their first commits. At 0.8, one remains (rc-03, 0.93).
- So a "complete" answer isn't enough to close an issue: it is a prompt
  to check the acceptance list item by item. A "not complete" answer was
  right every time (0 false no), so it is a safe reason to keep an issue
  open.

## How to use them

- Ask through `judge` (`set`, `state`). It returns the typed answers and a
  receipt (`questionSet`, `decision`, `model`, `door`, `receipt`,
  `answers`). Pass the receipt to `record_decision` in `judgments`; the
  decision log keeps it, and the next check-in prompt shows it.
- Code decides first where it can: a commit or push in the window is
  progress; a main run with no failures means the failure is the branch's
  own; a rule overlap needs no judgment.
- Thresholds are provisional, and each consumer owns its own. These are
  rates on 16 to 25 examples per set: repeat the run on new days before
  relying on a single figure.

## Rerun

```sh
export CARGO_TARGET_DIR=~/work/openagents-target-1
D=docs/decision-models/measurements/2026-10-18-coordination
for s in run-progress failure-on-main change-overlap ready-to-close; do
  cargo run -q -p jev --example judge_baseline -- "$s" "$D/$s.jsonl" > "$D/$s.results.jsonl"
done
```

`OPENAGENTS_DECISIONS_URL` points it at another gateway.
