# Scope and evidence

## Comparison method

The baseline is the current Rust Zeron desktop at the revision in the
[index](README.md). The comparison includes its UI, the engine and transport
behavior needed by that UI, installation, and local/remote operation. Zeron's
mobile application is included only where its behavior defines desktop
interoperability, such as queued attachments, remote sends, or voice ownership.
OpenAgents mobile, web, terminal, Verse, and infrastructure code count as reuse
candidates, not automatic desktop feature coverage.

The investigation inspected manifests, the current UI module tree, route and
settings enums, RPC methods, engine modules, relevant tests, and feature-specific
documentation. OpenAgents structure was queried through the repository's code
graph before following module and symbol references into source. The graph used
the OpenAgents baseline commit and stayed in `.git/code-graph/`; no graph artifact
belongs in this research directory.

### Status and priority

| Mark | Meaning |
| --- | --- |
| M | Missing from the inspected OpenAgents desktop workflow. A related CLI, web feature, protocol, or library may exist. |
| P | Partial: an exposed workflow exists, but named behavior or coverage is missing. |
| V | Verification gap: overlapping code exists, but equivalent behavior is not established by this source review. |
| D | Product difference: the workflows intentionally differ in existing code; parity needs an explicit design choice. It is not an approved exclusion. |

| Priority | Meaning |
| --- | --- |
| P0 | Foundation or correctness requirement for another parity feature; do before dependent UI ships. |
| P1 | Core coding-workspace parity. |
| P2 | Advanced workflow, customization, or polish; still required for an unqualified parity claim. |

Priorities describe dependencies and proposed sequence, not accepted scope cuts.
A row can be P0 even when the general chat application works today, because a
new file editor or remote terminal would be unsafe without its prerequisites.

### Evidence limits

“Present” means inspected implementation exists. It does not mean a test was run,
a feature is enabled in every package, a provider still accepts its wire
protocol, or a person verified its physical-device behavior. “Missing” means no
equivalent exposed path was found in the inspected desktop surface; it is not a
proof that no related code exists among all workspace crates.

No applications, authenticated sessions, provider calls, production chats,
microphones, browser accounts, or owner computers were used for this analysis.
No product code was copied from Zeron. Its local source and documentation are
reference evidence. This report's proposed designs are recommendations, not
claims that a particular architecture has already been selected.

The old Zeron feature inventory and `PARITY.md` describe an earlier rewrite.
Their “partial,” “deferred,” old tab model, dark-only theme, three-provider list,
and engine-hardening statements cannot be used as current product facts. Current
code wins. Feature-specific documents also retain historical measurements and
unperformed native checks; those limits carry into this analysis.

## OpenAgents source references

Links are relative to this repository. The baseline revision in the index fixes
the meaning of the comparison if a linked source changes later.

| Reference | Inspected source and relevance |
| --- | --- |
| O01 | [Desktop README](../../../crates/openagents-desktop/README.md), [route map](../../desktop/route-map.md), [local Coder](../../desktop/local-coder.md): surface orientation and historical verification |
| O02 | [Chrome](../../../crates/openagents-desktop/src/chrome.rs), [shell](../../../crates/openagents-desktop/src/shell.rs), [model](../../../crates/openagents-desktop/src/model.rs): navigation, sidebar, window state, local host requests |
| O03 | [Desktop chat](../../../crates/openagents-desktop/src/chat.rs), [chat actions](../../../crates/openagents-desktop/src/chat_action.rs), [shared session](../../../crates/openagents-chat-app/src/session.rs): exposed conversation/composer behavior |
| O04 | [Commands](../../../crates/openagents-chat-app/src/commands.rs), [preferences](../../../crates/openagents-chat-app/src/preferences.rs), [desktop settings](../../../crates/openagents-desktop/src/settings.rs), [settings integration](../../../crates/openagents-desktop/src/settings_shell.rs): settings and shortcuts |
| O05 | [Coder run](../../../crates/openagents-chat-app/src/coder_run.rs), [task chat](../../../crates/openagents-chat-app/src/task_chat.rs), [desktop worker](../../../crates/openagents-desktop/src/worker.rs): local execution and host-backed task control |
| O06 | [Change review](../../../crates/openagents-chat-app/src/changes.rs), [review comments](../../../crates/openagents-chat-app/src/review_comments.rs), [review/publish verification](../../desktop/verification/2026-09-30-review-publish/verification.md): exact-review model versus desktop exposure |
| O07 | [Plan panel](../../../crates/openagents-chat-app/src/plan_panel.rs), [subagents](../../../crates/openagents-chat-app/src/subagents.rs), [projection](../../../crates/openagents-chat-app/src/projection.rs): reusable presentation and transcript data |
| O08 | [Image drafts](../../../crates/openagents-chat-app/src/attachments.rs), [desktop image input](../../../crates/openagents-desktop/src/chat_images.rs), [shared Coder tab](../../../crates/openagents-chat-app/src/coder_tab.rs): image formats, limits, routing, enabled default |
| O09 | [Terminal pane](../../../crates/openagents-desktop/src/terminal_pane.rs), [terminal actions](../../../crates/openagents-desktop/src/terminal_action.rs), [PTY crate](../../../crates/coder-pty/README.md), [VT crate](../../../crates/coder-vt/README.md): existing terminal and reuse boundaries |
| O10 | [Transcript adapter](../../../crates/rust-native-desktop/src/transcript.rs), [composer adapter](../../../crates/rust-native-desktop/src/composer.rs), [shared visual values](../../../crates/openagents-chat-app/src/visual.rs): rendering and interaction foundation |
| O11 | [Retained history](../../../crates/openagents-chat-app/src/retained.rs), [history crate](../../../crates/coder-history/Cargo.toml), [host threads](../../../crates/openagents-chat-app/src/host_threads.rs): saved sessions and separate host chat APIs |
| O12 | [Preview switches](../../../crates/openagents-desktop/src/preview.rs), [release guide](../../desktop/release.md), [updates](../../../crates/openagents-desktop/src/updates.rs): default feature visibility and package-specific update behavior |
| O13 | [Native event bridge](../../../crates/openagents-desktop/src/native.rs), [macOS notifications](../../../crates/openagents-desktop/src/mac_notify.rs), [Windows notifications](../../../crates/openagents-desktop/src/win_notify.rs), [accessibility verification](../../desktop/verification/2026-09-30-accessibility/verification.md): OS integration |
| O14 | [Local broker design](../../desktop/local-coder.md), [Computers](../../../crates/coder-computers/README.md), [host](../../../crates/coder-host/README.md), [history connection](../../../crates/coder-connect/README.md), [SSH](../../../crates/coder-ssh/README.md): remote authority and installation reuse |
| O15 | [Browser crate](../../../crates/coder-browser/README.md), [Rust Native specification](../../../crates/rust-native/docs/spec.md), [architecture](../../coder/rust-native/architecture.md): distinguish existing browser-oriented surfaces from a native embedded website view |

### Decisive OpenAgents source checks

These anchors make the most consequential baseline distinctions easy to review:

| Finding | Source check |
| --- | --- |
| Five providers in the local Coder selection path | [`PROVIDERS`](../../../crates/coder/src/task/settings.rs#L76) lists Codex, Claude, Grok, Devin, and OpenCode. |
| Images are enabled again | [`ATTACHMENTS_ENABLED`](../../../crates/openagents-chat-app/src/coder_tab.rs#L542) is `true`; [`attachments_enabled`](../../../crates/openagents-desktop/src/chat.rs#L278) records the re-enablement. |
| Image input is not general multimodal chat | [Image draft routing](../../../crates/openagents-chat-app/src/attachments.rs#L1) holds media for Coder start and defines text-only follow-up restrictions. |
| Desktop shortcuts are read-only | [`bindings`](../../../crates/openagents-chat-app/src/commands.rs#L92) explicitly documents that keys cannot be rebound. |
| The terminal is a local shell, not the remote tabbed workspace | [Terminal ownership and lifecycle](../../../crates/openagents-desktop/src/terminal_pane.rs#L1) describes its in-process host and window lifetime. |
| The plan panel already has local Coder integration | [Run presentation](../../../crates/openagents-chat-app/src/coder_run.rs#L1038) inserts the shared plan panel into the run's rows. |
| Publish already exists and checks the reviewed state | [`can_publish` and `publish`](../../../crates/openagents-chat-app/src/changes.rs#L586) reject stale or unsuitable review state. |
| Several extra screens are hidden by default | [Preview switch](../../../crates/openagents-desktop/src/preview.rs#L1) gates Verse, Map, hosted Gym runs, and feedback. |

The links name source lines in the baseline checkout. If lines move, use the
symbol and baseline revision rather than assuming the same number still points
to the same behavior.

## Zeron source references

These links are pinned to the inspected commit. They are pointers to the local
checkout's corresponding public source, not claims of separate online research.

| Reference | Inspected source and relevance |
| --- | --- |
| Z01 | [Manifest](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/Cargo.toml), [UI modules](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/lib.rs), [current shell](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/shell.rs) |
| Z02 | [Spaces](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/shell/spaces.rs), [sidebar sections](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/shell/sidebar_sections.rs), [pins](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/shell/sidebar_pins.rs), [navigation](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/shell/tabs.rs) |
| Z03 | [Composer](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/composer.rs), [pickers](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/pickers.rs), [composer defaults](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/settings/composer.rs), [completion contract](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/harness-skill-completion.md) |
| Z04 | [Queue contract](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/reference/message-queue.md), [queue UI](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/queue.rs) |
| Z05 | [Transcript](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/transcript.rs), [message rail](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/rail.rs), [todo panel](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/todo-panel.md), [context usage](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/context-usage.md) |
| Z06 | [File surface](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/files/mod.rs), [document state](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/files/document.rs), [file tree contract](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/file-tree-interactions.md) |
| Z07 | [Workspace files](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/engine/src/workspace_files.rs), [file mutations](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/engine/src/workspace_files/mutations.rs), [workspace search](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/engine/src/workspace_search.rs), [search palette](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/shell/command_palette.rs) |
| Z08 | [Changes](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/changes.rs), [Git history](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/history.rs), [source control](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/engine/src/source_control.rs), [comments](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/comments.rs) |
| Z09 | [Pull request board](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/pull_requests.rs), [detail](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/pull_request_detail.rs), [host integration](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/engine/src/change_requests.rs) |
| Z10 | [Actions contract](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/reference/project-actions.md), [repositories](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/engine/src/repos.rs), [worktree settings](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/engine/src/worktree_settings.rs) |
| Z11 | [Provider types](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/proto/src/agent.rs), [harness crate](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/harness/README.md), [provider settings](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/settings/harnesses.rs), [account settings](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/settings/accounts.rs) |
| Z12 | [RPC catalog](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/rpc/src/lib.rs), [entities](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/proto/src/entities.rs), [engine RPC](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/engine/src/rpc.rs) |
| Z13 | [Terminal UI](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/terminal/panel.rs), [terminal engine](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/engine/src/terminals.rs) |
| Z14 | [Publication recovery](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/session-publication.md), [transport reliability](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/transport-reliability.md), [remote delivery](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/remote-send-delivery.md), [local import](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/engine/src/local_import.rs) |
| Z15 | [Browser implementation](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/browser/mod.rs), [browser profiles](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/reference/browser-profiles.md), [web links](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/transcript-browser-links.md), [previews](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/preview-networking.md) |
| Z16 | [Image viewing](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/image-preview.md), [generated images](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/generated-images-validation.md), [Appshots](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/appshots.md) |
| Z17 | [Dictation](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/reference/desktop-dictation.md), [voice UI](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/voice.rs), [call lifecycle](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/voice-session/src/lib.rs), [remote voice evidence limits](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/research/codex-voice-remote.md) |
| Z18 | [Visual renderer](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/visuals.rs), [MCP tools](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/mcp/src/tools.rs), [side chats](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/shell/side_chats.rs) |
| Z19 | [Settings schema](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/settings.rs), [theme system](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/theme-system.md), [custom themes](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/theme_library.rs), [thread naming](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/settings/thread_naming.rs) |
| Z20 | [Updater](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/update/src/lib.rs), [notifications](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/notify.rs), [instance lock](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/engine/src/instance_lock.rs), [window startup](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/lib.rs) |
| Z21 | [Resource measurements](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/performance-resource-usage.md), [revisit regression](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/regressions/whale-session-revisit.md), [selection regressions](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/docs/transcript-selection-regressions.md) |
| Z22 | [Motion](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/motion.rs), [panel geometry](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/shell/panel_layout.rs), [hover intent](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/popover/hover_intent.rs), [overscroll](https://github.com/zeronsh/zeron/blob/6ac8859998909bc5afd55838afb0635c281f31c3/crates/ui/src/overscroll.rs) |

## Implementation constraints

Use the existing OpenAgents host, grants, encryption, task commands, and Rust
presentation boundaries. Do not introduce Zeron's TypeScript edge into product
code. Do not turn the desktop into an additional unscoped reader of host keys or
provider credentials. Remote file, terminal, account, and repository operations
need explicit authorization under the OpenAgents authority model.

In particular, read-only history observation does not authorize execution or
file editing. A preview URL does not authorize arbitrary TCP forwarding. A
source-control read does not authorize destructive checkout changes or GitHub
comments. The parity work must make those actions concrete and explicit without
exposing protocol jargon in product copy.
