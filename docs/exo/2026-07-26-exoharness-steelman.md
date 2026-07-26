# The Exoharness Steelman — the Project's Own Case for Its Trajectory — 2026-07-26

Agent-facing steelman of `exoharness/exo` on its own terms. The exercise rule
for this document: **ignore every OpenAgents contribution, fork, adapter,
and integration.** No ACP transport, no Nostr adapter, no Omega lane, no
OpenAgents room appears in the argument below — this is the strongest honest
case the exoharness project itself would make for its design and its future,
reconstructed from its own published words: the landing page and README, the
spec (`docs/spec.md`), the RSI essay (`docs/RSI.md`), the roadmap
(README "Ongoing Work", PR #144), the harness-hosting doc
(`docs/coding-agent-harnesses.md`), and the commit history, read at the
local reference pin (`baa07f6` plus the public tree through #163). A
steelman is the strongest version of their argument, not an endorsement, and
the extrapolations are labeled. This document grants nothing and changes no
authority.

## 1. The thesis, in their own words

The landing page compresses the project to one imperative: **"write events,
not hidden state."** The spec expands it: most agent harnesses conflate two
different concerns — the infrastructure that serves an agent (history,
secrets, sandboxes) and the semantics of how the agent thinks and acts
(prompts, compaction, tools, approvals). Exoharness splits them. The
substrate owns durable state and brokers privileged resources and owns no
semantics. The executor owns the turn loop and every semantic choice, and
the landing page gives the executor its epitaph in two words —
"ephemeral" and "replaceable." The agent is a durable identity and history that
any executor can interpret.

The point of the minimalism is stated, and it is the load-bearing sentence
of the whole project: keeping only the irreducible substrate in the
exoharness maximizes "the space of behaviors that can evolve above it."
Memory, compaction, orchestration, and execution strategy become
programmable — even agentic — while the system stays recoverable.

## 2. Their strongest arguments, made as strong as they deserve

### 2.1 Records outlast behaviors

Every harness on the market is a behavior layer, and behavior layers are
churning weekly — SDKs break, prompt formats rot, vendors deprecate.
Exoharness bets the project on the only layer with a stable claim to
permanence: the record of what happened. Prompt history is demoted to "a
view the executor derives" from the log. When executor code changes, a tool
crashes, compaction goes wrong, or the agent edits its own implementation,
the record survives and the next executor reinterprets it. This is the same
move that made databases outlive applications and made Git outlive every
workflow built on it. The substrate is the schema of experience, and
whoever owns the schema of experience owns the durable asset.

### 2.2 The recursion argument is more precise than the field's

The RSI essay draws a distinction most RSI talk skips: autocatalysis (a
technology speeding up its own development — computers, steam, the
Internet) versus recursion (a complete version of a thing building the next
complete version, like a compiler compiling itself). Recursion, they argue,
requires runtime support — a language needs scoping and a call stack before
recursive functions are safe. Their claim: an agent needs the same, and the
append-only event log is that runtime support — "not exactly a call stack
but more a complete execution history that nothing can erase." An agent
that breaks itself, rewinds, and tries again "can see what it already
tried, instead of repeating the same mistake in a loop." Lineage survives
cloning. The log survives sandbox rewind. And exactly one component is
protected from the agent that rewrites everything else: the substrate
itself — with the candid footnote that even that protection "is actually a
policy consideration," enforced by default configuration rather than
physics. This is the most intellectually serious framing of harness-level
RSI published by any current agent project, and it earns the tagline: the
agent can change anything about itself except its own history.

### 2.3 The Bitter Lesson, applied one layer up

Their argument: general methods beat hand-engineered ones, and as models
get smarter, "the hand-engineered harness is the next thing to fall." Every
competing project is busy freezing agent behavior into code — planners,
memory schemes, tool policies. Exoharness deliberately refuses to compete
at that layer, because that layer is exactly what smarter models will
rewrite. The winning position is beneath the churn: provide the minimal
scaffolding that makes unfettered self-modification survivable, and let the
model own everything else. If the premise holds, every hand-built harness
is technical debt with a model-capability fuse attached, and the substrate
that assumed its own replacement from day one is the only one that ages
well.

### 2.4 Hosting the incumbents is an aggregation move

The repository does not merely tolerate Codex, Claude Code, and Cursor — it
runs them, as executor presets (`exo repl --harness codex`), with their
native runtimes inside exoharness sandboxes and their conversations
recorded as exoharness events. The spec names the ambition plainly: support
higher-level harnesses "by virtualizing their exoharness-like components
(e.g. `config.toml`)." Steelmanned, this is the classic aggregation play:
make the vendor harnesses interchangeable interpreters above a durable
agent they do not own. The user's agent — its identity, its memory, its
history — stops being hostage to any one lab's SDK. Switching executors
becomes a flag. If the incumbents churn, the agent persists, and the
substrate quietly becomes the layer everything else is an implementation
detail of. `--harness ./my-harness.ts` extends the same move to everyone
else: the executor ecosystem is open by construction.

### 2.5 Security by construction, not by interruption

The substrate's security posture is coherent once read on its own terms.
Sealed secrets let programs use credentials the model can never see —
mounted into sandboxes, injected into workers, referenced by non-secret
bindings, encrypted at rest. Sandboxes carry workspace mounts, network
policy, and snapshot lifecycle. And approvals are deliberately executor
semantics, not substrate policy — because an approval flow is a behavior,
and behaviors must remain replaceable. The substrate refuses to own the
permission model for the same reason it refuses to own compaction: baking
policy into the trusted layer would freeze exactly the thing that should
evolve. Isolation bounds the blast radius, the log makes every action
auditable after the fact, and rewind makes many mistakes cheap. Within
their frame, gates are a UX choice some executors will make, not a
substrate obligation.

### 2.6 Time travel is a product primitive, not a debugging trick

Fork or rewind from any event, and the entire agent — conversation, memory,
artifacts, with sandbox snapshots extending to filesystem state — is
recreated at that point "without losing resources." The obvious uses are
recovery and experimentation. The deeper one is the third roadmap act:
cloning with full lineage is the primitive for populations of agents, where
dividing work means forking a history and comparing results means comparing
futures grown from a common past.

## 3. The trajectory they would claim

Read from the roadmap, the commit arc, and the docs, the project's own
story of its future has three acts plus a stated horizon.

**Act 1 — make the loop survivable and adoptable (now, visible in
commits).** The recent history is launch polish with a clear intent:
REPL resilience against model failures (#156), the TUI upgrade (#163),
docs restructure and a spec promoted to the front door (#149), setup and
install work, a Discord, and the website renamed from the agent to the
substrate (#146) — the brand is exoharness, and exo the agent is the
reference demonstration. Early software, unstable API, said out loud.

**Act 2 — the durable agent becomes portable and self-tending (the
roadmap).** Three named workstreams, each with a stated success
criterion. Autonomous self-maintenance: the agent periodically inspects
its own context, memories, tools, scheduled tasks, and processes,
cleans up safely, and records what changed — success is "a long-running
agent becomes more organized over time rather than accumulating context
and tool rot," which is the sharpest one-line answer to context rot any
harness project has committed to. Recoverable, portable execution:
long-running work resumes after process, machine, or network failure and
moves "between compatible machines without losing canonical history or
duplicating side effects" — and the storage layer already speaks
`object_store` (S3, GCS, Azure), so the durable agent as a movable object
is an architectural intention, not a retrofit. Plus the standing intents:
MCP, native tools, and generalized computer use, all admitted as
unimplemented.

**Act 3 — families of agents (the roadmap's end state).** Cloning and
lineage graduate from primitives to policy: when to clone, how to divide
work, how children report, how conflicting conclusions resolve, and when a
lineage stops. The stated goal is "a family of agents that produces better
results than one agent without creating unbounded cost or coordination
overhead" — selection over lineages, on a substrate where every family
member shares a replayable ancestry.

**The horizon (their own footnote).** Full RSI "would also improve the
model, the compute underneath it, and the power feeding it all. Perhaps an
RSI harness is a step on that path." The modesty is doing work: they claim
the harness layer only, and place it as the first rung of a much longer
ladder.

**The business steelman.** `[SPECULATION]` The project sits in the
Braintrust orbit, with the model layer already flowing through Braintrust's
`lingua` and router. A substrate whose every agent action is a typed,
durable, replayable event is also, incidentally, the cleanest
evaluation-and-training data generator ever attached to an agent. The
plausible commercial arc writes itself without any public statement:
hosted durable agent state, fleet-scale observability over event logs, and
the neutral substrate position under everyone's executors — monetize the
record, never the behavior.

## 4. Their answers to the obvious objections

- **"No releases, no backwards compatibility."** The API is not the
  compatibility surface — the event log is. Freezing a pre-1.0 API would
  fossilize the wrong interface, and the house rule against fallback code
  keeps the substrate minimal while the design space is still open. The
  record's durability is the promise. Everything else is scaffolding.
- **"No permission gates."** Gates are semantics, and semantics belong to
  executors — a substrate-level permission model would push policy into
  the one layer that must stay neutral and unchanging. Isolation, sealed
  secrets, and rewind bound the damage. The protected-substrate default
  (footnote 2) shows the policy hook exists where policy belongs.
- **"Bus factor of two or three."** The committer roster includes the
  agent itself — roughly fifty commits across branches carry the agent's
  own identity. A project whose thesis is that agents build agents, being
  measurably built by its agent, is dogfooding at the thesis level. The
  small human core is what minimalism looks like from the inside.
- **"Millions of events cannot scale."** Events are UUIDv7 with typed
  cursor scans, derived history caches are explicitly executor policy, and
  the store already abstracts over cloud object storage. The design
  separates the truth (large, cheap, append-only) from the working set
  (small, derived, disposable) — which is how every log-structured system
  that scaled has scaled.
- **"The agent grades itself."** The substrate's reply is narrow and
  honest: grading is a behavior, and behaviors are the executor's problem.
  What the substrate guarantees is that no grade can falsify the record —
  the complete history is always there for any skeptic, human or program,
  to replay. Audit is possible by construction. Whether anyone performs it
  is above the substrate's pay grade — deliberately.

## 5. The falsifiable bets

A steelman worth writing states what would prove the project right in its
own terms, so the claims stay claims:

1. **The executor swap works.** One agent, months of history, moved
   between the built-in executor and at least two vendor harnesses with no
   loss of competence — demonstrating that identity really lives in the
   log, not in the loop.
2. **Self-maintenance beats rot.** A long-running agent measurably more
   organized at month six than at month one, by their own stated
   criterion, with the cleanup itself recorded as events.
3. **A lineage beats an individual.** A forked family, under their Act 3
   policies, producing better results than one agent at bounded cost — the
   first controlled win for selection over histories.
4. **Virtualization holds.** Codex or Claude Code running above the
   substrate with their native state faithfully virtualized, surviving an
   upstream SDK change that breaks standalone installations.
5. **The log survives its agent.** An adversarial self-modification
   campaign that rewrites executor, tools, and prompts while the
   append-only history remains complete and replayable — the recursion
   claim, tested at the only layer it can be tested.

If those five land, the project's own sentence becomes hard to argue with:
the harness was never the product. The history was, and exoharness got
there first.

---

*Series note: this steelman deliberately excludes all OpenAgents material.
The integration state, the verification counter-argument, and the network
case live in the companion documents in this folder.*
