# T3 Code and OpenAgents: gap analysis

OpenAgents has much of the infrastructure needed to compete with T3 Code, but
it does not yet provide the same complete coding workflow. The largest gaps are
sending attachments to an agent, reviewing and accepting repository changes,
managing native provider sessions, and recovering work consistently across
devices and restarts. Adding more desktop chrome would address less of the gap
than completing those workflows.

T3 Code is a useful reference for a coding-agent control surface. OpenAgents has
a different product scope: chat, Coder, measured capability improvement through
the Gym, Verse, and separately authorized payments. Preserve that scope and
the Rust implementation. Adopt the workflow semantics that make coding work
legible and recoverable; do not duplicate T3's TypeScript implementation or
replace Microcoder to achieve visual parity.

## Scope and evidence

| Field | Snapshot |
| --- | --- |
| Analysis date | 2026-09-30 |
| T3 Code checkout | `/Users/christopherdavid/work/projects/repos/t3code` |
| T3 Code revision | `6b286ae8a20ef2e18fa70b09e92f66edd859128b` |
| T3 Code upstream recorded by the checkout | `https://github.com/pingdotgg/t3code` |
| OpenAgents checkout | `/Users/christopherdavid/work/openagents` |
| OpenAgents revision | `ad40a50befd9068f290d9899e343bac928b27cf0`; original inventory at `f65137ae48fb0782fc37ca6c6956959a0abbe33f`, followed by review of the intervening palette/titlebar change |
| Working trees at inspection | Both clean; OpenAgents `HEAD` equals fetched `origin/main` |
| Method | Local documentation and source inspection, including contracts, state owners, adapters, persistence, and selected regression tests; review of retained OpenAgents verification records |
| Runtime verification in this analysis | None. No provider calls, application launches, owner-host probes, installs, or deployments |

This is an implementation comparison at those revisions, not a claim about
current public releases or observed performance equivalence. T3 links below
refer to its pinned revision. OpenAgents links resolve into this repository;
use the baseline commit when reproducing the analysis after later changes.
Tests found in source establish intended coverage, not a fresh passing result.
OpenAgents verification records describe earlier runs and retain their stated
limitations.

Navigation: [main findings](#main-findings),
[architecture](#architecture-and-ownership),
[capability matrix](#capability-matrix), [detailed gaps](#detailed-gaps),
[OpenAgents strengths](#what-openagents-should-retain-and-expose),
[implementation sequence](#recommended-implementation-sequence), and
[acceptance scenarios](#end-to-end-acceptance-scenarios).

The comparison covers the overlap between T3's coding control surface and
OpenAgents' desktop, phones, host, and Coder runtime. It also identifies
OpenAgents capabilities outside that overlap. It does not audit every decision
model, game subsystem, relay NIP, dependency, or security boundary.

Two stale-document traps matter:

- The [September 29 Zeron audit](2026-09-29-comet-desktop-ui-port-audit.md)
  describes substantial desktop work as pending. At this baseline, real chat,
  a composer, retained sessions, task controls, images, menus, notifications,
  settings, accessibility, and a read-only diff pane have implementations and
  verification records. The opening of the
  [desktop README](../../crates/openagents-desktop/README.md) still describes
  sample chats and no composer. Current
  [desktop chat source](../../crates/openagents-desktop/src/chat.rs) and the
  [shared application](../../crates/openagents-chat-app/README.md) take precedence
  for this analysis.
- T3's [root README][t3-readme] links store apps, while its
  [mobile README][t3-mobile] says mobile is not distributed. Its
  [source-control guide][t3-source-control] also explicitly limits the pull
  request **Code** tab to web and desktop. Native mobile review-diff modules
  alone do not establish mobile pull-request review parity or release status.

Gap labels used below:

| Label | Meaning |
| --- | --- |
| Missing | No equivalent product path found in the inspected implementation |
| Partial | A working subset exists, with a material behavior absent |
| Integration | The owning implementation exists, but a particular product surface does not expose the complete flow |
| Verification | Code exists; retained evidence does not establish the target device, release, or live-provider behavior |
| Deliberate difference | The products make different choices; parity needs a product decision |
| OpenAgents advantage | OpenAgents implements a relevant capability beyond T3's inspected scope |

## Main findings

1. **The attachment gap blocks an ordinary coding task.** OpenAgents can pick,
   decode, and preview PNG and JPEG drafts, but its current hosted conversation
   route accepts text only. The application preserves the draft and refuses
   Send. T3 uploads files to the workspace-owning environment and passes
   environment-local paths or native inputs to provider adapters. The gap is
   the authorized artifact and provider-input path, not the attach button.
2. **The repository review gap blocks completion.** OpenAgents runs work in
   isolated worktrees and shows a final unified diff. T3 connects that work to
   branches, staging, commits, publishing, pull requests, review comments,
   checks, and merge. OpenAgents has issue and project automation, but the
   desktop chat does not offer an equivalent review workflow.
3. **Native session fidelity is a distinct gap.** Reading saved Codex or Claude
   history and creating a new Coder task from bounded context does not resume
   that native conversation. T3 explicitly manages provider sessions,
   interruption, questions, compaction, and rollback. OpenAgents already has
   ACP session support for specific engines; a common, advertised product
   contract remains incomplete.
4. **OpenAgents has strong recovery components, with uneven application
   integration.** Exact task retries, durable grants, task journals, encrypted
   chat records, and source-bound history cursors exist. T3 additionally makes
   shell/detail synchronization, native resume, offline drafts, ordering, and
   independent client/server versions part of one control surface.
5. **OpenAgents' broader infrastructure is valuable but does not close those
   gaps automatically.** Decision models, ATIF, independent verification, Gym
   evals, signed grants, Verse, and wallet approval are differentiators. Their
   presence does not supply attachment delivery, native session rollback, or
   a repository review UI.

## Architecture and ownership

### T3 Code

T3's [architecture][t3-overview] and [provider interface][t3-provider-adapter]
divide the system into four layers:

| Layer | Responsibilities |
| --- | --- |
| Environment server | Provider processes and credentials, workspace files, Git, PTYs, authentication, and durable orchestration state |
| Orchestration | Commands, a decider, events, persisted projections, command receipts, and reactors for external effects |
| Shared client runtime | Authorization, environment identity, connection supervision, subscriptions, cached domain state, operations, and voice-input lifecycle |
| Platform clients | Web presentation, Electron desktop services, React Native mobile presentation, and native input/rendering integrations |

The [orchestration engine][t3-engine] serializes commands. Accepted events,
persisted projections, and a command receipt commit in one database
transaction. Only afterward does the engine replace its in-memory state and
publish events. Reactors perform provider and filesystem work after intent is
recorded. An acknowledgment means committed intent, not a completed side
effect.

The server owns the environment regardless of route. Direct access, Tailscale,
SSH, and T3 Connect reach that same owner. The hosted web app is a client that
connects to environments; it is not the coding executor. Provider integrations
normalize native behavior behind an adapter. A provider *instance* includes
its configuration and account lifecycle, so two accounts on one driver do not
share mutable session state. See [remote architecture][t3-remote],
[connection ownership][t3-connections], and [provider constraints][t3-providers].

### OpenAgents

OpenAgents already has corresponding ownership boundaries, spread across more
independent domains:

| Layer | Current owners and evidence |
| --- | --- |
| Runtime and effects | [Coder task owner](../coder/runtime/task-owner.md), [Microcoder repository adapter](../coder/runtime/microcoder-repository.md), `coder-boundary`, and `supervise` |
| Host authority | [Resident host](../../crates/coder-host/README.md), `coder-access`, `coder-reach`, and `coder-pty` |
| Connection and computer state | [Live Computers service](../../crates/coder-computers/src/live.rs), `coder-link`, and host connectors |
| Chat core | [OpenAgents chat](../../crates/openagents-chat/README.md): hosted transport, lifecycle, and encrypted records |
| Shared chat presentation | [OpenAgents chat application](../../crates/openagents-chat-app/README.md): lists, projections, task controls, retained history, cards, and client sessions |
| Platform clients | `openagents-desktop`, the separate `openagents-mobile` workspace, Rust Native, and thin SwiftUI/Kotlin adapters |
| Evidence and improvement | ATIF, Gym, extension evaluation, published results, and XP |

Desktop chat now has two coding paths. An admitted remote or brokered task uses
the resident host and its policy. A coding request originating in the desktop
can also call the shared local runner, which creates Coder's worktree and uses
the local capability settings. Reopening a chat does not start another run.
This newer local path is documented in
[desktop Coder access](../desktop/local-coder.md) and implemented in
[the local runner](../../crates/coder/src/task/local.rs) and
[desktop worker](../../crates/openagents-desktop/src/worker.rs). Therefore,
“desktop cannot run Coder without phone pairing” is no longer a valid gap.

The architectural opportunity is to make the existing owners compose into a
complete workflow. A new monolithic server, another agent loop, or a second
desktop-only chat model would duplicate working machinery.

### Concepts that must remain distinct

| T3 concept | Closest OpenAgents concepts | Constraint on the mapping |
| --- | --- | --- |
| Environment | Resident host, reach identity, and execution environment | A reachable host is not proof of execution admission or model capacity. |
| Project | Configured project, workspace grant, and local checkout | A display group does not grant access to every clone or directory. |
| Thread | OpenAgents conversation, Coder task, and native session lineage | These are separate identities; a chat can bind a task without becoming its native provider session. |
| Provider instance | Provider route, credential source, and native adapter | A provider name alone cannot identify an account, credential profile, or session owner. |
| Checkpoint | Source snapshots, retained candidate artifacts, and the designed WS checkpoint contract | A digest comparison establishes what changed; it does not restore files or rewind a provider. |
| Command receipt | Task disposition, exact retry record, and control receipt | An accepted request does not prove execution, independent verification, merge, or payment. |
| Subscription projection | Activity summaries, ATIF pages, chat snapshots, and source cursors | A healthy socket does not prove every view is current or that a snapshot and delta form one consistent cut. |

The [glossary](../glossary.md), [SESS](../../nips/openagents/NIP-SESS.md), and
[WS](../../nips/openagents/NIP-WS.md) already express much of this distinction.
Their broad designed contracts must not be counted as completed product roles.

## Capability matrix

Priorities describe the recommended OpenAgents coding workflow: **P0** blocks
common tasks or trustworthy completion, **P1** addresses repeated friction or
recovery, and **P2** is optional parity. They are not existing issue priorities.
The detailed sections supply the implementation evidence and acceptance scope.

| Capability | T3 Code at the snapshot | OpenAgents at the snapshot | Gap and priority |
| --- | --- | --- | --- |
| Real desktop chat and composition | Implemented | Implemented with shared state and native editing | Existing capability; do not reopen the shell/composer work |
| Coding from local chat | Starts an environment-owned provider thread | Starts shared local Coder runs in isolated worktrees | Deliberate engine difference |
| Sending images and files | Upload lifecycle and adapter-specific inputs | Bounded PNG/JPEG previews; text-only route refuses image send | Partial, P0 |
| Structured context | File, terminal, review, citation, preview, and media references | Text prompts, bounded retained-session context, and local image drafts | Partial, P1 |
| Workspace and artifact inspection | File read/write/search RPCs and format-specific viewers | Transcript, task artifacts, and diff readers; no equivalent general chat file surface found | Partial integration, P1; full editor P2 |
| Native Codex/Claude session lifecycle | Explicit native adapter operations | Retained history and new Coder continuation; other delegation paths exist | Partial, P1 if native harness control is a product goal |
| Provider breadth | Codex, Claude, Cursor, Grok, OpenCode, Antigravity | Codex and Claude generation; Devin, OpenCode, and Grok ACP routes; cloud fallback on specified paths | Different coverage, P1/P2 |
| Multiple accounts per provider | Instance and profile isolation | Route/capacity settings; no equivalent account manager found in app | Missing app flow, P2 |
| Questions, approvals, Stop, and steering | Normalized native interactions | Task question/approval inputs, Stop, queue, and phase-specific steering | Partial parity; preserve narrower authority semantics |
| Conversation compaction and model switching | Capability-aware adapter paths | Settings select routes/models; no equivalent generic native-session controls found in chat | Partial, P1/P2 |
| Conversation and file rewind | Checkpoints plus native rollback, with worktree guards | Source snapshots and final diff; no equivalent coordinated chat action found | Missing product path, P1 |
| Worktree isolation | New/shared worktree flows and branch controls | Local runner and task-owner isolation | Existing infrastructure; project/branch UI integration, P1 |
| Repository import | Clone/publish workflows with progress and cancellation | Local folder/project selection | Missing comparable app flow, P1 |
| Reviewing a task's diff | Turn/thread diffs and review context | Read-only final **What changed** pane | Partial, P0 |
| Staging, commit, push, and review publication | Integrated Git workflow | Issue/project automation and agent commands; no equivalent desktop review controls found | Integration/new app workflow, P0 |
| Pull-request review | Several forges, comments, checks, reviewers, merge, stacks | GitHub issue/project automation; no equivalent app review center found | Missing app flow, P1; all-forge parity P2 |
| Thread lifecycle | Pin, order, settle, snooze, archive, undo, and PR links | Pin, rename, archive/restore, project grouping, and title/project search | Partial, P1 |
| Message search | Searches user and final assistant messages across environments | Search uses loaded summary titles and project labels | Missing body-search path, P1 |
| Offline drafts and queued messages | Mobile persistence and resumed uploads | Durable host task queue; local-run queue and image drafts have different lifetimes | Partial, P0/P1 |
| View synchronization | Separate shell/detail streams, cursors, and cached state | Connection registry, activity watches, paged ATIF, and polling chat snapshots | Partial integration and scale work, P1 |
| Browser coding client | Local/hosted web controller | Rust website and local read-only task browser | Deliberate difference or missing client, P2 |
| Interactive terminals | Environment PTYs and retained output; platform renderers | Real NIP-TERM PTYs and VT emulator; Computers terminal surface | Existing infrastructure; desktop chat integration/history persistence, P1/P2 |
| Preview browser and screen capture | Browser profiles, preview context, and desktop SnapShots | No equivalent integrated coding-chat path found | Missing optional workflow, P2 |
| Voice input | iPhone transcription lifecycle and local draft insertion | No corresponding application-owned recording/transcription flow found in inspected chat surfaces | Missing, P2 |
| Usage | Cross-provider history, cost estimates, pooled subscription accounts | Route capacity, usage probes, status ring, ATIF usage, and Gym records | Partial product aggregation, P1/P2 |
| Notifications | Mobile push, ongoing activity, and deep links | Desktop notices; push infrastructure and wallet wakes; live phone delivery remains constrained | Partial integration/verification, P1 |
| Settings and updates | Client/environment/project settings and independent version capabilities | Persisted app/Coder settings, signed desktop updates, host service/SSH lifecycle | Partial scope/compatibility integration, P1 |
| Native accessibility | Context chips have documented screen-reader labels | AccessKit tree, native fields, shared editor, and phone contracts | Existing implementation; device evidence, P1 |
| Operational diagnostics | Local spans, optional OTLP export, and demand-driven process telemetry | ATIF, task journals, playtest records, and foreground timing artifacts | Strong evidence foundations; unified diagnostic flow is partial, P1 |
| Capability evaluation and improvement | Provider skills and harness integrations | Gym, controlled with/without extension evals, published evidence, and XP | OpenAgents advantage |
| Independent effect authority | Scoped environment RPC and native provider permissions | Signed host grants, execution admission, filesystem boundary, and separate spending approvals | OpenAgents has additional explicit boundaries; defaults differ |
| Shared world and wallet | Outside the inspected coding product | Verse and Rust-owned phone wallet/approval flows | OpenAgents scope beyond parity |

## Detailed gaps

### 1. Attachments and context need an end-to-end artifact path

T3's [composer guide][t3-composer] describes uploads before send, failed-upload
retry/removal, image conversion, file previews, large-paste attachments, and
environment-local files. Its [provider constraints][t3-providers] place
attachments outside the project and let adapters choose native input formats.
A file path in a prompt does not bypass provider filesystem permissions.

OpenAgents' [attachment implementation](../../crates/openagents-chat-app/src/attachments.rs)
limits a draft to four PNG/JPEG images, 8 MiB each, 4096 × 4096 pixels, and
16 MiB across conversation drafts. It creates bounded previews on a worker.
Those bytes remain in memory and are neither saved nor uploaded. The
[image verification](../desktop/verification/2026-09-30-images/verification.md)
records a visible text-only refusal; the
[phone contract verification](../desktop/verification/2026-09-30-phone-contracts/verification.md)
confirms native picking and previewing, not successful provider ingestion.

The resulting user failure is concrete: “Fix the layout in this screenshot”
can produce a preview but cannot reach the agent through this route. Raising
limits or accepting more file extensions does not fix it.

Implement a recipient-bound artifact path with a stable digest, size/MIME
bounds, retention, cancellation, and a provider capability check. Resolve a
local attachment on the selected execution host; do not silently send it to
the hosted chat worker or another computer. Either extend the admitted
conversation contract for media or route the request explicitly to an
artifact-capable Coder path. Keep unsupported-route refusal and draft
preservation.

**Acceptance:** attach a generated screenshot, restart the client, reconnect,
send to a scratch host, and establish that the intended adapter receives the
exact bytes. A refused route, failed upload, host switch, or lost acknowledgment
must preserve the draft and avoid duplicate submissions. Add file/terminal/diff
context references afterward, with source identity and revisions rather than
unqualified paths.

Artifact inspection belongs to the same flow. T3's [RPC contract][t3-rpc]
exposes workspace file read/write, entry search, and content search; its
composer documents code, Markdown, HTML, tabular, audio, image, and PDF
previews with platform-specific limits. OpenAgents can start with a bounded
read-only viewer for retained outputs and linked workspace files. An editable
file surface is a separate mutation contract: pin the host, canonical resource,
version, and write authority. Do not add a general editor as a prerequisite
for seeing the report or file a task produced.

### 2. Review must connect the diff to a disposition of the work

T3's [source-control guide][t3-source-control], [Git workflow service][t3-git],
and [pull-request service][t3-pr-service] span repository lookup, clone,
publish, commit/push, generated messages, review details, comments, checks,
reviewers, and merge. Linked reviews belong to threads, including multiple
links and stacks. Viewed-file state can become stale after a push; T3
documents the difference between “deleted” and “could not read this revision”
in its [file-revision contract][t3-pr-revisions].

OpenAgents' [What changed implementation](../../crates/openagents-chat-app/src/changes.rs)
and [verification](../desktop/verification/2026-09-30-what-changed/verification.md)
provide a useful foundation: a bounded, read-only final unified diff, visible
line painting, and syntax colors. This is already implemented and should be
extended. It is not a live Git index or a pull-request review.

The local diff path also has a concrete completeness gap. In
[`task::local::unified_diff`](../../crates/coder/src/task/local.rs), Git output
is collected with `Command::output`, then clipped to `max` and returned as a
plain string. The desktop requests the parser's 1 MiB limit. The parser can
label truncation that occurs inside its own loop, but the upstream string no
longer says that it was clipped; a diff clipped before parsing can therefore
appear complete. The helper also does not preserve a command failure as a
typed outcome. These are source findings, not reproduced runtime failures.
Bound collection while reading the subprocess, and return revision,
completeness, and read-error metadata alongside the diff. A bounded return
value alone does not bound subprocess-output memory.

There is also real Git automation in
[the issue runner](../../crates/coder/src/task/issue_run.rs),
[the delegate issue flow](../../crates/coder-delegate/src/issue.rs), and
[project GitHub integration](../../crates/coder-project/src/github.rs).
Thus the gap is not “OpenAgents cannot commit or make a PR.” It is a common
host-owned repository service and user-facing disposition of an ordinary chat
run: inspect its branch and base, choose what to keep, commit or publish, and
return to the linked task/review.

Start with status, changed files, base/head identities, a clear partial or
truncated diff indicator, and **Open pull request**. Add explicit host-side
stage/commit/push operations with stable request identities. Add GitHub review
actions only after that path is reliable. Multi-forge adapters and stacks are
later coverage; they should not block a complete GitHub workflow.

**Acceptance:** a scratch task changes two files, the reader inspects the exact
revision, a concurrent change invalidates a stale action, and publication
records the commit and review identity. An uncertain push or merge must be
reconciled before another attempt. Reading or opening a diff must not grant
write, merge, or spend authority.

### 3. Native provider sessions need their own support matrix

T3's [adapter contract][t3-provider-adapter] includes start/send, interrupt,
approval and user-input responses, session listing, thread reading, rollback,
optional compaction, and model-switch capabilities. Its
[provider constraints][t3-providers] document concrete differences, including
Codex async questions and Antigravity's inability to rewind conversation
history.

OpenAgents deliberately runs its own Microcoder loop for Codex/Claude
generation. That loop is not the Codex or Claude native harness. The
[retained-session reader](../../crates/openagents-chat-app/src/retained.rs)
imports bounded recent context into a new Coder task;
[its verification](../desktop/verification/2026-09-30-saved-sessions/verification.md)
does not claim native resume. Conversely,
[ACP](../../crates/acp-client/src/session.rs) and the
[repository adapter](../coder/runtime/microcoder-repository.md) already support
specific native-engine stages. Devin, OpenCode, and Grok can reattach through
their supported session paths. Do not flatten these into “no session support.”

| Provider | T3 integration | OpenAgents inspected path | Remaining distinction |
| --- | --- | --- | --- |
| Codex | Native app-server adapter | Codex generation transport, delegation, and retained history | Generation and new-context continuation do not supply the complete app-server lifecycle. |
| Claude Code | Native adapter | Claude generation/delegation and retained history | Existing CLI use does not prove generic native rewind, compaction, or mobile session control. |
| OpenCode | Native adapter, managed per-thread server | ACP repository stage | Publish the supported operations and limits instead of assuming every T3 operation is available. |
| Grok Build | Native adapter | ACP repository stage | Same requirement for explicit operation support and normalized interaction IDs. |
| Cursor | Native adapter | No equivalent app integration found | Optional provider coverage. |
| Antigravity | Managed install, account/profile lifecycle, and native adapter | No equivalent app integration found | Optional provider coverage; Lev is a different on-device decision-model integration. |
| Devin | Outside T3's advertised provider list | ACP repository stage and delegation | OpenAgents coverage beyond that list. |

Expose per-adapter support for native resume, conversation rollback, compaction,
model changes, steering, and pending interactions. Preserve native option IDs
and engine session identities. An unsupported operation needs a clear refusal
or an explicitly labeled new Coder continuation, not an emulation presented as
native resume.

**Acceptance:** protocol fixtures and a scratch-engine integration establish
each advertised operation. A session resumed after restart keeps its lineage
and rejects a stale attachment generation. An unsupported operation does not
mutate files, consume an interaction, or start another engine. Whether to add
full native Codex/Claude harness control is a product decision; Microcoder
remains the existing agent loop.

### 4. Checkpoints are more than snapshots, and rollback is not atomic

T3's [checkpoint store][t3-checkpoints] delegates to the VCS driver. Its
[Git implementation][t3-git-driver] uses a private temporary index and hidden
checkpoint commits/refs. The
[checkpoint reactor][t3-checkpoint-reactor] checks conversation rollback
support before restoring files and refuses file restore when another thread
or provider session shares or overlaps the checkout. T3's composer exposes
conversation-only rewind separately from conversation-and-file rewind.

OpenAgents' [boundary snapshots](../../crates/coder-boundary/src/snapshot.rs)
and retained candidate artifacts establish changes and verification inputs.
The designed [WS checkpoint contract](../../nips/openagents/NIP-WS.md) expresses
restoration requirements. Neither supplies the same current chat rewind flow.
The retained `coder-one` checkpoint module is not evidence that the desktop
chat can rewind its live task and native conversation.

There is an important limit in the reference implementation: T3 restores the
filesystem and then calls native conversation rollback. Those two external
effects are not in the orchestration database transaction. If the later
rollback fails, a durable intent does not make both states atomic. This is a
source-based recovery concern, not a reproduced T3 failure.

If OpenAgents adds rewind, record a recoverable multi-step operation: pin the
workspace owner, source revision, target checkpoint, engine support, and
conversation generation; retain the outcome of each effect. Unknown completion
must stop subsequent work until reconciliation. Refuse restoration for shared
or unresolved ownership. External side effects, such as a published PR, are
outside file rewind and need separate dispositions.

**Acceptance:** exercise dirty files, untracked files, staging, missing refs,
shared worktrees, provider rollback refusal, and process loss between file and
conversation changes. The resulting UI must identify which state changed and
offer a safe recovery path.

### 5. Worktrees exist; project workflows remain narrower

T3's [thread guide][t3-threads] supports new worktrees, shared worktrees,
background starts, and the same prompt sent to several models in separate
worktrees. Its newest commit adds projectless scratch threads. Its
[project settings][t3-project-settings] include submodule handling,
inheritance, safe automatic pull, and conservative managed-worktree cleanup.

OpenAgents' [local runner](../../crates/coder/src/task/local.rs) creates and
tracks its own task worktrees. The host's
[auto-start policy](../coder/runtime/host-autostart.md) admits configured
workspaces and bounded concurrency. These are implemented foundations.
Desktop folder selection and Coder settings do not yet form T3's repository
import, base-branch, workspace-mode, project-action, and cleanup workflow.

For the chat-first product, the immediate requirement is smaller than an IDE:
identify the selected project and base, show where the resulting work lives,
and make recovery or publication possible. Clone/import with progress is a
useful next step. Shared-worktree modes and arbitrary project scripts require
separate authority and ownership rules. Projectless file work also needs a
product decision because local Coder currently expects a Git checkout.

**Acceptance:** create a scratch repository, select a base, start two isolated
runs, and show their distinct worktrees. Removing a chat must retain dirty or
unresolved work. Any cleanup policy must prove management ownership, absence
of active users, and a disposition for retained artifacts before deletion.

### 6. Offline submission has several different lifetimes

T3's [composer behavior][t3-composer] says mobile drafts, local attachment
copies, and queued messages survive disconnection and app restarts. Uploads
resume. Sign-out retains that work until the same account returns. Web and
desktop have different limitations, including uploads interrupted by reload.
This is not uniform offline behavior on every T3 client.

OpenAgents has a real durable host queue with privacy, leases, exact retries,
and typed task commands. The
[task-chat verification](../desktop/verification/2026-09-30-task-chat/verification.md)
covers that path. However,
[the local-run controller](../../crates/openagents-chat-app/src/coder_run.rs)
holds its queue in a `VecDeque`, and image drafts are explicitly in memory.
A host-accepted queued input, an unsent desktop draft, and a local-run queued
follow-up therefore have different persistence guarantees.

Publish one application-level outbox contract that identifies the recipient,
operation bytes/digest, artifact dependencies, account/device binding, and
disposition: local, prepared, accepted, delivered, refused, or unknown. Bind
acknowledgment to the submitted draft revision. Existing host commands remain
the admission path; client persistence does not authorize replaying effects.

**Acceptance:** terminate the client after queueing but before acknowledgment,
restart it, and recover the same pending item. Repeat with a host-accepted
command and a lost reply: there must be one accepted operation. A newer draft,
different host, revoked grant, or different account must not inherit that
submission silently.

### 7. Synchronization must converge across domains

T3's [connection runtime][t3-connections] has one retry owner per environment,
initial-config readiness, separate transport health and data freshness,
shell/detail subscriptions, and a cache that retains state and replay cursor
together. Detail streams are reference-counted; the desktop keeps running
threads subscribed. Mutation retry stays in the operation instead of the
transport supervisor.

OpenAgents already separates transport and freshness in `coder-link`. Contrary
to an older sentence in its README, the
[live Computers service](../../crates/coder-computers/src/live.rs) uses the
registry, pumps connector reports, watches activity, and refreshes host data.
History is bounded and source-bound. These are implemented capabilities.

The remaining gap is broader projection integration. The shared
[chat session](../../crates/openagents-chat-app/src/session.rs) polls snapshots
at 100 ms while busy and 1 second otherwise. The
[chat service](../../crates/openagents-chat/src/service.rs) bounds returned
turns and bytes. This is different from a common shell/detail snapshot-and-delta
protocol with persisted cut/cursor semantics. NIP-WS defines that broader
contract, but its existence is not proof of full integration.

Extend the current connection owner and source cursors. Measure polling,
serialization, transfer, and projection before replacing them. Scope updates
by host, source incarnation, task/session generation, and revision. Define
retention and gap recovery independently for summaries, transcripts, queues,
and repository views.

**Acceptance:** two scratch clients converge after disconnect, host restart,
source replacement, a missed range, and rapid thread switches. Cached data
remains readable and visibly stale; an old response never replaces a newer
view. Reconnection alone never replays a mutation.

### 8. Thread management and search need durable application state

T3's [thread lifecycle][t3-threads] includes server-persisted active ordering,
snooze deadlines, settlement rules, undo, linked reviews, and message search.
Some server rules continue while clients are closed.

OpenAgents' [chat list](../../crates/openagents-chat-app/src/chat_list.rs)
groups pinned/project/recent/archived summaries and searches their titles and
project labels. Rename, pin, archive, and restore already have shared commands
and host persistence. The missing parts are message-body search, manual
ordering, snooze/settle distinctions, review links, and lifecycle rules that
follow the work rather than a window's presentation state.

Prioritize body search and explicit finished/active disposition over drag
animations. A search index must be scoped to admitted content, support
bounded results, and distinguish a current complete index from cached partial
results. Task completion, independent verification, review merge, and archive
are separate facts. A merged review must not hide resumed or still-running
work.

**Acceptance:** find an older message outside the loaded summary window,
return to its exact source, and show unavailable sources honestly. Changes
made on one client survive restart and appear on another. Undo must target
the exact transition and refuse a conflicting later revision.

### 9. Provider setup and usage need account-aware ownership

T3's [provider constraints][t3-providers] and [usage guide][t3-usage] cover
isolated instances, account homes, catalog refresh, installation/update
ownership, cross-provider history, model prices, and deduplicated subscription
accounts. Provider helpers and sign-in can launch hooks or MCP servers, so
background health checks avoid setup side effects.

OpenAgents has capacity failover, optional usage probes, local settings, and
a [read-only engine status strip](../desktop/verification/2026-09-30-engine-status/verification.md).
ATIF records generation usage and attempts. This is already more than a
provider-name badge. It does not provide T3's cross-host/account usage center
or app-managed credential profiles. Read-only status also does not select or
change a credential.

For the first complete coding workflow, expose the served route and reason for
failover, configuration provenance, stale/unknown readings, and limits that
prevent a start. Add historical aggregation only with stable account/source
identity, so the same account or trace is not counted twice. API-equivalent
cost, a provider subscription allowance, a task budget, and wallet spending
must remain separate values.

**Acceptance:** fixtures cover an exhausted route, a later admitted route,
stale usage, duplicate history sources, and unknown cost. No usage read grants
another route or leaks credentials into the renderer. Multiple-account setup
is a later slice requiring native process/profile isolation, not a dropdown
over shared global credentials.

### 10. Permissions are not interchangeable product modes

T3's [permission modes][t3-permissions] map **Supervised**, **Auto-accept
edits**, **Auto**, and **Full access** to provider behavior. Full access is its
initial new-thread default. Its
[environment auth contract][t3-auth] separately controls RPC scopes, pairing,
DPoP, and WebSocket tickets. Projects are organizational boundaries, not
filesystem sandboxes; read scope can disclose files the server account can
read outside a project.

OpenAgents distinguishes enrollment, operation rights, task execution grants,
the operator's auto-start policy, command confinement, and spending approval.
Its bounded repository path protects the task store and common Git directory;
explicit full-access settings change that boundary. Modern phone pairing can
grant all NIP-HOST rights, but those rights do not replace the host's execution
policy or authorize payments. See
[auto-start](../coder/runtime/host-autostart.md),
[task ownership](../coder/runtime/task-owner.md), and
[desktop broker admission](../desktop/local-coder.md).

Do not map T3's mode names directly onto host rights or treat an approval reply
in chat as a new execution grant. If native adapters are added, publish the
actual provider policy and the host boundary together. Keep cloud route,
computer route, engine permissions, device rights, and wallet approval legible
without exposing protocol details to ordinary users.

T3's [RPC scope table][t3-rpc-auth] has a useful structural property: adding an
RPC without selecting a scope is a type error. A closed Rust operation enum
and exhaustive authorization mapping can preserve that property when
OpenAgents adds repository, media, or session operations.

### 11. Notifications exist; mobile activity delivery remains a different slice

T3's [mobile notification guide][t3-notifications] covers finished/failed/input
alerts, thread deep links, Android ongoing activity, and iOS Live Activities.
Background delivery requires T3 Connect; a direct connection alone does not
provide push.

OpenAgents now implements desktop notifications on Linux, macOS, and Windows.
Do not repeat the older settings receipt's statement that only Linux supports
them. The [Windows notification receipt](../desktop/verification/2026-09-30-windows-notifications/verification.md)
records current toast support and its limitation: clicking after the app quits
has no registered COM activation path. The
[push gateway](../deployment/push-gateway.md) and
[phone wallet wake configuration](../../bins/openagents-ios/README.md#push-wakes-for-payment-requests)
are separate infrastructure and flows. They do not prove complete background
task notifications or live status on paired physical phones.

Prioritize “Coder needs you” and completion/failure wakes that open the exact
task. Keep sensitive content in the admitted read after wake; a push should
not become a transcript transport. Continuous live activity and widgets are
optional follow-up work.

**Acceptance:** a scratch task requests input while the client is suspended;
the configured gateway sends the registered wake, the correct task opens, and
revocation prevents further reads. Platform delivery and entitlement checks
must be recorded as device/release evidence, not inferred from protocol tests.

### 12. Terminals have a persistence and integration gap

T3's [terminal runtime][t3-terminal] retains bounded history on the server,
coalesces persistence, and avoids replaying historical terminal queries back
to the current shell. Renderer ownership stays on the platform. Retained
output does not prove a running command survives a server restart; T3's
[update guide][t3-updating] explicitly warns about interrupted terminals.

OpenAgents has real [PTY ownership and replay](../../crates/coder-pty/README.md),
[VT emulation](../../crates/coder-vt/README.md), and a
[Computers terminal](../../crates/coder-computers/src/terminal/session.rs).
Its PTY rings and deduplication state are process-local; host restart reports
terminals as lost. Therefore the gap is retained output across restart and a
terminal integrated into the desktop coding workflow, not “build a PTY.”

If that surface is wanted, reuse NIP-TERM and the VT implementation. Persist
bounded output with sequence/generation markers and an explicit lost-process
state. Restoring output must suppress device-query replies. Durable process
reattachment is a separate requirement and should not be implied by restored
scrollback.

### 13. Web, preview, capture, and voice are optional product expansions

T3 has a browser controller, Electron preview services, cookie import,
context capture, and desktop [SnapShots][t3-snapshot]. Its
[preview manager][t3-preview] and browser-import code are substantial platform
integrations. Its [composer guide][t3-composer] specifies on-device iPhone
transcription and cancellation that preserves the existing draft.

The [OpenAgents web crate](../../crates/openagents-web/README.md) serves the
website, pairing landing page, and local read-only task browser. It is not an
authenticated interactive coding client. Protocol voice primitives, CoderOS
desktop controls, or source-level capture tools elsewhere in the repository
do not establish corresponding chat product flows.

A browser client could expand access without a desktop install, but it is a
separate Rust-compatible delivery decision under the product-language
contract. An embedded preview browser and cookie import add platform state,
credential custody, permissions, and capture lifecycle. Voice adds recording,
transcription, insertion, cancellation, and privacy boundaries. None is a
prerequisite for finishing text-and-artifact coding work.

Prefer an external preview link and explicit screenshot attachment first.
Choose these expansions from user demand after the P0 workflow is complete.
Do not count T3's development `.devcontainer` as a shipped per-task sandbox;
its [runbook][t3-devcontainer] describes the contributors' development setup.

### 14. Updates and capability negotiation must cover persisted history

T3's clients and environments upgrade independently. Its
[architecture compatibility table][t3-overview] demonstrates fallback from
multiple PR links to legacy single-link support, including a downgrade that
must override cached capabilities. The
[provider guide][t3-providers] warns that old clients or servers can fail to
decode newly persisted attachment events. Wire compatibility includes restart
replay, not only the current socket.

OpenAgents has version ranges in reach/host state, typed schemas, signed
desktop update manifests, host update/rollback machinery, and SSH installation.
The [desktop release process](../desktop/release.md) coordinates application
and bundled binaries. These foundations do not establish every cross-version
combination of chat media, repository projections, native sessions, or phone
controls.

Make operation support a host/adapter capability and retain historical
decoders. Separate application version, protocol version, provider version,
and artifact schema. A renderer should hide or explain unsupported operations
instead of assuming the host matches its build. Never deploy an older checkout
over a newer host; retain the existing current-`origin/main` deployment rule.

**Acceptance:** an old client reads a new host's compatible state, a new client
falls back on an old host, unsupported media or commands refuse before side
effects, and a downgraded host restarts against retained history safely or
refuses startup with a recoverable explanation. Updates must reconcile active
work instead of assuming all provider sessions can continue.

### 15. Accessibility and performance need comparable evidence

OpenAgents now has [AccessKit and native accessibility evidence](../desktop/verification/2026-09-30-accessibility/verification.md),
shared grapheme/IME editing, and phone-native contract checks. Accessibility
is not an unimplemented framework prerequisite. Physical-device IME,
VoiceOver, TalkBack, and some platform release runs remain verification work.
Current settings intentionally keep the app dark; T3 theme breadth is an
optional product difference, not a missing engine capability.

OpenAgents' [latency record](../desktop/verification/2026-09-30-chat-latency/verification.md)
identifies concrete foreground fixes: retained pixels, damaged-region upload,
surface revisions, and asynchronous clipboard work. Later receipts measure
large transcript/sidebar fixtures. Those measurements cover CPU work and
submission, not GPU completion or scanout. This analysis ran no corresponding
T3 benchmark, so it makes no “Rust is faster” or “Electron is slower” claim.

Use the same task fixtures, history volume, concurrent runs, disconnects,
screen dimensions, and hardware for a future comparison. Measure initial
history load, input latency, first response, frame time, transfer volume, idle
CPU, peak memory, and recovery time separately. Compare a quiet chat view and
an opt-in Verse view separately, since the background world changes the
workload.

### 16. Documentation drift is an immediate maintenance gap

The source and verification receipts now outpace several overview documents.
This can lead planning to duplicate completed UI work or classify a finished
platform feature as absent. The mismatch also exists inside the T3 checkout,
so documentation volume alone is not a reliable maturity measure.

Reconcile the desktop README, local-Coder overview, older audit completion
tables, and `coder-link` integration notes with current code. Maintain a small
surface-by-operation matrix that separates implemented behavior, supported
platforms, live evidence, and owner steps. Link each state to its owning code
and retained verification artifact. This analysis records the drift but does
not modify those unrelated documents.

### 17. Operational diagnostics should connect the existing evidence

T3's [observability runbook][t3-observability] describes local NDJSON spans,
trace summaries, optional OTLP export, and event-loop stall diagnostics.
Its [resource telemetry design][t3-resource-telemetry] uses an isolated Rust
monitor with bounded history and demand-driven snapshots. It explicitly
distinguishes monitor generations and process identity from reused PIDs.
Thus T3's TypeScript product also uses Rust for a native operational boundary;
implementation language alone is not a useful diagnosis of resource cost.

OpenAgents has ATIF, durable task journals, activity state, verification
receipts, foreground [timing instrumentation](../../crates/rust-native-desktop/src/timing.rs),
and [playtest records](../../crates/playtest/src/lib.rs). These have different
disclosure rules. ATIF can contain task content; the structural playtest log
uses closed codes without message text and leaves the device only in a
previewed report. The currently unset triage key is an explicit limit on
report transmission, not a missing local logging implementation.

The product gap is a coherent diagnostic read: identify whether a stalled
task is waiting on admission, transport, inference, a subprocess, projection,
or rendering, and link the relevant retained evidence. Extend the existing
timing/task owners rather than introducing another task log. Keep content-rich
traces separate from content-free operational counters and explicit report
sharing.

T3's [product telemetry guide][t3-telemetry] separately documents default
PostHog events and an environment-variable opt-out. This is a product/privacy
choice, not an operational feature OpenAgents needs to copy. The useful
comparison is whether a failure can be diagnosed from bounded local artifacts
without transmitting prompts or provider credentials.

**Acceptance:** inject an admission refusal, offline host, silent fake
provider, capped command output, and stale projection. The app identifies the
stage and evidence source without reading unauthorized transcripts. Diagnostic
collection remains bounded, stops when unused, and cannot stall foreground
input. A report preview makes any shared content explicit.

## What OpenAgents should retain and expose

| OpenAgents capability | Evidence and current limit | Product consequence |
| --- | --- | --- |
| Typed decisions and explicit control policy | `jev`, decision routing, and the [caller contract](../decision-models/guides/caller.md); model judgments are not execution permission | Keep policy in code and expose the reason for routing or refusal without treating confidence as authorization. |
| Effect journals and independent verification | [Task owner](../coder/runtime/task-owner.md), retained artifacts, ATIF, and `coder task check`; a model finish is not an independent pass | Connect results to source revisions and checks in the review flow. |
| Measured capability improvement | [Extension evaluation](../extensions/evaluation.md): with/without runs, graders, published reports, hosted checks, and XP; adoption/release evidence remains specific to each record | A coding result can lead to a measured reusable capability rather than ending at a transcript. |
| Portable signed device authority | NIP-HOST grants, revocation, narrowing, and host-side admission | Remote UI actions can stay independent of a particular cloud account or transport. |
| Separate reach and effect ownership | [Live Computers service](../../crates/coder-computers/src/live.rs), direct/relay routes, and SSH host ownership | Route changes need not transfer credentials or create a second executor. |
| Capacity-aware Coder execution | [Auto-start and routing](../coder/runtime/host-autostart.md), recorded refusals, and bounded fallbacks | Show the route that actually served the task and its limits. |
| Rust-owned cross-platform application state | [Shared chat application](../../crates/openagents-chat-app/README.md) and thin native hosts | Add repository/media/session state once and project it onto the existing clients. |
| Verse and separately authorized wallet behavior | [Verse](../verse/README.md) and [wallet/spend design](../breez/spend-protocol.md); game identity and spending authority remain separate from chat | Preserve opt-in world and payment flows without making them setup requirements for coding. |

The inspected T3 tree does not present equivalents to OpenAgents' decision
model serving/training, controlled extension eval publications, XP referee,
shared world, or wallet approval model. That is a scope difference, not proof
that T3 cannot integrate such systems later.

## Reference behaviors to adapt carefully

Three implementation details prevent an uncritical port:

1. **T3 command receipts are not OpenAgents exact-byte retry records.** At
   this snapshot, the [receipt schema][t3-command-receipts] stores command ID,
   aggregate identity, result sequence, status, and error. The engine's early
   retry branch checks aggregate identity; these inspected fields do not bind
   a complete command-body fingerprint. This supports ID-based result reuse,
   but should not replace OpenAgents' stricter changed-bytes refusal. It is a
   source observation, not a claim of a reproduced exploit.
2. **Recorded intent does not make native/filesystem effects atomic.** T3's
   checkpoint reactor restores files before rolling back the provider.
   OpenAgents should preserve explicit uncertain-effect recovery when
   implementing the same user-facing action.
3. **T3 Connect and OpenAgents relays have different trust roles.** T3's
   [Connect design][t3-connect] gives its relay signing authority to request
   environment bootstrap credentials, bound to a client DPoP key. Its managed
   tunnel carries application traffic to the environment. OpenAgents' signed
   grants and encrypted relay artifacts are a different authorization and
   transport model. DPoP, tunnel links, NIP-42 authentication, and NIP-44
   encryption are not interchangeable capabilities.

Other useful patterns to reimplement are a pure decider, effect workers after
durable admission, exhaustive authorization, one connection retry owner,
scope-bound settings, account-isolated helpers, and tests that drain both a
worker's queue and its currently running item. Keep the existing Rust owners
and exact operation contracts. T3 is MIT-licensed at the snapshot, but this
analysis imports no source or assets. Any later substantial reuse needs its
own provenance and attribution review; design adaptations should be
reimplemented here and identified in their commit messages.

## Recommended implementation sequence

These are proposed research backlog slices, not filed issues. Before creating
an issue, check the existing desktop and protocol work so it does not duplicate
a completed slice. No calendar or hour estimate is justified by this source
survey alone; measure the first complete slice and use that pace for later
estimates.

| Slice | Priority | Owning layer | Dependencies | Completion evidence |
| --- | --- | --- | --- | --- |
| G01: reconcile implementation and surface matrix | P1, small maintenance task | Desktop/runtime docs | None | Current README/audit claims match code; links and platform limitations checked. |
| G02: deliver one screenshot to an admitted coding adapter | P0 | Chat core, artifact transport, provider adapter, shared app | Existing image drafts and host admission | Exact-byte media ingestion on a scratch host; refused routes and lost replies retain the draft. |
| G03: show repository revision and review completeness | P0 | Task/local-run result, shared changes model, native projections | Existing worktrees and diff pane | Exact base/head, file counts, truncation/unknown state, and stale-view handling. |
| G04: publish and link a reviewed change | P0 | Host-owned repository operations and shared app | G03 | Commit/push or draft PR has retained operation and review identity; uncertain results reconcile without duplicate effects. |
| G05: persist the client outbox and artifact dependencies | P0/P1 | Shared app and protected platform storage | G02; reuse task retry semantics | Crash/reconnect/account-change matrix proves one accepted request and preserved newer drafts. |
| G06: publish actual adapter operation support | P1 | Existing adapters, host capability projection, shared controls | None; precedes native-session expansion | Unsupported resume/rewind/compaction/model changes refuse before side effects. |
| G07: converge shell, detail, queue, and repository views | P1 | Existing connection owner, source cursors, shared projections | G03/G05; measured polling costs | Two-client reconnect and missed-range fixtures converge, retaining state/cursor together. |
| G08: GitHub review loop | P1 | Reusable repository/forge adapter and shared app | G04 | Open linked PR, read checks/comments, act on the intended revision, and retain disposition. |
| G09: native-session lifecycle, if selected | P1, conditional | Existing ACP/provider adapters and task owner | G06 | Explicit engine resume/interaction matrix with restart and stale-generation evidence. |
| G10: coordinated rewind, if selected | P1, conditional | Workspace checkpoints, native adapter, journal, shared app | G09 and exclusive workspace ownership | Fault injection between restore/rollback produces a truthful recoverable state. |
| G11: message search and durable work disposition | P1 | Admitted host index, chat records, shared app | G07 | Bounded body search and cross-client archive/finish state survive restart without hiding active work. |
| G12: task wakes and platform release evidence | P1 | Existing push gateway, notification adapters, native hosts | Supported task event/read path | Correct suspended-device wake and task navigation; owner-only evidence recorded separately. |
| G13: project import and safe cleanup | P1/P2 | Repository service, task ownership, shared settings | G03/G04 | Clone progress/cancel, explicit base, no cleanup of dirty/shared/unresolved work. |
| G14: historical usage and account-aware setup | P2 | Existing usage/capacity owners and isolated provider instances | G06; stable account/source identities | No double counting, clear unknown cost, no credentials in UI state. |
| G15: web control, preview, capture, voice, and more forges | P2, individually selected | Rust product cores and platform adapters | Complete G02–G08 workflow | Each feature has its own authority, cancellation, retention, platform, and release evidence. |
| G16: stage-specific diagnostics over existing records | P1 | Task owner, connection state, timing collector, and shared app | G07; reuse retained evidence | Injected failures identify their stage; bounded diagnostics respect content disclosure and foreground latency. |

The critical dependency path is **artifact delivery → durable submission →
revision-bound review → publication/review identity**. Adapter fidelity and
projection convergence support that path. Optional native-harness control,
browser embedding, and additional providers can proceed only as selected
product extensions; they need not delay a complete Microcoder coding flow.

## End-to-end acceptance scenarios

Use scratch roots, temporary homes, generated credentials, and fake providers
or explicitly configured test services. Never use the owner's retained chats
or installed hosts to fill evidence gaps. If a later authorized live smoke
creates a task, archive it when finished under the repository contract.

| Scenario | Required outcome | What it establishes |
| --- | --- | --- |
| Screenshot to change | Attach a generated image, select a project/host, deliver exact bytes, run under the existing grant, and inspect the resulting revision | Actual attachment ingestion and coding flow, beyond a preview |
| Review to publication | Inspect two changed files, reconcile a stale revision, publish once, and return from the review to the same conversation/task | Completion and repository provenance |
| Interrupted submission | Lose the acknowledgment, kill/restart client and host at defined points, and recover one operation with the current draft preserved | Durable intent and retry behavior |
| Two-device supervision | A phone and desktop observe one task; one queues input, the other sees its authorized state; revocation blocks later mutations | Shared state and authority across clients |
| Native session recovery | For each advertised adapter, restart during idle/running/input-needed states and recover or report unknown state accurately | Native lifecycle fidelity rather than synthetic continuation |
| Rewind partial failure | Fail after file restoration and before provider rollback; stop further effects and identify the changed state | Recovery across separate external effects |
| History and scale | Load large retained history, search an unloaded message, stream several tasks, disconnect one environment, and measure foreground/network costs | Bounded projection, useful search, and scale |
| Independent upgrades | Exercise old/new clients, host downgrade with persisted history, and missing capabilities without changing authority | Protocol and replay compatibility |
| Suspended phone | Deliver an allowed task wake, open the exact task, and read current state after foregrounding | Real background delivery and navigation |
| Improvement after a task | Turn a reusable result into a capability/test set, run with/without it, inspect the report, and publish only on request | OpenAgents' differentiation connected to ordinary work |

Run targeted checks for the changed Rust owners and their consumers when these
slices are implemented. The separate `openagents-mobile` workspace needs its
own manifest-path checks. Native controls require native evidence; provider
fixtures do not prove a signed-in engine run. Owner-only release/device steps
belong in `NEEDS_OWNER.md` and do not hold code-complete issues open. Keep
manual/non-GitHub verification and the current deployment rules.

## Research limitations and follow-up measurements

- This analysis reads local source and retained records. It does not verify
  T3's published binary, hosted service, store listings, release availability,
  or test results. The checkout's contradictory mobile release documentation
  remains an evidence limitation.
- “Missing” means absent from the inspected product path, not absent from every
  experimental module in the repository. Designed NIPs and pure validators
  are counted separately from host/client integrations.
- Security observations concern the named contracts and source paths. Neither
  repository received a comprehensive security review or adversarial run.
- No head-to-head model-quality, cost, latency, memory, or reliability result
  follows from this survey. Comparable fixtures and controlled runs are needed.
- Current OpenAgents desktop evidence contains Mac-native, Linux, cross-build,
  and Wine checks with different scopes. The
  [Windows receipt](../desktop/verification/2026-09-30-windows/verification.md)
  records a real-Windows owner step; a compiled AppContainer path and a
  fail-closed Wine test do not establish a successful Windows sandboxed run.

The next research experiment should run the first three acceptance scenarios
against each product using temporary repositories and the same supported
provider. Record steps, refusals, recovery outcomes, transferred bytes, first
response, and time to an inspectable/published change. That would quantify
which workflow gaps dominate actual use without conflating product scope,
native rendering, and model quality.

## Document verification

All local source/document paths, heading anchors, reference definitions, and
pinned T3 source paths were checked against the two checkouts. Markdown
whitespace and heading separation were checked. This adds one research
document and imports no code, assets, credentials, or transcript archives.
Rust behavior tests are not required for this documentation-only change under
[the repository verification policy](../verification.md).

## Pinned T3 sources

The links below are navigation to the revision inspected locally. Public URL
availability was not checked during this analysis.

| Area | Primary evidence |
| --- | --- |
| Product and license | [README][t3-readme], [MIT license][t3-license], [mobile README][t3-mobile] |
| Architecture | [Ownership and durability][t3-overview], [shared client runtime][t3-client-runtime], [RPC contract][t3-rpc] |
| Provider fidelity | [Provider constraints][t3-providers], [adapter contract][t3-provider-adapter] |
| Durable orchestration | [Engine][t3-engine], [receipt schema][t3-command-receipts], [engine tests][t3-engine-tests] |
| Checkpoints | [Store][t3-checkpoints], [Git driver][t3-git-driver], [reactor][t3-checkpoint-reactor], [reactor tests][t3-checkpoint-tests] |
| Remote and authority | [Remote architecture][t3-remote], [environment auth][t3-auth], [RPC authorization][t3-rpc-auth], [connection runtime][t3-connections], [Connect][t3-connect] |
| Coding workflow | [Composer][t3-composer], [threads][t3-threads], [project settings][t3-project-settings], [permission modes][t3-permissions] |
| Repository review | [Source control][t3-source-control], [Git workflow service][t3-git], [PR service][t3-pr-service], [viewed-file revisions][t3-pr-revisions] |
| Platform workflows | [Terminal runtime][t3-terminal], [SnapShots][t3-snapshot], [preview manager][t3-preview], [mobile notifications][t3-notifications] |
| Operations and evidence | [Usage][t3-usage], [updating][t3-updating], [telemetry][t3-telemetry], [development container][t3-devcontainer] |
| Diagnostics | [Observability][t3-observability], [resource telemetry][t3-resource-telemetry] |

[t3-readme]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/README.md
[t3-license]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/LICENSE
[t3-mobile]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/mobile/README.md
[t3-overview]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/internals/overview.md
[t3-client-runtime]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/packages/client-runtime/README.md
[t3-rpc]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/packages/contracts/src/rpc.ts
[t3-providers]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/internals/providers.md
[t3-provider-adapter]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/server/src/provider/Services/ProviderAdapter.ts
[t3-engine]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/server/src/orchestration/Layers/OrchestrationEngine.ts
[t3-command-receipts]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/server/src/persistence/Services/OrchestrationCommandReceipts.ts
[t3-engine-tests]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/server/src/orchestration/Layers/OrchestrationEngine.test.ts
[t3-checkpoints]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/server/src/checkpointing/CheckpointStore.ts
[t3-git-driver]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/server/src/vcs/GitVcsDriver.ts
[t3-checkpoint-reactor]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/server/src/orchestration/Layers/CheckpointReactor.ts
[t3-checkpoint-tests]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/server/src/orchestration/Layers/CheckpointReactor.test.ts
[t3-remote]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/internals/remote.md
[t3-auth]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/internals/environment-auth.md
[t3-rpc-auth]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/server/src/auth/RpcAuthorization.ts
[t3-connections]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/internals/connection-runtime.md
[t3-connect]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/internals/t3-connect.md
[t3-composer]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/user/composer.md
[t3-threads]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/user/thread-sidebar.md
[t3-project-settings]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/user/project-settings.md
[t3-permissions]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/user/permission-modes.md
[t3-source-control]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/user/source-control.md
[t3-git]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/server/src/git/GitWorkflowService.ts
[t3-pr-service]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/server/src/pullRequest/PullRequestService.ts
[t3-pr-revisions]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/internals/pull-request-file-revisions.md
[t3-terminal]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/internals/terminal-runtime.md
[t3-snapshot]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/user/snap-shot.md
[t3-preview]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/apps/desktop/src/preview/Manager.ts
[t3-notifications]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/user/mobile-notifications.md
[t3-usage]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/user/usage.md
[t3-updating]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/user/updating.md
[t3-telemetry]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/user/telemetry.md
[t3-devcontainer]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/internals/devcontainer.md
[t3-observability]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/operations/observability.md
[t3-resource-telemetry]: https://github.com/pingdotgg/t3code/blob/6b286ae8a20ef2e18fa70b09e92f66edd859128b/docs/internals/resource-telemetry.md
