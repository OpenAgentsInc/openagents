# System One hooks: judgments inside a constrained Claude

2026-10-12. Why Coder puts Claude Code (and other executors) in a box,
what hooks add to that box, and the ideas future work should derive
tasks from. Every judgment named here goes through the Decisions API
(`POST https://openagents.com/api/v1/decisions`, Clef first;
[docs/api/decisions.md](../../api/decisions.md),
`jev::decisions::DecisionsClient` in Rust), not a direct Jev call.

## Sources

- Episode 286, "System One in Coding Agents"
  ([transcript](../../transcripts/286.md)): the KV cache shapes today's
  agents; most of what they send the big model is System 1 work.
- Episode 287, "Building a System One Coding Agent"
  ([transcript](../../transcripts/287.md)): Coder One, the probe and
  briefing, lean Claude Code, the measured panels
  ([winning-runs-analysis.md](../../terminal-bench/winning-runs-analysis.md)).
- TypeSafe research ([docs/research/typesafe/](../../research/typesafe/)):
  three Diogo Almeida talks and the calibration memo.
- The briefed-agent A/B (#11211): on 21 closed issues with 3 runs each,
  the briefed agent accepted 19 of 63 like bare Claude Code, at a median of
  $0.52 against $1.19 and 121 s against 284 s. Bare Claude Code re-reads
  about a million tokens per run (its own system prompt plus about 23
  shell outputs); the briefed agent about 460,000.
- The system prompt capture
  ([claude-code-delegate-prompt/](../../terminal-bench/claude-code-delegate-prompt/README.md))
  and the 2026-10-12 retest below.

## The thesis

1. **Most of the work is System 1.** Deciding which files matter, which
   chunk of a log is the error, whether a step helped, whether the task is
   done: these are typed judgments a decision model answers in a fraction
   of a second for a fraction of a cent. Only writing the change needs the
   big model. "The thing that we actually need the model for is not the
   majority of the work that you're passing to it" (287).
2. **Code owns control. The decision model judges. The executor
   generates.** A probability informs a decision the code makes; it never
   grants authority, runs a command, or declares a task done by itself.
3. **Evidence, not advice.** Judgments fed to a model as hints "barely
   helped"; using them to choose *what the strong model sees* changed the
   outcome (287). "Information beats instruction." "A completion claim
   needs an observation."
4. **Two kinds of model, two jobs.** An RLHF'd assistant (Claude Code)
   produces plausible text for a person; a calibrated decision model
   produces probabilities for code. "Don't make decisions with stakes" with
   the first; gate them on the second with thresholds that come from the
   cost of being wrong, not from the model.
5. **The KV cache is the constraint to design around.** Changing the front
   of a conversation re-processes everything after it, so routing,
   compaction, tool lists and context selection are all bent around it
   (286).

## Where we are

- **The box.** The briefed agent (`crates/briefed-agent`) is one Claude
  Agent SDK session with a briefing-built system prompt, a short tool list,
  file tools held to the worktree, and `verify`/`finish` tools that run
  only the declared checks. Coder's chat runs Claude Code through the same
  SDK (`crates/coder-new/src/claude.rs`) with its floors hook.
- **The system prompt is ours.** `--system-prompt` (the SDK's
  `SystemPromptConfig::Custom`) replaces Claude Code's 5,600-character main
  prompt. Measured on Claude Code 2.1.296: the request then carries a
  billing header, the one-line identity "You are a Claude agent, built on
  Anthropic's Claude Agent SDK.", our text, the tool definitions, and a
  trailing environment block (working directory, platform, model, date).
  `--append-system-prompt` keeps all of theirs and adds ours at the end.
  With `--setting-sources ""` the commit-attribution reminder is gone too.
  A live call under a replaced prompt followed it (name and secret word).
- **Judgments sit outside the session.** Probes, the file survey and the
  briefing run before Claude starts; the close check runs after. Inside the
  session Claude is on its own. Hooks are how we get in.

## Why hooks

The Claude Agent SDK delivers 33 hook events to an in-process Rust
callback (`claude_agent_sdk::HookEvent`): `PreToolUse`, `PostToolUse`,
`PostToolBatch`, `UserPromptSubmit`, `Stop`, `PreCompact`, `PostCompact`,
`FileChanged`, and more. A hook can deny with a reason, allow, ask, rewrite
the tool's input (`updatedInput`; checked live on 2026-10-12: a rewritten
Bash command is the one that runs), add context, block a stop with a
reason, or end the session (`continue: false`).

What a hook adds lands at the **tail** of the conversation, so the cached
prefix (system prompt, tools, briefing) stays cached. Hooks are the
cache-safe place for System 1 between the executor's steps. The price: text
a hook adds stays in the history and is re-read on every later call, so
inject little, and only when a judgment says it matters.

Codex takes the same `PreToolUse` command hooks (`-c hooks.PreToolUse=...`,
as `floors::codex_args` does), so a hook written as a command works on both.

## The ideas, by hook

Each is one task-sized idea. "Deterministic first": a rule or parser
decides when it can, the Decisions API only when the rule is unsure.

1. **Shrink output before it enters the history** (`PreToolUse`,
   `updatedInput`). A noisy command (tests, builds, recursive searches) is
   rewritten to run through a squeezer that keeps the head, the tail, and
   the chunks a decision says decide the next step, writes the full output
   to a file the agent can read, and keeps the exit status. Attacks the
   measured cost driver directly: shell outputs re-read every turn.
2. **No done without an observation** (`Stop`). When the executor stops,
   the host runs the named checks; a decision gives P(checks pass | the
   agent says done). Below the threshold the stop is blocked with the
   failing evidence as the reason; above it the run ends with no extra
   turn. Replaces prompt wording ("verify everything") that cost Opus 50%
   more time in v3.
3. **The monitor** (`PostToolUse`, `PostToolBatch`). After each step a
   decision picks continue, steer (one line of host-observed evidence),
   fetch evidence (the host runs a probe and adds only its result), or
   escalate (`continue: false`; the loop re-briefs a stronger executor).
   Starts from the calibrated stall detector (`crates/coder-one/src/stall.rs`).
4. **A three-band gate over the floors** (`PreToolUse`). The floors'
   parser keeps the hard decisions. A decision answers the ambiguous
   middle: "this edit is outside the files the briefing named", "run a
   script you have not read". Deny with a reason or ask; never allow what a
   rule denies.
5. **Conditional context** (`PreToolUse` on `Read`/`Edit`, `PostCompact`).
   The first touch of a directory adds that directory's gotchas or ADR,
   picked by a decision; after a compaction the briefing's key facts are
   added back.
6. **Route each later message** (`UserPromptSubmit`). A choice over
   continue / answer from the host / re-brief, so multi-turn chats keep the
   briefing's benefit past the first message.
7. **Log every hook decision.** Raw probability, threshold, decision,
   model and door, then the outcome joined later (did the run pass, was the
   steer followed). Without the join no threshold above can be calibrated
   (the calibration memo's open gap).

## Constraints

- **Latency.** A decision takes about 0.2 to 0.6 s. On every one of ~23
  tool calls that is 5 to 14 s against a 121 s median. Use matchers, decide
  by rule first, and run pure monitoring asynchronously.
- **Steering is not hinting.** A steer carries something the host
  observed (a file, an output chunk, a probe result), never "consider X".
- **Thresholds come from costs.** Act when p > 1 − C(escalate)/C(wrong
  act); write the costs down next to the threshold, pin the model, and
  replace hand-picked numbers once the log (idea 7) has outcomes.
- **A failed decision call never blocks work.** Fall back to the rule's
  answer, or to doing nothing.
- **One change at a time** on the briefed-ab set
  (`scripts/bench/briefed-ab`), with the noise stated, before a lever
  becomes a default.
- **Bash is coming to the briefed agent.** Anything that shapes shell
  output is a shared library both Coder's Claude session and the briefed
  agent (and Codex, through a command hook) can install, not Coder-only
  code.

## Deriving tasks

When picking the next piece of work from these ideas, ask in order:

1. Which measured cost or failure does it attack (tokens re-read, turns,
   wall time, wrong accepts)? No measured target, no task yet.
2. Is the judgment a rule could make made by a rule?
3. Does what it adds land at the tail, and is it evidence?
4. Does it log its decision for the outcome join?
5. Which A/B run, changed in one lever only, settles it?

## Issues

Tracked on project 22.

| Idea | Issue |
| --- | --- |
| 1. Squeeze shell output | [#11486](https://github.com/OpenAgentsInc/openagents/issues/11486) |
| 2. No done without an observation | [#11487](https://github.com/OpenAgentsInc/openagents/issues/11487) |
| 3. The monitor | [#11488](https://github.com/OpenAgentsInc/openagents/issues/11488) |
| 4. Three-band gate | [#11489](https://github.com/OpenAgentsInc/openagents/issues/11489) |
| 5. Conditional context | [#11490](https://github.com/OpenAgentsInc/openagents/issues/11490) |
| 6. Route later messages | [#11491](https://github.com/OpenAgentsInc/openagents/issues/11491) |
| 7. Decision log and outcome join | [#11492](https://github.com/OpenAgentsInc/openagents/issues/11492) |
