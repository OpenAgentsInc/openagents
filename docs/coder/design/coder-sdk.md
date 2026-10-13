# Coder SDK: one host layer in front of every harness

2026-10-13. Specification, not built. What the Coder SDK is, what it takes
from the Claude Agent SDK, how hooks work on any harness, and what else
plugs in: the Decisions API, plugins, policy, tools, and the records that
let us calibrate. The issue set is at the end.

Related: [system-one-hooks.md](system-one-hooks.md) (the hook ideas this
SDK carries to every harness), the
[agentic execution router](../../api/2026-10-02-agentic-execution-router.md)
(the authority, admission and payment rules the SDK must keep inside a
session), #11451 (harness plugins and routing),
`crates/coder-new/src/harness_plugin.rs` and `route_contract::harness`
(the harness declarations), `crates/coder-new/src/floors.rs` (one policy
on every harness, #11475).

## What it is, and what it is not

The **Coder SDK** is the layer between Coder and the harness that runs a
turn. The harness declarations say *which* engines can run (where, who
pays, what they can do, health). The SDK says *how* a session runs under
host control: start and continue it, watch it, gate its tools, add what
the host knows, stop it, and record every decision.

It is not:

- **A model API.** Calling a model is the gateway's, OpenRouter's and
  `codex-transport`'s job. The SDK drives agents, not completions.
- **The Claude Agent SDK, generalized.** `crates/claude_agent_sdk` is the
  wire protocol of one CLI, pinned to its releases (`PARITY.md`). Making
  other engines pretend to be that CLI would chase a protocol we don't own
  and hide what they can't do. The Claude SDK becomes one *backend*.
- **Public yet.** It is a crate in this monorepo. Extraction into the
  reserved `openagents-ai` repo waits for that repo's written criteria.

## Why now

Coder already does SDK-shaped work, once per harness:

| Concern | Today | Where |
|---|---|---|
| Session lifecycle | `Session` trait: start, observe, stop, resume, steer, with a capability matrix | `crates/coder-delegate/src/session.rs` |
| Policy floors | One table, three wirings: Coder loop tool gate, Claude `PreToolUse` hook, Codex `-c hooks.PreToolUse` command | `crates/coder-new/src/floors.rs` |
| Output squeezing | A Claude `PreToolUse` hook that rewrites noisy commands | `crates/squeeze`, `claude.rs` |
| Worktree gate | Claude `can_use_tool` callback, Claude-only | `crates/microcoder/src/repository/claude_sdk.rs` |
| Briefed agent | Claude SDK session with our system prompt, short tool list, `verify`/`finish` tools | `crates/briefed-agent` |
| Coder tools | An in-process MCP server handed to Claude | `crates/coder-new/src/claude_tools.rs` |
| ACP agents | Permission requests and updates, handled per agent | `crates/acp-client` |
| Routing record | Inputs, probabilities and outcome per harness choice | `crates/coder-new/src/routing_record.rs` |

Each new System One hook (#11486–#11492) is written against
`claude_agent_sdk::HookEvent` first, then ported harness by harness, or not
at all. The SDK makes a hook a thing you write once.

## What we take from the Claude Agent SDK

These are the practices worth keeping, whatever the harness:

1. **Hooks are in-process callbacks with matchers.** A hook names an event,
   an optional tool matcher, and a timeout. The host registers it at
   session start; no shell round trip unless the hook is a command hook.
2. **A hook answers with a typed outcome**, not free text: allow, deny with
   a reason, ask, rewrite the tool input (`updatedInput`, verified live on
   2026-10-12), add context, block a stop with a reason, or end the session
   (`continue: false`).
3. **What a hook adds lands at the tail.** The cached prefix (system prompt,
   tools, briefing) stays cached. This is the reason hooks are the
   cache-safe home for System One.
4. **The host can replace the system prompt** and choose the tool list, so
   the box is ours.
5. **Permissions are a callback, not a mode.** `can_use_tool` lets the host
   answer each request, with the request's exact input.
6. **Tools arrive as MCP**, including in-process servers.
7. **Elicitation and user dialogs are callbacks**, so the host's own UI
   (studio approvals, the phone) answers them.
8. **Process discipline.** The agent leads its own process group; stopping
   the session stops the group.

## Hooks on any harness

### Events

One normalized event set. Names follow Claude Code's where they exist,
so a hook written for one reads naturally on the other.

| Event | Fires | Can |
|---|---|---|
| `SessionStart` | A session starts or resumes | add context |
| `UserPromptSubmit` | A message enters the session | add context, block, route |
| `PreToolUse` | Before a tool runs | allow, deny, ask, rewrite input, add context |
| `PostToolUse` | After a tool returns | add context, end session |
| `PostToolBatch` | After a batch of parallel tools | add context, end session |
| `Stop` | The agent says it is done | block with a reason, allow |
| `PreCompact` / `PostCompact` | Around a compaction | add context after |
| `SubagentStart` / `SubagentStop` | Around a subagent | add context, block stop |
| `FileChanged` | A file in the workspace changed | add context |
| `SessionEnd` | The session is over | observe only |

### Support tiers

Not every harness can do every event. Each backend declares, per event,
one of:

- **Native**: the harness calls the hook before it acts and obeys the
  outcome (Claude Code for all of the above; the Coder loop, because we
  own it).
- **Command**: the harness runs a shell hook and obeys it (Codex's
  `PreToolUse` command hooks, as `floors::codex_args` uses today). The SDK
  ships one hook binary that bridges a command hook to the in-process
  registry.
- **Emulated**: the host sees the event in the stream after the fact. It
  can add context on the next turn, cancel, or steer, but it cannot deny or
  rewrite what already ran (ACP agents, mostly).
- **None.**

The tier is part of the capability matrix the router reads. A hook that
needs to deny (a floor) declares that, and a harness that can only emulate
it is refused at start for that work, as the harness contract already
refuses a permission it can't honor (#11452). Nothing claims a guarantee
the harness can't give.

### Outcome and composition

```text
HookOutcome {
  permission: Allow | Deny { reason } | Ask { step } | NoOpinion,
  rewrite:    Option<ToolInput>,      // PreToolUse only
  context:    Option<Evidence>,       // lands at the tail
  stop:       Continue | Block { reason } | End,
}
```

Several hooks on one event combine by fixed rules:

1. **Floors run first and cannot be overridden.** Deny beats ask beats
   allow beats no opinion. No hook allows what a floor denies.
2. **Rewrites chain** in registration order, and the floors judge the
   *final* input (squeeze rewrites, the floors still decide).
3. **Context has a budget** per event and per session, because what is
   added is re-read on every later call. Over budget, the lowest-priority
   context drops and the drop is recorded.
4. **A failed hook never blocks work** unless it is a floor: a timeout or an
   error is `NoOpinion`. A floor that fails falls back to its rule's
   answer, which may be `ask`.

### Who can register a hook

| Source | Example | Authority |
|---|---|---|
| Host built-ins | floors, squeeze, worktree gate | full |
| Decision hooks | the System One ideas | context, steer, block stop, deny/ask; never allow past a rule |
| Plugin hooks | a Wasm guest from an installed plugin | tighten only: deny, ask, context; rewrite only when the person grants it |
| Command hooks | a project's or person's shell hook, Claude/Codex compatible | as the person configures, never past a floor |

## Decision hooks: System One as a hook kind

Most System One hooks are the same shape: a rule first, a Decisions API
question when the rule is unsure, a threshold from costs, an action, and a
log line. The SDK makes that one declared kind instead of seven bespoke
ones:

```text
DecisionHook {
  event, matcher,
  rule:      fn(event) -> Decided(outcome) | Unsure,
  questions: the Decisions API question set (noul / choice / score),
  state:     what of the event and session the questions read,
  act:       fn(answers) -> HookOutcome,   // thresholds live here, in code
  costs:     C(wrong act), C(escalate),    // written next to the threshold
  timing:    blocking | async,             // monitoring runs async
  model:     pinned,
}
```

The SDK owns the parts every one of them gets wrong alone:

- **Clef first, Jev backup**, through `jev::decisions::DecisionsClient`.
- **Latency.** A deadline per call (a decision takes 0.2–0.6 s), matchers
  so most tool calls never ask, and async for anything that only monitors.
- **Fallback.** A failed call is the rule's answer or `NoOpinion`.
- **The record.** Raw probabilities, threshold, decision, model, door and
  latency, keyed so the outcome (run passed, steer followed) joins later
  (#11492). Without the join no threshold can be calibrated.

#11487–#11491 then become decision hooks declared against the SDK, and run
on every harness at whatever tier that harness supports.

## Plugin hooks

The plugin system already runs Wasm guests with no ambient interface: a
packet in, a value or a refusal out, a scoped snapshot, limits, and an
invocation receipt that can be replayed (`crates/plugin`,
`crates/plugin-pdk`). That is exactly what a third-party hook should be.

- A plugin gains a **hooks** part next to skills, workflows, knowledge,
  Wasm and tests: each entry names an event, a matcher, the guest
  operation, and the authority it asks for.
- The hook's packet is the normalized event; its reply is a `HookOutcome`.
  A refusal is `NoOpinion`.
- A guest may ask the host for a decision through `oa_host.call`, so a
  plugin can ship its own decision hook without holding a key.
- Every firing produces the plugin's invocation receipt, so a hook's effect
  on a run can be replayed and checked, and a plugin's tests can show the
  hook helps (the plugin's own test set, run with and without it).
- Plugin hooks tighten by default. A plugin that wants to rewrite input
  shows that in its listing and the person turns it on.
- A plugin's hooks are off until the person turns the plugin on, the same
  rule the router keeps for a plugin's standing rules (router doc 13.9).

## Rules from the agentic execution router

The router decides what runs, where, for whom and at what price, before a
session starts. The SDK is where those decisions are kept *inside* the
session, step by step. Its rules carry over:

1. **Four authorities stay separate** (router section 4): observation,
   execution, disclosure and spending. A hook answer can narrow any of
   them and widen none. Installing a plugin, a hook's probability, or
   text in a tool result never grants one.
2. **A hook is a disclosure.** A decision hook sends part of the session
   (a command, an output chunk) to a decision model; a plugin hook sends
   the event to a guest. Each hook declares what it sends and to whom, and
   the SDK checks that against the session's admitted disclosure before
   registering it. A hook the admission doesn't cover is not registered,
   and that is recorded.
3. **The hook set is part of the admission.** The registered hooks, their
   question-set and policy versions, and their pinned models are digested
   into the route's adapter digest, beside the delegate recipe
   (`route_contract::recipe`). A continuation keeps that set; adding a hook
   that discloses more or spends is a new admission, not a quiet change.
4. **The recipe is the box.** The delegate recipe's per-engine row (briefing,
   knowledge, effort, lean tools, system prompt, cache, frozen checks) is
   what the SDK applies at session start, so one table describes every
   harness's box.
5. **Untrusted content stays untrusted** (router section 9). Tool output,
   plugin output and model text a hook reads can't change the admitted
   recipients or grants. Context a hook adds is labeled with its source.
6. **Fallback and escalation are new admissions** (router section 5). A
   hook that ends a session to escalate hands the router a checkpoint; the
   stronger harness runs only if it fits the same grant, disclosure and
   payer, or a new offer is confirmed. A fallback never switches the payer
   to OpenAgents when the person chose their own keys (13.6).
7. **Engines don't start engines** (13.2). `SubagentStart` lets the host
   see and gate the harness's own subagents; fan-out across harnesses is a
   dispatch plan the host runs, never something a hook or an engine starts.
8. **Uncertain effects are never replayed** (router section 6). A session
   a hook ended mid-command resumes from the task's recorded disposition;
   the SDK never re-runs a command to find out what happened.
9. **Calibrate, don't guess** (router section 5). Thresholds are frozen
   with their question-set version, tuned on one split and promoted on a
   held-out one, against the raw-delegation baseline, one lever at a time
   (`route_contract::qualify`, the briefed-ab set). A hook becomes a
   default only after that.
10. **The route record shows the hooks.** Each firing that changed
    something (a denial, a rewrite, added context, a blocked stop, a
    spend) appears in the route view (`route_contract::view`) in plain
    words, and its cost is recorded with the run's.

## Paid plugins (later)

Not built now, and not part of the issue set. The SDK is the right home
for it, so the hook points and records should leave room.

Some plugins will charge: an author-declared fee per call or per run, paid
in full after settlement (router section 8). Inside a session, the moment
to decide whether to use one is a hook moment: at session start, when a
step could use a capability the person hasn't paid for, or at a
checkpoint. At that moment the host would:

1. **List the candidates** in code: installed or installable plugins that
   fit the step, free ones included, each with its declared fee and author.
2. **Check what the person set**: their budget preferences (per task, per
   day, auto-approve under an amount, never for a category) and the current
   balance in their wallet. Only limits the person configured; none by
   default.
3. **Judge value**: a decision on whether the paid plugin is worth its fee
   here, against the free alternatives and the cost of finishing without it
   (the router's effective-cost objective).
4. **Act by rule**: under a standing budget, reserve and use it; otherwise
   make an offer, an immutable proposal with an expiry, a price and the
   effects, that the person confirms; or go on without it.

What has to hold: spending is its own authority, so a probability ranks
value but never authorizes a spend; a reservation is durable before the
call, so parallel steps can't overspend a balance or a budget; a changed
price or recipient is a new offer; and settlement, the author's fee and any
unused hold are recorded per call in the route record. The pieces exist
elsewhere (the x402 `ExecutionFront`, the payments ledger, Spark wallet
balance, offers in the route contract); the SDK would call them from a
`Spend` hook point, never reimplement them.

## Everything else the SDK should hold

1. **Session control.** The `Session` trait from `coder-delegate`: start,
   resume, steer, stop with a cleanup acknowledgment, fork, and the
   capability matrix (now including hook tiers).
2. **One event stream.** Normalized events (assistant text, tool call and
   result, file change, usage and cost, compaction, subagent) on every
   harness, so the rail, the phone and the records read one shape.
3. **Policy.** The one Coder permission and its floors (`floors.rs`) as
   built-in hooks; the worktree gate from the Claude SDK seat as another.
4. **The box.** System prompt replacement or append, tool list, setting
   sources, model and effort, per backend, from one declaration.
5. **Context.** The briefing, and conditional context (#11490, #11451
   phase 3): which AGENTS.md, memory or skill enters, chosen by a decision,
   delivered at session start or at the tail.
6. **Tools.** One MCP server of Coder tools (`claude_tools.rs` today)
   offered to every harness that takes MCP, including `verify` and
   `finish` from the briefed agent and a `decide` tool. Built (#11501):
   `coder_sdk::ToolServer` is declared once and reaches Claude Code in
   process (`in_process`) and every other harness over MCP stdio
   (`serve`). Coder chat's tools (`claude_tools::tool_server`, and `coder
   coder-tools-mcp [--decide]` on stdio) and the briefed agent's `oa`
   server (`verify`, `finish`, the lean tools; `--serve` on stdio) are
   built on it. `decide` (`coder_sdk::decide`) asks `openagents/decide`
   and is offered only when the session's `Admission` covers what it
   sends, the same check as decision hooks; each answer is a hook-log
   line. A gated server runs each call through the session's
   `PreToolUse`/`PostToolUse` hooks; Claude Code's in-process server is
   left ungated because Claude Code fires its own hooks on MCP tools.
7. **Human in the loop.** Ask outcomes, elicitation and user dialogs go to
   one approval surface (studio, chat, phone); an answer resumes the
   session with **Allow once** keyed by the request's digest.
8. **Checkpoints and handoff.** A checkpoint event at work boundaries,
   where the router may escalate to another harness with a briefing
   (#11451 phase 4), without breaking the prompt cache mid-step.
9. **Limits the person sets.** Spend, time or step limits only when the
   person configures them; the SDK enforces them as hooks, never as
   defaults.
10. **Recording and replay.** A scripted backend that replays a recorded
    session, so hook, policy and decision tests need no login and no live
    model.

## Backends

| Backend | Path | Hook tier | When |
|---|---|---|---|
| Claude Code | `claude_agent_sdk` (`coder_sdk::claude`: hooks, or `can_use_tool` for sessions that ask) | native, all events | built (#11496): floors and squeeze in one registry for Coder chat, squeeze for the briefed agent, the worktree gate for the `claude/sdk:` seat |
| Coder loop | `coder-new` live loop, Microcoder loop | native (we own it) | first |
| Scripted | recorded sessions | native | first, for tests |
| Codex | `exec` command hooks now; app-server later | command (`PreToolUse`), emulated otherwise | deferred: its usage is spent and it isn't being tested now |
| ACP agents (Cursor, Devin, OpenCode, Grok) | `acp-client` | permission requests native; the rest emulated | later |

## Open questions

- Is a decision hook's `act` plain Rust, or a workflow step, so a plugin
  can declare one without code?
- Should `FileChanged` come from the harness, or from a host file watcher
  on every harness (it would then be native everywhere)?
- What context budget per session is right? Measure on the briefed-ab set
  before setting one.
- Does the router read hook tiers as an exclusion only, or also as a cost
  (a harness that only emulates the monitor costs more to finish)?

## Issues

Tracked on project 22.

| # | Work | Issue |
|---|---|---|
| 1 | Coder SDK crate: events, outcome, composition, tiers, scripted backend; hook set in the adapter digest | [#11495](https://github.com/OpenAgentsInc/openagents/issues/11495) |
| 2 | Claude backend, with the floors, squeeze and worktree gate moved onto it | [#11496](https://github.com/OpenAgentsInc/openagents/issues/11496) |
| 3 | Coder loop backend | [#11497](https://github.com/OpenAgentsInc/openagents/issues/11497) |
| 4 | Decision hooks, with the disclosure check; then #11487–#11491 on them | [#11498](https://github.com/OpenAgentsInc/openagents/issues/11498) |
| 5 | Plugin hooks | [#11499](https://github.com/OpenAgentsInc/openagents/issues/11499) |
| 6 | Command hooks and the hook bridge | [#11500](https://github.com/OpenAgentsInc/openagents/issues/11500) |
| 7 | One Coder tools MCP server for every harness | [#11501](https://github.com/OpenAgentsInc/openagents/issues/11501) |

Deferred, filed when needed: a Codex backend on app-server (#11493
described its permission half), an ACP backend at the emulated tier, and
paid plugins.
