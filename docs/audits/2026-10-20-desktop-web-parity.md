# Bring the whole desktop app to the web

Recorded 2026-10-20, at `d3b447cad5`. Owner request: port the full
functionality of the OpenAgents desktop app (`apps/openagents`) into
openagents.com, drawn with GPUI on the web.

The [GPUI web chat plan](2026-10-19-gpui-web-chat.md) and issues #12216 to
#12226 cover the chat surfaces. This document inventories everything else
the desktop app does, screen by screen, from the code. It records where each
feature lives, what the web has today, how the feature can work in a
browser, what backend it needs, and its size. It then groups the gap into
phases 7 to 13, after that plan's phases 0a to 6, with one issue for each
feature group.

The [unified openagents.com plan](2026-10-20-openagents-com-unified-plan.md)
owns the public and social layer: profiles (U1, #12254), agent identities
(U2, #12255), follow, fork, and share (U3, #12256), XP and receipts (U4,
#12257), the browser wallet (U5, #12258), `/explore` (U6, #12259), the front
door (U7, #12260), and `/verse` (U8, #12261). This plan doesn't duplicate
those epics; its issues link to the one they touch.

Paths that start with `ui/`, `engine/`, `client/`, `account/`, `rpc/`,
`sync/`, `harness/`, `mcp/`, `preview/`, `visual/`, `voice/`, or `mobile/`
are under `apps/openagents/crates/`. Paths that start with `web:` are under
`crates/openagents-web/src/`.

## Summary

- **The browser is a client, like the phone.** The desktop UI talks only to
  its own engine, in process or over `ws://127.0.0.1` (`ui/src/state.rs:281`,
  `:361`, `:479`). The browser has no engine. It has to work the way the
  phone does: through `openagents-client`, which reads the registry room and
  chat rooms on `coder-sync` and sends host RPC to a computer through the
  device room (`client/src/client.rs:461`, `client/src/live/urls.rs:31-133`).
- **Most local features already have a remote path.** The engine forwards
  any call whose parameters carry `targetDeviceId` to that device over the
  device room (`engine/src/rpc.rs:1108-1140`). The forwardable list
  (`engine/src/rpc.rs:1518-1639`) already covers files, terminals, diffs,
  pull requests, repositories, worktrees, harnesses, agent logins, queues,
  moves, and voice. A browser that can reach the device room can drive a
  paired computer the same way the desktop drives another computer.
- **Four foundations unlock everything else** (phase 7): cookie auth on the
  rest of `/v1`, room tokens for the registry and device rooms, the browser
  as a paired device with its own revocable grant, and a sidebar read from
  the registry.
- **A few things never run in a browser.** Agents, shells, git, and files
  run on a computer: the user's own or a cloud computer. The browser never
  makes a phone-pairing code. Whether it holds the wallet seed is U5's
  custody decision (#12258). See [What the web can never do](#what-the-web-can-never-do-and-what-it-does-instead).
- **Not parity gaps.** The desktop has no land queue view, no Gym, no
  Verse, and no AgentGraph view (#12231). The web already has
  `/agent/foreman` and the land queue in `/settings/agents`; Gym results on
  agent pages are U4 (#12257), and `/verse` is U8 (#12261).
- The plan filed 33 issues in seven phases; the owner then closed seven
  as desktop-only (see [Desktop-only](#desktop-only-owner-decision-2026-10-20)).
  Each remaining issue is sized for one agent
  session.

## How the desktop is built, and what that means for the web

```
Desktop                                   Browser (target)
-------                                   ----------------
GPUI ui crate                             GPUI web app (openagents-web-chat)
  |  typed RPC (openagents_rpc)             |  openagents-client, web transport (#12217)
  v                                         v
local engine ---- device room ----------> any engine (paired computer or cloud computer)
  |  registry room, chat2 rooms              ^
  v                                         |  registry and chat2 rooms (#12224, phase 7)
coder-sync (sync.openagents.com) <----------+
  |
account SDK (api.openagents.com/v1) <---- cookie auth on /v1 (#12217, phase 7)
```

Three kinds of desktop feature follow from this:

1. **Account features** call the account API through `openagents-sdk`:
   saved chats, cloud chats, Agent Lab, Labs, schedules, voice choice,
   identity. The browser calls the same routes with the sign-in cookie.
   Today the cookie bridge (`web:cookie_api.rs:35-40`) covers only
   `/v1/threads*` and `/v1/projects`, so most of these need phase 7's cookie
   auth issue first.
2. **Host features** are engine RPC methods: chats on a computer, files,
   harnesses, agent accounts, check-ins, moves, voice. The browser sends them through the device room with
   `Client::host_rpc`, the way the phone does. This needs the browser to be
   a paired device.
3. **Device features** use the operating system of the machine running the
   app: the microphone, notifications, screen capture, the Keychain, native
   webviews, haptics. The browser has its own versions of some of these
   (`getUserMedia`, the Notification API, `getDisplayMedia`, IndexedDB).
   Others have no browser version.

One exception to the RPC rule: the Earn page runs the `openagents` command
line from the UI process (`ui/src/settings/earn.rs:462-677`), not through the
engine. A remote Earn page needs new engine RPC methods first.

## Feature matrix

Size: **S** is under about 500 changed lines, **M** is one agent session,
**L** is larger and is split into several issues. "Web today" lists HTML
pages (HTMX), `/v1` APIs, and GPUI surfaces (`/gpui`, `/demo/gpui`,
`/agent/foreman`, landers).

### Already planned (phases 0a to 6)

These rows are in the chat plan. They are listed so the matrix is complete.

| Feature | Desktop | Web today | Plan |
| --- | --- | --- | --- |
| Portable chat views, theme, transcript, Markdown | `ui/src/transcript.rs:10155`, `ui/src/markdown/` | `/gpui` uses copies in `web/chat-view` | #12216 |
| `openagents-client` web transport, cookie auth on `/v1/threads` | `client/src/client.rs` (phone only) | `web/client` for `/v1/threads*` and `/v1/projects` | #12217 |
| Rust Native cards through a GPUI adapter | `ui/src/native_actions.rs` | Named cards only | #12218 |
| Size and first paint | n/a | 1.90 MB brotli | #12219, #12237, #12244 |
| Accessibility mirror, find in chat, links, zoom | No find bar in `ui/src` | None in `/gpui` | #12220 |
| Web chat read, stream, reply, stop | `ui/src/openagents_composer.rs:93` | `/gpui/chat/{id}` | #12221 |
| New web chat and the web-chat sidebar (rename, pin, archive, move) | `ui/src/shell/account_chats.rs:346` | `/gpui`, `web:pages/chat_sidebar.rs:37-40` | #12222 |
| Chat cards: files, approvals, agents panel, computer asks, continue, GitHub tools, Lab, Game, CAD, Robot | `ui/src/lab_view.rs:109`, `ui/src/lab_live.rs:19` | HTML fragments | #12223 |
| Coder chats and computer sessions over chat rooms | `engine/src/doc_host.rs:2324` | `/chat/sessions/{id}`, read-only | #12224 |
| Environments, work runs, Mac jobs | none on desktop | `/environments`, `/work/{id}`, `/settings/mac-jobs/{id}` | #12225 |
| Default on, HTMX chat removed | n/a | n/a | #12226 |

### Phase 7: workspace foundation

| Feature | Desktop | Web today | In a browser | Backend needed | Size | Issue |
| --- | --- | --- | --- | --- | --- | --- |
| Cookie auth on the rest of `/v1` | SDK bearer from the Keychain (`account/src/platform.rs:224`) | Cookie bridge covers `/v1/threads*` and `/v1/projects` only (`web:cookie_api.rs:35-40`); traces, Mac jobs, work runs, and Labs take the cookie without the same-origin check (`web:traces.rs:985`, `web:mac_jobs.rs:1696`, `web:work_runs.rs:1815`, `web:labs_api.rs:15`) | Same-origin cookie plus CSRF header | Extend `cookie_api` to history, computers, cloud host, agent definitions, assignments, schedules, Labs, Agent Lab, published agents, voice agent, keys, device keys, account, attention, notify, land, project detail, files | S | [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262) |
| Registry and device room tokens | Engine signs in to the edge (`engine/src/workspace_host.rs:388`) | None; chat CSP is `connect-src 'self'` (`web:chat_html.rs:13`) | Short-lived token in the WebSocket query, as the phone does (`client/src/live/urls.rs:2`) | Mint registry and device room tokens beside #12224's chat room token; CSP allows `sync.openagents.com` | S | [#12263](https://github.com/OpenAgentsInc/openagents/issues/12263) |
| The browser as a paired device | Paired phones in `sync/src/paired_device.rs:226`; host side `engine/src/host_rpc.rs:49-58` | None | The browser gets its own NIP-HOST device grant, key in IndexedDB, revocable from Computers | Grant issuance through an approval on a signed-in computer or phone; `Client::host_rpc` and `pair_device_channel` (`client/src/client.rs:554`) on the web transport | M | [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264) |
| Sidebar from the registry: every computer's chats, devices, projects, pins, sections, view options, archived | `ui/src/shell.rs:9203`, `ui/src/shell/spaces.rs:5065`, `:4012`, `ui/src/shell/sidebar_pins.rs:111`, `ui/src/shell/sidebar_sections.rs:250`, `ui/src/settings/archived.rs:222` | Web chats only (`web:pages/chat_sidebar.rs`); `/v1/history/pins` | Registry room through `openagents-client` (`client/src/client.rs:955-1110`) | Room token ([#12263](https://github.com/OpenAgentsInc/openagents/issues/12263)) | M | [#12265](https://github.com/OpenAgentsInc/openagents/issues/12265) |

### Phase 8: chats on your computers

| Feature | Desktop | Web today | In a browser | Backend needed | Size | Issue |
| --- | --- | --- | --- | --- | --- | --- |
| New session on a computer: device, project, branch, and checkout pickers; add a project; harness, model, and reasoning picker | `ui/src/shell/tabs.rs:238`, `ui/src/pickers.rs:3651`, `:4128`, `:4271`, `:4333`, `ui/src/shell/spaces.rs:6854` | Web composer row picks project, branch, and runtime for cloud chats (`web:composer_row.rs:71-72`); no `/v1` branches API | Host RPC `ListHarnesses`, `ListModels`, `ListRefs`, `SwitchRef`, `ListFolders`, `CreateWorktree`; `create_session` in the registry | [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264), [#12265](https://github.com/OpenAgentsInc/openagents/issues/12265) | M | [#12266](https://github.com/OpenAgentsInc/openagents/issues/12266) |
| Full composer: multi-line Markdown, drafts, Enter or mod-Enter, slash commands and skills, @ file mentions | `ui/src/composer.rs:10701`, `:8258`, `:7838`, `ui/src/composer_markdown.rs:415` | Single-line field in `web/chat-view/src/composer.rs` | GPUI text input in wasm; IME through the hidden field | Host RPC `ListCommands`, `ListSkills`, `SearchFiles` | M | [#12267](https://github.com/OpenAgentsInc/openagents/issues/12267) |
| Message queue panel and the question wizard | `ui/src/queue.rs:361`, `ui/src/composer.rs:9871` | None | Queue lives on the chat document, so it syncs through the room | `QueueCommand` (`RespondInput`) and the queued-message methods (`client/src/session/mod.rs:1002-1420`) | M | [#12268](https://github.com/OpenAgentsInc/openagents/issues/12268) |
| Attachments to a computer, and screenshots | `ui/src/attachments.rs:527`, `:1169`, `ui/src/shell/chat_dropzone.rs`, `ui/src/appshots.rs:593` | Web-chat files only (`/v1/threads/{id}/files`) | Paste, pick, and drop files; `getDisplayMedia` for a screenshot in place of Appshots | Host RPC `UploadChunk`, `UploadCommit` (`client/src/client.rs:1787`) | M | [#12269](https://github.com/OpenAgentsInc/openagents/issues/12269) |
| Transcript extras: todo panel, context ring, plan-usage ring and account switch, message rail, tool progress, tool images, workspace links, fork, retry delivery, connection pill, saved-agent identity | `ui/src/todo_panel.rs:279`, `ui/src/context_usage.rs:8`, `ui/src/account_usage.rs:427`, `ui/src/rail.rs:131`, `ui/src/tool_progress.rs:28`, `ui/src/tool_images.rs:120`, `ui/src/workspace_links.rs:282`, `ui/src/shell/fork_chat.rs:31`, `ui/src/transcript.rs:7058`, `ui/src/shell.rs:8944`, `ui/src/openagents_saved_agent.rs:23` | None in `/gpui` | Chat document data plus host RPC | `FetchToolBlob`, `ForkChat`, `RetryDelivery`, `ListAgentAccounts`, `ActivateAgentAccount` | M | [#12270](https://github.com/OpenAgentsInc/openagents/issues/12270) |

### Phase 9: workspace panes over a computer

| Feature | Desktop | Web today | In a browser | Backend needed | Size | Issue |
| --- | --- | --- | --- | --- | --- | --- |
| Right pane and file panel, read: tree, search, text, Markdown and image preview, download | `ui/src/shell.rs:11829`, `:12157`, `ui/src/files/mod.rs:302`, `ui/src/files/preview.rs:2785`, `ui/src/files/download.rs:39` (#12213) | Chat files only (`web:chat_files.rs:643`) | Host files through the device room; download as a browser `Blob` | `ListWorkspaceDirectory`, `ReadWorkspaceFile`, `ReadWorkspaceImage`, `ReadWorkspaceBytes`, `SearchWorkspaceFiles` (`rpc/src/lib.rs:156-172`) | M | [#12271](https://github.com/OpenAgentsInc/openagents/issues/12271) |
| File panel, write: edit and save, rename, move, delete, drag, watch, autosave, word wrap | `ui/src/files/client.rs:140-411`, `ui/src/settings/files.rs:81` | None | Same | Workspace write methods, `WatchWorkspaceFiles` | M | [#12272](https://github.com/OpenAgentsInc/openagents/issues/12272) |
| Command palette and keyboard shortcuts | `ui/src/shell/command_palette.rs:1125`, `ui/src/app_menus.rs:145`, `ui/src/settings.rs:1160` | None | Same palette; the keymap skips chords the browser keeps (mod-W, mod-N, mod-T) | `SearchWorkspaceFiles`, `SearchWorkspaceContent` | M | [#12279](https://github.com/OpenAgentsInc/openagents/issues/12279) |

### Phase 10: computers and cloud computers

| Feature | Desktop | Web today | In a browser | Backend needed | Size | Issue |
| --- | --- | --- | --- | --- | --- | --- |
| Computers: devices, online status, rename, remove, this browser's grant | `ui/src/settings/devices.rs:267`, `sync/src/registry.rs` | Computers section on `/settings`; `POST /settings/computers/remove` (`web:device.rs:74`) | Registry room; revoke grants | `rename_device` (`client/src/client.rs:1198`), grant revocation | S | [#12280](https://github.com/OpenAgentsInc/openagents/issues/12280) |
| Cloud computers: list, create, stop, wake, usage, and use one for a new session | Reached only through the move sheet (`ui/src/shell/move_sheet.rs`) | `/v1/computers*` (`web:phone_api.rs:107-156`), `/v1/cloud-host` (`web:cloud_host.rs:128`); `/cloud` redirects to `/` | Account API | [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262) | M | [#12281](https://github.com/OpenAgentsInc/openagents/issues/12281) |
| Move a chat to a cloud computer and bring it back | `ui/src/shell/move_sheet.rs:93`, `engine/src/sessions/move_cloud.rs`, `rpc/src/moves.rs:12-26` | None | The source computer does the git work; the browser sends the move commands to it | `PlanMove`, `ApproveMove`, `StartMove`, `MoveStatus`, `BringBack` | M | [#12282](https://github.com/OpenAgentsInc/openagents/issues/12282) |
| Providers: per-computer harness install, enable, update policy, updates; agent accounts (add, switch, remove, usage) | `ui/src/settings/harnesses.rs:1156`, `ui/src/settings/accounts.rs:1873`, `ui/src/shell/harness_updates.rs:358` | `/settings/agent-accounts` lists cloud agent accounts | Host RPC | `InstallHarness`, `SetHarnessEnabled`, `Check/Apply/CancelHarnessUpdate`, `Start/Poll/Complete/CancelAgentLogin`, `Activate/ForgetAgentAccount` | M | [#12283](https://github.com/OpenAgentsInc/openagents/issues/12283) |

### Phase 11: agents

| Feature | Desktop | Web today | In a browser | Backend needed | Size | Issue |
| --- | --- | --- | --- | --- | --- | --- |
| Check-ins: sidebar strip, pause and resume, interval, standing authority, grants, decision log | `ui/src/shell/check_ins.rs:28`, `ui/src/settings/agent_wakes.rs:130`, `engine/src/doc_host/agent_wakes.rs:76` | Shown on `/agent/foreman` only; `/settings/schedules` for scheduled prompts | Host RPC for the wake schedule; account API for grants and decisions | `Get/SetAgentWakes`, `GrantAgent`, `RevokeAgentGrant`; `/v1/agent-definitions/{id}/grants`, `/decisions` | M | [#12284](https://github.com/OpenAgentsInc/openagents/issues/12284) |
| Agent Lab: Agent, Tests, and Results; start runs on your computers; promote | `ui/src/agent_lab.rs:122`, `account/src/agent_lab.rs:155-444`, root `crates/agent-lab` | `/labs/agent` (HTML with the Rust Native browser adapter, `web:agent_lab_page.rs:35`); `/v1/labs/agent/*` | Same views through the GPUI Rust Native adapter | [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262); runs still go to a computer acting as the Lab host (`engine/src/doc_host/agent_lab.rs:78`) | M | [#12285](https://github.com/OpenAgentsInc/openagents/issues/12285) |
| Saved agents in chat (mark, name, chip) | `ui/src/openagents_saved_agent.rs:23`, `ui/src/composer.rs:16241` | `/agent/{id}` pages | Chat metadata | None | S | in [#12270](https://github.com/OpenAgentsInc/openagents/issues/12270) |
| AgentGraph and public agent pages | None (#12231) | `/agent/foreman`, `/agent/{id}` | Already GPUI on the web | None | n/a | #12231, U2 #12255 |
| Fork and share agents and chats | Copy a chat link (`ui/src/links.rs:44`), fork a chat (`ui/src/shell/fork_chat.rs:31`) | `/share/{token}`, `/v1/published-agents/{id}/fork` | Account API | None | n/a | U3 #12256 |
| Land queue | None on desktop; MCP `land_status` only (`mcp/src/tools.rs:168`) | Shown in `/settings/agents`; `/v1/land` (`web:landing.rs:366`) | n/a | n/a | n/a | #12251 |

### Phase 12: Labs and visuals

| Feature | Desktop | Web today | In a browser | Backend needed | Size | Issue |
| --- | --- | --- | --- | --- | --- | --- |
| Lab row: image, compare, video, 3D, playable, report | `ui/src/lab_view.rs:109`, `ui/src/lab_model.rs:130` | `/labs/{kind}/{document}`, `/v1/labs/*` | Account API | None | n/a | #12223 |
| Agent visuals (`show_visual`): charts, tables, controls, canvases, 3D | `ui/src/visuals.rs`, `visual/` (Luau through `mlua`), `visual-gpu/` | None | Luau is C, so it needs a `wasm32` build or another runner; `visual-gpu` draws through wgpu on the web | None | M (spike and first visual) | [#12287](https://github.com/OpenAgentsInc/openagents/issues/12287) |
| Local Video Lab card | `ui/src/lab_card.rs:81` (loopback `127.0.0.1:8792`) | None | Never: it's a local developer server | n/a | n/a | none |

### Phase 13: settings, account, notifications, voice, and Earn

| Feature | Desktop | Web today | In a browser | Backend needed | Size | Issue |
| --- | --- | --- | --- | --- | --- | --- |
| Settings shell; General, Notifications, Shortcuts, Files, and Archived pages | `ui/src/shell.rs:534`, `:5556`, `ui/src/settings/shortcuts.rs:949`, `:1036`, `ui/src/settings/notifications.rs:145`, `ui/src/settings/thread_naming.rs`, `ui/src/settings/worktrees.rs` | `/settings` HTML: profile, appearance, API keys, memory, schedules, traces, Claude, data, chats (`web:settings.rs:36-56`) | Rust Native forms; device-only choices in browser storage; computer settings through host RPC | `Get/SetTitleSettings`, `Get/SetWorktreeSettings` | M | [#12288](https://github.com/OpenAgentsInc/openagents/issues/12288) |
| Appearance: scheme, light and dark themes, accent, glass, background image and effects, motion, fonts, conversation width, theme library | `ui/src/settings/appearance.rs:3380`, `ui/src/theme_library.rs:55` | Light and dark on `/settings` | Browser storage; a theme file is picked or fetched instead of read from a folder | None | M | [#12289](https://github.com/OpenAgentsInc/openagents/issues/12289) |
| Account and identity: ID (npub), linked devices and keys, sign out, delete account (profile name and picture are U1, #12254) | `ui/src/openagents_account.rs:25`, `ui/src/openagents_account/identity.rs:376`, `account/src/identity.rs` | `/login`, key and QR sign-in (`web:web_sign_in.rs:89-96`), `/settings/sign-in-keys`, `DELETE /v1/account` | Sign in on the web as today; the recovery words stay on the user's devices | `/v1/device/keys`, `/v1/account` with the cookie ([#12262](https://github.com/OpenAgentsInc/openagents/issues/12262)) | S | [#12290](https://github.com/OpenAgentsInc/openagents/issues/12290) |
| Notifications, sounds, and toasts | `ui/src/notify.rs:38`, `ui/src/sound.rs:45`, `ui/src/toast.rs:330`, `ui/src/shell.rs:3040-3090` | None; `/v1/attention` (SSE), `/v1/notify` | Notification API when the tab is in the background, Web Audio sounds, toasts; web push later | `/v1/attention`, `/v1/notify` with the cookie ([#12262](https://github.com/OpenAgentsInc/openagents/issues/12262)) | M | [#12291](https://github.com/OpenAgentsInc/openagents/issues/12291) |
| Dictation | `ui/src/dictation.rs`, `voice/src/lib.rs:56` (local Parakeet model) | None | Microphone through `getUserMedia`; transcription on a paired computer's model through the device room, not in the tab | New engine RPC to transcribe a clip | M | [#12292](https://github.com/OpenAgentsInc/openagents/issues/12292) |
| Voice agent stage | `ui/src/voice.rs:17`, `ui/src/shell/voice_stage.rs:382`, `engine/src/voice/remote.rs` | `GET/PUT /v1/voice-agent` | WebRTC is native in the browser; signaling goes to the computer's Codex voice session through the device room | `voice_transport` (`client/src/client.rs:672`), the remote voice methods | M | [#12293](https://github.com/OpenAgentsInc/openagents/issues/12293) |
| Earn (share a computer) | `ui/src/settings/earn.rs:781` (runs the `openagents` CLI) | `/pylons`, pylon-card; `/earn` answers 404; `/v1/earnings/*` through the gateway | Control a paired computer's Pylon | New engine RPC for Pylon status, earnings, settings, up, down, and payout | M | [#12294](https://github.com/OpenAgentsInc/openagents/issues/12294) |
| Wallet: balance, send, receive, history | Balance and create only, through the CLI (`ui/src/settings/earn.rs:462-677`); send and receive in `mobile/src/wallet/mod.rs` | `/v1/balance` through the gateway | Wasm Spark wallet in the browser | Breez SDK wasm binding | n/a | U5 #12258 |
| Phone pairing code | `ui/src/shell/connect_phone.rs:151`, `engine/src/connect_phone.rs:1-9` | `/connect` | Never: codes come only from the computer's own app | n/a | n/a | none |
| App update, host upgrade, harness CLI on this machine, MCP server, menu-bar item, haptics | `ui/src/app_update.rs:92`, `ui/src/host_upgrade.rs:36`, `mcp/src/lib.rs:88`, `ui/src/background.rs`, `ui/src/haptics.rs:3` | n/a | Not needed: the site updates on deploy, and these belong to the machine | n/a | n/a | none |
| Design Lab tour | `ui/src/shell/design_lab_tour.rs:107` | `/ui/screens.json` | Developer tool; capture the web app the same way later | n/a | n/a | none |

### Desktop-only (owner decision, 2026-10-20)

The owner decided the web app won't have these. Their issues are closed as
not planned. They stay in the desktop app.

| Feature | Desktop | Closed issue |
| --- | --- | --- |
| Terminal | `ui/src/terminal/panel.rs:321`, `engine/src/terminals.rs:202` | [#12273](https://github.com/OpenAgentsInc/openagents/issues/12273) |
| Changes pane: working tree, branch, and latest-turn diffs, unified or split, syntax, discard | `ui/src/changes.rs:1637`, `ui/src/syntax_cache.rs`, `engine/src/diff_sync.rs:378` | [#12274](https://github.com/OpenAgentsInc/openagents/issues/12274) |
| Line comments on diffs and files, staged into the next prompt | `ui/src/comments.rs:131`, `ui/src/comment_ui.rs:48` | [#12275](https://github.com/OpenAgentsInc/openagents/issues/12275) |
| Pull request pane: description, checks, review threads, review stream, post a comment, hand off to an agent | `ui/src/pull_request_detail.rs:405`, `ui/src/pull_request_handoff.rs:193`, `ui/src/shell/chat_pull_requests.rs` | [#12276](https://github.com/OpenAgentsInc/openagents/issues/12276) |
| Git history pane and project run actions | `ui/src/history.rs:1303`, `ui/src/shell/actions_ui.rs:508`, `ui/src/project_actions.rs:94` | [#12277](https://github.com/OpenAgentsInc/openagents/issues/12277) |
| Dev-server previews | `ui/src/browser/mod.rs:145`, `preview/src/service.rs:91`, `preview/src/signaling.rs:128` | [#12278](https://github.com/OpenAgentsInc/openagents/issues/12278) |
| Video timeline editor | `ui/src/video_timeline.rs:24`, `account/src/video_editor.rs` | [#12286](https://github.com/OpenAgentsInc/openagents/issues/12286) |

The file panel ([#12271](https://github.com/OpenAgentsInc/openagents/issues/12271), [#12272](https://github.com/OpenAgentsInc/openagents/issues/12272)) and the command palette ([#12279](https://github.com/OpenAgentsInc/openagents/issues/12279)) stand on their own without these.

## Start here

The ten issues to do first, in order:

1. [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262), cookie auth on `/v1`: small, and every account feature on the web needs it.
2. [#12263](https://github.com/OpenAgentsInc/openagents/issues/12263), room tokens: small, and it opens the registry and device rooms to the browser.
3. [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264), the browser as a paired device: it unlocks every feature that runs on a computer.
4. [#12265](https://github.com/OpenAgentsInc/openagents/issues/12265), the registry sidebar: the web lists every computer's chats, not only web chats.
5. [#12266](https://github.com/OpenAgentsInc/openagents/issues/12266), start a session on a computer: the first time the web runs agents the way the desktop does.
6. [#12267](https://github.com/OpenAgentsInc/openagents/issues/12267), the full composer: used on every turn, and the base for the queue, attachments, and dictation.
7. [#12268](https://github.com/OpenAgentsInc/openagents/issues/12268), the queue and the question wizard: without answers to its questions, an agent on a computer stalls.
8. [#12270](https://github.com/OpenAgentsInc/openagents/issues/12270), transcript extras: many small visible pieces, blocked only by the view split.
9. [#12281](https://github.com/OpenAgentsInc/openagents/issues/12281), cloud computers: people without a computer of their own get a place to run sessions.
10. [#12271](https://github.com/OpenAgentsInc/openagents/issues/12271), the file panel: the one workspace pane the web keeps, and where workspace links open.

## Phases and dependencies

Phases 7 to 13 extend the chat plan's phases 0a to 6. They use the same flag
(`chat_ui = gpui` or `?ui=gpui`) and the same acceptance rule: Chrome,
Safari, and Firefox at desktop size, on the test account and a scratch host.

| Phase | Issues | Needs |
| --- | --- | --- |
| 7. Workspace foundation | [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262), [#12263](https://github.com/OpenAgentsInc/openagents/issues/12263), [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264), [#12265](https://github.com/OpenAgentsInc/openagents/issues/12265) | #12217, #12222, #12224 |
| 8. Chats on your computers | [#12266](https://github.com/OpenAgentsInc/openagents/issues/12266), [#12267](https://github.com/OpenAgentsInc/openagents/issues/12267), [#12268](https://github.com/OpenAgentsInc/openagents/issues/12268), [#12269](https://github.com/OpenAgentsInc/openagents/issues/12269), [#12270](https://github.com/OpenAgentsInc/openagents/issues/12270) | Phase 7, #12216, #12221 |
| 9. Workspace panes | [#12271](https://github.com/OpenAgentsInc/openagents/issues/12271), [#12272](https://github.com/OpenAgentsInc/openagents/issues/12272), [#12279](https://github.com/OpenAgentsInc/openagents/issues/12279) | [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264), #12216 |
| 10. Computers and cloud computers | [#12280](https://github.com/OpenAgentsInc/openagents/issues/12280), [#12281](https://github.com/OpenAgentsInc/openagents/issues/12281), [#12282](https://github.com/OpenAgentsInc/openagents/issues/12282), [#12283](https://github.com/OpenAgentsInc/openagents/issues/12283) | [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262), [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264), [#12265](https://github.com/OpenAgentsInc/openagents/issues/12265), [#12266](https://github.com/OpenAgentsInc/openagents/issues/12266) |
| 11. Agents | [#12284](https://github.com/OpenAgentsInc/openagents/issues/12284), [#12285](https://github.com/OpenAgentsInc/openagents/issues/12285) | [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262), [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264), #12218 |
| 12. Labs and visuals | [#12287](https://github.com/OpenAgentsInc/openagents/issues/12287) | #12218 |
| 13. Settings, account, notifications, voice, Earn | [#12288](https://github.com/OpenAgentsInc/openagents/issues/12288) to [#12294](https://github.com/OpenAgentsInc/openagents/issues/12294) | #12216, #12218, [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262), [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264); links to U1 (#12254) and U5 (#12258) |

Phase 6 (#12226, default on) waits only on the chat phases. These phases can
ship behind the flag after it.

Issue-level dependencies:

- [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262) needs #12217. [#12263](https://github.com/OpenAgentsInc/openagents/issues/12263) needs [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262) and #12224. [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264) needs [#12263](https://github.com/OpenAgentsInc/openagents/issues/12263) and #12217. [#12265](https://github.com/OpenAgentsInc/openagents/issues/12265) needs [#12263](https://github.com/OpenAgentsInc/openagents/issues/12263) and
  #12222.
- [#12266](https://github.com/OpenAgentsInc/openagents/issues/12266) needs [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264) and [#12265](https://github.com/OpenAgentsInc/openagents/issues/12265). [#12267](https://github.com/OpenAgentsInc/openagents/issues/12267) needs #12216 and #12221. [#12268](https://github.com/OpenAgentsInc/openagents/issues/12268) needs [#12267](https://github.com/OpenAgentsInc/openagents/issues/12267) and #12224.
  [#12269](https://github.com/OpenAgentsInc/openagents/issues/12269) needs [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264) and [#12267](https://github.com/OpenAgentsInc/openagents/issues/12267). [#12270](https://github.com/OpenAgentsInc/openagents/issues/12270) needs #12216, #12224, and [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264).
- [#12271](https://github.com/OpenAgentsInc/openagents/issues/12271) needs [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264) and #12216. [#12272](https://github.com/OpenAgentsInc/openagents/issues/12272) needs [#12271](https://github.com/OpenAgentsInc/openagents/issues/12271). [#12279](https://github.com/OpenAgentsInc/openagents/issues/12279)
  needs [#12265](https://github.com/OpenAgentsInc/openagents/issues/12265) and [#12271](https://github.com/OpenAgentsInc/openagents/issues/12271).
- [#12280](https://github.com/OpenAgentsInc/openagents/issues/12280) needs [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264) and [#12265](https://github.com/OpenAgentsInc/openagents/issues/12265). [#12281](https://github.com/OpenAgentsInc/openagents/issues/12281) needs [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262) and [#12266](https://github.com/OpenAgentsInc/openagents/issues/12266). [#12282](https://github.com/OpenAgentsInc/openagents/issues/12282) needs [#12281](https://github.com/OpenAgentsInc/openagents/issues/12281) and [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264). [#12283](https://github.com/OpenAgentsInc/openagents/issues/12283) needs [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264) and
  [#12288](https://github.com/OpenAgentsInc/openagents/issues/12288).
- [#12284](https://github.com/OpenAgentsInc/openagents/issues/12284) needs [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262) and [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264). [#12285](https://github.com/OpenAgentsInc/openagents/issues/12285) needs [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262) and #12218.
- [#12287](https://github.com/OpenAgentsInc/openagents/issues/12287) needs #12218.
- [#12288](https://github.com/OpenAgentsInc/openagents/issues/12288) needs #12216 and #12218. [#12289](https://github.com/OpenAgentsInc/openagents/issues/12289) and [#12290](https://github.com/OpenAgentsInc/openagents/issues/12290) need [#12288](https://github.com/OpenAgentsInc/openagents/issues/12288); [#12290](https://github.com/OpenAgentsInc/openagents/issues/12290) also needs [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262). [#12291](https://github.com/OpenAgentsInc/openagents/issues/12291)
  needs [#12262](https://github.com/OpenAgentsInc/openagents/issues/12262) and [#12265](https://github.com/OpenAgentsInc/openagents/issues/12265). [#12292](https://github.com/OpenAgentsInc/openagents/issues/12292) needs [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264) and [#12267](https://github.com/OpenAgentsInc/openagents/issues/12267). [#12293](https://github.com/OpenAgentsInc/openagents/issues/12293) needs [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264). [#12294](https://github.com/OpenAgentsInc/openagents/issues/12294) needs [#12264](https://github.com/OpenAgentsInc/openagents/issues/12264) and [#12288](https://github.com/OpenAgentsInc/openagents/issues/12288).

## What the web can never do, and what it does instead

| The desktop does | The web can't, because | The web does instead |
| --- | --- | --- |
| Runs Claude Code, Codex, OpenCode, Cursor, and the other harnesses as local processes (`harness/src/lib.rs`, `engine/src/sessions.rs:582`) | A tab can't start processes | Runs them on a paired computer or a cloud computer, and shows the chat through its room |
| Opens a shell in a local PTY (`engine/src/terminals.rs:202`) | No PTY in a browser | Nothing: the terminal is desktop-only (owner decision) |
| Reads and writes project files and runs `git` and `gh` | No file system or git | Uses the computer's files through host RPC ([#12271](https://github.com/OpenAgentsInc/openagents/issues/12271), [#12272](https://github.com/OpenAgentsInc/openagents/issues/12272)); diffs, pull requests, and git history are desktop-only (owner decision) |
| Keeps the identity and session in the Keychain (`account/src/platform.rs:224`) | No Keychain | Uses the sign-in cookie for the account and a revocable device grant for computers ([#12264](https://github.com/OpenAgentsInc/openagents/issues/12264)); recovery words in the browser are U5's decision (#12258) |
| Makes a phone-pairing code | Codes come only from the computer's own app, by design (`engine/src/connect_phone.rs:5-9`) | Shows the computer list and lets the user revoke grants ([#12280](https://github.com/OpenAgentsInc/openagents/issues/12280)); pairing the browser is approved on a computer or phone |
| Earns by sharing the computer (Pylon) | A tab is not a compute provider | Turns a paired computer's Pylon on and off and shows earnings ([#12294](https://github.com/OpenAgentsInc/openagents/issues/12294)) |
| Holds and spends the wallet through the CLI and the phone | The desktop's wallet is a local process | A wasm Spark wallet in the browser, with custody chosen in U5 (#12258) |
| Captures other app windows with Appshots | Browsers capture only what the user picks | `getDisplayMedia` screenshot as an attachment ([#12269](https://github.com/OpenAgentsInc/openagents/issues/12269)) |
| Shows local dev servers in a native webview | No native webview | Nothing: previews are desktop-only (owner decision) |
| Transcribes speech with a local model | The model is too large for a tab | Records in the browser and transcribes on a paired computer ([#12292](https://github.com/OpenAgentsInc/openagents/issues/12292)) |
| Posts OS banners and keeps running in the menu bar | Closing the tab ends it | The work keeps running on the computer; banners while the tab is open ([#12291](https://github.com/OpenAgentsInc/openagents/issues/12291)), web push later |
| Self-updates, upgrades the host, serves MCP, plays haptics | Not a browser concern | Nothing; the site updates when it's deployed |
| Opens the local Video Lab at `127.0.0.1:8792` | Loopback developer server | Platform Labs cards (#12223) |

## Risks

- **A browser grant is a new kind of device.** It lives in IndexedDB, which
  an extension or a shared machine can read. Keep its rights narrow (no
  delegation), its lifetime short, and its revocation one click from
  Computers.
- **Room tokens in WebSocket URLs** can land in logs. Keep them short-lived
  and scoped to one room kind, as the phone's are.
- **Wasm builds of native crates.** The syntax highlighter used by file
  previews and `mlua`'s Luau may not build for `wasm32-unknown-unknown`.
  Each issue starts by checking its crate.
- **Size.** Each pane adds code. Load panes as separate modules on first use,
  under the size check from #12219.
- **The `ui` crate split.** Every pane here reuses desktop views, so each
  depends on #12216's split and on #12184 for compile times. Move views
  whole and re-export them.
