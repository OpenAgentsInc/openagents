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
        "packages/khala-qa-harness/src/seed-corpus.ts",
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
            "Mounts the transcript-level status renderer in a DOM: cache-miss thread switches render a polite 'Loading messages' transcript bubble instead of a sidebar spinner, and assistant thinking uses the same status-bubble structure.",
          id: "transcript_loading.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
        {
          description:
            "Runs the seed-corpus fixture RPC-driver scenario that lists threads, selects the fixture thread, and reads it back without using the row streaming spinner as load state.",
          id: "thread_select_fixture_rpc.scenario",
          kind: "qa-scenario",
          mode: "rpc",
          ref: "scenario.khala_code.seed.rpc_thread_select_fixture_driver.v1",
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
        "clients/khala-code-desktop/src/ui/recent-thread-hotkey-hints.ts",
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
            "Mounts the real thread sidebar and hotkey-hint listener in a DOM: enabling hotkey hints replaces the time slot of the nine most recent chats with their command-digit hints in place (no separate pane appears anywhere in the document), and Meta release or window blur restores the timestamps.",
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
    {
      authorityBoundary:
        "Binds duplicate visible assistant text for one Claude turn only. It does not change Claude SDK event ordering, token accounting, or the Codex lane projector.",
      blockerRefs: [],
      contractId: "khala_code.transcript.claude_assistant_turn_once.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8231",
        "clients/khala-code-desktop/src/bun/claude-thread-item-projector.ts",
        "clients/khala-code-desktop/tests/claude-app-sdk-chat-runtime.test.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Feeds the Claude projector a streamed assistant text block followed by the final assistant snapshot with the same body, and asserts the transcript keeps exactly one assistant message.",
          id: "claude_stream_final_snapshot_dedupe.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/claude-app-sdk-chat-runtime.test.ts",
        },
      ],
      productArea: "chat transcript",
      source: {
        channel: "github-issue",
        statedBy: "operator-agent",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "An assistant turn can render twice in the transcript — the same reply body appears as two consecutive assistant blocks for a single user turn. Observed on the Claude lane.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/claude-app-sdk-chat-runtime.test.ts tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "This binds control liveness only, not the exact reasoning-mode UI; that design is free to iterate as long as no control ships inert.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.composer.no_dead_controls.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/main.ts",
        "packages/ui/src/ai-elements/command-composer",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat composer",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "pending",
      statement:
        "Every composer control must visibly do something when interacted with. The 'Plan' toggle did nothing and confused the user about its purpose; it must be removed and replaced with a working reasoning-mode control that actually changes behavior.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Binds the attach control and queued-message rendering only; other composer chrome is out of scope for this contract.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.composer.attach_control_icon_only.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "packages/ui/src/ai-elements/command-composer",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat composer",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "The composer's attach control renders as an icon only, never the text label 'Attach'. Queued/follow-up messages (sent while a turn is still streaming) render in a compact, visually distinct style from a normal message, not as another full-size bubble.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Structure/liveness only \u2014 exact visual treatment is free to iterate with impeccable-skill review as long as inert chrome does not ship.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.composer.structure_not_bloat.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "packages/ui/src/ai-elements/command-composer",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat composer",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "pending",
      statement:
        "The composer stays compact, not oversized, and follows the StarCraft design system. Every visible composer icon must be functional; secondary controls that don't yet do anything real (e.g. mic, extra progress indicators, unused model dropdowns) stay hidden or removed rather than shipped as inert chrome the user has to look at.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Complements khala_code.chat.sidebar_active_thread_background_only.v2 (steady-state rendering) by additionally covering transient/mount-time flashes; the two contracts should be verified together.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.chat.no_current_chat_text_flash.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "pending",
      statement:
        "The active chat row must never flash a 'Current chat' text label anywhere in the sidebar, even momentarily during mount or a state transition. The active-row background alone (khala_code.chat.sidebar_active_thread_background_only.v2) is the only active indicator; no text heading or copy may appear, not even transiently.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Harness identity may still be surfaced elsewhere (e.g. a settings/detail view); this contract binds only the sidebar row itself.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.chat.harness_badge_removed.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "pending",
      statement:
        "Sidebar chat rows do not render a Codex/Claude harness-provider text badge next to the title. An earlier version showed this badge with stale/inaccurate values (older threads all labeled 'Claude' regardless of actual harness); the badge is removed entirely rather than kept and fixed.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Visual density only; does not change row content or interaction behavior contracted elsewhere.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.chat.sidebar_row_density.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/styles.css",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "pending",
      statement:
        "Sidebar chat rows render borderless, differentiated by background color only (no per-row border chrome), with tightened vertical padding between rows, matching the density of the reference Codex-desktop-style sidebar.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "This is a recurring defect reported repeatedly across 2026-07-02 and 2026-07-03; any fix must be verified against every reported recurrence, not just the most recent report.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.chat.thread_open_never_raw_error.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/thread-time.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "pending",
      statement:
        "Opening any thread from the sidebar must never surface a raw internal error string (e.g. 'no rollout found', 'invalid session id: invalid character ...') to the user. On a genuinely missing or corrupt session, show one typed, friendly, actionable message instead. Thread timestamps must never all collapse to showing 'now' when this happens.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Binds indicator truthfulness during navigation; does not change how many concurrent streams the app supports.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.chat.streaming_indicator_survives_navigation.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "pending",
      statement:
        "A background thread's streaming indicator must keep reflecting its real state even after the user switches to a different chat or starts a new one. Navigating away must never clear another thread's in-progress indicator, and reopening that thread later must still show it as streaming if it genuinely is.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Binds sidebar/list promptness and initial message rendering only.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.chat.new_thread_appears_promptly.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "A freshly created chat appears in the sidebar without delay, and its own first messages render immediately rather than starting blank while data that should already be present loads again.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Binds the rename affordance's visible result only, not its persistence/sync mechanics.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.chat.rename_applies_immediately.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "Confirming a thread rename (the check-mark action on the inline rename control) updates the visible sidebar title immediately, without requiring a refresh or a subsequent click.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Binds transcript completeness on rehydrate; does not require re-executing any tool.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.chat.rehydrate_shows_tool_calls.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/main.ts",
        "clients/khala-code-desktop/src/ui/transcript-render.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat transcript",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "Reopening or resuming an older thread renders its historical tool calls in the transcript, not just its text messages. The full turn history, including tool activity, must be reconstructible from a rehydrated session.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Theme parity only; does not change scroll behavior or keyboard/wheel handling contracted elsewhere.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.chat.starcraft_scrollbar_parity.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/styles.css",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "visual theme",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "The custom StarCraft-themed scrollbar used on openagents.com applies inside Khala Code desktop's scrollable surfaces too (sidebar, transcript, any other scrollable panel), not only the website.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Applies to runs of consecutive tool calls only; a tool call interleaved with an assistant message is not collapsed into an adjacent group.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.transcript.consecutive_tool_calls_collapsed.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/transcript-render.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat transcript",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "pending",
      statement:
        "Consecutive tool calls in the transcript collapse into a single line showing the latest call. Clicking that line expands it to reveal the full list of collapsed calls, and each item in that list can be further clicked to see its own detail.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Binds display formatting only; the underlying tool-call data may still carry the absolute path internally.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.transcript.tool_call_path_display.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/transcript-render.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat transcript",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "Tool-call summaries in the transcript show a workspace-relative path, never the absolute filesystem path or worktree prefix. Each summary is a short verb-prefixed label (e.g. 'Read ___', 'Edited ___') immediately beside the tool icon; status is conveyed by icon/color rather than a fully spelled-out word like 'Completed'.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "This is the cross-surface consistency category from the customer behavior-contract catalog applied to our own product first.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.transcript.streaming_state_cross_surface_consistency.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "chat transcript",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "The sidebar's streaming indicator and the composer's own status readout for the active thread must always agree. It must never be possible for the sidebar to show a thread as streaming while the composer for that same thread simultaneously shows 'ready' (or vice versa).",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Binds the hotbar button surface only, not the underlying route model.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.nav.hotbar_no_route_text.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/sidebar.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "app navigation",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "pending",
      statement:
        "The app-section nav hotbar (fleet/chat/forum/inbox/settings) shows icon plus hotkey only. It must never render a raw route or path fragment as visible text on a hotbar button.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "This is the app-section hotbar (Option+digit); it is distinct from khala_code.chat.recent_thread_cmd_hotkeys.v2's Cmd-hold recent-chat hints, which live in the sidebar rows instead of on hotbar buttons.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.nav.hotbar_hotkey_always_visible.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/sidebar.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "app navigation",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "Every nav hotbar button always displays its own trigger hotkey as a small visible badge (e.g. '\u23251'), not just discoverable by trial. Pressing the displayed modifier-plus-digit combination for a button always routes to that section.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Binds keyboard interception correctness only.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.nav.hotbar_no_stray_special_characters.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/sidebar.ts",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "app navigation",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "pending",
      statement:
        "The Option+digit hotbar shortcut must always be intercepted as a navigation command and must never leak macOS's special/garbled Option-key characters (e.g. \u00a1\u2122\u00a3\u00a2) into any input field or onto the page.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Applies to all flyout/context menus app-wide, including the thread-action menu and the fleet menu.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.menus.flyout_single_line_no_preamble.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "packages/ui/src/menu-dom.ts",
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "app-wide menus",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "Right-click and flyout menus render one line per item, with no explanatory subheadline text under each item and no header/preamble content above the options.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Narrow scope: this contract targets literal leaked internal labels, not the fleet menu's overall information architecture.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.fleet.menu_no_stray_labels.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/fleet-panel.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "fleet panel",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "The fleet menu must not render stray internal label text (e.g. a literal 'ACCT' tag) that is not part of a designed, human-readable element.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Binds resumability of in-flight work; does not require preserving unsent composer drafts unless separately contracted.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.app.resumes_after_restart.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/main.ts",
        "clients/khala-code-desktop/src/bun/index.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "app lifecycle",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "When the app restarts, whether voluntary or due to a crash/relaunch, any work that was in flight resumes rather than silently stopping. The user should not have to notice and manually recover in-progress state after a restart.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Binds first-launch behavior for currently-disabled features; does not block future opt-in enablement flows.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.app.no_unrequested_first_launch_scripts.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/bun/index.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "app lifecycle",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "Features that have not been enabled (e.g. Apple Bridge) must not run any preparation or background script on first launch. A disabled feature stays fully inert until explicitly turned on.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
    {
      authorityBoundary:
        "Display-only claim; does not change the exact-only token accounting invariants (usage_truth='exact', reconciliation against token_usage_events) owned elsewhere.",
      blockerRefs: ["blocker.khala_code_ux_mining.oracle_not_implemented_20260703"],
      contractId: "khala_code.tokens.per_thread_live_counter.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "token accounting",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "pending",
      statement:
        "A live per-thread token counter is visible in the top-right of the Khala Code screen while a thread is active, updating as tokens accrue. Clicking it shows how many of those tokens have synced to the public leaderboard.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: mined from Codex/Claude conversation history on 2026-07-03 (docs-only pass per owner directive) and recorded pending an oracle in the follow-up code pass. No test currently guards this statement.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-03.6",
}
