# Omega and T3 Code desktop and mobile gap analysis — 2026-07-27

Component and cloud architecture companion:
[T3 Code desktop, mobile, and cloud architecture audit](./2026-07-27-t3-code-desktop-mobile-component-cloud-architecture-audit.md)

Server consistency companion:
[T3 Code server projection and consistency architecture](./2026-07-27-t3-code-server-projection-consistency-architecture.md)

## Executive conclusion

Omega and T3 Code solve different parts of the same problem well.

T3 Code is the more complete agent control product. It has one server-owned
environment model. Desktop and mobile clients project that model. Its desktop
app has a complete workbench around each thread. Its mobile app is a real remote
controller.

Omega is the stronger native coding substrate. It has the Zed editor, language
services, terminal, project model, and native diff review. It also has stronger
work on identity, signed device grants, admission records, receipts, and
unattended run evidence.

The product gap is not a lack of basic agent capability in Omega. The main gap
is composition. Omega has several capable execution lanes and several durable
stores. It does not yet project them through one complete desktop and mobile
control model.

The desktop comparison is close in technical depth, but not in product breadth.
Omega can do more inside a local code workspace. T3 Code exposes more of the
agent operation lifecycle in one primary interface.

The mobile comparison is not close. The current Omega mobile app is an
authenticated activity mirror. Its send and steer controls are disabled. T3
Code Mobile can control threads, answer requests, inspect changes, use Git, and
open a terminal.

Omega should not copy T3 Code as a product. It should keep its native editor and
its authority model. It should adopt the complete control projection that makes
T3 Code coherent across devices.

## Scope

This report compares these source revisions:

| Product area                    | Revision                                   |
| ------------------------------- | ------------------------------------------ |
| Omega desktop                   | `dca8307b4f1ccda231b02175272c1edb510680a2` |
| Omega mobile                    | `017ae3dedb58a53b4d250f0cd83440490e125da6` |
| T3 Code broad teardown          | `c1ec1915`                                 |
| T3 Code mobile teardown         | `8b546986`                                 |
| T3 Code mobile deep dive        | `a148e08197fc38b24e59c10c7cd5ba06dd182dab` |
| T3 Code ACP teardown            | `bde0a4`                                   |
| T3 Code OpenCode release update | `fdca154`                                  |

The Omega desktop revision is the current `origin/main` revision from the Omega
repository. The Omega mobile revision is the last commit that changed the
committed app on this repository's `origin/main`; the later documentation
commits do not change `apps/openagents-mobile`.

The mobile section was refreshed against T3 Code `main` at
`a148e08197fc38b24e59c10c7cd5ba06dd182dab`, dated 2026-07-28. The desktop
findings retain the earlier teardown revisions. This distinction matters:
T3 Code Mobile changed substantially after the original `8b546986` review,
including Thread List v2, thread-shell synchronization, snoozing and settlement,
adaptive workspace work, native review and terminal expansion, incoming shares,
Live Activities, and OTA update controls.

The mobile worktree had separate uncommitted UI changes during this review.
Those changes are not part of this report. This rule prevents a local draft from
becoming a shipped capability claim.

The review used source, tests, product documents, release procedures, and
checked-in visual baselines. It did not use a live T3 Code environment. It did
not use an installed Omega release. Installed behavior can differ from source.

## Required reading

This report includes all eight T3 Code documents that existed in
`docs/teardowns/` at the start of the review:

1. [T3 Code teardown](./2026-07-13-t3-code-teardown.md)
2. [T3 Code and OpenAgents Desktop full gap analysis](./2026-07-15-t3-code-openagents-desktop-full-gap-analysis.md)
3. [T3 Code ACP implementation teardown](./2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md)
4. [T3 Code and OpenCode Electron build analysis](./2026-07-16-t3-code-opencode-electron-build-update-analysis.md)
5. [T3 Code Mobile teardown](./2026-07-17-t3-code-mobile-app-teardown.md)
6. [T3 Code and OpenAgents Desktop UI gap analysis](./2026-07-17-t3-code-openagents-desktop-ui-gap-analysis.md)
7. [T3 Code and OpenAgents Mobile component gap analysis](./2026-07-17-t3-code-openagents-mobile-component-gap-analysis.md)
8. [T3 Code and OpenAgents Mobile controller gap analysis](./2026-07-17-t3-code-openagents-mobile-controller-gap-analysis.md)

The older reports compare T3 Code with an Electron OpenAgents Desktop app. They
do not describe the current Omega desktop app. Omega is a Rust and GPUI
application that tracks Zed. This report replaces those older comparisons for
Omega planning.

## Status terms

This report uses four status terms:

- **Strong** means that Omega meets the user outcome and has meaningful proof.
- **Partial** means that code exists, but the primary flow is incomplete.
- **Missing** means that the current product does not expose the outcome.
- **Different** means that Omega uses another product model. It is not always a
  defect.

## Product shape

### T3 Code

T3 Code is a provider-neutral control plane. It does not implement the agent
engines. It adapts Codex, Claude, Cursor, Grok, and OpenCode into one server
model.

The server owns commands, events, projections, and durable state. SQLite stores
the event history. Accepted events, SQL projections, and accepted command
receipts commit atomically. Provider reactors translate committed intent into
runtime work, but they consume a live process-local stream without a durable
consumer cursor. A restart can therefore preserve the admitted intent while
losing the provider side effect. The `RuntimeReceiptBus` is test
synchronization, not a production bridge from provider effects into the
durable event log.

The Electron desktop client and the Expo mobile client are views over that
model. They share contracts and projection logic. They do not share all UI
components.

This model gives T3 Code a clear rule:

> An execution environment owns the work. Each client can control the same work
> through a scoped endpoint.

### Omega

Omega is a native IDE with several execution planes:

- The native Omega agent runs in the desktop process.
- External ACP agents can run through agent adapters.
- Terminal threads run through the native terminal.
- Full Auto runs use `omega-effectd`.
- Agent Computer and Sarah use separate workroom paths.
- The device bridge projects a bounded desktop mirror.
- A separate Nostr path carries signed remote commands.

This model has more local coding depth than T3 Code. It also has more authority
boundaries. The cost is fragmentation. No single projection describes every
thread, tool call, decision, artifact, and run.

### Architectural result

| Question                                       | T3 Code                            | Omega                                                   |
| ---------------------------------------------- | ---------------------------------- | ------------------------------------------------------- |
| What owns a thread?                            | Server environment                 | Native thread, ACP session, terminal, or Full Auto lane |
| What owns durable state?                       | Event store and projections        | Several local stores and supervisor records             |
| What does mobile consume?                      | Shared server projection           | Bounded device mirror and a separate command path       |
| Can a client reconnect?                        | Yes, through environment endpoints | Partial, by bridge cursor, generation, and lane rules   |
| Can a session move to another host?            | Not fully                          | Not fully                                               |
| Is the agent engine in the client?             | No                                 | Native Omega agent is in desktop                        |
| Is remote access a first-class product object? | Yes                                | Partial                                                 |

T3 Code has the cleaner multi-client model. Omega has the deeper local host.

## Top-level scorecard

| Capability group                    | Omega desktop                               | Omega mobile           | T3 Code lead             |
| ----------------------------------- | ------------------------------------------- | ---------------------- | ------------------------ |
| Native editing and language tools   | Strong                                      | Missing                | Omega                    |
| Agent timeline and tool detail      | Strong                                      | Partial                | Omega desktop            |
| Provider-neutral control            | Partial                                     | Missing                | T3 Code                  |
| Project and worktree control        | Partial                                     | Missing                | T3 Code product flow     |
| File tree and file inspection       | Strong in full editor                       | Missing                | Split                    |
| Diff review                         | Strong in full editor                       | Missing                | T3 Code across devices   |
| Git and forge actions               | Strong inherited Git, limited default shell | Missing                | T3 Code product flow     |
| Integrated terminal                 | Strong                                      | Missing                | Omega desktop, T3 mobile |
| Browser preview and automation      | Partial through tools                       | Missing                | T3 Code                  |
| Durable queue and steer law         | Strong design, partial restore              | Missing in current app | Omega desktop            |
| Approvals and questions             | Strong on desktop                           | Missing                | T3 Code across devices   |
| Remote environment catalog          | Missing                                     | Missing                | T3 Code                  |
| Secure device pairing               | Strong                                      | Strong                 | Omega                    |
| Remote thread control               | Partial command path                        | Missing in current UI  | T3 Code                  |
| Offline mobile outbox               | Missing                                     | Missing                | T3 Code                  |
| Mobile notifications and deep links | Partial platform base                       | Missing product flow   | T3 Code                  |
| Full Auto run evidence              | Strong                                      | Partial mirror         | Omega                    |
| Cross-platform release              | Partial                                     | Partial                | T3 Code                  |
| Identity and authority evidence     | Strong                                      | Strong                 | Omega                    |

## Desktop gap analysis

### 1. First run and product entry

#### Omega status: Strong, with one clarity gap

Omega starts in zero base by default. This mode removes the normal editor shell.
It keeps a thread, a composer, a small folder header, and a collapsible sidebar.
The user can start without a project. The user can also select a folder.

Identity onboarding runs before zero base is sealed. Provider credential errors
link to a focused settings surface. OpenAgents authentication does not open a
browser.

The current visual baselines show a calm and direct surface. The transcript has
the most space. The composer stays at the bottom. Narrow layouts remove
secondary chrome without changing the main flow.

The gap is product explanation. The footer still offers “Leave zero base.” A
new user must infer what that action enables. T3 Code presents its workbench
features as parts of the same product. Omega presents its full editor as another
mode.

#### T3 Code difference

T3 Code starts from projects and execution environments. That flow teaches the
user where work runs and how clients connect to it. Omega starts from identity
and a local thread.

#### Required closure

Keep zero base as the default. Add a clear workspace expansion model. Opening a
file, diff, terminal, or plan must feel like a normal thread action. It must not
feel like an exit to another application mode.

### 2. App shell and navigation

#### Omega status: Partial

Zero base has one persistent sidebar. It can show recent threads and the latest
public chat items. The public section is a small read-once projection. It has no
live subscription, moderation controls, or complete identity display.

The main thread header has a new-thread action, compact window actions, and a
folder selector. Settings are in a footer route.

The full editor has the mature Zed project panel, tab bar, docks, status bar,
Git panel, terminal panel, and command palette. Zero base intentionally hides
these surfaces.

T3 Code keeps project, thread, files, review, terminal, plan, and preview
navigation inside one workbench. Omega has the pieces, but the default product
does not compose them.

#### Gap

Omega has no default thread workbench rail. It has no visible activity switcher
for files, review, plan, terminal, or preview. The public chat section also uses
the same small sidebar area as local work, although it has a different purpose.

#### Required closure

Add one compact activity model to zero base. It should reveal existing native
surfaces. It should not rebuild them in a web shell.

### 3. Projects, repositories, and worktrees

#### Omega status: Partial

Omega inherits Zed project and worktree support. Agent threads carry work
directories. Threads can resolve mentions across visible worktrees. Agent
metadata can restore folders. The agent panel can create a sibling worktree for
some new-thread flows.

The default path still treats the current folder as the main unit. It does not
show a T3-style project catalog. It does not show a worktree as a normal thread
property. It does not make concurrent branch isolation obvious.

T3 Code makes a project, thread, branch, and worktree relationship visible. It
can create and remove worktrees from the agent flow.

#### Gap

- Omega has no project dashboard in zero base.
- A thread does not always show its branch and worktree state.
- Worktree creation is not a standard new-thread choice.
- Worktree cleanup is not part of thread lifecycle in the default UI.
- Mobile cannot select a project or worktree.

#### Required closure

Make the execution root explicit on every thread. Add a new-thread choice for
the current worktree, a new worktree, or no project. Show the cleanup result
when a thread owns a temporary worktree.

### 4. Agent engines and provider control

#### Omega status: Different and partial

Every new zero-base chat belongs to the native Omega loop. The composer has no
provider, model, mode, or executor selector. Hosted Gemini is the default model.
A local key is an optional fallback.

Omega can detect Codex, Claude, and Exo. It can delegate to explicit executors.
It can also run external ACP agents and terminal agents. Full Auto has Codex and
Claude ACP lanes.

This is a deliberate product choice. It gives Omega one visible identity. It
prevents provider selection from dominating the composer.

T3 Code takes the opposite approach. It exposes several engines through one
server contract. The user can choose an engine and resume its work.

#### Gap

Omega has adapter breadth, but not one provider-neutral session projection. A
native thread, an ACP thread, and a Full Auto run do not have the same lifecycle
or event model. A user cannot inspect one clear capability and authority summary
before a run starts.

#### Required closure

Keep Omega as the default agent. Add one read-only execution summary to each
thread. It must show the active lane, model class, effective tools, authority,
workspace, and remote-control scope. Advanced lane selection can stay outside
the composer.

### 5. Timeline, composer, and context

#### Omega status: Strong

The desktop transcript uses GPUI `ListState`. It virtualizes entries and follows
the tail. It preserves scroll state and supports keyboard movement. This is not
a simple unbounded message column.

The native thread renderer supports:

- Markdown and code.
- Tool calls and tool results.
- Terminal command output.
- File and diff content.
- Plans and completed plan snapshots.
- Permission requests.
- Structured elicitation forms.
- Subagent activity.
- Bounded tool-result previews with artifact references.

The composer supports mentions, images, dragged files, commands, skills, and
workspace context. File links can open in an editor. A modified click opens a
compact read-only view.

T3 Code has a richer contextual Lexical composer and a timeline minimap. It also
uses worker-backed diff rendering. Omega does not have the same minimap or the
same explicit right-panel context model.

#### Gap

- Omega has no transcript minimap.
- The default view does not expose an attachment or context inventory.
- Tool artifacts can be hard to rediscover after the related event scrolls out.
- The large native thread modules create a maintenance and review risk.
- The mobile transcript only receives bounded user and assistant text previews.

#### Required closure

Do not replace the native renderer. Add a compact thread outline and artifact
index. Reuse the existing virtual list, entry model, and editor renderer.

### 6. Send, queue, steer, and interrupt

#### Omega status: Strong design, partial recovery

Omega defines an explicit total law for a send during an active turn. A native
agent can accept a boundary steer. An external ACP agent can refuse when its
behavior is unknown. An engine lane cannot be steered through the normal
composer.

Queued work is written to a journal before the UI acknowledges it. Entries have
ordered terminal states. Quiescence is part of completion.

This is more explicit than the T3 Code draft-ahead behavior. T3 Code can use a
second send as an implicit steer in some Codex flows. It does not expose the
same durable product queue law.

The Omega gap is restart reconstruction. The journal is durable, but the live
composer queue is still partly view-owned. A restart does not always rebuild the
same pending rows in the composer.

#### Required closure

Project the durable queue into desktop and mobile. The same queue item must keep
its identity, order, state, and receipt reference after restart.

### 7. Approvals, questions, and plans

#### Omega status: Strong on desktop, missing on mobile

The native conversation supports tool permission cards and structured
elicitation. It can show allow-once and durable rule choices. It can show plan
state and completed plan snapshots.

T3 Code also treats approvals, questions, and plans as first-class timeline
events. Its mobile app can answer those events.

Omega does not project these events through the current device mirror. Mobile
receives only bounded text previews and run summaries.

#### Required closure

Define a portable interaction event. It must include a stable request ID,
expiry, allowed answers, authority effect, and terminal receipt. Use the same
event on desktop and mobile.

### 8. Files, editor, and search

#### Omega status: Strong in full editor, partial in zero base

Omega inherits a mature native editor. It has syntax trees, language servers,
symbols, diagnostics, multi-buffer search, file operations, and project search.
The agent can use these tools directly.

Zero base can open a file from a transcript. That action reveals an ordinary
editor pane. It can also open a compact read-only view. The last editor can
close and restore the agent-only layout.

T3 Code offers a file tree and file viewer as normal right-panel modes. Its
editor depth is lower, but its agent workbench composition is clearer.

#### Gap

The user cannot browse project files from the default zero-base shell until
another action opens an editor surface. The default thread also has no compact
search result panel.

#### Required closure

Add Files and Search as zero-base activities. Use the native Zed project and
search entities. Do not create a second file model.

### 9. Diff review and checkpoints

#### Omega status: Strong on desktop

Omega has a native `AgentDiff` view. It can show pending edits, accept or reject
changes, review branch diffs, and restore checkpoints. Subagent edits can enter
the parent action log for review.

This is one of Omega's strongest areas. It combines an agent transcript with a
real editor buffer model. T3 Code uses a specialized diff renderer and makes
review a visible workbench tab.

#### Gap

- Review is not a persistent default activity in zero base.
- Mobile receives no diff hunks or review state.
- The device mirror omits tool calls and artifacts that explain a change.
- A cross-device accept or reject receipt does not exist in the current app.

#### Required closure

Expose the existing Agent Diff pane as a standard zero-base activity. Add a
portable read-only diff contract first. Add mobile review commands only after
the authority and receipt model is complete.

### 10. Git and forge workflows

#### Omega status: Partial

The full editor inherits strong Git support. It can show changes, stage files,
commit, switch branches, and resolve conflicts. Agent actions can include branch
review context.

Omega also adds a useful unattended-work safeguard. Destructive Git commands
must not erase a dirty worktree without a record and an allowed path.

T3 Code places common Git actions in the agent workbench. It also integrates
four forge providers for pull requests and related review work.

#### Gap

- Zero base has no compact Git status or branch control.
- The primary agent view has no pull-request surface.
- Mobile has no Git status, stage, commit, push, or pull-request flow.
- Forge results do not have one portable event model across Omega lanes.

#### Required closure

Add a small Git activity to zero base. Reuse the native Git store. Add forge
actions after local Git state has one cross-device projection.

### 11. Terminal

#### Omega status: Strong on desktop, missing on mobile

Omega has the native Zed terminal and terminal panel. Agent tools can run tasks
and wait for completion. Terminal threads can appear beside agent threads.
Terminal state and working directories have restore paths.

T3 Code uses xterm in desktop and mobile. Its mobile terminal is a meaningful
control advantage. The T3 teardown also found an accessibility defect because
xterm screen-reader mode was disabled.

#### Gap

The zero-base shell does not present the terminal as a normal workbench
activity. Mobile has no terminal surface and no safe terminal command contract.

#### Required closure

Expose the native terminal in zero base. For mobile, start with bounded command
tasks and output frames. Do not start with a full remote pseudo-terminal unless
screen-reader, resize, credential, and reconnect behavior are proved.

### 12. Preview and browser automation

#### Omega status: Partial

Omega agents can read URLs and can use browser tools through configured
integrations. The full editor has inherited web and extension surfaces.

T3 Code has an explicit browser preview panel. It can manage preview sessions
and connect browser automation to the thread context.

Omega has no standard zero-base preview activity. Mobile has no preview.

#### Required closure

Add a preview session as a typed thread artifact. It needs a local URL,
ownership, lifecycle, and safe open action. Browser automation must use the same
session identity.

### 13. Full Auto and long work

#### Omega status: Strong

Full Auto is a dedicated dock panel. It supports an objective, advanced
settings, up to eight runs, pause, resume, handoff, stop, and retry. It does not
reuse the normal composer.

`omega-effectd` owns process health, restart generations, leases, run actions,
and durable run records. Reverse host calls have bounded timeouts. Packaged
runtime manifests and installed evidence bind important artifacts.

The device mirror can show up to 64 Full Auto runs. It includes state, receipt
references, engine health, and lane health.

T3 Code has a strong server reactor model, but Omega has the stronger explicit
evidence chain for unattended runs.

#### Gap

Full Auto remains a separate control plane. A normal thread cannot become a
Full Auto run through one continuous projection. Mobile can observe the run,
but the current mobile screen does not expose the signed command set.

#### Required closure

Create one portable run summary for all long-running work. Keep lane-specific
controls, but normalize state, progress, stop reason, evidence, and handoff.

### 14. Remote environments

#### Omega status: Missing as a product model

T3 Code defines execution environments, known environments, access endpoints,
and advertised endpoints. It can use direct WebSocket connections, relays,
Tailscale, and desktop SSH forwarding. It separates access from process launch.

Omega supports local work and inherits Zed remote-development technology. The
device bridge can listen on loopback or a Tailscale address. Discovery can
publish a structured MagicDNS endpoint.

These features do not form an environment catalog. A mobile user cannot select
one of several Omega hosts. A desktop user cannot inspect endpoint scope,
health, and launch authority in one place.

#### Required closure

Add an Omega host object. It should contain host identity, generation, access
endpoints, capabilities, and granted command scope. Do not expose raw network
details as the primary UI.

### 15. Persistence, restart, and recovery

#### Omega status: Strong but fragmented

Native threads persist locally. Terminal metadata persists. Queue journal
entries persist. Full Auto runs persist through `omega-effectd`. The device
bridge supports a snapshot, ordered deltas, cursor resume, and generation
changes.

T3 Code has one event-sourced SQLite model for commands, events, and
projections. That model gives every client one recovery story.

Omega has several good recovery stories. It does not have one end-to-end story.
A user cannot ask one surface which work is pending across all lanes.

#### Required closure

Create a read-only aggregate projection first. It can index native threads, ACP
sessions, terminal threads, and Full Auto runs without moving their authority.
Use stable source references and generation fences.

### 16. Settings and account surfaces

#### Omega status: Partial

Zero base has focused provider-key settings. Legacy settings remain available
outside the focused surface. Identity is an explicit top-level control.

T3 Code has broader settings for providers, environments, connections, and
release channels.

Omega settings do not yet explain effective tool authority, remote grants,
device sessions, or per-lane data flow in one place.

#### Required closure

Add an Authority and Devices settings group. Show effective rules and active
grants. Permit revocation. Show which surfaces can read or change each lane.

### 17. Release and platform coverage

#### Omega status: Partial

The owned Omega RC procedure produces a signed macOS arm64 disk image. It binds
the source revision, lockfile, icon set, package digest, signing identity, and
the packaged `omega-effectd` runtime. It records notarization and stapling
truthfully.

The inherited repository still contains Linux and Windows bundle scripts.
Those scripts are not proof that the current Omega release publishes those
targets.

T3 Code has a broader desktop artifact matrix and stable or nightly update
channels. Its teardown also found a historical Gatekeeper failure and
conditional signing risk.

#### Gap

- The owned Omega candidate is macOS arm64 only.
- There is no equivalent owned release proof for macOS x64, Windows, or Linux.
- The current procedure does not describe a complete update and rollback
  channel.
- Mobile release proof is separate from desktop host release proof.

#### Required closure

Keep the fail-closed release record. Add one target at a time. Each target needs
the same source, package, runtime, signature, install, update, and rollback
evidence.

## Mobile deep dive and gap analysis

### Evidence boundary and central conclusion

This section is a source-level replication study of both React Native apps. It
uses committed source, tests, native modules, app configuration, and the T3
showcase capture harness. It does not claim behavior observed in a production
store build. T3 Code's own README says that the mobile app is not currently
distributed; its source still defines a production-quality controller.

The current committed Omega app and current T3 Code app are not two
implementations of the same mobile product:

- Omega is a secure, bounded desktop mirror with one mounted screen.
- T3 Code is a remote coding workbench with a route graph, durable client
  state, multiple environment sessions, thread commands, Git, review, files,
  terminal, notifications, shares, widgets, and adaptive tablet panes.

The visual gap is large, but the decisive gap is below the visuals. T3's rows,
composer, review sheets, and terminal all operate on a shared environment and
thread command model. Omega cannot reach meaningful visual parity by styling
its current activity cards. It needs the missing controller contracts and
navigation topology, then it can bring the shell closely in line with T3.

### Current source footprint

| Measure                      |                            T3 Code Mobile |                                 Omega mobile |
| ---------------------------- | ----------------------------------------: | -------------------------------------------: |
| React Native                 |                                  `0.85.3` |                                     `0.86.0` |
| React                        |                                  `19.2.3` |                                     `19.2.7` |
| Expo                         |                                `~56.0.12` |                                     `57.0.2` |
| App source files             |                                       524 |                                           12 |
| App source lines             |                              about 95,715 |                                        2,586 |
| Shared client-runtime source |                        about 25,190 lines |      no equivalent mobile controller runtime |
| Mobile native-island source  |                        about 11,127 lines |                                         none |
| App tests                    |                                        92 |                                            4 |
| App routes                   | more than 20 root and nested destinations | one mounted screen with local view switching |

The T3 count is not a target by itself. It shows that the visible app rests on
substantial synchronization, persistence, platform, and native rendering work.
An Omega implementation can be smaller by reusing its own contracts, but it
cannot omit those responsibilities.

### React Native stack comparison

| Concern            | T3 Code Mobile                                                              | Omega mobile                                                     | Replication implication                                                                            |
| ------------------ | --------------------------------------------------------------------------- | ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Navigation         | React Navigation native stack and nested sheet stacks                       | none                                                             | Add a typed root stack before adding more local `selected*` state                                  |
| Server state       | Effect Atom over a shared client runtime                                    | local React state over one bridge client                         | Add an app-scoped controller registry keyed by host and work item                                  |
| Lists              | Legend List for high-churn transcript and thread lists                      | React Native `FlatList`                                          | `FlatList` is sufficient initially; preserve an upgrade boundary for anchoring and very long feeds |
| Gestures           | Gesture Handler and Reanimated                                              | none                                                             | Needed for thread lifecycle swipes, inspector animation, and composer morphs                       |
| Keyboard           | `react-native-keyboard-controller` plus a native composer                   | stock keyboard avoidance                                         | Needed for T3-like sticky composer and transcript anchoring                                        |
| Styling            | Uniwind tokens, automatic light/dark themes, DM Sans                        | JS dark tokens, System and Menlo                                 | Introduce semantic tokens and automatic appearance before screen replication                       |
| Markdown           | native selectable Markdown on iOS, Nitro fallback                           | plain text                                                       | Add a portable rich-message renderer with code, links, and copy                                    |
| Diff               | native Swift and Kotlin diff views, JS fallback                             | none                                                             | Start with bounded JS read-only diffs; native host is a later performance decision                 |
| Terminal           | native Ghostty-derived surface, text fallback                               | none                                                             | Ship task output first; isolate a future terminal host behind a session contract                   |
| Persistence        | SQLite, SecureStore, file-backed outbox and drafts                          | SecureStore for identity and bridge state                        | Add durable drafts, outbox, host catalog, and preferences                                          |
| Device integration | camera, notifications, widgets, Live Activities, shares, quick actions, OTA | camera and installed Expo foundations, no complete product flows | Add integrations only after exact routes and attention objects exist                               |

### T3's mobile information architecture

T3 uses progressive disclosure. The phone navigates from list to detail to
specialized full-screen tools. A large tablet keeps the list visible, renders
thread detail beside it, and can add a file or Git inspector. The same work item
identity is used in both layouts.

The current route graph includes:

```text
Root
├── Home
├── Thread
│   ├── ThreadTerminal
│   ├── ThreadReview
│   │   └── ThreadReviewComment
│   ├── ThreadFiles
│   │   └── ThreadFile
│   ├── GitOverview
│   │   ├── GitCommit
│   │   ├── GitBranches
│   │   └── GitConfirm
│   └── direct deep links by environmentId + threadId
├── SettingsSheet
│   ├── Settings
│   ├── Environments
│   ├── Appearance
│   ├── Client Storage
│   ├── Authentication / Waitlist
│   └── Archived Threads
├── NewTaskSheet
│   ├── Choose Project
│   ├── Draft
│   ├── Add Project
│   ├── Repository
│   ├── Destination
│   └── Local Path
├── Connect Onboarding
├── Connections
├── Add Environment
├── Legal
└── Not Found
```

Thread tools remain flat in the root native stack on iOS so native header
transitions can morph between them. Sheet-only overlays are excluded from
workspace selection, so opening Settings or New Task does not silently change
the selected thread underneath. That detail prevents a common tablet bug:
dismissing a sheet should return to the same list/detail/inspector composition.

Omega currently implements Home, Thread, and Pairing as conditional branches
inside `OmegaHomeView`. Adding Files or Settings that way would multiply local
state and break deep linking, back behavior, restoration, and tablet
composition. The first structural change should be a typed navigation graph
whose canonical work-item key contains host, lane, and thread reference.

### Adaptive workspace geometry

T3 does not use a simple `isTablet` boolean. Its layout is based on available
viewport and minimum usable pane sizes:

| Rule                               | Current T3 value |
| ---------------------------------- | ---------------: |
| Split layout minimum width         |       720 points |
| Split layout minimum height        |       600 points |
| Sidebar target                     |  32% of viewport |
| Sidebar clamp                      |   280–380 points |
| Sidebar resize ceiling             |       460 points |
| File inspector minimum viewport    |       820 points |
| Minimum main content width         |       560 points |
| Inspector clamp                    |   260–480 points |
| Supplementary pane minimum content |       960 points |
| Maximum chat content width         |       960 points |

The persistent sidebar lives next to the native navigator rather than inside
each screen. The inspector is registered by the focused route and is rendered
through a layout portal with the correct navigation and route context.
Registration is focus-scoped so a blurred screen cannot leave a stale inspector
mounted.

Sidebar and inspector widths animate, but the settled content pane is frozen at
its final width during the transition. This avoids repeatedly reflowing
Markdown and dropping frames. Omega should copy the invariant, not necessarily
the animation:

> A layout transition may animate chrome, but it must not continuously remeasure
> the expensive transcript or diff surface.

Recommended Omega layout modes:

| Mode      | Geometry                                 | Navigation                    |
| --------- | ---------------------------------------- | ----------------------------- |
| Compact   | under 720 wide or under 600 high         | list → detail → tool routes   |
| Split     | at least 720 × 600                       | persistent work list + detail |
| Workbench | at least 960 content width after sidebar | list + detail + inspector     |

### Screen 1: connect onboarding and environments

#### T3 structure

T3 separates connection objects from projects and threads. A connection
catalog can hold primary, bearer, SSH, and relay targets. Each environment owns
an independent supervisor with these user-visible phases:

- available but not requested;
- connecting, with preparation and probe stages;
- connected;
- offline because the network is offline;
- retrying after a transient failure;
- failed with a typed reason.

Connection establishment has 15-second preparation and probe timeouts.
Transient failures back off at 1, 2, 4, 8, and 16 seconds. A connection stable
for 30 seconds resets the backoff. Network changes and explicit wakeups can
restart a connection immediately.

The Add Environment screen supports:

- direct host and pairing-code entry;
- a QR scanner;
- raw pairing URLs;
- mobile deep links containing an encoded pairing URL;
- clear camera-denied and invalid-QR states;
- success navigation that works from both pushed and cold-started routes.

The Environments screen shows all registered environments as expandable rows.
A row can expose status, URL, reconnect, edit, and removal. With configured
cloud support, relay-discovered environments appear separately and can be
registered without retyping endpoints.

#### Omega now

Omega has a stronger cryptographic first-pair:

- a device-local Nostr key;
- fail-closed platform custody rules;
- a signed admission proof;
- short-lived pairing material;
- a stored grant;
- strict 64 KiB frames;
- ordered snapshot and delta application;
- generation and resume cursors;
- cached, announced, QR, and manual MagicDNS dial candidates.

The mounted product still treats those mechanisms as one implicit desktop.
There is no host catalog, connection list, diagnostics screen, reconnect
control, grant expiry display, or revoke action.

#### Omega target

Keep the existing key and grant law. Add an `EnvironmentCatalog` presentation
over it:

```text
EnvironmentRecord
├── environmentId
├── displayName
├── hostPublicKey
├── endpoints[]
├── grantRef + expiry
├── desiredConnection
├── supervisorState
├── lastSuccessfulEndpoint
├── lastHeartbeat
└── lastFailure
```

The initial Omega Environments row should show name, Direct/Relay/Offline,
staleness, grant expiry, and the last typed failure. Expanded actions should be
Reconnect, Rename, Revoke Device Grant, and Forget Host. Revocation and forgetting
must remain separate because they have different remote and local effects.

### Screen 2: Home and Thread List v2

#### T3 structure

T3's current Thread List v2 is a device-local beta, off by default. Its
important behavior is not the beta label; it is a new inbox model:

- one flat list rather than nested project groups;
- fixed newest-created-first ordering for active work;
- activity does not reorder active rows under the user's finger;
- active and attention-bearing work uses rich cards;
- settled work collapses into compact rows under a `Settled` divider;
- the settled tail is recency ordered;
- 10 settled rows appear initially, then `Show more` adds 25;
- queued offline tasks remain above server-backed threads.

Thread state precedence is:

1. approval required;
2. user input required;
3. starting or running;
4. failed;
5. ready.

Color is reserved for action:

- amber for approval;
- indigo for input;
- sky for working;
- red for failure;
- neutral time for ready.

A rich row contains:

- project favicon and project name;
- state or relative time;
- a two-line thread title;
- branch;
- environment label when more than one environment exists;
- pull-request number and state when present;
- provider icon;
- failure detail when failed.

Phone rows are edge-to-edge with inset separators. In the iPad sidebar they are
rounded and the selected row receives an accent fill. The information stays the
same; only selection and container treatment change.

#### Lifecycle gestures

The leading lifecycle action depends on row state:

- active thread → settle;
- settled thread → un-settle;
- server without settlement support → archive fallback.

Delete is always secondary and destructive. Full swipe performs the lifecycle
action, never delete. Long press exposes the same actions for users who cannot
or do not discover swipes. Gestures are disabled while the list is actively
scrolling.

Settlement is not merely a local archive flag. T3 computes an effective state
from:

- the server's explicit override;
- absence of a running session;
- absence of pending approval or input;
- inactivity warmth;
- merged or closed pull-request state.

Snoozed threads are hidden until their wake time. Approval, input, a fresh
failure, or completion after snoozing can raise the thread early. The list uses
an exact timeout for the next wake and a minute tick for inactivity
settlement. Current mobile has one notable gap: it names snoozed counts in an
empty state but has no snoozed shelf UI.

#### Header and search

The iPad sidebar is an intentionally navigation-inert native stack on iOS. It
exists to obtain a real `UINavigationBar`, large `Threads` title,
`UISearchController`, and native bar items without allowing the sidebar to own
workspace navigation.

Android and non-native fallbacks render:

- a 34-point title;
- 44-point filter and settings circles;
- a 38-point search field;
- a subtle scrolled-edge wash;
- a new-task floating action button on compact Android.

The list uses Legend List recycling, an estimated 64-point row size, and a
500-point draw distance.

#### Omega now

Omega mixes thread rows and Full Auto run rows in one activity feed sorted by
`updatedAt`. It has no search, filters, project identity, branch, pull request,
attention precedence, queued local tasks, lifecycle gestures, selection
treatment, or settled tail.

Its cards communicate executor and state, but generic cards make every row
equally heavy. A running thread, completed thread, and passive Full Auto record
compete at the same level.

#### Omega target

Bring Omega close to T3's list grammar:

- use rich cards only for running, blocked, failed, or queued items;
- use compact rows for ready and completed history;
- reserve color for attention and failure;
- keep active creation order stable;
- place locally queued commands or tasks above projected rows;
- add project/workspace, branch, lane, and host as secondary metadata;
- show an Omega-specific receipt or Full Auto evidence indicator without
  turning the row into a dashboard;
- expose Settle/Unsettle and Delete through both swipe and long press;
- never make destructive delete the full-swipe action.

Omega needs an explicit `WorkListItem` union for native agent threads, ACP
sessions, terminal threads, and Full Auto runs. A row must not infer capability
from a display state; it should receive allowed lifecycle actions from the
projection.

### Screen 3: choose project and create a task

#### T3 structure

New Task is a nested sheet flow, not a modal over the existing composer:

1. choose a logical repository/project;
2. enter the task draft;
3. choose environment when the repository exists on multiple hosts;
4. choose local workspace or worktree mode;
5. choose a branch and whether to start from origin;
6. choose provider/model;
7. choose runtime and interaction modes;
8. add image attachments;
9. submit or preserve as a pending task.

Project rows are grouped by repository identity across environments. Switching
an environment attempts to retain the same canonical repository, then falls
back to workspace basename or project title.

The draft is persisted per project. It contains text, image attachments,
model, provider options, runtime mode, interaction mode, and workspace
selection. Persistence is debounced by 200 ms and survives app restart.

Creating work in an offline environment does not discard the draft. T3 builds a
fully typed pending creation containing:

- environment, project, thread, command, and message IDs;
- prompt and attachments;
- provider/model selection;
- runtime and interaction mode;
- local or worktree mode;
- branch and optional worktree path;
- start-from-origin choice;
- project title and cwd snapshot for offline presentation;
- creation time.

The queued task appears in the thread list, can be reopened and edited, and is
held out of the drain while an editor owns it. A late save cannot resurrect a
task that was deleted or delivered.

#### Add Project flow

The project flow distinguishes repository source, destination environment, and
local path. This prevents one overloaded path field from representing local
folders, cloned repositories, and remote environments. Branch and worktree
choices are first-class task inputs rather than hidden Git consequences.

#### Omega now

Omega has no project catalog, repository identity, new-task route, durable
draft, attachment import, model/runtime choice, worktree choice, or create
command.

#### Omega target

The first Omega flow can be narrower than T3:

1. choose paired host;
2. choose projected workspace;
3. choose Native Agent or Full Auto when both are admissible;
4. enter prompt and optional images;
5. choose current workspace or isolated worktree;
6. show the effective authority summary;
7. create with a stable command ID.

Provider selection should remain secondary to Omega's product identity. Lane,
workspace, isolation, and authority are the important user choices.

### Screen 4: thread detail shell

#### T3 structure

T3 renders the thread shell immediately from the lightweight shell projection.
It does not wait for full message detail. The header contains:

- thread title;
- project and optional environment subtitle;
- Git status/action menu;
- Files;
- Terminal;
- sidebar and inspector controls when adaptive layout supports them.

A cold-start deep link with no navigation history gets an explicit Threads
button. Normal pushed navigation gets native back. Android uses an in-flow
header; iOS uses native header items and shared backgrounds.

The selected thread identity is exact:

```text
environmentId + threadId + projectId + cwd + optional worktreePath
```

Files and Git are pushed routes on compact phones. On tablets they can register
as the inspector without changing the selected thread. Terminal supports
several sessions and project scripts.

#### Omega now

Omega detail has a custom `← Activity` button, title, executor/model/state line,
plain transcript, and disabled composer. It has no route identity, project
context, worktree context, header actions, or cold-start path.

#### Omega target

Adopt the same shell hierarchy:

- native or platform-appropriate back/list affordance;
- title;
- workspace · branch · host subtitle;
- a compact lane/authority indicator;
- Review, Files, and Output actions;
- Terminal only when the lane and grant support it;
- overflow for receipts, device authority, and destructive actions.

The header should show what the user is controlling, not only which model ran.

### Screen 5: transcript and work log

#### T3 list mechanics

T3's transcript is a keyboard-aware Legend List with extensive anchoring law:

- initial scroll at end;
- maintain scroll at end on data, layout, and item-size changes;
- maintain visible content position when older content changes height;
- extra anchored space after the newly sent user message;
- explicit remount when an empty feed first becomes populated so native insets
  are applied;
- temporary feed freezing while work disclosures expand or collapse;
- automatic header and keyboard inset compensation;
- an estimated 180-point item size;
- item types by role;
- a centered maximum content width of 960 points.

When a thread changes, the feed resets its anchor and freeze state. Sending
awaits the message ID, dismisses the keyboard, then anchors that exact row. A
tap on the feed collapses the composer; a drag does not. Streaming assistant
growth produces a throttled haptic at most once every 320 ms.

These are not decorative details. Without them, a streaming transcript jumps,
buries the user's just-sent message, or fights the keyboard.

#### T3 message grammar

User messages:

- right aligned;
- blue surface;
- 20-point radius;
- approximately 14 horizontal and 10 vertical padding;
- at most 85% width;
- attachments above or below the text;
- timestamp and copy action.

Assistant messages:

- full-width and unboxed;
- rich Markdown;
- selectable text where supported;
- syntax-highlighted code;
- a code header with language and copy;
- horizontally scrollable non-wrapped code when preferred;
- timestamp and copy metadata kept visually quiet.

Links are typed. File links open the exact Thread File route and can carry a
line target. External links open externally. Images open a viewer with swipe
close and double-tap zoom.

Work events are not rendered as chat bubbles:

- current work has a compact running row and elapsed time;
- old turns can fold;
- related work events group and collapse;
- success/failure is carried by small semantic icons;
- a compact row expands to full detail;
- long press copies details;
- fresh rows may animate, but remounted rows older than three seconds do not
  replay entrance motion.

Inline review comments contain file identity, a bounded diff preview, and
comment text. This preserves the causal link between a user message and the
code selection that generated it.

#### Omega now

Omega renders every role inside the same bordered bubble shape. Assistant
Markdown, code, tool calls, attachments, work groups, plans, requests, file
links, timestamps, and copy are absent. The bridge intentionally withholds
paths and raw tool payloads, which is a sound security default.

#### Omega target

Copy T3's information grammar, not its raw event payloads:

| Portable event        | Mobile rendering       | Required bound                              |
| --------------------- | ---------------------- | ------------------------------------------- |
| User message          | right-aligned bubble   | text and bounded images                     |
| Assistant message     | unboxed Markdown       | sanitized Markdown and code limits          |
| Work started/running  | compact work row       | tool class and safe label only              |
| Work completed/failed | result row             | safe summary and receipt reference          |
| Plan update           | collapsible plan block | bounded step text                           |
| Approval request      | attention card         | exact request ID and allowed responses      |
| User question         | question card          | exact question and typed choices            |
| Artifact              | file/diff/output link  | opaque scoped artifact reference            |
| Full Auto evidence    | receipt row            | receipt type, digest, state, and safe label |

Do not send raw shell commands, environment variables, tool JSON, or absolute
paths merely to make the mobile feed look complete. The host should create the
portable summary.

### Screen 6: composer

#### T3 structure

T3's composer has two deliberate states:

| State     |   Approximate chrome height | Shape                                                         |
| --------- | --------------------------: | ------------------------------------------------------------- |
| Collapsed |                   60 points | single pill with 36-point editor and circular send/stop       |
| Expanded  | 174 points before safe area | 20-point card with multiline editor, attachments, and toolbar |

The multiline editor grows from about 80 to 160 points. iOS can use Liquid
Glass on supported versions; all other cases use an opaque tokenized surface
with a restrained shadow. Android disables layout animation while the IME is
moving. iOS uses an approximately 220 ms morph.

A connection pill floats above the composer for reconnecting, offline, error,
or syncing. It is actionable when reconnect is possible.

The primary action is:

- Stop during an active run;
- Send when connected, idle, and no queue exists;
- Queue when offline, busy, or already queued.

Queued count is visible. One send can be in flight per exact environment/thread
key. The composer does not imply success until the command resolves.

#### Composer inputs and menus

The toolbar exposes:

- attachment picker;
- grouped provider/model menu;
- provider options;
- runtime policy: approval required, auto-accept edits, auto, or full access;
- interaction mode: default or plan;
- stop;
- send or queue.

Inline triggers provide:

- `/` for app and provider commands;
- `$` for ranked skill search;
- `@` for server-backed path search rooted in the project cwd.

Attachments show thumbnails, preview, and removal. The native composer is an
optimization for selection, keyboard, and high-frequency editing behavior; the
state contract remains in React.

#### Omega now

Omega has a normal multiline field plus Send and Steer. Both actions are
disabled because `commandLaneAvailable` is hard-coded to false. The handlers
are no-ops. There is no draft persistence, attachment state, queue count,
stop, command search, path search, model/lane menu, or connection action.

#### Omega target

Replicate T3's collapsed/expanded shell and action truth, with Omega-specific
controls:

- primary action: Send, Queue, or Stop;
- secondary lane action only when the selected lane declares it, such as Steer;
- attachment;
- lane and model disclosure, not a provider-heavy main UI;
- authority summary;
- optional Plan mode;
- visible connection and queue state.

Every button must be driven by an explicit `ComposerCapability` projection.
Never render Steer merely because a thread is running.

### Screen 7: approvals, questions, and plans

#### T3 structure

Pending interactions sit directly above the sticky composer. They remain in
the user's action locus and do not disappear into the historical feed.

An approval card shows:

- `Approval needed`;
- request kind and detail;
- Allow once;
- Allow session;
- Decline.

A user-input card supports:

- several questions;
- typed option chips;
- custom free-text answers;
- completion validation;
- one submit action for the complete response.

T3's current implementation has an accessibility defect worth not copying:
several card pressables and option chips do not declare complete roles and
labels. Omega should preserve the layout while making the semantics explicit.

#### Omega now

Omega projects no approval, question, or plan request. A blocked agent can look
like stale or waiting text. This is the highest-value controller gap.

#### Omega target

Create an Attention Inbox before broad file or terminal work. Each interaction
must bind:

```text
requestId
hostId
workItemId
lane
generation
requestKind
safeContext
allowedResponses
createdAt
expiresAt
```

The answer command must be idempotent. A reconnect that replays the request must
show the already-terminal response rather than permit a second answer.

### Screen 8: files and attachments

#### T3 structure

The file browser supports:

- hierarchical expansion;
- sensible default expansion;
- search;
- pull to refresh;
- selected ancestor treatment;
- immediate optimistic selection for about one second;
- preload on `onPressIn`;
- initial render of 20 rows, batches of 12, and a window size of 5.

On a phone, the tree and file are routes. On a tablet, the file can occupy the
inspector. The same selected path drives both.

The file viewer chooses a presentation by content:

- source code with line numbers, syntax highlighting, line target, and
  word-wrap preference;
- Markdown using selectable native or Nitro rendering;
- image with cached preview and full-screen zoom;
- HTML, SVG, and web content through WebView;
- preview/source toggle where appropriate;
- copy path, refresh, and open externally.

Reads are bounded. Large content is truncated with a visible banner; the
current source path uses a one-megabyte first-read ceiling.

#### Omega now

Omega has no mobile file reference or file route. The bridge intentionally
withholds file paths.

#### Omega target

Define an opaque `ArtifactRef`:

```text
artifactId + workItemId + workspaceGeneration + mediaType + displayName
```

Opening it should issue a scoped read command. The host resolves the path after
checking the device grant and workspace generation. Start with bounded text,
Markdown, and image. Add tree search only after the host can return scoped
children without leaking parent paths.

### Screen 9: review and comments

#### T3 structure

T3 Review groups changes into:

- working tree;
- branch changes;
- latest turn;
- previous turns.

The native review surface provides file navigation, statistics, pull to
refresh, collapse, mark viewed, and visible-file synchronization.

Line-comment selection works as follows:

1. tap one line for a single-line selection;
2. long press to establish a range anchor;
3. tap the range end;
4. use the floating selection bar to comment;
5. review up to five selected preview lines in a sheet;
6. add text and optional image attachments.

The result is serialized into a structured `<review_comment>` block containing
section, file path, range, and diff context, then inserted into the thread
draft. It is agent feedback, not a direct GitHub review comment.

Large diffs and non-text files have explicit fallback paths. T3's native diff
modules are substantial—roughly 2,575 lines on iOS and 1,429 on Android—because
selection, virtualization, syntax color, and canvas drawing are expensive.

#### Omega now

Omega mobile has no diff projection. Omega desktop already has a native diff
model and review UI, which is an important advantage.

#### Omega target

Do not recreate Git diff authority in the phone. Project bounded hunks from the
desktop source of truth:

```text
DiffRef
├── workItemId
├── checkpointId
├── workspaceGeneration
├── files[]
│   ├── opaqueFileRef
│   ├── status
│   ├── additions/deletions
│   └── boundedHunks[]
└── truncation
```

Ship read-only review first. Review comments can then become typed thread
attachments. Accept/reject or apply actions must bind the checkpoint and fail
closed if the diff has changed.

### Screen 10: Git and forge

#### T3 structure

The Git overview shows:

- repository and current ref;
- changed-file count;
- ahead and behind counts;
- upstream state;
- pull request state;
- typed quick action;
- working-tree file selection;
- branches and worktrees.

The quick action is derived, not hard-coded:

- changes → Commit, Commit & push, or Commit, push & PR;
- behind → Pull;
- ahead → Push or Push & create PR;
- open PR and clean → View PR;
- diverged → disabled Sync branch with a reason;
- detached or missing remote → disabled with an exact reason.

The commit sheet supports:

- optional generated commit message;
- file inclusion/exclusion;
- commit on the current branch;
- commit on a new branch.

Pushing or opening a pull request from the default branch requires a
confirmation screen. The user can continue or create a feature branch and
continue. Branch management can create or switch refs and create worktrees.
Refs checked out elsewhere are disabled.

All Git operations run on the selected server environment and exact cwd. The
phone never owns a local clone. Progress stages and terminal success/failure
are explicit.

#### Omega now

Omega mobile has no Git projection or command. Omega desktop has deep inherited
Git support, but zero base and the device bridge do not expose a typed remote
workflow.

#### Omega target

Implement `GitStatus` before mutation. Then add:

1. Commit selected files;
2. Push;
3. Pull;
4. Create or open pull request;
5. Branch/worktree creation.

Use T3's derived action and disabled-reason pattern. Add Omega's stricter
authority disclosure and dirty-worktree guard. Every mutation should show host,
repository, branch, worktree, and command receipt.

### Screen 11: terminal and task output

#### T3 structure

T3's terminal is a real remote terminal:

- native Ghostty-derived renderer on iOS and Android;
- text fallback;
- environment, thread, terminal ID, cwd, and worktree binding;
- default 80 × 24 grid before measurement;
- resize commands from native surface dimensions;
- multiple sessions per thread;
- project scripts that launch into terminal sessions;
- attach buffer replay;
- running, exited, and stale-attach state;
- stale session reopening;
- close and back navigation when the remote process exits;
- hardware-key handling;
- an accessory row for Escape, modifier, Tab, Clear, arrows, and common shell
  characters.

The terminal route waits for environment readiness, font preferences, grid
measurement, and any pending launch before attaching. Buffer replay is keyed by
terminal and font so a stale render cannot overwrite a new session.

The native terminal is about 2,000 lines across its larger platform view files,
plus a bridge and frame model. This is a product subsystem, not a text area.

#### Omega now

Omega mobile has no terminal or task-output route. Full Auto rows can expose
receipt references, but not live logs.

#### Omega target

Use a staged path:

1. bounded structured task output with reconnect and copy;
2. typed one-shot command requests where policy allows;
3. read/write pseudo-terminal sessions;
4. native renderer only after profiling proves it necessary.

The session contract must exist before the native view:

```text
terminalId + hostId + workItemId + cwdRef + dimensions + sequence + state
```

Secret entry, resize ordering, reconnect replay, background suspension, and
screen-reader fallback are acceptance requirements, not follow-up polish.

### Screen 12: settings and client storage

#### T3 structure

Settings includes:

- account and optional T3 Connect cloud access;
- environments;
- device notifications;
- Live Activity updates;
- project grouping;
- appearance;
- device-local beta flags;
- archived threads;
- client storage;
- legal documents;
- app version and running bundle identity.

Appearance provides live previews and independent controls for:

- base text size;
- terminal font size;
- code font size;
- code word wrapping.

The app follows automatic light/dark appearance and updates CSS variables for
both themes so switching is immediate.

Preferences use optimistic patches over a persistent store. Writes made while
the initial read is still loading are versioned so the late read cannot
overwrite the user's change. SQLite is primary; SecureStore is a timestamped
fallback and migration source.

The version row distinguishes embedded JavaScript from an OTA bundle. Check for
Updates handles check, download, rollback-to-embedded, restart, and typed
failure. The runtime version uses Expo fingerprinting so JavaScript requiring a
new native module cannot land on an incompatible binary.

#### Omega now

Omega has a coherent dark palette and reusable basic components, but no
settings route, automatic light theme, typography controls, connection
management, grant UI, storage diagnostics, or installed-bundle disclosure.
Expo Updates and several platform dependencies are installed without a
comparable mounted product flow.

#### Omega target

Initial Settings groups should be:

- Environments and Devices;
- Appearance;
- Notifications;
- Storage and Offline Queue;
- Authority;
- Version and Updates.

Keep security truth visible: a paired host row should show device-key identity,
grant scope, expiry, and revoke. Do not hide authority under a generic account
screen.

### Screen 13: notifications, Live Activities, shares, and shortcuts

#### Notifications and deep links

T3 registers the device only when platform permission and relay registration
both succeed. The Settings switch does not claim notifications are active
merely because iOS permission was granted.

Notification payloads accept only exact thread destinations. An explicit deep
link is normalized, queries/fragments and protocol-relative paths are refused,
and environment/thread IDs are a fallback. Cold-start and live responses share
one route function and are deduplicated by notification ID.

#### Live Activities and widgets

The iOS Agent Activity surface aggregates several threads. It prioritizes:

1. approval or input;
2. failure;
3. running or starting;
4. completed or stale.

It uses semantic system foreground colors plus consistent state tints. The
compact, expanded, banner, watch, and widget presentations all derive from the
same phase model. Tapping opens the highest-priority exact thread.

#### Incoming shares

The system share target accepts text, URL, and up to eight images. Shared
content is converted into a durable draft before temporary files are removed.
Images are bounded by provider count and 10 MB size. Duplicate lifecycle
deliveries are fingerprinted and serialized. A draft can reserve a destination
project, survive sheet dismissal, and resume without importing twice.

#### Launcher shortcuts

Android exposes one static New Task shortcut and up to three recent threads.
Shortcut paths are allowlisted to exact New Task or environment/thread routes.
Persisted malformed or stale launcher data cannot navigate arbitrarily.

#### Omega now and target

Omega has Expo notification and update packages but no complete attention
route. It has no Live Activity, widget, share target, or shortcut product flow.

Implement in this order:

1. exact host/work-item/request deep links;
2. approval, input, failure, and completion notifications;
3. New Task and recent-work shortcuts;
4. incoming text/image share to a durable task draft;
5. Live Activity only when concurrent long-running work warrants it.

Do not build a Live Activity before the app can open and answer the request it
advertises.

### Offline, reconnect, and synchronization law

T3 treats three different problems separately:

1. **Connection supervision** establishes and retries an environment session.
2. **Shell synchronization** keeps lightweight projects and thread rows
   authoritative.
3. **Outbox delivery** stores commands accepted by the phone but not yet by the
   environment.

The outbox stores one JSON file per queued message. Messages are grouped by
environment/thread and ordered by creation time. Only the first message for a
thread drains. Only one message globally is marked dispatching at once in the
current mobile hook.

Delivery rules prevent duplication:

| Queued item             | Server state                  | Action                                     |
| ----------------------- | ----------------------------- | ------------------------------------------ |
| New task                | thread already exists         | remove local item; creation already landed |
| New task                | connected and shell is live   | send                                       |
| New task                | shell not yet authoritative   | wait                                       |
| Existing-thread message | thread absent, shell live     | remove; target is gone                     |
| Existing-thread message | thread absent, shell not live | wait                                       |
| Existing-thread message | connected and thread idle     | send                                       |
| Existing-thread message | busy or disconnected          | wait                                       |

Before a queued existing-thread message sends, T3 synchronizes model, runtime,
and interaction settings. A queued creation must have a prompt, model, and a
branch when worktree mode requires one. Transport failures and interruptions
retry with 1–16 second exponential backoff. Terminal command failures can be
discarded rather than retried forever. Delivered items are removed only after
the start-turn command reaches a terminal result.

Omega's resume cursor solves a different problem: replaying server-to-phone
state. It does not accept offline user intent. Omega needs both:

- bridge resume for observations;
- a signed intent outbox for commands.

An Omega outbox entry should include command ID, host and work-item identity,
target generation, grant reference, payload digest, expiry, creation time,
delivery state, and terminal receipt. Reconnect must revalidate grant,
generation, target existence, lane capability, and current request state before
delivery.

### State ownership and component boundaries

T3's useful architectural lesson is separation of state authority:

| Layer                    | Owns                                                                                 |
| ------------------------ | ------------------------------------------------------------------------------------ |
| Shared client runtime    | connection catalog, environment sessions, shell/detail projections, typed operations |
| Mobile state             | drafts, outbox, selected route, preferences, recent shortcuts, incoming shares       |
| React feature components | presentation and user intent                                                         |
| Native modules           | rendering or platform interaction that React Native cannot perform smoothly enough   |
| Server environment       | threads, files, Git, terminal processes, commands, durable work truth                |

Omega should mirror that separation without adopting T3's exact libraries.
Recommended feature topology:

```text
src/
├── app/
│   ├── root-navigator
│   ├── adaptive-workspace
│   └── app-providers
├── controller/
│   ├── environment-catalog
│   ├── connection-supervisor
│   ├── work-index
│   ├── thread-detail
│   ├── command-client
│   └── receipt-store
├── persistence/
│   ├── preferences
│   ├── composer-drafts
│   ├── command-outbox
│   └── recent-destinations
├── features/
│   ├── environments
│   ├── work-list
│   ├── new-task
│   ├── thread
│   ├── attention
│   ├── artifacts
│   ├── review
│   ├── git
│   ├── output
│   └── settings
└── ui/
    ├── tokens
    ├── typography
    ├── controls
    └── platform-header
```

This is a logical map, not a requirement to create many tiny files. New modules
should be introduced only when the responsibility is real.

### Recommended Omega screen and component map

| Route        | Primary components                                                               | Backing state or command                          |
| ------------ | -------------------------------------------------------------------------------- | ------------------------------------------------- |
| Home         | `WorkListHeader`, `AttentionSummary`, `WorkList`, `QueuedTaskRow`                | aggregate work index, filters, outbox             |
| Environments | `EnvironmentRow`, `ConnectionDiagnostics`, `GrantActions`                        | host catalog and connection supervisors           |
| New Task     | `ProjectPicker`, `TaskDraft`, `WorkspaceModePicker`, `AuthoritySummary`          | project projection, drafts, create command        |
| Thread       | `ThreadHeader`, `PortableEventList`, `PendingInteractionStack`, `ThreadComposer` | thread detail projection and command capabilities |
| Artifact     | `ArtifactHeader`, text/Markdown/image viewer                                     | scoped artifact read                              |
| Review       | `ReviewSummary`, `DiffFileList`, `DiffHunk`, `ReviewDraft`                       | checkpoint-bound diff projection                  |
| Git          | `GitStatusHeader`, `DerivedGitAction`, `ChangedFiles`, `BranchList`              | typed Git queries and commands                    |
| Output       | `OutputSessionHeader`, `OutputList`                                              | bounded task-output session                       |
| Settings     | environment, appearance, authority, storage, update sections                     | preferences, grants, app metadata                 |

### Visual system to bring closer to T3

Omega should move substantially closer to T3 Mobile's shell and density:

- automatic light and dark semantic tokens;
- one display family and one monospaced code family;
- large native page titles where the platform supports them;
- 20–24 point grouped sheet/card radii;
- 14–20 point horizontal screen padding;
- 44 point minimum controls;
- rich cards only for active or attention work;
- compact historical rows with inset separators;
- unboxed assistant messages;
- right-aligned user bubbles;
- sticky collapsed/expanded composer;
- semantic state colors shared by list, request cards, notification, and
  optional Live Activity;
- native-feeling menus and headers instead of custom icon rows on iOS;
- a floating New Task action on compact Android.

Copying T3 closely does not require copying every glass material. Liquid Glass
is a progressive platform treatment, not the information architecture. Omega
should implement an opaque tokenized fallback first and add platform glass only
when contrast, reduced transparency, and older OS behavior are verified.

### Accessibility audit

#### T3 strengths

- native navigation and search semantics on iOS;
- many header buttons have accessibility labels;
- platform text scaling preferences;
- 44-point controls are common;
- external routes have defensive validation;
- user-visible disabled reasons exist for Git operations.

#### T3 gaps not to copy

- several approval and question controls lack explicit roles or complete
  labels;
- some diff and Git affordances rely on icon meaning;
- terminal screen-reader behavior remains a special-case risk;
- swipe lifecycle actions require the long-press/menu equivalent;
- animation paths need explicit reduced-motion verification;
- color-coded states need text and icon redundancy.

#### Omega acceptance bar

Every new control needs role, label, state, and hint where behavior is not
obvious. Dynamic work rows should announce state changes without repeatedly
reading streaming tokens. Request focus should move to the pending card only
when it will not interrupt typing. Diff, code, and terminal views need a text
fallback. All swipe actions need discoverable menu alternatives.

### Performance audit

T3 invests in the correct hotspots:

- virtualized thread and transcript lists;
- item typing and estimates;
- focused-route inspector registration;
- feed freeze during disclosure changes;
- preloading on file press;
- bounded initial tree rendering;
- native Markdown, diff, composer, and terminal only where profiling justified
  the bridge cost;
- stale-animation suppression;
- content-width freezing during pane animation;
- bounded file reads and large-diff fallbacks.

Omega's bounded mirror and `FlatList` are good foundations. The current
performance profile is easy because the product is small. As it grows, Omega
should preserve these budgets:

- never rerender the whole transcript for a heartbeat;
- normalize entities by stable ID;
- append or patch portable events rather than replace all thread detail;
- keep connection state separate from message data;
- measure and anchor feed movement explicitly;
- cap Markdown, code, diff, image, and log payloads;
- add native surfaces only after a JS implementation fails a measured target.

### Testing and release proof

T3 has 92 app-local tests and 132 tests across mobile plus the shared client
runtime at the audited revision. Tests cover connection onboarding,
environment sections, thread settlement and snoozing, shell sync, outbox
delivery, drafts, incoming shares, notification routing, shortcuts, storage,
Git state, and showcase support.

Its showcase harness can:

- seed deterministic environments, projects, threads, terminal, and pending
  tasks;
- launch iOS simulators and Android emulators;
- capture named scenes in light and dark appearance;
- normalize PNG output;
- validate App Store and Google Play dimensions, color type, alpha, count, and
  file-size constraints.

Omega's four mobile tests concentrate on bridge, key custody, screen behavior,
and view behavior. That is reasonable for the current mirror, but not for the
target controller.

Before visual parity work is called complete, Omega needs deterministic fixtures
for at least:

- no hosts;
- host connecting, offline, refused, expired, and live;
- empty work list;
- active, approval, input, failed, queued, and settled rows;
- streaming thread;
- approval and multi-question cards;
- offline send and replay;
- bounded Markdown/code;
- truncated file and diff;
- tablet list/detail/inspector;
- light, dark, large text, reduced motion, and reduced transparency.

### Exact capability gap

| Capability            | T3 current                                            | Omega current                        | Omega closure                       |
| --------------------- | ----------------------------------------------------- | ------------------------------------ | ----------------------------------- |
| Multiple environments | catalog and independent supervisors                   | one implicit paired desktop          | durable host catalog                |
| Project discovery     | server shell projection and repository grouping       | none                                 | scoped workspace projection         |
| New task              | project/worktree/model/runtime flow                   | none                                 | typed create command                |
| Durable drafts        | file-backed per-project drafts                        | in-memory thread text only           | persistent draft store              |
| Work list             | active cards, settled tail, search, filters, gestures | generic activity cards               | aggregate typed work index          |
| Thread route          | exact environment/thread deep link                    | local selection only                 | canonical route identity            |
| Transcript            | Markdown, code, work log, images, review comments     | plain text bubbles                   | portable bounded event grammar      |
| Send/queue/stop       | operational and state-derived                         | Send/Steer disabled                  | admitted command client             |
| Approvals/input       | action cards                                          | absent                               | idempotent attention commands       |
| Files                 | tree, search, viewers                                 | absent                               | opaque artifact references          |
| Review                | native diff and review draft                          | absent                               | checkpoint-bound bounded diff       |
| Git                   | status, derived actions, branch/worktree, PR          | absent                               | typed remote Git commands           |
| Terminal              | multi-session native terminal                         | absent                               | output first, terminal later        |
| Offline               | persistent outbox and retry law                       | observation resume only              | signed intent outbox                |
| Notifications         | exact thread routing and device registration truth    | absent flow                          | request-aware push routing          |
| Shares                | durable text/image task import                        | absent                               | share-to-draft                      |
| Shortcuts             | New Task and recents                                  | absent                               | route allowlist and recents         |
| Tablet                | list/detail/inspector                                 | single column                        | adaptive workspace                  |
| Appearance            | automatic themes and font controls                    | dark-only tokens                     | semantic themes and scaling         |
| Updates               | fingerprinted OTA with manual control                 | dependency present, no comparable UI | installed-bundle truth and rollback |

### Replication sequence

#### Mobile phase 0: controller contracts

Define environment, work item, portable event, request, artifact, command,
receipt, and capability schemas. Add route identity and durable host catalog.

Exit criteria:

- the list can name every projected lane without guessing;
- every control is driven by a capability;
- one host can be forgotten and one grant can be revoked independently;
- a work item survives route serialization and cold start.

#### Mobile phase 1: T3-like shell

Add native-stack navigation, automatic themes, Home v2, Thread shell,
platform headers, search, active/settled row grammar, and compact/split layout.
The composer may remain read-only if it truthfully says so.

Exit criteria:

- phone and tablet fixtures share one route model;
- selected work remains stable while sheets open;
- active list order does not jump on updates;
- blocked work is visually dominant without decorative noise.

#### Mobile phase 2: attention and basic command loop

Add durable drafts, Send/Queue/Stop, approval, question, interrupt, command
admission, terminal receipt, and signed offline outbox.

Exit criteria:

- a paired owner can answer a blocked request;
- replay cannot answer twice;
- offline intent survives restart;
- stale host, grant, request, or generation refuses clearly;
- each admitted command reaches a visible terminal receipt.

#### Mobile phase 3: evidence

Add safe work events, plans, attachments, scoped files, task output, and
read-only diffs.

Exit criteria:

- the user can understand why an approval is requested;
- no raw secret, absolute path, or unrestricted tool payload crosses the
  bridge;
- content truncation is explicit;
- deep links open the exact artifact or request.

#### Mobile phase 4: typed workbench mutations

Add new-task/worktree flow, review comments, Git status and actions, share
target, shortcuts, and notifications.

Exit criteria:

- mutations bind exact host, workspace, generation, and checkpoint;
- disabled Git actions explain why;
- shared content becomes one durable draft exactly once;
- notifications open the actionable object.

#### Mobile phase 5: terminal and platform finish

Add full terminal only if task output is insufficient, then Live Activities,
widgets, OTA UI, and store-grade screenshot/accessibility automation.

Exit criteria:

- terminal resize and replay are ordered;
- background and reconnect behavior is tested;
- a text fallback exists;
- platform surfaces use the same attention state and deep-link contract.

### What Omega should and should not copy

Copy closely:

- the phone and tablet route topology;
- Thread List v2's active-card and settled-row hierarchy;
- stable active ordering;
- semantic state precedence and shared colors;
- sticky collapsed/expanded composer;
- user-bubble and unboxed-assistant grammar;
- interaction cards above the composer;
- typed File, Review, Git, and Terminal destinations;
- offline queued task presentation;
- native platform headers and search;
- exact deep links;
- deterministic showcase fixtures.

Adapt:

- T3 environment → Omega paired host plus execution lane;
- T3 provider/runtime menus → Omega lane, model disclosure, and authority;
- T3 server event store → Omega aggregate projection over existing native
  lanes;
- T3 Git command service → Omega desktop Git authority with signed receipts;
- T3 review comments → Omega checkpoint-bound portable review attachments.

Do not copy:

- unsafe full-access defaults;
- raw server paths or tool payloads on mobile;
- controls with missing accessibility semantics;
- glass as a prerequisite for hierarchy;
- native modules before profiling;
- a generic terminal as the first remote mutation surface;
- implicit queue or steer behavior.

### Current T3 mobile source map

The deep dive is pinned to
[`a148e08197fc38b24e59c10c7cd5ba06dd182dab`](https://github.com/pingdotgg/t3code/tree/a148e08197fc38b24e59c10c7cd5ba06dd182dab).
The most important sources are:

- [root app providers](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/App.tsx)
- [route graph](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/Stack.tsx)
- [adaptive workspace](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/layout/AdaptiveWorkspaceLayout.tsx)
- [Home](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/home/HomeScreen.tsx)
- [Thread List v2 rows](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/threads/thread-list-v2-items.tsx)
- [thread route](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/threads/ThreadRouteScreen.tsx)
- [thread detail](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/threads/ThreadDetailScreen.tsx)
- [thread feed](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/threads/ThreadFeed.tsx)
- [composer](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/threads/ThreadComposer.tsx)
- [new-task flow](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/threads/new-task-flow-provider.tsx)
- [persistent outbox](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/state/use-thread-outbox-drain.ts)
- [files](https://github.com/pingdotgg/t3code/tree/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/files)
- [review](https://github.com/pingdotgg/t3code/tree/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/review)
- [Git actions](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/threads/ThreadGitControls.tsx)
- [terminal](https://github.com/pingdotgg/t3code/tree/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/terminal)
- [environment connection supervisor](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/packages/client-runtime/src/connection/supervisor.ts)
- [notifications](https://github.com/pingdotgg/t3code/tree/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/agent-awareness)
- [incoming shares](https://github.com/pingdotgg/t3code/tree/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/sharing)
- [settings](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/src/features/settings/SettingsRouteScreen.tsx)
- [native modules](https://github.com/pingdotgg/t3code/tree/a148e08197fc38b24e59c10c7cd5ba06dd182dab/apps/mobile/modules)
- [showcase capture harness](https://github.com/pingdotgg/t3code/blob/a148e08197fc38b24e59c10c7cd5ba06dd182dab/scripts/mobile-showcase.ts)

## Cross-device workflow comparison

| Workflow               | T3 Code                | Omega now                  | Omega gap                        |
| ---------------------- | ---------------------- | -------------------------- | -------------------------------- |
| Pair a phone           | Complete               | Complete for one host      | Add host catalog and grant UI    |
| Find an environment    | Complete               | Missing                    | Define host and endpoint objects |
| Find a project         | Complete               | Missing                    | Project projection               |
| Open a thread          | Complete               | Partial                    | Bounded recent mirror            |
| Create a thread        | Complete               | Missing                    | Typed create command             |
| Read full activity     | Complete               | Missing                    | Portable event allowlist         |
| Send a message         | Complete               | Disabled in current app    | Wire signed command admission    |
| Steer active work      | Lane-aware             | Disabled in current app    | Declare per-lane behavior        |
| Interrupt              | Complete               | Command design only        | Wire and receipt                 |
| Answer a question      | Complete               | Missing                    | Attention event                  |
| Approve a tool         | Complete               | Missing                    | Scoped authority response        |
| Inspect a plan         | Complete               | Missing                    | Plan projection                  |
| Browse files           | Complete               | Missing                    | Opaque file references           |
| Review a diff          | Complete               | Missing                    | Diff generation contract         |
| Use Git                | Complete               | Missing                    | Typed Git commands               |
| Open terminal          | Complete               | Missing                    | Task output, then terminal       |
| Use browser preview    | Complete               | Missing                    | Preview session artifact         |
| Queue while offline    | Complete               | Missing                    | Signed offline outbox            |
| Receive a notification | Complete               | Missing                    | Push and exact deep link         |
| Watch Full Auto        | General run projection | Partial and Omega-specific | Normalize run summary            |

## Security and authority comparison

### T3 Code strengths

- Scoped environment credentials.
- Token exchange and DPoP.
- Separate access and launch models.
- Hardened Electron defaults.
- Typed durable commands and events.

### T3 Code weaknesses

The T3 teardown found a dangerous default posture. Some paths use full access,
no approval, and a danger-full-access sandbox mode. T3 Code does not supply a
separate process sandbox. It also lacks a complete user authority manifest and
cross-host session portability.

### Omega strengths

- Identity is part of first run.
- Device grants use signed Nostr keys.
- Pairing secrets are short-lived and one-use.
- Device frames and projections have strict bounds.
- Full Auto has admission, health, receipt, and release evidence.
- Queue and steer behavior is explicit by lane.
- Destructive Git commands have an extra dirty-worktree guard.
- Hosted OpenAgents authentication does not add a browser callback surface.

### Omega weaknesses

Omega does not yet have one effective authority view across native tools, ACP
agents, Full Auto, the bridge, and Nostr commands.

The native agent can use a broad unattended tool default. Omega has hard-coded
catastrophic command denials and a Git guard, but those controls are not a
general process sandbox.

Mobile command authority is present in design and protocol work, but absent from
the current mounted mobile flow. This split makes it hard for a user to know
what the paired phone can do.

### Required policy

Every thread and run should expose one effective authority record:

- Host and workspace.
- Execution lane.
- Model class.
- Tool profile.
- File and network scope.
- Approval policy.
- Remote device grants.
- Queue and steer behavior.
- Current generation.
- Revocation and receipt references.

This record must be a projection of enforced state. It must not be descriptive
UI text.

## UI quality audit

This is a source and visual-baseline audit. It is not an installed accessibility
test.

### Omega desktop

| Area              | Score     | Finding                                                                                                                    |
| ----------------- | --------- | -------------------------------------------------------------------------------------------------------------------------- |
| Accessibility     | 3/4       | Strong keyboard and focus model. Structured buttons and actions are common. Installed screen-reader proof is incomplete.   |
| Performance       | 4/4       | Native rendering, virtual transcript list, bounded previews, and background work are strong.                               |
| Responsive layout | 3/4       | Wide and narrow zero-base baselines exist. The default shell yields space well. A tablet-like middle width has less proof. |
| Theming           | 4/4       | The UI uses Zed theme tokens and supports the selected theme. It avoids a separate hard-coded brand palette.               |
| Visual discipline | 4/4       | The primary surface is quiet and direct. It avoids glass, gradients, oversized titles, and decorative motion.              |
| **Total**         | **18/20** | The main risk is installed accessibility proof, not visual quality.                                                        |

The strongest visual choice is restraint. The thread owns the window. Tool
output and status use secondary emphasis. The interface does not imitate a
marketing dashboard.

The main visual weakness is discoverability. The shell hides capable native
surfaces until the user leaves zero base or opens an artifact. T3 Code is more
visually complex, but it shows the complete workbench.

### Omega mobile

| Area              | Score     | Finding                                                                                                         |
| ----------------- | --------- | --------------------------------------------------------------------------------------------------------------- |
| Accessibility     | 3/4       | Controls have useful labels and mobile-size targets. Complete screen-reader and reduced-motion proof is absent. |
| Performance       | 3/4       | `FlatList` and bounded projections are suitable. Connection and rendering work is small.                        |
| Responsive layout | 2/4       | The phone flow is clear. There is no adaptive tablet workbench.                                                 |
| Theming           | 2/4       | The dark token set is coherent. There is no complete light-theme path.                                          |
| Visual discipline | 3/4       | The app is direct and avoids decorative effects. It relies heavily on generic cards and badges.                 |
| **Total**         | **13/20** | The UI foundation is sound, but it supports only a mirror.                                                      |

The mobile design is calm and legible, but its generic cards do not yet express
controller priority. It should adopt T3's active-card, compact-history,
unboxed-assistant, and sticky-composer hierarchy while keeping Omega's
authority and receipt language.

### T3 Code context

The prior broad T3 UI audit scored 13/20. The current mobile app has advanced
materially since that snapshot. A source-only audit of current mobile scores it
17/20:

| Area              |     Score | Current mobile finding                                                                                                                |
| ----------------- | --------: | ------------------------------------------------------------------------------------------------------------------------------------- |
| Accessibility     |       2/4 | Native headers and many labels are good; request chips, diff actions, terminal semantics, and reduced-motion proof remain incomplete. |
| Performance       |       4/4 | Virtualized lists, anchoring law, bounded reads, adaptive portals, native hot paths, and fallbacks are unusually thorough.            |
| Responsive layout |       4/4 | Phone routes, persistent tablet sidebar, and optional inspector use explicit usable-width constraints.                                |
| Theming           |       4/4 | Automatic light/dark themes, semantic tokens, font controls, and native materials have defined fallbacks.                             |
| Visual discipline |       3/4 | The mobile hierarchy is strong and close-copyable; some glass and dense menus add complexity without changing the work model.         |
| **Total**         | **17/20** | The main risk is accessibility completion, not information architecture.                                                              |

T3 Mobile should be both a feature and structural visual reference for Omega's
new mobile shell. Omega should closely adopt its navigation, density,
active-versus-settled rows, message grammar, interaction cards, and adaptive
workbench. It should not copy missing accessibility semantics, unsafe authority
defaults, or glass effects without tested fallbacks.

## Omega strengths to preserve

### 1. Native editor depth

Do not replace GPUI, the Zed project model, or the editor buffer model with a
web workbench. Expose the existing native objects through a clearer shell.

### 2. One visible Omega identity

Do not put a provider matrix back into the main composer. Show execution truth
in a compact detail surface.

### 3. Explicit queue and steer behavior

Do not adopt implicit second-send behavior. Keep lane-specific admission and
refusal.

### 4. Signed device grants

Do not weaken the pairing model to get faster mobile control. Add commands to
the same grant and receipt system.

### 5. Bounded mobile projections

Do not stream raw tool payloads or credentials to the phone. Add typed and
bounded event summaries.

### 6. Full Auto evidence

Do not merge Full Auto into the normal composer if that removes its supervisor,
lease, health, handoff, and receipt rules.

### 7. Quiet visual language

Copy T3 Mobile's hierarchy and native platform grammar closely. Keep Omega's
sparse product identity by treating glass and motion as progressive materials,
not as the source of structure. State, navigation, and information density
should carry the design.

## Prioritized gap ledger

### P0: Make control truth coherent

| ID         | Gap                                         | Required outcome                                                                          |
| ---------- | ------------------------------------------- | ----------------------------------------------------------------------------------------- |
| O-T3-P0-01 | No aggregate work projection                | One index covers native, ACP, terminal, and Full Auto work with stable source references. |
| O-T3-P0-02 | No effective authority record               | Desktop and mobile show enforced lane, tool, workspace, approval, and device scope.       |
| O-T3-P0-03 | Current mobile commands are disabled        | Mobile can send to an idle native thread and receives admission and terminal receipts.    |
| O-T3-P0-04 | Mobile cannot answer blocking requests      | Mobile has an attention inbox for questions and approvals.                                |
| O-T3-P0-05 | Queue restore is incomplete                 | Desktop and mobile rebuild durable queue rows after restart.                              |
| O-T3-P0-06 | Direct bridge and command path are separate | One host projection explains both read and command transport without merging authority.   |
| O-T3-P0-07 | Mobile architecture documents are stale     | The README describes the mounted plain React Native app and its real capability boundary. |

### P1: Complete the desktop workbench

| ID         | Gap                                        | Required outcome                                                                      |
| ---------- | ------------------------------------------ | ------------------------------------------------------------------------------------- |
| O-T3-P1-01 | Zero base hides key work surfaces          | Files, Search, Review, Git, Terminal, Plan, and Preview are normal thread activities. |
| O-T3-P1-02 | Worktree state is implicit                 | Each thread shows repository, branch, worktree, owner, and cleanup policy.            |
| O-T3-P1-03 | Artifacts are hard to rediscover           | Each thread has a bounded artifact and event outline.                                 |
| O-T3-P1-04 | Git and forge are outside the primary flow | Zero base shows status and common actions. Pull-request work has typed results.       |
| O-T3-P1-05 | Preview has no common artifact model       | A preview session has identity, owner, URL, lifecycle, and automation scope.          |
| O-T3-P1-06 | Settings do not show total authority       | Authority and Devices settings support inspection and revocation.                     |

### P1: Build the minimum mobile controller

| ID         | Gap                           | Required outcome                                                       |
| ---------- | ----------------------------- | ---------------------------------------------------------------------- |
| O-T3-M1-01 | No host catalog               | Mobile can select, inspect, and revoke paired hosts.                   |
| O-T3-M1-02 | No project or thread creation | Mobile can select a project and create a bounded native thread.        |
| O-T3-M1-03 | Text-only transcript          | Mobile shows safe tool, plan, request, artifact, and change summaries. |
| O-T3-M1-04 | No interrupt                  | Mobile can interrupt supported lanes with a receipt.                   |
| O-T3-M1-05 | No file viewer                | Mobile can open scoped text and image file references.                 |
| O-T3-M1-06 | No diff review                | Mobile can inspect a diff that is bound to a checkpoint generation.    |

### P2: Add operational breadth

| ID         | Gap                                 | Required outcome                                                      |
| ---------- | ----------------------------------- | --------------------------------------------------------------------- |
| O-T3-P2-01 | No offline outbox                   | Signed intents survive disconnect and revalidate before delivery.     |
| O-T3-P2-02 | No exact mobile notifications       | Requests and completions open by stable deep link.                    |
| O-T3-P2-03 | No mobile Git controls              | Typed status, commit, push, and pull-request actions exist.           |
| O-T3-P2-04 | No mobile terminal                  | Bounded task output ships first. A full terminal follows proof.       |
| O-T3-P2-05 | No tablet workbench                 | Larger screens use an activity and detail split.                      |
| O-T3-P2-06 | Owned desktop release is one target | Each new target has package, install, update, and rollback evidence.  |
| O-T3-P2-07 | No session portability              | A supported session can reconnect from another authorized Omega host. |

## Recommended implementation order

### Wave 0: Freeze the portable truth model

Define the host, work item, event, interaction, authority, command, and receipt
contracts. Do not move execution authority into a new service.

Acceptance result:

- One read-only index can list every current work item.
- Each row names its source lane and generation.
- No row claims a control that the source lane does not support.

### Wave 1: Expose the existing desktop workbench

Add zero-base activities for Files, Search, Review, Git, Terminal, and Plan.
Reuse native Zed entities.

Acceptance result:

- A user can complete a normal coding review without leaving the Omega shell.
- Closing the last secondary surface restores the quiet agent layout.
- Narrow layouts keep the composer and active request usable.

### Wave 2: Ship the mobile attention loop

Add host selection, an attention inbox, send, and interrupt. Do not start with a
full mobile IDE.

Acceptance result:

- A paired owner can find a blocked thread.
- The owner can answer the exact request.
- The desktop records the mobile command and terminal receipt.
- Reconnect does not duplicate the answer.

### Wave 3: Add mobile evidence

Add safe tool summaries, plans, artifacts, files, and read-only diffs.

Acceptance result:

- The owner can understand why a request exists.
- A file or diff reference cannot escape the granted workspace.
- Large content remains bounded.

### Wave 4: Add typed mutation flows

Add diff decisions, Git actions, thread creation, and an offline outbox.

Acceptance result:

- Each mutation binds a host generation and target state.
- Stale commands refuse with a clear reason.
- Every admitted command has a terminal receipt.

### Wave 5: Expand distribution and device integration

Add notifications, deep links, tablet layout, Live Activities where useful, and
new desktop release targets.

Acceptance result:

- A notification opens the exact work item.
- Each release target has installed proof.
- Accessibility tests cover the complete route on each supported platform.

## Definition of meaningful T3 Code parity

Omega does not need visual or architectural parity with T3 Code. It needs
outcome parity for the control loop.

Desktop parity exists when a user can:

1. Select or create a project context.
2. Start isolated work.
3. Follow the complete agent event stream.
4. Answer requests.
5. Inspect files and diffs.
6. Use Git and terminal tools.
7. Resume work after restart.
8. Inspect effective authority.

Mobile parity exists when a user can:

1. Select an authorized Omega host.
2. Find active and blocked work.
3. Read enough evidence to make a decision.
4. Send, interrupt, approve, and answer.
5. Inspect scoped files and diffs.
6. Queue a signed intent while offline.
7. Return through an exact notification link.
8. See the final receipt.

Omega exceeds T3 Code in its target areas when:

1. The native editor remains the source of file and diff truth.
2. All remote commands use signed grants and explicit admission.
3. Queue and steer rules remain exact for each lane.
4. Full Auto keeps its supervisor and evidence chain.
5. The user can inspect and revoke effective authority.
6. Mobile projections remain typed, scoped, and bounded.

## Final assessment

Omega desktop is not a weaker version of T3 Code Desktop. It is a deeper native
IDE with an incomplete agent workbench composition.

Omega mobile is a weaker version of T3 Code Mobile today. Its secure pairing and
bounded projection are good foundations, but the product stops before control.

The highest-value move is a portable control projection. That projection should
not replace Omega's execution lanes. It should make their state, authority,
requests, and receipts legible on every authorized client.

If Omega completes that layer, it can combine three strengths that T3 Code does
not combine today:

- A native editor and language platform.
- A precise owner authority and receipt model.
- A complete desktop and mobile agent control loop.

## Omega source map

The desktop findings use these current Omega sources:

- `PRODUCT.md`
- `README.md`
- `OMEGA_DELTAS.md`
- `docs/src/ai/agent-panel.md`
- `docs/src/ai/omega-agent.md`
- `docs/src/development/omega-rc-release.md`
- `docs/src/development/omega-full-auto-gpui-launcher.md`
- `docs/src/development/omega-full-auto-reports-sync-mobile.md`
- `docs/src/development/omega-full-auto-routing-liveness.md`
- `docs/src/development/omega-issue31-mobile-host-adjunct.md`
- `crates/agent_ui/src/agent_panel.rs`
- `crates/agent_ui/src/conversation_view.rs`
- `crates/agent_ui/src/conversation_view/thread_view.rs`
- `crates/agent_ui/src/message_editor.rs`
- `crates/agent_ui/src/omega_host_bridge.rs`
- `crates/omega_device_bridge/src/omega_device_bridge.rs`
- `crates/omega_effectd/src/`
- `crates/zed/test_fixtures/visual_tests/omega_zero_base_wide.png`
- `crates/zed/test_fixtures/visual_tests/omega_zero_base_narrow.png`

The mobile findings use these committed sources:

- [`apps/openagents-mobile/src/app.tsx`](../../apps/openagents-mobile/src/app.tsx)
- [`apps/openagents-mobile/src/screens/omega-home-screen.tsx`](../../apps/openagents-mobile/src/screens/omega-home-screen.tsx)
- [`apps/openagents-mobile/src/screens/omega-home-view.tsx`](../../apps/openagents-mobile/src/screens/omega-home-view.tsx)
- [`apps/openagents-mobile/src/workroom/omega-device-bridge-client.ts`](../../apps/openagents-mobile/src/workroom/omega-device-bridge-client.ts)
- [`apps/openagents-mobile/src/workroom/issue31-device-key-vault.ts`](../../apps/openagents-mobile/src/workroom/issue31-device-key-vault.ts)
- [`apps/openagents-mobile/src/ui/theme.ts`](../../apps/openagents-mobile/src/ui/theme.ts)
