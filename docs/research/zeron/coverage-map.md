# Coverage map and remaining evidence

This map checks that the gap register follows the current Zeron implementation,
not only the original rewrite checklist. Paths in the first column refer to the
pinned Zeron checkout; OpenAgents reuse candidates are linked through
[scope and evidence](scope-and-evidence.md). A grouped row is a workflow family,
not a claim that every helper or test file received a line-by-line audit.

## Current surface coverage

| Zeron source family | Compared behavior | Register |
| --- | --- | --- |
| `ui::shell`, `surface_chrome`, `shell::panel_layout`, `shell::tabs`, `navigation_focus` | Routes, pane fit/takeover, focus-aware cycling, titlebar and surface lifecycle | NAV-01/02/06; OPS-03/06; CUS-13–16 |
| `shell::spaces`, `project_icon`, `sidebar_sections`, `sidebar_pins`, `proto::sidebar_pins` | Project/device filtering, custom sections, pin ordering, scoped preferences | NAV-03–05/08/13; REM-01/02 |
| `shell::thread_search`, `command_palette`, `search_warm`, `engine::workspace_search`, `home_index` | Chat/action/file/content search, folder discovery, warm indexes | NAV-09; WRK-04/15; PERF-04 |
| `composer`, `composer_markdown`, `composer::chip`, `pickers`, `pickers::compact`, `settings::composer`, `settings::completion` | Provider/model/reasoning/options, input editing, semantic references, commands/skills, retained defaults | COM-01–09/15/16; RUN-04; CUS-07/08 |
| `queue`, queue-related RPC methods and document/engine state | Move/edit/send/steer, edit leases, attachments, adoption and recovery | COM-10–15; MED-01; REM-04/12 |
| `transcript`, `rail`, `todo_panel`, `context_usage`, `running_pill`, `loaders`, `roll_text` | Streaming/grouping, prompt jumps, plans, occupancy, working state | TRN-01–06/12/14; CUS-14/16; PERF-01/06 |
| `markdown::{render,selection,links,link_*,inline_code_links,mermaid,mermaid_cache,veil}`, `syntax_cache` | Rich text, selection, links, diagram rendering, bounded background work | TRN-03/07–09/14; PERF-06 |
| `files::{tree,model,client,search,git_status,context_menu,sections,watch}` | File explorer, selection, search, state refresh and owner routing | WRK-01–04/09/20; REM-03 |
| `files::{document,editor,editor_adapter,preview,mutations,rename,drag}`, `shell::file_mutations` | Document editing, concurrency, autosave, structure changes, gestures | WRK-05–12; GIT-04; OPS-12 |
| `files::{markdown_preview,markdown_media,image_preview}`, `image_media`, `image_viewer`, `workspace_links` | File previews, media, file links, zoom/pan and cache release | WRK-13/14; TRN-07/09; MED-02/04 |
| `changes`, `comments`, `comment_ui`, `history`, `engine::{source_control,source_control_review_threads,diff_sync}` | Scoped diffs, status, note anchors, history, ref/discard operations | GIT-01–09/15/16 |
| `pull_requests`, `pull_request_detail`, `pull_request_code`, `pull_request_interactions`, `pull_request_media`, `pull_request_handoff`, `pull_request_skeleton`, `change_requests` | PR board, detail/code/threads/checks, comments, media, handoff | GIT-10–16; NAV-08 |
| `project_actions`, `shell::actions_ui`, `engine::{repos,project_actions,worktree_settings,spaces}`, `settings::worktrees` | Folder/repository/worktree choices, actions, imports and setup | WRK-15–19; COM-03; TERM-08 |
| `terminal::{emulator,view,panel,dock}`, `composer_dock`, `engine::terminals` | PTY tabs, docking, replay, reconnect, keyboard/selection behavior | TERM-01–08; NAV-02/06; PERF-05 |
| `shell::side_chats`, `files::{subagents,sections}`, child-session document/harness handling | User and agent child conversations, completed/failed lists, transcript tabs | NAV-11/12; TRN-06; REM-11 |
| `harness::{claude,codex,cursor,acp,pi,opencode}`, model catalogs, skills | Nine providers, capability discovery, normalization, resume, children, commands | RUN-01/04–07/14–16; COM-01/02/06/07/09/10 |
| `harness::{install,adapter_install,archive_install,executable,shell_env,process,windows_process,code_signature}`, `engine::harness_updates` | Readiness, managed installation, source detection, update gates, subprocess cleanup | RUN-02/03/15/16; OPS-07/12 |
| `settings::{harnesses,accounts}`, `account_usage`, `engine::agent_accounts` | Enablement, provider logins, active accounts, quotas, reset times | RUN-02/08–10; CUS-09/12 |
| `state`, `client`, `engine::{auth,profile,local_import,registry,workspace_host,chat2_host,doc_host,doc_host_delivery,chat_persistence}` | Local/synced identity, runtime lifecycle, registry/doc state, adopted input | RUN-11/12/15; REM-01–12; OPS-01–03/08 |
| `rpc`, `proto::{entities,workspace,view}`, `sync`, `doc`, relay/edge services | Typed capability contracts, durable queues/publication, account/device scope | REM-03–12; COM-11/12/15; OPS-12 |
| `attachments`, `tool_images`, `appshots`, platform capture adapters, `settings::appshots`, uploads | Input/output images, original bytes, Appshots context, queue thumbnails | MED-01–06; TRN-10; COM-11 |
| `browser::{model,profile,view,macos,linux,windows_webview}`, link routing | Embedded/local profiles, navigation, Linux helper, external-platform fallback | WEB-01–04/08; TRN-08; OPS-06/07 |
| `preview`, `proto::preview`, `WatchPreviews` and browser empty-page integration | Process/service discovery, aliases, HTTP/WebSocket proxy, remote peer streams | WEB-05–08; REM-03/10; PERF-05 |
| `voice`, `dictation`, `settings::Voice` route, voice/model/audio crates | Local optional speech-to-text, hold/cancel/finalization, runtime/microphone management | VOI-01–03/07; OPS-07/11/12 |
| `ui::voice`, `shell::voice_stage`, `voice-session`, `voice-media`, realtime harness/engine, `orb`, `veil` | Realtime orchestrator, client audio/host Codex, ownership, call chrome/captions/activity | VOI-04–07; RUN-14; REM-10 |
| `visual`, `visual-gpu`, `ui::visuals`, `mcp` | Interactive native artifacts, control events, charts/canvas/scenes/shaders, agent workspace tools | VIS-01–03; TRN-11; RUN-14 |
| `appearance`, `theme`, `theme_library`, `typography`, `frost`, `glass`, `new_thread_background_*`, `settings::{appearance,wallpaper,wallpaper_colors}` | Independent variants/material/accent, imports, font families/sizes, wallpaper/effects | CUS-01–07/10/11/13 |
| `settings::{shortcuts,files,notifications,thread_naming,archived,devices,widgets}`, `toast`, `notice` | Keymaps, autosave, alerts, naming, archive restore, device management, error presentation | CUS-08–12; RUN-13; NAV-10/14; OPS-09 |
| `motion`, `overscroll`, `popover`, `edge_fade`, `rounded_clip`, `haptics`, `sound`, `notify`, `badges` | Input feel, menu anchoring, transitions, reduced motion, event/focus policy | CUS-09/10/13–16; OPS-06/10; PERF-02/06 |
| `app_menus`, `app_update`, `update`, `apps/zeron::{main,daemon,paths,auth_cli,update_cli}`, `dist`, package/native-check scripts | Boot/reopen/quit, account phases, single instance, app/provider install/update, OS packaging | OPS-01–08/11/12; RUN-02/03/11/12 |
| Feature regressions and performance documents/fixtures | Selection, sidebar pin integrity, focus cycling, revisit, memory, slow links and input responsiveness | All V rows; PERF-01–06; REM-09/12; VOI-07 |

The `windows_webview.rs` filename does not establish a shipped embedded Windows
browser. Current browser documentation explicitly routes Windows to the external
browser. Likewise, serialized legacy tab preferences are not proof that a main
session tab strip is still visible.

## RPC capability coverage

The current Zeron RPC catalog is a useful completeness check, not a proposed
OpenAgents wire format. Each group below has a corresponding register home.

| RPC group | Representative operations | Register |
| --- | --- | --- |
| Engine/profile/auth | EngineInfo/Ready, LocalDevice, StopEngine, auth/org selection, local workspace import | RUN-11/12/15; OPS-01–03 |
| Provider configuration | ListHarnesses/Models/Skills/Commands, Install/CancelInstall, enablement, title settings, harness update watch/apply/policy | RUN-01–04/13; COM-02/06/07 |
| Durable interaction | QueueCommand, RelayCommand, RetryDelivery, queue watch/move/remove/send/steer/edit lease | COM-10–15; REM-04–06 |
| Workspace discovery | WatchDevices/Chats/Sessions/Spaces/SidebarPreferences, Mutate, FocusChat, ProbeSync | NAV-03–05/08–11; REM-01/02/05/08 |
| Workspace I/O | Directory/file/image reads, file/content search/warm, write/move/delete/watch | WRK-01–14/20 |
| Repositories/actions | Add/clone/create/list repos, folders/drives, branches/refs, worktrees/settings, project action CRUD/run/setup handoff | WRK-15–19; TERM-08 |
| Source control | Status/diff watches, scoped/file diffs, history/search/avatars, fetch/switch/discard | GIT-01–09 |
| Pull requests | Checkout request watch, list/detail/repository/diff, explicit comment submission | GIT-10–16 |
| Terminals | Open/subscribe/write/resize/close | TERM-01–08 |
| Provider accounts | List/activate/forget and start/complete/poll/cancel login | RUN-08–10 |
| Attachments/artifacts | Chunk upload/commit/read, PublishVisual, FetchToolBlob, transfer watch | MED-01–04; VIS-01–03; REM-05 |
| Connection and previews | WatchConnectivity, SyncStatus/ProbeSync, WatchPreviews | REM-05/09/12; WEB-05–08 |
| Voice | Capability/prepare/own/negotiate/confirm/report/stop/cancel V2 controls | VOI-04–07 |
| App updates | UpdateStatus, ApplyUpdate | OPS-04/05 |

## Historical documentation corrections

1. OpenAgents' desktop README still describes image attachments as disabled
   after 2026-10-01. Current `coder_tab::ATTACHMENTS_ENABLED` is `true`, and
   desktop `Panel` documents re-enablement on 2026-10-09. This analysis counts
   actual image input and records its remaining routing/format limits.
2. Older OpenAgents desktop orientation describes six settings pages; the
   current `Pane::ALL` has nine. Model providers and Background are existing
   capabilities, not parity gaps to rebuild.
3. OpenAgents Verse, Map, hosted Gym runs, and feedback have retained code and
   verification artifacts, but default desktop builds hide their entry points
   unless compiled with `OPENAGENTS_DESKTOP_PREVIEW=on`. They cannot be counted
   as ordinary release-visible substitutes for Zeron workspace features.
4. OpenAgents shared review comments, subagents, and plan models are evidence
   of reusable work. Desktop integration differs: local Coder uses plan
   presentation, the terminal board lists background agents, and the desktop
   action set does not provide the full file/diff note and child-transcript UI.
5. Zeron's original feature inventory and `PARITY.md` predate many completed
   features. Current code has more providers, browser profiles, files/editing,
   source-control/PR surfaces, capture, voice, themes, and engine locks than
   those documents imply.
6. Zeron's former top-level session tab strip is explicitly removed in current
   `shell/tabs.rs`; right-panel resource tabs remain. Accounts is now folded
   into Providers with a retained old-route alias.
7. “WebRTC is gone” in the old Zeron architecture does not describe its current
   project-preview transport. Preview traffic now has a separate peer-stream
   design and documented connectivity limits.
8. Zeron's historical token-display exclusion does not remove its current
   context-occupancy ring. Context occupancy, billing usage, and account quota
   are three different measurements. OpenAgents' fleet token/cost display is
   additional functionality, not an occupancy-ring substitute.
9. Old Zeron platform plans do not establish current package/native quality.
   Feature-specific documents preserve known limits, including Windows browser
   and capture behavior and unperformed remote-voice live checks.

This report does not edit those historical documents, which may record the
correct state at their original date. Follow the current source baseline and
the corrections above when implementing parity.

## Explicitly retained OpenAgents differences

These existing capabilities should survive parity work:

- Hosted general OpenAgents chat and routing into Coder.
- Coder's provider failover/capacity controls, worktree isolation, ATIF history,
  exact-reviewed publishing, and native task questions/approvals.
- Paired-device grants, encrypted host/history connections, host installation,
  and durable task input.
- BYOK model keys and background-rule controls.
- AccessKit integration, Windows toasts, the background-agent board, and the
  existing release/update trust model.
- OpenAgents-specific decks and preview-gated Verse/Map/Gym surfaces.

They are not approved reasons to omit Zeron functionality. Conversely, full
parity does not require importing Zeron's service backend, authentication
vendor, synchronization format, scripting language, or CI workflows.

## Remaining investigation before implementation claims

The register makes these unknowns actionable rather than marking them absent:

- Run current packaged applications with matched synthetic data to compare
  native rendering, shortcuts, hover/drag/scroll feel, and focus behavior.
- Check provider protocols, model catalogs, account mechanisms, and native
  resume against supported pinned executable versions.
- Exercise remote files/terminals/previews with differently shaped owner/viewer
  paths and real revocation/offline transitions.
- Record microphone, capture-permission, browser-profile, native view-overlay,
  screen-reader, and installer behavior on the claimed operating systems.
- Benchmark equal workloads on equal hardware; existing reports do not supply
  a current head-to-head comparison.
- Decide explicit scope exceptions, if any, for provider coverage, local-only
  coding entry, theme imports/materials, wallpaper, voice, visuals, and portable
  updates. None are assumed by this analysis.

The source gap map is complete at the workflow-family level for the inspected
baseline. A later implementation audit must still check the concrete
provider/platform combinations and record defects as they are discovered.

## Report verification

On 2026-10-15, documentation checks verified all nine report files, 174 unique
register IDs, explicit ID references, relative Markdown link targets, and linked
Zeron paths at the pinned Git revision. The status totals match the index:
68 missing, 80 partial, 24 verification gaps, and 2 product differences. The
OpenAgents desktop/shared-chat sources inspected for the report did not change
between the recorded baseline and the final integration check.

Whitespace and Markdown table structure were checked before the documentation
commit. No Rust behavior changed, so Rust tests and formatting gates were not
required. Product runtime, native-platform, live-provider, performance, and
production deployment checks remain outside this report's verification.
