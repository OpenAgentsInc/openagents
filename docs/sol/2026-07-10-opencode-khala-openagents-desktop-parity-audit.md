# OpenCode, Khala Code, and OpenAgents Desktop parity audit

- Class: historical competitor-capability and implementation baseline
- Date: 2026-07-10
- Snapshot status: pinned; non-dispatch
- Current state authority: `apps/openagents-desktop`, its guarantees/tests,
  the Sol master roadmap, live issues, and receipts

Destination: `apps/openagents-desktop`

Legacy extraction source: `clients/khala-code-desktop`

Benchmark: `anomalyco/opencode` desktop and shared app packages

> **Historical baseline warning (2026-07-12):** The 20-area competitor
> capability model remains useful, but the OpenAgents implementation score and
> gap prose are pinned to this audit's source snapshot. CUT-13 through CUT-25
> and later Desktop work have invalidated it as current status. A future score
> must be a new dated audit. Relevant Khala Code MVP ideas also feed the Effect
> Native mobile remote-coding plan; see
> [`2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md`](./2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md).

## Executive decision at the snapshot

At this snapshot, OpenAgents Desktop was a secure, runnable development prototype, not yet a
desktop coding workbench and not yet a releasable desktop product.

Khala Code did substantially more OpenCode-parity work than the greenfield app
currently carries. It has broad first-pass coverage across the composer,
commands, keybindings, project home, editor, review, terminal inspection,
provider/model/MCP projections, updater, diagnostics, deep links, native menus,
support, and OpenAgents-only Fleet/Gym/Inbox surfaces. That work is useful
behavioral and service-extraction evidence. It is not a shippable predecessor:
the Electrobun app is frozen, its release commands are retired guards, several
of its parity surfaces are intentionally shallow or fixture-driven, and its
largest architectural gaps remain a real Khala-owned local server, interactive
PTYs, complete editing, multi-window/server behavior, and WSL.

At this snapshot, the greenfield OpenAgents Desktop had one landed foundation,
six real but
narrow partial slices, three scaffolds, and ten absent areas across the 20-area
matrix below. Its then-current product consisted of:

- a hardened Electron + Effect Native host;
- a local five-thread conversation store and non-streaming gateway call;
- a folder picker, bounded root listing, and bounded read-only file preview;
- a source-checkout-only Pylon bridge for Codex account readiness/device auth;
- a typed but currently unreachable Fleet brief adapter; and
- unit/build/smoke coverage for those limited surfaces.

The resulting recommendation was not a wholesale UI port: extract the proven behavior
and host services from Khala Code, adopt OpenCode's workbench interaction model,
keep Effect Native as the only application/component/intent architecture, and
connect each surface to current OpenAgents/Pylon/Sync authority before calling
it parity.

## Reproducible snapshot

This audit is pinned so later work can distinguish implementation drift from a
different reading of the same code.

| Source | Snapshot | Notes |
| --- | --- | --- |
| OpenAgents monorepo | `fc4e8e1bce0d72f8f1181a5d4e7bd492b7e2cd29` on `main` | Clean and equal to `origin/main` before this audit commit; the final two pulled commits did not touch audited desktop paths |
| OpenCode upstream | `1c7f65f1057b587adb131d6683e273fa55007f35` on `origin/dev` | 2026-07-10; desktop/app package version `1.17.18` |
| OpenCode local worktree | `d3459eb7403cbb33c197621777409954e9a1312f` | Left untouched; 109 commits behind `origin/dev` |
| Live OpenAgents Desktop issue | [OpenAgentsInc/openagents#8574](https://github.com/OpenAgentsInc/openagents/issues/8574) | Open; greenfield Electron + Effect Native destination |
| OpenAgents app epic | [OpenAgentsInc/openagents#8566](https://github.com/OpenAgentsInc/openagents/issues/8566) | Open; only web, mobile, and Desktop remain product surfaces at exit |

Primary OpenCode evidence is current upstream source, especially
[`packages/desktop`](https://github.com/anomalyco/opencode/tree/1c7f65f1057b587adb131d6683e273fa55007f35/packages/desktop)
and
[`packages/app`](https://github.com/anomalyco/opencode/tree/1c7f65f1057b587adb131d6683e273fa55007f35/packages/app).
Official documentation confirms that OpenCode is distributed as a terminal,
desktop, and IDE product and that Desktop can connect to a WSL-hosted server:
[OpenCode introduction](https://opencode.ai/docs/) and
[Windows/WSL](https://opencode.ai/docs/windows-wsl/).

OpenAgents evidence is the checked-in code and tests, not issue prose or visual
similarity. The most relevant local sources are:

- [`apps/openagents-desktop`](../../apps/openagents-desktop/README.md)
- [`docs/terra/DESKTOP_PARITY.md`](../terra/DESKTOP_PARITY.md)
- [`docs/terra/CURRENT_STATE.md`](../terra/CURRENT_STATE.md)
- [`docs/khala-code/2026-07-05-opencode-desktop-parity-gap-audit.md`](../khala-code/2026-07-05-opencode-desktop-parity-gap-audit.md)
- [`docs/khala-code/khala-code-ux-contract.md`](../khala-code/khala-code-ux-contract.md)
- [`docs/sol/issues/app-desktop.md`](./issues/app-desktop.md)

## How parity is scored

This is a capability audit, not a pixel-matching exercise. A capability counts
only when it has all of the following at a scope appropriate to its status:

1. a bounded host or authoritative data service;
2. a typed renderer projection;
3. a reachable user action or visible state;
4. explicit unavailable, failure, and recovery behavior; and
5. proportional unit, integration, visual, or packaged-app proof.

Status meanings:

- **Landed** — a coherent, reachable, useful vertical slice exists. It may not
  contain every OpenCode option.
- **Partial** — real reachable behavior exists, but a major part of the
  benchmark capability is missing.
- **Scaffold** — types, adapters, projections, disabled settings, or dormant UI
  code exist, but there is no useful end-to-end user capability.
- **Absent** — no current destination implementation.

The counts are deliberately unweighted. “Cross-platform packaging” is much
larger than “native file picker,” so the matrix is a coverage map rather than a
percentage-complete claim.

## Capability matrix

| # | Capability area | OpenCode current benchmark | Khala Code desktop | OpenAgents Desktop | Destination gap |
| --- | --- | --- | --- | --- | --- |
| 1 | Secure host and application substrate | Electron main/preload/renderer split, app initialization, native integration | **Landed** — mature Electrobun host and extensive RPC/service boundary, though not the destination host | **Landed** — sandboxed Electron, context isolation, no Node integration/webview, restrictive CSP, deny-by-default navigation/permissions, Effect Native renderer | Preserve; add sender/frame-origin validation and packaged fuse proof |
| 2 | Packaging, signing, updates, rollback | Electron Builder packages for macOS/Windows/Linux, updater state and install flow | **Partial** — signed-release knowledge and in-app updater exist, but release entrypoints are now retired | **Absent** — Bun dev bundle only; no Forge/Builder package, identity, fuses, signing, notarization, update feed, clean-machine or rollback proof | Freeze identity, then build a new independent release lane |
| 3 | Lifecycle, windows, deep links, native menus | Single-instance/deep links, persisted multi-window registry and geometry, titlebar integration, menus, recovery | **Partial** — deep links, single-instance forwarding, expanded menu and window actions landed; no comparable persisted multi-window workbench | **Absent** — one fixed window; no app protocol, single-instance lock, route restore, native command menu, window registry, zoom state, or notifications | Add typed lifecycle service and menu/command bridge after route model exists |
| 4 | Native file/directory/clipboard bridges | Authorized file grants, directory/file/save pickers, clipboard image, open path/app, markdown bridge | **Landed** — grant/read/release, expiry, directory/save/clipboard image paths and preview denial are implemented | **Partial** — directory picker plus fixed root listing/read; no file grants, save, clipboard image, attachment budget, or safe open-path action | Extract capability-grant contract, not a generic filesystem bridge |
| 5 | Local coding server and server switching | Bundled authenticated local server, health gate, remote servers, default server, alternate-server recovery | **Scaffold** — server capability contract and Settings manager projection exist, but lifecycle/default/remote actions remain disabled | **Absent** — generic gateway fetch and Pylon subprocesses are not a coherent local coding server or selectable backend | Define one OpenAgents desktop runtime contract across Pylon, Codex bridge, and gateway |
| 6 | Windows WSL lifecycle | Discovery, distro installation, OpenCode install/update, server start/stop/default, WSL terminal and settings | **Absent** | **Absent** | Explicit P2/P3; do not block the first useful macOS/Windows/Linux workbench |
| 7 | Home, projects, workspaces, routes | Project home, recent/search/archive, project/workspace sidebar, multi-server routes, background open | **Partial** — Project Home, search, new/open/background behavior, status and route persistence landed; project/tab model is narrower | **Partial** — recent local chats, folder choice, root label; no project catalog, route grammar, search, archive, status, or background tabs | Establish project/session identity before adding more permanent navigation |
| 8 | Session tabs and management | Sortable tabs, closed-tab recovery, lineage/fork, share, archive, ownership, previous/next navigation | **Partial** — command-backed fork/archive/navigation and disabled safe share; no full tab/lineage/share system | **Absent** — five-item rail only; no rename/delete/archive/fork/share/tab recovery/lineage | Build session route + tab state over authoritative thread records |
| 9 | Chat, streaming timeline, tool/work states | Virtualized streamed timeline, reconciliation, tools, permissions, questions, todos, errors, context usage | **Landed** — Codex/Claude/Grok runtimes, streamed items/tool calls, approvals, state indicators, usage and safety projections | **Partial** — local user/assistant/system rows and honest error row; non-streaming, no tool events, interrupt, approvals, plan/todo/question, virtualization, or usage | This is the first product-critical gap: replace request/response chat with a typed event stream |
| 10 | Composer and context | Rich editor, normal/shell modes, history, model/agent/variant selection, attachments, drag/paste, files/lines/comments, work docks | **Landed** — contenteditable foundation, modes/history, selectors, attachments/grants, selected context and docks landed | **Partial** — one plain text field, submit/clear/pending/focus; no rich editing, history, modes, attachments, selectors, context or docks | Port behavior via shared Effect Native components and typed request parts |
| 11 | File explorer and editor | Lazy tree, watcher/cache, diff markers, file tabs, selection/scroll persistence, search and source views | **Partial** — provider-neutral service, lazy tree, Monaco read-only source, tabs/search/context; editing/save/watch/rename/delete remain open | **Partial** — non-recursive root-only list, directories disabled, 240 KB text preview, no editor/tabs/search/watcher/cache/diff markers | Build bounded tree/read/edit/save/watch services before adding Monaco/foreign host |
| 12 | Git review, diffs, comments | File/review/context side panels, per-session review state, diff kinds, comments, revert and scaling tests | **Partial** — diff review panel, comments, context steering and source-control actions exist; integration is narrower | **Absent** — `review` is only a latent workspace name and currently renders the generic home fallback | Add typed git status/diff first, then review/comments/revert |
| 13 | Interactive terminal workbench | Workspace PTYs, terminal tabs, Ghostty web, restore/reconnect, copy/paste/links, resizing, shell/font settings | **Partial** — reachable Codex background-terminal inspection/lifecycle; intentionally no interactive new PTY | **Absent** — `terminal` is a latent workspace name with no route or host service | Add a bounded PTY session service; never expose arbitrary renderer command authority |
| 14 | Commands, palette, keybindings | Central command registry, searchable palette, menu integration, categories, editable conflict-safe bindings | **Landed** — registry, grouped palette and persisted keybinding editor with conflict handling | **Absent** — direct intents only; no registry, palette, menu binding or user keybindings | Land before workbench breadth creates permanent navigation clutter |
| 15 | Providers, models, MCP, permissions | Connect/custom/disconnect providers, model visibility/search, MCP states/actions, permission auto-response | **Partial** — rich projections and typed intents exist; several mutations/auth flows remain runtime-owned or disabled | **Scaffold** — Codex account refs/readiness and device-auth only; no model/provider/MCP/permission UI | Separate account custody from model/runtime selection; keep secrets host-side |
| 16 | Preferences, themes, locales, notifications, sounds | General/shortcut/server/provider/model settings, themes/fonts/shell, 18 locales, notifications/sounds, display settings | **Partial** — theme/font/notification/sound/layout preferences landed; native delivery, i18n and platform settings remain | **Scaffold** — static theme plus a Codex accounts screen; no preference model, locale, notification, sound, shell/font or layout settings | Add settings registry after command and runtime registries exist |
| 17 | Diagnostics, recovery, release notes, support | Crash/net logs, unresponsive/load recovery, public export, updater actions, docs/support/feedback | **Landed** — public-safe debug zip, watchdog/recovery overlay, support policy/menu/settings and update states | **Absent** — no diagnostic store/export, watchdog, crash/load recovery, support, release notes or update states | Extract redaction and recovery contracts early; connect packaging later |
| 18 | Fleet, Inbox, Gym, Forum and proof | Not an OpenCode benchmark; OpenAgents product differentiation | **Landed** — deep Fleet/Pylon supervision, account/capacity, approvals, Inbox, Gym evidence, Forum and proof surfaces | **Scaffold** — Fleet host contract/brief adapter exists but is not rendered; Inbox/Gym/Forum are absent; `inbox` is a latent name | Port only over authoritative FleetRun/approval/receipt projections after the core coding loop |
| 19 | Cross-device continuity and Blueprint authority | OpenCode server/session sync model; remote server is selectable | **Partial** — Khala Sync chat/fleet and desktop/mobile handshake exist but important paths remain flag-gated/default-off | **Absent** — local JSON threads and local folder state only; no Sarah/mobile/Sync continuation or authoritative run projection | Replace local-only identity with shared thread/run/work-unit contracts |
| 20 | Verification, performance and release evidence | Broad unit/browser/E2E/performance coverage including timeline and stacked review/terminal regressions | **Landed** — 992/993 tests passed in this audit, rich property/DOM/visual/live lanes; current suite still has one failure and no successor release proof | **Partial** — 60/60 unit tests, bundle and real Electron smoke pass; current typecheck is red, smoke is fixture/error-path biased, and there is no packaged E2E | Repair current-head health, add clean-state/live-runtime and packaged-app oracles |

Coverage summary:

| App | Landed | Partial | Scaffold | Absent |
| --- | ---: | ---: | ---: | ---: |
| Khala Code desktop | 8 | 10 | 1 | 1 |
| OpenAgents Desktop | 1 | 6 | 3 | 10 |

This is why “we already built much of OpenCode parity” and “OpenAgents Desktop
is still early” are both true. Most harvesting happened in the frozen Khala
Code package; very little of it has crossed the greenfield boundary.

## What changed in OpenCode after the July 5 Khala audit

The existing Khala audit was directionally strong, but OpenCode did not stand
still. The current upstream ref is 109 commits ahead of the local July 5
worktree. Across `packages/app` and `packages/desktop`, that delta touches 135
files and adds roughly 9,200 lines while deleting roughly 2,100.

Important benchmark movement includes:

- first-launch desktop onboarding;
- persisted session-tab information and tab-routed new sessions;
- inline file-browser tabs and continuing file-tree v2 work;
- per-session review-state persistence, bounded review patch loading, and
  review/terminal stacked regression tests;
- a command-palette v2 and first-open behavior fixes;
- draft-preserving composer add commands;
- extracted prompt state and composer model selection;
- expanded product translation parity;
- session/review width and scaling benchmarks; and
- continued provider connection/model selection polish.

Therefore the July 5 Khala implementation wave closed many named gaps but did
not freeze parity. Tab fidelity, review persistence/performance, composer state,
onboarding, and release-quality work have continued to move upstream.

## Khala Code: what is genuinely worth extracting

The frozen package is unusually rich as a behavior library. Its 350 tracked
files and approximately 118,000 TypeScript/TSX/CSS lines are not a reason to
port it wholesale; they show why a filename inventory can overstate the
greenfield app's current capability.

### Proven interaction and projection work

- Rich composer: `rich-composer.ts`, attachment resolution, prompt modes and
  history, model/provider/agent/reasoning selection, slash commands, selected
  file/line/comment context, and follow-up/permission/question/revert/todo
  docks.
- Command infrastructure: typed command registry, grouped palette, editable
  keybindings, command-driven hotbar/native menu actions, thread navigation and
  session actions.
- Coding workbench: Project Home, provider-neutral file service, file tree,
  Monaco read-only editor, file tabs/search, diff review, comments, source
  control actions, and bounded background-terminal inspection.
- Account/runtime surfaces: Codex settings and app-server bridge, Claude and
  Grok runtime lanes, provider/model/MCP/permission projections, rate limits,
  local-server capability contract and manager projection.
- Product differentiation: Fleet board/status/worker cards, approvals, account
  capacity, Inbox, Gym proof, Forum and Khala Sync projections.
- Desktop reliability: updater, diagnostics redaction/zip, unresponsive and
  load-failure recovery, deep links, single-instance coordination, support
  entrypoints and native menus.

### Limits that must survive the extraction narrative

- The editor remains primarily read-only; complete edit/save/dirty/watcher/
  rename/delete behavior did not land.
- The terminal is a Codex background-terminal inspector, not an interactive
  PTY workbench.
- The local server is a capability contract and manager projection, not a live
  OpenCode-equivalent server with remote switching.
- Provider, model visibility, MCP and permission surfaces contain disabled or
  intent-only actions where no safe authority API exists.
- Khala Sync chat and Fleet sources are still feature-flagged in the README and
  are not proof of default cross-device authority.
- There is no Windows WSL lifecycle.
- Release commands deliberately stop through the retired-release guard.
- Many parity slices were implemented and fixture-tested on 2026-07-05. They
  are strong extraction inputs, not independent clean-machine product proof.

## OpenAgents Desktop: current implementation truth

Path authority is unambiguous:

- `apps/openagents-desktop` is the active greenfield destination.
- `clients/khala-code-desktop` is the frozen extraction/parity source.
- `clients/openagents-desktop` has no tracked implementation. Commit
  `9046a507ae` removed the retired stub; an ignored local directory may remain
  only because of installed `node_modules` residue.
- `apps/autopilot-desktop` is an older withdrawn product surface, not the
  OpenAgents Desktop implementation home or an alternate parity authority.

### Landed foundation

The greenfield boundary is sound for its current narrow scope:

- Electron 43 host with `contextIsolation: true`, `nodeIntegration: false`,
  `sandbox: true`, `webviewTag: false`, and `webSecurity: true`;
- deny-by-default runtime permissions, navigation, new windows and webviews;
- restrictive renderer CSP;
- fixed preload methods instead of raw `ipcRenderer`, MessagePort, Node or
  Electron capability;
- Effect Schema decoding on the important bridge boundaries;
- 100% Effect Native renderer application/component/intent code; and
- no imports from the deprecated Khala Code package.

That substrate should be preserved. OpenCode's user affordances are a reference;
its broad renderer-facing Electron API is not the destination security model.

### Reachable product behavior

Conversation:

- at most five JSON-backed local threads;
- at most 80 stored messages per thread;
- create, open, select and submit;
- first user message becomes a 48-character title;
- one non-streaming OpenAI-compatible gateway request using a host environment
  token; and
- typed user/assistant/system transcript rows with submit/clear/pending focus
  behavior.

Workspace:

- one process-local selected root, defaulting to the launch working directory;
- native folder selection;
- only the selected root's first 120 non-dot entries;
- directories displayed but disabled;
- a coarse clean/changed/unavailable Git status;
- traversal-checked read of a selected file; and
- a 240 KB UTF-8 preview cap.

Accounts:

- Codex account refs and readiness projected from `pylon codex accounts list`;
- isolated `pylon auth codex` device flow;
- verification URL and code projection with the URL retained as host-owned
  open-external state; and
- typed loading, unavailable, awaiting-browser, connected and failed states.

### Important non-capabilities and dormant code

- `review`, `terminal`, and `inbox` appear in the workspace-name union but have
  no distinct reachable UI. The current view function routes these names to
  Project Home, and the sidebar exposes only Chat, Files and Home.
- The Fleet objective/dispatch state, renderer, preload method and Pylon
  adapter still exist, but the current minimal shell does not render a Fleet
  entrypoint. Historical Fleet screenshots are not current reachability proof.
- The old loop-proof intent and smoke helpers remain as source residue without a
  current user control.
- The chat call is not Sarah conversation authority, does not stream, and does
  not use the Codex account that Settings connects. Gateway authorization is a
  separate environment-only token path.
- Codex connect locates and spawns `apps/pylon/src/index.ts` from a monorepo
  source checkout and parses CLI stdout. It is not a packaged sidecar/service
  contract yet.
- The file payload includes absolute root/file paths in renderer state. The
  service is bounded, but a future public/evidence projection must not reuse
  that payload unredacted.
- There is no thread schema migration/validation, cross-device sync, streaming
  resume, cancellation, tool/approval state, project identity, terminal, diff,
  editor save, command palette, deep link, updater, diagnostic export, crash
  recovery, packaging, signing or release channel.

### Documentation and proof drift found by this audit

- [`apps/openagents-desktop/README.md`](../../apps/openagents-desktop/README.md)
  still says the titlebar exposes **Open Fleet** and describes its deployment
  brief as visible. The current renderer deliberately removed that control.
- The README's `smokeOpenFleetDesk` helper remains in `main.ts` but is not called
  by the current smoke.
- Existing status docs record 58 passing tests; the current package has 60.
- Existing status docs report typecheck green through the Settings slice. At
  this snapshot it is red because the shared Effect Native DOM icon lowering
  accepts the closed Effect Native icon union while the shared OpenAI icon
  catalog does not accept `Compose`.
- The smoke accepts either an assistant or system response. In this audit it
  passed with `responseSenderChip: "SYSTEM"` because no live gateway token was
  configured. That proves the error row and submit lifecycle, not a live model
  completion.
- Smoke state is not isolated from the development userData directory; this run
  began with ten existing transcript rows. It is a real Electron smoke, but not
  a clean-first-run proof.
- Settings smoke is explicitly a scripted device-auth fixture. No owner browser
  completion or live account-ready transition was proven here.

## Current verification result

Commands were run from the package directories at the snapshot above.

| Surface | Command | Result |
| --- | --- | --- |
| OpenAgents Desktop | `bun test` | **60 pass, 0 fail** |
| OpenAgents Desktop | `bun run build` | **pass** when run alone |
| OpenAgents Desktop | `bun run smoke` | **pass**; real Electron, system/error chat response, scripted Codex device-auth fixture |
| OpenAgents Desktop | `bun run typecheck` | **fail** in shared Effect Native DOM icon catalog (`Compose`) |
| Khala Code desktop | `bun test tests/*.test.ts` | **992 pass, 1 fail**; Fleet supervisor cleanup test failed |
| Khala Code desktop | `bun run typecheck` | **fail**; shared icon mismatch plus current Pylon exact-optional-property errors appear in the package graph |

The result is not “everything is broken.” It means the historical slices are
still well covered, while current `main` is not presently a fully green desktop
integration baseline. Fixing that is the first implementation gate.

## Capability disposition for the greenfield app

Issue #8574 requires every Khala Code idea to have an explicit home. The
following disposition avoids both idea loss and a second orchestration plane.

| Capability family | Destination disposition | Boundary |
| --- | --- | --- |
| Conversations, recents, composer, request states | Fold into the everyday OpenAgents Desktop shell | Shared Sarah/thread authority; no separate local truth after migration |
| Project/session home, tabs, commands, keybindings | Retain as core desktop infrastructure | App-owned navigation over authoritative project/session IDs |
| File tree, editor, diff/review, terminal | Retain as the coding specialist workspace | Typed host services/foreign-host nodes; no raw filesystem or shell bridge |
| Models, providers, accounts, MCP, permissions | Retain in Settings and composer controls | Host/runtime owns secrets and mutations; renderer receives sanitized status/intents |
| Fleet board, worker cards, approvals, assignments, capacity | Retain in the Fleet specialist cockpit | Pylon/Sync/Blueprint owns run state and receipts |
| Inbox and operator attention | Fold into a shared attention projection | No approval inferred from chat text or local UI state |
| Gym and proof/evidence views | Retain as read-only specialist evidence | Receipt source owns proof; raw private events stay private |
| Forum | Prefer current Forum API/deep link; embed only with a typed product need | Do not recreate an unrelated web app inside Desktop |
| Codex/Claude/Grok runtime adapters | Extract as engine services | Desktop consumes stable events/actions; no provider-specific shell architecture |
| Khala Sync and mobile handshake | Extract as continuity services | One user/thread/run reality across web, mobile and Desktop |
| Updater, diagnostics, redaction, recovery, support | Rebuild as Electron host services using legacy contracts as test vectors | New identity/feed/signing lane; no Electrobun release reuse |

## Recommended implementation sequence

### Gate 0 — make the baseline truthful and green

1. Fix the shared `Compose` icon contract mismatch and the current desktop
   package typecheck.
2. Remove or finish dormant workspace names, Fleet/loop helpers and stale README
   claims. A capability should be reachable, explicitly unavailable, or absent.
3. Isolate smoke userData, require a clean-first-run path, and distinguish
   `live_completion`, `expected_unconfigured`, and `fixture_auth` receipts.
4. Add a small generated parity manifest so docs, route registry, host methods
   and test coverage cannot silently disagree.

Exit: typecheck, 60+ package tests, build and isolated Electron smoke are green;
no current document claims an unreachable surface.

### Slice 1 — authoritative streamed conversation

Build the event and authority seam before rich UI:

- shared thread/session identity with Sarah/Sync rather than five local-only
  records;
- typed streaming text, reasoning, tool call/result, plan/todo, permission,
  question, approval, error, usage, completion and interruption events;
- resume/reconnect/cancel behavior;
- host-held account/runtime selection; and
- transcript projection with large-session virtualization and stable anchors.

Then move the Khala composer behavior onto Effect Native: multiline rich input,
history, normal/shell mode, attachments, model/agent/variant selection,
selected context and work docks.

Exit: one real authenticated turn streams, can be interrupted/resumed, projects
tool/request state, survives restart, and is visible on another authorized
OpenAgents surface without inventing local authority.

### Slice 2 — project/session navigation and command infrastructure

- project/session route grammar;
- project home with search, archive and honest status;
- sortable/recoverable session tabs;
- typed command registry;
- command palette and conflict-safe editable keybindings; and
- native menu/deep-link/single-instance wiring through the same command IDs.

Exit: every global/session/workbench action routes through the command registry
or has an explicit documented exception.

### Slice 3 — bounded coding workbench

Build one coherent vertical slice instead of independent placeholder panes:

1. recursive lazy file tree, capability grants, watcher/cache and search;
2. read/edit/save/dirty/reload behavior with atomic writes;
3. open file tabs, selected line/range state and composer context;
4. typed Git status/diff projection;
5. review/comments/revert state; and
6. interactive workspace-bounded PTY tabs with reconnect and teardown.

Monaco and terminal rendering should be Effect Native foreign-host nodes. The
renderer receives typed document/terminal sessions, never general filesystem or
arbitrary process authority.

Exit: select a project, edit and save a file, review the resulting diff, add a
line/diff comment to the prompt, run a bounded terminal session, and resume the
whole workspace after restart.

### Slice 4 — accounts, providers, models, MCP, permissions and settings

- separate OpenAgents sign-in, provider account custody, runtime/model choice
  and Fleet worker accounts;
- provider/model catalog and availability;
- model visibility/favorites and context/usage information;
- MCP status, authentication and enable/disable through server-owned APIs;
- enforced permission policy and auto-accept boundaries;
- theme/font/shell/layout preferences; and
- notification, sound, accessibility and locale infrastructure.

Exit: changing a setting affects the real runtime, unavailable mutations are
disabled with reasons, and no credential enters renderer logs, support bundles
or public evidence.

### Slice 5 — authoritative Fleet and continuity cockpit

Only after the core conversation/workbench event model is stable:

- active FleetRun projection and exact run/worker/account refs;
- pause/resume/stop/desired-slot and approval intents through current Pylon/
  Sync authority;
- Inbox attention, assignment and closeout states;
- Gym/proof/receipt read-only views; and
- cross-device continuation among Sarah web, mobile and Desktop.

Exit: a Sarah-started FleetRun opens with matching state and controls in
Desktop, and a Desktop-started run is summarized by Sarah from the same
authority records. This is the core #8574 product exit, not a local brief
accepted by Pylon.

### Slice 6 — desktop productization

- freeze bundle/app IDs, executable names, userData/session partition, deep-link
  scheme, OAuth redirects, update product/feed/channel, tag namespace and
  rollback policy;
- Electron packaging with verified fuses;
- signing/notarization for supported platforms;
- updater and release notes;
- diagnostic redaction/export, crash/load/unresponsive recovery and support;
- clean-machine install/first-run/update/rollback tests; and
- remove legacy Khala Code release/install paths only after successor proof.

Exit: the greenfield app is installable, updateable, recoverable and
independently identified; the frozen Electrobun app cannot release.

### Later — multi-window and WSL

OpenCode proves these are valuable for a mature desktop product, but they need
not block the first complete coding/Fleet cockpit. Keep their contracts visible:

- persisted multi-window routes and geometry;
- new/restore/focus window behavior;
- remote/default server management; and
- Windows WSL discovery, installation, runtime update, server lifecycle and
  path translation.

## Design and architecture rules for parity work

- Copy behavior and interaction models, not OpenCode or Khala UI code.
- No production import from `clients/khala-code-desktop`.
- New shared component needs go through Effect Native; do not build a parallel
  React/shadcn/Tailwind product tree.
- Every renderer capability is a named, schema-validated host service. No raw
  `ipcRenderer`, filesystem, terminal, process, token or arbitrary URL bridge.
- Do not make an OpenCode-style local server a second source of truth. It is a
  desktop runtime facade over current OpenAgents/Pylon/Sync authority.
- Do not infer run, approval, receipt, provider or update state from local UI.
- Keep the default surface conversation-first. Coding and Fleet depth should be
  fast to open through commands/tabs, not permanent developer/status chrome.
- Do not ship dead controls or optimistic fake data. Unconnected surfaces are
  absent or visibly unavailable.
- Every parity claim needs one normal path, one degraded/failure path and one
  restart/reconnect path in tests.

## Final assessment

Khala Code reached broad, meaningful OpenCode-inspired surface coverage, but
mostly as a one-day first-pass implementation wave inside a now-frozen host. It
is the best source for OpenAgents-specific behavior, invariants and test
fixtures. OpenCode remains the stronger reference for the integrated everyday
coding workbench, desktop lifecycle, tabs, terminal, server, cross-platform and
performance details.

OpenAgents Desktop has chosen the right destination architecture and security
boundary, but only the beginning of the product has crossed into it. The gap is
not primarily visual polish. It is typed streamed runtime state, authoritative
continuity, project/session navigation, the complete file/review/terminal loop,
command/settings infrastructure, and release engineering.

The shortest honest path is:

`green baseline -> authoritative stream -> commands/routes -> file/review/PTY -> settings/runtime -> Fleet/Sync -> signed release`

That sequence adds the OpenCode qualities users feel every minute while
preserving the OpenAgents qualities OpenCode does not provide: one relationship,
one Fleet authority, one evidence model, and cross-device continuity.
