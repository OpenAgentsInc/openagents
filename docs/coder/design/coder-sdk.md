# Coder SDK: one host layer in front of every harness

2026-10-13. Specification, not built. What the Coder SDK is, what it takes
from the Claude Agent SDK, how hooks work on any harness, and what else
plugs in: the Decisions API, plugins, policy, tools, and the records that
let us calibrate. The proposed issue set is at the end.

Related: [system-one-hooks.md](system-one-hooks.md) (the hook ideas this
SDK carries to every harness), #11451 (harness plugins and routing),
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
   `finish` from the briefed agent and a `decide` tool.
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
| Claude Code | `claude_agent_sdk` | native, all events | first |
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

## Proposed issues

Draft for review; not filed yet.

1. **Coder SDK crate: events, outcome, composition, tiers.** The normalized
   event and `HookOutcome` types, the registry with matchers and timeouts,
   the composition rules, the per-backend tier table in the capability
   matrix, and the scripted backend for tests.
2. **Claude backend, and today's hooks through it.** Map the SDK's events
   onto `claude_agent_sdk::HookEvent` and its permission callback; move the
   floors, squeeze and worktree-gate hooks onto the SDK with no change in
   behavior.
3. **Coder loop backend.** Fire the same events from the native loop; the
   loop's tool gate (`floors::hold`) becomes the floors hook.
4. **Decision hooks.** The declared kind: rule first, Decisions API, pinned
   model, deadline, fallback, async monitoring, costs beside thresholds,
   writing to the decision log (#11492). Then re-point #11487–#11491 as
   decision hooks on the SDK.
5. **Plugin hooks.** A hooks part in the plugin package; the normalized
   event as the Wasm packet, `HookOutcome` as the reply, tighten-only
   authority with an opt-in for rewrites, receipts per firing.
6. **Command hooks and the hook bridge.** One hook binary that serves
   Claude- and Codex-style command hooks from the in-process registry, plus
   project and personal hook files.
7. **One Coder tools MCP server for every harness**, including `verify`,
   `finish` and `decide`.

Deferred, filed when the harness is back in use: a Codex backend on
app-server (#11493 described its permission half), and an ACP backend at the
emulated tier.
