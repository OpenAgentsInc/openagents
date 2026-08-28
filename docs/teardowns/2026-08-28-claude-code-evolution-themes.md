# Claude Code conceptual evolution — snapshot 2.1.195 and after

- Date: 2026-08-28
- Subject: why Claude Code's architecture looks the way it does at the
  2.1.195 source snapshot, and what the public changelog plus Agent SDK
  changelog imply about the code that changed after that snapshot
- Companion to:
  [`2026-07-10-claude-code-teardown.md`](./2026-07-10-claude-code-teardown.md)
  (architecture at the snapshot) and
  [`2026-07-10-claude-subagents-rendering-analysis.md`](./2026-07-10-claude-subagents-rendering-analysis.md)
  (history-corpus chronology through 2.1.206)
- Status: analysis. Not a protocol guarantee, not an OpenAgents plan.

This document splits every theme in two:

1. **Observed up to 2.1.195** — claims that the source snapshot, the
   public changelog through 2.1.195, or the Agent SDK types of that era
   can support.
2. **Speculated after 2.1.195** — claims that the official
   `anthropics/claude-code` changelog (2.1.196 through 2.1.250) and the
   `@anthropic-ai/claude-agent-sdk` changelog (0.3.196 through 0.3.250)
   support as product behavior, plus a reasoned guess at which snapshot
   modules would have had to change. Those guesses are tagged
   `[speculative]`.

Evidence tags:

- **[source]** — visible in the 2.1.195 snapshot
- **[changelog]** — stated in the official Claude Code changelog
- **[sdk]** — stated in the Agent SDK changelog or public SDK types
- **[history]** — observed in the local `~/.claude` history corpus
  documented in the subagent analysis
- **[inferred]** — a conclusion drawn from several of the above
- **[speculative]** — a guess about post-snapshot source that we cannot
  see, grounded in changelog/SDK text plus the snapshot's module roles
- **[limitation]** — a bound on what this evidence can prove

The snapshot does not contain tests, a package manifest, or gated
implementations that `feature()` eliminates from the extract. A
changelog line that names a behavior is not a proof of the exact
implementation. [limitation]

---

## Method and identity

The snapshot is commit `813c06acfa2d705076df6193b405c81eb11a18d1`,
identified as **Claude Code 2.1.195 (approximately 2026-06-26)** by
changelog marker bracket. The 2026-03-31 import-commit date is workspace
metadata. See the architecture teardown §1 for the present/absent
marker table. [source] [changelog]

Post-snapshot evidence is the official changelog cloned at
`/tmp/claude-code-upstream` (head **2.1.250**) and the local
`claude-agent-sdk-typescript` repo at `0ec4158` (head **0.3.250**,
"parity with Claude Code v2.1.250"). SDK patch versions track Claude
Code patch versions in this range: 0.3.N is the typed host contract for
2.1.N. [changelog] [sdk]

Approximate volume:

| Window | Versions | Changelog bullets (approx.) |
| --- | ---: | ---: |
| 2.1.0 → 2.1.195 (in the snapshot's past) | 151 | 2,970 |
| 2.1.196 → 2.1.250 (after the snapshot) | 50 | 1,227 |
| SDK 0.3.196 → 0.3.250 | 55 | typed host-contract deltas |

Keyword clustering of those bullets is noisy (one line can hit several
themes) but the rank order is stable enough to use as a priority signal:
reliability, remote/IDE/web/mobile, MCP/skills/plugins/hooks,
permissions/sandbox, session/memory, then subagents. After the snapshot
the same rank holds, with reliability still first. [changelog]

---

## Conceptual eras before the snapshot

The snapshot is not a first design. It is a 2.1-era platform sitting on
four earlier product shapes. [changelog]

**0.2 (terminal agent).** MCP, custom slash commands, vim bindings,
approved-tools, auto-compaction, Keychain for macOS API keys. The
product is a local REPL that can use tools and remember a conversation.

**1.0 (permissions and IDE).** Bash permission matching becomes a
security surface (prefix-bypass fixes, redirection matching). VS Code
is already in the changelog. Hooks grow (`SessionEnd`,
`systemMessage`). Piped interactive input is deprecated. The product
discovers that a coding agent is an authority problem, not only a
prompt problem.

**2.0.0 (engine with several doors).** Native VS Code extension, rewind,
usage, sticky thinking, history search. The changelog line that names
the platform shift is: "The Claude Code SDK is now the Claude Agent
SDK." Subagents can be added with `--agents`. [changelog]

**2.0 mid-series (async and named sessions).** 2.0.64 is a hinge: agents
and bash can run asynchronously and wake the main agent; named
sessions; `.claude/rules/`; `AgentOutputTool` and `BashOutputTool`
unshipped in favor of a unified `TaskOutputTool`. 2.0.68 adds
enterprise managed settings. 2.0.72 adds Claude in Chrome (beta).
2.0.74 adds an LSP tool. [changelog]

**2.1.0 (skills, fork-context, teleport, unified backgrounding).**
Skills hot-reload; `context: fork` on skill frontmatter; agent
frontmatter hooks; MCP `list_changed`; `/teleport` and `/remote-env`;
Ctrl+B backgrounds bash and agents together. Most of 2.1.0's notes are
already reliability and permission-parser fixes. [changelog]

**2.1.49–2.1.195 (worktrees, auto-mode, workflows, agent manager).**
`--worktree` (2.1.49), `TaskCreated` hook (2.1.84), `PermissionDenied`
hook (2.1.89), `/ultrareview` (2.1.111), workflows (2.1.154), then a
dense month of background-agent, mailbox, and remote-control work
ending at 2.1.195. [changelog] [history]

The architecture teardown's one-sentence warning — capability growth
outrunning consolidation — is the 2.1-era condition, not a later
invention. By 2.1.195 the product is already an engine, an SDK, an IDE
extension, a remote-control target, a plugin platform, and a
background-agent supervisor. [inferred]

---

## Part A — Observed up to 2.1.195

### A1. The terminal is a client. The query loop is the product.

The snapshot has two conversation owners: the interactive REPL still
submits, cancels, and compact-handles queries directly, while
headless/SDK execution goes through a `QueryEngine` that owns messages,
usage, permission denials, and `ask()`. Subagents also re-enter the
inner `query()` generator, not `QueryEngine`. The `QueryEngine` module
comment names REPL consumption as a future phase; that phase is not
done here. [source]

Remote Control has to keep two `system/init` writers shape-identical:
the SDK stream from `QueryEngine`, and the bridge writer from the REPL,
which never hits that SDK layer. History-snip is a behavioral fork of
the same split: headless truncates messages to bound memory, the REPL
keeps full history for scrollback and projects a snipped view.
[source]

That split is the sediment of shipping order. The REPL existed first.
Print mode and the Agent SDK were attached later to the same `query()`
generator rather than extracting one conversation service and making
the REPL a consumer of it. `QueryEngine` is the extraction attempt;
interactive mode has not finished moving. [inferred]

The 2.0.0 SDK rename is the public statement of the same idea: hosts
should speak a control protocol, not scrape a terminal. [changelog]
The architecture teardown §7 documents that protocol. The dual owners
are why SDK and interactive behavior can still diverge on hooks,
permissions, and task notifications — a class of bugs the post-snapshot
changelog spends many lines closing. [inferred]

### A2. Compile-time flags are how one repo becomes many products.

Startup and the query loop are full of `feature('…')` gates consumed
from `bun:bundle`. Comments in the query loop say the point is dead-code
elimination: gated strings must be physically absent from external
builds. Internal-only surfaces (`USER_TYPE === 'ant'`, coordinator
mode, some isolation values) are compiled out of the public binary.
[source]

The architecture teardown counted 88 distinct compile-time feature
symbols. They mix product flavors (voice, workflows, background
sessions), experiments (fork-subagent, context-collapse, reactive
compact), and ANT-only tools. [source]

This is a shipping strategy: one tree, many binaries, experiments that
do not leak names. It is a poor semantic kernel. A code path that
type-checks here can be missing in a public build. Comments around the
Bash classifier also record a compiler cliff: if a function is too
complex, `feature()` can silently evaluate false and drop the
classifier from that binary. They restructured imports to stay under
the budget rather than abandon DCE. OpenAgents should not copy this
for authority, task terminality, or event durability. [source]
[inferred]

Runtime gates sit beside compile-time ones. GrowthBook / Statsig values
are snapshotted per query or read from a `_CACHED_MAY_BE_STALE` disk
cache so a mid-session flag flip does not crash schema parsing. [source]
That cache exists because they *do* flip flags under live users. The
Agent tool schema comments treat a one-session divergence window as
acceptable as long as Zod does not start rejecting arguments. [source]
[inferred]

### A3. Extensibility accreted as layers, not as one plugin model.

In changelog order the user-extension surfaces are: slash-command
markdown (0.2.31), MCP (0.2, with a project/local/user scope rename
in 0.2.49), hooks (1.0), plugins and marketplaces (2.0), skills and
agent frontmatter (2.1.0), then workflows (2.1.154). [changelog]

Each layer answers a different author: a repo wants a prompt snippet;
a vendor wants a tool server; an enterprise wants a policy hook; a
marketplace wants a bundle; a power user wants a reusable agent
definition; an orchestration user wants a scripted run. The snapshot
keeps all of them. [inferred]

The cost is several trust boundaries (MCP server approval, plugin
consent, hook decision vs permission rules vs sandbox) and several
discovery paths that must stay aligned on resume. 2.1.195 itself still
fixes plugin enable/disable when `plugin.json` `name` differs from the
marketplace entry, and hook matchers that substring-matched hyphenated
ids. [changelog]

They did not consolidate because each layer already had users and
settings files. Migrations in the snapshot move *config keys* (see A8),
not extension models. [inferred]

### A4. Background execution is the 2.1 pivot, and 2.1.195 is mid-rollout.

This is the theme the snapshot captures in the act of changing.

Visible sequence:

| When | What | Evidence |
| --- | --- | --- |
| 2.0.64 | Agents and bash run asynchronously and wake the parent | [changelog] |
| 2.1.0 | Ctrl+B backgrounds bash and agents together | [changelog] |
| 2.1.173 | Explicit `run_in_background` on Agent; `async_launched` results | [history] |
| 2.1.195 snapshot | `run_in_background` is optional, default unset; a 120s auto-background timer exists behind env `CLAUDE_AUTO_BACKGROUND_TASKS` or GrowthBook `tengu_auto_background_agents` | [source] |
| 2.1.195 snapshot | Fork-subagent experiment, when on, *strips* `run_in_background` from the tool schema and forces every spawn async, for a unified task-notification model | [source] |
| 2.1.195 histories | Omitted-field Agent results are mixed sync and async | [history] |
| 2.1.198 changelog | "Subagents now run in the background by default … (previously a gradual rollout)" | [changelog] |

The GrowthBook name and the "gradual rollout" sentence are the same
program. At the snapshot they are still A/B testing whether the model
and the user can live with "spawn returns a task id, you keep talking,
a notification arrives later." [inferred]

Why push it: a synchronous subagent blocks the parent turn, fills the
parent transcript, and makes "several agents at once" a lie. The Agent
prompt already tells the model not to sleep or poll when background is
used. The missing piece is making that the common case rather than an
opt-in parameter the model forgets to set. [source] [inferred]

Why not default it yet in 2.1.195: the history corpus and the changelog
around 2.1.193–2.1.195 are full of phantom resumed subagents, daemon
socket failures, and "end your response" instructions that stopped the
parent too early. They were still making background *correct* before
making it *default*. [changelog] [inferred]

Fork-subagent is a second, sharper bet on the same idea. When the
experiment is on: `subagent_type` becomes optional; omitting it inherits
the parent's conversation and the already-rendered system prompt bytes
(so the prompt-cache prefix stays byte-identical); permission mode is
`bubble`; recursive forks are rejected by a boilerplate tag. It is
disabled in coordinator mode (that mode already owns orchestration) and
in non-interactive sessions. [source]

Cache economics here are architecture, not a comment. Reconstructing
the child's system prompt by calling `getSystemPrompt()` again can
diverge across a GrowthBook cold→warm transition and bust the cache;
threading the rendered bytes does not. [source] [inferred]

### A5. Compaction, memory, and context-collapse overlap because each was a response to a different failure.

The query loop can run microcompact (by `tool_use_id`, cache-invisible),
autocompact, reactive compact (feature-gated), context-collapse
(feature-gated, a commit-log projection applied before autocompact),
history snip, and cached microcompact. Beside those sit memdir auto-memory,
`CLAUDE.md` / rules, and skill prefetch. [source]

They overlap because they solve different "the context window is full"
failures: tool results that must leave the prompt but stay addressable;
a conversation that must continue after a summary; a read-time
projection that should preserve granular file context; a cache that
must not break when history is rewritten. [inferred]

The architecture teardown already calls this provenance-hard. The
evolutionary reading is: they would rather add a new mechanism than
risk breaking prompt-cache prefixes or resume of old transcripts by
rewriting the one true compact. Feature gates let them try
context-collapse without shipping it to every binary. [inferred]

### A6. Authority is several mechanisms because each incident class got its own control.

Workspace trust, permission rules, permission modes, PreToolUse hooks
that can `ask` / allow / deny and rewrite input, an auto-mode
classifier, shell-command analysis, and an opt-in OS sandbox are all
present. [source]

Print mode skips the interactive trust dialog and warns. Sandbox is
opt-in. Approval is not isolation. Non-macOS credential storage in the
visible snapshot can fall back to a mode-restricted plaintext file.
[source]

2.1-era changelog through 2.1.195 keeps adding classifier coverage
(`autoMode.classifyAllShell` in 2.1.193), denial reasons in the
transcript, and sandbox credential blocking (`sandbox.credentials` in
2.1.187). Destructive git commands start getting auto-mode blocks
(2.1.183). [changelog]

The priority is: stop the model doing something irreversible *in the
common interactive case*, without turning the sandbox on for everyone
(compatibility with `gh`, language servers, local network). Fail-closed
isolation is deferred. [inferred]

Auto-mode itself is still an opt-in in this snapshot (`claude auto-mode
defaults|config|critique`, no `reset`). The classifier is a way to
offer "yes, and stop asking" without `bypassPermissions`. [source]
[changelog]

### A7. Local persistence is treated as a recovery substrate, then outgrows a log.

Append-only JSONL, sidechain transcripts for subagents, workflow
journals under `subagents/workflows/<runId>`, file checkpoints for
rewind, durable background-task output, and worktree registration are
all in the snapshot. [source]

This is why a UI can disappear and the session remains inspectable. It
is also why resume, fork, compact, and `/cd` become correctness-critical
on the storage format. 2.1.191 adds `/rewind` through `/clear`. 2.1.195
still fixes background-task reopen showing a blank screen. [changelog]

The implicit graph (parent pointers, sidechains, content replacement,
branch filtering) is locally dumpable and increasingly database-shaped.
They did not migrate to a real database in this snapshot. They added
prefilters and sidecars instead. [source] [inferred]

### A8. User-facing names are migrated; internal names leak, then get cleaned.

The `migrations/` directory is a short history of product language:

- model aliases: Fennec→Opus, legacy Opus→current, Opus→Opus 1M,
  Sonnet 1M→4.5→4.6
- `replBridgeEnabled` → `remoteControlAtStartup`, with a comment that
  the old key was an implementation detail that leaked into user config
- auto-update and bypass-permissions flags moved into settings
- auto-mode opt-in reset when the default offer changed
- Pro→Opus default reset

[source]

Remote Control is the productized REPL bridge. That is why the snapshot
still has a `bridge/` tree whose types talk about sessions, polling,
JWT, and work secrets, while commands and settings say remote control.
[source] [inferred]

Compatibility tax is accepted: migrations are idempotent, old keys are
read once, new keys win if already set. They would rather carry a
migration than break a setting that auto-started a bridge. [inferred]

### A9. Reliability already dominates 2.1, before the snapshot ends.

In 2.1.0–2.1.195, reliability/performance notes outnumber subagent notes
roughly two to one. 2.1.195 itself is mostly fixes: mouse-click disable,
hook matcher exactness, plugin consent, daemon socket, crashed-task
reopen, remote provisioning checklist. [changelog]

That ratio is the signature of a product in production at scale: new
capability in a minor, then a week of "background agent / MCP / resume
/ Windows / tmux" patches. They ship, then they pay. [inferred]

Performance work in the snapshot is equally structural: lazy
entrypoints, keychain prefetch, prompt-prefix stability, concurrent
read-only tools, disk spill, terminal virtualization, transcript byte
prefilter. Cache-hit rate is a product metric, not an implementation
detail. [source]

### A10. What 2.1.195 had not yet chosen.

Exact absences, not vibes:

- Background-by-default subagents (GrowthBook / optional param only)
- Fork-subagent as a default (compile-time experiment, off for
  non-interactive)
- Stream idle watchdog default-on (env `CLAUDE_ENABLE_STREAM_WATCHDOG`,
  90s idle timeout in the snapshot)
- Screen-reader mode
- Organization / role default models
- MCP "pending approval" for repo-self-approved servers in untrusted
  workspaces
- Readable default session names
- Sonnet 5 / Opus 4.7+ in the model table (tops out at Opus 4.6 /
  Sonnet 4.6)
- `EndConversation` tool, `/subtask`, `/dataviz`, `anthropicAws`
- `auto-mode reset`
- Tests in the extract
- Sandbox on by default
- A single conversation owner for REPL and SDK

[source] [changelog]

The deferred list is the post-snapshot agenda with surprising accuracy.
[inferred]

---

## Part B — After 2.1.195 (changelog 2.1.196–2.1.250, SDK 0.3.196–0.3.250)

This half is changelog- and SDK-grounded product history, plus a
reasoned map onto snapshot modules we can no longer see. Anything about
*code that would have changed* is `[speculative]`.

A useful frame: the fifty versions after 2.1.195 are not a new product.
They are the 2.1.195 engine being forced to tell the truth under load —
background as the common case, multiple clients, enterprise policy, and
hosts that were treating `completed` as a real terminal state.

### B1. Finish the async-default transition

**What the changelog says.** 2.1.196 improves background-session
survival across process stop/restart/update, including Windows shell
handoff, and auto-resumes workers killed by a daemon restart. 2.1.198
makes subagents background by default ("previously a gradual rollout"),
fires `Notification` hooks `agent_needs_input` / `agent_completed`, and
has background agents from `claude agents` commit/push/open a draft PR
when they finish worktree code. 2.1.212 turns `/fork` into a *session*
copy (own row in `claude agents`); the in-session subagent it used to
launch becomes `/subtask`. 2.1.232 turns subagent forking on by default:
`subagent_type: "fork"` inherits conversation and prompt cache, and
non-teammate spawns in interactive sessions run in the background.
Nesting limits then oscillate: 2.1.217 drops spawn depth 5→1 and adds a
concurrency cap (default 20) plus a 200-spawn-per-session cap; 2.1.219
raises depth 1→3; 2.1.224 removes the per-session spawn cap.
2.1.246 marks a child that hits `maxTurns` as partial and tells the
parent to continue it, instead of treating the cap as a clean finish.
[changelog]

**Why.** The snapshot already had the machinery and the experiment. The
remaining work is operational: daemons that do not lose workers, a
notification so the parent model is not told to poll, a split between
"fork my session so I can keep typing" and "fork my context into a
child." Until those are true, defaulting background just multiplies
lost work. [inferred]

**What would have changed in the snapshot.** `[speculative]`

- Agent tool schema: `run_in_background` default inverts, or the field
  is omitted the way fork-subagent already omitted it when forcing
  async. The 120s GrowthBook auto-background path becomes redundant for
  the common spawn.
- Agent prompt text: "optionally run in the background" becomes "runs
  in the background unless you set false."
- `isForkSubagentEnabled()`: the `feature('FORK_SUBAGENT')` compile-time
  false-path goes away or inverts for interactive sessions; coordinator
  and non-interactive exclusions likely remain.
- `/fork` command: stops launching an in-session subagent; starts a
  background session copy. New `/subtask` command takes the old
  in-session job.
- SDK: `Notification` payloads gain `agent_needs_input` /
  `agent_completed`. 0.3.203 adds `background_tasks_changed` as a
  *level* (full set of live tasks on every membership change) so hosts
  stop pairing start/stop edges. 0.3.238 adds `is_backgrounded` and
  `spawn_depth` on `task_started`. [sdk]

### B2. Lifecycle honesty: "completed" was lying

**What the SDK changelog says.** 0.3.204 adds `terminal_reason` values
`tool_deferred_unavailable`, `turn_setup_failed`, `api_error`,
`malformed_tool_use_exhausted`, `budget_exhausted`,
`structured_output_retry_exhausted`. Turns that died on exhausted API
retry or malformed tool use previously reported `completed`; budget and
structured-output exhaustion previously omitted `terminal_reason`.
Commands consumed by those turns now report `command_lifecycle` state
`cancelled` instead of `completed` ("dup-over-loss"). 0.3.205 adds
interrupt receipts (`still_queued`, `interrupt_receipt_v1`). 0.3.229
classifies 32 MB request-body overflow as `api_error` rather than
`image_error`. 0.3.234 removes unused `bypass_permissions_disabled`
from `ExitReason` because it was never emitted. 0.3.238 adds
`command_lifecycle` state `refused` for a cross-session message the
receive-side policy declines. 0.3.243 adds `queued_turn_count` on
result messages. 0.3.246 adds `perTaskStopAffordance` so `interrupt()`
can abort only the current turn. [sdk]

**Why.** SDK and remote hosts were driving automation off the result
frame. If a dead turn says `completed`, a fleet scheduler marks work
done and a command-lifecycle sweeper thinks the slash command
succeeded. The snapshot's `Terminal` union on `query()` already had
richer internal exits (`blocking_limit`, `aborted_streaming`,
`prompt_too_long`, …) than the SDK result advertised. The post-snapshot
work is pushing those internals onto the wire. [source] [inferred]

**What would have changed.** `[speculative]` Result mapping in
`QueryEngine` / print-mode JSON; `command_lifecycle` emission next to
slash-command handling; control-request replies for interrupt. The
Rust `claude_agent_sdk` crate's later `terminal_reason` taxonomy is
tracking this honesty pass, not inventing it.

### B3. Watchdogs, reconnect, isolation: reliability of a multi-client engine

**What the changelog says.** 2.1.196 turns the streaming idle watchdog
on by default for all providers (abort and retry after five minutes of
silence; `CLAUDE_ENABLE_STREAM_WATCHDOG=0` disables). In the snapshot
the same watchdog is opt-in via that env and the idle timeout is 90
seconds. [changelog] [source]

The rest of 2.1.196–2.1.250 is dominated by: Remote Control reconnect
dropping permission prompts; cloud containers restarting mid-background
work; MCP reconnect in SDK/`-p`; worktree sessions losing their
checkout because cleanup removed them; background agents running
destructive git against the *main* checkout (2.1.210, 2.1.222);
PreToolUse auto-allow hooks bypassing tool restrictions in background
summaries/compaction (2.1.222); Windows PowerShell permission-check
bypass (2.1.214); `dir/**` allow rules auto-approving nested writes
anywhere in the tree (2.1.214). [changelog]

**Why.** Once background, remote, and SDK are first-class, a hung
stream or a worktree that is not actually isolated is data loss or a
security hole, not a UI glitch. The watchdog comment in the snapshot
already names the failure: the HTTP client's request timeout covers
`fetch()`, not a silently dropped streaming body. Default-on is the
admission that opt-in was not protecting the users who hang. [source]
[inferred]

Worktree isolation bugs are a sharper admission: `isolation: worktree`
was a product promise whose git-command allow path still saw the
parent checkout. That is the kind of bug you only find after
background agents run unsupervised. [inferred]

**What would have changed.** `[speculative]`

- Stream watchdog: invert `isEnvTruthy(CLAUDE_ENABLE_STREAM_WATCHDOG)`
  to default-true; likely raise idle timeout 90s → 5 minutes.
- Worktree/git permission analysis: thread the child's worktree cwd
  into the destructive-git detector; stop consulting the parent repo.
- Background-task hook path: apply the same tool-restriction floor as
  the parent (2.1.222).
- Bridge/session runner: resume after container restart without
  replaying a hook notification as the user prompt (2.1.243).

### B4. Rename "default" to "Manual"; make auto-mode a real product

**What the changelog says.** 2.1.200 changes the permission mode named
"default" to "Manual" across CLI, `--help`, VS Code, and JetBrains;
`manual` is accepted alongside `default`. SDK 0.3.200 accepts `'manual'`
as an alias for `'default'`. 2.1.207 makes auto-mode available without
the `CLAUDE_CODE_ENABLE_AUTO_MODE` opt-in on Bedrock, Vertex, and
Foundry (`disableAutoMode` to turn it off). 2.1.212 adds
`claude auto-mode reset`. 2.1.247 adds a one-keystroke "Yes, and switch
to auto mode" on Bash permission prompts. [changelog] [sdk]

**Why.** Calling the prompting mode "default" hid that it is the
*strict* mode. Auto-mode could not become the recommended path while
the word "default" meant "ask every time." The rename is copy as
architecture. Enterprise providers getting auto-mode without an env
opt-in is the same bet the snapshot was still gating: a classifier
instead of `bypassPermissions` for unattended runs. [inferred]

**What would have changed.** `[speculative]` Permission-mode enums and
help strings; settings schema; SDK `permissionMode` on `system/init`
(0.3.247 later fixes that field reporting turn-start mode instead of
live mode — hosts were showing a stale mode after a switch). Auto-mode
subcommand family gains `reset`. Classifier prompt/tooling stays, with
more denial reasons already started in 2.1.193.

### B5. Models as forcing functions

**What the changelog says.** 2.1.197 introduces Claude Sonnet 5 as the
default (1M context). 2.1.201: Sonnet 5 sessions no longer use the
mid-conversation system role for harness reminders. 2.1.219 adds Opus 5
as the default Opus, 1M context, fast mode pricing. SDK 0.3.233 removes
todo/task-tracking tools from the default tool surface on Opus 4.8,
Sonnet 5, Fable 5, Mythos 5, and newer — name them in `tools` /
`allowedTools` to keep them. [changelog] [sdk]

The snapshot model table ends at Opus 4.6 / Sonnet 4.6. Explore in the
snapshot inherits the parent model only on internal builds; external
users get haiku. [source]

**Why.** A 1M default model changes compaction urgency, Explore-agent
model choice (2.1.198: Explore inherits the main model, capped at
opus, instead of haiku), and which tools are worth putting in the
always-on schema (todo tools become noise for models that already plan).
Harness reminders moving off the mid-conversation system role is a
cache-prefix and "what did the model actually see" issue. [inferred]

**What would have changed.** `[speculative]` `utils/model/configs.ts`
and picker; default-model resolution (plus 2.1.196 org/role default
models — an admin console setting the snapshot's picker did not have);
Explore agent definition `model: 'inherit'`; tool-registry default set
per model family.

### B6. The SDK becomes the contract that cannot drift

Almost every interesting 0.3.196–0.3.250 line is a host-visible event,
field, or honesty fix, not a CLI widget. Parity stubs ("Updated to
parity with Claude Code v2.1.N") fill the versions that had no new
host contract. [sdk]

Notable host-contract additions after the snapshot, besides B1–B2:

- 0.3.196: `prompt_id` on hook payloads; control-protocol dedup was
  dropping tool-use ids after 1000 resolutions
- 0.3.199: `requestId` on `canUseTool`; `blocked` on workflow_agent
  progress (auto-mode classifier); sandbox credential `mode:"mask"`
- 0.3.202: `parent_agent_id` on subagent session messages (depth-2+
  trees from disk)
- 0.3.211 era flags / 0.3.224: `crossSessionInbound`, `dialogExpiry`,
  archive plugin source, sandbox credential-masking fields
- 0.3.232: `/context` result carries structured `context_usage`
- 0.3.246: `user_message_uuid` linking replies to the triggering user
  message; `modelUsage.costBasis`; `modelPricing` in managed settings

[sdk]

**Why.** Desktop, web, mobile, VS Code, JetBrains, and third-party
hosts all consume this stream. A CLI-only field is invisible to them; a
missing terminal reason mis-drives them. The 1:1 version lock with
Claude Code is how they keep those hosts from depending on
undocumented CLI behavior. [inferred]

**What would have changed.** `[speculative]`
`entrypoints/sdk/coreTypes.ts`, `coreSchemas.ts`, `controlSchemas.ts`,
and the print-mode JSON emitter in lockstep. Hook input structs. Task
event shapes. The Rust port's residual "post-0.3.247 types" list is
exactly this surface moving.

### B7. Accessibility, naming, and "the TUI is a real app"

**What the changelog says.** 2.1.196: readable default session names.
2.1.208: screen-reader mode (`claude --ax-screen-reader` /
`CLAUDE_AX_SCREEN_READER`). 2.1.217: emoji shortcode autocomplete.
2.1.200 already improves screen-reader output (hide decorative glyphs,
read nested tables as `Header: value.`). [changelog]

The snapshot's 2.1.195 mouse-click disable (`CLAUDE_CODE_DISABLE_MOUSE_CLICKS`)
is the last pre-a11y concession: fullscreen mouse tracking was stealing
clicks from the terminal's own selection, and the fix is an env kill
switch rather than a mode. [changelog] [source]

**Why.** A custom Ink renderer that owns the alternate screen *is* the
accessibility problem. Screen-reader mode is a second presentation
projection of the same engine state — the architecture the teardown
already recommended for OpenAgents, forced by users who cannot use the
rich TUI. [inferred]

**What would have changed.** `[speculative]` A parallel render path that
skips Yoga/Ink rich output; session-name generator at session start
(the snapshot already had `/rename`); prompt-input autocomplete table.

### B8. Enterprise, self-host, restricted: the engine as infrastructure

**What the changelog says.** 2.1.196 org default models and MCP pending
approval for untrusted workspaces. 2.1.208 `CLAUDE_CODE_PROCESS_WRAPPER`
for corporate launchers. 2.1.219 `sandbox.network.strictAllowlist`.
2.1.221 sandbox credential `mode: "mask"`. 2.1.224 self-hosted runners
(`claude self-hosted-runner` turns your machines into a place web and
mobile can run) and `archive` plugin source (HTTPS zip, optional
SHA-256). 2.1.248 `--restricted` / `CLAUDE_CODE_RESTRICTED=1` removes
shell/code/`WebFetch`, keeps file tools inside the working directory,
refuses `bypassPermissions`, ignores user/project/local settings.
Cross-session `SendMessage` / `ListAgents` expands to Bedrock, Vertex,
Foundry. [changelog]

**Why.** Team/Enterprise users need a mode that is not "trust this
binary with bash." Restricted mode is the hermetic/bare profile the
snapshot already had, turned into an explicit reduction of *capability*
rather than only of ambient context. Self-hosted runners are Remote
Control without Anthropic-hosted workers. Masked sandbox credentials
are the 2.1.187 `sandbox.credentials` setting growing up into
something that can run `gh` without leaking the token to the model or
the command line. [source] [inferred]

**What would have changed.** `[speculative]` New CLI flag plumbing next
to bare/print profiles; tool registry filtering; managed-settings
schema; a runner entrypoint beside the existing bridge; credential
inject/mask in the sandbox wrapper.

### B9. Prompt cache is now a user-visible invariant

**What the changelog says.** 2.1.248: a prompt-cache miss (and lost
extended-thinking context) roughly once an hour, caused by tool
definitions re-rendering after OAuth token refresh. `ScheduleWakeup`
tool definition changing between a session and its `--resume` on usage
overage, full cache miss on the first resumed turn. 2.1.201: Sonnet 5
drops mid-conversation system-role harness reminders. The snapshot
already threaded rendered system-prompt bytes into forks for this
reason. [changelog] [source]

**Why.** At 1M context and with thinking blocks, a cache miss is minutes
and dollars, and it drops thinking. They now treat byte-identical
prefixes as a product promise across OAuth refresh, resume, and fork.
[inferred]

**What would have changed.** `[speculative]` Tool-definition renderer
must not include fields that change on token refresh or overage;
wakeup/schedule tool schema frozen across resume; more "thread the
rendered bytes" along the lines of fork-subagent.

### B10. What 2.1.250 still does not claim

Changelog silence is weak evidence, but through 2.1.250 there is still
no public claim of:

- sandbox on by default
- a single conversation owner replacing the REPL/QueryEngine split
- a database-backed transcript (JSONL remains implied)
- tests as a published part of the product
- a signature chain for plugins that would replace marketplace trust
  (archive SHA-256 is pinning, not publisher authorization)

[limitation] [inferred]

The post-snapshot work hardens the 2.1.195 shape. It does not replace
it.

---

## Defaults that flipped (the conceptual scoreboard)

| Concern | At 2.1.195 snapshot | By 2.1.250 | Pivot version |
| --- | --- | --- | --- |
| Subagent scheduling | Explicit `run_in_background`; GrowthBook auto-background | Background by default | 2.1.198 |
| Fork-subagent | Compile-time experiment; off when non-interactive | On by default in interactive sessions | 2.1.232 |
| `/fork` | In-session subagent | Background session copy; old job is `/subtask` | 2.1.212 |
| Stream watchdog | Opt-in env, 90s idle | Default on, 5 minute idle | 2.1.196 |
| Permission mode name | "default" means prompt | "Manual"; `manual` alias | 2.1.200 |
| Auto-mode on Bedrock/Vertex/Foundry | Env opt-in | Available without opt-in | 2.1.207 |
| Explore agent model | Haiku for external users (inherit is internal-only) | Inherit parent, cap opus | 2.1.198 |
| Subagent nest depth | Implied cap 5 | 5→1 (2.1.217) then 1→3 (2.1.219) | 2.1.217–2.1.219 |
| Default frontier model | Sonnet 4.6 / Opus 4.6 | Sonnet 5, then Opus 5 | 2.1.197, 2.1.219 |
| MCP in untrusted repos | Self-approval via committed settings | Pending approval | 2.1.196 |
| Session identity | Rename if you care | Readable default names | 2.1.196 |
| Chrome integration | Beta in 2.0.72 | Generally available | 2.1.198 |

[changelog] [source]

The pattern: ship behind a gate, survive a month of daemon/resume/MCP
bugs, then invert the default and rename the old default so users can
still opt out.

---

## Exact-code speculation map (snapshot module → likely post-2.1.195 change)

These are reasoned guesses for a reader who has the 2.1.195 tree and
the later changelogs, not a diff.

| Snapshot role | Likely change after 2.1.195 | Grounding |
| --- | --- | --- |
| Agent tool input schema / prompt | Invert background default; keep explicit `false`; maybe omit the field when fork is on | 2.1.198 [changelog]; omit-pattern already in snapshot [source] |
| Fork-subagent gate | Default true in interactive, still off for coordinator and probably print/SDK unless separately enabled | 2.1.232 [changelog] |
| `/fork` command vs new `/subtask` | Session copy vs in-session child split | 2.1.212 [changelog] |
| Stream watchdog in API client | Default enabled; idle 90s → 5 min; env becomes disable-switch | 2.1.196 [changelog] vs snapshot [source] |
| Model config table | Sonnet 5, Opus 4.7/4.8/5, Fable/Mythos; org default row | 2.1.197–2.1.219 [changelog] |
| Explore agent definition | `model: inherit` with opus cap | 2.1.198 [changelog] |
| Permission mode enum / help | `manual` name; `default` compatibility alias | 2.1.200 [changelog] [sdk] |
| Auto-mode CLI | `reset` subcommand; drop env opt-in on enterprise providers | 2.1.207, 2.1.212 [changelog] |
| QueryEngine result mapping | `terminal_reason` taxonomy; command_lifecycle cancelled/refused | 0.3.204–0.3.238 [sdk] |
| SDK core/control schemas | `background_tasks_changed`, `parent_agent_id`, `is_backgrounded`, `spawn_depth`, `queued_turn_count`, `user_message_uuid`, `perTaskStopAffordance`, structured `context_usage` | 0.3.202–0.3.246 [sdk] |
| Hook types | `Notification` agent payloads; `DirectoryAdded`; `prompt_id`; classifierContext from PostToolUse | 2.1.198, 2.1.219 [changelog]; 0.3.196, 0.3.236 [sdk] |
| Task / workflow tools | TaskGet/TaskOutput already present; `/subtask`; later todo tools leave default surface on new models | snapshot [source]; 2.1.212 [changelog]; 0.3.233 [sdk] |
| Tool registry | `EndConversation`; `/dataviz` skill; `SendFeedback` | 2.1.214, 2.1.198, 2.1.247 [changelog] |
| Sandbox settings schema | `strictAllowlist`, `filesystem.disabled`, credential `mode: mask`, JWT/SigV4 inject | 2.1.216–2.1.224 [changelog] [sdk] |
| MCP listing / trust | Pending-approval state; no spawn of committed `.mcp.json` in untrusted workspaces | 2.1.196 [changelog] |
| Session naming | Generator at start, not only `/rename` | 2.1.196 [changelog] |
| TUI renderer | Screen-reader projection; emoji autocomplete; mouse-report framing fixes | 2.1.208, 2.1.217, 2.1.247 [changelog] |
| Git/worktree isolation | Destructive git and mutating commands bound to the child's worktree, not the main checkout | 2.1.210, 2.1.222 [changelog] |
| Bridge / remote / runner | Self-hosted runner entry; reconnect that preserves permission prompts; `terminal_slash_commands` on init | 2.1.224, 2.1.248 [changelog]; 0.3.229 [sdk] |
| Cross-session messaging | Settings `crossSessionInbound`; `fromMode` on peer origin; Bedrock/Vertex/Foundry enable | 0.3.224, 0.3.234 [sdk]; 2.1.248 [changelog] |
| Restricted / bare profile | `--restricted` as capability cut, not only ambient-context cut | 2.1.248 [changelog] |
| Plugin loader | `source: archive` with sha256 | 2.1.224 [changelog] [sdk] |

---

## Lessons (for reading Claude Code, and for OpenAgents)

1. **Defaults are the real release.** The snapshot is full of the
   background-agent, fork, watchdog, and auto-mode *mechanisms*. The
   next fifty versions mostly invert who has to opt in. Designing the
   gated form so a flag flip cannot crash the schema (the Agent tool
   omit comments) is what made those inversions possible. [source]
   [changelog] [inferred]

2. **The SDK stream is where they pay down lies.** Internal `Terminal`
   reasons existed at 2.1.195. Host-visible `terminal_reason` and
   `command_lifecycle` come after hosts automated on the wire. If
   OpenAgents has an internal enum richer than its protocol, that gap
   will become someone else's incident. [sdk] [inferred]

3. **Isolation that is not threaded into every git/permission path is
   a setting, not a boundary.** The post-snapshot worktree bugs are the
   proof. A child's cwd that the destructive-git detector does not see
   is ambient host authority. [changelog] [inferred]

4. **Cache-prefix stability is a product feature.** Fork-subagent
   threading rendered prompt bytes, then OAuth refresh and resume
   keeping tool definitions still, are one program: do not silently
   drop a 1M thinking session on the floor. [source] [changelog]

5. **Layered extensibility is sticky.** Skills did not replace slash
   commands; workflows did not replace subagents; plugins did not
   replace MCP. Plan for coexistence or pay a migration the snapshot
   shows they were unwilling to force. [changelog] [inferred]

6. **Compile-time product flavors leak into semantics.** Fork disabled
   in print mode, coordinator mutually exclusive with fork, ANT-only
   isolation values: the same source is not the same program. OpenAgents
   should keep adapters at the edge and one state machine in the core.
   That recommendation in the architecture teardown is stronger after
   watching fifty more versions still patching SDK vs REPL vs daemon
   divergences. [source] [changelog] [inferred]

7. **Hermetic and restricted are different cuts.** Bare mode in the
   snapshot suppresses ambient context (hooks, plugins, CLAUDE.md).
   `--restricted` in 2.1.248 suppresses *tools that run code* and
   ignores settings files. CI, eval, and untrusted-repo modes need
   both cuts named. [source] [changelog]

8. **Do not copy the dual query owner.** Everything in Part B that is
   "fixed headless/SDK so it matches interactive" is the tax on A1.

---

## Source basis

- Claude Code source snapshot `813c06acfa2d705076df6193b405c81eb11a18d1`
  (2.1.195, approximately 2026-06-26) [source]
- Official `anthropics/claude-code` `CHANGELOG.md` through 2.1.250
  [changelog]
- `@anthropic-ai/claude-agent-sdk` `CHANGELOG.md` through 0.3.250 [sdk]
- [`2026-07-10-claude-code-teardown.md`](./2026-07-10-claude-code-teardown.md)
- [`2026-07-10-claude-subagents-rendering-analysis.md`](./2026-07-10-claude-subagents-rendering-analysis.md)
  especially §7 chronology and §13.1 provenance
- [`cc/README.md`](./cc/README.md) comparison series (implementation
  detail against OpenAgents Coder; not required reading for this
  evolution argument)

No credentials, private transcripts, or live Claude control were used.
No source was copied into this document.
