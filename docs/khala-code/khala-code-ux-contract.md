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
  file exists and references its contract id.
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

Registry version: `2026-07-03.4` (schema `openagents.behavior_contracts.v1`)

### `khala_code.chat.sidebar_spinner_streaming_only.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via khala-code-session on 2026-07-03
- **Statement:** The spinner on a chat row in the thread sidebar means an assistant response is streaming in that chat, and nothing else. Clicking a chat must not show that spinner while its messages load; message-loading indication belongs in the chat transcript itself.
- **Enforcement tier:** test-sweep
- **Oracle** `sidebar_spinner.dom` (bun-test, dom): Mounts the real thread sidebar in a DOM: selecting a thread while the resume RPC is in flight renders no spinner anywhere in the list, while a genuinely streaming thread renders the spinner in its time slot. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Oracle** `transcript_loading.source` (bun-test, unit): Pins the transcript-level 'Loading messages' indicator wiring for cache-miss thread switches (source-level until the full shell boots under the DOM harness). — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
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
- **Oracle** `sidebar_hotkey_hints.dom` (bun-test, dom): Mounts the real thread sidebar in a DOM: enabling hotkey hints replaces the time slot of the nine most recent chats with their command-digit hints in place (no separate pane appears anywhere in the document), and disabling hints restores the timestamps. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Cmd+0 additionally maps to the tenth most recent chat and Cmd+ArrowUp/ArrowDown cycle through recency; those are compatible extensions, not part of this contract. The generalized overlay-menu component remains available for future dialog menus but is not mounted for this feature.
