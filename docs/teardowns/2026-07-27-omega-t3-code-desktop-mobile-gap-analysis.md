# Omega and T3 Code desktop and mobile gap analysis — 2026-07-27

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
| T3 Code ACP teardown            | `bde0a4`                                   |
| T3 Code OpenCode release update | `fdca154`                                  |

The Omega desktop revision is the current `origin/main` revision from the Omega
repository. The Omega mobile revision is the committed `origin/main` revision
from this repository.

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
the event history. Reactors resume provider work from checkpoints. A runtime
receipt bus connects provider effects to the durable event stream.

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

## Mobile gap analysis

### Current Omega mobile product

The current committed mobile app is small by design. It uses plain React Native.
Its source has about 2,586 lines of TypeScript and TSX under `src/`.

The app has one mounted screen. It provides three main states:

1. An unpaired state with a desktop QR scan action.
2. An activity list with thread and Full Auto run cards.
3. A thread detail view with a bounded transcript preview.

The app securely stores a Nostr device key and a scoped grant. It verifies the
desktop connection and resumes a bounded delta stream. These are meaningful
foundations.

The mobile README still describes an Effect Native application with older
screens and shared view programs. The mounted app says that Effect Native is
gone. This documentation drift can cause false capability and architecture
claims.

The thread screen renders Send and Steer buttons. The current screen sets the
command lane to unavailable. Both handlers are no-ops. The input explains that
the signed command lane is unavailable.

This means the current app is an authenticated mirror. It is not a remote
controller.

### 1. Pairing and connection

#### Omega status: Strong

Desktop can show a one-use QR code. The pairing secret has a five-minute limit.
The protocol uses a Nostr device key and a scoped grant. Discovery can advertise
a structured Tailscale MagicDNS endpoint.

The direct bridge supports loopback and Tailscale. It has bounded frames,
snapshots, ordered deltas, resume cursors, and server generations.

T3 Code supports QR pairing, environment credentials, token exchange, and DPoP.
It also supports a broader endpoint catalog and relay model.

#### Gap

Omega pairing is strong for one desktop. It does not yet provide a mobile host
catalog, grant management UI, connection diagnostics, or a multi-host switcher.

### 2. Activity and thread list

#### Omega status: Partial

The list can show recent agent threads and Full Auto runs. Thread cards include
state, executor, model, time, and the latest transcript preview. Run cards
include state and health information.

The projection is bounded to 64 threads and 64 Full Auto runs. This limit is
reasonable for a first activity view.

T3 Code Mobile can navigate environments, projects, and threads. It can create a
project thread and control its worktree context.

#### Gap

- Omega has no environment list.
- Omega has no project list.
- Omega cannot create a thread from mobile.
- Omega cannot filter or search activity.
- Omega has no unread or attention queue.
- Omega does not distinguish requests that need an answer.

### 3. Transcript

#### Omega status: Partial

The device mirror sends bounded user and assistant text. It excludes tool calls,
raw payloads, credentials, and file paths. This rule reduces leakage risk.

The current mobile view also has generic roles for system and tool messages.
The committed bridge does not project complete events for those roles.

T3 Code Mobile renders Markdown, code, tool calls, questions, approvals, plans,
diffs, and attachments. It uses native modules for high-density Markdown,
composer, controls, diff, and terminal work.

#### Gap

Omega mobile cannot explain what an agent did. It can show the agent's text
summary, but not the evidence around that summary.

#### Required closure

Add a portable event allowlist. Start with safe tool headers, plan changes,
request cards, artifact names, and diff summaries. Keep raw tool payloads out of
the bridge.

### 4. Composer and commands

#### Omega status: Missing in the current app

A separate Nostr command path exists in the wider Omega design. It includes
signed commands for Sarah, thread messages, interrupt, read state, reminders,
community actions, and provider handoff.

The current mobile app does not connect its composer to that path. Send and
Steer are disabled.

T3 Code Mobile can send messages, attach context, interrupt, and continue work.
It uses the same server command model as desktop.

#### Required closure

Connect one command first: send a message to an idle native Omega thread. The
command needs a stable command ID, admission result, queue state, and terminal
receipt. Then add interrupt. Add steer only for lanes that declare exact steer
behavior.

### 5. Approvals, questions, and plans

#### Omega status: Missing

The current bridge does not send these events. The current mobile UI has no
cards or answer controls for them.

This is a critical remote-control gap. A long run can stop for a decision while
the owner sees only a stale text preview.

T3 Code Mobile can answer approvals and questions. It can also inspect plan
state.

#### Required closure

Make an attention inbox the first mobile control surface. It has higher value
and lower risk than a full mobile IDE.

### 6. Files and attachments

#### Omega status: Missing

The mobile app has no file tree, file viewer, attachment viewer, or search.
T3 Code Mobile has these surfaces.

The Omega bridge intentionally withholds file paths. A mobile file surface
therefore needs a new scoped path contract. It cannot reuse transcript preview
strings.

#### Required closure

Add file references with opaque IDs. Resolve an ID only after the device grant
permits that workspace and operation. Start with read-only text and image files.

### 7. Diff review

#### Omega status: Missing

The mobile app has no diff renderer and no accept or reject controls. T3 Code
Mobile has a native diff module and a review flow.

#### Required closure

Project diff metadata and bounded hunks. Add a read-only review first. A change
decision must bind the exact diff generation and checkpoint.

### 8. Git and forge

#### Omega status: Missing

The mobile app has no Git or forge surface. T3 Code Mobile can inspect status and
perform common Git actions.

#### Required closure

Do not expose a general shell as the first answer. Add typed Git status, commit,
push, and pull-request commands. Each command must show the target repository,
branch, and authority before admission.

### 9. Terminal

#### Omega status: Missing

The current mobile app has no terminal. T3 Code Mobile has a terminal surface.

#### Required closure

Start with task output and bounded command requests. A full pseudo-terminal can
follow after resize, reconnect, secret entry, and accessibility tests exist.

### 10. Offline behavior

#### Omega status: Missing

The bridge can resume a stream after a disconnect. That is not an offline
outbox. The current app cannot accept a command while offline.

T3 Code Mobile has an offline outbox and reconnect rules.

#### Required closure

Store signed command intents with a visible pending state. Revalidate the host
generation, grant, thread state, and command expiry before delivery.

### 11. Notifications and deep links

#### Omega status: Missing in the current product flow

The current app does not provide a complete notification route for approvals,
questions, completion, or failure. It has no Live Activity, widget, or share
target flow.

T3 Code Mobile has push notifications, deep links, Live Activities, widgets,
and share targets.

#### Required closure

Add notifications after the attention-event contract. A notification must open
the exact request or run. It must not open a generic home screen.

### 12. Tablet and responsive layout

#### Omega status: Partial

The current app uses a single-column phone layout. Lists use `FlatList`, and
controls use mobile-size targets. The layout has no tablet workbench or
side-by-side thread detail.

T3 Code Mobile uses adaptive navigation and supports larger device layouts.

#### Required closure

Add a two-pane activity and detail layout for tablets. Keep the phone path
single-column.

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

The mobile design has the correct level of visual density for a remote
controller. It should add semantic control surfaces before it adds decoration.

### T3 Code context

The prior T3 UI audit scored 13/20. It found good performance and responsive
work. It also found unnamed controls, pointer-only sidebar reordering, incomplete
dialog focus, incomplete reduced-motion handling, and disabled terminal
screen-reader support.

T3 Code should be a feature reference, not a visual reference. Omega should not
copy its glass, noise, animated gradients, or dense nested panels.

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

Do not copy T3 Code decoration. Omega's sparse shell fits the product identity.
Add structure through navigation and information, not through effects.

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
