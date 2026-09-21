# The shell loop

How a Coder turn touches its environment: the host decides whether this
turn runs commands, the model proposes some, the terminal runs them,
Classify judges the round, and the model reads the outputs. Classify and
Generate decide together what a command means — the same two-stage split
as the turn itself, applied one level down. What neither of them decides
is whether anything runs at all.

Status: implemented in `crates/coder` (`permit.rs`, `shell.rs`,
`agent.rs::turn`, `classify.rs` shell questions).

The [TypeSafe-native roadmap](typesafe-agent-roadmap.md#phase-2-close-the-native-coding-loop)
proposes native coding operations and context selection over retained
observation artifacts. Those changes are not implemented by the current
shell loop. Its short deadlines, round cap, and output heads remain actual
limits; a useful coding executor needs measured workload-specific bounds
and access to diagnostics before context selection can help.

## Why a loop, not a tool call

The first version of the agent answered every project question from
memory — it had text out and nothing in. Tool use in most agent shells
is a function-call protocol baked into one provider's API. This one is
deliberately dumber and more portable: a command plan is just JSON the
model emits as its whole reply, the terminal parses it, and the loop is
code in the agent, not a feature of any door. Every `Generate`
implementation — own-key, relay worker, stub — gets the loop for free.

## The plan

When a request needs the environment — files read, code searched, tests
run, anything checked on disk — the model replies with one JSON object
and nothing else:

```json
{
  "v": 1,
  "commands": [
    { "command": "git grep -rn jev crates/", "why": "find where jev is used" },
    { "command": "cargo test -p jev", "why": "see if the crate is green" }
  ]
}
```

- The whole reply is the plan: one JSON object, or one fenced
  ```` ```json ```` block and nothing else, for models that insist on
  decorating. A sentence before or after the fence makes the reply prose
  whatever the fence holds, which is what makes an example an example.
- `v` says which schema the reply speaks, and this host runs version `1`.
  A reply that asks for commands under another version, or under none, is
  refused rather than read as far as it happens to parse.
- `commands` carries 1–10 entries; each has a `command` (`sh -c` text)
  and a `why` the terminal displays and the judge reads. An entry the
  host cannot read whole refuses the plan it belongs to, and so does an
  eleventh command: a truncated plan is a plan nobody wrote.
- Anything that does not ask for commands is prose: the reply is the
  answer, the turn ends.
- A refused plan is not the answer. The user reads a sentence from the
  host saying that none of the proposed commands ran and why, followed by
  what any earlier rounds did observe; the plan's JSON is never rendered
  as the reply. The trace keeps the proposal and the host's reason, and
  the turn ends `refused` rather than `answered`.
- The instructions forbid decorating a plan with prose and forbid an
  empty plan — "no commands needed" is expressed by answering in text.

## Execution intent

A plan says what the model wants to run. Whether this turn runs anything
is the host's to say, and the host says it before the turn generates a
word. `Permit::for_route` builds that answer out of two things:

- **The route.** Only `respond` carries execution. A turn the router sent
  to clarification asks one question, and `end` and `halt` run nothing
  either. Clarification changes the prompt; separately, and for its own
  reason, it changes what the host permits. A valid plan that arrives on
  a clarifying turn is an answer that looks like a plan.
- **The operator.** `CODER_SHELL=off` — or `no`, `false`, `none`, `0` —
  withdraws execution from every turn on this host.

A permit narrows and never widens. Spending the round cap or drawing a
`stop` verdict withdraws it for the rest of the turn, and `Agent::turn`
withdraws it again for a clarifying turn whatever its caller passed.
`shell::run` reads it once more before it spawns anything, so reaching a
shell takes a permit rather than a `Proposal`. Nothing in a reply grants
one: not a confident classification, not an instruction the model was
given, and not the model's own account of what it is about to do. The
deny list below is the second question — whether this command is the kind
that ends a machine — and it is never the first.

The terminal and `coder --print` both run `turn::run`, so both get the
same answer to that question.

## Execution

Each proposal runs as `sh -c` in the terminal's working directory:

| Bound | Value |
| --- | --- |
| Per-command timeout | 15 s, which ends the command's whole process tree |
| Captured output | 16 KiB per stream, held as it is read, marked when cut |
| Retained output (stdout + stderr) | 16 KiB, with the byte count of everything printed |
| Output the judge and transcript see | 2 KiB head per command |
| Commands per plan | 10 |
| Plan rounds per turn | 3 |

The timeout and the cap are `crates/supervise`'s, which is what makes them
bounds rather than intentions: the command runs in a process group of its
own, the deadline terminates that group and reaps the child rather than
abandoning the wait, and a command that reached its deadline still reports
what it printed first. Read [`subprocesses.md`](subprocesses.md).

A deny list refuses commands that end a machine or a session rather than
answer a question — `sudo`/`doas` (they would hang on a password
prompt), `rm -rf` of root or home, `mkfs`, `dd of=/dev`, fork bombs,
`shutdown`/`reboot`/`halt`, `| sh`-style pipe-to-shell, keychain reads,
device writes, recursive `chmod`/`chown` from `/`. A denied command never
spawns; its outcome is `denied: <reason>` and the judge sees it like any
other result — the model learns what was refused and why. A proposal that
reaches the runner on a turn that runs nothing never spawns either, and
its outcome reads `refused: this turn does not run commands`.

## The judgment

After each round, Classify reads the task plus every outcome —
`{command, why, status, output head}` — and answers one question,
`outcome` (choice): `pass` — hand the outputs to the model and continue;
`retry` — a command failed or missed, the model should correct and try
again; `stop` — the outputs show damage, a stuck loop, or nothing left to
learn. This is the round half of the `coder-turns-v2` question set.

Routing: `stop` ends the loop. `pass` feeds the round back unchanged.
`retry` feeds it back with a suffix on the next instructions that says
the judge read the round as a retry and asks for different commands or a
prose answer. Consecutive retries are bounded by `RETRIES_MAX` (2): a
second `retry` in a row stops the loop as `Exhausted::Retries`, and a
`pass` between them resets the count. A missing classifier or a
malformed verdict defaults to `pass`: the safest informative route,
since the model still decides what the output means. The round cap (3)
and the final-only suffix bound the loop even if every judgment says
`pass`.

### The retired damage gate

`coder-turns-v1` asked two more questions each round, `useful` and
`damage`, and read `damage` against a stop threshold of 0.7 when the door
said the number was a calibrated probability. Both are retired by
[the question baselines](../decision-models/2026-09-20-coder-question-baselines.md):
`damage` was `no` on 55 of 55 labelled rounds and hosted Jev never
answered above 0.13, `useful` was read by nothing, and the gate never
fired on the door `coder` ships with because a hosted door says nothing
about calibration. The v1 text and the rows that measured it stay in
`crates/gym/questions/coder-turns-v1.json` and
`crates/gym/results/coder-turns-v1.jsonl`. A harm question comes back only
with a suite that holds harmful rounds to score it on.

## Exhaustion

The loop stops for one of three reasons, and the turn says which: the
judge said `stop`, the judge said `retry` `RETRIES_MAX` rounds in a row,
or the permit's rounds are spent. Either way execution is
withdrawn for the rest of the turn, the model reads the final-only
suffix, and its next reply is expected to be prose. If it is prose, that
is the answer, and the turn carries the exhaustion as metadata for the
trace. If it is another plan, nothing in it runs. The host asks once, in
a message the model reads, for prose from the output it already has
(`REPAIRS_MAX` is 1, bounded separately from the rounds). Prose then is
the answer. A second plan ends the turn as a refusal: the host writes the
reply itself, naming why the plan did not run, why the loop stopped, and
each command that did run with its status and a bounded slice of its
output, so what was observed is not lost with the model's answer. The
round cap is not raised and the budget is not reset for the repair.

## The turn, whole

```
draft
  → classify (action)                                  ← Jev, turn level
  → respond route
  → permit (route + operator: does this turn run commands?)  ← the host
  → loop:
      generate (instructions + repo card + sniff)
        ├─ prose    → done: answer streams to the user
        ├─ refused  → done: the host's sentence is the reply, the trace says why
        └─ plan     → run commands (permitted, bounded, deny-listed)
                    → judge the round                    ← Jev, round level
                    → outcomes fold into the transcript
      (a spent permit forces prose: "answer with what you have";
       one repair request if a plan comes back; a second plan is refused
       and the host answers with what ran)
```

The transcript records plans as assistant turns and outcomes as a user
message (`ran shell commands: $ …, exit, output head`), so the model's
next generation sees its own history whole. Usage accumulates across
rounds for the token rail.

## What the user sees

```
> describe how we use jev in this project
  classify → respond
  $ git grep -rn "jev" crates/ --include="*.rs" -l
    find where jev is used
    exit 0 · 0.2s
  $ cargo doc -p jev --no-deps 2>&1 | head
    skim the crate's own docs
    exit 0 · 4.1s
  shell → pass 0.93
  Jev is the System One crate …
```

The `$` lines replace the plan's raw JSON in the display; the verdict
line reads exactly like the classify line above it — same shape, same
transparency.

## Relay parity

The loop is door-agnostic. Over the relay door each round is a new
NIP-CJ job: the transcript carries plans and outcomes, so the remote
model sees the same history a local door would. A worker that wants to
run commands itself can adopt the same plan/judge structure server-side
without a protocol change — `status`/`judgment`/`partial`/`result`
already carry everything the terminal needs to draw.

## Failure shape

- Reply asks for commands the host cannot read — an unsupported version,
  an entry missing its text or its reason, an eleventh command → nothing
  runs, the user reads the host's refusal rather than the JSON, and the
  trace records the proposal and the sentence.
- Command fails to spawn → `failed: <io error>`, judged like an exit.
- Timeout → `timed out`, output empty; the judge usually retries once
  or passes.
- Every proposal denied → the round is all refusals; the judge sees the
  reasons and typically stops or the model rephrases.
- Jev unreachable mid-loop → `pass`; the model decides from raw output.
- Model emits plans past the cap → final-only suffix asks for prose, one
  repair message asks again, and a plan after that is refused with the
  host's own account of what ran; the turn ends bounded, never a hang
  and never with a plan on screen.
