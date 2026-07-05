# Khala Code UX Behavior Contract

This is the durable home for stated UX expectations for Khala Code — the
answer to "where is correct behavior defined and how is it tested against?"

The machine source of truth is the typed registry at
`clients/khala-code-desktop/src/contracts/ux-contracts.ts`
(schema: `packages/behavior-contracts`, `@openagentsinc/behavior-contracts`).
This document is the human rendering; the test
`clients/khala-code-desktop/tests/ux-contracts.test.ts` fails the normal test
sweep if this doc, the registry, or the oracle tests drift apart.

## Rules

- When the owner (or later, a customer) states a UX expectation in any
  session, the receiving agent must land it in the registry in the same
  change: statement verbatim, source recorded, oracle test written or the
  entry marked `pending` with a blocker ref. Telling a session is recording
  it — that is the point of this file.
- `enforced` requires: at least one oracle, an automated enforcement tier
  (`test-sweep` or `nightly`), and zero blocker refs — the same mechanical
  green-gate discipline as the product-promise registry
  (`docs/promises/registry.md`).
- Oracles must assert on real behavior (mounted DOM, RPC results, harness
  scenarios). Source-string assertions are acceptable only as an explicitly
  labeled stopgap and should carry a follow-up.
- Bump `version` (`YYYY-MM-DD.N`) on every registry change and regenerate the
  registry section below with
  `renderBehaviorContractMarkdown(khalaCodeUxContractRegistry)`.
- Contract deviations found in the wild are strict bugs: file them with the
  contract id in the title.

## How this runs in the normal sweep

- `bun test tests/*.test.ts` in `clients/khala-code-desktop` includes the
  oracle + coverage + doc-sync tests, so they run in the package `verify`
  chain and the repo-root `test:khala-code-desktop` sweep before pushes to
  `main`.
- The registry validation mirrors promise-transition checks
  (`validateBehaviorContractRegistry`), and the coverage check
  (`checkBehaviorContractCoverage`) proves every enforced `bun-test` oracle
  file exists and references its contract id, and every enforced `qa-scenario`
  oracle resolves against the Khala QA seed corpus.
- QA-harness integration: contracts may also carry `qa-scenario` oracles
  referencing `packages/khala-qa-harness` seed-corpus scenario ids; those run
  under the harness runner rather than this package's test glob.
- Nightly: the owned-runner matrix (`bun run qa:nightly`,
  `docs/qa/khala-code-nightly-matrix.md`) runs these oracles via its desktop
  `verify` step and the registry machinery via its `behavior-contracts`
  step, and the QA Swarm customer-one weekly report
  (`docs/qa/qa-swarm-khala-code-standing-engagement.md`) carries per-contract
  status.

## Registry

Registry version: `2026-07-04.6` (schema `openagents.behavior_contracts.v1`)

### `khala_code.chat.sidebar_spinner_streaming_only.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via khala-code-session on 2026-07-03
- **Statement:** The spinner on a chat row in the thread sidebar means an assistant response is streaming in that chat, and nothing else. Clicking a chat must not show that spinner while its messages load; message-loading indication belongs in the chat transcript itself.
- **Enforcement tier:** test-sweep
- **Oracle** `sidebar_spinner.dom` (bun-test, dom): Mounts the real thread sidebar in a DOM: selecting a thread while the resume RPC is in flight renders no spinner anywhere in the list, while a genuinely streaming thread renders the spinner in its time slot. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Oracle** `transcript_loading.dom` (bun-test, dom): Mounts the transcript-level status renderer in a DOM: cache-miss thread switches render a polite 'Loading messages' transcript bubble instead of a sidebar spinner, and assistant thinking uses the same status-bubble structure. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Oracle** `thread_select_fixture_rpc.scenario` (qa-scenario, rpc): Runs the seed-corpus fixture RPC-driver scenario that lists threads, selects the fixture thread, and reads it back without using the row streaming spinner as load state. — `scenario.khala_code.seed.rpc_thread_select_fixture_driver.v1`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** This contract binds indicator semantics only. Thread-switch latency budgets stay owned by docs/qa/khala-code-latency-budgets.md, and it makes no claim about streaming correctness itself.

### `khala_code.chat.sidebar_active_thread_background_only.v1` — RETIRED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via khala-code-session on 2026-07-03
- **Statement:** Current chat is just supposed to be highlighted as a background bar.
- **Enforcement tier:** test-sweep
- **Verification:** Superseded by khala_code.chat.sidebar_active_thread_background_only.v2.
- **Authority boundary:** Retired 2026-07-03 (owner restatement): the highlight existed as a hook but the tone had drifted to a near-invisible surface mix. Superseded by khala_code.chat.sidebar_active_thread_background_only.v2, which additionally pins noticeability; kept for history.

### `khala_code.chat.sidebar_active_thread_background_only.v2` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via khala-code-session on 2026-07-03
- **Statement:** The active chat in the sidebar must have a noticeable background color — not very bright, but clearly visible — so it is always obvious which chat is the active one. It renders as a background bar only, with no 'Current chat' heading or copy, and it must not fade into the sidebar background or disappear.
- **Enforcement tier:** test-sweep
- **Oracle** `active_thread_background_only.dom` (bun-test, dom): Mounts the real thread sidebar in a DOM: an optimistic current chat renders as the active row with the active background hooks and no visible 'Current chat' heading or copy. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Oracle** `active_row_distinct_tone.source` (bun-test, unit): Pins the active-row background to a distinct energy-blue tone: the [data-active="true"] rule must use the khala-energy-blue accent (not the surface mix shared with hover/selecting rows), so the highlight cannot silently fade back into the sidebar background. Rendered appearance is additionally covered by the visual smoke tier. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** The exact tone may be tuned with owner sign-off, but the active row must stay visibly distinct from the hover, selecting, and plain-row tones; reverting it to the sidebar's base surface mix (the drift that made it invisible) is a contract violation, not a style tweak. Persisted project, Codex, Claude, and session-catalog group labels remain owned by their source catalogs.

### `khala_code.chat.recent_thread_cmd_hotkeys.v1` — RETIRED

- **Surface:** khala-code-desktop (chat thread switching)
- **Stated by:** owner via khala-code-session on 2026-07-03
- **Statement:** Holding Cmd shows an overlay listing the nine most recent chats numbered 1 through 9, and pressing Cmd+1 through Cmd+9 jumps to that chat. Releasing Cmd hides the overlay.
- **Enforcement tier:** test-sweep
- **Verification:** Superseded by khala_code.chat.recent_thread_cmd_hotkeys.v2.
- **Authority boundary:** Retired 2026-07-03 (owner correction): the overlay reading of the original ask was wrong — no separate pane should appear. Superseded by khala_code.chat.recent_thread_cmd_hotkeys.v2; kept for history.

### `khala_code.chat.recent_thread_cmd_hotkeys.v2` — ENFORCED

- **Surface:** khala-code-desktop (chat thread switching)
- **Stated by:** owner via khala-code-session on 2026-07-03
- **Statement:** Holding Cmd does not open a separate pane; it temporarily replaces the timestamps of the nine most recent chats in the sidebar with their command-digit hotkeys (⌘1 through ⌘9). Pressing Cmd+1 through Cmd+9 jumps to that chat, and releasing Cmd restores the timestamps.
- **Enforcement tier:** test-sweep
- **Oracle** `cmd_digit_gating.unit` (bun-test, unit): Cmd+1..Cmd+9 map to the first through ninth most recent threads; unmodified digits and digits with other modifiers map to nothing. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Oracle** `sidebar_hotkey_hints.dom` (bun-test, dom): Mounts the real thread sidebar and hotkey-hint listener in a DOM: enabling hotkey hints replaces the time slot of the nine most recent chats with their command-digit hints in place (no separate pane appears anywhere in the document), and Meta release or window blur restores the timestamps. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Cmd+0 additionally maps to the tenth most recent chat and Cmd+ArrowUp/ArrowDown cycle through recency; those are compatible extensions, not part of this contract. The generalized overlay-menu component remains available for future dialog menus but is not mounted for this feature.

### `khala_code.chat.codex_stored_session_records_not_resumed.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via khala-code-session on 2026-07-03
- **Statement:** Codex session rows must not show raw 'invalid session id' parser errors when the row only has stored local or legacy metadata, and they must not appear as normal resumable chats without a loaded title.
- **Enforcement tier:** test-sweep
- **Oracle** `stored_codex_catalog_projection.unit` (bun-test, unit): Projects a stored-only Codex catalog record with a legacy non-UUID ref into a disabled local-record sidebar summary instead of a resumable chat thread. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Oracle** `stored_codex_sidebar.dom` (bun-test, dom): Mounts the real thread sidebar in a DOM: a stored-only Codex record remains visible, is disabled, never displays a raw parser error, and recent-chat selection skips it. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** This contract only governs sidebar behavior for stored/local Codex metadata that lacks a current app-server UUID thread id. It does not define the upstream Codex thread-store retention policy, subagent semantics, or whether historical rollouts can be recovered through a separate import flow.

### `khala_code.transcript.claude_assistant_turn_once.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat transcript)
- **Stated by:** operator-agent via github-issue on 2026-07-03
- **Statement:** An assistant turn can render twice in the transcript — the same reply body appears as two consecutive assistant blocks for a single user turn. Observed on the Claude lane.
- **Enforcement tier:** test-sweep
- **Oracle** `claude_stream_final_snapshot_dedupe.unit` (bun-test, unit): Feeds the Claude projector a streamed assistant text block followed by the final assistant snapshot with the same body, and asserts the transcript keeps exactly one assistant message. — `clients/khala-code-desktop/tests/claude-app-sdk-chat-runtime.test.ts`
- **Verification:** bun test tests/claude-app-sdk-chat-runtime.test.ts tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds duplicate visible assistant text for one Claude turn only. It does not change Claude SDK event ordering, token accounting, or the Codex lane projector.

### `khala_code.claude_lane.isolated_home_and_user_prompt_only.v1` — ENFORCED

- **Surface:** khala-code-desktop (Claude lane)
- **Stated by:** customer via github-issue on 2026-07-03
- **Statement:** The Claude lane starts from a clean Khala Code context rather than the user's global Claude Code memory/config. Non-user transcript system/error text must not be fed to the model as conversation.
- **Enforcement tier:** test-sweep
- **Oracle** `claude_app_sdk_config_dir_isolated.unit` (bun-test, unit): Starts a real Claude runtime with an ambient CLAUDE_CONFIG_DIR and proves query() receives Khala Code's app-managed config directory instead of the user's default/global Claude home. — `clients/khala-code-desktop/tests/claude-app-sdk-chat-runtime.test.ts`
- **Oracle** `claude_prompt_user_only.unit` (bun-test, unit): Submits a transcript containing system, tool, assistant, and older user rows and proves the Claude SDK prompt contains only the latest user-authored message. — `clients/khala-code-desktop/tests/claude-app-sdk-chat-runtime.test.ts`
- **Verification:** bun test tests/claude-app-sdk-chat-runtime.test.ts tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** This binds the desktop Claude chat lane only. Fleet worker Claude accounts remain owned by Pylon's isolated-account registry, and an explicit KHALA_CODE_DESKTOP_CLAUDE_CONFIG_DIR override may intentionally point the desktop lane at a caller-selected app config directory.

### `khala_code.history.app_sessions_default.v1` — ENFORCED

- **Surface:** khala-code-desktop (History sidebar)
- **Stated by:** customer via github-issue on 2026-07-03
- **Statement:** History defaults to chats created in Khala Code Desktop, not every Codex or Claude session from the user's home stores. Showing all home sessions is an explicit opt-in, and stale missing-rollout rows must not permanently bury desktop chats as undismissable red errors.
- **Enforcement tier:** test-sweep
- **Oracle** `session_catalog_app_scope.unit` (bun-test, unit): Builds a mixed app-owned plus headless-runtime catalog and proves the default sessionCatalog scope includes only the app-owned desktop thread while omitting unrelated home/headless prompts. — `clients/khala-code-desktop/tests/session-catalog.test.ts`
- **Oracle** `history_scope_toggle.dom` (bun-test, dom): Mounts the real History sidebar in a DOM and proves the header toggle is off by default, requests app-only history first, and sends includeHomeSessions only after explicit user activation. — `clients/khala-code-desktop/tests/codex-thread-sidebar.test.ts`
- **Oracle** `history_error_dismiss.dom` (bun-test, dom): Mounts the real History sidebar in a DOM, forces a 'no rollout found' resume failure on a dead thread, and proves both the per-thread error row and the global error banner render a dismiss control that clears the error on click without triggering another listThreads fetch. — `clients/khala-code-desktop/tests/codex-thread-sidebar.test.ts`
- **Verification:** bun test tests/session-catalog.test.ts tests/codex-thread-sidebar.test.ts inside clients/khala-code-desktop; both run in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** This contract binds the History/session-catalog default and its explicit opt-in only. It does not prevent app-owned sessions from being enriched with runtime metadata, and it does not promise that externally created home sessions can always be opened successfully.

### `khala_code.composer.no_dead_controls.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat composer)
- **Stated by:** owner via codex-session on 2026-07-03
- **Statement:** Every composer control must visibly do something when interacted with. The 'Plan' toggle did nothing and confused the user about its purpose; it must be removed and replaced with a working reasoning-mode control that actually changes behavior.
- **Enforcement tier:** test-sweep
- **Oracle** `no_dead_controls.source` (bun-test, unit): Pins that the composer has a working reasoning-mode select wired to a real RPC and no lingering dead 'Plan' toggle in mounted code or active CSS. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** This binds control liveness only, not the exact reasoning-mode UI; that design is free to iterate as long as no control ships inert.

### `khala_code.composer.attach_control_icon_only.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat composer)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** The composer's attach control renders as an icon only, never the text label 'Attach'. Queued/follow-up messages (sent while a turn is still streaming) render in a compact, visually distinct style from a normal message, not as another full-size bubble.
- **Enforcement tier:** test-sweep
- **Oracle** `attach_icon_only.source` (bun-test, unit): Pins the desktop-scoped CSS override that keeps the attach control's text label hidden regardless of viewport width, and that queued follow-up messages render in a compact style distinct from a full message bubble. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds the attach control and queued-message rendering only; other composer chrome is out of scope for this contract.

### `khala_code.composer.structure_not_bloat.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat composer)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** The composer stays compact, not oversized, and follows the StarCraft design system. Every visible composer icon must be functional; secondary controls that don't yet do anything real (e.g. mic, extra progress indicators, unused model dropdowns) stay hidden or removed rather than shipped as inert chrome the user has to look at.
- **Enforcement tier:** test-sweep
- **Oracle** `structure_not_bloat.source` (bun-test, unit): Pins that the composer has a working reasoning-mode select and no mic/runtime-badge/harness-pill chrome mounted or styled active. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Structure/liveness only — exact visual treatment is free to iterate with impeccable-skill review as long as inert chrome does not ship.

### `khala_code.chat.no_current_chat_text_flash.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-03
- **Statement:** The active chat row must never flash a 'Current chat' text label anywhere in the sidebar, even momentarily during mount or a state transition. The active-row background alone (khala_code.chat.sidebar_active_thread_background_only.v2) is the only active indicator; no text heading or copy may appear, not even transiently.
- **Enforcement tier:** test-sweep
- **Oracle** `no_current_chat_text_flash.source` (bun-test, unit): Asserts the sidebar source never contains the string 'current chat' anywhere, so no code path (including transient/mount-time ones) can render it. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Complements khala_code.chat.sidebar_active_thread_background_only.v2 (steady-state rendering) by additionally covering transient/mount-time flashes; the two contracts should be verified together.

### `khala_code.chat.harness_badge_removed.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** Sidebar chat rows do not render a Codex/Claude harness-provider text badge next to the title. An earlier version showed this badge with stale/inaccurate values (older threads all labeled 'Claude' regardless of actual harness); the badge is removed entirely rather than kept and fixed.
- **Enforcement tier:** test-sweep
- **Oracle** `harness_badge_removed.dom` (bun-test, dom): Mounts the real thread sidebar with threads carrying Codex/Claude badges and asserts no harness badge element or text renders. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Harness identity may still be surfaced elsewhere (e.g. a settings/detail view); this contract binds only the sidebar row itself.

### `khala_code.chat.sidebar_row_density.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** Sidebar chat rows render borderless, differentiated by background color only (no per-row border chrome), with tightened vertical padding between rows, matching the density of the reference Codex-desktop-style sidebar.
- **Enforcement tier:** test-sweep
- **Oracle** `sidebar_row_density.source` (bun-test, unit): Pins the sidebar row CSS to borderless with the tightened 0.1rem/0.5rem padding and confirms active/hover states never add a non-zero border. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Visual density only; does not change row content or interaction behavior contracted elsewhere.

### `khala_code.chat.thread_open_never_raw_error.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-03
- **Statement:** Opening any thread from the sidebar must never surface a raw internal error string (e.g. 'no rollout found', 'invalid session id: invalid character ...') to the user. On a genuinely missing or corrupt session, show one typed, friendly, actionable message instead. Thread timestamps must never all collapse to showing 'now' when this happens.
- **Enforcement tier:** test-sweep
- **Oracle** `thread_open_error_mapping.unit` (bun-test, unit): Unit-tests the internal-error detector and friendly-message mapper against real raw Codex RPC error strings, and confirms unrelated error text passes through unchanged. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Oracle** `thread_open_error_rendering.dom` (bun-test, dom): Mounts the real thread sidebar with a resumeThread that throws a raw internal error and asserts the rendered row shows the friendly message, never the raw text. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** This is a recurring defect reported repeatedly across 2026-07-02 and 2026-07-03; any fix must be verified against every reported recurrence, not just the most recent report.

### `khala_code.chat.streaming_indicator_survives_navigation.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** A background thread's streaming indicator must keep reflecting its real state even after the user switches to a different chat or starts a new one. Navigating away must never clear another thread's in-progress indicator, and reopening that thread later must still show it as streaming if it genuinely is.
- **Enforcement tier:** test-sweep
- **Oracle** `streaming_survives_navigation.source` (bun-test, unit): Pins the per-thread streamingThreadIds tracking: populated at submit time by thread, only cleared when its own turn finishes, and never blanket-cleared by any thread-switch function. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds indicator truthfulness during navigation; does not change how many concurrent streams the app supports.

### `khala_code.chat.new_thread_appears_promptly.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** A freshly created chat appears in the sidebar without delay, and its own first messages render immediately rather than starting blank while data that should already be present loads again.
- **Enforcement tier:** test-sweep
- **Oracle** `new_thread_appears_promptly.dom` (bun-test, dom): Mounts the real thread sidebar and asserts an optimistically-inserted pending thread appears in the list immediately with its preview visible, with no RPC round trip required. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds sidebar/list promptness and initial message rendering only.

### `khala_code.chat.sync_remote_thread_appears_without_restart.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via github-issue on 2026-07-04
- **Statement:** A thread created on another device appears in the Khala Code thread sidebar without restarting the app. Chat sync ordering is newest-first, spinners stay truthful, and the sidebar must not poll the legacy session catalog while a connected chat_thread sync source is available.
- **Enforcement tier:** test-sweep
- **Oracle** `chat_sync_sidebar_source.unit` (bun-test, unit): Pins the renderer source path: the sidebar calls khalaSyncChatThreads before the legacy sessionCatalog cache, projects chat_thread rows through chatThreadToSidebarSummary, and enqueues chat.createThread when the app receives a new thread id. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Oracle** `chat_sync_two_client_collection.integration` (bun-test, unit): Runs the two-client TanStack DB collection integration test: client A creates a chat_thread, client B observes it through the live collection without restart, and recency ordering stays newest-first. — `packages/khala-sync-db-collection/src/index.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop plus bun test packages/khala-sync-db-collection/src/index.test.ts from the repo root.
- **Authority boundary:** Binds the desktop sidebar's source selection and freshness semantics for chat_thread sync rows. It does not promise offline recovery, message-body sync, or cross-device identity beyond the owner chat scope.

### `khala_code.chat.rename_applies_immediately.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** Confirming a thread rename (the check-mark action on the inline rename control) updates the visible sidebar title immediately, without requiring a refresh or a subsequent click.
- **Enforcement tier:** test-sweep
- **Oracle** `rename_applies_immediately.dom` (bun-test, dom): Mounts the real thread sidebar, drives the context-menu rename flow end to end, and asserts the visible title updates before the mocked rename RPC resolves. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds the rename affordance's visible result only, not its persistence/sync mechanics.

### `khala_code.chat.rehydrate_shows_tool_calls.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat transcript)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** Reopening or resuming an older thread renders its historical tool calls in the transcript, not just its text messages. The full turn history, including tool activity, must be reconstructible from a rehydrated session.
- **Enforcement tier:** test-sweep
- **Oracle** `rehydrate_shows_tool_calls.source` (bun-test, unit): References the thread-history projector's full-variant coverage test and confirms messagesFromThread replays every item through the same projector used for live streaming. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds transcript completeness on rehydrate; does not require re-executing any tool.

### `khala_code.chat.starcraft_scrollbar_parity.v1` — ENFORCED

- **Surface:** khala-code-desktop (visual theme)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** The custom StarCraft-themed scrollbar used on openagents.com applies inside Khala Code desktop's scrollable surfaces too (sidebar, transcript, any other scrollable panel), not only the website.
- **Enforcement tier:** test-sweep
- **Oracle** `starcraft_scrollbar_parity.source` (bun-test, unit): Asserts the StarCraft scrollbar theme is declared with the universal selector (automatic parity for every scrollable surface) and that no container opts out. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Theme parity only; does not change scroll behavior or keyboard/wheel handling contracted elsewhere.

### `khala_code.transcript.consecutive_tool_calls_collapsed.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat transcript)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** Consecutive tool calls in the transcript collapse into a single line showing the latest call. Clicking that line expands it to reveal the full list of collapsed calls, and each item in that list can be further clicked to see its own detail.
- **Enforcement tier:** test-sweep
- **Oracle** `consecutive_tool_calls_collapsed.source` (bun-test, unit): Pins the grouping pass and collapsible summary renderer wired into the transcript render path, including the click-to-expand toggle and matching CSS. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Applies to runs of consecutive tool calls only; a tool call interleaved with an assistant message is not collapsed into an adjacent group.

### `khala_code.transcript.tool_call_path_display.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat transcript)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** Tool-call summaries in the transcript show a workspace-relative path, never the absolute filesystem path or worktree prefix. Each summary is a short verb-prefixed label (e.g. 'Read ___', 'Edited ___') immediately beside the tool icon; status is conveyed by icon/color rather than a fully spelled-out word like 'Completed'.
- **Enforcement tier:** test-sweep
- **Oracle** `tool_call_path_display.source` (bun-test, unit): References the projector's relative-path labeling test and confirms tool-call titles are built from the workspace-relative displayPath helper, never an absolute path. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds display formatting only; the underlying tool-call data may still carry the absolute path internally.

### `khala_code.transcript.streaming_state_cross_surface_consistency.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat transcript)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** The sidebar's streaming indicator and the composer's own status readout for the active thread must always agree. It must never be possible for the sidebar to show a thread as streaming while the composer for that same thread simultaneously shows 'ready' (or vice versa).
- **Enforcement tier:** test-sweep
- **Oracle** `streaming_cross_surface_consistency.source` (bun-test, unit): Pins that the composer status and the sidebar streaming badge both derive from the same per-thread isThreadStreaming/streamingThreadIds source of truth, not independent flags that can disagree. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** This is the cross-surface consistency category from the customer behavior-contract catalog applied to our own product first.

### `khala_code.nav.hotbar_no_route_text.v1` — ENFORCED

- **Surface:** khala-code-desktop (app navigation)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** The app-section nav hotbar (fleet/chat/forum/inbox/settings) shows icon plus hotkey only. It must never render a raw route or path fragment as visible text on a hotbar button.
- **Enforcement tier:** test-sweep
- **Oracle** `hotbar_no_route_text.dom` (bun-test, dom): Mounts the real nav hotbar and asserts every slot's visible label matches its static configured label with no route/path-shaped text. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds the hotbar button surface only, not the underlying route model.

### `khala_code.nav.hotbar_hotkey_always_visible.v1` — ENFORCED

- **Surface:** khala-code-desktop (app navigation)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** Every nav hotbar button always displays its own trigger hotkey as a small visible badge (e.g. '⌥1'), not just discoverable by trial. Pressing the displayed modifier-plus-digit combination for a button always routes to that section.
- **Enforcement tier:** test-sweep
- **Oracle** `hotbar_hotkey_always_visible.dom` (bun-test, dom): Mounts the real nav hotbar and asserts every slot renders a non-empty hotkey badge containing its configured digit. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** This is the app-section hotbar (Option+digit); it is distinct from khala_code.chat.recent_thread_cmd_hotkeys.v2's Cmd-hold recent-chat hints, which live in the sidebar rows instead of on hotbar buttons.

### `khala_code.nav.hotbar_no_stray_special_characters.v1` — ENFORCED

- **Surface:** khala-code-desktop (app navigation)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** The Option+digit hotbar shortcut must always be intercepted as a navigation command and must never leak macOS's special/garbled Option-key characters (e.g. ¡™£¢) into any input field or onto the page.
- **Enforcement tier:** test-sweep
- **Oracle** `hotbar_no_stray_special_characters.regression_ref` (bun-test, unit): References the existing regression test that dispatches a real Option+Digit2 keydown (producing the macOS special character '™' in event.key) at a focused input and asserts it is intercepted (defaultPrevented) and routes correctly instead of leaking into the input. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds keyboard interception correctness only.

### `khala_code.menus.flyout_single_line_no_preamble.v1` — ENFORCED

- **Surface:** khala-code-desktop (app-wide menus)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** Right-click and flyout menus render one line per item, with no explanatory subheadline text under each item and no header/preamble content above the options.
- **Enforcement tier:** test-sweep
- **Oracle** `flyout_single_line_no_preamble.dom` (bun-test, dom): Mounts the real thread sidebar, opens its context menu, and asserts no header element and no per-item description/subheadline render. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Applies to all flyout/context menus app-wide, including the thread-action menu and the fleet menu.

### `khala_code.fleet.menu_no_stray_labels.v1` — ENFORCED

- **Surface:** khala-code-desktop (fleet panel)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** The fleet menu must not render stray internal label text (e.g. a literal 'ACCT' tag) that is not part of a designed, human-readable element.
- **Enforcement tier:** test-sweep
- **Oracle** `fleet_menu_no_stray_labels.source` (bun-test, unit): Asserts the fleet panel source never contains the literal string 'ACCT'. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Narrow scope: this contract targets literal leaked internal labels, not the fleet menu's overall information architecture.

### `khala_code.app.resumes_after_restart.v1` — ENFORCED

- **Surface:** khala-code-desktop (app lifecycle)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** When the app restarts, whether voluntary or due to a crash/relaunch, any work that was in flight resumes rather than silently stopping. The user should not have to notice and manually recover in-progress state after a restart.
- **Enforcement tier:** test-sweep
- **Oracle** `resumes_after_restart.source` (bun-test, unit): Pins that the last active thread id is read (not cleared) at boot, restored via a dedicated best-effort function after the initial render, and that a failed restore clears the stale id instead of retrying forever. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds resumability of in-flight work; does not require preserving unsent composer drafts unless separately contracted.

### `khala_code.app.no_unrequested_first_launch_scripts.v1` — ENFORCED

- **Surface:** khala-code-desktop (app lifecycle)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** Features that have not been enabled (e.g. Apple Bridge) must not run any preparation or background script on first launch. A disabled feature stays fully inert until explicitly turned on.
- **Enforcement tier:** test-sweep
- **Oracle** `no_unrequested_first_launch_scripts.regression_ref` (bun-test, unit): References the existing Apple FM bridge disabled-on-launch regression tests and confirms package.json carries no prepare:apple-fm-bridge script. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds first-launch behavior for currently-disabled features; does not block future opt-in enablement flows.

### `khala_code.tokens.per_thread_live_counter.v1` — ENFORCED

- **Surface:** khala-code-desktop (token accounting)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** A live per-thread token counter is visible in the top-right of the Khala Code screen while a thread is active, updating as tokens accrue. Clicking it shows how many of those tokens have synced to the public leaderboard.
- **Enforcement tier:** test-sweep
- **Oracle** `per_thread_live_counter.source` (bun-test, unit): Pins the token counter's top-right CSS placement, its click handler opening the sync-detail popover, and that the popover surfaces both leaderboard-synced and pending-sync token fields. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Display-only claim; does not change the exact-only token accounting invariants (usage_truth='exact', reconciliation against token_usage_events) owned elsewhere.

### `khala_code.terminal.tui_mode_available.v1` — ENFORCED

- **Surface:** khala-code-desktop (terminal)
- **Stated by:** TheBenMeadows (community; relayed via Lathe operator agent PR) via community-feedback-discord on 2026-07-03
- **Statement:** A terminal (TUI) mode is available: an interactive REPL over the same Codex app-server harness the desktop app uses, for users who want the engine without the window.
- **Enforcement tier:** test-sweep
- **Oracle** `tui_mode_available.source` (bun-test, unit): Pins that the TUI script reuses the exact desktop chat/harness/status functions (createCodexAppServerChatRuntime, createCodexAppServerHost, inspectCodexHarnessStatus) rather than a parallel implementation, and exposes the /new, /status, and /exit slash commands. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** Enforced 2026-07-03: shipped via PR #8221 (merged). bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds the existence and reuse of the desktop harness for a terminal REPL; does not change the underlying Codex app-server chat runtime or its approval/sandbox behavior.

### `khala_code.settings.hidden_models_excluded_from_picker.v1` — ENFORCED

- **Surface:** khala-code-desktop (settings)
- **Stated by:** TheBenMeadows (community; relayed via Lathe operator agent issue #8230) via community-feedback-discord on 2026-07-03
- **Statement:** Internal/hidden Codex models (e.g. 'Codex Auto Review') never appear as selectable entries in the Settings model picker.
- **Enforcement tier:** test-sweep
- **Oracle** `hidden_models_excluded_from_picker.dom` (bun-test, dom): Mounts the real Codex settings panel with a model catalog that includes a hidden entry (e.g. 'Codex Auto Review') and asserts it never appears as a selectable option and its label/'(hidden)' marker never leaks into the panel text. — `clients/khala-code-desktop/tests/codex-settings-panel.test.ts`
- **Verification:** Enforced 2026-07-03: fixed for GitHub issue #8230 via PR #8236 (merged). bun test tests/codex-settings-panel.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds the Settings model picker only; other consumers of the model catalog (e.g. diagnostics) may still see hidden entries.

### `khala_code.settings.no_bare_unset_labels.v1` — ENFORCED

- **Surface:** khala-code-desktop (settings)
- **Stated by:** TheBenMeadows (community; relayed via Lathe operator agent issues/PR) via community-feedback-discord on 2026-07-03
- **Statement:** A read-only settings value never displays the bare, unexplained word 'Unset'. When it reflects a default, it says so in plain language (e.g. 'Default').
- **Enforcement tier:** test-sweep
- **Oracle** `no_bare_unset_labels.codex_panel.unit` (bun-test, unit): Mounts the real Codex settings panel with a projection whose read-only config fields (provider, reasoning summary, verbosity, approval, sandbox, etc.) are null, and asserts every rendered metric value is 'Default' and none is the bare word 'Unset'. — `clients/khala-code-desktop/tests/codex-settings-panel.test.ts`
- **Oracle** `no_bare_unset_labels.claude_panel.unit` (bun-test, unit): Mounts the real Claude settings section with a projection whose account fields are null, and asserts every rendered metric value is 'Default' and none is the bare word 'Unset'. — `clients/khala-code-desktop/tests/claude-settings-panel.test.ts`
- **Verification:** Enforced 2026-07-03: fixed as part of the response to community feedback that also produced #8230-#8233 and PR #8221. Both the Codex and Claude settings panels now render 'Default' instead of 'Unset' for null/undefined read-only metric values. Runs on every test-sweep invocation.
- **Authority boundary:** Binds display labeling only — whether a read-only settings value reads as an unexplained 'Unset' or an honest 'Default'. Does not itself make any field editable; that is tracked separately by khala_code.settings.editable_not_env_var_only.v1.

### `khala_code.plans.free_trace_capture_explicit_consent.v1` — ENFORCED

- **Surface:** khala-code-desktop (plans and billing settings)
- **Stated by:** owner via github-issue on 2026-07-04
- **Statement:** Explicit consent UI in the desktop is default OFF and never dark-patterned. It gates a capture pipeline of session events to Rampart redaction to owner_only trace ingest, aligned with data.free_tier_capture_disclosure.v1 and the paid-plan opt-out. Any redaction failure fails closed to not-captured, and capture grants no payout or settlement.
- **Enforcement tier:** test-sweep
- **Oracle** `trace_capture_planner.unit` (bun-test, unit): Runs the pure desktop trace-capture planner and proves default-off consent, paid-plan exclusion, redaction failure, and owner-only ingest gates all return not-captured unless every gate passes; successful owner-only capture keeps payout and settlement markers inert. — `clients/khala-code-desktop/tests/trace-capture.test.ts`
- **Oracle** `trace_capture_consent_panel.dom` (bun-test, dom): Mounts the real plans panel in a DOM and proves the trace-capture consent control is off by default, performs no write before a user toggle, writes only after explicit checkbox activation, and shows paid-plan opt-out as not captured. — `clients/khala-code-desktop/tests/plans-panel.test.ts`
- **Oracle** `trace_capture_consent_rpc.unit` (bun-test, unit): Exercises the desktop RPC consent setting and proves it persists only the explicit boolean consent, calls no network, stays owner-gated, exposes the served disclosure ref, and reports not_captured with inert payout/settlement markers. — `clients/khala-code-desktop/tests/rpc-handlers.test.ts`
- **Verification:** bun test clients/khala-code-desktop/tests/trace-capture.test.ts clients/khala-code-desktop/tests/plans-panel.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/ux-contracts.test.ts; these files run in the package test glob and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds only the desktop consent gate and local capture planner. Production capture remains owner-gated by KHALA_CODE_DESKTOP_TRACE_CAPTURE_ENABLED plus an owner-only ingest sink; this contract does not authorize public traces, payout eligibility, settlement eligibility, promise-green movement, or capture on paid plans.

### `khala_code.plans.checkout_handoff_server_truth.v1` — ENFORCED

- **Surface:** khala-code-desktop (plans and billing settings)
- **Stated by:** owner via github-issue on 2026-07-04
- **Statement:** Khala Code desktop plans panel hands off to real checkout (RL-4) with honest not-purchasable state while unarmed; credit packages (BF-2.4 tiers) purchasable from same surface via existing web checkout handoff. Post-purchase state (plan/entitlement/credits) renders from server truth via existing RPCs — never fabricated client-side.
- **Enforcement tier:** test-sweep
- **Oracle** `plans_checkout_handoff.dom` (bun-test, dom): Mounts the real plans panel in a DOM: while the paid-plan seam is unarmed the purchase control is disabled, an armed Stripe payment_required response opens exactly the server-returned checkout URL and re-reads status, and the same surface opens the existing /billing checkout for credits without rendering local fake package or balance state. — `clients/khala-code-desktop/tests/plans-panel.test.ts`
- **Oracle** `plan_purchase_payment_required_rpc.unit` (bun-test, unit): Decodes the plan-purchase RPC's Stripe payment_required response as a checkout handoff and asserts it does not contain a receiptRef or entitlementRef until the server returns a fulfilled receipt. — `clients/khala-code-desktop/tests/rpc-handlers.test.ts`
- **Verification:** bun test clients/khala-code-desktop/tests/plans-panel.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/ux-contracts.test.ts; these files run in the package test glob and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds the desktop settings payment surface only. Plan catalog, entitlement, and purchase settlement remain owned by openagents.com plan APIs; credit package catalog, balance, and checkout fulfillment remain owned by the existing web billing surface. The desktop may open those checkout URLs, but it must not synthesize paid entitlement or credit balance state.

### `khala_code.settings.editable_not_env_var_only.v1` — ENFORCED

- **Surface:** khala-code-desktop (settings)
- **Stated by:** TheBenMeadows (community; relayed via Lathe operator agent issues/PR) via community-feedback-discord on 2026-07-03
- **Statement:** Read-only Codex config metrics that reflect a genuinely configurable value (model provider, approval policy, sandbox mode, reasoning summary, verbosity) are editable from the settings UI itself, reusing the existing config-value write RPC, rather than requiring the user to edit an external environment variable or config file.
- **Enforcement tier:** test-sweep
- **Oracle** `config_enum_selects.dom` (bun-test, dom): Mounts the real Codex settings panel and proves the enum-backed Summary, Verbosity, Approval, and Sandbox controls write through the existing codexConfigValueWrite RPC path. — `clients/khala-code-desktop/tests/codex-settings-panel.test.ts`
- **Oracle** `provider_select_from_model_list.dom` (bun-test, dom): Mounts the real Codex settings panel with provider options sourced from model/list and proves the Provider select writes model_provider through codexConfigValueWrite, including clearing back to Default. — `clients/khala-code-desktop/tests/codex-settings-panel.test.ts`
- **Verification:** Enforced 2026-07-04: fixed for GitHub issue #8254. bun test tests/codex-settings-panel.test.ts tests/codex-settings.test.ts tests/rpc-handlers.test.ts tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Binds settings-panel editability for values that are structurally configurable (Codex/Claude config keys); does not apply to values that are genuinely environment-only by design (e.g. secrets that must never be typed into the UI), which should instead say so honestly per khala_code.settings.no_bare_unset_labels.v1.

### `khala_code.chat.khala_lane_connect_button.v1` — ENFORCED

- **Surface:** khala-code-desktop (Khala lane)
- **Stated by:** TheBenMeadows (community; relayed via Lathe operator agent issues/PR) via community-feedback-discord on 2026-07-03
- **Statement:** When the Khala lane is unavailable because the desktop process has no OPENAGENTS_AGENT_TOKEN, the lane offers a 'Connect' button that drives an in-app flow to obtain and persist a token, instead of only explaining the missing environment variable in text.
- **Enforcement tier:** test-sweep
- **Oracle** `desktop_auth_device_route.unit` (bun-test, unit): Exercises the Worker desktop auth device route: start creates a short-code verification link without an agent token, browser verify mints and links an agent credential, and poll requires the short-lived secret before returning the raw token once to the desktop client. — `apps/openagents.com/workers/api/src/khala-code-openagents-auth-routes.test.ts`
- **Oracle** `desktop_auth_rpc_persistence.unit` (bun-test, unit): Exercises the desktop RPC persistence path: persisted tokens satisfy plan status, start saves only a pending attempt to the settings file, and poll persists the linked token while returning only its prefix to the renderer. — `clients/khala-code-desktop/tests/rpc-handlers.test.ts`
- **Oracle** `missing_token_connect_panel.source` (bun-test, unit): Static shell guard proving the missing-token transcript path renders the OpenAgents Connect panel and wires start/poll/open-link RPC methods instead of remaining a plain text banner. — `clients/khala-code-desktop/tests/app-shell.test.ts`
- **Verification:** Enforced 2026-07-04 for GitHub issue #8255: Worker route tests cover the browser-verified token mint/poll flow; desktop RPC tests cover local pending-attempt/token persistence and persisted-token use for hosted plan status; app-shell tests cover the inline missing-token Connect panel. Runs in the package test glob, the Worker API test sweep, and the deploy check before pushes to main.
- **Authority boundary:** Binds the Khala lane missing-token recovery path: the browser-verified device flow may mint a linked OpenAgents agent token, and the desktop may persist that token locally for hosted Khala. It does not touch the default Codex home, grant provider-account authority, publish the raw token in renderer UI, or authorize any promise-state/payment/payout change.

### `khala_code.fleet.khala_sync_indicator_truthful.v1` — ENFORCED

- **Surface:** khala-code-desktop (fleet cockpit)
- **Stated by:** owner via issue on 2026-07-04
- **Statement:** Synced fleet indicators reflect server truth: the Fleet screen may claim live freshness only while the Khala Sync live socket is open, and any other sync state is shown as an explicit syncing or reconnecting state — never fake freshness.
- **Enforcement tier:** test-sweep
- **Oracle** `khala_sync_indicator_truthful.dom` (bun-test, dom): Mounts the real Fleet panel in a DOM with a fake Khala Sync source and proves the indicator renders 'Live' ONLY when the sync session's phase is live (open live socket); bootstrapping/catching_up/must_refetch/idle phases render explicit syncing, resyncing, or reconnecting labels and never the live marker. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Oracle** `khala_sync_phase_is_session_truth.service` (bun-test, unit): Drives the real desktop Khala Sync service over a deterministic fake transport and proves the RPC-exposed phase is the session's real scope state: live only after bootstrap+catch-up complete and the live socket is open. — `clients/khala-code-desktop/tests/khala-sync-service.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts tests/khala-sync-service.test.ts tests/fleet-status.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main. Live end-to-end verification against the deployed sync routes is tracked on epic #8282.
- **Authority boundary:** Binds the Fleet screen's Khala Sync freshness indicator semantics only (KS-6.2, #8303). It does not claim delivery latency, server availability, or correctness of the fleet projection itself. The Fleet cockpit is Khala Sync-first by default; local status/list reads are degraded fallback state for missing auth, disconnected sync, or explicit `KHALA_SYNC_FLEET=0`/`false`/`off` opt-out.

### `khala_code.fleet.khala_sync_must_refetch_recovers.v1` — ENFORCED

- **Surface:** khala-code-desktop (fleet cockpit)
- **Stated by:** owner via issue on 2026-07-04
- **Statement:** MustRefetch never strands the Fleet screen: the sync client re-bootstraps automatically and the screen shows a visible re-sync state until live truth returns.
- **Enforcement tier:** test-sweep
- **Oracle** `khala_sync_must_refetch_rebootstraps.service` (bun-test, unit): Drives the real desktop Khala Sync service over a fake transport, emits a MustRefetch frame after a server-side scope reset, and proves the session re-bootstraps automatically: the RPC view converges on the replaced scope content and returns to live without any manual recovery call. — `clients/khala-code-desktop/tests/khala-sync-service.test.ts`
- **Oracle** `khala_sync_must_refetch_visible.dom` (bun-test, dom): Mounts the real Fleet panel in a DOM with the sync source in must_refetch and proves the screen stays populated (polling fallback data still renders) with a visible 'Resyncing' indicator instead of a stranded or empty state. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts tests/khala-sync-service.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main. Live end-to-end verification against the deployed sync routes is tracked on epic #8282.
- **Authority boundary:** Binds the Fleet screen's recovery behavior on a MustRefetch signal (KS-6.2, #8303): automatic re-bootstrap with a visible re-sync state. It does not bind bootstrap retry budgets or server compaction policy, which stay owned by the khala-sync client/server packages and docs/khala-sync/SPEC.md.
