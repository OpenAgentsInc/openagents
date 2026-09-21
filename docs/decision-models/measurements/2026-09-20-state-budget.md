# A state budget for the smallest door

[`2026-09-19-coder-turns.md`](2026-09-19-coder-turns.md) kept `coder` on
hosted Jev for one reason that was not about accuracy: Apple's on-device
door refused 35 of 130 real items, 28 of the 31 states over 12 KB among
them, because the state `classify::state_of` built did not fit its context
window. Lev's accuracy on the items it could read was competitive. The
door was decided by the size of the state, and the state was never
designed; it was twelve whole turns because twelve was the number written.

This record measures what the state weighs, shrinks it one contributor at a
time with hosted Jev scoring each rung, and sets the caps
`classify::state_of` now applies from the door's measured refusal boundary
rather than from a round number.

**The result: every one of the 40 real turn states fits under the
boundary at the chosen caps, the largest at 7,118 bytes against a budget
of 8,028, and hosted Jev's accuracy on the development partition does not
move anywhere on the ladder that the run-to-run noise of a 64-item
partition can see.** On what was measured, the size of the state is no
longer a reason to keep the on-device lane closed.

## What the states weigh

The states are the 40 turn states in
[`crates/gym/suites/coder-turns-v1.json`](../../../crates/gym/suites/coder-turns-v1.json),
the ones `classify::state_of` builds; the suite's 55 shell-round states
are `shell::state_of`'s, are already small, and are not touched here.
Sizes are `serde_json::to_string` bytes, which is what the wire carries.

| Caps | n | Median | p90 | Largest | Over 10,704 |
| --- | --- | --- | --- | --- | --- |
| Before: 12 turns, whole messages, 10 commands, 2,048 B output | 40 | 12,761 | 20,053 | 23,680 | 30 |

Where the bytes are, summed over the 40 states:

| Part | Share |
| --- | --- |
| The assistant's own text | 54% |
| Shell records (`ran shell commands:` messages) | 41% |
| Other user messages | 2% |
| The task | 2% |

Command output was assumed to be the largest contributor. It is not. Shell
records are already bounded by `shell.rs` at ten commands and 2,048 bytes
of output each, and the assistant's own text, unbounded, is the larger
part. Reproduce with:

```text
python3 - <<'EOF'
import json
s=json.load(open('crates/gym/suites/coder-turns-v1.json'))
seen=set(); A=S=U=T=0
for it in s['items']:
    st=it['state']
    if 'transcript' not in st: continue
    k=it['id'].split('/')[1]
    if k in seen: continue
    seen.add(k); T+=len(st['task'])
    for m in st['transcript']:
        n=len(m['text'])
        if m['role']=='assistant': A+=n
        elif m['text'].startswith('ran shell commands:'): S+=n
        else: U+=n
tot=A+S+U+T
print(len(seen), 'assistant %.0f%% shell %.0f%% user %.0f%% task %.0f%%'%(100*A/tot,100*S/tot,100*U/tot,100*T/tot))
EOF
```

## Where the door's window is

`docs/lev/` states no byte figure for Apple's context window, and the
window is in tokens, so the boundary comes from the retained rows in
[`crates/gym/results/coder-turns-v1.jsonl`](../../../crates/gym/results/coder-turns-v1.jsonl):
joining each `lev-base` row to its item's state size, Lev answered states
up to **12,101** bytes and refused `branch_too_long` from **10,704**
bytes. The two overlap by 1,400 bytes because tokens per byte depend on
what the text is; a shell record tokenizes worse than prose.

```text
python3 - <<'EOF'
import json
suite=json.load(open('crates/gym/suites/coder-turns-v1.json'))
size={it['id']: len(json.dumps(it['state'],separators=(',',':'))) for it in suite['items']}
ans=[];ref=[]
for line in open('crates/gym/results/coder-turns-v1.jsonl'):
    r=json.loads(line)
    if r['door']!='lev-base': continue
    b=size[r['item_id']]
    if r['refusal']=='branch_too_long': ref.append(b)
    elif r['answered']: ans.append(b)
print('answered',len(ans),'largest answered',max(ans),'branch_too_long',len(ref),'smallest refused',min(ref))
EOF
answered 95 largest answered 12101 branch_too_long 32 smallest refused 10704
```

The budget is three quarters of the smallest refused state:
`STATE_BUDGET = 10_704 / 4 * 3 = 8,028` bytes. The quarter is margin for
the token-per-byte spread the overlap shows, and for the 1,031 bytes of
question set that ride in the same request. Every number here is a byte
count from a run on this suite; none is Apple's stated figure, and the
budget should move when a Lev run on a Mac moves the boundary.

## The ladder

Hosted Jev scored the 16 development-partition turn states, 64 items over
four families, at eleven rungs. Each rung rebuilds the same 16 states under
one set of caps and asks the production question set, so the only thing
that changes between rungs is the state. The ladder takes the largest
contributor first as the issue asked, output then commands then turns, and
then adds the per-message cap the byte shares say is needed. The
`unbudgeted` rung reproduces the suite's recorded states byte for byte,
which `crates/coder/tests/state_caps.rs` asserts.

```text
export CARGO_TARGET_DIR=$HOME/target-openagents
cargo test -p coder --test state_caps sweep -- --ignored --nocapture
```

| Rung | Median B | Largest B | Pooled | `action` | `needs_code` | `progress` | `risk` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| unbudgeted | 11,914 | 20,053 | 33/64 | 11/16 | 10/16 | 6/16 | 6/16 |
| output 512 | 10,064 | 15,561 | 32/64 | 11/16 | 10/16 | 7/16 | 4/16 |
| output 256 | 9,240 | 14,781 | 33/64 | 12/16 | 10/16 | 7/16 | 4/16 |
| commands 3 | 9,240 | 14,781 | 36/64 | 13/16 | 10/16 | 8/16 | 5/16 |
| turns 8 | 6,766 | 9,945 | 36/64 | 14/16 | 10/16 | 7/16 | 5/16 |
| turns 6 | 5,649 | 8,021 | 35/64 | 13/16 | 10/16 | 7/16 | 5/16 |
| turns 4 | 3,546 | 5,473 | 34/64 | 13/16 | 10/16 | 6/16 | 5/16 |
| turns 6, message 1024 | 3,631 | 4,879 | 34/64 | 13/16 | 10/16 | 6/16 | 5/16 |
| **production: turns 6, message 768** | 3,106 | 4,100 | 32/64 | 12/16 | 10/16 | 6/16 | 4/16 |
| turns 6, message 512 | 2,717 | 3,309 | 33/64 | 12/16 | 10/16 | 7/16 | 4/16 |
| turns 4, message 512 | 1,806 | 2,312 | 36/64 | 13/16 | 10/16 | 8/16 | 5/16 |

Each rung keeps the caps of the rung above it; `commands 3` is output 256
and three commands, `turns 8` is that and eight turns, and so on. Jev
refused nothing at any rung. The rows are in
[`2026-09-20-state-budget.jsonl`](2026-09-20-state-budget.jsonl), 704 of
them, one per item per rung, with the caps, the state's size, the option
chosen, and the truth. They sit beside this record rather than in
`crates/gym/results/` because their schema is this sweep's, not a Gym
row's, and that store admits only the schemas it knows.

### Where accuracy starts to fall

Nowhere the measurement can see. Pooled accuracy runs from 32/64 to 36/64
across a tenfold range of state size, and the same `unbudgeted` rung
scored 36/64 on a first run of the sweep earlier the same hour, so the
run-to-run noise of hosted Jev on this partition is at least 3 items and
the standard error of a proportion near one half on 64 items is ±4. No
rung is outside that band. `needs_code` is 10/16 at every rung, which is
the constant baseline the previous record found; the state's size never
mattered to it.

Two readings are honest and both are stated. First, the state can be cut
to a fifth of its size without a measurable cost, which is what the caps
below rely on. Second, 64 items cannot see a cost smaller than about six
points, and a real one that size would be worth knowing; the locked
partition and a larger development set are the way to find it, not more
rungs on these 16.

## The caps

`Caps::PRODUCTION` in `crates/coder/src/classify.rs`:

| Cap | Value | Why this value |
| --- | --- | --- |
| `turns` | 6 | The first turn count on the ladder that puts the ladder's largest development state under the budget without a message cap (8,021 B). Eight turns leaves it at 9,945 B, over the budget and under the boundary by 759 B. |
| `message_bytes` | 768 | With turns alone, three of the 40 states are still over 8,028 B (largest 12,336 B), because one long assistant message is enough. 1,024 B per message leaves one state at 8,154 B, over the budget by 126 B; 768 B puts the largest at 7,118 B. 512 B is the next rung and was not needed. |
| `commands` | 3 | The smallest command count on the ladder. On these states it changed no size at all against `output 256`; the cap is kept because the bound it gives is on the record's shape, not its bytes. |
| `output_bytes` | 256 | The first two rungs. Together they take 2,674 B off the development median, less than the byte shares predicted, because the shell records that matter are the many-command ones and those were already at `HEAD_MAX`. |

Every one of the 40 turn states under these caps, from a run of the
same code that produces them:

| Caps | n | Median | p90 | Largest | Over 8,028 | Over 10,704 |
| --- | --- | --- | --- | --- | --- | --- |
| `Caps::PRODUCTION` | 40 | 3,109 | 4,100 | 7,118 | 0 | 0 |

`cargo test -p coder --test state_caps` holds this: it rebuilds every turn
state under `Caps::PRODUCTION` and fails if one is over `STATE_BUDGET`, so
a cap that drifts fails the build rather than the door. A shell record is
bounded block by block, so a command is never cut mid-line, and the count
of dropped commands is written in its place. Text is cut on a character
boundary. Nothing here changes what a door is asked or how a refusal is
read; `branch_too_long` and `invalid_request` stay typed in
`gym::eval`, and this sweep records a Jev refusal, had there been one, by
status and kind.

## What this does not show

- **The initial run did not ask Lev.** The Apple follow-up below now
  measures base Lev at every rung on the 16 development states. At the
  production caps, all 16 were answered. The all-40-state claim remains
  a byte-budget assertion, not a live acceptance test of all 40 states.
- **The locked partition is unread.** The ladder ran on development only.
- **The caps were chosen on the same 40 states the fit is asserted on.**
  A longer session than any in the suite would produce a longer
  assistant message; the per-message cap bounds that, but the task field
  and the repo member list are not capped, and a state is
  `task + 6 × 768 B + overhead`, so a long task moves the ceiling.
- **The states are reconstructions**, as the previous record says, not
  persisted `coder` transcripts.

## What changed in the code

- `classify::Caps`, `classify::STATE_BUDGET`, `Caps::PRODUCTION`,
  `Caps::UNBUDGETED`, `classify::bounded_text`, and
  `classify::state_within`. `classify::state_of` now delegates to
  `state_within` under `Caps::PRODUCTION`; its signature and the state's
  shape are unchanged.
- `crates/coder/tests/state_caps.rs`: the fit test, the reproduction
  test, two tests on the bounding itself, and the ignored sweep that
  produced the rows above.


## Lev on the same ladder

**The production caps answered all 16 development states: 38/64 correct,
with no refused requests, against 21/64 and eight refused requests on the
unbudgeted rung.** This removes the observed coverage problem on this
subset. It does not establish that all 40 states were answered by Lev.

### The paired results

| Rung | Median B | Largest B | Pooled | `action` | `needs_code` | `progress` | `risk` | Refused requests |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| unbudgeted | 11,914 | 20,053 | 21/64 | 8/16 | 4/16 | 5/16 | 4/16 | 8 |
| output 512 | 10,064 | 15,561 | 25/64 | 10/16 | 4/16 | 5/16 | 6/16 | 6 |
| output 256 | 9,240 | 14,781 | 26/64 | 11/16 | 5/16 | 4/16 | 6/16 | 5 |
| commands 3 | 9,240 | 14,781 | 26/64 | 11/16 | 5/16 | 4/16 | 6/16 | 5 |
| turns 8 | 6,766 | 9,945 | 39/64 | 16/16 | 8/16 | 6/16 | 9/16 | 0 |
| turns 6 | 5,649 | 8,021 | 41/64 | 16/16 | 8/16 | 5/16 | 12/16 | 0 |
| turns 4 | 3,546 | 5,473 | 38/64 | 16/16 | 10/16 | 3/16 | 9/16 | 0 |
| turns 6, message 1024 | 3,631 | 4,879 | 37/64 | 16/16 | 8/16 | 4/16 | 9/16 | 0 |
| **production: turns 6, message 768** | 3,106 | 4,100 | 38/64 | 16/16 | 8/16 | 4/16 | 10/16 | 0 |
| turns 6, message 512 | 2,717 | 3,309 | 40/64 | 16/16 | 8/16 | 6/16 | 10/16 | 0 |
| turns 4, message 512 | 1,806 | 2,312 | 35/64 | 16/16 | 7/16 | 5/16 | 7/16 | 0 |

Each rung retains 16 request outcomes, four question rows per request. The
[704 raw rows](2026-09-20-state-budget-lev.jsonl) match the Jev rows by rung, state, family, truth,
caps, and state byte count. Their SHA-256 is
`22535d570d20436d99c7f0c192e5b253cd7b5ce02d8f10a93ae357de26bb8e8a`. The
[run metadata](2026-09-20-state-budget-lev-run.json) retains the source revisions, start and end times,
loads, and runner digest.

The first four rungs returned HTTP 413 on 8, 6, 5, and 5 requests,
respectively. Every later rung answered all 16 requests. The SDK recorded
these errors as `api 413: Other`; their response bodies were not retained,
so this record does not assign a more specific typed cause.

### What this changes about the door recommendation

The coverage objection is removed for these development states at the
production caps. The recommendation remains hosted Jev for now. The pooled
comparison is Lev 38/64 against the retained Jev run's 32/64, but it includes
three questions that production no longer asks. On the remaining `action`
question, Lev is 16/16 and Jev is 12/16; every truth in this subset is
`respond`, so the constant `respond` is also 16/16. This sample does not
establish that a model adds value to that decision or that changing doors
improves the production workload.

The sweep fits no calibration map and changes no release admission. It also
does not measure the latency of the current one-question route: each timed
request here asks four historical questions. These are 16 states with
correlated questions, not 64 independent workloads, and one pass per rung
establishes no new noise floor. The tighter caps' scores are retained rather
than selecting whichever rung happened to score highest.

### Machine, method, and limitations

This follow-up runs the same eleven rungs against base Lev on an Apple M5
Max, macOS 26.4, build `25E246`. It asks the same 16 development states and
four historical questions as the retained Jev run: 64 rows per rung, 704
rows in total. Matching a row by rung, state, and family must also match its
truth, caps, and serialized state size. This is a live measurement of those
16 states. The assertion that all 40 states fit the byte budget is a
separate check and does not establish that Lev answered all 40.

This section measures the base release only. The initial publication
incorrectly said that all adapted manifests admit no families, following
stale disposition prose. `lev-adapted-v1.json` already references an admitted
routing map; the band and permutation manifests admit no families. The
choice adapter still needs the same eleven-rung sweep, and #9398 remains
open until that evidence is recorded. These base rows do not establish
coverage for the choice adapter.

The base server uses eight seeded samples per question, seed block zero,
and four helpers. Its manifest is
`lev-base@1`; its published runtime signature is `9799725`. No calibration
directory is loaded, so the answers are sampling frequencies. The manifest's
routing admission does not make these four workload families calibrated.

### Method and retained evidence

The measurement runner is derived from
`crates/coder/tests/state_caps.rs` at
`dbab6bef920ed77e45cb2fd74f1245ab90e44ec6`. Production has since retired three
of the four questions. To preserve the comparison, the runner uses the
four-question `questions()` function from
`b8c3124e15:crates/coder/src/classify.rs`, including its option order.
The suite, labels, ladder, state construction, and answer selection are
unchanged. The production test file is not edited.

The first attempt completed the unbudgeted rung. Its controller had a
30-minute overall limit and the runner saved only at rung boundaries. After
observing how long requests took, the operator stopped that attempt,
retained its 64 completed rows, and resumed with a checkpoint after each
request and a three-hour overall limit. The per-request deadline stayed at
120 seconds, with SDK retries disabled. The resumed process reads complete
retained requests and skips them. It does not ask the completed unbudgeted
rung again. Any work beyond the completed rung that had not been checkpointed
is not recoverable and is not counted; subsequent rungs come from the resumed
attempt. This is a complete grid assembled across two attempts, not one
uninterrupted run. The original attempt's raw rows remain unchanged in the final
file; reparsing and serializing them would round one latency value.

The source checkout was `dbab6bef92` for the first attempt and `29afd7c2dd`
for the resume. There is no change between them in `classify.rs`, Jev, or
Lev. The companion runner patch applies to the pinned test source and
reproduces the resumed runner byte for byte. Its suite path names this
measurement's checkout; change only that path when reproducing elsewhere.

The server command, from the repository root:

```sh
LEV_OS_BUILD=25E246 \
  /tmp/openagents-supervision/root-target/debug/lev-serve \
  --manifest crates/lev/manifests/lev-base-v1.json \
  --port 11456 --policy-refresh off
```

The external Rust runner uses local path dependencies on `coder` and `jev`,
plus `serde_json = "1"`, `indexmap = "2"`, and Tokio with its `full` feature.
It calls `http://127.0.0.1:11456` with a local placeholder credential. Its
commands are:

```sh
CARGO_TARGET_DIR=/tmp/openagents-apple-handoff/sweep-target \
  cargo build --manifest-path /tmp/openagents-apple-handoff/state-sweep/Cargo.toml
STATE_SWEEP_ROWS=/tmp/openagents-apple-handoff/state-budget-lev.jsonl \
  /tmp/openagents-apple-handoff/sweep-target/debug/lev-state-sweep-record
```

Each request asks all four questions. A failed request contributes four
incorrect rows, so unanswered items stay in the pooled denominator. The
error column preserves the SDK's status and category; a generic `Other`
category does not establish a more specific refusal reason. A request's
latency is repeated on its four question rows, not four independent timings.
This is an accuracy and state-size run, not the quiet latency experiment:
scoped compilation and tests also ran during the sweep, and ordinary desktop
applications remained open.


Load averages (1, 5, and 15 minutes) and controller wall times:

| Attempt | Started, UTC | Load before | Load after | Wall time | Outcome |
| --- | --- | --- | --- | --- | --- |
| Initial | 2026-09-20 15:41:29 | 1.24, 2.20, 2.57 | 2.57, 1.87, 2.11 | 835.8 s | Stopped after retaining the unbudgeted rung |
| Resume | 2026-09-20 15:56:31 | 2.49, 1.95, 2.12 | 3.73, 3.78, 3.78 | 5,706.3 s | Completed the remaining ten rungs; exit 0 |

The resumed attempt finished at 2026-09-20 17:31:38 UTC. These wall times
include startup and checkpointing; they are not isolated inference timings.

### Reproducing the runner

The [runner patch](2026-09-20-state-budget-lev-runner.patch),
[Cargo manifest](2026-09-20-state-budget-lev.Cargo.toml), and
[lockfile](2026-09-20-state-budget-lev.Cargo.lock) retain the measured harness.
Create an external directory with `src/main.rs` containing
`git show dbab6bef920ed77e45cb2fd74f1245ab90e44ec6:crates/coder/tests/state_caps.rs`,
then apply the patch there with `patch -p1`. Copy the manifest and lockfile
there as `Cargo.toml` and `Cargo.lock`. The patch has no context lines; when
using `git apply` instead, pass `--unidiff-zero`.

Adjust the checkout path in `src/main.rs` and the two path dependencies in
`Cargo.toml` for your machine. Use the recorded source revision and pinned
Rust 1.97.1, build the Swift helper, and start the server as above. Build the
runner with `cargo build --locked --manifest-path <runner>/Cargo.toml` and
a separate `CARGO_TARGET_DIR`. For a fresh measurement, set
`STATE_SWEEP_ROWS` to a new file: an existing checkpoint resumes its retained
requests instead of asking them again.

On this checkout, applying the patch reproduced the running source byte for
byte, SHA-256
`4dc2c642e21514fc0368fe50ef6f4b118d4b6711f5e51ceda1bf32fb13f42979`.
The row validation checked all 704 keys, their caps, state byte counts, and
truths against the retained Jev rows. Scoped Coder Clippy with
`--all-targets -- -D warnings` and `cargo test -p coder` passed. Workspace
formatting passed before publication. The original suite,
question files, gates, and production tests remain unchanged by this
measurement.


## Choice adapter comparison

The completed `lev-adapted@1` sweep uses the same sixteen development states,
four historical questions, and eleven rungs as the base comparison. At the
production caps it answers all 16 requests and scores 25/64, versus base
38/64 and hosted Jev 32/64. State fit alone does not make this adapter a better
coding router. Keep the current caps and door disposition; the adapter's
support-routing admission does not establish coding-task quality.

| Rung | Median B | Largest B | Pooled | `action` | `needs_code` | `progress` | `risk` | Refused requests |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| unbudgeted | 11,914 | 20,053 | 12/64 | 2/16 | 3/16 | 3/16 | 4/16 | 9 |
| output 512 | 10,064 | 15,561 | 15/64 | 2/16 | 3/16 | 2/16 | 8/16 | 6 |
| output 256 | 9,240 | 14,781 | 16/64 | 2/16 | 3/16 | 2/16 | 9/16 | 5 |
| commands 3 | 9,240 | 14,781 | 16/64 | 2/16 | 3/16 | 2/16 | 9/16 | 5 |
| turns 8 | 6,766 | 9,945 | 26/64 | 6/16 | 6/16 | 2/16 | 12/16 | 0 |
| turns 6 | 5,649 | 8,021 | 25/64 | 7/16 | 5/16 | 3/16 | 10/16 | 0 |
| turns 4 | 3,546 | 5,473 | 28/64 | 10/16 | 6/16 | 2/16 | 10/16 | 0 |
| turns 6, message 1024 | 3,631 | 4,879 | 24/64 | 6/16 | 6/16 | 3/16 | 9/16 | 0 |
| **production: turns 6, message 768** | 3,106 | 4,100 | 25/64 | 7/16 | 6/16 | 4/16 | 8/16 | 0 |
| turns 6, message 512 | 2,717 | 3,309 | 24/64 | 7/16 | 6/16 | 4/16 | 7/16 | 0 |
| turns 4, message 512 | 1,806 | 2,312 | 24/64 | 9/16 | 6/16 | 3/16 | 6/16 | 0 |

Refused requests counts SDK request failures, each contributing four incorrect
rows. Exact error categories are retained in the JSON summary; an SDK `Other`
category does not identify a more specific server cause.

| Rung | Choice | Base | Jev | Choice − base | Choice − Jev |
| --- | --- | --- | --- | --- | --- |
| unbudgeted | 12/64 | 21/64 | 33/64 | -9/64 | -21/64 |
| output 512 | 15/64 | 25/64 | 32/64 | -10/64 | -17/64 |
| output 256 | 16/64 | 26/64 | 33/64 | -10/64 | -17/64 |
| commands 3 | 16/64 | 26/64 | 36/64 | -10/64 | -20/64 |
| turns 8 | 26/64 | 39/64 | 36/64 | -13/64 | -10/64 |
| turns 6 | 25/64 | 41/64 | 35/64 | -16/64 | -10/64 |
| turns 4 | 28/64 | 38/64 | 34/64 | -10/64 | -6/64 |
| turns 6, message 1024 | 24/64 | 37/64 | 34/64 | -13/64 | -10/64 |
| production: turns 6, message 768 | 25/64 | 38/64 | 32/64 | -13/64 | -7/64 |
| turns 6, message 512 | 24/64 | 40/64 | 33/64 | -16/64 | -9/64 |
| turns 4, message 512 | 24/64 | 35/64 | 36/64 | -11/64 | -12/64 |

This is a declared two-attempt composite. The first six rungs retain exactly
384 original rows, including 24 HTTP 413 requests and one timeout. An HTTP
500 failure beginning in the next rung led to repeating the entire five-rung
suffix: all 80 requests, including its originally answered requests. The fresh
suffix completes with no failed requests. The original full 704-row failed
attempt, including 272 HTTP 500 rows, remains retained separately. Its outer
queue observes controller exit 1; its inner runner exit is unknown because
cleanup suppressed terminal metadata. Recovery records runner exit 0 and
no cleanup errors.

The [choice measurement record](2026-09-20-state-budget-choice.md) retains both
attempts, immutable hashes, per-family results, timing limitations, source,
model identities, and offline reproduction. The questions match this historical
comparison, including three since-retired production questions. These 64 rows
per rung are four correlated questions over 16 states; the always-`respond`
`action` label is not a balanced routing task. This evidence does not justify
selecting a new rung or changing today's production routing policy.
