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

   **The log (`crates/hook-log`).** Every hook writes the same line through
   `hook_log::append`: `hook`, `question`, `p` (raw probability), `threshold`,
   `action`, `how` (`rule`, `decision`, or `fallback`), `model`, `door`,
   `latency_ms`, `run_id` (from `CODER_RUN_ID` when set), `ts` (ms), and a
   free `detail`. It lives at `~/.openagents/decisions/<hook>.jsonl`;
   `CODER_<HOOK>_LOG` overrides the path (`CODER_SQUEEZE_LOG` still works).
   The squeezer writes one line per squeezed run, with the per-chunk ratings
   in `detail.rated`.

   **Outcomes file.** One JSON line per run:
   `{"run_id": "...", "passed": true, "labels": [{"decision_ts": 1760000000000, "correct": true}]}`.
   `passed` is the run's check result; a label says whether one decision (its
   `ts`) was right. Labels are optional.

   **Report.**
   `cargo run -p hook-log -- report ~/.openagents/decisions/gate.jsonl --outcomes outcomes.jsonl`
   joins on `run_id` and `ts`, then prints per hook and question the count,
   mean p, a 5-bin table (n, mean p, observed rate), and ECE (expected
   calibration error: the bin gaps weighted by size; 0 is perfect). Without
   `--outcomes` it prints counts and the p histogram.

## Token savers: the plugin every lever lives in

Every lever ships inside one built-in plugin, **Token savers**
(`token-savers`, #11494, `crates/coder-new/src/token_savers.rs`). It is on
by default, and a person turns it off on the Plugins screen (`/plugins`,
Space).

- **One install point.** A lever is a `Lever` variant with one plain
  sentence in `LEVERS`, and its hooks go in only through
  `token_savers::install`. With the plugin off, no hook is installed.
- **Where the on/off state comes from.** The app sets a process-wide flag
  when it loads its settings and when the person toggles the plugin.
  Headless runs (`coder -p`, the cloud runtime, issue runs) read the saved
  `bundled-plugins.json` instead, and a missing file means on.
- **Exceptions.** A per-lever environment variable (`CODER_SQUEEZE=off`)
  still works as an override. The bench's `briefed-agent` keeps explicit
  config switches, because an A/B arm must state its levers.

A new lever adds a `Lever`, its sentence, and its arm in `install`, and
nothing else. Per-lever toggles can come later, when there is more than one
lever.

## Status

**Idea 4 is started** (#11489, `crates/coder-new/src/gate.rs`, lever `Gate`): a `PreToolUse` hook after the floors that can only add a deny. First question: `Bash` runs a script in the cwd the session never read or wrote and that is not a README-listed `scripts/` entrypoint; then one Decisions predicate, deny at p >= 0.95 (1 - 1/20 from written costs). A failed call does nothing; decisions log to `~/.openagents/decisions/gate.jsonl` (`CODER_GATE_LOG`). More questions are still to add.

**Idea 1 is built** (#11486, `crates/squeeze`).

**What it does.** A `PreToolUse` hook on `Bash` rewrites only the noisy segments of a command (build, test and lint programs from a declared table), so `cd x && cargo test` keeps its `cd` in the agent's shell. It leaves alone:

- commands with pipes, redirections, subshells or command substitution;
- background jobs.

The hook answers with `updatedInput` only. It never gives a permission decision, so the floors and the session's own rules still decide whether the command runs. It works in default permission mode without auto-approving.

**The squeezer.**
- It runs the command in bash and keeps the exit status.
- Output of up to 150 lines passes unchanged.
- Longer output keeps 8 head lines, 40 tail lines, and up to 100 lines of middle chunks.
- Chunks flagged by the rule are shown as their flagged lines with two lines of context, and no API call is made.
- When too many or no chunks are flagged, one Decisions API call rates up to 64 chunks and keeps those at p ≥ 0.5.
- A failed call falls back to the rule.
- A failed squeeze shows the output unchanged.

**Where it's installed:**
- **Coder's Claude Code chat** (`coder-new`, full access) and the in-process issue-run session once it has Bash, through the Token savers plugin, as the `coder-squeeze` subcommand of `coder`, `openagents` and `coder-cloud-runtime`.
- **The briefed agent**, with `"squeeze": true` and `Bash` in `tools`. Full outputs go to the run's `squeeze/` directory, which `Read` may open, and decisions to `squeeze/decisions.jsonl`.

**Settings:**
- `CODER_SQUEEZE=off` turns it off.
- `CODER_SQUEEZE_DIR` sets where full outputs are saved.
- `CODER_SQUEEZE_LOG` sets the decision log (default `~/.openagents/decisions/squeeze.jsonl`).

**Measured on 2026-10-13** with a 400-test crate:

| Case | Lines in | Lines out | How | Notes |
| --- | --- | --- | --- | --- |
| 2 failures | 430 | 58 | Rule | No API call. |
| 40 failures | 735 | 123 | Decision | Clef (`clef-flash`) rated 54 chunks in 854 ms. |

In a live Claude Sonnet session, the briefed agent ran plain `cargo test` and saw the 58-line squeezed output. It named both failures with the right values, in 2 turns. The A/B lever is `squeeze` on the briefed-ab set once Bash is in the briefed agent's tools. Codex is not wired yet; its command hook needs `updatedInput` support checked first.

**Idea 2 is built for the bench's briefed agent** (#11487,
`crates/briefed-agent/src/stop_gate.rs`, `"stop_gate": true`).

**What happens when the agent stops without `finish` passing:**

1. If the agent already stopped once on this exact candidate after being
   sent back, the gate lets it stop.
2. Otherwise, one Decisions API choice reads its last message as done,
   blocked, question or other.
3. A blocker or a question at p ≥ 0.9 stops without running the checks.
   That threshold comes from written costs: an unverified "done" costs
   about ten times a check run.
4. Anything else runs `finish`'s final checks. A pass stops; a failure
   sends the agent back with the verdict as the reason.

Each decision is logged as a `hook-log` line in the run's
`stop_gate.jsonl`.

**Live result, 2026-10-13.** On a one-line bug, Sonnet fixed it and
stopped. The gate (Clef, 147 ms) sent it back with `no_tests`; the agent
added a test, the checks passed, and the run ended, in 5 turns.

**Next steps:** wire it into the product issue-run session as a Token
savers lever, and run the A/B arm with prompt wording about verification
removed.

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
