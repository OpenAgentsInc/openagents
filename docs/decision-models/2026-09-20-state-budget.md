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
[`crates/gym/suites/coder-turns-v1.json`](../../crates/gym/suites/coder-turns-v1.json),
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
[`crates/gym/results/coder-turns-v1.jsonl`](../../crates/gym/results/coder-turns-v1.jsonl):
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
[`crates/gym/results/coder-turns-v1-state-budget.jsonl`](../../crates/gym/results/coder-turns-v1-state-budget.jsonl),
704 of them, one per item per rung, with the caps, the state's size, the
option chosen, and the truth.

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

- **Lev was not run.** There is no Apple hardware on this machine. The
  claim that every real state now fits is a byte count against a boundary
  read from retained rows, not a refusal count from a fresh Lev run. The
  next Lev run on `coder-turns-v1` with these caps is the measurement
  that closes the door question; its `branch_too_long` count should be
  zero, and if it is not, `STATE_BUDGET` is wrong and this record says by
  how much.
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
