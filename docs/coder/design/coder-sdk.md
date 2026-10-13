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

Built (#11500): the bridge is `coder_sdk::bridge`. The host keeps a
session's runner on a Unix socket (`bridge::Host`, named to the harness
by `CODER_HOOK_SOCKET`); the hook program (`coder coder-hook --format
claude|codex`, and `coder-floor-hook` for Codex) reads the harness's hook
input, asks that host, and prints the reply in the harness's format
(Codex's narrower one: an ask is held, `allow` only with
`updatedInput`). With no host, the floors answer alone. A Codex run
(`bundled_runtime::codex_cli`) starts a host with the floors and the
command hooks (`sdk_hooks::codex_registry`). Command hooks
(`coder_sdk::command`) come from `.openagents/hooks.json` in the checkout
and `~/.openagents/hooks.json`, in Claude Code's `hooks` shape, run under
`sh -c` with Claude Code's hook input and are read as Claude Code reads
them (exit 2 blocks); they compose with the built-ins, never past a floor.

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

Built (#11499): `coder_sdk::plugin_hooks`. The package record's `hooks`
pins documents under `hooks/` (event, matcher, operation, module by
digest, `authority`, `discloses`, `decide`); `coder::package` accepts the
part. Each firing runs the guest in a fresh `snapshot-read` instance with
an empty snapshot, so its one import reaches only a granted capability
(`decide`, through `plugin::invoke_with_host`); the firing keeps the
invocation receipt and the host exchanges and replays exactly
(`plugin_hooks::replay`). Authority is narrowed in code (`narrow`); a
rewrite asked for without the person's grant is withheld and recorded;
`spend` parses and is never granted. `coder-new::plugin_hooks` registers
every installed plugin's hooks into each session's registry while the
plugin is on, admitting what its listing shows, and keeps notes and
firings under `~/.openagents/plugin-hooks/`. `with_and_without` replays a
recorded session with and without a plugin's hooks for its tests. The
example is `crates/plugin-hook-guard`.

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
| Coder loop | `microcoder_loop::run` (`Env::hook`, `microcoder_loop::hooks`), used by Coder's live loop and Microcoder | native for SessionStart, UserPromptSubmit, Pre/PostToolUse, PostToolBatch, Stop, PostCompact, SessionEnd; none for the subagent, PreCompact and FileChanged events it never fires | built (#11497): the floors are the loop's hook (`sdk_hooks::coder_loop`); Microcoder's own places fire with no hooks yet |
| Scripted | recorded sessions | native | first, for tests |
| Codex | `codex app-server` (`coder_sdk::codex`); `exec` with command hooks as the fallback | app-server: native for SessionStart, UserPromptSubmit, PreToolUse (Codex asks before each command and patch under `approval_policy = "untrusted"`), Stop (a block starts the next turn on the same thread) and SessionEnd; emulated otherwise. exec: command (`PreToolUse`), emulated otherwise | built (#11510): approvals answered by the session's `PreToolUse` composition, an ask answered on the approval card while Codex waits, Coder's tools as dynamic tools in process |
| ACP agents (OpenCode, Grok Build, Devin, Cursor, Amp, Goose, the rest) | `acp-client` (`coder_sdk::acp`) | per agent, from live runs: OpenCode asking before every tool is native for `PreToolUse`; Grok Build's plugin hooks and Devin's config hooks run the bridge, the command tier; Cursor and the rest stay emulated before a tool. Every agent: native for SessionStart, UserPromptSubmit, Stop and SessionEnd, which the host drives; emulated otherwise | built (#11508) |

Built (#11510): Codex on `codex app-server` (`coder_sdk::codex`).
Threads open under `approval_policy = "untrusted"` with full disk access,
never Codex's bypass flags, and every command and patch approval runs the
session's `PreToolUse` composition (`sdk_hooks::codex`: the fence, the
floors, the gate, conditional context, the command and plugin hooks, the
monitor). An allow accepts the call; a deny declines it and the reason is
steered into the same turn; an ask goes to the approval card while Codex
waits, so Allow once and Deny both continue the same thread. Calls reach
the hooks in Claude Code's shapes (`Bash` with the command, `apply_patch`
with the patch text), and the floors read through Codex's
`/bin/zsh -lc '...'` wrapper. Coder's tools ride along as Codex dynamic
tools in process, through the same hooks, so delegations reach the agent
tray. Codex keeps a thread's dynamic tools from its start and restores
them on `thread/resume` in a new process (it takes none on resume), so a
reopened thread that lacks one of Coder's tools is refused and the chat
starts a thread that has them, from the pasted conversation (#11525).
Codex runs exactly the command it asked about, so a rewrite (the
squeezer) declines the command and steers the rewritten one to run
instead; the developer instructions tell Codex to run build, test, and
lint commands through the squeezer from the start, which it does (a live
`make test` came back as 57 lines of 602, #11525). Coder chat runs Codex as a harness
(`coder_new::codex_chat`): one app-server per chat, follow-ups on the same
thread, the briefing as developer instructions with `AGENTS.md` reading
off, a /models list of Codex's models, its plan windows on the rail while
it runs. A background Codex agent with full access uses app-server too;
`exec` with the command hook stays the fallback where app-server won't
start, and for read-only and workspace-write runs, whose sandbox Codex
enforces itself.

Built (#11508): every ACP agent the ACP Subagents plugin delegates to runs
on `coder_sdk::acp`, with the same hooks as Codex (`sdk_hooks::acp`: the
fence, the floors, the gate and conditional context when Token savers is
on, the command and plugin hooks, the monitor). Each agent is started so
its tools reach those hooks first where it allows it, and the tier table
(`coder_delegate::session::acp_backend`) records only what was seen live
on 2026-10-13:

| Agent | Started with | Before a tool | Seen live |
|---|---|---|---|
| OpenCode 1.18.26 | `permission` set to ask (`OPENCODE_CONFIG_CONTENT`, the person's other settings kept) | native: it asks before every read, command and edit, and each ask is answered by the hooks; a read's ask names no file, so a secret file read that way is not judged | yes |
| Grok Build | a `--plugin-dir` plugin whose `PreToolUse` hook runs `coder coder-hook --format acp` | command: the hook runs before every tool, reads included, and Grok obeys a denial and shows the model its reason | yes |
| Devin 3000.11.3 | `--config` with the person's own configuration plus that hook | command, as for Grok, in any of its modes | yes |
| Cursor | its `agent` mode | emulated: it asks before a command outside its allowlist, and those asks are answered by the hooks, but it edits and reads without asking | yes |
| Amp, Goose, Kimi, Oh My Pi, Hermes, Codex's ACP adapter | as before | emulated; their asks are answered by the hooks | not installed here |

Claude Code delegated from the plugin runs `claude -p` (the Claude Code
backend), not ACP.

On an agent that can't be stopped before a tool, a hook that must deny
there (the floors, the fence) is reported before the run, in plain words,
on the agent tray and in the delegation's `checks` ("Cursor doesn't ask
before every step, so Coder can't hold a deploy, a merge, a secret or a
payment for you, or keep it out of the shared checkout before it acts.
The steps it asks about are still checked."); the hooks stay registered
and answer the asks it does send. A floor's ask waits on the approval card
for an agent that asks; through the bridge it is held (nobody can answer
from inside the agent), as for Codex `exec`. ACP's refusal carries no
reason, so a refused step's reason is sent as the next prompt in the same
session; a `Stop` block's reason is sent the same way, a few times at
most. The chat's briefing goes ahead of the task.

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

Built since: the Codex backend on app-server (#11510; #11493 described
its permission half), and the ACP backend with per-agent tiers (#11508).
Deferred, filed when needed: paid plugins.
