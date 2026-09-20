# The shell loop

How a Coder turn touches its environment: the host decides whether this
turn runs commands, the model proposes some, the terminal runs them,
Classify judges the round, and the model reads the outputs. Classify and
Generate decide together what a command means — the same two-stage split
as the turn itself, applied one level down. What neither of them decides
is whether anything runs at all.

Status: implemented in `crates/coder` (`permit.rs`, `shell.rs`,
`agent.rs::turn`, `classify.rs` shell questions).

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
- A refused plan is also the answer. The user reads what the model wrote,
  and the trace records why none of it ran.
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
| Per-command timeout | 15 s |
| Captured output (stdout + stderr) | 16 KiB, truncated with a marker |
| Output the judge and transcript see | 2 KiB head per command |
| Commands per plan | 10 |
| Plan rounds per turn | 3 |

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
`{command, why, status, output head}` — and answers three questions:

- `outcome` (choice): `pass` — hand the outputs to the model and
  continue; `retry` — a command failed or missed, the model should
  correct and try again; `stop` — the outputs show damage, a stuck
  loop, or nothing left to learn.
- `useful` (noul): do the outputs help answer the request?
- `damage` (noul): did anything suggest the commands harmed files,
  state, or secrets?

Routing: `damage ≥ 0.7` forces `stop` whatever the choice says; `stop`
ends the loop; `retry` and `pass` both feed the round back — the
difference is the instruction the model reads into its next plan. A
missing classifier or a malformed verdict defaults to `pass`: the
safest informative route, since the model still decides what the output
means. The round cap (3) and the final-only suffix bound the loop even
if every judgment says `pass`.

## The turn, whole

```
draft
  → classify (action/needs_code/risk/progress)          ← Jev, turn level
  → respond route
  → permit (route + operator: does this turn run commands?)  ← the host
  → loop:
      generate (instructions + repo card + sniff)
        ├─ prose    → done: answer streams to the user
        ├─ refused  → done: the reply is the answer, the trace says why
        └─ plan     → run commands (permitted, bounded, deny-listed)
                    → judge the round                    ← Jev, round level
                    → outcomes fold into the transcript
      (a spent permit forces prose: "answer with what you have")
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
  shell → pass 0.93 · useful 0.9 · damage 0.0
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
  runs, the reply is the answer, and the trace records the sentence.
- Command fails to spawn → `failed: <io error>`, judged like an exit.
- Timeout → `timed out`, output empty; the judge usually retries once
  or passes.
- Every proposal denied → the round is all refusals; the judge sees the
  reasons and typically stops or the model rephrases.
- Jev unreachable mid-loop → `pass`; the model decides from raw output.
- Model emits plans past the cap → final-only suffix forces prose; the
  turn still ends with an answer, never a hang.
