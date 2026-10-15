# Agents and remote-work gaps

Status and priority use [scope and evidence](scope-and-evidence.md). OpenAgents
already has Coder, multiple engines, capacity-aware selection, host authority,
encrypted observation, durable task input, and paired devices. The primary gap
is exposing a complete local/remote coding workspace through the desktop,
along with additional providers and native provider-session behavior.

## Provider coverage

Sources: Z03, Z11–Z12; OpenAgents O04–O05, O11, O14.

Zeron's current `HarnessId` has nine production providers and a mock. The table
below concerns the OpenAgents desktop workflow, not whether an SDK, experiment,
CLI, or model with a similar name exists somewhere in this repository.

| Provider | Zeron adapter | OpenAgents desktop baseline | Parity requirement |
| --- | --- | --- | --- |
| Claude Code | Persistent streaming subprocess/control protocol | Supported through Coder; saved-history reader | Expose catalog/options, accounts, provider-specific commands, native resume, and subagent/session details. |
| Codex | App-server JSON-RPC, with realtime support | Supported through Coder; saved-history reader | Expose catalog/options, native thread resume, image-generation lifecycle, context occupancy, and voice controls. |
| Cursor | Pinned SDK shim | No equivalent exposed Coder choice established | Add a supported adapter and complete desktop install/login/model/input/continuation workflow. |
| Devin | ACP | Supported Coder engine | Validate direct selection, live catalogs, typed input, steering limits, commands, and child-session presentation. |
| Grok Build | ACP | Supported Coder engine | Same complete desktop capability contract; preserve Coder's existing failover behavior. |
| Hermes | ACP | No equivalent exposed Coder choice established | Add adapter, installation/auth readiness, catalogs, events, and session continuation. |
| Pi | Native JSONL RPC | No equivalent exposed Coder choice established | Add native protocol support, model/command discovery, steering, and MCP integration. |
| OpenCode | Native HTTP/SSE server | Supported Coder engine | Validate model/provider catalog, commands, planning, context usage, persistent session lifecycle, and child events. |
| Antigravity | ACP, managed adapter archive | No equivalent exposed Coder choice established | Add verified installation/discovery, authentication readiness, model/options, lifecycle, and cancellation. |

Provider names alone are not parity. Each installed adapter needs a capability
record covering model discovery, options, steering mode, file input, questions,
approvals, skills/commands, session resume, context measurement, and children.
Unsupported capabilities should be unavailable in UI rather than emulated
silently. Native SDK/CLI support must be checked during implementation against
the pinned supported version; this analysis made no live provider calls.

## Agent lifecycle, models, and accounts

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| RUN-01 | P / P1 | Nine selectable production harnesses expose normalized events and capabilities. | OpenAgents' documented desktop Coder path supports five engines, with different orchestration semantics. Add missing exposed providers and unify the desktop capability view. A mock adapter is test infrastructure, not a tenth product provider. |
| RUN-02 | P / P1 | Provider settings discover installation and enablement per device, with install/cancel/progress paths and actionable readiness. | OpenAgents has engine toggles/readiness and Claude setup. Add the complete catalog and missing installers, preserve source ownership/package manager identity, and support cancellation/failure without leaving an enabled unusable provider. |
| RUN-03 | M / P2 | Per-agent updates expose installed/latest versions, notify/auto-when-idle/off policy, progress, manual paths, and failure recovery. | App/host updates are not provider CLI updates. Add device-owned update state and an execution gate; never replace a running provider. Unknown install ownership must produce manual instructions, not a guessed package-manager mutation. |
| RUN-04 | P / P1 | Models refresh from provider-native discovery with reasoning and option catalogs. | Existing Coder selection/defaults do not give the desktop the full interactive catalog. Add bounded refresh, cache/failure state, unsupported-option handling, and selection preservation. Distinguish an unreachable catalog from a provider with no models. |
| RUN-05 | P / P1 | A chat retains harness/native session identity and applies appropriate configuration to subsequent turns. | Coder continuation exists, but model/provider choices and native-provider resume are not equivalent. Specify when settings can change, how forks copy context, and which identity resumes. A failed resume must not silently start a context-free session. |
| RUN-06 | P / P1 | Provider events normalize text, reasoning, tools, plans, inputs, errors, context occupancy, children, and completion. | OpenAgents has ATIF and Coder events. Map additional provider events without losing request/tool/child identity, duplicating initialization, or interpreting billing as context occupancy. Keep provider-specific limits explicit. |
| RUN-07 | P / P1 | Native steering support and turn-boundary behavior drive available actions. | OpenAgents supports queueing and explicit stop-and-send, with early-turn steering. Verify actual capabilities per adapter; do not advertise mid-turn steering when the implementation interrupts or queues instead. Failed steering preserves the message for deliberate retry. |
| RUN-08 | M / P1 | Multiple agent-account slots can be added, activated, and forgotten on the selected engine device. | OpenAgents' BYOK provider keys and engine sign-in detection are different from multiple CLI account slots. Add an explicit account-management boundary, preserve the provider's supported credential mechanism, and avoid exposing credential bytes to the window. |
| RUN-09 | P / P1 | Account cards show email/plan, active identity, rate-limit windows, reset times, and warning/error states. | OpenAgents has capacity/usage signals and run explanations. Add per-account/device UI and distinguish quota estimates, stale data, and unknown state. Show identity and usage for the selected host, not always the viewer. |
| RUN-10 | P / P1 | Login supports provider-specific paste-code/browser polling, cancellation, errors, and device-scoped state. | Existing sign-in/setup paths cover part of this. Complete the provider matrix, keep remote callbacks with the correct host, and ensure cancel or account switch invalidates pending completion. Never log or display raw tokens. |
| RUN-11 | D / P1 | A local coding workspace can exist before service sign-in and can be explicitly imported into a synced profile later. | OpenAgents' hosted-chat entry and local Coder are separate paths. Decide how desktop local-only sessions start without hosted routing. If supported, make import resumable/idempotent and avoid silently merging one person's local records into another account. |
| RUN-12 | P / P2 | Local/synced/development profiles isolate workspace stores, uploads, browser identity, and remembered settings. | OpenAgents separates development keychain entries and has account/host boundaries. Audit every new editor cache, draft, account slot, browser store, and provider default for profile isolation. Signing out must cancel old background work before another identity begins. |
| RUN-13 | M / P2 | Thread naming can choose an agent/model with an explicit provider catalog and retained preference. | Automatic chat titles and manual rename do not provide this control. Add optional naming-provider settings and an asynchronous title lifecycle; failed naming leaves the chat usable, and later generated names must not overwrite a manual rename. |
| RUN-14 | P / P1 | Workspace MCP tools list devices/projects/providers/chats, create or message chats, wait for turns, interrupt, answer input, and publish visuals. | OpenAgents already has tools, plugins, and delegation, but no equivalent complete desktop workspace-control contract was established. Map and expose the missing operations through existing authority; preserve origin identity and prevent accidental self-message loops. |
| RUN-15 | P / P0 | Run journals, ownership, cancellation, restart recovery, and readiness barriers protect persistent sessions. | OpenAgents already has durable tasks, traces, and resident hosts. Verify newly added direct-provider sessions use those guarantees. Window closure, host restart, provider EOF, and lost replies must have defined resumable/failed/stopped outcomes. |
| RUN-16 | V / P1 | Provider protocol regressions are exercised with fake processes, model fixtures, lifecycle failures, and opt-in real probes. | Expand OpenAgents' adapter tests only for the changed providers/consumers. Require fixture coverage for quiet/stalled processes, malformed events, canceled inputs, subprocess reaping, and wrong-session child events before enabling a new provider. |

### Preserve the distinction between three kinds of credentials

OpenAgents service sign-in, a person's BYOK model keys, and an installed coding
provider's account are separate concepts. Zeron's account-swapping internals
should not be copied into the desktop. An OpenAgents design must choose a
supported provider mechanism under a host-owned credential service and make
active-account effects clear before a person switches one.

Similarly, a model name in a run card tells the person what ran; it does not
provide pre-send model selection. A provider installed on the viewer says
nothing about readiness on a remote execution host.

## Desktop remote workspace and synchronization

Sources: Z02, Z04, Z12–Z15, Z17–Z18; OpenAgents O03, O05, O11, O14.

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| REM-01 | P / P0 | The same desktop UI targets local and registered remote engine devices. | OpenAgents already enrolls/controls hosts through grants, but the main desktop Coder path emphasizes this computer. Add an explicit execution-target state used by all workspace surfaces. Revocation/offline changes invalidate actions immediately. |
| REM-02 | P / P1 | Projects and sessions across devices are browsable together, with remote identity and freshness visible. | Paired-computer and hosted-chat lists are partial coverage. Integrate remote host conversations into desktop navigation with deduplication, seen state, project filtering, and cached/offline labels. Opening a remote chat never starts a new run. |
| REM-03 | P / P1 | Remote files, search, branches, worktrees, actions, account settings, and terminals use owner-routed operations. | Existing task control/history does not authorize all of these. Add capability-specific routes and desktop clients as each feature lands. Never treat an arbitrary remote path or method name as an authorized request. |
| REM-04 | P / P0 | Commands survive sender suspension, host cold-open, reconnect, and lost acknowledgments with a stable identity. | OpenAgents already has outboxes and task-owner/request checks. Validate the desktop's new session/create/queue paths against those guarantees. Show pending adoption distinctly from an actively running model; retries must not create two sessions. |
| REM-05 | P / P1 | Connectivity separates edge reachability, device reachability, chat synchronization, command delivery, and file-transfer progress. | OpenAgents has notices and host status, without the complete integrated workspace feedback. Add truthful actionable states and user-driven retry. A connected socket alone must not imply a host adopted the current send. |
| REM-06 | P / P0 | Locally authored transcript updates survive snapshot debounce, eviction, shutdown, and reconnect. | OpenAgents uses a different encrypted event/history architecture. Prove equivalent durability for new remote-session streams rather than introducing Loro for appearance's sake. Kill the sender/host at publication boundaries and verify complete, deduplicated replies after recovery. |
| REM-07 | P / P1 | Local/synced profile migration preserves conversation identities, uploads, and native-session context with retry-safe import. | No equivalent desktop import flow was established. If RUN-11 introduces local-only sessions, add explicit scope selection, progress, per-account completion markers, partial-retry behavior, and read access to previously imported media. |
| REM-08 | P / P1 | Devices can be renamed/managed and project catalogs remain owner-authored; older peers expose only supported capabilities. | OpenAgents Computers and grants already cover substantial management. Extend desktop workspace integration without replacing enrollment. Validate version/capability negotiation for every added RPC, including read-only fallback and explicit unsupported actions. |
| REM-09 | V / P1 | Slow links preserve control responsiveness, bounded queues, resumable replay, and cancellation while large data transfers. | Comparable OpenAgents behavior needs measurement for remote files/media/terminals, not just text chat. Test throttled and half-open links, held acknowledgments, queue saturation, and interrupted chunks with strict memory/deadline bounds. |
| REM-10 | P / P0 | Account/runtime teardown cancels old watches, caches, requests, and peer resources before reuse. | Extend existing host/profile isolation to every new surface and background job. A late old-account result must not render, save a file, resume audio, post a comment, or change the next account's defaults. |
| REM-11 | P / P1 | Remote child conversations and MCP-originated work remain visible in the same workspace. | Existing delegated Coder work is partial coverage. Connect user-created side chats, agent-created chats, parent/child identity, and unread/attention state; enforce both host authority and the operation's originating conversation. |
| REM-12 | V / P1 | Delivery, durable publication, and reconnect behavior have repeatable isolated two-engine regressions. | Add an OpenAgents local/remote fixture matrix for the new desktop workflows. It must exercise host acceptance, sender exit, stale cache, revocation, reconnect, and complete final content without touching the owner's production account or computers. |

### Behavioral parity does not require Zeron's synchronization design

Zeron now has durable local update publication and opaque relay paths beyond its
old architecture plan. Its documented defects and regression tests are useful
failure scenarios. They do not establish that OpenAgents has those same bugs.
OpenAgents should retain its existing signed/encrypted task and history design
and demonstrate equivalent user outcomes with its own failure injection.

Zeron's service workspace uses authenticated user/organization identity, while
OpenAgents pairs hosts and grants explicit rights. Replacing those grants with
“the user is signed in” would lose an existing boundary. Add file editing,
terminal control, preview access, and credential management under explicit
rights; read-only observation stays read-only.

## Terminal parity

Sources: Z10, Z12–Z13; OpenAgents O09, O14.

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| TERM-01 | P / P1 | Terminal sessions belong to a chat and its owning host/checkout. | OpenAgents' current pane runs a window-local shell starting in home. Add chat/checkout-scoped launch and explicit host identity. Changing the selected chat must not retarget an already-open shell. |
| TERM-02 | M / P1 | Multiple terminal tabs support create, select, close, middle-click close, and drag reorder. | The exposed OpenAgents pane offers show/hide/restart plus agent controls, without this terminal tab set. Add stable terminal IDs and per-chat order; closing one tab must not kill another terminal or archive the chat. |
| TERM-03 | P / P1 | A terminal can appear in the bottom drawer or a right surface, coordinated with composer height and fit. | OpenAgents has a side terminal pane. Add the missing dock/surface layouts, resize bounds, shortcut routing, and session-scoped visibility; avoid covering Send or stealing text input when a panel opens. |
| TERM-04 | P / P0 | Host-owned PTYs survive detach; reconnect replays bounded sequence-numbered output and then follows live data. | The window-local pane keeps its shell while hidden but closes it with the window. Reuse `coder-pty` host APIs for persistent/remote terminals. Distinguish hide/detach/close/exit and show replay gaps truthfully. |
| TERM-05 | P / P1 | Remote terminal write/resize/close retain the owner; rights are rechecked at the host. | OpenAgents already has NIP-TERM infrastructure, but the desktop pane is in-process/local. Wire the desktop to the granted host path. Revocation and stale terminal references must fail without touching any viewer-local process. |
| TERM-06 | V / P1 | ANSI/VT behavior, alternate screen, paste, selection/copy, scrollback, mouse input, exit state, and theme palette support common TUIs. | `coder-vt` deliberately implements a subset, while Zeron uses `alacritty_terminal`. Compare a concrete sequence/TUI corpus and fill observed gaps. Do not replace the emulator merely because the libraries differ. |
| TERM-07 | P / P2 | Terminal font/family/size and full palette follow dedicated settings and the selected theme. | OpenAgents uses a fixed terminal font size and drawn palette. Add independent settings, fallback validation, Unicode/cell-width checks, and resize propagation. Code/chat text preferences must not unexpectedly change terminal geometry. |
| TERM-08 | P / P1 | Project Action terminals have replay from startup, reliable command submission, and setup handoff. | The existing shell is not a named action runner. Integrate WRK-17–19 with terminal identity and launch acknowledgment; long/multiline commands must not truncate at the interactive shell's input buffer. |

The existing OpenAgents Agents board is additional value: it lists background
agents with status, time, tokens, cost, stop, and usage-limit continuation
actions. Preserve it while adding terminals. It is not equivalent to Zeron's
per-conversation subagent transcripts, so both NAV-12 and TERM work remain.
