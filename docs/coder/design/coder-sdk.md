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
| OpenCode (harness) | `opencode run --format json --session` per turn (`coder_new::opencode_chat`), Coder's plugin `tool.execute.before` (`coder_sdk::opencode`) asking the hook bridge | command for `PreToolUse` (before every tool, built-in or MCP, with its exact arguments; a denial stops the tool, a rewrite replaces its arguments; an ask is held); emulated otherwise. A must-deny hook on any other event refuses the turn at start (`check_backend`) | built (#11507) |
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

Built (#11507): OpenCode is also a Coder harness, the way Claude Code is
one: a chat routed to it runs each turn as `opencode run` in one OpenCode
session that later turns continue (`--session`), on OpenCode's free
`opencode/big-pickle` or a model the person picks in /models from what
their OpenCode reaches. The harness order is Claude Code (on, signed in,
usage left), then OpenCode (installed, its plugin on), then Coder's own
loop; a failed Claude Code turn judged to move falls back the same way
(`escalate::fallback`). It runs on OpenCode's own CLI and plugin rather
than ACP: the plugin hook sees each tool's exact arguments (an ACP ask for
a read names no file), so the fence and the floors read what will run.
Coder writes its plugin into a configuration folder of its own
(`OPENCODE_CONFIG_DIR`, the person's configuration still read) and passes
the model, the permissions and Coder's tools (`coder coder-tools-mcp`)
inline. Each turn's host (`sdk_hooks::opencode_registry`) has the fence,
the floors held (nobody can answer from inside OpenCode's tool call), the
command hooks and the plugin hooks. Token savers' squeezer is left out:
OpenCode keeps a rewritten command in the session, and in the live run the
model, reading `coder coder-squeeze 'make test'` for its own `make test`,
ran the check five times for one fix and later wrote the wrapper itself;
the floors now read through the squeezer like a shell (`floors::wrapped_script`).
Seen live on 2026-10-13 (OpenCode 1.18.26, `opencode/big-pickle`, an
isolated home): a question, an edit checked with `make test`, a follow-up
in the same OpenCode session, `gh pr merge 1 --squash; touch merged.txt`
held before it ran (no `merged.txt`), and with Claude Code signed out the
chat routed to OpenCode, and with OpenCode off to Coder's loop.

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

## Delegation across harnesses

Built (#11511). Any harness hands work to any other through **one
`delegate` tool**: Claude Code, Codex, OpenCode, the other ACP agents
and Coder's own loop, as parent or child. It is served the way every
Coder tool is: in process for Claude Code (`mcp__coder__delegate`), as a
dynamic tool in the `coder` namespace for Codex on app-server, by `coder
coder-tools-mcp [--harness NAME]` on stdio for OpenCode and ACP agents,
and natively for the Coder loop (`ExecutionSettings::defs`). The SDK half
is `coder_sdk::delegation`; Coder's host is
`crates/coder-new/src/delegate_host.rs`.

**Cache-safe for the parent.**

- The tool's name, description and schema are constants
  (`delegation::DESCRIPTION`, `schema()`). No harness list, model list or
  plugin state is embedded. The target harness and its model are checked
  when the call arrives (`Host::target`; models against the harness's
  catalog, as #11482 does for preferences), so an unavailable harness or
  model is refused with the ones that exist.
- In the Coder tools server the harness plugins' own tools (`agent`,
  `acp_subagent`, `microcoder`) are no longer listed; `delegate` and the
  tray tools (`agent_list`, `agent_message`, `agent_stop`) replace them.
  Turning a harness plugin on or off (Claude Code, Codex, OpenCode, the
  Coder loop, any local agent) leaves the server's `tools/list` bytes, the
  Codex `dynamicTools` bytes and the server's instructions identical
  (`delegate_host` tests). Other plugins (Solver, cloud, GitHub) still add
  their own tools; turning Claude Code off also takes Solver away and
  narrows `routing_preference` (#11516, #11518), while `delegate` stays
  the same bytes.
- Nothing is added to the parent's system prompt for delegation; the old
  line about background agents now names `delegate`.
- The child's answer is a `Report`: outcome, a summary clipped to 800
  bytes, up to 40 changed files (the rest counted), up to 20 checks, the
  worktree and branch, depth, run id, chain, and the path of the full
  transcript on disk. Never the transcript itself.
- A background child (`background: true`) answers at once with its id and
  runs on the agent tray (`agent_fleet`), which shows its activity; its
  result reaches the tray, not the parent's context.

**The child.** It reads its own briefing (`delegation::briefing`: the
task, where it works, and a closing JSON block with summary, done and
checks), not the parent's history. A writing child gets a fresh worktree
on its own branch (`coder::branch_checkout`); a read-only child works in
the folder with each engine held to reading. It runs under its harness's
own hooks: Claude Code through the Agent SDK with the fence and floors
(`claude::run_delegated`), Codex on app-server under `sdk_hooks::codex`
or in its read-only sandbox (`bundled_runtime::codex_delegated`), the
Coder loop under its floors hook, and ACP agents through `acp_briefed`
(their permission requests answered by Coder; the SDK's ACP backend from
#11508 is not yet wired into this path). A worktree with no changes is
removed with its branch.

**Authority never widens.** `Authority { write, admission }`: a child's is
`parent.child(read_only)`, so a read-only (gated) parent can only start
read-only children, and a child carries only the parent's admitted
disclosures. What a child's environment claims (`CODER_DELEGATION_AUTHORITY`)
is intersected with its own host's authority, never added to it.

**Nesting is bounded and recorded.** `Lineage` travels in the child's
environment (`CODER_RUN_ID` is the root's run id, so every decision in the
tree logs under it; `CODER_DELEGATION_ID`, `_DEPTH`, `_MAX`, `_CHAIN`,
`_HARNESS`). The bound is 2 (parent, child, grandchild); a child may lower
it, never raise it. Every call, refused ones included, appends an
`openagents.coder.delegation.v1` record (run, parent, parent harness,
child, harness, model, depth, chain, outcome) to
`delegations/delegations.jsonl` under `--state` or `~/.openagents/coder`.

**One code path.** `Delegator::delegate` runs every delegation: the tool
on each harness, `coder delegate --to HARNESS --task TEXT [--model ID]
[--read-only]` (a one-shot command has no tray, so `--background` is
refused there as in any chat without one), and the headless JSON path (`--json`
prints the child's events, then the report).

**Tests.**

- Scripted matrix (`coder_sdk::delegation` tests): every parent
  transport (in process, Codex dynamic, MCP stdio, native) calls
  `delegate` to every scripted harness (Claude Code, Codex, OpenCode, an
  ACP agent, the Coder loop): 25 pairs, each child writes a file in its
  own worktree and the parent gets a short report. Plus call-time
  refusals, fixed tool bytes, nesting to the bound, authority, and
  background.
- Live (`scripts/coder/delegation-live.sh`, isolated HOME and `--state`,
  2026-10-13): Claude Code -> Codex, Codex -> Claude Code, Coder loop
  (OpenRouter) -> Claude Code, Claude Code -> OpenCode
  (`opencode/big-pickle`), Claude Code -> OpenCode's ACP agent
  (read-only), and OpenCode -> Codex over `openagents coder-tools-mcp`:
  each child changed its file in its own worktree and the parent got the
  short report; the records show each parent -> child. Cache check: the
  same three-turn Claude Code chat with and without a delegation in turn
  2 read 0.997 and 0.998 of its last turn's prompt from the cache (Claude
  Code's usage records), and the delegating chat's first call read the
  other chat's whole cached prefix, so the tools and system prompt are
  the same bytes. A free model can claim a change it never made; the
  report's empty `changed_files` shows it.
- The bundled Microcoder loop with no model provider (on a Codex login)
  has only shell commands, not `delegate`; there it uses `openagents coder
  delegate --to`.

## Where sessions start (#11509)

Every place an agent session starts, its harnesses, and what of the SDK it
runs. "The registry" is one `coder_sdk::Registry` composed by one
`Runner`: the floors (`coder_sdk::floors`, one table, moved out of Coder
for this), the shared-checkout fence (#11485, still in
`coder_new::checkout_guard`), the worktree guard (`coder_sdk::worktree`,
the one copy of the "file tools held to the worktree" check that the issue
run, the briefed agent and the Solver's Devin boundary each wrote), Token
savers' levers, the plugins' hooks, and the System One monitor; decisions
log through `hook-log`.

| Surface | Starts at | Harnesses | On the SDK | Left |
|---|---|---|---|---|
| Coder chat (terminal, desktop) | `coder_new::claude`, `codex_chat`, `opencode_chat`, `provider::chat_with_plugins` | Claude Code, Codex app-server, OpenCode, Coder loop | `sdk_hooks::claude` / `codex` / `opencode_registry` / `coder_loop`: the registry, harness order Claude Code → OpenCode → Coder loop (`escalate::fallback`) | Claude's gate and context levers are raw Claude hooks; the loop's file tools call `floors::hold` directly (#11531) |
| Issue runs (`coder issue-run`) | `issue_run::agent::run_gated` | Claude Code (Agent SDK), Devin (ACP), Coder loop | Claude: `sdk_hooks::task_claude` (the registry, the worktree guard over the offered tools, squeeze and the Stop gate as registry hooks, `can_use_tool` through the runner). Devin: `sdk_hooks::task_acp` answering every ask, Devin's own hooks running the bridge (`acp_gate`). Coder loop: `sdk_hooks::coder_loop` | No OpenCode engine, so no OpenCode step in the order (#11533) |
| Solver hands (Intaker, Briefer, Coder, Tester) | `solver::hands` → the same `agent::run` | as issue runs | as issue runs; a read-only hand's guard refuses every write | as issue runs |
| Briefed agent (bench, `openagents work`) | `briefed-agent` `run` → `compose` | Claude Code (Agent SDK); Codex `exec`; OpenCode `run` | Claude: one registry on the Claude backend: the worktree guard (offered tools, the squeeze folder readable), the floors held, squeeze, the Stop gate | Codex and OpenCode engines run no hooks (#11530); no shared-checkout fence (#11532) |
| Coder One delegations | `coder_delegate::adapter::CliSession` from `coder_one` (issue mode `main.rs`, episodes `episode.rs`) | Claude Code `-p`, Codex `exec`, OpenCode `run` | `coder_one::hooks::install`: a bridge host with the floors held; `Cli::hooks` (`HookBridge`) puts the bridge in as Claude Code's `--settings` PreToolUse hook and Codex's `-c hooks.PreToolUse`; `coder-one coder-hook` is the bridge program, holding the floors alone when the host is gone | OpenCode `run` takes no command hooks (emulated) |
| Coder Terminal turn and `crates/coder`'s issue flow | `coder_delegate::terminal::answer` (`delegate_door`), the issue flow's Microcoder turns (`delegate_door::microcoder::answer`) | Claude Code, Codex, the Coder loop | `coder::hooks`: a bridge host with the held floors, the worktree guard (writes held to the turn's folder, none on a read-only turn) and the shared-checkout fence; `terminal::Request::hooks` puts the bridge in as the CLI's own `PreToolUse` hook, `coder coder-hook` (or `openagents coder-hook`) answering; the Microcoder loop fires the same registry (`microcoder_loop::env::Hooked`) (#11529) | none |
| ACP Subagents, background agents, the `delegate` tool | `bundled_runtime::acp_briefed`, `delegate_host` (#11511) | ACP agents, Codex, Coder loop, Claude Code | `sdk_hooks::acp` / `codex` / `coder_loop`; delegation per #11511; a Claude background agent (`claude_print`, `claude -p`) runs `sdk_hooks::claude_print` through the hook bridge: the held floors, the person's and the project's command hooks, the plugins' hooks, the shared-checkout fence, the worktree guard on writes, and the squeezer when Token savers is on (#11528) | none |
| Microcoder | `microcoder::repository` (claude_sdk seat, lean/Claude/Codex CLI sessions, Devin/OpenCode/Grok ACP turns), `microcoder_loop::run` | Claude Code, Codex, ACP agents, the loop | `microcoder::repository::hooks` (#11527): one registry per session, the held floors, the worktree guard on writes (`Guard::writes`) and the shared-checkout fence. The loop fires it through `microcoder_loop::env::Hooked` (repository runs and terminal-bench); lean CLI sessions run it through the hook bridge (`microcoder coder-hook`); ACP asks meet it before the access's own answer, and the agents' own hooks reach it through `coder_sdk::acp::gate` (moved here from `coder-new`; OpenCode asks before every tool under full access); the `claude/sdk:` seat's gate admits a file tool only where `coder_sdk::worktree` keeps it inside and no floor names it, and asks the person otherwise. Its own `Gate::inside` path check and `resolved` copy are gone | none |
| Cloud runtime | `coder_cloud::drive` → `coder delegate <agent>` on the machine | as background agents | as background agents, Claude through `claude_print` with its hooks (#11528) | none |
| Web and phone chat worker | `crates/coder/src/bin/coder-worker.rs` `start_model` | none: model calls through the gateway fallback chain, no tools | nothing to run: no session, no tool | none: its "Run Claude Code" goes through the cloud runtime |

Each surface moved here has one test that runs a scripted session through
its real start path and shows a floor holding:
`sdk_hooks::tests::a_task_session_on_claude_code_meets_the_floors_and_the_guard`
(issue runs and Solver hands on Claude Code),
`solver::tests::a_hands_asks_meet_the_floors_and_the_worktree_guard` (the
recorded Devin turn with asks injected, through `agent::run`),
`compose_tests::a_scripted_session_meets_the_floors_and_the_worktree_guard`
(the briefed agent),
`hooks::tests::a_delegated_session_runs_the_floors_through_the_bridge`
(Coder One, through `CliSession` on a stand-in Claude Code),
`delegate_door::tests::a_terminal_turn_runs_the_floors_through_the_hook_bridge`
(the Terminal turn, through the delegate door on a stand-in Claude Code), and
`delegate_door::tests::a_microcoder_turn_holds_a_floored_command_for_the_person`
(a Microcoder turn, as the issue flow runs its sessions),
`claude_print::tests::a_background_claude_task_runs_the_floors_through_the_hook_bridge`
(a background Claude agent, through `acp_briefed` on a stand-in Claude Code),
`claude_session::session_tests::a_lean_session_runs_the_floors_through_the_hook_bridge`
(Microcoder's lean CLI session, through the repository stages),
`claude_sdk::tests::a_floored_read_inside_the_worktree_ends_the_turn_asking`
(the `claude/sdk:` seat), `devin::tests::a_devin_turn_meets_the_floors_under_full_access`
(Microcoder's ACP turns), and
`repository::tests::a_repository_run_holds_a_floored_command` (the loop).

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
| 8 | Delegation across harnesses through one `delegate` tool | [#11511](https://github.com/OpenAgentsInc/openagents/issues/11511) |
| 9 | Every surface on the SDK: the inventory, the floors and worktree guard in the SDK, issue runs, Solver hands, the briefed agent, Coder One | [#11509](https://github.com/OpenAgentsInc/openagents/issues/11509) |
| 9a | Microcoder's sessions | [#11527](https://github.com/OpenAgentsInc/openagents/issues/11527) |
| 9b | Background Claude agents and the cloud runtime | [#11528](https://github.com/OpenAgentsInc/openagents/issues/11528) |
| 9c | The Coder Terminal turn and `crates/coder`'s issue flow | [#11529](https://github.com/OpenAgentsInc/openagents/issues/11529) |
| 9d | The briefed agent's Codex and OpenCode engines | [#11530](https://github.com/OpenAgentsInc/openagents/issues/11530) |
| 9e | Coder chat's remaining per-harness wiring | [#11531](https://github.com/OpenAgentsInc/openagents/issues/11531) |
| 9f | The shared-checkout fence into the SDK | [#11532](https://github.com/OpenAgentsInc/openagents/issues/11532) |
| 9g | OpenCode in the issue-run and Solver engine order | [#11533](https://github.com/OpenAgentsInc/openagents/issues/11533) |

Built since: the Codex backend on app-server (#11510; #11493 described
its permission half), and the ACP backend with per-agent tiers (#11508).
Deferred, filed when needed: paid plugins.
