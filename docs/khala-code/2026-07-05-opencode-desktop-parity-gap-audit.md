# Khala Code Desktop / OpenCode Desktop Parity Gap Audit

Date: 2026-07-05
Status: audit / implementation roadmap. First #8435-#8437 implementation passes complete.
Scope: `projects/repos/opencode/packages/desktop`, `projects/repos/opencode/packages/app`, and `clients/khala-code-desktop`.

## Executive Decision

OpenCode desktop is a mature Electron desktop workbench wrapped around the
OpenCode server. Khala Code desktop is a Khala/Codex/Pylon/Fleet shell wrapped
around Codex app-server integration, local Khala services, and newer
AI SDK-shaped runtime work. Khala has stronger OpenAgents-specific surfaces,
but OpenCode has a much broader generic desktop coding app surface.

The largest OpenCode capabilities not present in Khala Code desktop are:

- A native Electron app lifecycle: in-app updater, crash/network logs,
  debug-log export, deep links, single-instance handling, window recovery,
  custom renderer protocol, native permission gates, and rich native menus.
- A multi-window, multi-server workbench: persisted windows/routes, default
  server selection, remote server rows, connection health gate, and eventually
  Windows WSL server lifecycle.
- A full project/session IDE shell: home dashboard, project/workspace sidebar,
  session tabs, file tree, file tabs, diff/review panels, comments, and
  workspace-scoped terminal tabs.
- A richer interactive composer: contenteditable editor, shell mode, model and
  agent selectors, provider-aware controls, image attachments with previews,
  drag/drop/paste attachments, selected-file/line context, prompt history, and
  command/search popovers.
- User-configurable application settings: keybind editor, providers, models,
  themes, language, shell, fonts, notifications, sounds, server management, WSL
  management, update controls, and layout feature toggles.
- A Khala-owned local coding server/runtime surface analogous to OpenCode's
  server, likely sharing authority with Pylon and the existing AI SDK Core lane
  rather than staying only a Codex app-server wrapper.

This audit does not argue that Khala should copy OpenCode wholesale. Khala's
primary product surface is still Codex plus OpenAgents Fleet/Pylon. The useful
reading is: if Khala Code wants to feel like a complete desktop coding app
instead of a specialized Khala control room, these are the missing pieces.

## Comparison Baseline

OpenCode evidence:

- Native shell: `packages/desktop/src/main/index.ts`,
  `packages/desktop/src/main/windows.ts`,
  `packages/desktop/src/main/ipc.ts`,
  `packages/desktop/src/main/menu.ts`,
  `packages/desktop/src/main/updater.ts`,
  `packages/desktop/src/main/logging.ts`,
  `packages/desktop/src/main/server.ts`,
  `packages/desktop/src/main/sidecar.ts`, and
  `packages/desktop/src/main/wsl/*`.
- Renderer bridge: `packages/desktop/src/preload/index.ts` and
  `packages/desktop/src/renderer/index.tsx`.
- Workbench: `packages/app/src/pages/home.tsx`,
  `packages/app/src/pages/layout.tsx`,
  `packages/app/src/pages/session.tsx`,
  `packages/app/src/context/*`,
  `packages/app/src/components/*`,
  `packages/app/src/pages/session/*`, and `packages/app/src/wsl/*`.

Khala Code evidence:

- Product boundary and release lane: `clients/khala-code-desktop/README.md`,
  `clients/khala-code-desktop/package.json`, and
  `clients/khala-code-desktop/electrobun.config.ts`.
- Native shell and local services:
  `clients/khala-code-desktop/src/bun/index.ts`,
  `clients/khala-code-desktop/src/bun/application-menu.ts`,
  `clients/khala-code-desktop/src/bun/rpc-handlers.ts`, and
  `clients/khala-code-desktop/src/shared/rpc.ts`.
- UI shell:
  `clients/khala-code-desktop/src/ui/index.html`,
  `clients/khala-code-desktop/src/ui/main.ts`,
  `clients/khala-code-desktop/src/ui/sidebar.ts`,
  `clients/khala-code-desktop/src/ui/codex-thread-sidebar*.ts*`,
  `clients/khala-code-desktop/src/ui/transcript-render.ts`,
  `clients/khala-code-desktop/src/ui/codex-settings-panel.ts`,
  `clients/khala-code-desktop/src/ui/fleet-*`,
  and `clients/khala-code-desktop/src/ui/inbox.ts`.
- AI SDK-shaped runtime direction:
  `docs/khala-code/2026-07-04-ai-sdk-harness-fork-sandbox-feasibility-audit.md`,
  `packages/khala-ai-sdk-core`,
  `packages/ai-sdk-sandbox-local`, and
  `packages/ai-sdk-sandbox-openagents`.
- VS Code explorer/editor adoption companion:
  [`2026-07-05-vscode-explorer-editor-adoption-audit.md`](./2026-07-05-vscode-explorer-editor-adoption-audit.md).

## Non-Gaps

The following are not counted as OpenCode gaps because Khala intentionally has
different, stronger OpenAgents-specific surfaces:

- Fleet/Pylon controls, own-capacity worker accounting, assignment lifecycle,
  and Khala Sync are Khala-only lanes.
- Codex app-server integration is a current bridge and parity source, not the
  exclusive long-term execution boundary.
- The desired OpenCode-server gap is a Khala-owned server/runtime surface,
  possibly folded into Pylon, with AI SDK-shaped streams and OpenAgents policy
  authority. The gap is not "import OpenCode server verbatim."
- Khala already has extensive tests and smokes for its own product contracts.
  Test gaps below mean "no corresponding OpenCode-style feature tests because
  the feature does not exist", not "Khala has no tests".

## Strategic Corrections

These corrections should guide implementation work that follows this audit:

- Build toward a Khala-owned OpenCode-style local server. Codex app-server
  remains important, but Khala should also have its own runtime/server layer for
  provider selection, AI SDK-shaped streams, tools, permissions, workbench state,
  and Pylon/Fleet integration.
- Reuse the already-started Vercel AI SDK-shaped direction. The narrow
  `@openagentsinc/khala-ai-sdk-core` lane maps AI SDK `streamText` parts into
  OpenAgents runtime events; the sandbox provider packages prove the adapter
  shape. The desktop roadmap should connect those packages to user-facing
  composer, provider, terminal, file, and server surfaces.
- Treat the VS Code explorer/editor adoption audit as the companion design input
  for file tree, editor, tabs, command palette, and navigation behavior. OpenCode
  is the desktop-agent workbench reference; VS Code is the mature explorer/editor
  interaction reference.
- Keep OpenAgents authority outside AI SDK. AI SDK should be a compatibility
  and stream substrate; OpenAgents still owns policy, permissions, secrets,
  sandbox/workspace boundaries, token accounting, and private raw-event storage.
- Postpone WSL implementation, but keep it on the roadmap. It is not a near-term
  blocker for native lifecycle, composer, hotkeys, workbench, and server parity.

## Gap Inventory

### 1. Native App Lifecycle And Distribution

OpenCode has an Electron desktop lifecycle that Khala Code desktop does not
currently match:

- Electron builder packaging with per-channel app IDs/names and desktop
  protocol registration. Khala uses Electrobun and a macOS-oriented release
  lane, but no comparable in-app Electron channel/update substrate.
- `opencode://` deep links, initial deep-link buffering, second-instance
  deep-link handling, and route resolution into the renderer. Khala has no
  observed app URL protocol or deep-link router.
- Single-instance lock and second-instance focus behavior. Khala does not show
  an equivalent desktop instance coordinator.
- In-app updater state machine: check, download, install, manual dialogs,
  renderer subscription, settings action, app-menu action, error surfacing, and
  periodic update checks. Khala has release planning/signing/upload scripts but
  no in-app updater UI/RPC/menu path.
- Native crash reporter, Electron netLog capture, fatal renderer error
  reporting, and user-facing debug log export as a zip. Khala has smokes,
  traces, and logs in product lanes, but no comparable native desktop log
  export/recovery package.
- Renderer unresponsive detection and recovery modals with relaunch/export
  logs/keep waiting/quit choices. Khala does not show an equivalent native
  unresponsive-window recovery workflow.
- Renderer load-failure and render-process-gone recovery paths. Khala does not
  show equivalent native crash/reload affordances.
- System certificate loading, shell environment loading, proxy environment
  handling, and loopback `no_proxy` setup before sidecar start. Khala starts
  Bun/Codex/Pylon services, but does not expose an equivalent general desktop
  environment bootstrap layer.
- Native context menu integration. Khala has custom overlay menus in the web
  UI, not Electron-style system context menus.
- Custom renderer protocol (`oc://renderer`) with path traversal checks and
  a document policy for JavaScript call stacks. Khala uses the Electrobun view
  pipeline and does not show an equivalent custom renderer protocol gate.
- Native permission gating for renderer clipboard and notification requests.
  Khala does not show the same main-process permission request policy.
- Electron-store based scoped renderer storage and cleanup of stale store
  files. Khala has its own JSON/config stores, but not a general renderer
  storage API mirroring OpenCode's `window.api.store`.
- Temporary onboarding test mode that rewires XDG paths and database state.
  Khala has fixture servers and visual smokes, but no matching native desktop
  onboarding isolation mode.

### 2. Native Windows, Menus, And Desktop Controls

OpenCode has a larger desktop window and menu system:

- Persisted multi-window registry and per-window route restoration. Khala
  appears to operate as one primary Electrobun app view.
- Per-window `electron-window-state` geometry persistence. Khala does not show
  an equivalent window-state registry.
- New window, close window, restore all previous windows, and focused-window
  lookup/focus/show IPC. Khala does not expose comparable window management to
  the renderer.
- macOS hidden titlebar and Windows titlebar overlay integration with runtime
  theme/height updates. Khala has a custom web shell, but no equivalent native
  titlebar overlay controls.
- Native app menu categories for File, Edit, View, Go, Window, and Help.
  Khala's menu is currently minimal: app, edit, and window roles.
- Native menu commands for new session, open project, new window, close,
  toggle sidebar, toggle terminal, toggle file tree, reload webview, restart,
  export logs, developer tools, zoom, fullscreen, back/forward, previous/next
  session, previous/next project, docs, support, feedback, bug report, and
  update checks. Khala lacks these system-menu commands.
- Renderer-triggerable desktop menu actions. Khala's native menu is not wired
  into a general command registry.
- Keyboard/webview zoom, pinch zoom preference, and zoom factor persistence.
  Khala does not show comparable user-facing webview zoom controls beyond
  ordinary window role behavior.
- Native notifications with click-to-focus behavior and focused-window gating.
  Khala has product status/inbox indicators, but no equivalent native
  notification bridge was observed.
- Open external URL, open local path, open app, relaunch, and app-exists/path
  resolution APIs. Khala does not expose the same broad native helper API to
  the UI.

### 3. Native File, Directory, Clipboard, And Markdown Bridges

OpenCode exposes desktop-native affordances through preload IPC:

- Native directory picker and directory authorization model. Khala has local
  workspace concepts and Codex filesystem pass-through, but no equivalent
  native directory picker flow.
- Native file picker with attachment authorization budget, `readPickedFile`,
  and release of picked-file grants. Khala has composer file input support, but
  not OpenCode's grant-based native picker/read/release bridge.
- Native save dialog. Khala does not expose a matching save picker.
- Clipboard image reading for prompt attachments. Khala has attachment UI, but
  no observed native clipboard-image IPC.
- Main-process markdown parsing bridge. Khala renders transcript Markdown in
  its UI, but does not have a comparable native markdown IPC API.

### 4. Embedded Local Server And Server Switching

OpenCode desktop owns a local OpenCode server sidecar and makes server choice a
first-class product concept:

- Bundled local server spawned in an Electron utility process with random
  loopback port, generated password, Basic auth, CORS limited to the renderer,
  startup stall timeout, health polling, and controlled shutdown. Khala starts
  Codex app-server/Pylon/Khala Bun services and has AI SDK-shaped runtime
  packages, but not yet an OpenCode-style selectable local HTTP server with
  renderer Basic-auth connection metadata and workbench APIs.
- Automatic sidecar shutdown on quit and process signals. Khala manages local
  services, but does not expose the same server lifecycle as a selectable app
  backend.
- Local sidecar health gate before renderer initialization. Khala has service
  readiness and fixture app-server paths, but no equivalent OpenCode server
  selection gate.
- Default server URL persisted in native storage. Khala is oriented around the
  local Codex app-server and Khala services rather than a user-visible local
  Khala coding server plus optional remote server URLs.
- Remote HTTP server add/edit/remove UI. Khala has no observed server manager
  for arbitrary coding backends.
- ConnectionGate UX that retries health checks and lets the user choose another
  configured server. Khala does not show this alternate-server recovery UI.

Implementation direction: Khala should build its own server/runtime API,
potentially hosted inside Pylon or beside it, rather than depend only on Codex
app-server. That server should expose workbench state, AI SDK-shaped stream
events, tool/permission policy, file/terminal/review APIs, provider/model
selection, and health/default-server semantics to the desktop renderer.

### 5. Windows WSL Server Lifecycle

OpenCode has a full Windows WSL integration that Khala Code desktop does not
show:

- Probe whether WSL is available/installed.
- Install WSL from the desktop UI.
- Discover installed distros and online installable distros.
- Install a distro.
- Probe whether a distro can be added as a server.
- Install/update OpenCode inside the distro.
- Start, stop, add, remove, and default WSL servers.
- Open a WSL terminal.
- Subscribe renderer UI to WSL server events.
- WSL settings rows and an Add WSL Server dialog with runtime-missing,
  distro-install, and server-add states.

Khala has local Pylon/Codex worker concepts and a TUI mode, but no comparable
Windows WSL coding-backend lifecycle. This is desired eventually, but should sit
behind native lifecycle, composer/keybindings, workbench, and local-server work.

### 6. Home Dashboard, Projects, Workspaces, And Session Routing

OpenCode has a broad project/session home that Khala does not currently match:

- Home dashboard with project navigation, recent session groups, session search,
  background-open session tabs, and archive support. Khala opens into the
  Khala/Codex shell with a thread sidebar, not a project dashboard.
- Project sidebar and workspace sidebar, including project rows, avatars,
  status badges, unread/permission/error indicators, ordering, and active
  project navigation. Khala's sidebar is a hotbar for chat, fleet, forum,
  inbox, and settings.
- Open Project and New Session commands as first-class app actions. Khala has
  new chat/thread affordances, not OpenCode's project/session command model.
- Project edit dialog. Khala does not show an equivalent project metadata UI.
- Route model for `/`, `/:dir/session/:id?`, `/new-session`, and
  `/server/:serverKey/session/:id`, with last active URL persisted per window.
  Khala has Codex thread selection and panel state, but no equivalent
  multi-server/project route grammar.
- Titlebar history, tab strip, tab gestures, closed tabs, session sortable
  tabs, and tab memory. Khala has thread sidebar/hotkeys, but not a full
  browser-like session tab system.
- Session lineage/root/child navigation and fork dialog. Khala has Codex thread
  projections, but no observed session lineage/fork UI matching OpenCode's.

### 7. File Tree, File Tabs, Search, And Workspace Context

OpenCode's session workbench includes file-system UI that Khala does not have:

- Lazy file tree provider backed by SDK file listing.
- File content cache with eviction accounting.
- File watcher invalidation and directory sync.
- File tree UI with collapsible folders, file icons, draggable items, and
  changes/all views.
- Diff-kind markers for added/deleted/modified files in the file tree.
- File tabs for opened files.
- File tab scroll persistence and selected-line persistence.
- File search/select dialog integrated with the command palette.
- Add selected file/line context into the prompt.
- View cache for file scroll/selection state.
- Directory picker policy/domain helpers.

Khala exposes Codex filesystem-related RPCs and mention candidates, and it has
source-control/diff steering helpers, but it does not yet provide OpenCode's
file explorer plus editor/tab surface.

### 8. Diff Review, Comments, And Side Panels

OpenCode has a review-oriented side panel stack that Khala only partially
approximates:

- Session side panel with file, context, review, and terminal regions.
- Review tab and review panel v2 with diff kind modeling.
- Comments provider and comment-note utilities.
- Active review toggle and keyboard command.
- Session context metrics/breakdown tabs.
- File/context/review side-panel layout persistence.
- Revert dock tied into the composer region.

Khala has source-control action modeling and diff-review contract tests, but no
observed complete file/review side panel with comments, review tabs, and
integrated file tree.

### 9. Integrated Terminal Workbench

OpenCode has a real workspace terminal surface:

- `ghostty-web` terminal component.
- Workspace-scoped PTY sessions.
- Terminal provider with persisted terminal tabs.
- Terminal websocket URL building and terminal writer helpers.
- Auto-created terminal when the terminal panel opens.
- New terminal command and terminal toggle command.
- Drag/drop reorderable terminal tabs.
- Terminal clone/recover behavior after connect errors.
- Serialized buffer restore.
- Copy, paste, clickable links, theme/font integration, and focus recovery.
- Terminal panel resizing/collapse and v2 panel layout.
- Shell selection setting and terminal font setting.

Khala has Codex background-terminal RPC support and a terminal/TUI mode
contract, but no user-facing OpenCode-style embedded PTY panel with tabbed
interactive terminals.

### 10. Composer And Prompt Input

OpenCode's prompt composer is more like an IDE input surface:

- Contenteditable rich prompt editor with DOM normalization.
- Normal prompt mode and shell prompt mode.
- Prompt history for normal and shell modes.
- Slash popover integrated with command registry.
- `@`/file/context style insertion through context items.
- Model selector in the composer.
- Agent selector/cycle controls.
- Model variant cycling.
- Provider-aware composer controls.
- Image attachments with thumbnails, preview/open, removal, and native source
  path handling.
- Drag/drop attachment overlay.
- Paste handling for rich text and files.
- Native file picker attachment fallback.
- Context items for selected files/lines/comments.
- Request-part builder tests and submission-state modeling.
- Composer docks for follow-up, permission, question, revert, and todo state.

Khala has a command composer, slash commands, attachments, transcript
projection, follow-up handling, and Fleet controls. It does not have the same
contenteditable editor, model/agent/provider controls in-composer, image
attachment previews, native attachment picker, selected-line context, or full
composer dock stack.

### 11. Command Registry, Palette, And Keybind Editing

OpenCode has a central command system:

- Command registry with categories and keybind metadata.
- Searchable command/file/session selection dialogs.
- Global layout commands for sidebar, settings, project open, tab navigation,
  provider connect, project/session switching, and more.
- Session commands for new, undo, redo, compact, fork, share/unshare, file
  open, close tab, add selection, terminal toggle/new, review toggle, file tree
  toggle, focus input, previous/next message, MCP toggle, and auto-accept.
- Composer commands for model, agent, and prompt variants.
- Keybind rendering components and tooltips.
- User-facing keybind settings with search, capture, assign, clear, conflict
  detection, grouped commands, and reset-all.

Khala has thread hotkeys, hotbar shortcuts, slash commands, and app-specific
controls, but no comparable central command registry plus editable keybind UI.

### 12. Providers, Models, MCP, And Permissions UX

OpenCode has provider/model/MCP management surfaces that Khala does not mirror:

- Provider connection dialog for popular providers.
- Custom OpenAI-compatible provider dialog.
- Provider disconnect UI.
- Provider catalog and connected/paid/env/config/custom state.
- Model picker with search, provider grouping, unpaid/usage-exceeded handling,
  and model tooltips.
- Manage models dialog with per-model and per-provider visibility toggles.
- MCP selection dialog with connected/failed/needs-auth/needs-client-registration
  statuses and enable/disable controls.
- Permission provider UI with per-session/directory auto-accept behavior and
  command to toggle auto-accept.
- Permission, question, revert, and todo docks tied to active session work.

Khala has Codex settings projection, model/provider configuration reads and
writes, Codex MCP/plugin/skill marketplace pass-through, and an Inbox for
operator attention. The first provider-catalog slice landed in
[#8461](https://github.com/OpenAgentsInc/openagents/issues/8461), and the
first model/MCP/permission manager slice landed in
[#8462](https://github.com/OpenAgentsInc/openagents/issues/8462), but Khala
still needs runtime-backed provider connect/disconnect authority, durable model
visibility storage, deeper MCP mutation APIs, and fully enforced per-session
permission auto-accept policy.

### 13. Settings, Themes, Locales, Notifications, And Sounds

OpenCode has a much wider application preferences surface:

- Settings v2 tabs for General, Shortcuts, Servers, Providers, and Models.
- Language picker and i18n dictionaries for Arabic, Bosnian, Brazilian
  Portuguese, Chinese, Traditional Chinese, Danish, German, English, Spanish,
  French, Japanese, Korean, Norwegian, Polish, Russian, Thai, Turkish, and
  Ukrainian.
- Theme/color-scheme selection, theme preload, and custom theme support.
- UI font, code font, and terminal font settings.
- Shell setting and terminal/default shell handling.
- Reasoning summaries setting.
- Shell/edit tool-parts expanded settings.
- Feature toggles for new layout designs, file tree, search, status, and
  custom agents.
- Notification settings for agent events, permission events, and errors.
- Sound settings.
- Update and release-notes settings.
- Pinch-zoom and display backend/Wayland settings.
- Server settings and WSL settings embedded in the same preferences model.

Khala has Codex and Claude settings panels, role/model configuration, release
lane data, product-specific feature flags/contracts, and the first app
preferences slice from
[#8463](https://github.com/OpenAgentsInc/openagents/issues/8463). It still
does not show the same broad i18n layer, native OS notification implementation,
native sound playback, server/WSL settings UI, or advanced display-backend
settings.

### 14. Timeline, Status, Error, And Usage UX

OpenCode has app-wide UI models around status and errors:

- Virtualized message timeline model and measurement tests.
- Timeline row reconciliation and hash-scroll behavior.
- Session context usage, metrics, and breakdown UI.
- Status popover body.
- Error page with provider-auth/model-not-found/provider-init-specific
  descriptions and update retry controls.
- Usage exceeded dialogs tied to provider/model state.
- Notification click routing.
- Debug bar.

Khala has transcript rendering, status indicators, rate-limit/capacity panels,
public-safety projection rules, and the first status/usage projection slice
from [#8464](https://github.com/OpenAgentsInc/openagents/issues/8464), but not
this exact OpenCode fully virtualized timeline, hash-scroll navigation,
provider-error dialog, usage-exceeded modal, and context-usage UI stack.

### 15. Sharing, Forking, Archive, And Session Management Extras

OpenCode includes several session-management affordances not observed in Khala:

- Fork dialog and session fork command.
- Share/unshare session commands.
- Session archive model on home.
- Closed-tabs restoration.
- Previous/next message navigation.
- Previous/next session and previous/next project navigation.
- Background-open tab behavior from the home screen.
- Session ownership helpers.

Issue: add session fork, share, archive, tab recovery, and navigation extras
([#8465](https://github.com/OpenAgentsInc/openagents/issues/8465)).

- Status: first implementation pass complete on 2026-07-05. Khala Code now has
  a shared session-action projection, a Settings > Session Actions section, and
  central command-registry entries for fork, share/unshare, archive/unarchive,
  restore closed session, previous/next session, and previous/next message.
  Fork/archive/unarchive call the existing Codex-compatible thread RPCs;
  previous/next session and closed-tab restore reuse the Project Home resume
  path; previous/next message scrolls existing transcript message anchors.
  Share/unshare are intentionally registered but disabled until a Khala-owned
  server/Pylon share record provides an explicit safe backing path. The
  projection never includes transcript bodies or local file content in sharing
  intent metadata.
- Runtime boundary: Codex app-server is only the current fork/archive carrier.
  Khala still wants its own OpenCode-style local server surface, potentially
  folded into Pylon, so share ownership, public links, and cross-runtime
  archive semantics remain server-roadmap work rather than app-server-only
  assumptions.
- Remaining gaps: a true fork dialog, archive list filters from every home and
  project context, previous/next project commands, multi-window closed-tab
  restoration, native menu labels for these commands, and runtime-backed
  share/unshare records.
- Acceptance gates: unit and DOM tests cover fork/share/archive/navigation
  intent, unsupported-runtime disabled states, central command IDs, and safe
  sharing refusal.

### 16. Release Notes, Help, Feedback, And Support Entrypoints

OpenCode surfaces product support from inside the desktop app:

- Release notes dialog.
- Help menu links for docs, support, feedback, and bug reports.
- Updater action components in settings and error surfaces.
- Native menu item for exporting debug logs before support handoff.

Issue: add in-app release notes, help, feedback, support, and bug-report
entrypoints ([#8466](https://github.com/OpenAgentsInc/openagents/issues/8466)).

- Status: first implementation pass complete on 2026-07-05. Khala Code now has
  a shared support-entrypoint policy, Settings > Help And Support links,
  command-registry actions for release notes, docs, support, feedback, bug
  report, diagnostics export, and copying issue metadata, plus native Help menu
  entries for docs/support/feedback/bug report. Release notes reuse the #8440
  updater status/build metadata; diagnostics export hands off to the #8441
  debug-log service; native Help menu routing extends the #8442 menu surface.
- Safety boundary: support URLs are allowlisted to `openagents.com` and the
  `OpenAgentsInc/openagents` GitHub repository before opening through the
  desktop bridge. Copied issue metadata is public-safe by construction: it
  records version/channel/state/counts and explicitly excludes transcripts,
  raw logs, tokens, and private local paths.
- Remaining gaps: a richer release-notes dialog, customer-support backend,
  automatic attachment of exported diagnostics to a support flow, and complete
  native menu parity for every command-registry label.

### 17. Test Coverage For The Missing Surfaces

OpenCode's desktop app has tests around many of the above surfaces:

- Native desktop tests for updater controller/subscriptions, shell env, store
  cleanup, window registry, attachment picker, onboarding initialization, WSL
  servers, and renderer initialization.
- App tests for command/keybind registry, file tree, file watcher/cache,
  terminal provider/panel, timeline projection/measurement, settings/provider
  dialogs, theme preload, titlebar tabs/history/gestures, server health/scope,
  global sync, permission auto-respond, prompt input, attachment building, and
  many i18n/parity helpers.

Khala has its own strong contract, unit, live-smoke, visual-smoke, Fleet, Codex,
Claude, and safety test suites. It lacks corresponding tests for the
OpenCode-style native lifecycle, file tree/editor/review workbench, terminal
panel, keybind editor, provider catalog, WSL servers, server manager, and
in-app updater because those features are absent.

## First Roadmap / Issue Set

These are written as implementation issues that should be filed or mapped onto
existing issues. P0 means "start now"; P1 means "next wave"; P2 means "keep
explicitly scoped, but do not block the first usable parity push."

Opened issue set:

- Active editor track already in progress:
  [#8430](https://github.com/OpenAgentsInc/openagents/issues/8430),
  [#8431](https://github.com/OpenAgentsInc/openagents/issues/8431),
  [#8432](https://github.com/OpenAgentsInc/openagents/issues/8432),
  [#8433](https://github.com/OpenAgentsInc/openagents/issues/8433), and
  [#8434](https://github.com/OpenAgentsInc/openagents/issues/8434).
- New parity issues from this audit:
  [#8435](https://github.com/OpenAgentsInc/openagents/issues/8435),
  [#8436](https://github.com/OpenAgentsInc/openagents/issues/8436),
  [#8437](https://github.com/OpenAgentsInc/openagents/issues/8437),
  [#8438](https://github.com/OpenAgentsInc/openagents/issues/8438),
  [#8439](https://github.com/OpenAgentsInc/openagents/issues/8439),
  [#8440](https://github.com/OpenAgentsInc/openagents/issues/8440),
  [#8441](https://github.com/OpenAgentsInc/openagents/issues/8441),
  [#8442](https://github.com/OpenAgentsInc/openagents/issues/8442),
  [#8443](https://github.com/OpenAgentsInc/openagents/issues/8443),
  [#8444](https://github.com/OpenAgentsInc/openagents/issues/8444),
  [#8445](https://github.com/OpenAgentsInc/openagents/issues/8445), and
  [#8446](https://github.com/OpenAgentsInc/openagents/issues/8446).
- Next-wave parity issues:
  [#8459](https://github.com/OpenAgentsInc/openagents/issues/8459),
  [#8460](https://github.com/OpenAgentsInc/openagents/issues/8460),
  [#8461](https://github.com/OpenAgentsInc/openagents/issues/8461),
  [#8462](https://github.com/OpenAgentsInc/openagents/issues/8462),
  [#8463](https://github.com/OpenAgentsInc/openagents/issues/8463),
  [#8464](https://github.com/OpenAgentsInc/openagents/issues/8464),
  [#8465](https://github.com/OpenAgentsInc/openagents/issues/8465), and
  [#8466](https://github.com/OpenAgentsInc/openagents/issues/8466).

Do not duplicate [#8430](https://github.com/OpenAgentsInc/openagents/issues/8430)
through [#8434](https://github.com/OpenAgentsInc/openagents/issues/8434):
another agent is already working the Phase 1 read-only editor source-browser
slice.

Issue: add file explorer, tabs, search, and workspace context
([#8459](https://github.com/OpenAgentsInc/openagents/issues/8459)).

- Status: first next-wave implementation pass complete on 2026-07-05. The
  existing Editor hotbar surface now has a compact workspace search field,
  visible added/modified/deleted tree markers when a provider supplies
  `changeKind`, an OpenCode-style open-files tab strip with switch/close
  behavior, and selected-line/file context tests that prove the composer handoff
  stays local to the typed editor contract. The implementation continues to
  follow the VS Code explorer/editor adoption audit: stable node identity, lazy
  tree hydration, keyboard tree navigation, read-only Monaco models, and a
  Khala-owned local workspace provider. Editing, dirty tabs, file watchers, LSP,
  and multi-window editor state remain follow-up work.
- Scope: file explorer projection, read-only file tabs, loaded-tree file search,
  selected file/line context, and typed change markers.
- Acceptance gates: DOM tests cover lazy tree projection, tab open/switch/close,
  file search selection, selected-line context insertion, binary/oversized file
  refusal, and typed workspace service errors without public-safe file-content
  leakage.

### P0: Composer And Prompt Input

Goal: emulate OpenCode's composer closely. This is the highest-priority product
gap because it is the most frequently used interaction surface.

Issue: build the rich contenteditable composer foundation
([#8435](https://github.com/OpenAgentsInc/openagents/issues/8435)).

- Status: first implementation pass complete on 2026-07-05. Khala Code desktop
  now uses a contenteditable plaintext composer surface with DOM/plain-text
  normalization, paste normalization, explicit normal/shell mode state, per-mode
  prompt history, slash-command continuity, IME-safe Enter handling, and
  multiline visual-smoke coverage. Model/provider controls, composer docks, and
  richer attachment previews remain in the follow-on issues below.
- Scope: contenteditable editor, DOM normalization, multiline handling, prompt
  history, normal mode, shell mode, slash popover integration, and keyboard-safe
  submission behavior.
- Acceptance gates: unit tests for editor DOM transforms, prompt history,
  normal/shell submission, paste normalization, and visual smoke coverage for
  long text, multiline text, and empty/error/loading states.

Issue: add model, agent, provider, and variant controls in the composer
([#8436](https://github.com/OpenAgentsInc/openagents/issues/8436)).

- Status: first implementation pass complete on 2026-07-05. The composer now
  renders model, provider, agent-role, reasoning, and variant/service-tier
  controls from the existing Codex settings projection; selector changes write
  the current Codex config keys, and submitted turns carry a sanitized
  `composerSelection` object for runtime routing and the AI SDK Core lane.
  Provider secrets and raw provider payloads remain outside renderer state and
  support-safe projections.
- Scope: OpenCode-style model selector, agent selector/cycle, provider-aware
  display, variant cycling, usage/quota/error hints, and direct integration with
  Khala's AI SDK Core/provider direction.
- Acceptance gates: fixture providers render in the composer, selection changes
  affect the submitted turn, unavailable providers produce legible errors, and
  the UI never exposes raw provider secrets.

Issue: add attachment parity
([#8437](https://github.com/OpenAgentsInc/openagents/issues/8437)).

- Status: first implementation pass complete on 2026-07-05. Khala Code desktop
  now has native picker grant/read/release RPCs, private-path grants held only
  in the Bun handler, public grant labels for workspace-relative files, native
  grant release on attachment removal/reset/window unload, and native image
  grants routed through the existing local attachment uploader. Browser file
  input, paste/drop attachments, image previews, oversized paste refusal, and
  public-safe transcript projection stay on the shared composer-state path. The
  native directory picker, save-dialog, clipboard-image, and deterministic grant
  expiry bridge landed in the next-wave native bridge issue below.
- Scope: native file picker bridge, drag/drop files, paste files/images,
  clipboard image import, image thumbnails, preview/open, removal, source-path
  metadata, attachment budgets, and release of native file grants.
- Acceptance gates: tests for picker grants, file release, paste/drop handling,
  image preview rendering, oversized file refusal, and public-safe transcript
  projection.

Issue: add native file, directory, save, and clipboard-image bridges
([#8460](https://github.com/OpenAgentsInc/openagents/issues/8460)).

- Status: first implementation pass complete on 2026-07-05. Khala Code desktop
  now exposes typed native directory picker, save-dialog, and clipboard-image
  RPCs beside the existing file picker/grant bridge. Native file grants carry a
  public expiry timestamp, expire deterministically in the Bun handler, and can
  be backed either by a private local path or in-memory clipboard image bytes.
  Renderer preview classifies all native picker/grant/clipboard/save methods as
  mutating so browser preview cannot inspect local native state. The composer
  paste path now asks the native bridge for an image when the web clipboard has
  no files or text.
- Scope: directory picker, save dialog, clipboard-image import, expiring grants,
  preview read-only policy, and composer paste fallback.
- Acceptance gates: fixture tests cover pick-grant-read-release, grant expiry,
  directory picker, save cancellation, clipboard unavailable, and clipboard
  image grant/read states. App-shell and preview tests cover renderer wiring and
  mutating RPC classification.

Issue: add provider catalog, connection, and custom OpenAI-compatible provider
UI ([#8461](https://github.com/OpenAgentsInc/openagents/issues/8461)).

- Status: first implementation pass complete on 2026-07-05. Khala Code desktop
  now has a provider catalog settings section projected from the existing Codex
  settings/model-list surface, with connected, missing-auth, env-configured,
  paid, disabled, and custom states. The section can select/disconnect through
  the current `model_provider` config write path, validates custom
  OpenAI-compatible provider metadata locally, and keeps API-key/secret values
  out of renderer text and provider projections. This is intentionally aligned
  with the AI SDK-shaped runtime direction: the renderer owns provider UX and
  sanitized intent; Codex, Pylon, Fleet, and the Khala-owned runtime/server own
  execution authority and credential handling.
- Scope: provider catalog projection, connection/disconnection intent display,
  custom OpenAI-compatible provider metadata validation, renderer settings
  section, composer/provider selection continuity, and secret-blind tests.
- Acceptance gates: tests cover provider list projection, connection next-step
  intents, custom provider validation with URL credential scrubbing, settings
  UI selection writes, and app-shell wiring. Remaining work is runtime-backed
  provider registration, durable custom-provider storage, richer billing/auth
  status, model visibility management, and MCP picker parity.

Issue: add model manager, MCP status picker, and permission auto-accept controls
([#8462](https://github.com/OpenAgentsInc/openagents/issues/8462)).

- Status: first implementation pass complete on 2026-07-05. Khala Code desktop
  now has a combined Settings section for searchable model management, MCP
  status, and permission controls. The model manager groups by provider,
  filters by search term, and supports local visibility toggles while keeping
  runtime-hidden models disabled. The MCP picker projects Codex ecosystem MCP
  state into connected, failed, needs-auth, needs-registration, and disabled
  rows with typed enable/disable/authentication intent instead of pretending the
  renderer owns MCP OAuth or server registration. Permission profile changes
  continue through the Codex `default_permissions` config path, and
  per-session/directory auto-accept controls are disabled with explanatory copy
  when Codex managed requirements own the policy boundary.
- Scope: model search/grouping/local visibility, MCP status projection and
  intent, permission profile selector, auto-accept mode projection, settings
  section wiring, and secret-blind tests.
- Acceptance gates: tests cover model visibility changes, hidden model
  filtering, MCP status rendering, MCP enable/auth intent, permission profile
  writes, auto-accept managed-policy disabling, and source wiring. Remaining
  work is durable model visibility storage, real MCP enable/disable
  app-server/runtime APIs, server-owned OAuth flows, and enforced
  per-session/directory permission policy in the Khala-owned runtime/Pylon
  boundary.

Issue: expand settings for themes, fonts, notifications, sounds, and feature
toggles ([#8463](https://github.com/OpenAgentsInc/openagents/issues/8463)).

- Status: first implementation pass complete on 2026-07-05. Khala Code desktop
  now has a typed app-preferences settings section backed by a
  `khala-code-desktop.app-preferences.v1` renderer storage contract. It covers
  color scheme, UI/code/terminal font preferences, notification toggles for
  agent events, permission events, errors, and completions, sound toggles plus
  volume, and layout feature toggles for compact composer, dense workbench,
  provider diagnostics, and terminal tabs. Preferences apply to the root
  document through `data-khala-*` attributes and CSS variables while defaults
  preserve the current Khala look, notifications-on/sounds-off behavior, and
  existing app density unless the user opts in.
- Scope: typed preference parser/storage/reset, root preference application,
  Settings UI controls, narrow-layout CSS, and tests for read/write/defaults,
  theme/font application, and notification/sound toggle projection.
- Postponed intentionally: full i18n dictionaries, native OS notification
  delivery, native sound playback, Windows/Wayland/WSL display backend
  settings, and deep terminal settings until the terminal parity issue lands.

Issue: add timeline, status, provider-error, and usage UX
([#8464](https://github.com/OpenAgentsInc/openagents/issues/8464)).

- Status: first implementation pass complete on 2026-07-05. Khala Code desktop
  now has a shared status/usage projection for transcript timeline metrics,
  token/usage breakdown, runtime health rows, and provider-specific error
  classes. A Settings status panel renders message/tool-call counts,
  virtualization usefulness, token sync/missing-usage state, runtime degraded
  or unavailable rows, and provider error entries that distinguish provider
  auth, model unavailable, quota/rate limit, local server unavailable, and
  generic failure. Error details run through the existing diagnostics redactor
  before projection so public-safe UI/tests never retain provider credentials.
- Scope: timeline metrics, usage breakdown, runtime health summary,
  provider-error classification, status settings panel, and tests for
  status/error rendering and secret redaction.
- Remaining work: true transcript virtualization and measurement in the main
  message list, hash/anchor navigation, dedicated provider-error and
  usage-exceeded dialogs, notification-click routing into the active session,
  and richer context-window usage once the runtime exposes that data.

Issue: add composer context and docks
([#8437](https://github.com/OpenAgentsInc/openagents/issues/8437)).

- Status: first implementation pass complete on 2026-07-05. The editor source
  pane can add selected file/line context chips, diff-review comments stage as
  selected composer context, submitted prompts include an explicit selected
  context summary, and the composer renders keyboard-reachable follow-up,
  permission, question, revert, todo, and request-tree work-state buttons. The
  read-only preview bridge now classifies local native grant RPCs as mutating
  so preview mode cannot use them to inspect local files.
- Scope: selected file/line/comment context, follow-up dock, permission dock,
  question dock, revert dock, todo dock, and request tree.
- Acceptance gates: context survives thread switching, docks are keyboard
  reachable, permission and question states are explicit, and rejected/expired
  states never look successful.

### P0: Command Registry, Hotkeys, And Keybind Editor

Goal: adopt OpenCode's command/keybind model as a first-class Khala primitive.

Issue: create a central desktop command registry
([#8438](https://github.com/OpenAgentsInc/openagents/issues/8438)).

- Status: first implementation pass complete on 2026-07-05. Khala Code now has
  a typed desktop command registry with stable command IDs, categories,
  default keybindings, availability predicates, disabled reasons, and
  analytics-safe metadata. The visible hotbar now routes through registry
  commands, the core view/session/composer actions are registered, and the
  existing recent-thread number/cycle hotkeys are documented as the first
  explicit compatibility exception because their resolution depends on live
  thread order. File/editor, terminal/review, deeper server actions, native
  menu wiring, and user-editable keybinds remain follow-up slices.
- Scope: command IDs, categories, default keybinds, availability predicates,
  menu binding, slash binding, palette binding, and analytics-safe execution
  metadata.
- Acceptance gates: every sidebar, composer, session, file, terminal, review,
  settings, and server action routes through the registry or has an explicit
  exception.

Issue: build the command palette
([#8438](https://github.com/OpenAgentsInc/openagents/issues/8438)).

- Status: first implementation pass complete on 2026-07-05. `Cmd+K`/`Ctrl+K`
  opens a grouped command palette with command, project, active-session,
  selected file/context, model, provider, and server-action records. The
  palette supports deterministic search ordering, keyboard selection,
  execution, loading, empty, and disabled states. Non-command records navigate
  to their current best owning surface for this pass: file/project to Editor,
  session to Chat, model/provider to Settings, and server refresh through the
  existing session refresh command. DOM tests cover search/navigation/disabled
  and empty/loading states, while the composer visual smoke now captures and
  asserts the command palette on desktop and mobile Khala Code viewports.
- Scope: searchable commands, files, sessions, projects, models, providers, and
  server actions with grouped results and keyboard navigation.
- Acceptance gates: palette opens from the default hotkey, resolves collisions
  deterministically, supports empty/loading/error states, and is covered by DOM
  and visual tests.

Issue: build editable keybindings
([#8439](https://github.com/OpenAgentsInc/openagents/issues/8439)).

- Status: first implementation pass complete on 2026-07-05. Khala Code now
  loads command keybinding overrides from local desktop storage before command
  registration, applies them to hotkey dispatch and palette labels, and exposes
  a Settings > Keybindings section backed by the command registry. The section
  supports search, grouped command rows, click-to-capture assignment, clear,
  reset one, reset all, and visible conflict blocking that names the affected
  commands before saving. Sidebar tooltip/visible labels now reflect effective
  registered view keybindings. Native menu label reflection remains with
  [#8442](https://github.com/OpenAgentsInc/openagents/issues/8442) because the
  current native menu only contains static platform-role items, not registered
  command actions.
- Scope: keybind settings panel, capture, assign, clear, reset all, conflict
  detection, grouped command list, tooltips, and migration for existing Khala
  hotkeys.
- Acceptance gates: conflicts are visible before save, custom keybinds persist,
  defaults can be restored, and native menu labels reflect effective keybinds.

### P0: Native Lifecycle Basics

Goal: make Khala Code behave like a dependable desktop app before expanding the
workbench.

Issue: add in-app updater plumbing
([#8440](https://github.com/OpenAgentsInc/openagents/issues/8440)).

- Scope: update check, update state subscription, download/install controls,
  periodic checks, settings row, app-menu item, error state, and release-notes
  entrypoint.
- Acceptance gates: fixture update server tests, no silent install, legible
  failure states, and no raw signing/notarization secrets in logs.

Issue: add diagnostics and recovery
([#8441](https://github.com/OpenAgentsInc/openagents/issues/8441)).

- Scope: crash/load failure handling, unresponsive-window recovery, debug-log
  export zip, fatal renderer error reporting, net/process/service log capture,
  restart/relaunch actions, and support handoff metadata.
- Acceptance gates: exported bundle is public-safe by default, load failure can
  recover or quit cleanly, and visual smoke covers recovery modals.

Issue: add deep links and single-instance handling
([#8442](https://github.com/OpenAgentsInc/openagents/issues/8442)).

- Status: first implementation pass complete on 2026-07-05. Khala Code now has
  a typed `khala-code://` deep-link parser for view, thread/session, project,
  and server targets; renderer boot can consume an app-link URL from the actual
  location or an encoded `khala-code-url` query/hash and route it into the
  initial view/thread restore path. Invalid links are harmless typed failures,
  path-like private project IDs are rejected, and unsupported targets fall back
  to normal persisted view/thread state.
- Follow-up (2026-07-05, same day): the first pass parsed links but never
  registered `khala-code://` as an OS URL scheme, never handled the native
  `open-url`/`reopen` events, and had no single-instance coordination at all --
  so a real double-click on a `khala-code://` link, or a second app launch,
  could not reach the parser in practice (only a link already present in the
  renderer's boot-time `location` could route, which nothing outside the app
  ever sets). Closed the gap: `electrobun.config.ts` now registers the
  `khala-code` URL scheme; a new `src/bun/single-instance-lock.ts` binds a real
  local Unix domain socket so a second launch forwards its link and focuses the
  existing window instead of opening a duplicate; a new
  `src/bun/deep-link-coordinator.ts` buffers a cold-launch link until the
  renderer reports ready (`rendererReady` RPC) and flushes it via a new
  `deepLinkTarget` push message, reusing the existing `../shared/deep-links.ts`
  parser end to end.
- Scope: `khala-code://` protocol, first-open buffering, second-instance focus,
  route resolution into thread/project/session/server targets, and invalid-link
  errors.
- Acceptance gates: deep links work cold and warm, invalid links are harmless,
  and no link can bypass workspace/auth/policy gates.

Issue: expand the native menu
([#8442](https://github.com/OpenAgentsInc/openagents/issues/8442)).

- Status: first implementation pass complete on 2026-07-05. The native menu now
  has File, Edit, View, Go, Window, and Help groups. Platform-native edit/window
  roles stay native, while app actions expose the same command IDs used by the
  command registry and palette for new session, project home, settings, command
  palette, view switches, previous/next session, previous/next message, release
  notes, docs, support, feedback, and bug report. Help link dispatch remains
  allowlisted through the support-entrypoint policy from #8466; updater and
  diagnostics items continue through #8440/#8441.
- Follow-up (2026-07-05, same day): the View menu's "Reload", "Toggle Full
  Screen", and "Toggle Developer Tools" items used Electrobun menu roles that
  do not exist (`"reload"`, `"togglefullscreen"`, `"toggleDevTools"` --
  Electrobun's real role set only has `"toggleFullScreen"`, capital F), so
  those three rendered as blank, non-functional rows. Fixed: Reload and Toggle
  Developer Tools now round-trip through a new `nativeWindowAction` RPC (same
  bun-side handler a hotkey or the command palette reaches), Toggle Full
  Screen uses the real `toggleFullScreen` role, and File > New Window plus
  View > Zoom In/Out/Actual Size were added through the same mechanism (all
  previously entirely missing from the menu).
- Scope: File, Edit, View, Go, Window, Help, command-registry integration,
  New Session, Open Project, New Window, Close, Reload, Restart, Export Logs,
  Settings, command palette, terminal/file tree/review toggles, docs/support,
  zoom, fullscreen, and developer tools in dev builds.
- Acceptance gates: menu actions share the same command IDs as hotkeys/palette,
  disabled states are correct, and platform-specific menu roles stay native.

### P1: Workbench Basics

Goal: make Khala Code feel like a desktop coding workbench, not only a chat and
Fleet control surface. This workstream should cross-check OpenCode against
[`2026-07-05-vscode-explorer-editor-adoption-audit.md`](./2026-07-05-vscode-explorer-editor-adoption-audit.md)
before implementing explorer/editor primitives.

Issue: add project/home dashboard
([#8443](https://github.com/OpenAgentsInc/openagents/issues/8443)).

- Scope: project list, recent sessions, session search, open project, new
  session, background-open session behavior, status badges, and route
  persistence.
- Acceptance gates: projects/sessions can be navigated without losing active
  work, empty/loading/error states are clear, and route state survives restart.

Issue: add file tree and source pane
([#8430](https://github.com/OpenAgentsInc/openagents/issues/8430),
[#8431](https://github.com/OpenAgentsInc/openagents/issues/8431),
[#8432](https://github.com/OpenAgentsInc/openagents/issues/8432),
[#8433](https://github.com/OpenAgentsInc/openagents/issues/8433),
[#8434](https://github.com/OpenAgentsInc/openagents/issues/8434)).

- Scope: lazy file tree, file watcher invalidation, file content cache, open
  file tabs, selected-line persistence, search/select file dialog, diff markers,
  add-to-composer context, and the VS Code audit's explorer/editor interaction
  guidance.
- Acceptance gates: large repos stay responsive, file changes invalidate only
  the right nodes, selected context submits correctly, and no private local path
  leaks into public-safe projections.
- Current issue boundary: [#8430](https://github.com/OpenAgentsInc/openagents/issues/8430)
  through [#8434](https://github.com/OpenAgentsInc/openagents/issues/8434)
  cover provider-neutral file service, Editor hotbar/panel shell, lazy file tree,
  Monaco read-only source pane, and verification. File tabs, editing, save,
  rename/delete, watchers, compact folders, file nesting, and dirty state remain
  follow-up work after that active Phase 1 slice lands.

Issue: add diff/review panel
([#8444](https://github.com/OpenAgentsInc/openagents/issues/8444)).

- Scope: review tab, diff kind modeling, comments, revert actions, active diff
  focus, side-panel layout, and source-control action integration.
- Acceptance gates: added/modified/deleted files render correctly, comments and
  revert states are explicit, and review state remains stable across session
  switches.

Issue: add embedded terminal
([#8445](https://github.com/OpenAgentsInc/openagents/issues/8445)).

- Status: first implementation pass complete on 2026-07-05. Khala Code now has
  a Terminal hotbar slot, `view.terminal` command, native View/Go menu entries,
  and a workbench terminal panel backed by the existing Codex app-server
  background-terminal RPCs. The panel is bounded to the active session/thread,
  renders background terminal tabs, supports switch, refresh, clean exited,
  terminate running, and copy-output actions, and deliberately disables
  interactive "New Terminal" creation until a Khala-owned local PTY bridge or
  local server terminal transport exists.
- Transport decision: this pass uses Codex background terminals for inspection
  and lifecycle management only. It is intentionally separated from a future
  interactive local PTY so reconnect/recover cannot duplicate commands or leak
  stale output across sessions.
- Scope: workspace-scoped PTY tabs, terminal websocket or local bridge, tab
  persistence, resize/collapse, copy/paste, clickable links, shell/font/theme
  settings, reconnect/recover, and command-registry actions.
- Acceptance gates: terminal sessions are bounded to the selected workspace,
  account-home defaults are explicit, reconnect does not duplicate commands, and
  background-terminal Codex APIs are either integrated or deliberately separated.

### P1: Khala-Owned Local Server Runtime

Goal: build Khala's version of OpenCode's server layer. The local server can be
folded into Pylon or run beside it, but the desktop should have a coherent
server/runtime API of its own.

Issue: define the local server contract
([#8446](https://github.com/OpenAgentsInc/openagents/issues/8446)).

- Scope: health, authentication, renderer CORS/origin policy, server identity,
  project/session routes, provider/model listing, stream events, tool calls,
  permissions, file APIs, terminal APIs, review APIs, and lifecycle controls.
- Acceptance gates: contract tests cover success, refusal, auth failure,
  health failure, restart, and version skew.
- 2026-07-05 first pass: `clients/khala-code-desktop/src/shared/local-server-runtime.ts`
  now defines the desktop-visible Khala local server capability contract and
  projects runtime rows for Khala Local Server, Pylon, Codex app-server bridge,
  and Khala AI SDK Core. This deliberately keeps Codex app-server as an
  important bridge for Codex threads/approvals, not Khala's whole execution
  boundary.

Issue: bridge AI SDK Core into the local server
([#8446](https://github.com/OpenAgentsInc/openagents/issues/8446)).

- Scope: wire `@openagentsinc/khala-ai-sdk-core` into a desktop-visible runtime
  path, map AI SDK stream parts into OpenAgents events, preserve tool authority,
  and keep raw chunks private or discarded.
- Acceptance gates: text, reasoning, usage, finish reason, tool call/result,
  providerOptions, and provider error fixtures all render through the same
  transcript consumer as existing Codex/Pylon events.
- 2026-07-05 first pass: the desktop manager exposes `Khala AI SDK Core` as a
  planned/ready runtime lane and states that AI SDK stream parts map into
  OpenAgents events while OpenAgents keeps tool authority. Full stream fixture
  parity remains the next implementation step.

Issue: add server manager UI
([#8446](https://github.com/OpenAgentsInc/openagents/issues/8446)).

- Scope: local server row, default server, remote server add/edit/remove,
  health retry, alternate server chooser, and settings integration.
- Acceptance gates: switching servers does not corrupt local session state,
  unhealthy servers are obvious, and remote server credentials never enter
  public traces.
- 2026-07-05 first pass: Settings now includes a `Local Server Runtime` manager
  section with health refresh, disabled lifecycle/default-server actions until
  credential-safe storage and lifecycle controllers exist, and a credential
  policy that keeps remote server credentials out of renderer logs, support
  bundles, command palette records, and public traces.

### P2: WSL Server Lifecycle

Goal: keep Windows WSL parity explicit, but do it after the first desktop parity
push.

Issue: add WSL discovery and setup.

- Scope: WSL availability, installed distro discovery, online distro catalog,
  install WSL, install distro, and open terminal.
- Acceptance gates: Windows-only code is isolated, non-Windows platforms hide
  the flow, and setup failures are recoverable.

Issue: add WSL coding servers.

- Scope: install/update Khala/OpenAgents runtime inside distro, add/remove/start
  WSL server, default WSL server, event subscription, and settings rows.
- Acceptance gates: WSL server health is visible, server removal is safe, and
  workspace paths are translated intentionally.

### P2: Preferences And Support Surface

Goal: fill in the remaining desktop polish once core interaction surfaces are
landed.

Issue: broaden settings.

- Scope: providers, models, MCP picker, i18n, theme/color scheme, UI/code/
  terminal fonts, shell, notifications, sounds, updates, display backend, and
  feature toggles.
- Acceptance gates: settings are searchable, changes are reversible, defaults
  are explicit, and unavailable platform settings are hidden or disabled.

Issue: add help/support/release notes.

- Status: first pass landed through
  [#8466](https://github.com/OpenAgentsInc/openagents/issues/8466): allowlisted
  support links, command-registry actions, native Help menu entries, exported
  diagnostics handoff, and public-safe issue metadata.
- Scope: release notes dialog, docs/support/feedback/bug links, exported logs
  entrypoint, and public-safe support metadata.
- Acceptance gates: support flow can be used without exposing secrets, release
  notes match the current build channel, and links are native-opened safely.

## Boundary Notes

- Do not weaken Khala's public-safety, own-capacity, token-accounting, or
  projection invariants to copy OpenCode UI speed. Khala's Fleet surfaces are
  higher-trust than OpenCode's generic coding UI and must keep their stricter
  constraints.
- Build a Khala-owned server/runtime layer, but keep Codex app-server as an
  important bridge where it is the right authority for Codex threads, approvals,
  sandboxing, MCP/plugins/skills/session state, and app-server protocol parity.
- Prefer adopting OpenCode-style user affordances around OpenAgents-owned
  contracts: file explorer via Codex or Khala server filesystem APIs, terminal
  via Codex background-terminal APIs or a deliberate PTY contract, provider
  selection via AI SDK/OpenAgents provider APIs, and deep links into Khala's
  thread/project/session/server model.
