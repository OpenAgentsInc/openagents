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

## Registry

Registry version: `2026-07-03.1` (schema `openagents.behavior_contracts.v1`)

### `khala_code.chat.sidebar_spinner_streaming_only.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread sidebar)
- **Stated by:** owner via khala-code-session on 2026-07-03
- **Statement:** The spinner on a chat row in the thread sidebar means an assistant response is streaming in that chat, and nothing else. Clicking a chat must not show that spinner while its messages load; message-loading indication belongs in the chat transcript itself.
- **Enforcement tier:** test-sweep
- **Oracle** `sidebar_spinner.dom` (bun-test, dom): Mounts the real thread sidebar in a DOM: selecting a thread while the resume RPC is in flight renders no spinner anywhere in the list, while a genuinely streaming thread renders the spinner in its time slot. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Oracle** `transcript_loading.source` (bun-test, unit): Pins the transcript-level 'Loading messages' indicator wiring for cache-miss thread switches (source-level until the full shell boots under the DOM harness). — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** This contract binds indicator semantics only. Thread-switch latency budgets stay owned by docs/qa/khala-code-latency-budgets.md, and it makes no claim about streaming correctness itself.

### `khala_code.chat.recent_thread_cmd_hotkeys.v1` — ENFORCED

- **Surface:** khala-code-desktop (chat thread switching)
- **Stated by:** owner via khala-code-session on 2026-07-03
- **Statement:** Holding Cmd shows an overlay listing the nine most recent chats numbered 1 through 9, and pressing Cmd+1 through Cmd+9 jumps to that chat. Releasing Cmd hides the overlay.
- **Enforcement tier:** test-sweep
- **Oracle** `cmd_digit_gating.unit` (bun-test, unit): Cmd+1..Cmd+9 map to the first through ninth most recent threads; unmodified digits and digits with other modifiers map to nothing. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Oracle** `hold_overlay.dom` (bun-test, dom): Mounts the recent-chats overlay in a DOM: holding Meta shows the numbered list of at most nine recent chats with the active chat highlighted, releasing Meta hides it, and clicking an entry selects that chat. — `clients/khala-code-desktop/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.
- **Authority boundary:** Cmd+0 additionally maps to the tenth most recent chat and Cmd+ArrowUp/ArrowDown cycle through recency; those are compatible extensions, not part of this contract.
