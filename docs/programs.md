# Capabilities, executors, and programs

How a Coder instance learns what it can hand work to, how the decision
engine chooses, and how a **program** defines a reusable workflow.

The [program and extension specification](extensions/README.md) defines the
target integration of programs, Wasm plugins, skills, operation discovery,
and packages. This guide retains the original rationale and operational
history; the new specification distinguishes implemented behavior from
proposed composition and distribution.

The worked case throughout is delegating to the Devin CLI, because it is the
one an operator here actually has and wants used. It is a row in a table,
not a special case, and the last section says what generalizes.

## Current delivery direction

The [TypeSafe-native Coder roadmap](coder/design/typesafe-agent-roadmap.md) places
these programs over shared evidence and task-specific context. The earlier
reference-design discussion below explains the concepts; it is not a claim
that every proposed capability or composition rule is implemented here.
Use [the consumer inventory](coder/design/coder-as-decision-router-consumer.md)
for current scope.

Explicit [program grants](coder/guides/program-authority.md),
[scoped tracker intake](coder/guides/tracker-intake.md),
[project supervision](coder/guides/project-supervision.md), and a bounded typed
[`run-suite` host path](coder/guides/artifact-verification.md) now exist. Full
program recovery, typed composition, portable package resolution, and the
proposed evidence/context store remain delivery work. The new roadmap
extends these components; it does not introduce a separate private program
runtime.

## The word

Use **program** for a workflow and **plugin** for a bounded Wasm guest. A
Claude Code or Codex package under `plugins/` is a client plugin package, a
different thing; the [glossary](glossary.md#plugins-and-skills) separates the
meanings. Call the combined product surface **programs and extensions**. Selecting a program
answers which workflow the request asks for; it does not select a package to
install or grant permission to execute it.

The glossary in the reference implementation already allocates the terms,
and four of them matter:

| Term | What it already means there |
| --- | --- |
| **Capability** | A specifically granted ability — a tool, a readable directory. A capability a grant file does not declare is offered to no run. |
| **Executor** | The implementation that performs an agent session, including the built-in runner or an external agent adapter. `coder-devin` is listed among the executor adapters. |
| **Program (Jev)** | A state machine in code with named steps and per-step bounds, itself a signature. **The decision engine picks the program at run start** and the next signature inside a step. |
| **Plugin** | A sandboxed WebAssembly guest, with an authoring CLI, a package, an install, and a catalog. Explicitly distinct from a skill. |

So the words already exist and already fit:

- **Devin is an executor**, reached through an executor adapter. That term
  names it exactly, and `coder-devin` is already in the list.
- **The reusable, composable unit is a program.** That term was already
  claimed for the decision engine, which is precisely where this belongs.
- **A plugin is a Wasm guest**, and a Devin delegation cannot be one.

That last point is structural rather than a preference. The reference
design's plugin host sorts guests into three tiers: pure compute loads
without asking, a guest wanting read-only directories needs an operator, and
a guest declaring network access **never loads**. A Devin delegation spawns a process, reaches
the internet, writes files, and is not deterministic. It is the tier that
never loads. The host here, `crates/plugin`, implements only the first two
tiers, as its `Pure` and `SnapshotRead` profiles, and has no network
profile to load.

The
[capability-sockets review](decision-models/research/2026-09-19-capability-sockets.md)
found the same boundary for trained adapters: the manifest contract can
generalize beyond Wasm, while the sandbox applies to eligible guests.
Retain the bounded guest host for those operations and keep effectful
executors behind native adapters. A common operation descriptor supports
discovery without merging their execution contracts.

**A program is the workflow composition unit.** The target composition
contract connects typed step outputs to inputs under narrowed bounds. Two
automatic plugins eligible to replace the same call's output need explicit
host ownership so completion order cannot choose the result. The runtime
runs `module` steps; plugins can also compose through declared program
dataflow once typed bindings between steps are implemented. See
[Programs and decisions](extensions/programs.md) and [plugin host roles](extensions/plugins.md#host-roles).

## The three layers

**A capability manifest** says how to drive an executor: transport, how to
detect it, which bounds it enforces, **which bounds it will silently
ignore**, whether it can see the repository, and who pays.

**An operator policy** says what this operator wants: which capabilities to
prefer, how wide a fan-out may go, and what is never to be used.

**A program** says what to do: named steps, each with bounds, each either a
query, a decision, a deterministic check, or a delegation.

They are addressable Nostr events in two NIPs, split on purpose.
[NIP-PRG](../nips/openagents/NIP-PRG.md) defines programs (`30182`) and
[NIP-CAP](../nips/openagents/NIP-CAP.md) defines capabilities (`30180`,
`30181`). The relay is the workspace, which is the pattern `AGENTS.md` names
for application behaviour: event kinds and relay policy rather than a
private backend.

**A program is the general primitive and does not belong to Coder.** It says
nothing about an agent, an executor, a model, or a product — only what the
steps are and what bounds them. Filing it with capabilities implied it was
part of one product's surface, and it is not. Capabilities are specific to a
machine; programs are specific to nothing.

That split buys three properties a combined document could not state
cleanly: programs compose by reference under bounds that **narrow and never
widen**, a host **refuses** a program whose step kinds it does not recognize
rather than skipping them, and cycles are refused outright rather than
bounded by depth. Those are rules about programs, not about what a program
happens to reach.

**Local presence is not an event.** Whether `devin` is on *this* computer is
found by running the manifest's `detect` and stays on the machine.
Publishing it would broadcast an inventory of somebody's computer, and no
party here needs it.

## The local registry

Manifests and programs are files before they are events. The repository
carries both, and `crates/coder` reads them:

| Directory | What it holds |
| --- | --- |
| `capabilities/` | One `kind:30180` manifest per file. `devin-local` is the first; `coder-one-ask` is Coder One's read-only ask mode, and `gym` is the Gym's read-only binary for a command source. |
| `programs/` | One `kind:30182` program per file: `delegate-fan-out`, `burn-down`, `review-changes`, `answer-question`, `run-suite`, `review-runs`, `evidence-guests`. |
| `questions/` | One question set per file, addressed by identifier: `openagents.program.v1`, `openagents.independence.v1`, `openagents.independence.v2`, `openagents.completion.v1`. |
| `sources/` | One task source per file. `work-list` reads a file, `gym-runs` runs a command; `request` is built in. |

A host reads `CODER_CAPABILITY_DIR` first, then the repository's directory,
then `~/.openagents/capabilities`, and the same three for programs, for
questions, and for sources. The first definition of a slug wins, so an
operator overrides a checkout without editing it.

### A `decide` step names a question, and the wording lives elsewhere

A program keeps decision-question wording outside its steps. A `decide`
step names an identifier such as
`openagents.independence.v2`; the text behind it is a file in `questions/`,
digested as a whole, and the digest is recorded beside every answer.

Rewording a question changes what was asked. A program that inlined its
wording could not say which version produced a result, so `Program::load`
refuses a `decide` step carrying `instructions`, `criteria`, `questions`,
`text`, or `prompt`, and `Runtime::admit` refuses one whose identifier this
host has no wording for.

Some existing delegation programs have a `briefing` field for execution
guidance. That legacy field is not decision-question wording or authority.
The [portable program contract](extensions/programs.md#program-definitions-and-bindings)
separates guidance assets and requires an explicit migration rather than
changing the identity of historical programs.

A set fills in exactly two things at run time, and both are bounded fields
chosen after the route was:

- A Choice question declaring `"options": "supplied"` takes its options
  from the run, beside any it declares itself. The program-selection
  question's options are the programs this host would admit, which is how
  an operator without an executor gets a shorter option set rather than a
  broken one, plus the `none` the file declares. An option whose wording is
  the same on every host belongs in the set, where the digest covers it;
  only the slugs and summaries come from the run.
- A set declaring `per_requirement` is a template. The host asks it once
  per requirement and writes the requirement's name into the instructions,
  because a set of identical questions under different identifiers gives a
  model nothing to tell them apart with.

A question, or a template, may also carry a `decision` block: how its
answer becomes a decision, kept apart from what it asks. The block takes
`threshold` for a Noul (a probability at or above it reads as yes),
`cuts` for a Score (ascending boundaries that turn the
probability-weighted mean into a level), and `weights` for a Choice (the
decision is the option with the largest probability times weight):

```json
"per_requirement": {
  "type": "noul",
  "instructions": "The requirement the state lists under {requirement} landed.",
  "decision": { "threshold": 0.75 }
}
```

Every setting is optional. Without one, a Noul reads as yes at 0.5, a
Score is the level the model selected, and a Choice is the option the
model picked, which is what a host did before settings existed. The block
is never sent and is outside the set's digest, so changing a setting
leaves every request and every recorded answer unchanged. The host
records the settings' own digest as `decision_digest` beside the set's
digest, only when the set carries a block. Choice weights apply where an
answer is used and never change a calibration map, which never overrides
the model's pick. The Gym's question sets in `crates/gym/questions/` take
the same block, and Coder One's policy manifest records the digest of the
settings it reads Jev's answers under as `policy.jev.decision` when any
differs from its default.

The program registry read records the Nostr filter it would have sent
beside the answer it got from disk, because the query is the part that has
to keep working when the answer does not. Publishing to the relay changes
where the answer comes from and not what was asked.

### A `query` step names a source, and the command lives elsewhere

The same rule, one step over, and it is the rule that decides whether a
program can find work at all. A `query` step carries a **source slug** such
as `request` or `work-list`. What that slug reads is a file in `sources/`,
and a host that cannot resolve the slug refuses the step rather than falling
back to whatever work was handed in.

"Run `gh issue list`" is not a step kind. A program that carried a command
would be code, and a program that carries none is the one property
everything else rests on — it is what makes a program safe to read from a
stranger. So the program says *which* lookup, the machine says *what* the
lookup is, and the two can differ between machines running the same program
the way a `delegate` step's executor already does.

A source declares where its answer comes from and the order it is in:

```jsonc
{
  "v": 1,
  "slug": "work-list",
  "name": "The work list this checkout carries",
  "summary": "Reads an ordered work list from .coder/work-list.json, by identifier.",
  "from": {"file": {"path": ".coder/work-list.json"}},
  "order": "id"
}
```

`request` is the work the request carried, built in because its meaning
cannot be anything else, and a file source reads a work list under the
workspace. Neither runs a process.

A `command` source runs one, and only through the trust boundary and the
subprocess bounds
[#9427](https://github.com/OpenAgentsInc/openagents/issues/9427) is about. It
names a **capability**, never a binary, so the program it runs is the one
that capability's approval pins: `capability-trust approve gym` records the
manifest's digest and the binary's canonical path and content, and a
capability that isn't present refuses the step with `source_unavailable`.
The command runs in the workspace through the supervisor, bounded at 60
seconds and the probe's 64 KiB output cap, and its output must declare the
schema the source names. A lookup that executed an argv because it had read
a file is still the finding rather than the fix; the approval is what makes
this one different. `gym-runs` is the first:

```jsonc
{
  "v": 1,
  "slug": "gym-runs",
  "from": {"command": {"capability": "gym",
                       "args": ["runs", "--order", "learning", "--json", "--limit", "5"],
                       "schema": "openagents.gym.runs.v1"}},
  "order": "given"
}
```

This host reads one command schema, `openagents.gym.runs.v1`, and it becomes
one work item: the operator's request, with the Gym's totals and the runs it
listed as context. A question about runs is one question, so it's one
delegation, never one per run. A `query` step whose source is a command
declares the `subprocesses` effect as well as `reads`. An operator who wants
the open issues still writes them to a work list with one command of their
own.

A work list names what each item touches, what it comes after, and what
answer it expects back:

```jsonc
{
  "v": 1,
  "work": [
    {"id": "9391", "prompt": "…", "reads": "crates/gym/src/digest.rs", "writes": true,
     "expects": "3"},
    {"id": "9401", "prompt": "…", "touches": ["crates/gym/src/gate.rs"], "after": ["9391"]}
  ]
}
```

`expects` is the item's stated answer, the way a CoderBench task's
`expects` entry states one, and it is what the `accept` step judges: a
delegation whose output matches it passes, one whose output does not
fails, and an item that states none is **unverifiable**. An unverifiable
item is never counted as passed, however plausible its output reads; the
run's summary counts the three apart (`1 passed, 1 failed, 1
unverifiable`) and the acceptance state carries each requirement's
`expects` and `verdict`. The first burn-down episode reported "0 of 0
correct" because its items stated nothing to judge
([#9413](https://github.com/OpenAgentsInc/openagents/issues/9413)).

The answer is what follows the last `Final answer:` in the delegate's
output, or the whole output when it carries none. The Devin CLI prints
every text block the agent emits with nothing between them, so a writing
task's stdout runs narration and answer together
(`…Verifying the scratch repo is clean.done`); the briefing asks the
delegate to end with a `Final answer:` line, the narration before it is
recorded under the call's `transcript`, and `accept` judges only the
answer. The second episode reported "0 passed, 2 failed" for two items
that were done, because the whole stream was judged against `done`
([#9451](https://github.com/OpenAgentsInc/openagents/issues/9451)).

### An explicit list is a source, not a shortcut

`request` is reached through the same code every other source is, and that
is deliberate. The first real burndown will run on work chosen by
inspection, and work chosen by inspection has to exercise the ordering, the
bound, and the collision record that a queried list depends on later. A
second code path for the easy case is a second code path nobody tests.

### What a lookup does, in order

1. **Order.** Whatever the source answered with, in the order the source
   declares — `given` or `id` — and the identifiers are recorded. A burndown
   that silently reorders is not reproducible.
2. **Declared order is enforced.** An item whose `after` names work still in
   the same list is dropped from this batch and recorded as dropped, because
   running the two at once is wrong by construction. #9391 has to land before
   #9401, and that is a fact in the list rather than a judgment about it.
3. **The bound.** More items than `max_results` either truncates or refuses,
   as `on_overflow` says, and the trace records which happened along with
   everything dropped and why.

`delegate-fan-out` refuses. It is the stricter of the two and it is the one
the backlog needs: the work is not independent, the gate that should catch
that is the one #9414 measured at eleven of twelve wrong answers above the
floor, and a lookup that quietly chose six of twenty-one would be making a
selection nobody reviewed.

### Collisions are computed, not asked about

Work items that touch the same file cannot run beside each other, and that
is discoverable without a model: it is in the list. The lookup records every
path more than one selected item touches, and puts the collisions in front
of the decision that follows.

It records them rather than refusing on them, and the asymmetry is the
measurement's.
[#9414](https://github.com/OpenAgentsInc/openagents/issues/9414) put plans
whose tasks genuinely collide to four doors: eleven of twelve answers
cleared the 0.7 bound on the local ones, `kev-8b` at 0.96 and 0.97, and the
wrong answers sat above the right ones, so raising the bound does not help.
Only hosted Jev held. A gate in that state is not where a computable fact
belongs, so the fact is computed and recorded whether or not the gate reads
it. Which pairs genuinely collide is still the gate's question, and fixing
the gate is #9414's work rather than the lookup's.

A plan whose tasks touch six different files says nothing about collisions,
so the state those measurements were taken against reads the way it did.

### Three states, not two

A probe answers **present**, **absent**, or **present and unavailable**.

Absence is not an error. The capability is not an option, which is the
whole reason an operator without Devin loses nothing — the option set for
the program-selection decision is built from what the probe found, so an
absent executor is a route nobody was offered rather than one that fails
when it is taken.

The third state is the one a present-or-absent probe cannot report, and it
happened before it was implemented. Six of six delegations in the
[`coderbench` golden](coderbench.md) were declined with `Refusing to run in
an untrusted workspace` from a git worktree under `/private/tmp`, while the
executor stayed installed and kept reporting its version. A host that reads
that as present offers a route that fails every time.

The manifest states it: `refuses` names what the executor declines while
installed, and `workspace_probe` is the argv a host runs in a candidate
directory to ask. The argv stops short of starting a session, because a
host asks this whenever it considers a directory and a probe that did the
work would charge the operator for a question.

### The manifest drives the executor

`invoke` is the argv that hands one task over, with the prompt appended
last, so a delegation runs the binary the probe resolved under the
arguments the manifest names rather than a name written into the source.
A capability that is absent, or refusing this workspace, produces no
executor — which is how it drops out of a fan-out instead of failing in
one.

### `PATH` is a hint, not the answer

The probe resolves an absolute path and runs that path. `devin` was on the
operator's interactive `PATH` and not on the one a spawned subshell
inherited, and six delegations failed with `command not found` before the
full path was resolved. So the search reads `PATH` for candidate
directories, then keeps looking through the directories a login shell
usually adds, and what it reports — and what a delegation later runs — is
always the resolved path.

## What the decision engine is actually for

This is the part that is easy to get wrong, and we have this week's
measurements saying exactly how.

### The thing not to build

**Do not offer capabilities to the model as tools it may elect to use.**

The reference implementation ran that experiment and published the null: a
model-called capability got **zero calls across 18 attempts on six task
shapes**, in runs making 15 to 99 calls to the general tool it already had,
while the declaration cost **2,307 extra bytes on every request of every
turn**. A second one got zero calls in six of six.

**And do not ask "is this a delegation?" every turn.** On the real-turn
suite, six of seven production questions score no better than answering with
a constant, because most turns are the same kind of turn
([#9395](https://github.com/OpenAgentsInc/openagents/issues/9395)). "Is this
a delegation request" would join them: nearly always no, and a question that
is nearly always the same answer is a latency cost with a false-positive
risk.

The turn asks one anyway, because nothing else reaches the runtime from an
operator's sentence, and it does join them — measured rather than assumed in
[`decision-models/2026-09-19-program-selection.md`](decision-models/measurements/2026-09-19-program-selection.md).
On 32 real turns the constant scores 0.969 and hosted Jev scores 0.938,
which is the warning above coming true. What the measurement adds is the
shape of the error: **no program request was missed**, three ordinary turns
in 35 were answered with a program, every one of those involved
`answer-question` rather than `delegate-fan-out`, and none of them ran
anything, because a program cannot fan out over work the request did not
name. Read that report before changing the question, the option set, or the
programs' summaries.

Nor is keyword matching available, and not only because `AGENTS.md` forbids
it for intent routing. The reference's own capability search ranks a query
against a name and description by bag-of-words overlap — the one place a
carefully built system reached for exactly that, at the one point nobody was
measuring.

### The call site is the operator's sentence

When someone says *delegate six instances, one for each of the top six
issues*, they have already made the decision. Nothing needs to infer intent.
The count is stated, "top six open issues" is a **structured query**, and the
route was chosen by a person.

What the host does not know, and what is genuinely worth a decision model,
is **whether that is safe and admissible**. Those are bounded, typed
questions, asked at fixed points in a program, with option sets built from
what is actually present.

### The four questions

**1. Are these six tasks independent?** The one that matters. Six agents on
six issues that touch the same files produce six conflicting branches and a
mess that costs more than it saved. This is a Noul per pair, it has real
variance on real inputs, and — the rare part — **it has a mechanical outcome
label**: did the branches conflict on merge. That makes it trainable and
gateable rather than a matter of taste.

**2. Does this task need a bound the executor cannot enforce?** The
manifest's `cannot_enforce` is the field this reads. An executor that
ignores a bound is more dangerous than one that refuses it. Partly
deterministic — does the task name a tool restriction — and deterministic
parsing is allowed once the semantic route is already chosen.

**3. Can this task be done without the local checkout?** A cloud lane never
sees the working directory. A Noul, with an outcome label: did the
delegation fail for missing context.

**4. Did the delegated work actually land?** One Noul per stated
requirement, refusing to accept while any is below threshold. This is the
completion gate the
[terminal-bench audit](gym/terminal-bench.md) argues for, and it is the same
question as *is the task complete* asked where it has variance and
consequence instead of where it is 39-to-1 constant.

**None of these is "should I delegate?"** The operator said to delegate. The
engine's job is admission and safety.

The one question that *is* asked before the operator's sentence is read as
work — which program, or none — is the one the turn needs to reach a program
at all, and it is measured with the same suspicion:
[the program-selection report](decision-models/measurements/2026-09-19-program-selection.md)
publishes its baseline and headroom before its accuracy, and counts the two
errors apart.

## The honest risk

On 38 real decisions, routing through a decision model **changed what the
agent would otherwise have done twice, and both changes were wrong.**

That is the whole reason to build the independence question first rather
than all four: it is the one with a free outcome label, so it can be
measured rather than believed. Before it gates anything it needs the
treatment everything else got this week — a baseline, a headroom check, a
noise floor, and a gate that can refuse it. A decision model that fires on
one fan-out in seven and is wrong each time is worse than no decision model,
and
[#9397](https://github.com/OpenAgentsInc/openagents/issues/9397) is a live
example of exactly that shape.

## The first program

`delegate-fan-out`, whose steps are in [NIP-PRG](../nips/openagents/NIP-PRG.md):

```text
select  (query)     the work a named source answers with, ordered and bounded
        ↓
independence (decide)  are these N tasks disjoint?      ← the measured one
        ↓
admit   (check)     does any task need a bound the executor cannot enforce?
        ↓
fan_out (delegate)  one session per task, bounded, isolated per worktree
        ↓
accept  (decide)    per requirement, did each one land?
```

Two steps are deterministic and two are decisions, which is the right ratio:
the program is mostly mechanism, and the decision model is asked only where
a judgment is genuinely required.

`coder::runtime` runs all six from the file, and
[`coder/delegate.md`](coder/runtime/delegate.md) covers the `fan_out` step in
detail: what it records, how it is bounded, and why a refusal, a timeout,
and a failure are three outcomes.

Bounds are enforced, not declared. `concurrent_max` bounds the fan-out,
`isolation: worktree` gives each session a checkout of its own so a
collision is recoverable, and the `minutes` bound reaches the executor only
after the admission check has established that somebody is holding the
delegation to it.

## The runtime

`coder::runtime` is the interpreter. It takes a program, the work, and the
capability slug that is to do the work, and runs the steps the program
lists in the order the program lists them. Four rules make it a runtime
rather than a loop over a list, and all four are NIP-PRG's.

**A step whose bounds the host cannot enforce does not run.** Not a warning
and not a substitution. `Runtime::admit` checks every step's bounds against
the host before the first step runs, so a program this host cannot hold to
fails before it has done anything. The host keeps a table of the bound keys
it enforces per step kind, and it checks the values as well as the keys: a
step naming `isolation: "vm"` is refused, because running it in a shared
directory instead is the substitution the rule forbids.

**A step kind the host does not run refuses the whole program.** An
unrecognized kind is refused when the file is read. A kind this version
recognizes and does not run — `invoke`, which names a host operation this
host has not admitted — is refused. Neither is skipped.

**A `decide` step names a question, never its wording**, and **a `query`
step names a source, never a command.** See the previous section.

**A refused step stops the program**, and the reason it stopped is what the
run reports. An answer below a `refuse_below` floor, a check that will not
admit the delegation, and an executor this machine cannot reach are all
refusals, and each one stops the rest.

### What a step does here

| Kind | What the runtime does |
| --- | --- |
| `query` | Resolves the source the step names, orders the answer, enforces the order the work declares, and holds it to `max_results` — truncating or refusing, as `on_overflow` says. A step naming no source reads the work the request carried, which for a turn is the list the operator's sentence writes out. |
| `decide` | Puts the named question set to a decision door and records `openagents.decision-call.v1`, with the set's identifier and digest beside the answer. |
| `check` | Runs the admission test the `refuse_on` bound names. |
| `delegate` | Hands the work to the executor the capability probe resolved, at the width, isolation, and wall bound the step states. A step with nothing to hand over refuses rather than reporting that none of nothing answered. |
| `program` | Runs the child program the step's address resolves to, nested inside the parent's run. Admission checks the whole composition first: cycles, depth, step and call totals, and bounds that would widen. |
| `module` | Runs the Wasm guest the step carries inline through `plugin::invoke`, under the `pure` or `snapshot-read` profile. The step's `fuel`, `memory_bytes`, `output_bytes`, `read_bytes`, and `module_bytes` bounds narrow the host's ceilings, and admission refuses a wider one. A `snapshot-read` guest reads only the workspace files its `read` scope names, and nothing when the step names no scope. The run's deadline stops a running guest. A step without guest bytes refuses at admission. See [Wasm plugins](extensions/plugins.md#what-is-built). |

### Admission has three answers, not two

For every bound a `delegate` step names, the check records who holds the
delegation to it: this host, the executor, or **nobody**.

| State | What it means |
| --- | --- |
| `host` | The host keeps it, and does not need the executor's agreement. `concurrent_max` and `isolation` are the host's. |
| `executor` | The manifest's `enforces` list names it. |
| `ignored` | The manifest's `cannot_enforce` list names it. Refused. |
| `unknown` | Neither list names it. Refused. |

The third state is the one a two-state answer gets wrong. Intersecting the
required bounds with `cannot_enforce` and admitting everything else admits
a bound that is enforced, apparently, by having gone unmentioned — and the
delegation then runs as though the bound held, with a trace recording that
it was checked. Refusing there is stricter than the `refuse_on` bound
names, which is always allowed: a host never has to run a step.

This does not establish that a delegate respected a bound. It establishes
who claimed to be holding it. Observing what a delegate actually did is
separate work.

### The first program, live

The repository's own `delegate-fan-out`, its own `devin-local` manifest,
its own question sets, a hosted decision door, and the Devin CLI on the
operator's computer:

| Step | What it did |
| --- | --- |
| `select` | 6 of 6 work items |
| `independence` | `independent` 0.96, clearing the 0.7 floor |
| `admit` | `minutes` kept by `devin-local`; `concurrent_max` and `isolation` kept by the host |
| `fan_out` | 6 of 6 answered at a width of 6, one checkout each |
| `accept` | 6 answers, one per requirement |

33.9 seconds of wall clock against 77.6 seconds of summed agent time, six
of six correct, and no faults from the reader that judges the
`devin-fan-out-six` golden. A second run took 47.7 seconds against 186.1
seconds summed, and was also six of six with no faults: the wall clock is
the slowest delegate and the executor is not fast twice in a row. Run it
yourself with the command in `crates/coder/tests/program_run.rs`.

### How a sentence reaches it

`coder::turn::run` asks the selection question before it classifies. A
program answer runs the program; `none` falls straight through to the turn
that was there before. Three rules hold the path together:

- **The option set is the programs this host would admit.** `Runtime::admit`
  already refuses a program whose bounds or steps this host cannot keep, so
  offering one as an option would put a choice on the question whose only
  outcome is a refusal. On this repository that leaves `delegate-fan-out`,
  `burn-down`, `answer-question`, and `review-runs`; `run-suite` requires a protected
  verification plan that ordinary turns do not install, and
  `review-changes` requires a protected reviewer and captured artifact scope
  that ordinary turns do not install. `evidence-guests` declares module
  bounds wider than the terminal's default ceilings, so the terminal never
  admits or offers it; Coder One's probe stage runs it when a manifest
  turns it on, as [Evidence guests](#evidence-guests) describes. `burn-down` is `delegate-fan-out` with the `work-list` source in
  place of `request`, `isolation: worktree`, and thirty minutes per item:
  the program the backlog runs through, where the work is written to
  `.coder/work-list.json` by inspection and every delegate's worktree is
  kept for review. Its `fan_out` step carries a `briefing`, the text every
  delegate reads before its item: the common Git directory is sealed, a
  change is committed to a scratch Git directory inside the worktree, and
  the reviewer fetches from that directory. A briefing is not a
  question's wording and not a command; it is what the program knows
  about the place the delegate runs in and the work list does not. It gates on `openagents.independence.v2`, whose
  wording speaks of the listed tasks; v1 says "the six tasks", which is
  the golden's count, and a three-item list asked v1 came back at 0.06
  on the first real run. A writing task runs the manifest's
  `invoke_writing` argv — for `devin-local`, `--permission-mode dangerous`
  — because an executor that stops to confirm each edit answers a fan-out
  with nothing done; the worktree boundary, not the executor's own
  confirmation, is what holds it.
- **The work the request carries is the list the sentence writes out.** One
  task per bulleted or numbered line, in the order written. That is what the
  `request` source reads, and a `query` step naming a file source reads a
  burndown instead. Reading a list is deterministic parsing of a bounded
  field, which is allowed once the semantic route has been chosen — and the
  route was chosen by a decision model one step earlier.
- **A step with nothing to work on refuses.** A lookup that found no work
  and a `delegate` step with nothing to hand over both stop the program.
  That is what bounds a wrong selection: an ordinary turn writes out no
  list, so a program chosen for one declines instead of running.

`review-runs` answers a question about Terminal-Bench runs: a `query` step
over the `gym-runs` command source, then a `delegate` step whose binding
names its executor, `coder-one-ask`, which runs `coder-one ask --scope gym`.
A `delegate` step may name its executor by capability slug, and a program
that does runs its delegation there whatever `CODER_DELEGATE` says; a
capability a program names is that program's, and the session never picks
it, or an adapter such as `gym`, as the default executor. A turn whose
program handed one piece of work to one executor replies with what the
executor answered, then the run's summary. Both capabilities need an
approval, and `coder-one-ask` needs its state granted:

```sh
capability-trust approve gym
capability-trust approve coder-one-ask --writable ~/.codex --writable ~/.openagents/coder-one/asks
```

Keep the trust store outside `~/.openagents` when the ask records there: a
delegation seals the directory that holds the store. The selection
question's five-option baseline is
[program-selection-v3](decision-models/measurements/2026-09-23-program-selection-v3.md).
[Ask Coder One about runs](coder/guides/coder-one-ask.md) covers the ask
itself.

Three live episodes have taken the path from a sentence, the first in 25.6
seconds: six delegations, six correct answers, and nothing on the path that
`coderbench run devin-fan-out-six` calls a fault. What the grade still
cannot see is in [`coderbench.md`](coderbench.md).

### What the runtime does not do yet

- **Read a source that is not a file.** Both sources are reads. An
  executable source is where a lookup would reach a tracker directly, and it
  waits on the trust boundary and subprocess bounds in
  [#9427](https://github.com/OpenAgentsInc/openagents/issues/9427).
- **Run an `invoke` step.** The kind parses and refuses, because no host
  operation is admitted.
- **Fetch.** `coder` reads programs from `programs/` on disk, and a
  `module` step carries its guest's bytes inline. `coder` doesn't resolve
  a program or a module from a relay or a catalog. The
  [interoperability suite](coder/verification/2026-09-22-relay-interoperability.md)
  fetches a program and a guest over a relay in a test, but no product path
  does. `evidence-guests` is the one program in `programs/` with `module`
  steps, and it carries its three guests inline.
- **Grant a guest anything but workspace files.** A `snapshot-read`
  module step's `read` scope names workspace paths only.

### Evidence guests

`programs/evidence-guests.json` is the first program built from `module`
steps. Each step runs a `snapshot-read` Wasm guest over a `read` scope of
`.`:

| Step | Guest | What it returns |
| --- | --- | --- |
| `repo_map` | `crates/plugin-repo-map`, operation `map` | Files, bytes, languages, top-level entries and the directories below them, the largest files, build manifests, and test files. |
| `code_search` | `crates/plugin-code-search`, operation `search` | Lines that match up to 16 literal patterns, where `*` matches within a line, grouped by file, files that match more patterns first. |
| `test_report` | `crates/plugin-test-report`, operation `parse` | Failing tests with file, line, and message from JUnit XML, `cargo test` output, and pytest output. |

These revive the pre-reset evidence plugins, which the model never called
when they were offered to it as tools. Here code calls them, and no model
sees them as tools. Coder One's probe stage runs the program when a policy
manifest sets `policy.evidence.guests` (for example, `"guests": {}` for all
three steps), which needs `evidence.probes`. The switch is absent from
every checked-in manifest, so no manifest's digest changed. Code decides
each step's input: the search runs only when the issue yields search terms
and uses them as its patterns, and the report parser runs only when a
granted file's content shows a test report. Each output joins the probe
battery's outputs at the probe keep question, so Jev decides what reaches
the briefing, the same as for a command's output. `crates/coder-one/src/guests.rs`
is that host; [Wasm plugins](extensions/plugins.md#evidence-guests) covers
the build, the grant, and the measurement plan that decides whether each
guest stays.

## What to build first

1. **A capability manifest for `devin-local`**, resolved from a local file
   before anything is fetched from a relay. Presence probe, `enforces`,
   `cannot_enforce`, `sees_repository`. **Landed**, with the program
   registry read beside it — see [The local registry](#the-local-registry).
2. **The `independence` question**, as a typed question with a question-set
   digest, served through the existing contract so all three doors can
   answer it.
3. **A suite of real fan-out decisions**, labelled by whether the branches
   conflicted. This is harvestable from history: pairs of issues that were
   worked in parallel, and whether their branches collided.
4. **The program, as a host-driven call site**, with every step recorded in
   the trace. **Landed**, and reached from the operator's sentence — see
   [How a sentence reaches it](#how-a-sentence-reaches-it).
   [#9400](https://github.com/OpenAgentsInc/openagents/issues/9400) puts
   decision calls in ATIF's `extra`, which is where the evidence for step 3
   comes from next time.
5. **Only then**, the relay: publish the manifest and the program as
   `30180`/`30182` and fetch them by address.

Steps 1 through 4 need no Nostr at all, and doing them first means the wire
format is designed against something that works rather than the reverse.

## What generalizes

Devin is one row. An operator without it loses nothing, because **the option
set is built from what `detect` finds** — an absent capability is not an
option, not a failure.

The manifest shape holds for the built-in runner, another agent's CLI, a
cloud lane, or a subprocess: each has bounds it keeps, bounds it ignores,
and a relationship to the checkout. `cannot_enforce` is the field that makes
a heterogeneous set safe to choose among, and it is the field a system built
around one executor would never have thought to add.

The four questions are about delegation in general, not about Devin. The one
worth measuring first is independence, because six parallel agents is a good
idea exactly when the six pieces of work do not touch, and nobody can tell
by looking.

## Operator authority and project supervision

Program selection proposes a program; [operator authority](coder/guides/program-authority.md)
determines whether it may run. [Scoped tracker intake](coder/guides/tracker-intake.md)
feeds host-prepared tasks into the existing source contract. The
[project supervisor](coder/guides/project-supervision.md) adds a durable outer queue with
resource admission, immediate refill, and independent result review.
