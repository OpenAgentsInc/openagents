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
        "This contract only governs sidebar behavior for stored/local Codex metadata that lacks a current app-server UUID thread id. It does not define the upstream Codex thread-store retention policy, subagent semantics, or whether historical rollouts can be recovered through a separate import flow.",
      blockerRefs: [],
      contractId: "khala_code.chat.codex_stored_session_records_not_resumed.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/shared/session-catalog.ts",
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/thread-hotkeys.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Projects a stored-only Codex catalog record with a legacy non-UUID ref into a disabled local-record sidebar summary instead of a resumable chat thread.",
          id: "stored_codex_catalog_projection.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
        {
          description:
            "Mounts the real thread sidebar in a DOM: a stored-only Codex record remains visible, is disabled, never displays a raw parser error, and recent-chat selection skips it.",
          id: "stored_codex_sidebar.dom",
          kind: "bun-test",
          mode: "dom",
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
        "Codex session rows must not show raw 'invalid session id' parser errors when the row only has stored local or legacy metadata, and they must not appear as normal resumable chats without a loaded title.",
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
        "This binds the desktop Claude chat lane only. Fleet worker Claude accounts remain owned by Pylon's isolated-account registry, and an explicit KHALA_CODE_DESKTOP_CLAUDE_CONFIG_DIR override may intentionally point the desktop lane at a caller-selected app config directory.",
      blockerRefs: [],
      contractId: "khala_code.claude_lane.isolated_home_and_user_prompt_only.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/bun/claude-app-sdk-chat-runtime.ts",
        "clients/khala-code-desktop/src/bun/claude-session-store.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Starts a real Claude runtime with an ambient CLAUDE_CONFIG_DIR and proves query() receives Khala Code's app-managed config directory instead of the user's default/global Claude home.",
          id: "claude_app_sdk_config_dir_isolated.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/claude-app-sdk-chat-runtime.test.ts",
        },
        {
          description:
            "Submits a transcript containing system, tool, assistant, and older user rows and proves the Claude SDK prompt contains only the latest user-authored message.",
          id: "claude_prompt_user_only.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/claude-app-sdk-chat-runtime.test.ts",
        },
      ],
      productArea: "Claude lane",
      source: {
        channel: "github-issue",
        statedBy: "customer",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "The Claude lane starts from a clean Khala Code context rather than the user's global Claude Code memory/config. Non-user transcript system/error text must not be fed to the model as conversation.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/claude-app-sdk-chat-runtime.test.ts tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "This contract binds the History/session-catalog default and its explicit opt-in only. It does not prevent app-owned sessions from being enriched with runtime metadata, and it does not promise that externally created home sessions can always be opened successfully.",
      blockerRefs: [],
      contractId: "khala_code.history.app_sessions_default.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/bun/session-catalog.ts",
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/tests/session-catalog.test.ts",
        "clients/khala-code-desktop/tests/codex-thread-sidebar.test.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Builds a mixed app-owned plus headless-runtime catalog and proves the default sessionCatalog scope includes only the app-owned desktop thread while omitting unrelated home/headless prompts.",
          id: "session_catalog_app_scope.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/session-catalog.test.ts",
        },
        {
          description:
            "Mounts the real History sidebar in a DOM and proves the header toggle is off by default, requests app-only history first, and sends includeHomeSessions only after explicit user activation.",
          id: "history_scope_toggle.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/codex-thread-sidebar.test.ts",
        },
        {
          description:
            "Mounts the real History sidebar in a DOM, forces a 'no rollout found' resume failure on a dead thread, and proves both the per-thread error row and the global error banner render a dismiss control that clears the error on click without triggering another listThreads fetch.",
          id: "history_error_dismiss.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/codex-thread-sidebar.test.ts",
        },
      ],
      productArea: "History sidebar",
      source: {
        channel: "github-issue",
        statedBy: "customer",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "History defaults to chats created in Khala Code Desktop, not every Codex or Claude session from the user's home stores. Showing all home sessions is an explicit opt-in, and stale missing-rollout rows must not permanently bury desktop chats as undismissable red errors.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/session-catalog.test.ts tests/codex-thread-sidebar.test.ts inside clients/khala-code-desktop; both run in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "This binds control liveness only, not the exact reasoning-mode UI; that design is free to iterate as long as no control ships inert.",
      blockerRefs: [],
      contractId: "khala_code.composer.no_dead_controls.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/main.ts",
        "packages/ui/src/ai-elements/command-composer",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Pins that the composer has a working reasoning-mode select wired to a real RPC and no lingering dead 'Plan' toggle in mounted code or active CSS.",
          id: "no_dead_controls.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat composer",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "Every composer control must visibly do something when interacted with. The 'Plan' toggle did nothing and confused the user about its purpose; it must be removed and replaced with a working reasoning-mode control that actually changes behavior.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the attach control and queued-message rendering only; other composer chrome is out of scope for this contract.",
      blockerRefs: [],
      contractId: "khala_code.composer.attach_control_icon_only.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/ui/src/ai-elements/command-composer",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Pins the desktop-scoped CSS override that keeps the attach control's text label hidden regardless of viewport width, and that queued follow-up messages render in a compact style distinct from a full message bubble.",
          id: "attach_icon_only.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat composer",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "The composer's attach control renders as an icon only, never the text label 'Attach'. Queued/follow-up messages (sent while a turn is still streaming) render in a compact, visually distinct style from a normal message, not as another full-size bubble.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Structure/liveness only \u2014 exact visual treatment is free to iterate with impeccable-skill review as long as inert chrome does not ship.",
      blockerRefs: [],
      contractId: "khala_code.composer.structure_not_bloat.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/ui/src/ai-elements/command-composer",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Pins that the composer has a working reasoning-mode select and no mic/runtime-badge/harness-pill chrome mounted or styled active.",
          id: "structure_not_bloat.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat composer",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "enforced",
      statement:
        "The composer stays compact, not oversized, and follows the StarCraft design system. Every visible composer icon must be functional; secondary controls that don't yet do anything real (e.g. mic, extra progress indicators, unused model dropdowns) stay hidden or removed rather than shipped as inert chrome the user has to look at.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Complements khala_code.chat.sidebar_active_thread_background_only.v2 (steady-state rendering) by additionally covering transient/mount-time flashes; the two contracts should be verified together.",
      blockerRefs: [],
      contractId: "khala_code.chat.no_current_chat_text_flash.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Asserts the sidebar source never contains the string 'current chat' anywhere, so no code path (including transient/mount-time ones) can render it.",
          id: "no_current_chat_text_flash.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "The active chat row must never flash a 'Current chat' text label anywhere in the sidebar, even momentarily during mount or a state transition. The active-row background alone (khala_code.chat.sidebar_active_thread_background_only.v2) is the only active indicator; no text heading or copy may appear, not even transiently.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Harness identity may still be surfaced elsewhere (e.g. a settings/detail view); this contract binds only the sidebar row itself.",
      blockerRefs: [],
      contractId: "khala_code.chat.harness_badge_removed.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Mounts the real thread sidebar with threads carrying Codex/Claude badges and asserts no harness badge element or text renders.",
          id: "harness_badge_removed.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "enforced",
      statement:
        "Sidebar chat rows do not render a Codex/Claude harness-provider text badge next to the title. An earlier version showed this badge with stale/inaccurate values (older threads all labeled 'Claude' regardless of actual harness); the badge is removed entirely rather than kept and fixed.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Visual density only; does not change row content or interaction behavior contracted elsewhere.",
      blockerRefs: [],
      contractId: "khala_code.chat.sidebar_row_density.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/styles.css",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Pins the sidebar row CSS to borderless with the tightened 0.1rem/0.5rem padding and confirms active/hover states never add a non-zero border.",
          id: "sidebar_row_density.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "enforced",
      statement:
        "Sidebar chat rows render borderless, differentiated by background color only (no per-row border chrome), with tightened vertical padding between rows, matching the density of the reference Codex-desktop-style sidebar.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "This is a recurring defect reported repeatedly across 2026-07-02 and 2026-07-03; any fix must be verified against every reported recurrence, not just the most recent report.",
      blockerRefs: [],
      contractId: "khala_code.chat.thread_open_never_raw_error.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/thread-time.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Unit-tests the internal-error detector and friendly-message mapper against real raw Codex RPC error strings, and confirms unrelated error text passes through unchanged.",
          id: "thread_open_error_mapping.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
        {
          description:
            "Mounts the real thread sidebar with a resumeThread that throws a raw internal error and asserts the rendered row shows the friendly message, never the raw text.",
          id: "thread_open_error_rendering.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "Opening any thread from the sidebar must never surface a raw internal error string (e.g. 'no rollout found', 'invalid session id: invalid character ...') to the user. On a genuinely missing or corrupt session, show one typed, friendly, actionable message instead. Thread timestamps must never all collapse to showing 'now' when this happens.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds indicator truthfulness during navigation; does not change how many concurrent streams the app supports.",
      blockerRefs: [],
      contractId: "khala_code.chat.streaming_indicator_survives_navigation.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Pins the per-thread streamingThreadIds tracking: populated at submit time by thread, only cleared when its own turn finishes, and never blanket-cleared by any thread-switch function.",
          id: "streaming_survives_navigation.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "enforced",
      statement:
        "A background thread's streaming indicator must keep reflecting its real state even after the user switches to a different chat or starts a new one. Navigating away must never clear another thread's in-progress indicator, and reopening that thread later must still show it as streaming if it genuinely is.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds sidebar/list promptness and initial message rendering only.",
      blockerRefs: [],
      contractId: "khala_code.chat.new_thread_appears_promptly.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Mounts the real thread sidebar and asserts an optimistically-inserted pending thread appears in the list immediately with its preview visible, with no RPC round trip required.",
          id: "new_thread_appears_promptly.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "A freshly created chat appears in the sidebar without delay, and its own first messages render immediately rather than starting blank while data that should already be present loads again.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the rename affordance's visible result only, not its persistence/sync mechanics.",
      blockerRefs: [],
      contractId: "khala_code.chat.rename_applies_immediately.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Mounts the real thread sidebar, drives the context-menu rename flow end to end, and asserts the visible title updates before the mocked rename RPC resolves.",
          id: "rename_applies_immediately.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat thread sidebar",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "Confirming a thread rename (the check-mark action on the inline rename control) updates the visible sidebar title immediately, without requiring a refresh or a subsequent click.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds transcript completeness on rehydrate; does not require re-executing any tool.",
      blockerRefs: [],
      contractId: "khala_code.chat.rehydrate_shows_tool_calls.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/main.ts",
        "clients/khala-code-desktop/src/ui/transcript-render.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "References the thread-history projector's full-variant coverage test and confirms messagesFromThread replays every item through the same projector used for live streaming.",
          id: "rehydrate_shows_tool_calls.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat transcript",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "Reopening or resuming an older thread renders its historical tool calls in the transcript, not just its text messages. The full turn history, including tool activity, must be reconstructible from a rehydrated session.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Theme parity only; does not change scroll behavior or keyboard/wheel handling contracted elsewhere.",
      blockerRefs: [],
      contractId: "khala_code.chat.starcraft_scrollbar_parity.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/styles.css",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Asserts the StarCraft scrollbar theme is declared with the universal selector (automatic parity for every scrollable surface) and that no container opts out.",
          id: "starcraft_scrollbar_parity.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "visual theme",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "The custom StarCraft-themed scrollbar used on openagents.com applies inside Khala Code desktop's scrollable surfaces too (sidebar, transcript, any other scrollable panel), not only the website.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Applies to runs of consecutive tool calls only; a tool call interleaved with an assistant message is not collapsed into an adjacent group.",
      blockerRefs: [],
      contractId: "khala_code.transcript.consecutive_tool_calls_collapsed.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/transcript-render.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Pins the grouping pass and collapsible summary renderer wired into the transcript render path, including the click-to-expand toggle and matching CSS.",
          id: "consecutive_tool_calls_collapsed.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat transcript",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "enforced",
      statement:
        "Consecutive tool calls in the transcript collapse into a single line showing the latest call. Clicking that line expands it to reveal the full list of collapsed calls, and each item in that list can be further clicked to see its own detail.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds display formatting only; the underlying tool-call data may still carry the absolute path internally.",
      blockerRefs: [],
      contractId: "khala_code.transcript.tool_call_path_display.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/transcript-render.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "References the projector's relative-path labeling test and confirms tool-call titles are built from the workspace-relative displayPath helper, never an absolute path.",
          id: "tool_call_path_display.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat transcript",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "Tool-call summaries in the transcript show a workspace-relative path, never the absolute filesystem path or worktree prefix. Each summary is a short verb-prefixed label (e.g. 'Read ___', 'Edited ___') immediately beside the tool icon; status is conveyed by icon/color rather than a fully spelled-out word like 'Completed'.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "This is the cross-surface consistency category from the customer behavior-contract catalog applied to our own product first.",
      blockerRefs: [],
      contractId: "khala_code.transcript.streaming_state_cross_surface_consistency.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Pins that the composer status and the sidebar streaming badge both derive from the same per-thread isThreadStreaming/streamingThreadIds source of truth, not independent flags that can disagree.",
          id: "streaming_cross_surface_consistency.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat transcript",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "The sidebar's streaming indicator and the composer's own status readout for the active thread must always agree. It must never be possible for the sidebar to show a thread as streaming while the composer for that same thread simultaneously shows 'ready' (or vice versa).",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the hotbar button surface only, not the underlying route model.",
      blockerRefs: [],
      contractId: "khala_code.nav.hotbar_no_route_text.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/sidebar.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Mounts the real nav hotbar and asserts every slot's visible label matches its static configured label with no route/path-shaped text.",
          id: "hotbar_no_route_text.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "app navigation",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "enforced",
      statement:
        "The app-section nav hotbar (fleet/chat/forum/inbox/settings) shows icon plus hotkey only. It must never render a raw route or path fragment as visible text on a hotbar button.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "This is the app-section hotbar (Option+digit); it is distinct from khala_code.chat.recent_thread_cmd_hotkeys.v2's Cmd-hold recent-chat hints, which live in the sidebar rows instead of on hotbar buttons.",
      blockerRefs: [],
      contractId: "khala_code.nav.hotbar_hotkey_always_visible.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/sidebar.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Mounts the real nav hotbar and asserts every slot renders a non-empty hotkey badge containing its configured digit.",
          id: "hotbar_hotkey_always_visible.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "app navigation",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "Every nav hotbar button always displays its own trigger hotkey as a small visible badge (e.g. '\u23251'), not just discoverable by trial. Pressing the displayed modifier-plus-digit combination for a button always routes to that section.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds keyboard interception correctness only.",
      blockerRefs: [],
      contractId: "khala_code.nav.hotbar_no_stray_special_characters.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/sidebar.ts",
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "References the existing regression test that dispatches a real Option+Digit2 keydown (producing the macOS special character '\u2122' in event.key) at a focused input and asserts it is intercepted (defaultPrevented) and routes correctly instead of leaking into the input.",
          id: "hotbar_no_stray_special_characters.regression_ref",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "app navigation",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-02",
      },
      state: "enforced",
      statement:
        "The Option+digit hotbar shortcut must always be intercepted as a navigation command and must never leak macOS's special/garbled Option-key characters (e.g. \u00a1\u2122\u00a3\u00a2) into any input field or onto the page.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Applies to all flyout/context menus app-wide, including the thread-action menu and the fleet menu.",
      blockerRefs: [],
      contractId: "khala_code.menus.flyout_single_line_no_preamble.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/ui/src/menu-dom.ts",
        "clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Mounts the real thread sidebar, opens its context menu, and asserts no header element and no per-item description/subheadline render.",
          id: "flyout_single_line_no_preamble.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "app-wide menus",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "Right-click and flyout menus render one line per item, with no explanatory subheadline text under each item and no header/preamble content above the options.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Narrow scope: this contract targets literal leaked internal labels, not the fleet menu's overall information architecture.",
      blockerRefs: [],
      contractId: "khala_code.fleet.menu_no_stray_labels.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/fleet-status.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Asserts the fleet panel source never contains the literal string 'ACCT'.",
          id: "fleet_menu_no_stray_labels.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "fleet panel",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "The fleet menu must not render stray internal label text (e.g. a literal 'ACCT' tag) that is not part of a designed, human-readable element.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds resumability of in-flight work; does not require preserving unsent composer drafts unless separately contracted.",
      blockerRefs: [],
      contractId: "khala_code.app.resumes_after_restart.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/main.ts",
        "clients/khala-code-desktop/src/bun/index.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Pins that the last active thread id is read (not cleared) at boot, restored via a dedicated best-effort function after the initial render, and that a failed restore clears the stale id instead of retrying forever.",
          id: "resumes_after_restart.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "app lifecycle",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "When the app restarts, whether voluntary or due to a crash/relaunch, any work that was in flight resumes rather than silently stopping. The user should not have to notice and manually recover in-progress state after a restart.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds first-launch behavior for currently-disabled features; does not block future opt-in enablement flows.",
      blockerRefs: [],
      contractId: "khala_code.app.no_unrequested_first_launch_scripts.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/bun/index.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "References the existing Apple FM bridge disabled-on-launch regression tests and confirms package.json carries no prepare:apple-fm-bridge script.",
          id: "no_unrequested_first_launch_scripts.regression_ref",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "app lifecycle",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "Features that have not been enabled (e.g. Apple Bridge) must not run any preparation or background script on first launch. A disabled feature stays fully inert until explicitly turned on.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Display-only claim; does not change the exact-only token accounting invariants (usage_truth='exact', reconciliation against token_usage_events) owned elsewhere.",
      blockerRefs: [],
      contractId: "khala_code.tokens.per_thread_live_counter.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Pins the token counter's top-right CSS placement, its click handler opening the sync-detail popover, and that the popover surfaces both leaderboard-synced and pending-sync token fields.",
          id: "per_thread_live_counter.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "token accounting",
      source: {
        channel: "codex-session",
        statedBy: "owner",
        statedOn: "2026-07-01",
      },
      state: "enforced",
      statement:
        "A live per-thread token counter is visible in the top-right of the Khala Code screen while a thread is active, updating as tokens accrue. Clicking it shows how many of those tokens have synced to the public leaderboard.",
      surface: "khala-code-desktop",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the existence and reuse of the desktop harness for a terminal REPL; does not change the underlying Codex app-server chat runtime or its approval/sandbox behavior.",
      blockerRefs: [],
      contractId: "khala_code.terminal.tui_mode_available.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/scripts/khala-code-tui.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Pins that the TUI script reuses the exact desktop chat/harness/status functions (createCodexAppServerChatRuntime, createCodexAppServerHost, inspectCodexHarnessStatus) rather than a parallel implementation, and exposes the /new, /status, and /exit slash commands.",
          id: "tui_mode_available.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "terminal",
      source: {
        channel: "community-feedback-discord",
        statedBy: "TheBenMeadows (community; relayed via Lathe operator agent PR)",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "A terminal (TUI) mode is available: an interactive REPL over the same Codex app-server harness the desktop app uses, for users who want the engine without the window.",
      surface: "khala-code-desktop",
      verification:
        "Enforced 2026-07-03: shipped via PR #8221 (merged). bun test tests/ux-contracts.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the Settings model picker only; other consumers of the model catalog (e.g. diagnostics) may still see hidden entries.",
      blockerRefs: [],
      contractId: "khala_code.settings.hidden_models_excluded_from_picker.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-settings-panel.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Mounts the real Codex settings panel with a model catalog that includes a hidden entry (e.g. 'Codex Auto Review') and asserts it never appears as a selectable option and its label/'(hidden)' marker never leaks into the panel text.",
          id: "hidden_models_excluded_from_picker.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/codex-settings-panel.test.ts",
        },
      ],
      productArea: "settings",
      source: {
        channel: "community-feedback-discord",
        statedBy: "TheBenMeadows (community; relayed via Lathe operator agent issue #8230)",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "Internal/hidden Codex models (e.g. 'Codex Auto Review') never appear as selectable entries in the Settings model picker.",
      surface: "khala-code-desktop",
      verification:
        "Enforced 2026-07-03: fixed for GitHub issue #8230 via PR #8236 (merged). bun test tests/codex-settings-panel.test.ts inside clients/khala-code-desktop; runs in the package test glob, the package verify chain, and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds display labeling only — whether a read-only settings value reads as an unexplained 'Unset' or an honest 'Default'. Does not itself make any field editable; that is tracked separately by khala_code.settings.editable_not_env_var_only.v1.",
      blockerRefs: [],
      contractId: "khala_code.settings.no_bare_unset_labels.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-settings-panel.ts",
        "clients/khala-code-desktop/src/ui/claude-settings-panel.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Mounts the real Codex settings panel with a projection whose read-only config fields (provider, reasoning summary, verbosity, approval, sandbox, etc.) are null, and asserts every rendered metric value is 'Default' and none is the bare word 'Unset'.",
          id: "no_bare_unset_labels.codex_panel.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/codex-settings-panel.test.ts",
        },
        {
          description:
            "Mounts the real Claude settings section with a projection whose account fields are null, and asserts every rendered metric value is 'Default' and none is the bare word 'Unset'.",
          id: "no_bare_unset_labels.claude_panel.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/claude-settings-panel.test.ts",
        },
      ],
      productArea: "settings",
      source: {
        channel: "community-feedback-discord",
        statedBy: "TheBenMeadows (community; relayed via Lathe operator agent issues/PR)",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "A read-only settings value never displays the bare, unexplained word 'Unset'. When it reflects a default, it says so in plain language (e.g. 'Default').",
      surface: "khala-code-desktop",
      verification:
        "Enforced 2026-07-03: fixed as part of the response to community feedback that also produced #8230-#8233 and PR #8221. Both the Codex and Claude settings panels now render 'Default' instead of 'Unset' for null/undefined read-only metric values. Runs on every test-sweep invocation.",
    },
    {
      authorityBoundary:
        "Binds only the desktop consent gate and local capture planner. Production capture remains owner-gated by KHALA_CODE_DESKTOP_TRACE_CAPTURE_ENABLED plus an owner-only ingest sink; this contract does not authorize public traces, payout eligibility, settlement eligibility, promise-green movement, or capture on paid plans.",
      blockerRefs: [],
      contractId: "khala_code.plans.free_trace_capture_explicit_consent.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8250",
        "clients/khala-code-desktop/src/shared/trace-capture.ts",
        "clients/khala-code-desktop/src/ui/plans-panel.ts",
        "clients/khala-code-desktop/src/bun/rpc-handlers.ts",
        "clients/khala-code-desktop/tests/trace-capture.test.ts",
        "clients/khala-code-desktop/tests/plans-panel.test.ts",
        "clients/khala-code-desktop/tests/rpc-handlers.test.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Runs the pure desktop trace-capture planner and proves default-off consent, paid-plan exclusion, redaction failure, and owner-only ingest gates all return not-captured unless every gate passes; successful owner-only capture keeps payout and settlement markers inert.",
          id: "trace_capture_planner.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/trace-capture.test.ts",
        },
        {
          description:
            "Mounts the real plans panel in a DOM and proves the trace-capture consent control is off by default, performs no write before a user toggle, writes only after explicit checkbox activation, and shows paid-plan opt-out as not captured.",
          id: "trace_capture_consent_panel.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/plans-panel.test.ts",
        },
        {
          description:
            "Exercises the desktop RPC consent setting and proves it persists only the explicit boolean consent, calls no network, stays owner-gated, exposes the served disclosure ref, and reports not_captured with inert payout/settlement markers.",
          id: "trace_capture_consent_rpc.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/rpc-handlers.test.ts",
        },
      ],
      productArea: "plans and billing settings",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-04",
      },
      state: "enforced",
      statement:
        "Explicit consent UI in the desktop is default OFF and never dark-patterned. It gates a capture pipeline of session events to Rampart redaction to owner_only trace ingest, aligned with data.free_tier_capture_disclosure.v1 and the paid-plan opt-out. Any redaction failure fails closed to not-captured, and capture grants no payout or settlement.",
      surface: "khala-code-desktop",
      verification:
        "bun test clients/khala-code-desktop/tests/trace-capture.test.ts clients/khala-code-desktop/tests/plans-panel.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/ux-contracts.test.ts; these files run in the package test glob and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the desktop settings payment surface only. Plan catalog, entitlement, and purchase settlement remain owned by openagents.com plan APIs; credit package catalog, balance, and checkout fulfillment remain owned by the existing web billing surface. The desktop may open those checkout URLs, but it must not synthesize paid entitlement or credit balance state.",
      blockerRefs: [],
      contractId: "khala_code.plans.checkout_handoff_server_truth.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8249",
        "clients/khala-code-desktop/src/ui/plans-panel.ts",
        "clients/khala-code-desktop/src/bun/rpc-handlers.ts",
        "clients/khala-code-desktop/tests/plans-panel.test.ts",
        "clients/khala-code-desktop/tests/rpc-handlers.test.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Mounts the real plans panel in a DOM: while the paid-plan seam is unarmed the purchase control is disabled, an armed Stripe payment_required response opens exactly the server-returned checkout URL and re-reads status, and the same surface opens the existing /billing checkout for credits without rendering local fake package or balance state.",
          id: "plans_checkout_handoff.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "clients/khala-code-desktop/tests/plans-panel.test.ts",
        },
        {
          description:
            "Decodes the plan-purchase RPC's Stripe payment_required response as a checkout handoff and asserts it does not contain a receiptRef or entitlementRef until the server returns a fulfilled receipt.",
          id: "plan_purchase_payment_required_rpc.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-code-desktop/tests/rpc-handlers.test.ts",
        },
      ],
      productArea: "plans and billing settings",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-04",
      },
      state: "enforced",
      statement:
        "Khala Code desktop plans panel hands off to real checkout (RL-4) with honest not-purchasable state while unarmed; credit packages (BF-2.4 tiers) purchasable from same surface via existing web checkout handoff. Post-purchase state (plan/entitlement/credits) renders from server truth via existing RPCs — never fabricated client-side.",
      surface: "khala-code-desktop",
      verification:
        "bun test clients/khala-code-desktop/tests/plans-panel.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/ux-contracts.test.ts; these files run in the package test glob and the repo test:khala-code-desktop sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds settings-panel editability for values that are structurally configurable (Codex/Claude config keys); does not apply to values that are genuinely environment-only by design (e.g. secrets that must never be typed into the UI), which should instead say so honestly per khala_code.settings.no_bare_unset_labels.v1.",
      blockerRefs: ["blocker.github_issue.8254"],
      contractId: "khala_code.settings.editable_not_env_var_only.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/codex-settings-panel.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "settings",
      source: {
        channel: "community-feedback-discord",
        statedBy: "TheBenMeadows (community; relayed via Lathe operator agent issues/PR)",
        statedOn: "2026-07-03",
      },
      state: "pending",
      statement:
        "Read-only Codex config metrics that reflect a genuinely configurable value (model provider, approval policy, sandbox mode, reasoning summary, verbosity) are editable from the settings UI itself, reusing the existing config-value write RPC, rather than requiring the user to edit an external environment variable or config file.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: recorded from community feedback (relayed by TheBenMeadows, formalized by the Lathe operator agent) on 2026-07-03. Tracked in GitHub issue #8254, filed the same day: needs per-field enum/option sourcing for approval policy, sandbox mode, verbosity, and reasoning summary before it can flip to enforced.",
    },
    {
      authorityBoundary:
        "Binds the Khala lane's missing-token UI affordance only; does not itself define the token-minting backend flow, which is separate implementation work tracked by the same issue.",
      blockerRefs: ["blocker.github_issue.8255"],
      contractId: "khala_code.chat.khala_lane_connect_button.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-code-desktop/src/ui/main.ts",
        "docs/khala-code/khala-code-ux-contract.md",
      ],
      oracles: [],
      productArea: "Khala lane",
      source: {
        channel: "community-feedback-discord",
        statedBy: "TheBenMeadows (community; relayed via Lathe operator agent issues/PR)",
        statedOn: "2026-07-03",
      },
      state: "pending",
      statement:
        "When the Khala lane is unavailable because the desktop process has no OPENAGENTS_AGENT_TOKEN, the lane offers a 'Connect' button that drives an in-app flow to obtain and persist a token, instead of only explaining the missing environment variable in text.",
      surface: "khala-code-desktop",
      verification:
        "Not yet enforced: recorded from community feedback (relayed by TheBenMeadows, formalized by the Lathe operator agent) on 2026-07-03. Tracked in GitHub issue #8255, filed the same day: needs a backend token-minting/device-auth flow and a new local persistence RPC before it can flip to enforced.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-04.2",
}
