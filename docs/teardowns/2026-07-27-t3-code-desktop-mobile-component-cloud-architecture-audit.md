# T3 Code desktop, mobile, and cloud architecture audit — 2026-07-27

Status: Read-only source audit and Omega replication reference

T3 Code source commit: `a148e08197fc38b24e59c10c7cd5ba06dd182dab`

OpenAgents source commit at study start: `dedc1e85682e8648f6207a630b64fc052f913b35`

## Purpose

This report is the component-level companion to:

- [Omega and T3 Code desktop and mobile gap analysis](./2026-07-27-omega-t3-code-desktop-mobile-gap-analysis.md)
- [T3 Code server projection and consistency architecture](./2026-07-27-t3-code-server-projection-consistency-architecture.md)
- [T3 Code Sidebar V2 replication analysis](./2026-07-27-t3-code-sidebar-v2-replication-analysis.md)
- [T3 Code mobile app teardown](./2026-07-17-t3-code-mobile-app-teardown.md)

The gap analysis tells Omega what is different. This report tells an
implementation team how T3 Code composes the product. It covers desktop,
hosted web, mobile, native mobile modules, the local environment server, and
T3 Connect.

This report answers five questions:

1. Which component owns each visible surface?
2. Which state and service layers supply each component?
3. How do messages, tool calls, navigation, layout, and interaction work?
4. What runs on the client, on the environment host, and in T3 Tools cloud
   infrastructure?
5. Which parts can Omega adapt without copying T3 Code's implementation
   boundaries?

## Evidence rules

The report uses three labels.

- **Observed** means that the pinned source or an official T3 Tools page
  contains direct evidence.
- **Inference** means that the conclusion follows from several observed facts,
  but T3 Tools does not state it directly.
- **Recommendation** means an Omega design proposal.

The audit did not use private traffic, private accounts, or production
credentials. It did not inspect a running production relay. Cloud resource
names and runtime behavior come from public source and public policy pages.

## Executive finding

T3 Code is one agent workspace with three main execution planes.

1. A React web workspace supplies almost all desktop product UI.
2. A React Native workspace supplies a mobile-specific shell over the same
   environment contracts.
3. A local or remote T3 environment server owns projects, files, Git,
   terminals, provider processes, thread state, and agent execution.

T3 Connect adds a fourth plane. It is a hosted control plane, not a hosted
agent runtime. It links a cloud identity to an environment, creates a managed
HTTPS and WSS endpoint, issues proof-bound connection credentials, stores
small discovery and notification records, and sends Apple push updates.
After connection bootstrap, the client talks directly to the selected
environment. The relay is not in the normal agent, file, terminal, or Git
traffic path.

The desktop and mobile products do not share view components. They share
contracts, runtime services, state models, message semantics, and product
concepts. Each client has a presentation grammar that fits its platform.

The highest-value lesson for Omega is the separation of:

- environment authority from client presentation
- durable domain state from local view state
- message semantics from row rendering
- cloud discovery from the normal data path
- desktop density from mobile navigation
- agent activity from raw provider events

## Scale and code shape

### Desktop and hosted web

At the pinned commit, `apps/web/src` contains:

- 623 TypeScript and TSX files
- 136,415 lines
- 208 TSX files
- 189 production TSX files
- 180 test files
- 32,608 test lines

The Electron package contains:

- 117 TypeScript files
- 33,656 lines
- 50 test files
- 13,146 test lines

Large desktop components show where orchestration pressure collects.

| Component              | Approximate lines | Primary responsibility                             |
| ---------------------- | ----------------: | -------------------------------------------------- |
| `ChatView`             |             6,095 | Complete active-thread workspace orchestration     |
| `Sidebar`              |             3,643 | Project navigation, drag and drop, and prewarm     |
| `ConnectionsSettings`  |             3,399 | Local, remote, SSH, pairing, and cloud setup       |
| `ChatComposer`         |             3,196 | Prompt, attachments, modes, approvals, and send    |
| `SidebarV2`            |             2,739 | Active, snoozed, and settled task inbox            |
| `CommandPalette`       |             2,095 | Commands, projects, threads, files, and clone      |
| `GitActionsControl`    |             2,038 | Commit, push, pull request, and repository actions |
| `MessagesTimeline`     |             2,082 | Virtual message and work-entry timeline            |
| `ComposerPromptEditor` |             1,843 | Lexical editor and structured tokens               |
| `SettingsPanels`       |             1,766 | Settings route and panel composition               |
| `ThreadTerminalDrawer` |             1,550 | Persistent terminal drawer and splits              |

These files are screen controllers with many derived states and interaction
branches. Omega should copy the product boundaries, but it should split
controller state from GPUI rendering earlier.

### Mobile

At the pinned commit, `apps/mobile/src` contains:

- 418 TypeScript and TSX files
- 67,557 lines
- 127 production TSX files
- 92 test files
- 13,497 test lines

The native Expo module directory contains:

- 36 Swift, Kotlin, TypeScript, and TSX files
- 11,163 lines
- five product-specific native module families

Large mobile screens are:

| Component                   | Approximate lines | Primary responsibility                            |
| --------------------------- | ----------------: | ------------------------------------------------- |
| `ThreadFeed`                |             1,838 | Virtual transcript, work log, and scroll behavior |
| `ThreadTerminalRouteScreen` |             1,325 | Full-screen terminal and session controls         |
| `ThreadNavigationSidebar`   |             1,207 | Adaptive workspace navigation                     |
| `NewTaskDraftScreen`        |             1,159 | Project, model, mode, and prompt draft            |
| `HomeScreen`                |               984 | Environment and task home state                   |
| `NewTaskFlowProvider`       |               936 | Multi-step draft lifecycle                        |
| `ThreadComposer`            |               934 | Collapsed and expanded prompt composer            |
| `ReviewSheet`               |               880 | Native diff review and comments                   |
| `ThreadRouteScreen`         |               853 | Thread state, title, actions, and routing         |
| `AddProjectScreen`          |               839 | Remote project creation                           |

## Product topology

```text
T3 Tools hosted services
├── Marketing site on Vercel
├── Hosted web channels on Vercel
│   ├── app.t3.codes router
│   ├── latest.app.t3.codes
│   └── nightly.app.t3.codes
├── Clerk identity and waitlist
└── T3 Connect relay on Cloudflare
    ├── Worker API
    ├── managed Cloudflare tunnels and DNS
    ├── APNs queue and dead-letter queue
    ├── PlanetScale Postgres through Hyperdrive
    └── Axiom traces

User clients
├── Electron desktop
│   └── React web renderer
├── Hosted React web client
└── React Native mobile client
    ├── iOS native modules
    └── Android native modules

User-controlled environments
├── local desktop child server
├── headless t3 serve
├── desktop-managed SSH server
└── background Linux systemd user service
    ├── projects and files
    ├── Git and worktrees
    ├── provider processes and credentials
    ├── threads, turns, messages, and activity
    ├── terminal PTYs
    ├── environment HTTP and WebSocket APIs
    └── optional cloudflared connector
```

## Desktop render tree

### Native host

**Observed**

The Electron main process owns:

- `BrowserWindow` creation
- the native menu
- title-bar and traffic-light integration
- deep links
- update state
- secure preload IPC
- local server lifecycle
- preview webview lifecycle
- SSH launch and local port forwarding
- managed relay-client installation

The renderer does not start arbitrary native processes directly. It asks the
preload bridge to call typed main-process handlers.

### Root composition

The main render chain is:

```text
DesktopWindow
└── BrowserWindow
    └── preload bridge
        └── AppRoot
            ├── atom registry
            ├── router
            └── PreviewAutomationHosts
                └── RootRouteView
                    ├── auth and pairing gates
                    ├── toast providers
                    ├── sync coordinators
                    ├── dialogs
                    └── CommandPalette
                        └── AppSidebarLayout
                            ├── SidebarChrome
                            ├── Sidebar or SidebarV2
                            ├── SidebarRail
                            └── route Outlet
                                └── ChatView or settings route
```

`AppRoot` creates the application state boundary. `RootRouteView` handles
global states that must cover every route. `AppSidebarLayout` owns the
persistent product frame. Route content owns the workspace body.

### Desktop chrome

`SidebarChrome` is shared by both sidebar versions. It owns:

- the desktop drag region
- narrow-width close behavior
- the T3 wordmark and `Code` label
- release-channel branding
- provider update status
- application update status
- the Settings entry

The reusable `ui/sidebar.tsx` primitive owns:

- expanded and collapsed state
- cookie persistence
- off-canvas behavior
- mobile sheet behavior
- pointer resizing
- header, content, group, footer, trigger, and rail primitives

The primitive owns shell mechanics. `SidebarChrome` owns product content.

### Sidebar V1

`Sidebar` is the project-first navigator. It renders:

- project groups
- project favicons
- project menus
- thread rows
- draft rows
- archived access
- drag targets
- update indicators
- connection state

It also:

- derives project and thread order
- stores group collapse state
- prewarms likely thread routes
- supports drag and drop
- exposes thread and project context actions
- responds to connection changes
- handles missing or stale entities

V1 is useful when the project is the user's primary location model. Its
weakness is that active work across many projects is visually fragmented.

### Sidebar V2

`SidebarV2` is a task inbox. It groups work by lifecycle:

- active
- snoozed
- settled

It maintains stable visual order while activity updates arrive. It does not
sort the full list again on each render. This avoids rows that move while the
user targets them.

Each row can contain:

- thread title
- project identity
- status glyph
- relative time
- provider or model context
- unread or attention state
- running, waiting, success, or failure state
- row actions

The lifecycle actions are:

- snooze
- settle
- reactivate
- open
- archive

The active section is supervision-first. The settled section is
completion-first. The snoozed section is an explicit deferral queue.

### Command palette

`CommandPalette` is a multi-mode command surface. Its modes include:

- root
- root browse
- submenu
- submenu browse

It can render:

- application actions
- projects
- threads
- settings routes
- add-project flows
- local file-system browsing
- remote file-system browsing
- WSL UNC path browsing
- Git clone source selection
- clone destination selection

It separates query, mode, parent mode, selection, result provider, and action
execution. `CommandPaletteResults` owns the row collection and selection
behavior. The palette is a cross-route controller, not only a search input.

### Chat workspace

The active-thread render tree is:

```text
ChatView
├── ChatHeader
│   ├── project and thread identity
│   ├── status indicators
│   ├── scripts
│   ├── Open In menu
│   └── GitActionsControl
├── MessagesTimeline
│   ├── user rows
│   ├── assistant rows
│   ├── work-log rows
│   ├── turn folds
│   ├── plan cards
│   └── working state
├── ChatComposer
├── BranchToolbar
└── RightPanelTabs
    ├── preview
    ├── terminal
    ├── diff
    ├── files
    ├── file preview
    └── plan
```

`ChatView` reads environment, project, thread, provider, Git, preview,
terminal, and responsive state. It converts those states into one workspace.

### Header and placement controls

`ChatHeader` shows:

- project favicon and title
- thread title
- session or provider state
- status indicators
- project scripts
- external editor actions
- source-control actions
- narrow-layout overflow actions

`OpenInPicker` maps project paths to installed editors and terminals.
`ProjectScriptsControl` exposes configured scripts. `GitActionsControl`
provides commit, push, pull request, and repository workflows.

`BranchToolbar` composes:

- `BranchToolbarEnvironmentSelector`
- `BranchToolbarEnvModeSelector`
- `BranchToolbarBranchSelector`

It represents the selected environment, direct repository or worktree mode,
active branch, branch creation, switch state, and disabled reasons. Execution
placement is visible before a prompt starts.

## Desktop message and tool-call system

### Timeline and row grammar

`MessagesTimeline` uses `LegendList`. It handles:

- virtual rows
- dynamic row heights
- bottom anchoring
- initial position
- prepend and append changes
- follow-tail behavior
- a minimap
- folded turns
- transient working state

The timeline does not render raw provider messages. Server projections and
client adapters first produce stable message and activity objects. The
timeline maps those objects to:

- user message
- assistant message
- compact work entry
- expanded work entry
- turn boundary
- folded turn
- proposed plan
- plan follow-up
- current working indicator
- provider or runtime error

User messages use a bounded right-aligned surface. Assistant messages use an
unboxed document surface. Work entries use compact operational rows.

### User and assistant messages

A user row can show:

- prompt text
- images
- file attachments
- preview annotations
- selected page elements
- terminal excerpts
- review comments
- copy and revert actions
- timestamp

Context objects are first-class chips or cards. They are not flattened into
invisible prompt text.

An assistant row can show:

- Markdown
- code blocks
- tables
- task lists
- details elements
- file links
- external links
- changed-file summary
- copy action
- timestamp

`ChatMarkdown` supplies:

- GitHub Flavored Markdown
- HTML sanitization
- task lists
- tables
- details and summary elements
- Shiki syntax highlighting
- code titles and copy controls
- file-link routing
- external-link favicons
- heading fragment links
- a 500-entry and 50 MB highlight cache

The file-link handler can open the internal file panel, an editor, or an
external browser. The renderer participates in workspace navigation.

### Work-log normalization

Provider-specific events do not directly select React components. A
normalization layer converts activities into semantic work entries.

A work entry includes:

- kind
- title
- status
- summary
- detail
- timing
- related files
- related command or tool data
- expandable content

Examples include:

- command execution
- file read
- file edit
- search
- web fetch
- browser action
- source-control action
- approval request
- user-input request
- runtime error

The compact row answers what the agent did, its state, and whether the user
must act. Expanded detail shows the tool input, output, error, and changed
files.

### Approvals and user input

Desktop approvals can render in the transcript and composer. The composer owns
the current actionable version.

`ComposerPendingApprovalPanel` and `ComposerPendingApprovalActions` render:

- request summary
- command or tool detail
- approve
- reject
- alternative or scoped response when supported
- pending and failure state

`ComposerPendingUserInputPanel` renders provider questions with answer
controls. The prompt editor is disabled or constrained when the runtime needs
a required response.

## Desktop composer

`ChatComposer` has three main forms:

- draft hero
- docked composer
- collapsed composer

`ComposerPromptEditor` uses Lexical. It supports inline atomic nodes for:

- files
- skills
- terminal context
- selected elements
- review comments

Atomic nodes preserve identity and deletion behavior. They also let the
submission layer serialize structured context without parsing display text.

The composer composes:

- `ComposerPromptEditor`
- attachment strip
- `ComposerCommandMenu`
- `ComposerPrimaryActions`
- `CompactComposerControlsMenu`
- `ProviderModelPicker`
- `TraitsPicker`
- `ContextWindowMeter`
- `ComposerBannerStack`
- pending approval and user-input panels
- pending element, review, and terminal contexts
- preview annotation cards
- plan follow-up banner
- stash badge and menu

The controls expose provider instance, model, traits, runtime mode,
interaction mode, plan mode, attachments, send, stop, stash, and restore.

The submit path:

1. validates connection and provider state
2. resolves structured editor nodes
3. collects attachments and context
4. selects runtime, interaction, and plan modes
5. creates or updates a thread
6. dispatches a turn request
7. clears or stashes the accepted draft
8. reports a dispatch failure to the composer

## Desktop secondary work surfaces

### Right panel

`RightPanelTabs` supports open, select, close, middle-click close, context
menu, disabled reasons, persistence, resizing, maximize, and restore.

Its width rules are:

- default near 540 px
- minimum near 360 px
- maximum near 70 percent of the workspace

`RightPanelResizeHandle` owns pointer resize. `RightPanelSheet` supplies a
narrow-layout alternative.

### Preview

The preview family contains:

- `PreviewPanel`
- `PreviewPanelShell`
- `PreviewView`
- `PreviewChromeRow`
- `BrowserDeviceToolbar`
- `PreviewMoreMenu`
- `PreviewEmptyState`
- `PreviewLocalServerCard`
- `PreviewUnreachable`
- `ThreadPreviewMiniPlayer`
- `ZoomIndicator`
- `AgentBrowserCursor`
- `PreviewAutomationHosts`
- `ElectronBrowserHost`
- `HostedBrowserWebview`
- `BrowserSurfaceSlot`
- `BrowserViewportResizeHandles`

The Electron preview manager owns isolated webviews. The renderer owns product
chrome and automation state. The surface supports URL entry, navigation,
reload, stop, device sizes, viewport resize, zoom, picture in picture,
screenshot, recording, page-element selection, annotation, agent cursor, and
automation.

### Terminal, diff, files, and plan

Desktop terminal presentation uses Xterm. It supports multiple sessions,
splits, resize, focus, persistent drawer and panel state, context capture, and
reconnect.

Diff components include:

- `DiffPanel`
- `DiffPanelShell`
- `AnnotatableCodeView`
- `ChangedFilesTree`
- `DiffStatLabel`

File components include:

- `FileBrowserPanel`
- `FilePreviewPanel`
- `LocalCommentAnnotation`

Plan presentation includes:

- `PlanSidebar`
- `ProposedPlanCard`
- `ComposerPlanFollowUpBanner`

A review comment can become composer context. A changed-file row can open the
diff or file surface. A structured plan can stay visible while the user reads
or writes.

## Desktop settings and design primitives

Settings components include:

- `SettingsPanels`
- `SettingsSidebarNav`
- `ConnectionsSettings`
- `ProviderSettingsForm`
- `ProviderInstanceCard`
- `ProviderModelsSection`
- `AddProviderInstanceDialog`
- `AddProviderInstanceWizardSteps`
- `ProviderAccentColorPicker`
- `SourceControlSettings`
- `SourceControlWritingSettings`
- `KeybindingsSettings`
- `DiagnosticsSettings`
- `BetaSettingsPanel`
- `RedactedSensitiveText`

Connection UI includes local child, direct, pairing, QR, LAN, custom HTTPS,
Tailscale, SSH-launched, and T3 Connect environments.

Cloud desktop components are:

- `CloudEnvironmentConnectList`
- `ConnectCliAuthSurface`
- `ConnectOnboardingDialog`
- `RelayClientInstallDialog`

The web client has approximately 40 reusable primitives. They include alert
dialog, alert, autocomplete, badge, button, card, checkbox, collapsible,
combobox, command, dialog, draft input, empty state, field, fieldset, form,
group, input group, input, keyboard hint, label, menu, number field, popover,
QR code, radio group, scroll area, select, separator, sheet, sidebar, skeleton,
spinner, switch, table, text area, toast, toggle group, toggle, and tooltip.

## Mobile application shell

### Provider and route tree

The React Native root composes:

```text
RegistryProvider
└── CloudAuthProvider
    └── AppearancePreferencesProvider
        └── GestureHandlerRootView
            └── KeyboardProvider
                └── SafeAreaProvider
                    └── StatusBar
                        └── BlurTargetProvider
                            └── IncomingShareProvider
                                └── NavigationContainer
                                    ├── RootStack
                                    ├── ConfirmDialogHost
                                    └── OverlayPortalHost
```

The root navigator contains home, thread, terminal, settings, connections,
add-project, archive, new-task, review, and Git routes. Thread routes stay flat
in the root stack so iOS headers can morph between list and detail states.

### Adaptive workspace

`AdaptiveWorkspaceLayout` changes the product model, not only style.

Split mode starts near:

- 720 px minimum width
- 600 px minimum height

The sidebar uses:

- about 32 percent of width
- a 280 px minimum
- a 380 px normal maximum
- a 460 px resize cap

The file inspector becomes available near:

- 820 px viewport width
- 560 px minimum remaining content width
- 260 px to 480 px inspector width

The chat column has a maximum near 960 px.

The persistent sidebar sits outside the detail navigator. Routes register
inspector content with the workspace shell. The app freezes expensive content
width while native chrome animates. This reduces repeated diff, Markdown, and
list layout during transitions.

### Platform chrome

iOS uses native stack headers, search, menus, form sheets, context menus,
glass, and blur surfaces. Android uses in-flow headers, anchored menus, and
explicit surfaces.

`ControlPill` standardizes a 44-point target. `AndroidScreenHeader` and
`AndroidAnchoredMenu` keep Android behavior explicit.

## Mobile navigation and home

`HomeRouteScreen` provides route and header integration. `HomeScreen` owns
environment state, active and settled sections, empty state, connection
actions, new-task entry, and queued offline tasks.

`ThreadNavigationSidebar` composes header actions, filter, task list, active
cards, settled rows, archive, new task, and connection status. It has
platform-specific iOS and Android header actions.

`thread-swipe-actions` maps gestures to:

- snooze
- settle
- reactivate
- archive

The action is lifecycle-aware. The row supplies visual distance, threshold,
label, and confirmation state.

## Mobile thread workspace

### Controller and layout

`ThreadRouteScreen` binds environment, thread shell, project shell, title,
connection, Git, terminal, review, inspector, and header actions. It passes a
smaller view model to `ThreadDetailScreen`.

`ThreadDetailScreen` owns feed, composer, keyboard relation, safe area,
connection banner, pending actions, and inspector coordination. Keyboard
height and composer height update the feed inset.

### Feed and rows

`ThreadFeed` uses `KeyboardAwareLegendList`. It has custom or patched behavior
for:

- keyboard insets
- initial bottom position
- item-height changes
- maintain-visible-content-position
- follow-tail
- pending local rows
- prepend history
- bottom affordance

Its row grammar matches desktop concepts:

- user message
- assistant message
- work entry
- approval
- user input
- plan
- error
- current activity

User rows contain text, attachment previews, context, copy, and timestamp.
Assistant rows contain native Markdown, code, copy, and timestamp. Work rows
contain icon, title, phase, time, expandable detail, and copy.

### Composer and new task

`ThreadComposer` has a collapsed pill and an expanded card. It contains a
native editor, attachment strip, command popover, model and configuration
controls, send, and stop.

`NewTaskFlowProvider` keeps a draft across routes and sheets.
`NewTaskDraftScreen` selects environment, project, worktree or branch,
provider, model, runtime mode, interaction mode, prompt, and attachments. It
also handles offline queue state.

### Approvals and questions

`PendingApprovalCard` shows request, command or tool detail, actions,
progress, and error. `PendingUserInputCard` shows questions, answer controls,
and submission state.

**Observed weakness**

Several Pressable controls in these cards do not provide complete
accessibility roles, labels, and state. Some colors are direct values. The
question card can show all questions at once and create a long interaction
block.

**Recommendation**

Omega mobile should use explicit roles, accessibility state, semantic colors,
and one-question focus with visible progress for long requests.

## Mobile files, review, Git, and terminal

File components include:

- `FileTreeBrowser`
- `ThreadFilesRouteScreen`
- `SourceFileSurface`
- `FileMarkdownPreview`
- `WorkspaceFileImagePreview`
- `WorkspaceFileWebPreview`
- `thread-file-navigator-pane`

Review components include:

- `ReviewSheet`
- `ReviewCommentComposerSheet`
- `ReviewHighlighterProvider`
- `reviewDiffRendering`

The native review sheet supports file and diff navigation, line selection,
comments, comment composition, and changed-file state.

`ThreadGitControls` opens `GitOverviewSheet`, `GitBranchesSheet`,
`GitCommitSheet`, and `GitConfirmSheet`. `GitActionProgressOverlay` keeps long
Git actions visible across sheet state.

Terminal components are `NativeTerminalSurface`, `ThreadTerminalPanel`, and
`ThreadTerminalRouteScreen`. Both panel and route use environment PTY and
WebSocket contracts.

## Mobile native modules

### `t3-composer-editor`

This module provides native prompt editing for keyboard behavior, selection,
composition input, paste, focus, height measurement, and structured insertion.

### `t3-markdown-text`

This module renders long assistant text through native text primitives. It
reduces React Native view count and improves selection and scrolling.

### `t3-native-controls`

This module supplies native menu, search, glass, and blur integration.

### `t3-review-diff`

This module renders large diffs with Swift and Kotlin drawing code. It owns
line layout, insert and delete backgrounds, delete stripes, syntax spans,
selection, highlighter geometry, and comment anchors.

### `t3-terminal`

The terminal receives output data, dimensions, appearance, focus, and
lifecycle state. It emits `{ data }` for input and `{ cols, rows }` for resize.
iOS uses GhosttyKit. Android uses `libghostty-vt` and Canvas. Remote output
comes from the environment WebSocket. The device does not run the shell.

## Shared state and transport

The environment server owns:

- environment descriptor
- project catalog and files
- Git repositories and worktrees
- provider instances and credentials
- threads, turns, messages, and activities
- approvals and user-input requests
- plans
- terminal PTYs
- browser and preview coordination

Clients keep caches and view preferences. They do not become the durable owner
of these objects.

A client connection has separate catalog, HTTP session, orchestration
snapshot, shell subscription, thread subscription, command, terminal, and
preview layers. A reachable HTTP server does not mean that each subscription
is healthy.

The server projects provider events into stable orchestration objects. Clients
derive project rows, task sections, message rows, work entries, attention, and
connection state. This projection lets desktop and mobile use different view
trees with the same product semantics.

The database half of this design is stronger than the side-effect half.
Accepted events, SQL projections, and the accepted command receipt commit in
one SQLite transaction. Provider, checkpoint, deletion, Git, and file reactors
consume hot process-local delivery or perform work outside that transaction.
They do not all have durable cursors or replay after a restart. The focused
[server projection and consistency audit](./2026-07-27-t3-code-server-projection-consistency-architecture.md)
traces the exact commit, snapshot, replay, buffering, deduplication, outbox,
reactor, and recovery boundaries.

## T3 Connect product boundary

### What T3 Connect is

**Observed**

The public relay README calls T3 Connect a hosted control plane. It owns:

- environment linking
- managed endpoint provisioning
- short-lived connection credential bootstrap
- environment and device discovery
- mobile notification registration
- Live Activity registration
- agent-activity publication
- relay persistence and diagnostics

It is in private beta. Users join a Clerk waitlist from the application.

### What T3 Connect is not

**Observed**

The relay is outside the normal traffic path after connection. The environment
host still owns agent processes, provider subscriptions and credentials,
source code, project files, Git, terminal processes, full transcripts, thread
execution, and environment API responses.

The client connects directly to the environment endpoint for normal HTTPS and
WSS traffic.

The relay database has no tables for prompts, assistant messages, files,
source code, diffs, Git objects, terminal output, or provider credentials.

The relay stores bounded agent-awareness state when the user enables it. That
state can include project title, thread title, model title, phase, headline,
short detail, deep link, and update time. Failure detail becomes a generic
message. Other detail is trimmed to 160 characters.

### Business model

**Observed**

The T3 Code site says:

- the application is open source under MIT
- users bring their own coding-agent subscription
- T3 Code does not resell tokens
- T3 Code does not impose a model-token quota

The current Terms say that T3 Code has no T3 Tools subscription fee unless a
feature clearly states otherwise. They reserve the option to introduce a paid
feature with price disclosure before purchase.

No Stripe dependency, billing table, entitlement service, price table, or
public T3 Connect price was found in the pinned repository.

**Inference**

The current strategy appears to be adoption first. A free MIT client grows
reach. Bring-your-own-provider avoids token resale cost and margin risk. T3
Connect adds an optional account-backed convenience layer. The waitlist limits
early infrastructure and support load.

T3 Connect is the clearest future paid-feature candidate because it creates
recurring identity, database, tunnel, DNS, observability, queue, and push
costs. This is only an inference. Public evidence does not show a committed
price or billing launch.

### Evidence from the Ping.gg organization

**Observed**

The public GitHub organization says that Ping.gg builds tools for modern
developers. Its public repositories include T3 Code, Lawn, UploadThing,
Markerthing, and older utilities.

Lawn is an open-source video review product with a different backend stack.
UploadThing is a separate file-upload product.

**Conclusion**

These repositories show a pattern of open-source developer or creator tools
with hosted surfaces. They do not prove that T3 Code shares Lawn or
UploadThing infrastructure, accounts, billing, or data. T3 Code source is the
only reliable infrastructure source for this audit.

## T3 Connect deployed infrastructure

### Deployment stack

**Observed**

Alchemy provisions:

- one Cloudflare Worker API
- one APNs delivery queue
- one APNs dead-letter queue
- Cloudflare managed tunnels
- Cloudflare DNS records
- a retained PlanetScale Postgres database in `us-west`
- two database replicas
- a `PS_20` database cluster
- a Cloudflare Hyperdrive connection
- an Axiom OpenTelemetry trace dataset
- scoped Axiom ingest tokens
- a recent-spans Axiom view

Hyperdrive disables caching and limits origin connections to 20. The trace
dataset keeps data for 30 days. Separate scoped ingest tokens exist for the
Worker, mobile, and first-party clients.

### Stages and release channels

Production owns the retained API DNS zone, tunnel DNS zone, and PlanetScale
database. Non-production stages reference the zones, create isolated database
branches and roles, and create stage-specific runtime resources.

Stable and nightly clients share the production relay. The relay deploys on
each push to `main`. Pull requests do not deploy relay stages.

Desktop, CLI, hosted web, and mobile builds receive public configuration at
build time. Cloud UI is omitted when public configuration is absent. Fresh
source clones stay usable without a T3 Tools relay.

### Hosted web

The hosted React web product uses Vercel:

- `app.t3.codes` is a channel router
- `latest.app.t3.codes` is the stable application origin
- `nightly.app.t3.codes` is the nightly origin
- a secure HTTP-only cookie selects a channel
- an SPA rewrite serves `index.html`

The marketing site has a separate Vercel deployment. Vercel hosts the client.
It does not host the user's agent environment.

### Identity and push

One Clerk application supports hosted web, Electron, and mobile. Relay JWTs
use the `t3-relay` template and `t3-code-relay` audience.

The headless CLI uses a public Clerk OAuth application with PKCE and no client
secret. Its loopback callback is `http://127.0.0.1:34338/callback`.

Desktop uses external browser OAuth, custom schemes, encrypted Clerk token
persistence, and optional macOS passkeys. Mobile uses Clerk native auth.

The relay uses Apple Push Notification service for notification and Live
Activity start, update, and end. The schema currently supports iOS device
records. Expo supplies mobile builds, updates, and related app services.

## T3 Connect data model

The relay schema contains:

| Table                                | Stored state                                     |
| ------------------------------------ | ------------------------------------------------ |
| `relay_mobile_devices`               | user, device, version, APNs tokens, preferences  |
| `relay_live_activities`              | activity token, start, end, aggregate, delivery  |
| `relay_environment_links`            | user, environment, public key, endpoints, scopes |
| `relay_managed_endpoint_allocations` | hostname, tunnel, DNS, readiness                 |
| `relay_managed_tunnel_limits`        | per-user maximum tunnel count                    |
| `relay_environment_credentials`      | credential ID, public key, hash, revocation      |
| `relay_agent_activity_rows`          | current bounded state by environment and thread  |
| `relay_delivery_attempts`            | bounded APNs delivery diagnostics                |
| `relay_dpop_proofs`                  | proof thumbprint and JWT ID until expiry         |

The default managed-tunnel limit is three per user. An override row can change
it. The plaintext environment credential stays on the environment.

A five-minute cron removes expired DPoP records. It also removes terminal
activity rows after approximately 30 minutes.

## T3 Connect trust boundaries

| Credential                   | Issuer           | Holder                         | Purpose                 |
| ---------------------------- | ---------------- | ------------------------------ | ----------------------- |
| Clerk session-template JWT   | Clerk            | web, desktop, or mobile        | Link and bootstrap      |
| CLI OAuth tokens             | Clerk            | environment secret store       | Headless link           |
| Environment Ed25519 key      | environment      | environment secret store       | Signed proofs           |
| Relay cloud-mint key         | relay deployment | Worker and environment         | Health and mint         |
| Relay environment credential | relay            | hash in database, text on host | Activity publish        |
| Relay DPoP token             | relay            | client token cache             | Status, connect, mobile |
| Bootstrap credential         | environment      | passed through relay           | Token exchange          |
| Environment access token     | environment      | client                         | Normal HTTP access      |
| WebSocket ticket             | environment      | client until use               | One-time WSS upgrade    |
| Cloudflare connector token   | Cloudflare       | environment secret store       | Managed tunnel          |

A Clerk account is not an environment login. The environment remains the
final issuer of environment access.

The main authentication boundaries are:

1. Client to relay link management uses a Clerk bearer.
2. Client to protected relay endpoints uses relay DPoP.
3. Relay to environment health and mint uses relay-signed proofs.
4. Client to environment uses an environment-issued DPoP token.
5. Environment activity publication uses an environment bearer and signed
   state proof.

## Link, connect, and notification flows

### Desktop link

```text
User
  -> Connections UI
  -> local relay-client availability check
  -> optional install consent
  -> relay link challenge with Clerk bearer
  -> local environment link proof with relay:write token
  -> relay verification and tunnel reconciliation
  -> relay link and environment credential
  -> local relay configuration
  -> local cloudflared process
```

The install path reports checking, waiting for lock, downloading, verifying,
installing, validating, and activating.

### Headless link

```text
Operator
  -> t3 connect link
  -> optional cloudflared install
  -> Clerk OAuth browser flow with PKCE
  -> local durable desired-link state
  -> next t3 serve or t3 start
  -> relay challenge and environment proof
  -> managed endpoint configuration
  -> local cloudflared process
```

The CLI separates login, link, status, unlink, and logout. Unlink keeps Clerk
authorization. Logout removes it. The background service has an independent
lifecycle.

### Remote connect

```text
Client -> Clerk: get session-template JWT
Client -> relay: exchange Clerk JWT and DPoP proof
Relay -> client: scoped relay DPoP token
Client -> relay: request environment connection
Relay -> environment: signed mint request
Environment -> relay: bootstrap credential and signed response
Relay -> client: endpoint and bootstrap credential
Client -> environment: exchange bootstrap credential with DPoP proof
Environment -> client: DPoP-bound environment access token
Client -> environment: request one-time WebSocket ticket
Client -> environment: connect WSS with ticket
```

Normal requests do not return through the relay after this flow.

### Agent activity and push

```text
Environment
  -> project bounded thread awareness
  -> remove sensitive failure detail
  -> sign state
  -> publish to relay
Relay
  -> validate bearer and proof
  -> store current row
  -> enqueue delivery jobs
Queue consumer
  -> APNs
  -> mobile notification or Live Activity
```

The APNs queue uses a maximum batch size of 10, five retries, a batch wait near
five seconds, a retry delay near 30 seconds, and a dead-letter queue.

## Managed endpoint behavior

The relay creates a deterministic hostname from deployment namespace, user
ID, and environment ID. Production uses a `prod-<digest>` form.

The managed tunnel can target only:

- `127.0.0.1`
- `::1`
- `localhost`
- a valid TCP port

Both the environment and relay validate this origin. The cloud tunnel reaches
the T3 server on the user's host. It does not proxy an arbitrary application.

Cloudflare bills for each provisioned tunnel. On shutdown, T3 asks the relay
to delete the active tunnel. The relay keeps the environment link, hostname,
tunnel name, and DNS reservation. A later start recreates the tunnel under the
same public hostname.

On Linux, T3 can install a systemd user service. It runs `t3 serve`, restarts
after failure, uses user linger, writes an append log, and has a restart-rate
limit. Cloud sign-out does not uninstall it.

## Relay HTTP component inventory

### Public and metadata

| Endpoint                                      | Authentication | Function          |
| --------------------------------------------- | -------------- | ----------------- |
| `GET /health`                                 | none           | Worker health     |
| `GET /.well-known/oauth-authorization-server` | none           | token metadata    |
| `GET /.well-known/oauth-protected-resource`   | none           | DPoP metadata     |
| `GET /openapi.json`                           | none           | generated OpenAPI |
| `GET /docs`                                   | none           | Scalar API docs   |

### Clerk bearer API

| Endpoint                                                    | Function                 |
| ----------------------------------------------------------- | ------------------------ |
| `GET /v1/environments`                                      | list linked environments |
| `GET /v1/client/devices`                                    | list devices             |
| `POST /v1/client/environment-link-challenges`               | create link challenge    |
| `POST /v1/client/environment-links`                         | create or update link    |
| `DELETE /v1/client/environment-links/:environmentId`        | revoke link              |
| `DELETE /v1/client/environment-links/:environmentId/tunnel` | release tunnel           |

### DPoP API

| Endpoint                                       | Function                 |
| ---------------------------------------------- | ------------------------ |
| `POST /v1/client/dpop-token`                   | exchange Clerk token     |
| `POST /v1/environments/:environmentId/status`  | verify health            |
| `POST /v1/environments/:environmentId/connect` | get bootstrap credential |
| `POST /v1/mobile/devices`                      | register device          |
| `DELETE /v1/mobile/devices/:deviceId`          | remove device            |
| `POST /v1/mobile/live-activities`              | register activity token  |
| `GET /v1/mobile/agent-activity`                | read current aggregate   |

Relay DPoP scopes are `environment:connect`, `environment:status`, and
`mobile:registration`. Public client IDs are `t3-web` and `t3-mobile`.

### Environment publication API

`POST /v1/environments/:environmentId/threads/:threadId/agent-activity`
publishes signed bounded activity.

## Environment cloud HTTP inventory

| Endpoint                               | Authentication      | Function               |
| -------------------------------------- | ------------------- | ---------------------- |
| `POST /api/connect/link-proof`         | local `relay:write` | sign link proof        |
| `POST /api/connect/relay-config`       | local `relay:write` | store config and start |
| `GET /api/connect/link-state`          | local `relay:read`  | read link              |
| `POST /api/connect/preferences`        | local `relay:write` | update publication     |
| `POST /api/connect/unlink`             | local `relay:write` | stop and clear         |
| `POST /api/t3-connect/health`          | relay proof         | signed health          |
| `POST /api/t3-connect/mint-credential` | relay proof         | mint bootstrap         |
| `POST /oauth/token`                    | bootstrap and DPoP  | issue access token     |
| `POST /api/auth/websocket-ticket`      | environment token   | issue WSS ticket       |

## Client relation to cloud

The web client stores its DPoP key in IndexedDB, uses client ID `t3-web`,
lists relay environments, requests status and connection credentials, and
adds the result to the normal environment catalog.

Electron adds native Clerk integration, local server control, relay-client
installation, SSH launch, and native encrypted persistence.

Mobile stores its DPoP key in Expo SecureStore, uses client ID `t3-mobile`,
registers APNs and Live Activity tokens, discovers environments, and connects
through the same runtime.

The relay token cache key includes account ID, client ID, relay URL, DPoP
thumbprint, and scopes.

The environment stores its key pair, plaintext relay environment credential,
cloud-mint public key, and connector token. It runs cloudflared, answers health
and mint requests, issues final credentials, and publishes bounded activity.
It does not give the relay provider credentials.

## Cross-device component equivalence

| Product concept       | Desktop or web           | Mobile                    |
| --------------------- | ------------------------ | ------------------------- |
| persistent navigation | `Sidebar` or `SidebarV2` | `ThreadNavigationSidebar` |
| task home             | sidebar sections         | `HomeScreen` and sidebar  |
| thread controller     | `ChatView`               | `ThreadRouteScreen`       |
| transcript            | `MessagesTimeline`       | `ThreadFeed`              |
| assistant text        | `ChatMarkdown`           | native Markdown row       |
| tool activity         | normalized work entry    | `thread-work-log`         |
| composer              | `ChatComposer`           | `ThreadComposer`          |
| prompt editor         | Lexical                  | native composer editor    |
| approval              | composer panel           | `PendingApprovalCard`     |
| user question         | composer panel           | `PendingUserInputCard`    |
| plan                  | sidebar and card         | feed card or route state  |
| files                 | right panel              | route or inspector        |
| diff review           | right panel              | native review sheet       |
| Git                   | header and dialogs       | Git sheets                |
| terminal              | Xterm drawer or panel    | native route or panel     |
| connection setup      | settings and dialogs     | connection routes         |
| DPoP key storage      | IndexedDB                | Expo SecureStore          |
| secondary surface     | right panel or sheet     | adaptive inspector        |

## Full production component registry

Tests, stories, and pure utility modules are not included.

### Desktop root, shell, and navigation

- `AnimatedHeight`
- `AppSidebarLayout`
- `CommandPalette`
- `CommandPaletteResults`
- `ConnectionStatusDot`
- `NoActiveThreadState`
- `ProjectFavicon`
- `Sidebar`
- `SidebarStageBackdrop`
- `SidebarV2`
- `SidebarChrome`
- `SidebarProviderUpdatePill`
- `SidebarUpdatePill`
- `SplashScreen`
- `SlowRpcRequestToastCoordinator`
- `ThreadStatusIndicators`

### Desktop thread and composer

- `BranchToolbar`
- `BranchToolbarBranchSelector`
- `BranchToolbarEnvModeSelector`
- `BranchToolbarEnvironmentSelector`
- `ChatComposer`
- `ChatHeader`
- `ChatMarkdown`
- `ChatView`
- `ChangedFilesTree`
- `CompactComposerControlsMenu`
- `ComposerBannerStack`
- `ComposerCommandMenu`
- `ComposerPendingApprovalActions`
- `ComposerPendingApprovalPanel`
- `ComposerPendingElementContexts`
- `ComposerPendingReviewComments`
- `ComposerPendingTerminalContexts`
- `ComposerPendingUserInputPanel`
- `ComposerPlanFollowUpBanner`
- `ComposerPreviewAnnotationCards`
- `ComposerPrimaryActions`
- `ComposerPromptEditor`
- `ComposerStashBadge`
- `ComposerStashMenu`
- `ContextWindowMeter`
- `DiffStatLabel`
- `DraftHeroHeadline`
- `ExpandedImageDialog`
- `ExpandedImagePreview`
- `FileTagChip`
- `MessageCopyButton`
- `MessagesTimeline`
- `ModelListRow`
- `ModelPickerContent`
- `ModelPickerSidebar`
- `OpenInPicker`
- `PanelLayoutControls`
- `PierreEntryIcon`
- `ProposedPlanCard`
- `ProviderInstanceIcon`
- `ProviderModelPicker`
- `ProviderStatusBanner`
- `SkillInlineText`
- `TerminalContextInlineChip`
- `ThreadErrorBanner`
- `TraitsPicker`

### Desktop browser and preview

- `AgentBrowserCursor`
- `BrowserDeviceToolbar`
- `BrowserMockup`
- `BrowserSurfaceSlot`
- `BrowserViewportResizeHandles`
- `ElectronBrowserHost`
- `HostedBrowserWebview`
- `PreviewAutomationHosts`
- `PreviewChromeRow`
- `PreviewEmptyState`
- `PreviewLocalServerCard`
- `PreviewMoreMenu`
- `PreviewPanel`
- `PreviewPanelShell`
- `PreviewUnreachable`
- `PreviewView`
- `RightPanelResizeHandle`
- `ThreadPreviewMiniPlayer`
- `ZoomIndicator`

### Desktop work surfaces

- `AnnotatableCodeView`
- `DiffPanel`
- `DiffPanelShell`
- `FileBrowserPanel`
- `FilePreviewPanel`
- `LocalCommentAnnotation`
- `PlanSidebar`
- `RightPanelSheet`
- `RightPanelTabs`
- `ThreadTerminalDrawer`

### Desktop settings, auth, cloud, actions, and updates

- `AddProviderInstanceDialog`
- `AddProviderInstanceWizardSteps`
- `AuthSurfaceShell`
- `BetaSettingsPanel`
- `CloudEnvironmentConnectList`
- `ConnectCliAuthSurface`
- `ConnectOnboardingDialog`
- `ConnectionsSettings`
- `DiagnosticsSettings`
- `GitActionsControl`
- `KeybindingsSettings`
- `MobileClientsUserProfilePage`
- `PairingRouteSurface`
- `ProjectScriptsControl`
- `ProviderAccentColorPicker`
- `ProviderInstanceCard`
- `ProviderModelsSection`
- `ProviderSettingsForm`
- `ProviderUpdateEnvironmentRows`
- `ProviderUpdateLaunchNotification`
- `ProviderUpdatePrimaryNotification`
- `PullRequestThreadDialog`
- `RedactedSensitiveText`
- `RelayClientInstallDialog`
- `ServerUpdateAction`
- `SettingsPanels`
- `SettingsSidebarNav`
- `SourceControlSettings`
- `SourceControlWritingSettings`
- `SshPasswordPromptDialog`
- `T3ConnectSidebarSignIn`

### Mobile global primitives

- `AndroidAnchoredMenu`
- `AndroidScreenHeader`
- `AppSymbol`
- `AppText`
- `BrandMark`
- `ComposerAttachmentStrip`
- `ComposerEditor`
- `ComposerToolbarTrigger`
- `ConfirmDialogHost`
- `ControlPill`
- `CopyTextButton`
- `EmptyState`
- `ErrorBanner`
- `GlassSafeAreaView`
- `GlassSurface`
- `LoadingScreen`
- `LoadingStrip`
- `OverlayPortal`
- `PierreEntryIcon`
- `ProjectFavicon`
- `ProviderIcon`
- `SourceControlIcon`
- `StatusPill`
- `T3Wordmark`

### Mobile home, navigation, and layout

- `AdaptiveWorkspaceLayout`
- `AndroidHomeFab`
- `HomeHeader`
- `HomeRouteScreen`
- `HomeScreen`
- `ThreadNavigationSidebar`
- `WorkspaceConnectionStatus`
- `WorkspaceEmptyDetail`
- `workspace-inspector-pane`
- `workspace-pane-divider`
- `workspace-sidebar-toolbar`

### Mobile thread and task

- `ComposerCommandPopover`
- `GitActionProgressOverlay`
- `NewTaskDraftRouteScreen`
- `NewTaskDraftScreen`
- `NewTaskRouteScreen`
- `PendingApprovalCard`
- `PendingUserInputCard`
- `ThreadComposer`
- `ThreadDetailScreen`
- `ThreadFeed`
- `ThreadGitControls`
- `ThreadRouteScreen`
- `new-task-flow-provider`
- `sidebar-filter-button`
- `sidebar-header-actions.android`
- `sidebar-header-actions.ios`
- `sidebar-navigation-shell`
- `thread-inspector-content-stack`
- `thread-list-items`
- `thread-list-v2-items`
- `thread-swipe-actions`
- `thread-work-log`

### Mobile files, review, Git, and terminal

- `FileMarkdownPreview`
- `FileTreeBrowser`
- `GitBranchesSheet`
- `GitCommitSheet`
- `GitConfirmSheet`
- `GitOverviewSheet`
- `NativeTerminalSurface`
- `ReviewCommentComposerSheet`
- `ReviewHighlighterProvider`
- `ReviewSheet`
- `SourceFileSurface`
- `ThreadFilesRouteScreen`
- `ThreadTerminalPanel`
- `ThreadTerminalRouteScreen`
- `WorkspaceFileImagePreview`
- `WorkspaceFileWebPreview`
- `gitSheetComponents`
- `reviewDiffRendering`
- `thread-file-navigator-pane`

### Mobile connections, cloud, projects, archive, and settings

- `AddProjectDestinationRoute`
- `AddProjectLocalRoute`
- `AddProjectRepositoryRoute`
- `AddProjectScreen`
- `AddProjectSourceRoute`
- `ArchivedThreadsRouteScreen`
- `ArchivedThreadsScreen`
- `AppearancePreferencesProvider`
- `AppearancePreviews`
- `ClerkSettingsSheetDetent`
- `CloudAuthProvider`
- `CloudEnvironmentRows`
- `CloudWaitlistEnrollment`
- `CodeAppearanceSection`
- `ConnectOnboardingRouteScreen`
- `ConnectionEnvironmentRow`
- `ConnectionSheetButton`
- `ConnectionStatusDot`
- `ConnectionsNewRouteScreen`
- `ConnectionsRouteScreen`
- `EnvironmentConnectionNotice`
- `FontSizeSliderRow`
- `IncomingShareProvider`
- `SettingsLegalDocumentRouteScreen`
- `SettingsRow`
- `SettingsSection`
- `SettingsSwitchRow`
- `TerminalAppearanceSection`
- `TextAppearanceSection`
- settings route screens

## Interaction audit

### Strong patterns

- Running work is visible outside the open thread.
- User, assistant, and tool activity have different visual grammar.
- Structured context remains visible in the composer and sent message.
- Execution placement, branch, and worktree state are visible.
- Secondary work surfaces remain attached to the thread.
- Desktop uses dense persistent chrome.
- Mobile uses routes, sheets, native controls, and adaptive panes.
- The environment remains the authority across all clients.
- Cloud discovery does not become a content proxy.
- Managed endpoint credentials use narrow trust boundaries.

### Systemic risks

#### P1: large screen controllers

`ChatView`, both sidebars, `ChatComposer`, `ConnectionsSettings`, and major
mobile screens combine state derivation, effects, and rendering. Omega should
use domain controllers, immutable view models, small render components,
explicit commands, and separate async task ownership.

#### P1: activity detail privacy

Agent-awareness state is small, but it sends project title, thread title,
model, headline, detail, and deep link to T3 Tools infrastructure when enabled.
Omega must show the exact field set before activation. Users must be able to
disable publication separately from remote access.

#### P1: managed endpoint operating cost

Each active managed tunnel creates recurring provider cost. T3 reduces this
with shutdown release and per-user limits. Omega must model cost and lifecycle
before it copies the feature.

#### P2: mobile accessibility gaps

Some pending-action controls do not expose complete accessibility metadata.
Direct colors weaken theme and contrast guarantees.

#### P2: two sidebar information architectures

V1 and V2 solve different user models. Keeping both increases settings, test,
and state-transition complexity. Omega should select one primary model and
preserve the other as a filtered view.

#### P2: platform behavior can diverge

Desktop web, hosted web, iOS, and Android use different storage, auth, menu,
header, and editor implementations. Shared contracts reduce semantic drift,
but do not remove interaction drift.

#### P3: compact controls rely on icon knowledge

Several dense controls become clear only after hover tooltips. Touch and
keyboard users do not always receive the same discovery path.

## Impeccable interface score

The scores use a five-point scale.

| Dimension                              | Desktop | Mobile | Evidence                               |
| -------------------------------------- | ------: | -----: | -------------------------------------- |
| hierarchy and information architecture |     4.4 |    4.2 | Strong task and work-surface split     |
| interaction clarity                    |     4.1 |    3.9 | Strong lifecycle, some dense controls  |
| visual coherence                       |     4.4 |    4.2 | Quiet surfaces and consistent state    |
| accessibility and resilience           |     3.7 |    3.3 | Strong desktop keyboard, mobile gaps   |
| responsive and platform fit            |     4.2 |    4.6 | Right panels and adaptive mobile shell |

Overall:

- desktop is 4.2 of 5
- mobile is 4.0 of 5

The product succeeds because the component grammar reflects agent work. It
does not render each event as generic chat.

## Omega replication specification

Omega is a tracked Zed fork and a native GPUI application. It must not add
Electron or React Native to copy T3 desktop presentation. It should adapt the
product grammar to GPUI entities, Zed worktrees, Zed editor and terminal items,
current Omega agent adapters, and OpenAgents authority.

Recommended desktop layers are:

```text
OmegaApp
├── EnvironmentRegistry
├── AgentWorkspaceProjection
├── CloudConnectionBroker
└── WorkspaceWindow
    ├── AgentInboxPanel
    ├── AgentThreadItem
    │   ├── ThreadHeader
    │   ├── TranscriptList
    │   ├── Composer
    │   └── WorkSurfaceDock
    ├── ProjectPanel
    └── StatusAndAttentionModel
```

Each view should read a prepared view model. Each action should dispatch a
typed command. Async work should remain in an entity-owned task.

Desktop implementation order:

1. Build one lifecycle-aware `AgentInboxPanel`.
2. Add stable active, snoozed, and settled ordering.
3. Build a semantic transcript row model.
4. Add compact and expanded tool-call rows.
5. Add structured composer context.
6. Attach native editor, terminal, diff, file, and preview items.
7. Add branch and worktree placement controls.
8. Add connection settings and remote environments.

Mobile parity order:

1. Keep the existing React Native application.
2. Share environment and thread semantics with desktop.
3. Implement adaptive sidebar and detail panes.
4. Use a virtual bottom-anchored feed.
5. Implement semantic work-log rows.
6. Implement collapsed and expanded composer states.
7. Add native terminal and diff modules only when measurements require them.
8. Add complete accessibility metadata before visual polish.

### Cloud recommendation

Omega must not copy T3's Cloudflare Worker, Queue, tunnel, or PlanetScale
stack. OpenAgents policy makes Google Cloud the production infrastructure
authority.

If Omega needs equivalent remote convenience, use:

- Google Cloud Run for account and discovery control plane
- Cloud SQL for bounded link and device records
- Google Cloud Tasks or Pub/Sub for delivery jobs
- Secret Manager for server credentials
- Cloud Logging and Trace for diagnostics
- an admitted Google Cloud or direct-network endpoint design

Keep the T3 boundary:

- cloud identity is not environment authority
- the environment issues final access
- the control plane is not the normal content path
- activity publication is optional
- the activity field set is bounded
- remote access and notifications are separate preferences

Do not copy large all-in-one controllers, two independent sidebar
architectures, icon-only discovery, incomplete accessibility metadata, direct
semantic colors, conflicting cloud providers, undisclosed activity
publication, or tunnel provisioning without limits and cleanup.

## Acceptance checks for Omega

An implementation is not complete until:

- every running task is visible outside its open thread
- active order does not move under the pointer without a user action
- user, assistant, and work rows remain distinct
- a tool row has compact and expanded forms
- approval and user-input requests are actionable without transcript search
- structured context remains visible before and after send
- branch and worktree placement are visible before execution
- desktop keyboard navigation covers all primary surfaces
- mobile controls have role, label, state, and 44-point targets
- mobile feed anchoring survives keyboard and row-height changes
- environment data has one authority
- cloud discovery does not become a second transcript store
- the environment issues final remote access credentials
- notification publication is optional and field-bounded
- cloud resource cost has explicit limits and cleanup
- source builds can omit hosted cloud integration

## Source map

Primary T3 Code source:

- `apps/desktop/src`
- `apps/web/src`
- `apps/mobile/src`
- `apps/mobile/modules`
- `apps/server/src`
- `packages/contracts/src`
- `packages/client-runtime/src`
- `infra/relay`
- `docs/cloud`
- `docs/environment-auth.md`
- `docs/architecture/connection-runtime.md`
- `docs/architecture/remote.md`
- `docs/user/background-service.md`
- `apps/marketing/src/pages/index.astro`
- `apps/marketing/src/pages/privacy-policy.astro`
- `apps/marketing/src/pages/terms-of-service.astro`
- `apps/web/vercel.ts`
- `apps/marketing/vercel.ts`

Public pages:

- [T3 Code](https://t3.codes/)
- [T3 Code GitHub repository](https://github.com/pingdotgg/t3code)
- [T3 Connect relay README](https://github.com/pingdotgg/t3code/blob/main/infra/relay/README.md)
- [T3 Code Terms source](https://github.com/pingdotgg/t3code/blob/main/apps/marketing/src/pages/terms-of-service.astro)
- [T3 Code Privacy source](https://github.com/pingdotgg/t3code/blob/main/apps/marketing/src/pages/privacy-policy.astro)
- [Ping.gg GitHub organization](https://github.com/pingdotgg)
- [Lawn repository](https://github.com/pingdotgg/lawn)
- [UploadThing repository](https://github.com/pingdotgg/uploadthing)

## Final conclusion

T3 Code's main advantage is coherent supervision across clients. Desktop,
hosted web, and mobile do not share view code, but they present the same
environment, thread, activity, and connection model.

The environment remains the execution authority. The clients remain
presentation and command surfaces. T3 Connect adds reachability and awareness
without moving the full coding session into T3 Tools infrastructure.

Omega should reproduce that clarity. It should use Zed and GPUI for desktop,
React Native for mobile, and OpenAgents Google Cloud services only for
admitted hosted features. The target is one task and environment model with
platform-specific presentation, not one visual implementation forced across
all platforms.
