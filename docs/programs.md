# Capabilities, executors, and programs

How a Coder instance learns what it can hand work to, how the decision
engine chooses, and why the reusable component is a **program** rather than
a plugin.

The worked case throughout is delegating to the Devin CLI, because it is the
one an operator here actually has and wants used. It is a row in a table,
not a special case, and the last section says what generalizes.

## The word

"Plugin" is the wrong word, and the reasons are not stylistic.

The glossary in the reference implementation already allocates the terms,
and three of them matter:

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

That last point is structural rather than a preference. The plugin host
sorts guests into three tiers: pure compute loads without asking, a guest
wanting read-only directories needs an operator, and a guest declaring
network access **never loads**. A Devin delegation spawns a process, reaches
the internet, writes files, and is not deterministic. It is the tier that
never loads.

This is the second time the same boundary has appeared this week from a
different direction. The
[capability-sockets review](decision-models/research/2026-09-19-capability-sockets.md)
asked whether a trained adapter could move through the plugin socket and
found the same split: the **manifest** half of that system generalizes and
the **sandbox** half does not. Two payloads, two independent analyses, one
conclusion — "plugin" was always two things wearing one name, and the part
worth keeping is the declaration.

**A program is the composable unit.** Programs compose because a step's
output is the next step's input and every step carries bounds. Plugins
compose badly even in the reference implementation, where two members
eligible for the same call would race and the rule table exists to forbid
it.

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
select  (query)     the top N open issues
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

The `fan_out` step is the one `crates/coder` implements today:
[`coder/delegate.md`](coder/delegate.md) covers what it records, how it is
bounded, and why a refusal, a timeout, and a failure are three outcomes.
Nothing yet reaches it from an operator's sentence.

Bounds are enforced, not declared. `concurrent_max` bounds the fan-out,
`isolation: worktree` gives each session its own branch so a collision is
recoverable, and the `minutes` bound comes from the manifest's `enforces`
list rather than from hope.

## What to build first

1. **A capability manifest for `devin-local`**, resolved from a local file
   before anything is fetched from a relay. Presence probe, `enforces`,
   `cannot_enforce`, `sees_repository`.
2. **The `independence` question**, as a typed question with a question-set
   digest, served through the existing contract so all three doors can
   answer it.
3. **A suite of real fan-out decisions**, labelled by whether the branches
   conflicted. This is harvestable from history: pairs of issues that were
   worked in parallel, and whether their branches collided.
4. **The program, as a host-driven call site** reached by the operator's
   request, with every step recorded in the trace.
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
