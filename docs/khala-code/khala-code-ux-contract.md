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

Registry version: `2026-07-03.6` (schema `openagents.behavior_contracts.v1`)

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

### `khala_code.composer.no_dead_controls.v1` — PENDING

- **Surface:** khala-code-desktop (chat composer)
- **Stated by:** owner via codex-session on 2026-07-03
- **Statement:** Every composer control must visibly do something when interacted with. The 'Plan' toggle did nothing and confused the user about its purpose; it must be removed and replaced with a working reasoning-mode control that actually changes behavior.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** This binds control liveness only, not the exact reasoning-mode UI; that design is free to iterate as long as no control ships inert.

### `khala_code.composer.attach_control_icon_only.v1` — PENDING

- **Surface:** khala-code-desktop (chat composer)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** The composer's attach control renders as an icon only, never the text label 'Attach'. Queued/follow-up messages (sent while a turn is still streaming) render in a compact, visually distinct style from a normal message, not as another full-size bubble.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Binds the attach control and queued-message rendering only; other composer chrome is out of scope for this contract.

### `khala_code.composer.structure_not_bloat.v1` — PENDING

- **Surface:** khala-code-desktop (chat composer)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** The composer stays compact, not oversized, and follows the StarCraft design system. Every visible composer icon must be functional; secondary controls that don't yet do anything real (e.g. mic, extra progress indicators, unused model dropdowns) stay hidden or removed rather than shipped as inert chrome the user has to look at.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Structure/liveness only — exact visual treatment is free to iterate with impeccable-skill review as long as inert chrome does not ship.

### `khala_code.chat.no_current_chat_text_flash.v1` — PENDING

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-03
- **Statement:** The active chat row must never flash a 'Current chat' text label anywhere in the sidebar, even momentarily during mount or a state transition. The active-row background alone (khala_code.chat.sidebar_active_thread_background_only.v2) is the only active indicator; no text heading or copy may appear, not even transiently.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Complements khala_code.chat.sidebar_active_thread_background_only.v2 (steady-state rendering) by additionally covering transient/mount-time flashes; the two contracts should be verified together.

### `khala_code.chat.harness_badge_removed.v1` — PENDING

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** Sidebar chat rows do not render a Codex/Claude harness-provider text badge next to the title. An earlier version showed this badge with stale/inaccurate values (older threads all labeled 'Claude' regardless of actual harness); the badge is removed entirely rather than kept and fixed.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Harness identity may still be surfaced elsewhere (e.g. a settings/detail view); this contract binds only the sidebar row itself.

### `khala_code.chat.sidebar_row_density.v1` — PENDING

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** Sidebar chat rows render borderless, differentiated by background color only (no per-row border chrome), with tightened vertical padding between rows, matching the density of the reference Codex-desktop-style sidebar.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Visual density only; does not change row content or interaction behavior contracted elsewhere.

### `khala_code.chat.thread_open_never_raw_error.v1` — PENDING

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-03
- **Statement:** Opening any thread from the sidebar must never surface a raw internal error string (e.g. 'no rollout found', 'invalid session id: invalid character ...') to the user. On a genuinely missing or corrupt session, show one typed, friendly, actionable message instead. Thread timestamps must never all collapse to showing 'now' when this happens.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** This is a recurring defect reported repeatedly across 2026-07-02 and 2026-07-03; any fix must be verified against every reported recurrence, not just the most recent report.

### `khala_code.chat.streaming_indicator_survives_navigation.v1` — PENDING

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** A background thread's streaming indicator must keep reflecting its real state even after the user switches to a different chat or starts a new one. Navigating away must never clear another thread's in-progress indicator, and reopening that thread later must still show it as streaming if it genuinely is.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Binds indicator truthfulness during navigation; does not change how many concurrent streams the app supports.

### `khala_code.chat.new_thread_appears_promptly.v1` — PENDING

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** A freshly created chat appears in the sidebar without delay, and its own first messages render immediately rather than starting blank while data that should already be present loads again.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Binds sidebar/list promptness and initial message rendering only.

### `khala_code.chat.rename_applies_immediately.v1` — PENDING

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** Confirming a thread rename (the check-mark action on the inline rename control) updates the visible sidebar title immediately, without requiring a refresh or a subsequent click.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Binds the rename affordance's visible result only, not its persistence/sync mechanics.

### `khala_code.chat.rehydrate_shows_tool_calls.v1` — PENDING

- **Surface:** khala-code-desktop (chat transcript)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** Reopening or resuming an older thread renders its historical tool calls in the transcript, not just its text messages. The full turn history, including tool activity, must be reconstructible from a rehydrated session.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Binds transcript completeness on rehydrate; does not require re-executing any tool.

### `khala_code.chat.starcraft_scrollbar_parity.v1` — PENDING

- **Surface:** khala-code-desktop (visual theme)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** The custom StarCraft-themed scrollbar used on openagents.com applies inside Khala Code desktop's scrollable surfaces too (sidebar, transcript, any other scrollable panel), not only the website.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Theme parity only; does not change scroll behavior or keyboard/wheel handling contracted elsewhere.

### `khala_code.transcript.consecutive_tool_calls_collapsed.v1` — PENDING

- **Surface:** khala-code-desktop (chat transcript)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** Consecutive tool calls in the transcript collapse into a single line showing the latest call. Clicking that line expands it to reveal the full list of collapsed calls, and each item in that list can be further clicked to see its own detail.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Applies to runs of consecutive tool calls only; a tool call interleaved with an assistant message is not collapsed into an adjacent group.

### `khala_code.transcript.tool_call_path_display.v1` — PENDING

- **Surface:** khala-code-desktop (chat transcript)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** Tool-call summaries in the transcript show a workspace-relative path, never the absolute filesystem path or worktree prefix. Each summary is a short verb-prefixed label (e.g. 'Read ___', 'Edited ___') immediately beside the tool icon; status is conveyed by icon/color rather than a fully spelled-out word like 'Completed'.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Binds display formatting only; the underlying tool-call data may still carry the absolute path internally.

### `khala_code.transcript.streaming_state_cross_surface_consistency.v1` — PENDING

- **Surface:** khala-code-desktop (chat transcript)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** The sidebar's streaming indicator and the composer's own status readout for the active thread must always agree. It must never be possible for the sidebar to show a thread as streaming while the composer for that same thread simultaneously shows 'ready' (or vice versa).
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** This is the cross-surface consistency category from the customer behavior-contract catalog applied to our own product first.

### `khala_code.nav.hotbar_no_route_text.v1` — PENDING

- **Surface:** khala-code-desktop (app navigation)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** The app-section nav hotbar (fleet/chat/forum/inbox/settings) shows icon plus hotkey only. It must never render a raw route or path fragment as visible text on a hotbar button.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Binds the hotbar button surface only, not the underlying route model.

### `khala_code.nav.hotbar_hotkey_always_visible.v1` — PENDING

- **Surface:** khala-code-desktop (app navigation)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** Every nav hotbar button always displays its own trigger hotkey as a small visible badge (e.g. '⌥1'), not just discoverable by trial. Pressing the displayed modifier-plus-digit combination for a button always routes to that section.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** This is the app-section hotbar (Option+digit); it is distinct from khala_code.chat.recent_thread_cmd_hotkeys.v2's Cmd-hold recent-chat hints, which live in the sidebar rows instead of on hotbar buttons.

### `khala_code.nav.hotbar_no_stray_special_characters.v1` — PENDING

- **Surface:** khala-code-desktop (app navigation)
- **Stated by:** owner via codex-session on 2026-07-02
- **Statement:** The Option+digit hotbar shortcut must always be intercepted as a navigation command and must never leak macOS's special/garbled Option-key characters (e.g. ¡™£¢) into any input field or onto the page.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Binds keyboard interception correctness only.

### `khala_code.menus.flyout_single_line_no_preamble.v1` — PENDING

- **Surface:** khala-code-desktop (app-wide menus)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** Right-click and flyout menus render one line per item, with no explanatory subheadline text under each item and no header/preamble content above the options.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Applies to all flyout/context menus app-wide, including the thread-action menu and the fleet menu.

### `khala_code.fleet.menu_no_stray_labels.v1` — PENDING

- **Surface:** khala-code-desktop (fleet panel)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** The fleet menu must not render stray internal label text (e.g. a literal 'ACCT' tag) that is not part of a designed, human-readable element.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Narrow scope: this contract targets literal leaked internal labels, not the fleet menu's overall information architecture.

### `khala_code.app.resumes_after_restart.v1` — PENDING

- **Surface:** khala-code-desktop (app lifecycle)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** When the app restarts, whether voluntary or due to a crash/relaunch, any work that was in flight resumes rather than silently stopping. The user should not have to notice and manually recover in-progress state after a restart.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Binds resumability of in-flight work; does not require preserving unsent composer drafts unless separately contracted.

### `khala_code.app.no_unrequested_first_launch_scripts.v1` — PENDING

- **Surface:** khala-code-desktop (app lifecycle)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** Features that have not been enabled (e.g. Apple Bridge) must not run any preparation or background script on first launch. A disabled feature stays fully inert until explicitly turned on.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Binds first-launch behavior for currently-disabled features; does not block future opt-in enablement flows.

### `khala_code.tokens.per_thread_live_counter.v1` — PENDING

- **Surface:** khala-code-desktop (token accounting)
- **Stated by:** owner via codex-session on 2026-07-01
- **Statement:** A live per-thread token counter is visible in the top-right of the Khala Code screen while a thread is active, updating as tokens accrue. Clicking it shows how many of those tokens have synced to the public leaderboard.
- **Enforcement tier:** unenforced
- **Verification:** Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.
- **Blockers:** `blocker.khala_code_ux_mining.oracle_not_implemented_20260703`
- **Authority boundary:** Display-only claim; does not change the exact-only token accounting invariants (usage_truth='exact', reconciliation against token_usage_events) owned elsewhere.
