import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts"

/**
 * Khala Code desktop UX behavior contracts.
 *
 * This registry is the durable home for owner-stated (and later
 * customer-stated) UX expectations. Every entry records the statement
 * verbatim, who stated it and where, and the oracle tests that enforce it in
 * the normal test sweep. The paired coverage test in
 * tests/ux-contracts.test.ts fails the sweep if an enforced contract loses
 * its oracle, so stated behavior cannot silently drift.
 *
 * Human rendering: docs/khala-code/khala-code-ux-contract.md (kept in sync by
 * the same test file).
 */
export const KHALA_CODE_UX_CONTRACT_DOC_PATH =
  "docs/khala-code/khala-code-ux-contract.md"

export const khalaCodeUxContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract binds indicator semantics only. Thread-switch latency budgets stay owned by docs/qa/khala-code-latency-budgets.md, and it makes no claim about streaming correctness itself.",
      blockerRefs: [],
      contractId: "khala_code.chat.sidebar_spinner_streaming_only.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Mounts the real thread sidebar in a DOM: selecting a thread while the resume RPC is in flight renders no spinner anywhere in the list, while a genuinely streaming thread renders the spinner in its time slot.",
          id: "sidebar_spinner.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
        {
          description:
            "Pins the transcript-level 'Loading messages' indicator wiring for cache-miss thread switches (source-level until the full shell boots under the DOM harness).",
          id: "transcript_loading.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat thread sidebar",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "The spinner on a chat row in the thread sidebar means an assistant response is streaming in that chat, and nothing else. Clicking a chat must not show that spinner while its messages load; message-loading indication belongs in the chat transcript itself.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Cmd+0 additionally maps to the tenth most recent chat and Cmd+ArrowUp/ArrowDown cycle through recency; those are compatible extensions, not part of this contract.",
      blockerRefs: [],
      contractId: "khala_code.chat.recent_thread_cmd_hotkeys.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/thread-hotkeys.ts",
        "clients/khala-code-desktop/src/ui/recent-thread-overlay.ts",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Cmd+1..Cmd+9 map to the first through ninth most recent threads; unmodified digits and digits with other modifiers map to nothing.",
          id: "cmd_digit_gating.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
        {
          description:
            "Mounts the recent-chats overlay in a DOM: holding Meta shows the numbered list of at most nine recent chats with the active chat highlighted, releasing Meta hides it, and clicking an entry selects that chat.",
          id: "hold_overlay.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat thread switching",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "Holding Cmd shows an overlay listing the nine most recent chats numbered 1 through 9, and pressing Cmd+1 through Cmd+9 jumps to that chat. Releasing Cmd hides the overlay.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-03.1",
}
