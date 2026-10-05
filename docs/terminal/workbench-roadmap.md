# OpenAgents Terminal: the workbench roadmap

Status: owner-directed delivery plan, October 5, 2026. The first release
target is today: the same real terminal application in the desktop Grid and
as a separately installable native window. The next pass connects that
application to the Agent Studio in Everglade. Later releases make it a
multiplexer for all work across desktop, mobile, web, Verse, and headless
computers. This document records intended behavior; it is not release evidence.

This plan takes precedence over the delivery order in the October 5
[smart terminal proposal](smart-terminal.md). That page remains the technical
specification for blocks, input routing, emulation, sessions, and sharing.
The [user guide](README.md) describes the shipped chat TUI. The
[original scope](scope.md) and [October 2 gap analysis](2026-10-02-coder-terminal-gap-analysis.md)
record earlier delivery stages.

## Product decision

OpenAgents Terminal becomes the workbench application. Its first panes are
real shells and OpenAgents threads. It grows to hold agent runs, files,
diffs, tools, previews, knowledge, evaluations, background work, and cloud
computers. Each pane opens an existing domain resource; the workbench does
not acquire a second router, task scheduler, studio coordinator, or wallet.

The standalone window and Verse mount the same Rust application. Pressing
`T` in the Grid opens that application as an overlay. In Everglade, a desk,
console, keyboard shortcut, or another interaction can open it with the
workshop's context. Whether terminals are furniture, floating windows, or
both remains a separate world-design decision. It does not block either
release and does not change where work executes.

Keep the existing commands usable:

| Entry point | Role in this plan |
| --- | --- |
| `openagents-terminal` | Proposed binary of `terminal-app`: the standalone graphical application |
| `T` in desktop Verse | Mount the same terminal application over the Grid or another zone |
| `openagents terminal` | Existing text chat and thread view, usable in a PTY, over SSH, and in tmux |
| `openagents studio` | Existing Agent Studio operations; the workbench projects these operations |
| `openagents computer shell HOST` | Existing NIP-TERM client for a paired computer |

The package can ship `openagents-terminal`, `openagents`, and `microcoder`
together. The standalone application must work without installing or
launching Verse. The existing `openagents-terminal` library stays the chat
view; its name does not imply that it already contains a graphical emulator.

## What the recent episodes require

The review starts with the [archive guide](../transcripts/README.md) and
covers all fifteen retained episodes from [275](../transcripts/275.md)
through [289](../transcripts/289.md). These are historical statements and
demonstrations, not proof that a feature survived a repository reset.

| Source | Requirement carried into this plan |
| --- | --- |
| [275: Coder](../transcripts/275.md), [277: Coder Terminal](../transcripts/277.md) | A dependable daily tool, immediate typing, visible commands, local data, and optional keys, compute, and synchronization |
| [276: Coder Cloud](../transcripts/276.md), [280: CoderOS](../transcripts/280.md) | Optional rented computers and a broader computer-work product; historical dollar prices are not current offers |
| [278: Codex and Claude](../transcripts/278.md), [279: Claude Code study](../transcripts/279.md) | Provider-neutral execution with truthful child identities, readiness, progress, cancellation, and results |
| [281: Coder Mobile](../transcripts/281.md) | Continue and control the same work through explicitly trusted devices |
| [282](../transcripts/282.md), [283](../transcripts/283.md), [284](../transcripts/284.md) | Broad access and a productive world; progression rewards checked outcomes and useful contributions, rather than tokens or activity |
| [285](../transcripts/285.md), [286](../transcripts/286.md), [287](../transcripts/287.md) | Rust control, typed judgments, explicit context, reusable components, and retained checks; an oracle portfolio is not an implemented routing policy |
| [288](../transcripts/288.md), [289: OpenAgents](../transcripts/289.md) | One composable general agent across terminal, desktop, phone, and web; Coder, plugins, knowledge, and evaluations are capabilities within it |

The resulting product is broader than an AI shell and broader than a coding
dashboard. The first useful release stays small, while resource identities
and adapters leave room for the full suite.

## Current foundations and gaps

| Foundation | Current evidence | Work this plan adds |
| --- | --- | --- |
| Chat TUI | [README](README.md): shared threads and router, Coder runs, steering, files, diffs, settings, plugins, and pairing | Use it as the first thread pane inside a real terminal |
| Desktop terminal | [In-world terminal](../verse/in-world-terminal.md): PTYs, `coder-vt`, splits, tabs, input, copy, search, and measured output budgets | Shell blocks, requests, proposals, shared extraction, and independent installation |
| Host terminals | `coder-pty`, `coder-host`, and [NIP-TERM](../../nips/openagents/NIP-TERM.md) | Authoritative emulation, snapshots, session records, typists, and sharing |
| Agent Studio | [Primary specification](../verse/agent-studio.md): host coordinator, goals, seats, task worktrees, decisions, review, local merge, CLI, and phone actions | Bind terminal and workbench panes to those same resources |
| Agentic execution | [Router plan and local exit evidence](../api/2026-10-02-agentic-execution-router.md#phase-1-exit-evidence-the-local-vertical-slice-2026-10-02-10207): `route-contract`, shared route policy, journals, dispatch, and checks | Project route identity, placement, cost, and lifecycle in the workbench; extend ordinary shell proposals |
| Cloud | [Cloud index](../cloud/README.md): Boat and GCE placement for issue work; [cloud fallback](../coder/runtime/cloud-fallback.md) for hosted inference | A customer-facing paid cloud-computer option with credits, admission, and recovery |
| Other products | Knowledge, plugins, Gym, background rules, wallet, and retained artifacts have their own owners | Typed panes and links into those owners, progressively supported on each surface |

The October 4 [AgentCraft parity](../verse/agentcraft-parity.md) and
[studio audit](../verse/agent-studio-audit.md) are dated observations. The
primary studio document records subsequent mixed-engine execution and
phone actions. Do not repeat their earlier absence as today's state.
Conversely, a studio seat's log monitor is not already an interactive PTY.

The current Verse overlay owns local PTYs in process. Hiding it keeps them;
exiting Verse ends them. The standalone MVP has the same honest lifetime.
Host-backed chat threads and managed Coder or studio tasks retain their own
existing lifetimes. Durable terminals arrive in a separate milestone.

## Today's release: Grid and standalone

Target date: October 5, 2026. Three issues deliver one MVP. The date is the
priority, not evidence that the code or publication has passed its checks.

### 1. Smart behavior in the shared terminal

Issue [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642).
Keep behavior reusable as the application is extracted in parallel.

- Open a login shell as the initial pane. Keep `openagents terminal` one
  action away as a thread pane. The current chat-first pane is not the MVP.
- Parse OSC 133, OSC 7, and bounded private hook messages in `coder-vt`.
  Anchor marks to absolute lines and preserve correct ranges through
  scrollback. Treat all output marks as advisory, never execution authority.
- Inject a zsh hook without editing dotfiles. Keep the shell's editor,
  history, completion, aliases, and keybindings. zsh is the first release's
  supported integrated shell; bash and fish follow with isolated hook tests.
  Other shells still work as terminals and can use the explicit ask action.
- Record command, output range, exit status, timing, and directory as a
  block. Add a gutter, block navigation, copy, collapse, rerun, and attach.
- Show Shell or Request before Enter. Explicit mode and `# ` request prefix
  win; resolve command structure locally. Ambiguous input stays Shell for
  this release. Without reliable hook information, preserve ordinary shell
  input and offer explicit Ask. No local classifier or remote shell-line
  classification is required today.
- An ask action works over a program without intercepting its ordinary
  keys. A request previews its directory, Git summary, and attached blocks
  in a removable context strip, then opens an OpenAgents thread in a split.
  Scrub attached output before the preview. A nearby failure is offered as
  context and is sent only as shown in that strip.
- Consume typed proposals from the shared client and router. Start with
  the CLI route and add the ordinary-shell proposal needed by the demo.
  Never infer executable commands by scraping Markdown. Every live-shell
  proposal remains pending until Enter approves its exact command. Editing
  it changes the proposal; completion reports its own resulting block back
  to the thread once. Coder worktree runs keep their existing policy.

The PTY thread view needs a new typed bridge: create and submit the request
once through the shared client, then attach the TUI to that thread ID.
Bind a shell proposal to its thread, proposal revision, exact command,
target terminal generation, directory, displayed context, and approval
identity. Send the resulting block with that identity and acknowledge it
idempotently. Drawing a TUI in a pane alone does not provide this bridge.
A lost execution acknowledgment requires reconciliation, never automatic
shell-command replay. A rerun or edited command creates a new block and
retains its relation to the original proposal.

The supported MVP does not auto-run read-only proposals. Full native thread
rendering, the local line classifier, other integrated shells, and session
sharing can follow without weakening the live-shell approval boundary.

### 2. The same application in an installable window

Issue [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643) can start
beside #10642: agree on the block and request interfaces first and integrate
their implementations into the extracted application.

- Extract `terminal-core` from Verse's pane, layout, input, copy, selection,
  and session state. Give it application-owned key types and an injected
  transport interface; it has no Verse, window, or network dependency.
- Extract `terminal-gfx` from the existing glyph-grid drawing, using
  `verse-gfx` foundations. It has no world or studio dependency.
- Add `terminal-app`, a thin `winit` window mounting those crates, and move
  Verse onto the same application through an adapter. Preserve existing
  keyboard, mouse, Unicode, clipboard, resize, output, and frame budgets.
- Share blocks, requests, proposal approval, context, and layout behavior
  between the window and Verse. Mounting in a world is presentation, not a
  different terminal backend.
- Package the native macOS build first, with the matching chat and engine
  helpers. Record supported platforms rather than claiming a Windows,
  Linux, phone, or web run that has not happened.

A window that links the whole Verse application or opens only the existing
chat TUI does not satisfy this issue.

### 3. Integrate, install, and retain the demo

Issue [#10644](https://github.com/OpenAgentsInc/openagents/issues/10644)
depends on #10642 and #10643.
Use a scratch repository and isolated hosts, never the owner's real chats
or credentials, for scripted acceptance.

The video can demonstrate this sequence:

1. Enter the desktop Grid and press `T`. A real shell opens over the world;
   the world does not consume keys while the terminal has focus.
2. Run a fixture's failing `cargo test`. Inspect and copy its failed block.
3. Type `# why did that fail`. Review the context strip and send it. The
   thread opens beside the shell and proposes a command.
4. Edit or select the pending proposal and press Enter. Observe shell output
   as a new block and the thread's continuation with that result.
5. Split another shell, run a full-screen program, switch tabs, and return
   focus to the world. Hiding and reopening preserves local panes.
6. Install the published standalone build into an isolated directory and
   repeat the same shell/request flow with Verse absent. Show its version
   and source commit. This demonstrates the same behavior; moving a live
   PTY between the two processes is a later milestone.

Retain captures, commands, executable identities, fixture source, actual
checks, performance percentiles, and limits under `docs/verse/verification/`.
Measure startup to usable input and report it; freeze a target before making
a startup claim. Reuse the [terminal performance workload](../verse/verification/2026-10-05-terminal-performance/README.md)
and its declared budgets, with blocks enabled and both surfaces measured.
The request/proposal path needs a real end-to-end receipt as well as scripted
fixtures; do not present a simulated answer as a live route.

Reuse the signing, notarization, checksums, source manifest, public readback,
and install checks of the [terminal release process](../release/terminal.md).
A new standalone package uses the `openagents-terminal/` prefix and an explicit platform manifest;
do not weaken the existing seven-platform `openagents` channel rule. Publish
only supported artifacts and name the installation command in the receipt.
Today requires a separately downloadable installation, not only `cargo run`.

## Next pass: the Everglade workshop

The [Agent Studio](../verse/agent-studio.md) is the authority for team work.
Everglade projects its state into characters and stations. Add the shared
workbench as another projection; preserve those two existing boundaries.

Define a station-opening context that references the host, admitted
workspace, repository or task worktree, goal, seat, task, thread, and review
revision when present. These are resource references, not permission and
not copies of the records. An ordinary `T` opening may have no studio context.

| Studio resource | Workbench experience | Owner of changes |
| --- | --- | --- |
| Goal and Task Wall | Goal, dependency graph, task status, and linked runs beside shells | Existing studio coordinator |
| Seat and desk | Log or run view, engine identity, task worktree, and a separately authorized shell | Task owner and host |
| Podium | Questions and exact pending approval decisions | Existing studio decision queue |
| Merge station | Exact-revision diff, checks, lead review, merge or request changes | Existing studio review and merge path |
| Library | Shared memory, plan, knowledge, and admitted reusable components | Studio memory and existing knowledge owners |
| Workbench and test bench | Commands, output blocks, test artifacts, and Gym evidence where attached | Terminal, task owner, and Gym |

Opening a seat log does not hand raw PTY input to its managed agent. Steering
uses the task's existing operation. A shell in a task worktree requires
explicit host admission; concurrent edits must not invalidate a review
silently. Refresh or refuse a stale review using the existing exact-revision
rules. A terminal proposal cannot approve a studio merge.

Start with desktop Everglade and its existing host control path. Later phone
and browser workbench adapters reach the same resources through their
admitted transports. World membership grants no repository, terminal,
disclosure, review, or spending rights. Never put shell bytes, commands,
paths, or private studio artifacts in presence or public world messages.

Acceptance: open the same goal from `T` and a workshop entry point, inspect
a worker, answer a decision, open its exact diff and checks, and perform the
existing reviewed local merge. Reopen the workbench without duplicating a
goal or task. Retain a labeled simulated receipt and a scratch real-engine
receipt; do not spend on the owner's devices for a smoke.

## Sessions and the multiplexer for all work

The durable session is the unit of work. A session references terminals,
threads, runs, studio resources, artifacts, and a default layout. A pane is
a viewer's rectangle on one resource. Closing a pane detaches; stopping a
task or closing a terminal is a separate operation with an honest result.

Keep task lifecycle, thread storage, studio state, financial records, and
files in their current owners. The session owns relationships and navigation.
Bind references to the owning host, generation, and revision where required;
a stale reference becomes lost or stale, never a newly created resource
disguised as the old one. One session may reference several admitted hosts.

Host terminals gain authoritative `coder-vt` emulation, side-effect ownership,
screen-first snapshots with parser continuation, history pages, and block
journals. Only the host answers device queries. Add the one-typist rule,
explicit handoff, viewer-local selection and scroll, and separately scoped
watch/drive shares. A client closing preserves terminal processes; a host
restart reports them lost. This does not promise process migration.

Grow the pane registry in useful slices:

- Shells, threads, agent runs, child tasks, decisions, files, diffs, previews,
  and retained artifacts.
- Studio goals and seats, knowledge and memory, plugin/program tools, and
  Gym experiments with sources, recipes, traces, checks, and comparisons.
- Background rules, cloud-computer readiness, resource usage, quoted costs,
  payer records, receipts, and links to the wallet or account owner.
- Specialist computer tools and product views through admitted capabilities.
  Each capability declares what a surface can draw and do; a missing view
  opens a useful fallback or a link, rather than a fictional working pane.

The same resource directory and intent contracts serve desktop, Verse,
phones, browsers, and headless clients. Phone and browser shell panes drive
host PTYs; they do not acquire a shell on the device. Preserve the existing
TTY chat and paired-host shell; a full TTY multiplexer client is a later
option, not a prerequisite for the native MVP.

Make reuse a workbench workflow: discover an admitted component, inspect
its version and provenance, test its effect with Gym, and use it through the
shared router. A missing capability can enter the existing create/evaluate/
publish flow. Lessons harvested from a run need source references, separate
admission, and evidence on work that did not teach them. Preserve failed
trials and the cost of setup and checking. Author fees, provider earnings,
and progression follow their own accepted contributions; a busy terminal
does not establish any of them.

## One execution router, including cloud

The terminal's Shell/Request choice is local input routing. The
[agentic execution router](../api/2026-10-02-agentic-execution-router.md)
then decides what a request needs: answer, knowledge, model, plugin/program,
local command, Coder task, or an offer for missing capacity. Studio operations
initially use the existing typed studio intents; a router adapter for them
is a proposed extension, not an existing route family.
The workbench reads shared typed results and admissions; it does not infer
intent from keywords, create a parallel generator, or treat a displayed
command as authority.

Keep placement separate from engine selection and the viewing surface.
A request may run on this computer, another granted computer, or an
explicitly selected paid OpenAgents cloud computer. It may use the user's
engine login, BYOK model, sponsored inference, or an admitted paid provider.
Each choice retains its computer, executor, disclosure, payer, and cost
identity. A tablet in Everglade can watch a cloud task without executing it.

Reuse `route-contract`, `openagents_chat::route`, the route journal, task
owner, host grants, capacity book, supervisor, and existing placement
adapters. Show actual state and artifacts, including checking, failure,
unknown cost, and uncertain dispatch. Continue or steer the existing run
instead of starting another. A changed computer, recipient, effect, or price
needs a new admission; a timeout never authorizes execution elsewhere.

### Paid OpenAgents cloud computers and credits

The owner wants an optional cloud-compute product usable inside the game
and every other surface, paid with OpenAgents credits. This is a new retail
integration over existing Boat/GCE foundations, not a claim of a purchasable
credit product today. Hosted inference fallback remains a different resource
from a rented remote computer.

Treat credits as the display of a prepaid compute balance. Define the unit
and any conversion in a versioned price book, while preserving the API's
[sats accounting decisions](../api/2026-10-02-agentic-execution-router.md#8-prices-reservations-and-payments).
Do not equate credits with XP, game gold, a wallet balance, provider credits,
Boat's allowance, or GCE's infrastructure bill. Rewards for useful work and
payments for compute remain separate, even when the world shows both.

The first paid slice needs:

1. A top-up/account view and a durable purchased balance with documented
   denomination and refund terms.
2. An immutable offer showing computer class, repository/material disclosure,
   model payer, compute and provider charges, maximum charge, and price-book
   version. Ordinary local work needs no OpenAgents credit purchase.
3. Separate observation, execution, disclosure, and spending admission.
   Neither pairing nor sufficient balance supplies the other rights.
4. Durable reservation before provisioning or dispatch, one funded execution
   identity, teardown, retained artifacts, and exact metering scope.
5. Settlement, release of unused funds, and reconciliation of unknown costs
   after a crash. Do not free an uncertain reservation or duplicate a task
   when a client reconnects. Revocation and cancellation retain their actual
   acknowledgment and final charges.
6. A visible receipt per task or session, plus shared balances and usage
   across the window, workshop, phone, and web. Do not create a game-only
   balance or a terminal-only payment ledger.

Launch one computer class and one task class first. Prove fake-payment and
crash recovery before a small funded test. Broader paid workers, bids, escrow,
training markets, and contributor payouts have their own acceptance; they
do not block the terminal MVP or become implemented through this roadmap.

Code-complete qualification tooling closes after it lands and its own safe
checks pass. An owner-only funded run stays in `NEEDS_OWNER.md`, with exact
limits and commands. Paid availability remains disabled until the supported
configuration has actual qualification evidence; an issue's closure is not
that evidence. A failed owner verification opens a new defect issue.

## Delivery sequence and ownership

Dates beyond today are ordered milestones, not elapsed-time promises.
Host/protocol work can run beside the Everglade adapter once application
interfaces are stable. Paid cloud depends on funding and admission, not on
decorating every workshop station.

| Milestone | Deliverable | Dependencies and exit evidence |
| --- | --- | --- |
| Today A | Smart shell, blocks, requests, pending proposals (#10642) | Shared interfaces with B; isolated hook/parser/policy tests and the failing-test flow |
| Today B | Shared core/renderer and standalone native window (#10643) | Can begin alongside A; identical behavior in Grid and window, no Verse runtime dependency |
| Today C | Public native install and video-ready receipt (#10644) | A + B; signed supported artifact, checksum/public readback, isolated install, both-surface demo, measured performance |
| Next pass | Everglade studio adapter and bash/fish integration | A + B; existing studio resource identities, decisions, review, no duplicate execution, declared shell coverage |
| Host sessions | Authority parse, snapshots/history, journal, durable attach | NIP-TERM conformance and host/client side-effect tests; closing a client preserves work |
| Workbench breadth | Typed product panes, children, artifacts, background work | One noncoding task, such as meeting-note processing with an admitted plugin, keeps its thread, output, artifact, and check navigable in one session; truthful fallbacks |
| Reuse and contribution | Capability creation/evaluation/publication and cited knowledge candidates | Build one missing capability, compare with/without it, publish an exact version, and confirm useful reuse on unseen work; knowledge promotion remains separately admitted |
| Multi-host and sharing | Saved sessions, typist handoff, watch/drive grants | Host sessions; per-message revocation and explicit viewer/driver behavior |
| Paid cloud | Credit-funded cloud placement through the shared router | Account ledger, price book, four admissions, metering and crash recovery; one funded checked outcome |
| Mobile and web | Same session/resource contracts and permitted controls | Supported transport and renderer per surface; real reconnect, device/browser input, no duplicate dispatch |
| World screens and productive economy | Optional screen textures and shared views; useful-work progression | Explicit sharing and evidence; world placement decided independently, rewards never inferred from activity |

Terminal owns panes and input; host/task owners own execution; the studio
owns team coordination; the router owns admitted route selection; payments
own funding; Verse owns world presentation. Extend those owners rather than
building another product backend inside the terminal.

## Issue tracking

Today's implementation stays in three issues: [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642)
for smart behavior, [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643)
for the shared standalone application, and [#10644](https://github.com/OpenAgentsInc/openagents/issues/10644)
for release/demo integration. The
last issue is blocked by the first two; application extraction and behavior
work start in parallel. The owner requested the entire remaining roadmap
as issues on October 5. The public
[OpenAgents Terminal and Workbench project](https://github.com/orgs/OpenAgentsInc/projects/20)
and [complete issue directory](issue-roadmap.md) track all delivery milestones,
including separate conditional research and later market contracts. Every
issue also stays on the required OpenAgents project 19.

The project has dedicated views for today's deployable MVP, the next Everglade
pass, every subsequent issue, paid cloud, and conditional research. Native
GitHub dependencies are completion blockers; related integrations remain
nonblocking. Existing studio intents deliver the next workshop pass; the
later studio router adapter and optional public API do not gate it. Resource
identity contracts precede product adapters, while host arbitration, owned
remote-task admission, and paid retail contracts can advance independently
of graphical attachment.
