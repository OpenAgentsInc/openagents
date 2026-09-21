# Coder agent and terminal rebuild plan

Status: implementation overview and next steps, checked against `9dd4ddab67`
on 2026-09-21. The [consumer contract](coder-as-decision-router-consumer.md)
defines the product boundary. The [TypeSafe-native analysis](typesafe-agent-analysis.md)
and [roadmap](typesafe-agent-roadmap.md) provide the architecture and current
implementation order derived from the founder's coding-agent proposal.

## Design and repository boundary

Coder separates typed semantic judgments from open-ended generation and
host execution. `Classify` answers bounded questions about supplied state.
`Generate` writes explanations, plans, and code. Rust determines what those
answers mean, what may run, and how observations return to the next step.
Typed answers can be wrong; their value must be measured against simpler
rules on the workload that consumes them.

The terminal and two-tool design were inspired by earlier private work and
reimplemented here. Product code and contracts in this repository are public.
Do not copy private backend code, prompts, endpoints, credentials, or other
private source. The public `ResponsesDoor` client now implements generation
against configured Open Responses endpoints; a private generation client is
not a prerequisite. A credential-free stub remains available.

## What has landed

`crates/coder-terminal` owns the shared terminal design system:

- `Intensity` supplies the four-step amber ladder: `Quarter`, `Half`,
  `ThreeQuarters`, and `Full`, over a near-black field.
- `Ladder` adapts that intensity to truecolor, 256-color terminals, and
  `NO_COLOR` through dim styling.
- `Editor` handles grapheme and word movement, editing, word wrap, a
  two-to-eight-row composer window, and prompt history.
- `Composer`, `frame`, and `rail` draw the input frame, prompt, status,
  location, and token information. Other repository terminals use this
  foundation rather than defining separate visual systems.
- `Guard` restores terminal state across fallible setup, normal exit, and
  panic. Event delivery preserves control events and counts dropped text
  previews; scrollback is bounded and wrapping is cached.

See [terminal lifecycle and events](../runtime/terminal.md) for the implemented
contracts and verification. The example shell exercises the visual core;
`cargo run -p coder` runs the agent.

`crates/coder` provides the live agent:

| Component | Current behavior |
| --- | --- |
| `turn::run` | Shared terminal/headless turn; program selection precedes the ordinary action path |
| `classify` | `coder-turns-v2` action Choice (`respond`, `clarify`, `end_conversation`, `none`) and a separate shell-outcome question; missing action answers halt, while `none` takes the unrouted response path |
| `generate` | Streaming Open Responses client, configured provider/model, relay or local-executor doors, and a stub when no generation door is selected |
| `agent` | Conversation state, typed decisions, streaming replies, bounded shell rounds, and trace recording |
| Program runtime | File-defined queries, decisions, checks, delegation, and explicit host grants; unsupported kinds refuse |
| Repository workflows | Scoped tracker intake, bounded local/relay delegation, worktrees, project scheduling, and independent artifact checks |

The [shell loop](../runtime/shell-loop.md), [program authority](../guides/program-authority.md),
[tracker intake](../guides/tracker-intake.md), [project supervision](../guides/project-supervision.md),
and [artifact verification](../guides/artifact-verification.md) describe the actual
bounds. A manifest in the registry does not imply that every proposed
program or step kind is implemented.

## The two model capabilities

### Typed judgments

A decision call sends structured state and explicitly worded questions.
Choice selects among supplied alternatives; Noul assesses independent
propositions; Score uses an ordered rubric. The caller's IDs associate
answers with their consumers; IDs do not replace model-visible wording.

Use each primitive where it fits:

- Include a no-match path when none of the alternatives may apply.
- Select real candidates for closed choices, such as known files or
  eligible operations. An omitted candidate cannot be recovered by selection.
- Ask independent questions together when they share evidence and the
  request fits backend limits. Dependent questions need the preceding
  observation first. Batching does not make arbitrary question growth free.
- Keep state, question, model/artifact, and consuming policy identities
  together. A threshold needs workload evidence; confidence alone is not
  authority or correctness.
- Retain deterministic baselines and typed unavailable/refused outcomes.
  Do not silently change providers to satisfy a required judgment.

The [question baseline record](../../decision-models/measurements/2026-09-20-coder-question-baselines.md)
retired `needs_code`, `risk`, `progress`, and other questions that did not
justify their place. The current action question also has weak evidence
against constant response on its retained real-turn distribution. A richer
agent should introduce specific useful functions rather than restore a
large generic question set.

### Generation

`Generate` is the public asynchronous interface implemented by
`ResponsesDoor`, the configured door variants, and the stub. It streams
available output and reports usage where the provider supplies it.
`CODER_DOOR_URL`, `CODER_MODEL`, and the documented credential/profile
settings select the generation path. See [headless usage](../guides/headless.md) and
[relay transport](../measurements/relay-transport.md) for their current contracts.

The generator currently receives the in-memory conversation, while action
classification uses bounded state slices. A common evidence store and
context manifest are proposed, not already implemented. The next design
makes generation consume a task-specific context with source references,
applicable instructions, and known omissions.

Generation remains necessary for new patches, open-ended arguments,
explanations, and summaries. The host validates structured proposals and
source versions before execution. A generated shell plan is executable only
under the existing shell permit; a selected program needs its own host grant.

## The next agent increments

Follow the detailed [roadmap](typesafe-agent-roadmap.md):

1. Unify decision configuration and outcomes across `Agent` and `Runtime`.
   Carry function, model, attempt, and available receipt identities.
2. Add versioned evidence and task frames. Build deterministic candidates,
   one measured relevance function, and inspectable context manifests.
3. Complete a native repair flow with bounded reading/search, anchored edits,
   retained diagnostics, suitable execution budgets, and independent checks.
4. Load optional operation schemas and guidance progressively. Preserve
   mandatory instructions by scope and precedence. Measure generation
   routing against full-task quality and context/cache costs.
5. Share snapshots and structured findings between tasks. Add one bounded
   background explanation that reuses observations and yields to the user.
6. Extend current claim recovery into complete durable programs, composition,
   whole-run accounting, and portable packages.

All increments preserve `turn::run` and the public decision contract. A
native Coder context improvement does not change the hidden internal loop
of an external executor such as Devin.

## The next terminal increments

Keep the composer and conversation as the primary surface. Add expandable
views for the task, evidence, decisions, and artifacts through the same
shared event stream. Show source freshness, unknown outcomes, and independent
verification before adding decorative confidence displays. Background
findings carry the revision they describe and do not interrupt the user
when nothing actionable changed.

The [planned view contract](../runtime/terminal.md#planned-evidence-and-task-views)
defines behavior for terminal and headless clients. Markdown rendering,
selection/paste improvements, and syntax emphasis can improve readability
without creating a second agent or replacing the amber design system.
