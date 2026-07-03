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
        "Retired 2026-07-03 (owner restatement): the highlight existed as a hook but the tone had drifted to a near-invisible surface mix. Superseded by khala_code.chat.sidebar_active_thread_background_only.v2, which additionally pins noticeability; kept for history.",
      blockerRefs: [],
      contractId: "khala_code.chat.sidebar_active_thread_background_only.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "contract:khala_code.chat.sidebar_active_thread_background_only.v2",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat thread sidebar",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "retired",
      statement:
        "Current chat is just supposed to be highlighted as a background bar.",
      surface: "khala-code-desktop",
      verification:
        "Superseded by khala_code.chat.sidebar_active_thread_background_only.v2.",
    },
    {
      authorityBoundary:
        "The exact tone may be tuned with owner sign-off, but the active row must stay visibly distinct from the hover, selecting, and plain-row tones; reverting it to the sidebar's base surface mix (the drift that made it invisible) is a contract violation, not a style tweak. Persisted project, Codex, Claude, and session-catalog group labels remain owned by their source catalogs.",
      blockerRefs: [],
      contractId: "khala_code.chat.sidebar_active_thread_background_only.v2",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/styles.css",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Mounts the real thread sidebar in a DOM: an optimistic current chat renders as the active row with the active background hooks and no visible 'Current chat' heading or copy.",
          id: "active_thread_background_only.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
        {
          description:
            "Pins the active-row background to a distinct energy-blue tone: the [data-active=\"true\"] rule must use the khala-energy-blue accent (not the surface mix shared with hover/selecting rows), so the highlight cannot silently fade back into the sidebar background. Rendered appearance is additionally covered by the visual smoke tier.",
          id: "active_row_distinct_tone.source",
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
        "The active chat in the sidebar must have a noticeable background color — not very bright, but clearly visible — so it is always obvious which chat is the active one. It renders as a background bar only, with no 'Current chat' heading or copy, and it must not fade into the sidebar background or disappear.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Retired 2026-07-03 (owner correction): the overlay reading of the original ask was wrong — no separate pane should appear. Superseded by khala_code.chat.recent_thread_cmd_hotkeys.v2; kept for history.",
      blockerRefs: [],
      contractId: "khala_code.chat.recent_thread_cmd_hotkeys.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "contract:khala_code.chat.recent_thread_cmd_hotkeys.v2",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat thread switching",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "retired",
      statement:
        "Holding Cmd shows an overlay listing the nine most recent chats numbered 1 through 9, and pressing Cmd+1 through Cmd+9 jumps to that chat. Releasing Cmd hides the overlay.",
      surface: "khala-code-desktop",
      verification:
        "Superseded by khala_code.chat.recent_thread_cmd_hotkeys.v2.",
    },
    {
      authorityBoundary:
        "Cmd+0 additionally maps to the tenth most recent chat and Cmd+ArrowUp/ArrowDown cycle through recency; those are compatible extensions, not part of this contract. The generalized overlay-menu component remains available for future dialog menus but is not mounted for this feature.",
      blockerRefs: [],
      contractId: "khala_code.chat.recent_thread_cmd_hotkeys.v2",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/thread-hotkeys.ts",
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
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
            "Mounts the real thread sidebar in a DOM: enabling hotkey hints replaces the time slot of the nine most recent chats with their command-digit hints in place (no separate pane appears anywhere in the document), and disabling hints restores the timestamps.",
          id: "sidebar_hotkey_hints.dom",
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
        "Holding Cmd does not open a separate pane; it temporarily replaces the timestamps of the nine most recent chats in the sidebar with their command-digit hotkeys (⌘1 through ⌘9). Pressing Cmd+1 through Cmd+9 jumps to that chat, and releasing Cmd restores the timestamps.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-03.4",
}
