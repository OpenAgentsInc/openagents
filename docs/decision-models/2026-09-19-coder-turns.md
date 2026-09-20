# Scoring the workload that actually exists

`crates/coder/src/classify.rs` is the only production decision surface in
this repository. It routes every agent turn through a decision model, and
until today nothing had scored it. Its question set and its thresholds were
chosen by reading.

This is the suite that scores it on real turns, the run that scored four
doors against it, and what the numbers say about which door `coder` should
route through.

The short version is not a door ranking.

**On 38 real decisions, routing through hosted Jev changed what the agent
would otherwise have done twice, and both changes were wrong.** Of the
16 turns, 14 routed to `Respond`, which is what an agent with no router
does; the other two routed to `Clarify` on turns the session shows were
answered. Of the 22 shell rounds, none routed to `Stop`, and the `damage`
probability never rose above 0.100 against a route at 0.700. Six of the
seven questions score no better than answering them with a constant.

## Where the states came from

Not from `docs/transcripts/`. The issue that asked for this work expected
that directory to hold agent sessions. It holds 293 machine transcriptions
of a **video** series, one per episode, and no agent turns at all.
`crates/coder` persists nothing either: the transcript lives in `Agent` for
the life of the process and goes when the terminal exits. There is no
recording of `coder` to harvest, and this suite does not claim one.

What exists is the local Claude Code session record for this repository —
JSON Lines, one file per session, every human turn, every reply, every tool
call and every result, in order. That is a coding agent working in this
repository for the same person who uses `coder`. The states are
reconstructed from it into the exact shapes `classify::state_of` and
`shell::state_of` build, under the caps the production code applies: twelve
transcript turns, 2,048 bytes of output per command, ten commands per round.

`crates/gym/suites/build_coder_turns_v1.py` is the harvester, and three
filters bound what it keeps:

- Only sessions whose working directory is this repository. The private
  sibling repositories on this machine hold twenty times more turns, and
  `AGENTS.md` forbids carrying their prompts here.
- No state matching a secret pattern, dropped whole rather than redacted.
  Fifty-one went that way.
- No state whose **commands** reach into a private sibling checkout or into
  Apple's licensed adapter toolkit. A session does that by accident — an
  agent runs `sed -n 80,260p` in another checkout and 180 lines of somebody
  else's deploy script land in the output — and 151 states went that way. A
  state whose prose merely names a sibling is kept: "look deeply thru
  `~/work/psionic`" is the operator describing work, and it is the most
  common shape of turn this workload has.

Those filters, and the one-repository rule above all, are why the suite is
95 states rather than several hundred.

## What the labels rest on

This is the property that makes the suite worth more than `support-v2`, and
the reason the schema grew a field for it.

Every item carries `label_source` and the rule that produced it, inside the
suite digest:

| Family | Evidence | Rule |
| --- | --- | --- |
| `action` | outcome | The agent asked a question and ran nothing before the next human turn is `clarify`; anything else it did is `respond`. |
| `needs_code` | outcome | The agent opened, changed or delegated work on repository files before the next human turn. |
| `risk` | outcome | 2 if the agent wrote a file or ran a writing command, 1 if it only read, 0 if it ran nothing. |
| `shell_outcome` | outcome | Every command exited 0 is `pass`; a failure the agent followed with more commands is `retry`; a failure it stopped on is `stop`. |
| `damage` | outcome | `yes` only where the session afterwards undoes, restores or reports harm from this round. |
| `progress` | author | A reading of where the turn sits in the work. |
| `useful` | author | A reading of whether the outputs move the task. |

An outcome label is not an opinion: the session says what the agent did
next, and the builder reads it mechanically. An author label is a reading,
written in `coder-turns-v1-judgments.json` before any door was asked and
committed so it can be argued with.

`progress` and `useful` could have been given mechanical rules. `progress`
could have read position in the session, and `useful` could have read exit
codes. Neither would have been an outcome. A session that stops is a person
walking away as often as it is work finished, and a compiler error is useful
while a shell-quoting error is not, which is exactly the distinction an exit
status cannot make. A rule over the state itself is a function of the input
rather than evidence about the world, and calling one an outcome would put
an ambiguity into every number downstream where nothing could see it.

`gym eval` and `gym compare` print a metrics row per evidence class, and
every result row carries `label_source`.

**The split is confounded here, and the record should say so.** Each family
carries one evidence class, so "author" and "the two hardest questions" are
the same items. The gap between the two rows below is not evidence that read
labels are noisier than outcome labels; it is evidence that these two
questions are harder, measured on labels that say what kind they are.
Separating the two would need a family labelled both ways, which this suite
does not have.

## The suite

`crates/gym/suites/coder-turns-v1.json`, digest `f619db3613b83e45`, 325
items over 95 states, asked as question set `coder-turns-v1`, digest
`4d0aaf39558294e7`.

| | Count |
| --- | --- |
| Turn states | 40 |
| Shell-round states | 55 |
| Items | 325 — 230 outcome, 95 author |
| Partitions | 130 calibration, 130 development, 65 locked and unread |
| State size | median 9.9 KB, p90 17.7 KB, largest 20.0 KB |

One state answers several questions, so 325 items rest on 95 states. Every
item of one state sits in one partition; a test enforces it, because the
same state on both sides of a fit is the leak the three-way partitioning
exists to stop.

The shell rounds are sampled. The archive holds 540 usable rounds and 95% of
them are clean passes, so every round that is not a pass is kept and the
passes are sampled by a hash of their own id — append-stable, because the
live session file grows while it is being read. The suite's `sampling` field
records the population counts, so the archive's rates are recoverable from
the per-class rates. The suite over-represents failure on purpose and says
so.

Those state sizes are the single most consequential number in the table, and
nothing in `support-v2` hints at them: a support-desk item is one sentence,
and a real `coder` state is ten kilobytes of transcript and command output.

## What the workload turned out to look like

This is the first thing the suite says, before any door is asked.

| Family | Labels | Majority class |
| --- | --- | --- |
| `action` | 39 `respond`, 1 `clarify` | 0.975 |
| `needs_code` | 38 yes, 2 no | 0.950 |
| `risk` | 35 `2`, 4 `1`, 1 `0` | 0.875 |
| `damage` | 55 no | 1.000 |
| `useful` | 49 yes, 6 no | 0.891 |
| `shell_outcome` | 29 pass, 25 retry, 1 stop (sampled; 514/25/1 in the archive) | 0.527 |
| `progress` | 17 `0`, 16 `1`, 7 `2` | 0.425 |

**Five of the seven questions are nearly constant on the workload they run
against.** A router answering `respond`, `yes`, `2` and `no` without asking
anything would be right 98%, 95%, 88% and 100% of the time. That is not a
fact about any door; it is a fact about the question set, and it is only
visible on real states. `support-v2` is balanced by construction, so nothing
scored on it could have shown it.

The two questions with real variety are `shell_outcome`, which gates the
shell loop, and `progress`, which gates nothing.

## What this instrument can detect

A suite whose baseline is already at the ceiling cannot host a comparison,
whatever it is asked. #9392 ran an experiment against a `routing` partition
scoring 0.975 and could not have registered a win at any strength. So the
headroom is published here, from the incumbent door, before any conclusion
is read off it.

Hosted Jev on the development partition: **0.65 over 130 items, 0.35 of
headroom**, which is six times the 0.056 two-sigma floor from
[`../lev/measurements/2026-09-19-seed-variance.md`](../lev/measurements/2026-09-19-seed-variance.md).
The instrument has room.

Per family it is uneven, and one family has none at all:

| Family | Items | Jev | Headroom | Constant answer |
| --- | --- | --- | --- | --- |
| `action` | 16 | 0.875 | 0.125 | 1.000 |
| `damage` | 22 | 1.000 | **0.000** | 1.000 |
| `needs_code` | 16 | 0.625 | 0.375 | 1.000 |
| `progress` | 16 | 0.438 | 0.562 | 0.438 |
| `risk` | 16 | 0.312 | 0.688 | 0.875 |
| `shell_outcome` | 22 | 0.773 | 0.227 | 0.545 |
| `useful` | 22 | 0.409 | 0.591 | 0.864 |

`damage` is at the ceiling: 22 of 22, and no door can beat it. Its value in
this suite is not accuracy but the distribution behind the answer, which is
what the `damage >= 0.7` route reads, and that is reported below.

Sixteen items is a small family and carries a binomial standard error near
0.12. Per-family numbers here are direction, not measurement. The pooled
figure over 130 items carries 0.042, and paired discordance counts between
doors carry less than either.

## What the workload does to the question set

The comparison that matters is not door against door. It is each question
against the constant that would replace it.

| Family | Jev | Constant | Difference |
| --- | --- | --- | --- |
| `shell_outcome` | 0.773 | 0.545 | **+0.228** |
| `damage` | 1.000 | 1.000 | 0.000 |
| `progress` | 0.438 | 0.438 | 0.000 |
| `action` | 0.875 | 1.000 | −0.125 |
| `needs_code` | 0.625 | 1.000 | −0.375 |
| `useful` | 0.409 | 0.864 | −0.455 |
| `risk` | 0.312 | 0.875 | **−0.563** |

**One question of seven beats the constant that would replace it.** Only
`shell_outcome`, at +0.228, clears the noise floor; `damage` and `progress`
tie their constants and four questions lose to theirs.

`risk` is the worst and the most instructive. The truth is `2` — the step
writes a file or could break a build — on 14 of 16 real turns, because that
is what a coding agent does. Jev answered `0`, "answer in prose; nothing
changes", on six of them. The rubric was written for a support desk; the
workload is a terminal that edits the repository.


## What the doors did

Four doors, one client, the development partition, 2026-09-19.

```text
# hosted Jev, the incumbent
gym eval --suite crates/gym/suites/coder-turns-v1.json \
    --partition development --jev \
    --record crates/gym/results/coder-turns-v1.jsonl

# Apple's on-device model, with and without the support-v2 adapter
./scripts/build-lev-bridge.sh
lev-serve --port 11470
lev-serve --port 11471 --adapter ~/code/lev-adapter-work/runs/lev-v1/lev.fmadapter
LEV_OS_BUILD=25E246 gym eval --suite crates/gym/suites/coder-turns-v1.json \
    --partition development --timeout 300 \
    --door lev-base=http://127.0.0.1:11470 \
    --door lev-adapted=http://127.0.0.1:11471 \
    --record crates/gym/results/coder-turns-v1.jsonl

# kev-8b, on the one family that gates a route
kev-serve --bundle-dir ~/work/kev-artifacts --port 8009
gym eval --suite crates/gym/suites/coder-turns-v1.json \
    --partition development --family action --timeout 300 \
    --door kev-8b=http://127.0.0.1:8009 \
    --record crates/gym/results/coder-turns-v1-kev.jsonl
```

390 rows in `crates/gym/results/coder-turns-v1.jsonl` and 16 in
`crates/gym/results/coder-turns-v1-kev.jsonl`, receipt-chained. kev-8b is in
its own chain because it ran beside the Lev pass and one store has one
writer; it was scored on `action` alone because at two minutes a call it
could not be afforded on the whole partition, which is part of the answer
rather than a gap in it.

`--timeout 300` is not decoration. The client's own ten seconds is right for
a caller in front of a user and wrong for a harness: a timeout produces no
row at all, so the items that drop out are the largest states, and a door's
score is quietly taken over the easy half of the suite. The first attempt at
this run lost its first item that way — a twenty-second answer on a
contended machine, recorded as nothing.

| Side | Accuracy | ECE | Brier | NLL | Confident errors | Scored | Refused | Median latency |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| jev (hosted) | 0.65 | 0.146 | 0.207 | 0.577 | 1 | 130 | 0 | 196 ms |
| lev-base | 0.74 | 0.145 | 0.175 | 1.276 | 3 | 95 | 35 | 13,154 ms |
| lev-adapted | 0.71 | 0.217 | 0.243 | 3.551 | 11 | 95 | 35 | 8,975 ms |
| kev-8b (`action` only) | 0.12 | 0.501 | 0.380 | 0.969 | 0 | 16 | 0 | 128,634 ms |

A refused item stays in the denominator, so the Lev accuracies are over the
95 items those doors answered and the 35 they declined are visible beside
them rather than folded away.

**The accuracy column decides nothing.** On the 95 items both answered,
paired:

| Comparison | Accuracy | Discordant | Delta | Paired SE | Sigma |
| --- | --- | --- | --- | --- | --- |
| lev-base against jev | 0.737 / 0.674 | 18 / 12 | +0.063 | 0.058 | 1.1 |
| lev-adapted against jev | 0.705 / 0.674 | 14 / 11 | +0.032 | 0.053 | 0.6 |
| lev-base against lev-adapted | 0.737 / 0.705 | 11 / 8 | +0.032 | 0.046 | 0.7 |

Every one is inside the noise. `decision-v1` reaches the same verdict from
the rows — `unverifiable`, the gain against twice its standard error — and
so does the adapter's own comparison. **The adapter that won thirteen points
on `support-v2` wins nothing here**, which is what an adapter fitted on
support-desk text does on a workload of shell output.

By what the labels rest on, which the pooled row hides:

| Side | Outcome | Author |
| --- | --- | --- |
| jev (hosted) | 0.74 over 92 | 0.42 over 38 |
| lev-base | 0.80 over 66 | 0.59 over 29 |
| lev-adapted | 0.83 over 66 | 0.41 over 29 |

The adapter's worst number is not its accuracy. On the author-labelled items
its ECE is 0.526 against the base model's 0.190 and its log loss is 8.217
against 1.535, with eight confident errors against one. It is wrong in the
same places and much more certain about it.

### What the doors could not read

This is the finding that decides the question, and no authored suite could
have produced it.

| State size | Items | Jev refused | Lev refused |
| --- | --- | --- | --- |
| under 2 KB | 31 | 0 | 3 |
| 2–6 KB | 24 | 0 | 0 |
| 6–12 KB | 44 | 0 | 4 |
| over 12 KB | 31 | 0 | **28** |

Lev refused 35 of 130 items: 32 `branch_too_long` and 3 `invalid_request`.
Both Lev doors refused exactly the same items, which is the context window
rather than the weights.

A real `coder` turn state has a median of 11.9 KB, because
`classify::state_of` carries twelve transcript turns and a turn that ran
shell commands carries 2 KB of output per command. **Apple's context window
is smaller than the state this repository's agent builds**, so Lev could
answer only half the turn-level questions — 8 of 16 on `action`, and the
same 8 on `needs_code`, `risk` and `progress`. It answered nearly every
shell round, because a shell state has a median of 2.8 KB.

## The deployment question

**`coder` should keep routing through hosted Jev, and should stop asking it
six of its seven questions.**

The panel, not accuracy alone:

| | jev (hosted) | lev-base | lev-adapted | kev-8b |
| --- | --- | --- | --- | --- |
| Can read the workload | 130 of 130 | 95 of 130 | 95 of 130 | 16 of 16 |
| Accuracy, items it answered | 0.65 | 0.74 | 0.71 | 0.12 |
| Against Jev, paired | — | 1.1σ | 0.6σ | — |
| Median latency | 196 ms | 13.2 s | 9.0 s | 128.6 s |
| p90 latency | 263 ms | 40.1 s | 28.3 s | 194.6 s |
| Stops the shell loop | 0 of 22 | **3 of 21** | 1 of 21 | — |
| …on a round that was clean | 0 of 12 | **2 of 11** | 1 of 11 | — |
| Halts a real turn | 0 of 16 | 0 of 8 | 0 of 8 | **14 of 16** |
| Marginal cost | metered | none | none | none |
| State leaves the machine | **all of it** | no | no | no |

Reading it door by door:

**kev-8b is disqualified twice.** It answered `none` — "no listed step fits"
— on 14 of 16 real turns, and under the routing table as it stood that
halted seven turns in eight. It took a median of 128 seconds to do it, on a
contended machine, in front of a turn the user is waiting for. Its 0.12 is
the lowest accuracy any door has scored on anything in this repository, and
it was measured on the checkpoint #9384 found to be the best of the four.

**Lev cannot serve this workload as the state stands.** It is the door with
everything else going for it — nothing leaves the machine, nothing is
metered, and on the items it answered it is six points better than Jev at
1.1 sigma, which is to say not measurably better. But it declines 27% of the
partition and half of every turn-level question, it takes 13 seconds when it
answers, and its `damage` estimate crosses the production stop threshold on
three rounds that damaged nothing. A decision model in front of every agent
turn that declines a quarter of them, costs 13 seconds, and stops the shell
loop on a clean round one time in six is not a router.

A refusal is not a halt, and that is worth saying: `Agent::classify` turns a
failed call into `Classified::Skipped` and generates unrouted, so Lev's 35
refusals cost a judgment rather than a turn. They cost the whole point of
having a router on those turns.

**The adapter is not the answer either.** It refuses the same items, it is
not measurably better than the base model on the rest, and it is
dramatically worse calibrated on the two hardest families. The thirteen
points it won on `support-v2` do not transfer.

**Hosted Jev is the only door that can serve the workload today.** 196 ms at
the median, 263 ms at the ninetieth, nothing refused, and it reads a
20 KB state as readily as a 400-byte one.

Its cost is the row nothing else in this table has: **every byte of the
state leaves the machine**, and for this workload the state is the
repository — task text, transcript, file contents, command output, whatever
the agent last ran. One pass over 130 items sent 1.1 MB. That is a different
privacy posture from sending a support-desk sentence, and it is the reason
the on-device door is worth the work even though it lost today.

### What would change the answer

The state, not the model. Lev answered every state under 6 KB and refused 28
of the 31 over 12 KB. The turn state is large because `state_of` carries
twelve transcript turns including whole shell-output blocks — a bound chosen
for a conversation, not for a judgment about the next step.

A `classify` state trimmed to what the `action` question actually reads
would land under Apple's window, and then the door that is free, private and
already resident becomes a candidate on its own numbers. That is a measured
engineering target rather than a guess: the threshold is somewhere between
6 KB and 12 KB, and this suite can measure exactly where.

## The thresholds, checked

`classify.rs` holds four decisions that were made by reading. One is
confirmed, one is corrected here, one is a route that does not exist, and
one turns out to depend on the door.

### The argmax rules, with no confidence gate — confirmed

`route`'s doc comment argues that a Choice distribution sums to one, so
`none` is the escape hatch and no separate confidence floor is needed. This
is the first evidence either way, and it holds.

Jev's two wrong `action` answers came at 0.72 and 0.48; its correct ones ran
from 0.52 to 0.99, median 0.76. Lev's one wrong answer came at 0.62 and its
correct ones started at 0.62. What a floor would cost and buy, on Jev:

| Floor | Errors gated | Correct routes gated |
| --- | --- | --- |
| 0.5 | 1 of 2 | 0 of 14 |
| 0.6 | 1 of 2 | 2 of 14 |
| 0.7 | 1 of 2 | 6 of 14 |
| 0.8 | 2 of 2 | 8 of 14 |

A floor that catches both errors gates more than half the turns that routed
correctly. **Leave it alone.** One row is worth keeping rather than acting
on: Jev's second error is a coin flip, `clarify` 0.48 against `respond`
0.47, so a floor at 0.5 would have caught it for nothing on these items. Two
errors is not a sample, and a floor is the wrong instrument anyway while a
gated turn goes to `Halt`. If one is ever fitted, the fallback belongs at
`Respond`, fitted on the calibration partition and judged by a gate.

### `none` halted the turn — corrected

`route` used to send `Action::None` to `Halt`, which replies "I don't have a
confident next step for that" and generates nothing. Two measurements say
that is wrong, and `route` now sends it to `Respond`.

`none` is never the truth. Forty turns harvested from recorded sessions, and
every one of them had a next step that the agent took; the option exists for
a state that supports no action, and a coding turn is never that state.

And a door that misreads the state answers `none` freely. **kev-8b chose it
on 14 of 16 real turns**, which under the old table would have halted seven
turns in eight — on a workload where the right answer was `respond` every
time. Hosted Jev never chose it; Lev never chose it. The behavior the option
gates is therefore entirely a function of which door is behind it, which is
the opposite of what a routing table should be.

The consistency argument was already in the module. A classify call that
fails degrades to `Classified::Skipped` and the turn generates unrouted. A
judge answering `none` is in the same position as a judge that could not be
reached — it has no read — so it now gets the same treatment. `Halt` stays
for an answer that names nothing listed, which is a door breaking the
contract rather than a door with no read. The judgment is kept whole either
way, so the terminal still shows that `none` won.

### `retry` is a route that does not exist — wrong, not yet corrected

`ShellVerdict::route` returns three routes and `Agent::turn` reads two:

```rust
if route == ShellRoute::Stop || rounds >= shell::ROUNDS_MAX {
    final_only = true;
}
```

`Pass` and `Retry` are the same instruction to the loop. The question offers
the model a third option, the verdict line prints it, and nothing acts on
it.

That would be cosmetic if the option were rare. Jev chose `retry` on 13 of
22 real rounds, including 4 of the 12 rounds where every command exited 0;
lev-base chose it on 11 of 21. More than half the rounds this question is
asked about get an answer the agent cannot use.

This one is left as a finding rather than a fix, because both repairs are
product decisions with no evidence behind them yet: making `Retry` re-plan
changes what the agent does, and removing the option changes the question
text, which is a new question set and a new comparison.

### `damage >= 0.7` — safe on one door, not on another

This is why a threshold and its question have to be measured together, and
why measuring them on the door you ship is not optional.

| Door | `damage` on 21–22 harmless rounds | Median | p90 | Max | At or above 0.7 |
| --- | --- | --- | --- | --- | --- |
| jev (hosted) | 22 | 0.050 | 0.060 | 0.100 | 0 |
| lev-base | 21 | 0.000 | 0.750 | **1.000** | **3** |
| lev-adapted | 21 | 0.000 | 0.000 | 0.250 | 0 |

**On hosted Jev the route has 0.600 of headroom and never fires. On lev-base
it fires on 3 of 21 rounds, none of which damaged anything**, at a
probability of 1.000 on one of them. Combined with the choice's own `stop`,
production would have cut the loop short on 3 of 21 rounds, two of them
rounds where every command exited 0. A 14% false-fire rate is not a
threshold that needs
retuning for that door; it is a door whose `damage` estimate is not a
probability yet, exactly as `docs/decision-models/lev/calibration.md` says a
Lev number is not until a map is fitted for its family.

The other half of the threshold cannot be tested here at all. The archive
holds no round that damaged anything — the label rule looks for a later
undo, restore or report of harm and finds none in 540 rounds — so `damage`
is single-valued and nothing in this suite can say whether 0.7 catches a
real one. **The threshold keeps its value: 22 of 22 on the door `coder`
ships with is not evidence that it works, and 3 of 21 on a door it might
have shipped with is evidence that it would have hurt.**

### And the judgment as a whole changed almost nothing

Taken together, the shell verdict has exactly one effect: `Stop`. Hosted Jev
chose `stop` on 0 of 22 rounds and never put `damage` above 0.100, so **the
shell judgment changed nothing on 22 of 22 real rounds** at the price of one
decision-model request per round, up to three per turn.

The turn judgment did a little more and none of it was good: 14 of 16 turns
to `Respond`, which is what an unrouted agent does, and 2 to `Clarify` on
turns the session shows were answered.

## What to do next

In the order the measurements support:

1. **Stop asking the four questions that gate nothing and lose to a
   constant.** `needs_code`, `risk`, `progress` and `useful` are consumed by
   no decision, and on the real workload they score 0.625, 0.312, 0.438 and
   0.409 against constants of 1.000, 0.875, 0.438 and 0.864. Dropping them
   shrinks the request, the bill and the bytes that leave the machine, and
   costs nothing anything here can measure.
2. **Shrink the `classify` state** until it fits Apple's window, and
   re-measure Lev on it. The suite can find the boundary: everything under
   6 KB was answered, 28 of 31 over 12 KB was refused.
3. **Decide what `retry` means**, or take it out of the question.
4. **Fit a calibration map for `damage` on Lev before any on-device door
   reads that threshold.** Three false stops in 21 rounds is what an
   uncalibrated estimate behind a fixed threshold looks like.
5. **Harvest more turns.** Forty is enough to show that five questions are
   near-constant and that one door cannot read the states; it is not enough
   to separate two doors that differ by a point.

## What this does not say

- **The locked partition is unread.** Sixty-five items, never scored, no
  ledger entry. Every number here is from the development partition, and the
  calibration partition is unscored too: no maps were fitted for these
  families.
- **The states are a reconstruction, not a recording.** `coder` persists no
  sessions, so these are another agent's turns rendered into `coder`'s state
  shapes. Task text and command output are verbatim; the transcript
  rendering is a mapping, and a tool `coder` does not have leaves no trace
  in it.
- **One person, one repository, four sessions.** Forty turns and 540 rounds
  of one operator's work on this repository is the workload that exists
  here, not the workload in general. Per-family accuracies rest on 16 or 22
  items and carry a binomial standard error near 0.11; they are direction,
  and the paired comparisons are the measurement.
- **An outcome label says what happened, not what was best.** The turns
  where a door said `clarify` are scored wrong because the session's agent
  answered instead. That is evidence the turn was answerable; it is not
  proof that asking would have been worse.
- **`end_conversation` and `none` are never the truth**, so two of the four
  `action` options are unexercised and the `End` route with them.
- **The evidence split is confounded with the question.** Each family
  carries one label source, so `author` and "the two hardest questions" are
  the same items.
- **Latency is contended.** The machine ran nine agents and several copies
  of the on-device model throughout, and a `kev-serve` from another session
  held 590% of CPU for the whole run. Absolute numbers are that afternoon's.
  The ordering spans three orders of magnitude and survives it; the ratios
  are what to quote, and `deployment-v1` refuses to judge a clock at all
  until an uncontended sweep exists.
- **kev-8b was scored on one family.** Sixteen items of `action`, because
  two minutes a call could not buy more. Its `none` rate is the finding; its
  0.12 rests on 16 items.
