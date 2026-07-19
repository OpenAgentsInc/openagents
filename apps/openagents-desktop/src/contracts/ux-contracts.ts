import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts";

export const openAgentsDesktopUxContractRegistry: BehaviorContractRegistryDocument =
  {
    schemaVersion: BehaviorContractSchemaVersion,
    version: "2026-07-19.1",
    contracts: [
      {
        contractId: "openagents_desktop.chat.no_noop_spec_revalidation_error_rows.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "post-turn spec revalidation transcript projection",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-screenshot-review", statedBy: "owner", statedOn: "2026-07-17" },
        statement: "works but whawt the fuck is that error",
        authorityBoundary:
          "The main-owned ProductSpec/AssuranceSpec revalidation remains fail-closed and re-reads the same bounded authority after a provider turn. An identical before/after snapshot is a private no-op receipt, not a conversation event, and is never persisted. Historical product-owned no-op receipts are omitted from transcript projection. A genuinely changed spec snapshot may still produce the existing bounded owner-visible receipt, but it is an informational system record rather than a provider or turn failure even when its structural diagnostics contain the word errors. This presentation rule does not confirm an obligation, repair an invalid spec, weaken validation, or grant release authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/spec-lane-workflow.ts",
          "apps/openagents-desktop/src/renderer/tool-cards.ts",
          "apps/openagents-desktop/src/renderer/react-timeline.tsx",
          "github:OpenAgentsInc/openagents#8995",
        ],
        oracles: [
          {
            id: "spec_revalidation.identical_snapshot_emits_no_note",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/spec-lane-workflow.test.ts",
            description:
              "Proves identical before/after authority snapshots emit no conversation receipt for either local provider lane.",
          },
          {
            id: "spec_revalidation.historical_noop_receipts_are_not_rendered",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/tool-cards.test.ts",
            description:
              "Proves a historical product-owned no-op receipt is omitted while changed receipts and assistant-authored quoted text remain visible.",
          },
          {
            id: "spec_revalidation.changed_receipt_is_not_a_turn_error",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-timeline.test.tsx",
            description:
              "Proves a genuinely changed spec receipt remains an informational system record even when bounded diagnostics mention multiple errors.",
          },
        ],
        verification:
          "Spec-lane, transcript projection, React timeline, behavior-contract, Desktop typecheck/build, repository checks, and local oa-dev visual verification. No release command is part of the oracle.",
      },
      {
        contractId: "openagents_desktop.chat.installed_codex_model_catalog_without_protocol_warning_noise.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Codex composer model selection and transcript diagnostics",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-screenshot-review", statedBy: "owner", statedOn: "2026-07-17" },
        statement:
          "kinda works but i get this fucked up warnings. and i cant choose anything other than gpt 5.5 WHAET TH EFUCK. FIX THAT. and no new rc build until i test it locally. tell me when its ready to test\\",
        authorityBoundary:
          "Electron main reads the visible model/list catalog and per-model reasoning efforts from the user's validated installed Codex app-server through the existing control plane. That exact bounded catalog is policy-intersected into the provider-lane projection; the renderer offers a direct native model select, chooses the installed default when a stale selection is absent, and reconciles unsupported reasoning to that model's advertised default. Exact turn admission remains main-owned and fails closed against the same catalog. The installed Codex thread/resume response may carry its two newer pagination cursors; the wire boundary projects away only itemsBackwardsCursor and turnsBackwardsCursor before complete generated-schema validation, so all authority-bearing response fields and any other unknown drift still fail closed. App-server compatibility receipts, rate-limit decode drift, and token-usage decode drift remain private connection diagnostics and release-gate evidence; they never become transcript lane_notice rows. This work stops at a locally testable development app and grants no RC, tag, package, release-asset, or publication authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-control-plane.ts",
          "apps/openagents-desktop/src/codex-local-runtime.ts",
          "apps/openagents-desktop/src/provider-lane-capabilities.ts",
          "apps/openagents-desktop/src/renderer/react-composer.tsx",
          "apps/openagents-desktop/src/renderer/tool-cards.ts",
          "apps/openagents-desktop/src/codex-app-server-client.ts",
          "apps/openagents-desktop/src/codex-app-server-turn.ts",
          "packages/codex-app-server-protocol/src/decode.ts",
          "github:OpenAgentsInc/openagents#8995",
        ],
        oracles: [
          {
            id: "installed_codex_model_catalog.direct_visible_model_and_reasoning_selects",
            kind: "bun-test",
            mode: "dom",
            ref: "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
            description:
              "Renders all seven visible installed-catalog model ids in a native select, proves direct GPT-5.4 Mini selection dispatches the exact id, and limits the reasoning selector to the active model's advertised efforts.",
          },
          {
            id: "installed_codex_model_catalog.default_and_effort_reconciliation",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves a stale model falls back to the installed default and an unsupported reasoning level falls back to that selected model's advertised default.",
          },
          {
            id: "installed_codex_model_catalog.compatibility_receipts_are_not_transcript",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-app-server-turn.test.ts",
            description:
              "Forces compatibility receipts from deliberately incomplete app-server fixture responses and proves no Codex compatibility notice is emitted as a lane_notice while intentional Guardian review notices remain visible.",
          },
          {
            id: "installed_codex_model_catalog.historical_compatibility_notes_are_not_rendered",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/tool-cards.test.ts",
            description:
              "Proves a compatibility notice persisted by a pre-fix build is omitted from transcript projection while Guardian, account-rotation, and assistant-authored text remain visible.",
          },
          {
            id: "installed_codex_model_catalog.resume_additive_cursors_are_bounded",
            kind: "bun-test",
            mode: "unit",
            ref: "packages/codex-app-server-protocol/src/protocol.test.ts",
            description:
              "Proves strict installed-Codex thread resume accepts only the two observed additive pagination cursors, omits them from the decoded payload, and still rejects any other unknown response authority.",
          },
        ],
        verification:
          "Desktop control-plane, local-runtime, provider-capability, shell, React composer, app-server-turn, behavior-contract, typecheck, build, and local oa-dev visual verification. No release command is part of the oracle.",
      },
      {
        contractId: "openagents_desktop.chat.full_auto_resume_identity_followup_progress.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Full Auto conversation resume",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-screenshot-review", statedBy: "owner", statedOn: "2026-07-17" },
        statement:
          "Multiple bugs in trying to resume a stalled Full Auto conversation. One is, the chat shows twice in left sidebar. one with 6h one with a loadspinner. I click Full Auto and see the status indicator but nothing happens for awhile. And I can't send followup messages in the chat wtf. debug and fix on new worktree then push to main",
        authorityBoundary:
          "Electron main remains the authority that verifies a provider-history ref aliases one mutable Desktop-local thread, owns background Full Auto execution, persists streamed progress, and promotes a durable queued follow-up. The renderer canonicalizes to the returned local thread ref, removes only that verified top-level provider alias, and uses the canonical ref for queue, Full Auto, lane, hydration, navigation, and composer state. A main-owned background turn stays distinct from renderer pending state: Stop keeps the thread-scoped main interrupt route, while text submission is queue-only and cannot start a concurrent turn. Background events publish bounded persisted thread snapshots without granting the renderer question-answer or dispatch authority. A promoted durable queue identity is consumed once before the next generic Full Auto continuation.",
        evidenceRefs: [
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/full-auto-followup.ts",
          "apps/openagents-desktop/src/provider-lane.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/react-composer.tsx",
          "apps/openagents-desktop/src/codex-durable-queue.ts",
        ],
        oracles: [
          {
            id: "full_auto_resume.canonical_alias_and_control_hydration",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Resumes a provider-history ref as a different canonical local UUID, removes the verified duplicate row/search alias, and proves queue, Full Auto, lane, transcript hydration, running state, and composer selection all use the canonical ref.",
          },
          {
            id: "full_auto_resume.background_queue_only_composer",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
            description:
              "Projects a main-owned running Full Auto turn as queue-only composer admission with an enabled follow-up action while retaining the exact Stop affordance.",
          },
          {
            id: "full_auto_resume.background_queue_promotion_once",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/full-auto-followup.test.ts",
            description:
              "Transfers a main-owned background Full Auto promotion to exactly one next dispatch while leaving foreground and ordinary-turn promotion ownership unchanged.",
          },
          {
            id: "full_auto_resume.background_progress_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/provider-lane.test.ts",
            description:
              "Proves background Full Auto events cross the durable post-projection observer before terminal completion without being forwarded as renderer-owned stream events.",
          },
        ],
        verification:
          "Desktop shell/composer/provider-lane suites, behavior-contract validation, Desktop typecheck/build, and repository completion gate.",
      },
      {
        contractId: "openagents_desktop.chat.empty_state_centers_current_directory.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "empty conversation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-15" },
        statement:
          "An empty conversation centers the Start a conversation with Codex prompt, shows the current working directory, and offers a Change action beside it.",
        authorityBoundary:
          "Electron main remains the sole WorkContext authority and exposes only a schema-decoded working-directory projection plus the existing fixed workspace-picker capability through preload. React receives the displayed value in Effect-owned DesktopShellState and may only dispatch DesktopWorkspacePickerRequested; it cannot read process.cwd(), submit an absolute path, or infer a directory from history. Main initializes the native directory dialog from the current root when available and admits a selection through the same workspace authority used by files, terminal, Git, and Codex. Cancel or failure retains the current workspace and path. The action renders only in a genuinely empty new-chat timeline.",
        evidenceRefs: [
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/preload.cts",
          "apps/openagents-desktop/src/workspace-contract.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/renderer/react-timeline.tsx",
          "packages/ui/src/desktop-workbench.css",
        ],
        oracles: [
          {
            id: "empty_conversation.centered_current_working_directory",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx",
            description:
              "Proves the empty React conversation renders the host-projected working directory, folder icon, and keyboard-accessible Change action; the action dispatches the existing picker intent and disappears once timeline content exists.",
          },
          {
            id: "empty_conversation.narrow_schema_decoded_host_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/electron-boundary.test.ts",
            description:
              "Proves preload exposes a narrow schema-decoded working-directory query and fixed picker capability, while main seeds the native dialog from the current root and reports cancel as no selection without restoring the broad workspace-summary bridge.",
          },
        ],
        verification:
          "The React workbench, shell intent-loop, runtime-workspace, and Electron boundary suites plus Desktop typecheck enforce presentation, cancel/selection semantics, and the single WorkContext authority.",
      },
      {
        contractId: "openagents_desktop.window.launch_fills_work_area.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Desktop window launch geometry",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-15" },
        statement:
          "When OpenAgents opens, make it take up the full width and height of the screen without entering fullscreen.",
        authorityBoundary:
          "At BrowserWindow creation, Electron resolves the display under the current cursor and applies that display's usable workArea x, y, width, and height. The window remains ordinary and resizable with fullscreen explicitly false, so the menu bar, Dock/taskbar, traffic lights, and separate typed fullscreen command retain their existing semantics. No renderer, persistence, display-reconfiguration, or window-management authority is added.",
        evidenceRefs: [
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/tests/startup-contract.test.ts",
        ],
        oracles: [
          {
            id: "window_launch.active_display_work_area_not_fullscreen",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/startup-contract.test.ts",
            description:
              "Proves active-display workArea resolution precedes BrowserWindow construction; all four bounds come from that workArea; fullscreen is explicitly false; and startup does not substitute maximize().",
          },
        ],
        verification:
          "Desktop typecheck and the focused startup-contract suite.",
      },
      {
        contractId: "openagents_desktop.chat.shadcn_message_scroller_and_composer.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "React transcript scrolling and message composer",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-component-review", statedBy: "owner", statedOn: "2026-07-15" },
        statement:
          "Use the supplied Message Scroller ideas throughout the message area and make the input bar match its compact icon-led quality.",
        authorityBoundary:
          "The registry-installed shadcn MessageScroller owns viewport-only mechanics: last-user-turn opening, stable row IDs, user-turn anchoring with previous-context peek, live-edge following, reader-interaction release, prepend preservation, jump controls, scrollability attributes, accessibility, and offscreen paint containment. Effect-owned DesktopShellState remains the sole message/stream/history authority and typed intents retain paging and composer actions. The composer remains the admitted shadcn Textarea/Button composition with closed-catalog icon slots plus accessible names; one textual mode toggle shows only the active Steer or Queue behavior and switches to the other admitted behavior without changing submission authority. #8828 restores the already-authorized bounded image picker/paste/drop projection without adding model, transport, branching, or persistence authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/components/ui/message-scroller.tsx",
          "apps/openagents-desktop/src/renderer/react-timeline.tsx",
          "apps/openagents-desktop/src/renderer/react-composer.tsx",
          "packages/ui/src/desktop-workbench.css",
          "apps/openagents-desktop/src/renderer/react-timeline.test.tsx",
          "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
        ],
        oracles: [
          {
            id: "message_scroller.reader_intent_and_composition",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-timeline.test.tsx",
            description:
              "Exercises stable turn anchors, last-edge following, wheel-release/manual-position hold, prepend preservation, same-row streaming resize, jump-to-latest re-engagement, typed edge paging, keyboard-focusable region plus additions-only log semantics, busy state, and the 500-row bounded corpus.",
          },
          {
            id: "message_composer.compact_icon_actions",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
            description:
              "Proves Commands, Stop, and Send/Steer/Queue submission use catalog-marked icon controls with accessible labels, the pending-mode control renders only its active Steer or Queue choice, and toggling preserves exact intents, focus, autosize, duplicate-send defense, IME composition, and pending-mode behavior.",
          },
        ],
        verification:
          "Desktop typecheck plus focused React timeline/composer, contract-validation, renderer-boundary, and design-conformance suites.",
      },
      {
        contractId: "openagents_desktop.sidebar.codex_shaped_react_anatomy.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "React workbench sidebar chrome",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-screenshot-review", statedBy: "owner", statedOn: "2026-07-15" },
        statement:
          "Lay out the OpenAgents sidebar like Codex: left-aligned menus, OpenAgents identity, icon-only search, sidebar expander, back/forward positions, and appropriate icons from the existing Apps SDK catalog.",
        authorityBoundary:
          "Presentation over already-admitted authority only. One Effect-owned typed projection supplies the compatibility shell and React workbench with exactly two controls around the Recent list: New session and Settings. Project home, the dead Chat destination, and the coding Workspaces section are absent. React lowers New session beneath the OpenAgents identity row and icon-only search disclosure, while Settings is pinned alone at the bottom of the rail in the former workspace-box region. Each retained control reuses its canonical command identity and typed intent, carries a closed @effect-native/core IconName, and projects selected/current state without inventing an unread count or status when no such authority exists. The Recent section uses single-line conversation rows whose truncated title stays left while the compact relative timestamp is right-justified on that same line when idle; exact per-thread pending or Full Auto turn-running authority replaces that timestamp with the shared loading icon while the chat works. Lifecycle/status words (including Completed, Running, or Waiting), provider labels, and search match-kind strings are forbidden in conversation rows. New session remains the primary action rather than a selected destination. Sidebar collapse is Effect-owned presentation state and only its boolean preference persists through the versioned main-process preferences boundary. Session-search disclosure is deliberately launch-ephemeral and starts closed after reload or restart; its query remains exclusively in the existing history authority and closing search clears it through HistorySearchChanged. Restoring a collapsed rail reuses the preferences read already required before shell mount, adds no new startup read, leaves a reachable expander, and never steals focus from the composer. Search, destination selection, collapse, and Back/Forward dispatch only typed intents. No enabled placeholder, React-owned navigation store, parallel query store, fabricated destination, or Project home route is authorized.",
        evidenceRefs: [
          "apps/openagents-desktop/src/desktop-preferences-contract.ts",
          "apps/openagents-desktop/src/renderer/sidebar-destinations.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/react-primitive-adapters.tsx",
          "packages/ui/src/desktop-workbench.css",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/main.ts",
          "github:OpenAgentsInc/openagents#8826",
        ],
        oracles: [
          {
            id: "react_sidebar.shared_destination_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/sidebar-destinations.test.ts",
            description:
              "Pins the exact three-control order, labels, closed-catalog icons, canonical command identities, typed intents, selected/current projection, and truthful absence of unread/status indicators without backing state.",
          },
          {
            id: "react_sidebar.anatomy_icons_search_and_alignment",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx",
            description:
              "The real React workbench proves New session above Recent, Settings alone in the bottom footer, absence of Project home, Chat, and Workspaces, selected state, left alignment, and a dedicated right-justified same-line relative timestamp that becomes the shared loading icon under exact per-thread working authority, with no lifecycle, provider, or search-match strings; it also covers icon-only search disclosure/close and collapse recovery through the always-reachable expander.",
          },
          {
            id: "react_sidebar.collapse_preference_migration_and_roundtrip",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-preferences.test.ts",
            description:
              "Proves the v1-to-v2 migration preserves every prior preference while defaulting sidebar collapse to false, validates hostile patches, persists the boolean owner-only, restores it through a fresh store, and resets it to the canonical default.",
          },
          {
            id: "react_sidebar.built_destination_and_restart_smoke",
            kind: "visual-smoke",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built default React Electron journey activates every admitted destination, asserts its real root and selection, discloses and closes search, captures expanded and collapsed sidebar receipts, persists collapse through typed preferences, reloads, and proves collapsed-at-mount plus closed search and retained composer focus.",
          },
        ],
        verification:
          "Desktop preferences, shared destination projection, compatibility shell, focused React workbench DOM, contract-validation, and built React Electron smoke/reload suites enforce the completed #8826 projection and presentation-state boundary.",
      },
      {
        contractId: "openagents_desktop.sidebar.chat_created_order.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "conversation sidebar ordering",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-16" },
        statement:
          "openagents, left sidebar sorts chats by last message so they bounce around. no, sort instead by created date.",
        authorityBoundary:
          "The merged local, confirmed Sync, and Codex-history session rail orders conversations by creation time, newest-created first, with the stable thread ref as the deterministic tie-break. Message arrival, streaming activity, title changes, and other updatedAt changes may refresh row metadata but may never move an existing row. Newly persisted local chats and forks record one immutable createdAt alongside updatedAt; legacy local rows adopt their previously persisted timestamp once and retain it through later writes. Confirmed Sync carries the source thread creation timestamp additively. This changes presentation and bounded local retention order only; it does not change transcript order, active selection, history search authority, Sync confirmation, runtime routing, or persistence ownership.",
        evidenceRefs: [
          "packages/khala-sync-client/src/conversation.ts",
          "apps/openagents-desktop/src/chat-contract.ts",
          "apps/openagents-desktop/src/codex-thread-lifecycle.ts",
          "apps/openagents-desktop/src/codex-history.ts",
          "apps/openagents-desktop/src/merged-history.ts",
          "apps/openagents-desktop/src/thread-store.ts",
          "apps/openagents-desktop/src/renderer/runtime-conversation.ts",
          "apps/openagents-desktop/src/renderer/react-primitive-adapters.tsx",
        ],
        oracles: [
          {
            id: "sidebar_chat_order.created_at_is_stable_across_activity",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx",
            description:
              "Projects merged local and history rows whose creation and activity orders disagree, then proves newer-created stays first after the older chat receives a later update timestamp.",
          },
          {
            id: "sidebar_chat_order.local_created_at_persistence",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/thread-store.test.ts",
            description:
              "Proves new and forked local chats persist immutable creation timestamps and a legacy row adopts its prior persisted timestamp before later activity updates.",
          },
        ],
        verification:
          "Desktop typecheck plus focused thread-store, runtime-conversation, React sidebar projection, behavior-contract validation, and repository check suites.",
      },
      {
        contractId: "openagents_desktop.navigation.authoritative_history.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "workbench navigation history",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-15" },
        statement:
          "Disabled means no reachable target; enabled controls always perform one visible traversal.",
        authorityBoundary:
          "One ephemeral bounded stack is owned by the Effect shell SubscriptionRef and records admitted workspace, local-session, Codex-history, and coding-session destinations only after their authoritative open succeeds. Adjacent duplicates collapse; Back/Forward preserve the forward branch until a new successful navigation replaces it; failed and stale targets cannot advance the cursor. DesktopShellState exposes only enabled state and optional public-safe target titles. React dispatches DesktopNavigationBackRequested or DesktopNavigationForwardRequested exactly once and owns no stack, window.history mutation, filesystem authority, provider authority, persistence, composer focus, or transcript scroll behavior. Default key chords remain unassigned after the editable-control and Electron native-menu collision review; user rebindings remain available through the canonical command contract.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/navigation-history.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/react-primitive-adapters.tsx",
          "apps/openagents-desktop/src/desktop-command-contract.ts",
          "github:OpenAgentsInc/openagents#8825",
        ],
        oracles: [
          {
            id: "navigation_history.stack_semantics",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/navigation-history.test.ts",
            description:
              "Proves bounded push, adjacent deduplication, Back, Forward, forward preservation, successful branch truncation, and unreachable-target removal.",
          },
          {
            id: "navigation_history.effect_commit_and_react_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx",
            description:
              "The shell registry proves failed session opens do not record or move selection, and the React DOM suite proves projected disabled/enabled state plus exactly one typed intent per enabled click.",
          },
          {
            id: "navigation_history.built_react_electron_traversal",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built installed-default React Electron smoke traverses three committed destinations backward and forward while checking the visible title and transcript at every stop.",
          },
        ],
        verification:
          "Focused navigation, shell, React DOM, and command-contract suites plus Desktop typecheck, build, smoke:react, and the repository pnpm run check gate.",
      },
      {
        contractId: "openagents_desktop.design.apps_sdk_starcraft_harmonization.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "desktop design system",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-14" },
        statement:
          "ALL styles harmonized with apps-sdk-ui while preserving our starcraft design",
        authorityBoundary:
          "OpenAgents Desktop composes the Effect Native catalog's typed components, variants, and shared token scales for component appearance while app.css is restricted to Electron host physics: viewport geometry, containment, scrolling, overlays, responsive adaptation, and reduced-motion policy. The only desktop theme is the pinned Tokyo Night semantic projection. This contract grants no new runtime, filesystem, provider, payment, or network authority and does not authorize one-off component recipes outside the typed catalog.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/app.css",
          "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
          "apps/openagents-desktop/src/renderer/theme.ts",
          "docs/fable/2026-07-14-desktop-ui-harmonization-screenshot-receipt.md",
          "github:OpenAgentsInc/openagents#8811",
        ],
        oracles: [
          {
            id: "desktop_design.catalog_and_host_physics_boundary",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
            description:
              "Proves renderer source has no raw colors or non-token sizing recipes, app.css stays within the bounded host-physics vocabulary, typed catalog components own component appearance, and the pinned Tokyo Night projection is the one mounted palette.",
          },
        ],
        verification:
          "Desktop design-conformance and full test sweeps, typecheck, production build, built-Electron smoke, and screenshot receipts enforce the catalog/host boundary without widening product authority.",
      },
      {
        contractId: "openagents_desktop.mvp.assurance_surface_congruence.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "MVP visible-surface assurance coverage",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-14" },
        statement:
          "The assurance spec needs to be fully covering everything that we should expect to actually work, and then nothing else, and ensure that that is specified in the contract.",
        authorityBoundary:
          "UX-5 (#8791) treats the UX-4 MVP dock allowlist as the exact expected-working surface set. Every allowlisted surface interaction must map to ProductSpec criteria, proposed AssuranceSpec items, enforced behavior contracts, and executable oracles; a missing row, empty proof field, or row for a non-MVP surface fails the normal Desktop test sweep. Files and read-only review remain bounded command-reachable supporting views under CW-AC-12/CW-AC-14, not visible dock surfaces. This proof map adds no product authority and cannot admit or verify its proposed AssuranceSpec revision.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.ts",
          "apps/openagents-desktop/src/renderer/mvp-assurance-congruence.ts",
          "docs/mvp/openagents-codex-workroom-mvp-assurance-coverage-matrix.md",
          "docs/mvp/openagents-codex-workroom-mvp.rev4-proposed.assurance-spec.md",
          "github:OpenAgentsInc/openagents#8791",
        ],
        oracles: [
          {
            id: "mvp_assurance.exact_surface_congruence",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/mvp-assurance-congruence.test.ts",
            description:
              "Diffs the UX-4 allowlist against the coverage matrix, checks every proof link and the checked-in Markdown projection, and proves planted under-coverage and over-coverage both fail.",
          },
        ],
        verification:
          "Desktop typecheck and the normal test sweep enforce the exact allowlist-to-assurance mapping with under-coverage and over-coverage falsifiers.",
      },
      {
        contractId: "openagents_desktop.mvp.visible_surface_allowlist.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "MVP visible workroom surface",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-screenshot-review", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "remove from the interface everything not in the MVP spec.",
        authorityBoundary:
          "The accepted ProductSpec Scope and User Experience, plus owner-issued MAINT-1 and #8828 bounded composer-image authority, are the visible-surface allowlist. UX-4 (#8790), #8826, the 2026-07-15 owner simplification, and the 2026-07-16 removal of Project home reconcile both renderers to one typed projection: exactly New session and Settings around the Recent list. React renders Settings alone at the bottom and renders neither Project home, the dead Chat destination, nor the coding Workspaces box. Per-item authority lives in apps/openagents-desktop/src/renderer/sidebar-destinations.ts with composition enforcement in mvp-visible-surfaces.ts. ProductSpec and AssuranceSpec remain internal authoring/verification tooling with no user-facing route, screen, dock item, command, or native-menu destination. Bounded Files and read-only Git review stay reachable through their closed CW-AC-12 command identities, not through dock icons. The review surface renders no Git mutation affordance, and Files renders no file create/rename/delete/reveal affordance. Fleet, provider/account selection, OpenAgents account linking, MCP/plugin configuration, Terminal/Inbox, model/reasoning selection, and voice controls remain absent. The sole admitted attachment affordance is the existing typed image picker/paste/drop path; it adds no arbitrary filesystem or provider authority.",
        evidenceRefs: [
          "docs/mvp/openagents-codex-workroom-mvp.product-spec.md",
          "apps/openagents-desktop/src/desktop-command-contract.ts",
          "apps/openagents-desktop/src/renderer/sidebar-destinations.ts",
          "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/react-primitive-adapters.tsx",
          "apps/openagents-desktop/src/renderer/settings.ts",
          "apps/openagents-desktop/src/main.ts",
          "github:OpenAgentsInc/openagents#8756",
          "github:OpenAgentsInc/openagents#8790",
        ],
        oracles: [
          {
            id: "mvp_surface.shell_allowlist",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the compatibility dock consumes the exact shared three-control projection while rejecting Chat, ProductSpec, AssuranceSpec, Fleet, accounts, provider/model/reasoning selection, and voice; #8828 separately admits only bounded composer images.",
          },
          {
            id: "mvp_surface.rendered_composition",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.test.ts",
            description:
              "Walks the ACTUAL rendered shell tree for every reachable workspace state, requires the dock to equal the cited allowlist exactly (no additions, no silent losses), forbids every non-MVP surface key, and proves the oracle rejects a planted non-MVP surface.",
          },
          {
            id: "mvp_surface.settings_allowlist",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/settings.test.ts",
            description:
              "Proves Settings contains current-Codex-session truth but no OpenAgents account, Pylon, MCP, plugin, or extension-lifecycle controls.",
          },
          {
            id: "mvp_surface.command_allowlist",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-command-contract.test.ts",
            description:
              "Proves Fleet, Terminal, and Inbox are absent from schema-decoded palette, deep-link, shortcut, and native-menu command authority.",
          },
          {
            id: "mvp_surface.read_only_review_boundary",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/git-panel.test.ts",
            description:
              "Proves the review workspace renders no commit/push/stage/discard/branch/issue/PR affordance even when the substrate state carries them.",
          },
        ],
        verification:
          "Desktop typecheck, shared projection, shell/React/settings/command/composition suites, build, and built-host smoke enforce the ProductSpec-visible allowlist against both actual rendered docks and screens.",
      },
      {
        contractId: "openagents_desktop.mvp.visible_surface_sweep.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "MVP visible workroom surface",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-screenshot-review", statedBy: "owner", statedOn: "2026-07-14" },
        statement:
          "This menu, when I click the settings button, looks horrible. This folder thing looks horrible. I thought we made a pass removing all screens that are not specifically called for in the MVP. You need to clean all this up and make a pass to remove everything from the sidebar and all UI that's not specifically called for in our MVP spec.",
        authorityBoundary:
          "UX-4 (#8790): the sidebar dock composition is mechanically enforced against the rendered view tree by the mvp-visible-surfaces oracle — Files and the command palette lose their dock icons (their CW-AC-12 command identities remain the entry points), the Git review panel and Files browser drop every mutation affordance to the CW-AC-14 read-only boundary, and the retained Settings, palette, and Files surfaces are design-passed on the shared tokens (one centered settings column on the raised-panel recipe, family-grouped palette rows with keycap chords, quiet grant-boundary presentation). No copy changed; styling, layout, and composition only. Removal is conservative: a swept surface returns only with an explicit spec citation.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.ts",
          "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.test.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/git-panel.ts",
          "apps/openagents-desktop/src/renderer/workspace-browser.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "github:OpenAgentsInc/openagents#8790",
        ],
        oracles: [
          {
            id: "mvp_sweep.composition_oracle_with_falsifier",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.test.ts",
            description:
              "Renders every reachable workspace through desktopShellView, asserts zero visible-surface violations, and proves planted non-MVP dock items, removed affordances, forbidden screen keys, and silent allowlist shrink each FAIL the oracle.",
          },
          {
            id: "mvp_sweep.built_host_smoke",
            kind: "visual-smoke",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke journey asserts the exact rendered dock ids, the absence of the swept dock icons and Git mutation controls, and captures the pixel receipts for the cleaned sidebar and each retained screen.",
          },
        ],
        verification:
          "Desktop typecheck, the composition suite with falsifiers, design conformance, build, and built-host smoke.",
      },
      {
        contractId: "openagents_desktop.assurance_spec.document_visualization.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "AssuranceSpec document support",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "we'll want to be able to open different files and support different formats. And so just think about like we are adding support for the assurance spec format now. What do we want that file to look like? ... I want to see a beautiful visualization of Assurance Spec in the app",
        authorityBoundary:
          "The Desktop build parses the exact checked-in .assurance-spec.md artifact through the browser-safe package grammar and embeds only its bounded presentation snapshot; future editor-opened source uses the same app-owned projection boundary. Structural validity, criterion mapping, and repository candidates are presentation facts only: the view cannot admit work, execute checks, verify evidence, waive obligations, release software, or change public promises. Invalid bytes replace the document visualization with an explicit invalid state, and no filesystem or repository authority is added to the renderer.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/assurance-spec-workspace.ts",
          "apps/openagents-desktop/src/renderer/assurance-spec-workspace.test.ts",
          "packages/assurance-spec/src/browser.ts",
          "docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md",
        ],
        oracles: [
          {
            id: "assurance_spec.document_is_source_driven_and_authority_honest",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/assurance-spec-workspace.test.ts",
            description:
              "Parses the checked-in MVP AssuranceSpec, proves all 18 obligations and their incomplete proof-design fields render, proves invalid bytes fail closed, and proves the view exposes no execution or verification actions.",
          },
        ],
        verification:
          "Desktop typecheck, renderer unit tests, design conformance, and production build enforce the source-driven Effect Native document visualization and proposal-only authority boundary.",
      },
      {
        contractId: "openagents_desktop.mvp.uses_logged_in_codex_session.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "MVP Codex runtime and Settings",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "this must work with the user's logged in Codex account. This whole idea of linking to pylons does not belong in this desktop application. The MVP needs to be, it uses your logged in Codex session, nothing else. Simplify this all right now.",
        authorityBoundary:
          "The local Desktop MVP admits exactly the ordinary current Codex session and launches app-server from the user's validated installed Codex executable. OpenAgents never packages, copies, or re-signs Codex. Inherited CODEX_HOME is removed so Codex uses its ordinary default ~/.codex state, reusing the user's existing config and authentication. Named Pylon accounts, account rotation, isolated device-auth, and Pylon account rows are not eligible for or rendered by the MVP workroom. The app-owned ProductSpec skill remains digest-pinned under the signed application resources and is registered as an explicit app-server extra root; it is not copied into ~/.codex. Fleet-only account custody remains outside this local-workroom contract. No credential bytes or home paths cross preload or renderer.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-local-runtime.ts",
          "apps/openagents-desktop/src/codex-preflight.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/renderer/settings.ts",
          "apps/openagents-desktop/src/mvp-proof.ts",
          "github:OpenAgentsInc/openagents#8756",
        ],
        oracles: [
          {
            id: "mvp_codex_session.current_only_app_server",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-local-runtime.test.ts",
            description:
              "Offers both the ordinary session and a Pylon account, then proves the production app-server selects only the ordinary session and completes the native interaction round trip.",
          },
          {
            id: "mvp_codex_session.no_linking_surface",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/settings.test.ts",
            description:
              "Proves Settings explains current-session reuse and renders no Codex/Claude Pylon account rows, connect action, reconnect action, or device-auth status.",
          },
          {
            id: "mvp_codex_session.app_owned_skill_root",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/builtin-productspec-skill.test.ts",
            description:
              "Pins ProductSpec skill registration to the signed app-owned extra root while allowing the current Codex session and forbidding default-home skill mutation.",
          },
        ],
        verification:
          "Desktop typecheck and the Codex local-runtime, preflight, Settings, built-in skill, MVP-proof, package, and built-host suites enforce the current-session-only MVP boundary.",
      },
      {
        contractId: "openagents_desktop.settings.reachable_workspace.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Settings navigation and scrolling",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-screenshot-review", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "when I toggle like that settings button or whatever that rightmost button in the sidebar is, it'll open and collapse the Command K bar. That doesn't make any fucking sense. I can't scroll down in the settings page.",
        authorityBoundary:
          "The sidebar dock wraps instead of clipping its final controls and keeps the typed Settings action last, so the visible rightmost gear dispatches only DesktopSettingsToggled; Command-K remains a separate typed command. The Settings workspace is the bounded vertical scroll owner and keeps every control reachable without changing body-level viewport policy. No renderer path, credential, provider payload, or broader runtime authority is introduced.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "github:OpenAgentsInc/openagents#8756",
        ],
        oracles: [
          {
            id: "settings_navigation.rightmost_action_is_settings_only",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves Settings is the final dock item and its real typed intent opens and closes Settings without opening the Command-K palette.",
          },
          {
            id: "settings_navigation.workspace_is_scroll_owner",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
            description:
              "Proves the token-conformant Settings workspace owns bounded vertical overflow while the host body remains fixed.",
          },
        ],
        verification:
          "Desktop shell and design-conformance suites plus typecheck/build enforce the distinct navigation intents and scroll ownership.",
      },
      {
        contractId: "openagents_desktop.chat.launch_directory_is_default_cwd.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local coding workspace",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-review", statedBy: "owner", statedOn: "2026-07-12" },
        statement:
          "the sessions are saved to this fucking app support shit ... needs to go into the current directory where the app was started from by default, and later configurable in a dir",
        authorityBoundary:
          "Electron main captures the owner launcher directory at module launch (OPENAGENTS_DESKTOP_LAUNCH_CWD for a managed launcher, otherwise process.cwd()), validates it as a directory, admits it as the initial canonical WorkContext, and supplies that exact directory as the top-level Claude and Codex coding cwd. It supersedes stale persisted navigation on every ordinary launch; an explicit in-app directory choice may replace it afterward. The provider runtimes may not silently substitute an Application Support per-thread directory. Smoke/live-proof runs remain isolated under test userData; probes, account custody, and delegated child scratch work are unchanged. This launch admission does not grant the renderer absolute path-selection authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/desktop-launch-workspace.ts",
          "apps/openagents-desktop/src/desktop-launch-workspace.test.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/codex-local-runtime.ts",
        ],
        oracles: [
          {
            id: "local_workspace.claude_exact_host_root",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description: "Proves an explicit host workspace root becomes the exact Claude SDK cwd.",
          },
          {
            id: "local_workspace.codex_exact_host_root",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-local-runtime.test.ts",
            description: "Proves the same explicit host workspace root becomes Codex's process cwd and -C argument.",
          },
        ],
        verification:
          "Desktop typecheck/build plus the Fable and Codex local-runtime suites enforce exact host-root propagation while retaining isolated fallback coverage.",
      },
      {
        contractId: "openagents_desktop.chat.codex_turns_do_not_time_out.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Codex local turn lifecycle",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-12" },
        statement: "WHAT THE FUCK IS THIS TIMEOUT --- FIX IT",
        authorityBoundary:
          "A production top-level local Codex turn has no host wall-clock deadline: long or temporarily quiet work remains alive until the Codex process completes, fails, or the owner dispatches the existing typed Stop intent. Elapsed time alone never sends SIGTERM and never fabricates a timeout/provider-unavailable state. A deadline remains dependency-injectable only for deterministic unit coverage of the typed failure path; it is not wired by Electron main and grants no renderer or provider authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-local-runtime.ts",
          "apps/openagents-desktop/src/main.ts",
        ],
        oracles: [
          {
            id: "codex_turn_lifecycle.no_production_deadline",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-local-runtime.test.ts",
            description:
              "Runs a non-closing Codex process with production defaults, proves it remains pending, then proves the existing exact-turn interrupt still terminates it as interrupted rather than timed out.",
          },
        ],
        verification:
          "pnpm exec vp test apps/openagents-desktop/src/codex-local-runtime.test.ts plus Desktop typecheck and build enforce the no-default-deadline lifecycle and explicit Stop authority.",
      },
      {
        contractId: "openagents_desktop.chat.provider_event_interleaving.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript event ordering",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-12" },
        statement:
          "the assistant message there at end is out of order like the tool calls appeared BEFORE that, i need everything interleaved sequentially in the order we get it. fix it and push",
        authorityBoundary:
          "The renderer and durable local thread store project display-bearing provider events in their exact arrival order. Consecutive text deltas coalesce into one assistant segment and publish at most once per renderer cadence until a display-bearing non-text event creates a new visible timeline position; that insertion boundary synchronously flushes and closes the segment, the event is inserted next, and later text opens a new assistant segment after it. Header-only accounting and lifecycle events never split assistant prose. Keyed progress/completion, plan, question, child, and queue refreshes that update an existing card in place also never split assistant prose because they add no visible position; the renderer and durable journal share one typed boundary tracker so finalization cannot reintroduce phantom paragraph gaps. Renderer-to-shell projection is bounded latest-state-wins: at most the in-flight and newest complete thread snapshots are retained, inactive-chat events publish no shell revision, and settlement awaits the newest projection. The durable main-process journal remains the complete ordered event authority. Completion still flushes before settlement. A tool result updates its matching invocation card in place at the invocation's original position. Final usage/model metadata may enrich the last assistant segment through a keyed in-place upsert but may never append or move that segment past intervening tool, model, reasoning, or lane events. No event gains new renderer, filesystem, provider, or persistence authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/renderer/latest-only-queue.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/thread-store.ts",
        ],
        oracles: [
          {
            id: "provider_event_interleaving.live_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/local-harness.test.ts",
            description:
              "Streams model, assistant text, tool use, then more assistant text interleaved with keyed tool progress/result updates and proves those in-place refreshes do not fragment the second paragraph; the transcript still retains two correctly attributed assistant segments around the actual tool-card insertion, the durable journal matches the live renderer, and a 10,000-delta stress case proves one cadence publication with exact text.",
          },
          {
            id: "provider_event_interleaving.bounded_shell_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/latest-only-queue.test.ts",
            description:
              "Blocks the active projection, submits 10,000 newer complete snapshots, and proves only the in-flight and exact latest snapshots are retained and processed before flush settles.",
          },
          {
            id: "provider_event_interleaving.durable_upsert_position",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/thread-store.test.ts",
            description:
              "Proves final assistant metadata/text enrichment replaces the exact keyed durable note in place without moving it after a later tool note.",
          },
        ],
        verification:
          "pnpm exec vp test apps/openagents-desktop/src/renderer/local-harness.test.ts apps/openagents-desktop/src/renderer/shell.test.ts apps/openagents-desktop/src/thread-store.test.ts plus Desktop typecheck and build cover live projection, tool-card folding, durable keyed replacement, and host integration.",
      },
      {
        contractId: "openagents_desktop.chrome.command_notice_is_transient_toast.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "command notice chrome",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "what is that yellow command request shit at top, is that supposed to be there, fix if not",
        authorityBoundary:
          "The command notice (a duplicate/unavailable deferred-command rejection or a keybinding-save failure) presentation changes ONLY: from a permanent full-width top-edge caption banner that never cleared until the next successful command, to a compact, floated, token-styled warn TOAST that auto-dismisses on a bounded (~4.5s) Effect-scheduled clear and is dismissible immediately via a typed intent (× / click). The auto-clear is a forked Effect fiber (never a leaked raw setTimeout); a new notice cancels any prior pending clear; and the mount registers the controller's shutdown as a scope finalizer so a pending clear can never fire after unmount. The underlying rejection behavior is unchanged (CUT-15): the command is still rejected/ignored; only the notice is now transient, not permanent. No raw colors/px enter the renderer (apps-sdk chrome tokens + the design-conformance oracle), and the render stays commit-idempotent (the keyed toast is not re-parented on unrelated re-renders, so its enter animation never replays).",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/command-notice.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "command_notice.transient_controller_and_toast",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/command-notice.test.ts",
            description:
              "Drives the controller under Effect's TestClock (not wall-clock): a notice auto-clears exactly on the bounded delay boundary, a new notice cancels the prior pending clear (no early/double dismiss), the dismiss intent clears immediately and kills the pending timer, the view renders a warn Toast carrying the DesktopCommandNoticeDismissed intent (and nothing when clear), and the real intent registry clears the notice on that dismiss intent.",
          },
          {
            id: "command_notice.transient_rejection_smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke's command-duplicate-visible-rejection step asserts the duplicate command's rejection notice APPEARS as a dismissible toast (data-en-role=toast with a dismiss control) AND then auto-dismisses on its own bounded timer, rather than persisting as a permanent banner.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the command-notice controller/toast suite, the shell view suite, and the Electron smoke command-duplicate-visible-rejection step asserting appear-then-auto-dismiss.",
      },
      {
        contractId: "openagents_desktop.chat.details_affordance_visibility_is_pointer_only.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript chrome",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "the 'details' thing under message, when i hover it or its visible or whatever, it flashes back in every time i type something in the input, WHY??? WHY IS IT CONNECTED TO ANOTHER COMPONENT - fix it - and ensure that category of error wont happen anywhere else in codebase",
        authorityBoundary:
          "The per-message details affordance is a hover/focus reveal (opacity-0 at rest). Its visibility must be a pure function of pointer/focus ONLY — never of composer input, global state, or re-render timing. Root cause: the pure state->View re-render on every keystroke re-parented the persisted keyed affordance in the DOM renderer (Transcript wrappers were rebuilt from scratch and Stack children were unconditionally re-appended via replaceChildren); detaching + re-attaching a node restarts its CSS opacity transition, flashing it visible. Fix is structural, not cosmetic: (1) the resting opacity:0 is keyed on the affordance itself and no longer requires the [data-en-message] ancestor, so a momentary detach cannot expose it; (2) the shared render-dom commit is now idempotent — persisted keyed content is never re-parented on an unrelated re-render, killing the whole transition-replay category (also fixes the tool-title shimmer and disabled-reason popover).",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/app.css",
          "apps/openagents.com/packages/effect-native-render-dom/src/index.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
        ],
        oracles: [
          {
            id: "details_affordance.stable_on_composer_input.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke types into the composer while NOT hovering the message row and fails if the details affordance's computed opacity ever rises above 0, or if its DOM node is replaced or re-parented (sameNode/sameParent/restingOpacity=0/finalOpacity=0/maxOpacityDuringTyping=0).",
          },
          {
            id: "details_affordance.commit_idempotent_no_reparent",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents.com/packages/effect-native-render-dom/tests/index.test.ts",
            description:
              "Provider-agnostic render-dom guard: re-committing a Transcript whose only change is a sibling (the composer value) performs ZERO DOM moves of the persisted keyed hover-reveal affordance, so its CSS transition can never replay from an unrelated re-render.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the details-affordance-stable-on-composer-input Electron smoke step; pnpm exec vp test at apps/openagents.com/packages/effect-native-render-dom runs the commit-idempotency guard.",
      },
      {
        contractId: "openagents_desktop.chat.reading_flow_layout_stability.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript reading stability",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-screenshot-review", statedBy: "owner", statedOn: "2026-07-15" },
        statement:
          "hovering over a message shows its metadata along the top row but this changes the positioning of the message because the height of the metadata bar was never included before ... i dont want that top metadata bar at all ... content the user should be able to read easily jumps around because of hidden elements",
        authorityBoundary:
          "The React transcript never renders the removed top metadata row. Message metadata remains available through the existing stable details inspector and accessible item label. More generally, hover and focus selectors within readable transcript rows are paint-only: they may alter opacity, color, visibility, or an out-of-flow overlay, but may not change box dimensions, margin, padding, border width, type metrics, display, grid, or flex geometry. A hidden in-flow element may never become layout-bearing on hover/focus, so pointer movement cannot move prose, change the reader's scroll anchor, or shift neighboring messages.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/react-timeline.tsx",
          "packages/ui/src/desktop-workbench.css",
          "apps/openagents-desktop/src/react-conversation-assurance.test.ts",
          "apps/openagents-desktop/src/renderer/react-timeline.test.tsx",
        ],
        oracles: [
          {
            id: "reading_flow.no_top_metadata_bar",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-timeline.test.tsx",
            description:
              "Renders a real React message row and proves the removed metadata-bar node is absent while authored prose remains readable.",
          },
          {
            id: "reading_flow.hover_geometry_static_guard",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/react-conversation-assurance.test.ts",
            description:
              "Scans every transcript hover/focus CSS rule for layout-bearing declarations, rejects the former height:auto plus margin defect, includes a known-bad falsifier, and proves paint-only opacity/color reveals remain eligible.",
          },
        ],
        verification:
          "Focused React timeline and conversation assurance suites enforce the absent metadata node and the generalized no-hover-geometry rule; the normal Desktop verify sweep retains both tests.",
      },
      {
        contractId: "openagents_desktop.chat.compact_message_details_affordance.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript chrome",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "that metadata button needs to be way smaller and more like an icon button, not a huge ginormous circle.",
        authorityBoundary:
          "Only the details affordance presentation changes: it stays a real keyboard-focusable catalog Button dispatching the same typed DesktopMessageSelected intent with the same accessible label. The catalog IconButton's fixed 44px circle is simply no longer used for this affordance; no local UI primitive is introduced (ghost Button lowered via typed style tokens: zero padding, caption scale, muted color).",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "compact_details_affordance.not_icon_circle",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves every message row's details affordance is a ghost catalog Button (NOT the IconButton circle variant) with zero padding, caption type scale, and muted color, still dispatching DesktopMessageSelected with the message key.",
          },
          {
            id: "compact_details_affordance.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke fails if the rendered details affordance carries the icon-button variant or exceeds a compact line-height bound before opening the inspector through it.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the shell view suite and the Electron smoke message-inspector step with the compact-affordance guards.",
      },
      {
        contractId: "openagents_desktop.chat.typed_tool_call_cards.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript tool-call rendering",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "why don't you go improve the UI of those tool calls so it's not just JSON stuff? Like I thought we had some custom components that showed things properly, not these JSON blobs.",
        authorityBoundary:
          "Tool cards re-present the SAME bounded, redacted trace payloads the system notes already carried — no new data crosses the Electron boundary. One card per invocation updates in place from started to ok/failed (pairing is honest renderer-side toolName+order FIFO because the events carry no invocation id); the bounded raw args/result stay reachable behind a compact expand affordance and are never the default rendering; failure text renders as content. Tool cards drop the SYSTEM role label (the tool title is the header) but keep timestamps.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/tool-cards.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/react-timeline.tsx",
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "tool_cards.pairing_humanization_fallback",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/tool-cards.test.ts",
            description:
              "Table-driven humanization per known tool (delegate/Agent/Bash/Read/Write/Edit/Glob/Grep/ToolSearch/WebSearch/WebFetch), started+ok folding into one card, unknown-tool bounded compact fallback with no raw JSON, failed-state result text, and the text-parse fallback for pre-typed persisted notes.",
          },
          {
            id: "tool_cards.transcript_rendering",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the transcript renders one role=tool card per invocation with humanized title/detail, toned status chip, result line, no SYSTEM sender label, collapsed-by-default raw details behind the compact toggle, and the expand intent loop through the real registry.",
          },
          {
            id: "tool_cards.react_lifecycle_reconciliation",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-timeline.test.tsx",
            description:
              "Proves the React timeline consumes the shared tool-card reconciler: a local started+ok/failed pair retains the started-note key and command preview while updating one row's terminal status and result.",
          },
          {
            id: "tool_cards.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron fixture journey asserts the delegate invocation renders ONE updating card carrying the humanized task text and the child's answer, with no raw JSON args rendered by default anywhere in the transcript.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the shared tool-card projector, React timeline, shell view, and Electron smoke tool-card assertions.",
      },
      {
        contractId: "openagents_desktop.chat.interactive_question_cards.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript agent-question cards",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "make the question UI too. Why not? proper effect native primitives and add some if needed.",
        authorityBoundary:
          "Question cards render only the bounded typed question_pending payload from the FROZEN additive FableLocalEvent contract and answer only through the typed fableLocal.answerQuestion bridge in the frozen shape (answers: one { question, labels } entry per question, labels an array even for single-select; single-select dispatches on click, multiSelect toggles behind an explicit confirm). A bridge without answerQuestion renders read-only pending — the card never invents answer authority. Outcomes (answered/timeout/denied) come from question_resolved and render as dim resolved states; never raw JSON, never a SYSTEM label. Option rows compose catalog primitives (Button label + caption Text description); no local one-off primitives.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/chat-contract.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "question_cards.intent_loop_and_bridge",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Drives the real intent registry with a fake typed bridge: single-select option click dispatches the answer immediately in the frozen shape, multiSelect toggles then confirms, timeout/denied render dim resolved states, and an absent bridge renders disabled read-only options that dispatch nothing.",
          },
          {
            id: "question_cards.event_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/local-harness.test.ts",
            description:
              "Proves question_pending projects an interactive question note into the streaming transcript and question_resolved updates the same note in place with the runtime-authoritative outcome.",
          },
          {
            id: "question_cards.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron journey renders the fixture question as an interactive card (header chip, option labels, dim description, no SYSTEM label, no raw JSON), clicks an option through the REAL typed answerQuestion IPC, and proves the runtime's typed rejection reverts the card to honest pending with the selection retained.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the shell question-card suite, the local-harness projection suite, and the Electron smoke question-card step.",
      },
      {
        contractId: "openagents_desktop.chat.opencode_card_design_language.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript card design language",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Make a design pass through the projects/repos/opencode desktop app. any of its tool/message card formatting, we should port its tailwind stuff to our Effect Native, i want our component slooking just like theirs but adapted to our starcraft blue etc, and using the openai apps sdk icons we are.",
        authorityBoundary:
          "This is a presentation port only: opencode's card anatomy and density translate into typed Effect Native style objects on the shared tokens vocabulary — never Tailwind class strings (owner decision 2026-07-08), never local one-off primitives, never a light theme, and always our existing catalog icon set (apps-sdk-ui lineage), our Protoss-blue tokens, and our typed intents. No opencode code is vendored; the port provenance is recorded in-repo (docs/design-ports.md) with spec receipts.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/tool-cards.ts",
          "apps/openagents-desktop/docs/design-ports.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "opencode_card_design.structural_anatomy",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Asserts the opencode-derived structural properties on our cards: tool cards render the header icon + title + status-chip anatomy in that order, detail/result lines stack beneath, raw output is collapsed by default (no raw JSON in the default rendering), and styling is typed token style objects (no class strings anywhere in the card views).",
          },
          {
            id: "opencode_card_design.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke asserts the rendered tool cards carry the ported anatomy (one card per invocation, humanized header, no raw JSON default) in the uniform Protoss-blue theme.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the shell card suites and the Electron smoke; the design-port provenance note records the opencode source receipts.",
      },
      {
        contractId: "openagents_desktop.chat.no_assistant_role_label.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript chrome",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement: "Remove where it says assistant. I don't care about that.",
        authorityBoundary:
          "Only the visible ASSISTANT role header is removed from assistant transcript rows. Timestamps stay; the user YOU label and system SYSTEM label stay; the effective-model caption stays as its existing compact system trace line; typed role data on the message contract is unchanged and grants no new rendering authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "chat_no_assistant_label.note_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves noteMessage emits no senderLabel for assistant rows while keeping the timestamp, YOU on user rows, and SYSTEM on system rows.",
          },
          {
            id: "chat_no_assistant_label.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron fable streaming smoke asserts the finalized assistant row renders no sender chip.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the shell view suite and the Electron smoke journey asserting the label-free assistant row.",
      },
      {
        contractId: "openagents_desktop.chat.message_metadata_inspector.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat message metadata inspector",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-12" },
        statement: "when i load message details in right sidebar it doesnt scroll down, fix that.",
        authorityBoundary:
          "The inspector projects only bounded per-message metadata persisted by the host. When a new message selection appends details below a taller live-agent graph, the exact right rail becomes its own scroll owner and synchronously reveals a unique keyed marker immediately before the inspector. The reveal runs once per changed target after generic scroll restoration, never moves the transcript viewport, and does not keep pinning after the user scrolls manually. Selection remains a typed intent and grants no runtime, resume, or filesystem authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/chat-contract.ts",
          "apps/openagents-desktop/src/main.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "chat_message_inspector.intent_loop_and_fields",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Drives the real intent registry: details click selects the message, the right-side rail renders role/time/lane/model/account/turn/tokens/duration, Close and Escape deselect, and stale selections drop on thread switches.",
          },
          {
            id: "chat_message_inspector.keyed_scroll_reveal",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents.com/packages/effect-native-render-dom/tests/index.test.ts",
            description:
              "Proves a changed Stack scroll target becomes the exact overflow owner, reveals the keyed details marker after generic position restoration, clears its one-shot marker, and leaves later manual scrolling untouched.",
          },
          {
            id: "chat_message_inspector.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke clicks the streamed assistant message's details affordance and asserts the inspector shows the fixture's effective model, lane, account ref, and exact token total, then closes it.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the shell inspector suite and the Electron smoke message-metadata-inspector step.",
      },
      {
        contractId: "openagents_desktop.chat.no_composer_disabled_caption.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer harness lane affordances",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "I have no idea why the bottom says Codex requires Open Agent session. Don't put that shit in the UI ever. Remove that.",
        authorityBoundary:
          "Removing the caption never enables a dead lane: an unavailable chip stays visually disabled and refuses the action, and the reason string survives only in the chip's accessible label and host logs/journal. This does not weaken the evidence-gated composer or no-silent-substitution contracts.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/live-proof.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "chat_no_disabled_caption.composer_render",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves no caption node and no reason-bearing Text renders anywhere in the composer for any lane state, while the disabled chip keeps the reason as its accessible label and Send stays evidence-gated.",
          },
          {
            id: "chat_no_disabled_caption.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke asserts the disabled Codex chip carries its reason via aria-label only and that no standing caption text exists in the composer.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the composer suite and the Electron smoke; the live-proof driver journals the chip's disabled state + aria-label instead of any visible caption.",
      },
      {
        contractId: "openagents_desktop.chat.composer_stop_button.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer turn interruption",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "capability-audit", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "interrupt a running turn from the UI",
        authorityBoundary:
          "While a turn streams (pending), the composer's trailing icon-only Send is replaced by an icon-only Stop that dispatches the DesktopTurnInterrupted intent for the exact active thread. The handler signals that thread's already-plumbed local-lane interrupt IPC path and invents no terminal state — the runtime's typed `interrupted` result finalizes the turn and reverts the control to Send. An owner-requested Stop is neutral presentation: it creates neither a Turn failed banner nor an error timeline row, while the durable journal retains `owner_interrupted` truth. Stop grants no new authority: it cannot start a turn, route to another lane, or fabricate a completion, and a host without a matching local streaming lane simply no-ops.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/capability-registry.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "composer_stop_button.render_and_intent_loop",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the composer renders the icon-only Stop (and no Send) while pending and Send (no Stop) while idle, dispatches interruption only for the selected pending thread, and settles an owner interruption without an error row or failure banner state.",
          },
          {
            id: "composer_stop_button.interrupt_path",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/capability-evals.test.ts",
            description:
              "Drives the local-harness interruptActive seam headlessly against a fake lane bridge: a streaming turn's exact turnRef is signalled on the frozen interrupt channel, and the runtime's typed `interrupted` FableLocalEvent maps to a turn_failed reason that finalizes the turn.",
          },
          {
            id: "composer_stop_button.live_proof_step",
            kind: "visual-smoke",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/live-proof.ts",
            description:
              "The interrupt-stop live-proof step clicks Stop mid-turn in the real Electron window and journals the interrupted transcript state with a PNG receipt (rung-4; executed by the live-proof driver, not the headless sweep).",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the shell Stop-button suite and the capability-evals interrupt-path oracle; the interrupt-stop live-proof step is exercised by the live-proof driver run.",
      },
      {
        contractId: "openagents_desktop.chat.durable_runtime_turn_controls.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "durable runtime turn controls",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "issue-completion-criteria", statedBy: "owner", statedOn: "2026-07-12" },
        statement:
          "Provider questions, permission/tool approvals, plan/review transitions, interrupt, resume, retry, and cancel are first-class typed timeline items.",
        authorityBoundary:
          "CUT-16 (#8696) Desktop slice for the durable Khala runtime path. (1) The composer Stop button now also interrupts a DURABLE turn: the runtime conversation host implements interruptActive over the exact confirmed thread/run this renderer has in flight, dispatching conversation.interrupt through the protocol-v10 gateway with the confirmed run's expectedVersion. The acknowledgement is admission truth only — the confirmed canceled terminal (never the Stop handler) finalizes the turn and reverts the composer; a host with no in-flight durable send returns false and sends nothing. (2) Queue-until-idle now works on the durable path: a mid-turn submit enqueues a text follow-up that is promoted only at the previous turn's CONFIRMED terminal, as a real conversation.append plus conversation.start on the same lane; a refused enqueue restores the cleared draft instead of dropping text. If the app restarts after claiming a follow-up but before recording its provider receipt, that ambiguous promoting entry is quarantined as failed and is never replayed automatically; a possibly accepted user message must not be duplicated. Terminal queue history is not projected as an active composer queue. (3) Every control intent (chat Stop and fleet-cockpit pause/cancel/resume/retry/close) carries the EXACT confirmed run lane (claude_code→claude_pylon, codex/opencode_codex→codex_app_server, openagents_native→hosted_khala) as an additive optional gateway field threaded into the shared control-intent builders, because the durable authority's lane fence (runtime_target_lane_mismatch) rejects a mismatched target — the previous hard-coded Codex default made Claude/hosted turn controls unadmittable from Desktop. No schema, migration, server, or intent-contract change: openagents.khala_runtime_control_intent.v1 and protocol v10 are unchanged apart from the additive optional lane field.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/runtime-conversation.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/fleet-workspace.ts",
          "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          "apps/openagents-desktop/src/main.ts",
          "github:OpenAgentsInc/openagents#8696",
        ],
        oracles: [
          {
            id: "durable_runtime_turn_controls.interrupt_lane_exact",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-conversation.test.ts",
            description:
              "Proves interruptActive during an in-flight durable send dispatches conversation.interrupt with the exact threadRef/runRef, the lane derived from the confirmed run runtime (claude_code→claude_pylon), and the confirmed run version; returns true only on an admitted outcome; and returns false with no command when no durable send is in flight or the confirmed run is already terminal.",
          },
          {
            id: "durable_runtime_turn_controls.queue_until_idle_confirmed_drain",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-conversation.test.ts",
            description:
              "Proves a follow-up queued mid-turn is promoted only after the first run's confirmed terminal as a real append + start on the same lane, that the final thread carries both confirmed user messages, and that queueFollowup without an in-flight durable send reports queued:false and sends nothing.",
          },
          {
            id: "durable_runtime_turn_controls.queue_refusal_restores_draft",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the DesktopNoteSubmitted pending branch restores the cleared composer draft when the host reports queued:false, and leaves newer user input untouched.",
          },
          {
            id: "durable_runtime_turn_controls.interrupted_promotion_fails_closed",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-durable-queue.test.ts",
            description:
              "Proves a promotion left without a provider receipt across app restart is quarantined as failed, cannot be admitted or replayed, does not remain in the active composer queue, and does not block the next genuinely queued message.",
          },
          {
            id: "durable_runtime_turn_controls.gateway_lane_passthrough",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Proves the protocol-v10 gateway decodes the additive optional lane on conversation.interrupt/continue/retry/close, hands it to the runtime command service unchanged, rejects an unknown lane literal as invalid_request, and that main's control adapters thread input.lane into the shared control-intent context instead of the hard-coded Codex default (source oracle).",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the runtime-conversation control suite, the shell queue-refusal suite, and the gateway lane pass-through oracle in the normal sweep.",
      },
      {
        contractId: "openagents_desktop.chat.opencode_composer_shape.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat input composer layout",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-12" },
        statement:
          "Between codex/claude and reasoning dropdowns we need model selector. Codex select between GPT-5.6 and GPT-5.5, Claude select between Fable, Opus 4.8, Sonnet 5. Also that plus button make it not a huge circle, must be icon only.",
        authorityBoundary:
          "The single OpenCode-shaped composer card keeps its multiline input above one compact action bar. That bar orders compact icon-only Attach, Provider, provider-scoped Model, Codex-only Reasoning, account/permission controls, a flexible spacer, and circular Send/Stop. Claude model IDs remain closed typed values; Codex model IDs come only from the bounded visible catalog reported by the validated installed app-server, and exact selected IDs reach the corresponding provider launch field. No model is inferred from its display label and Claude refuses provider substitution before content. Attach uses the shared Effect Native IconButton's `sm` size (32px) with a required accessible label rather than inheriting the generic 44px circular action treatment. No attach, queue, stop, availability, or submission behavior is removed.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/docs/design-ports.md",
          "projects/repos/opencode/packages/app/src/components/prompt-input.tsx",
        ],
        oracles: [
          {
            id: "opencode_composer_shape.structural_layout",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the composer card holds a multiline shell-input above a bar ordered Attach, Provider, Model, Codex Reasoning, spacer, and Send/Stop; model options change with provider, exact selected IDs ride the next send, and Attach is the shared compact icon-only control.",
          },
          {
            id: "opencode_composer_shape.token_conformance",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
            description:
              "The design-conformance oracle holds on the composer: no raw color literals, spacing/radius values stay on shared scales, and Provider, Model, and Reasoning are compact native Select components.",
          },
          {
            id: "opencode_composer_shape.smoke",
            kind: "visual-smoke",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke renders the composer with input above the action bar and fails if the typed controls or trailing Send are missing.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the shell composer-layout suite, the design-conformance oracle, and the Electron smoke; pixel receipts of the empty/text/image/streaming composer states are captured under scratchpad ep250-composer-shots/.",
      },
      {
        contractId: "openagents_desktop.chat.composer_image_input.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer image input",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "capability-audit", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "attach a screenshot to a coding turn (image input)",
        authorityBoundary:
          "The active React composer carries a leading attach affordance plus drag-drop and paste-from-clipboard image attach. Accepted images are PNG/JPEG/WebP/GIF, bounded to at most 8 per message and 10 MB each; oversize, wrong-type, or unreadable files are rejected honestly with transient accessible copy (no standing caption). Acquisition batches are serialized so concurrent paste/drop cannot race the count bound. The renderer holds each attachment as bounded base64 and NEVER reads an arbitrary filesystem path — bytes come only from an in-renderer drop/paste File or a main-mediated native file picker. An idle image-only turn is valid; Steer and Queue remain text-only, and a failed send restores the attachments for retry. Fable sends images as SDK base64 image content blocks. The default Codex app-server lane writes bounded temporary files and sends `localImage` inputs; its exec fallback retains `-i <path>` lowering. Attaching grants no new authority: it starts no turn on its own, routes to no other lane, and reads no file the user did not hand the app.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/composer-images.ts",
          "apps/openagents-desktop/src/renderer/composer-image-acquisition.ts",
          "apps/openagents-desktop/src/renderer/react-composer.tsx",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/codex-local-runtime.ts",
          "apps/openagents-desktop/src/capability-registry.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "composer_image_input.decode_and_state",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/composer-images.test.ts",
            description:
              "Proves media-type/size classification with honest rejection copy, base64 decoding of an in-renderer File (drop/paste path), the ≤8 count bound, and the boundary projection that drops renderer-only fields.",
          },
          {
            id: "composer_image_input.render_and_intent_loop",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the active React composer renders the attach affordance and bounded thumbnails with remove, disables attach at the 8-image limit and while pending, surfaces accessible rejection copy and drag state, and that add/remove/submit through the real intent registry thread the image into the chat host (including image-only and failure-retry turns).",
          },
          {
            id: "composer_image_input.fable_sdk_block",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Drives the real Fable runtime prompt construction: a captured fake query proves an image turn lowers to an AsyncIterable user message whose content carries a { type:\"image\", source:{ type:\"base64\", media_type, data } } block (sdk.d.ts receipt), while a no-image turn keeps the plain string prompt.",
          },
          {
            id: "composer_image_input.codex_image_flag",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-local-runtime.test.ts",
            description:
              "Proves the retained Codex exec fallback writes each attachment into the turn workspace and passes it as `-i <path>`, terminated by `-C` before the positional prompt so the variadic --image never swallows the prompt; the built smoke separately receipts default app-server `localImage` lowering.",
          },
          {
            id: "composer_image_input.smoke_step",
            kind: "visual-smoke",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built React image-attach smoke drops a fixture PNG onto the real Electron composer, asserts its data-URL thumbnail without visible base64, submits an image-only turn, approves the provider request, verifies the preview clears, and requires a privacy-safe app-server receipt of exactly one localImage input without retaining its path or bytes.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the composer-images, shell, fable-local-runtime, and codex-local-runtime suites plus the image-attach smoke step; a real live provider image turn is deferred to a live-proof run.",
      },
      {
        contractId: "openagents_desktop.chat.markdown_rendering.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "assistant message markdown rendering",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "The markdown isn't rendered as markdown, so fix our fucking markdown rendering. I thought we had built a component for that.",
        authorityBoundary:
          "Assistant bodies parse a bounded markdown subset (headings, bold, italics, inline code, fenced code, lists, blockquotes, rules) into the typed catalog Markdown/CodeBlock/Divider views — text nodes only, no raw HTML is constructible, and links render as safe text, never navigation. User input stays literal. Mid-stream unterminated markers render as plain text until closed; re-parsing per append never throws.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/markdown.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "chat_markdown.projector_unit",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/markdown.test.ts",
            description:
              "Proves the bounded subset parses to typed blocks, hostile link schemes stay inert text, unterminated **/`/``` render gracefully mid-stream, and segments lower to Markdown/CodeBlock/Divider catalog views with stable keys.",
          },
          {
            id: "chat_markdown.assistant_body",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves assistant note bodies project through the markdown views while user text stays literal.",
          },
          {
            id: "chat_markdown.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron fable fixture journey streams a mid-marker-split **streaming** reply and asserts the final assistant body renders a real <strong> with no literal ** text.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the markdown projector suite, the shell view suite, and the Electron smoke markdown assertion.",
      },
      {
        contractId: "openagents_desktop.seam.replaceable_owned_correlated_services.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "replaceable service lifecycle and operation correlation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Desktop runtime, workspace, Sync, account, and history services are replaceable through the production host lifecycle; project, session, window, and app teardown closes exactly the resources it owns once, while one public-safe operation/session/run/correlation context survives the renderer-to-Sync path.",
        authorityBoundary:
          "Correlation carries bounded refs only and maps to private Sync causality refs; it never carries a path, URL, prompt, body, owner, token, credential, raw error, native handle, or provider payload. Replacement and disposal do not widen renderer or test-fixture authority.",
        seam: {
          client: "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          server: "apps/openagents-desktop/src/desktop-host-lifecycle.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/desktop-host-lifecycle.test.ts",
          "apps/openagents-desktop/src/desktop-operation-context.test.ts",
          "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
          "github:OpenAgentsInc/openagents#8684",
        ],
        oracles: [
          {
            id: "desktop_architecture.replaceable_owned_lifecycle",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/desktop-host-lifecycle.test.ts",
            description:
              "Uses the production lifecycle constructor with substitute runtime/workspace/Sync/account/history services and proves replacement, window close, app close, late-resource refusal, exact finalizer counts, and zero active slots.",
          },
          {
            id: "desktop_architecture.operation_correlation",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Preserves the same bounded operation/session/run/correlation refs through gateway observation, runtime command admission, Sync causality, response decoding, and the public-safe journal.",
          },
        ],
        verification:
          "The canonical Desktop verify gate runs lifecycle/correlation mutation and leak tests, builds Electron, executes the structured correlation path with substitute backing services, reloads the renderer, explicitly disposes the host, and requires active=0.",
      },
      {
        contractId: "openagents_desktop.seam.codex_trace_electron_acceptance.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "real Electron Codex trace acceptance",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-codex-session", statedBy: "owner", statedOn: "2026-07-10" },
        statement:
          "The built Electron app must render real owner-local Codex history as a stable named top-level catalog with nested agents, loss-accounted trace items, keyboard-operable topology, a reachable structured tool inspector, and ref-only selection restoration across a real renderer reload.",
        authorityBoundary:
          "The acceptance journey reads only the existing schema-bounded Runtime Gateway projection. Its receipt contains aggregate counts and timings only—never titles, transcript text, paths, raw JSONL, credentials, or stable private refs—and it grants no resume, write, sync, or provider execution authority.",
        seam: { client: "apps/openagents-desktop/src/electron-trace-acceptance.ts", server: "apps/openagents-desktop/src/main.ts" },
        evidenceRefs: [
          "apps/openagents-desktop/tests/electron-trace-acceptance.test.ts",
          "docs/sol/issues/desktop-codex-trace-acceptance.md",
          "github:OpenAgentsInc/openagents#8675",
        ],
        oracles: [{
          id: "codex_trace_real_electron_acceptance",
          kind: "bun-test",
          mode: "e2e",
          ref: "apps/openagents-desktop/src/electron-trace-acceptance.ts",
          description:
            "Runs inside built Electron against the real local Codex catalog, checks named/order-stable roots, child containment, nested topology, completeness, keyboard bindings, tool inspection, public-safe timings, and reload restoration.",
        }],
        verification:
          "pnpm --dir apps/openagents-desktop run verify builds Electron, runs the normal contract suite, executes the real-history journey, reloads the renderer, and fails on every named video-blocking regression.",
      },
      {
        contractId: "openagents_desktop.seam.identity.local_first_account_link.v1",
        state: "retired",
        surface: "openagents-desktop",
        productArea: "two-tier native identity",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Desktop creates one immutable device-local identity before OpenAuth and remains usable in local-only mode. A verified OpenAgents account link is additive and reversible; unlink, denial, failed link, and restart retain local-authority rows and never relabel them server-confirmed.",
        authorityBoundary:
          "Local identity and LocalRevision rows use separate host-owned tables and device-local scopes. Only server-verified owner input may create a link. The renderer receives identity tier only; local refs, owner refs, tokens, storage, rows, and transport remain host-only, and hosted Sync refuses device-local scopes.",
        seam: {
          client: "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          server: "apps/openagents-desktop/src/desktop-sync-host.ts",
        },
        evidenceRefs: [
          "packages/khala-sync/src/local-authority.ts",
          "packages/khala-sync-client/src/store-core.ts",
          "apps/openagents-desktop/src/renderer/settings.ts",
          "github:OpenAgentsInc/openagents#8666",
        ],
        oracles: [
          {
            id: "desktop_local_first_identity",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/local-first-identity.e2e.test.ts",
            description:
              "Proves local identity restart stability, verified account link, reversible unlink, local retention, and tokenless projection.",
          },
        ],
        verification:
          "Desktop host, Runtime Gateway, Settings, boundary, build, and Electron smoke run in the normal Desktop verify sweep.",
      },
      {
        contractId: "openagents_desktop.seam.codex_loss_accounted_history.v2",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local Codex history",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "OpenAgents Desktop discovers active and archived Codex history without an age ceiling, preserves the parent/child agent graph, and shows a bounded source-order page for the selected agent with explicit redaction and gap accounting. The source equation is visible and unknown/corrupt records are never silently dropped.",
        authorityBoundary:
          "The worker is read-only and returns only schema-bounded, credential-redacted catalog/page projections through Runtime Gateway v4. Source files, raw JSONL, encrypted reasoning, credentials, filesystem authority, session resume, cloud sync, and provider runtime authority never enter the renderer.",
        seam: {
          client: "apps/openagents-desktop/src/renderer/boot.ts",
          server: "apps/openagents-desktop/src/runtime-gateway.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-history-contract.ts",
          "apps/openagents-desktop/src/codex-history.ts",
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
        ],
        oracles: [
          {
            id: "codex_loss_accounted_history",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/codex-subagent-history.test.ts",
            description:
              "Proves nested history graph, source order, rich items, paging, archive compression, redaction, and visible gaps.",
          },
          {
            id: "codex_history_scale",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/codex-history-performance.e2e.test.ts",
            description:
              "Generates valid 100MiB/100-child/100k-item history and proves metadata-first discovery plus bounded pages.",
          },
        ],
        verification:
          "The Desktop verify sweep runs projection, gateway, Effect Native view, scale, boundary, build, and Electron smoke evidence.",
      },
      {
        contractId:
          "openagents_desktop.chat.thread_first_content_under_50ms.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "thread loading performance",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Threads must always show their first bounded message content in less than 50 milliseconds, regardless of total rollout size. Large threads must be chunked; full-rollout parsing is forbidden on the selection path.",
        authorityBoundary:
          "The 50-millisecond budget covers local first-content projection after selection. It does not authorize loading unbounded history, exposing raw events, or moving filesystem work onto Electron's main process.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-history.ts",
          "apps/openagents-desktop/src/codex-history-worker.ts",
        ],
        oracles: [
          {
            id: "oversized_rollout_first_content.performance",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/codex-history.e2e.test.ts",
            description:
              "Creates a 256 MiB sparse rollout and requires bounded first-content projection to finish under 50 ms.",
          },
        ],
        verification:
          "pnpm exec vp test apps/openagents-desktop/tests/codex-history.e2e.test.ts enforces the 50 ms wall-clock budget in the normal desktop test sweep.",
      },
      {
        contractId:
          "openagents_desktop.seam.runtime_gateway_closed_protocol.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Desktop Runtime Gateway",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "The signed Desktop renderer reaches host runtime state through one versioned closed query/command/event protocol. Protocol v8 includes bounded provider-native Codex history, canonical confirmed conversation/timeline/graph/command-outcome projections, and exact-ref start/interrupt commands; unknown requests fail schema decoding, unavailable or pending commands never appear completed, expired commands never dispatch, lifecycle events are ordered and disposable, and the renderer never receives runtime credentials or a generic transport.",
        authorityBoundary:
          "Electron main owns the Runtime Gateway and validates the invoking top-level bundled renderer. The renderer may request bounded OpenAgents session entry/exit and canonical conversation operations but receives only typed projections/outcomes; it gets no credential, callback/authorize URL, raw Khala Sync/store/session/transport authority, provider credential, raw IPC channel, MessagePort, filesystem handle, process handle, or raw runtime event.",
        seam: {
          client: "apps/openagents-desktop/src/preload.cts",
          server: "apps/openagents-desktop/src/runtime-gateway.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          "apps/openagents-desktop/tests/electron-boundary.test.ts",
        ],
        oracles: [
          {
            id: "runtime_gateway_closed_protocol.e2e",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Round-trips schema-decoded renderer requests and proves truthful capability, unavailable command, lifecycle ordering, disposal, and rejection behavior.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the seam suite, mechanical boundary oracle, bundle, and real Electron bootstrap smoke.",
      },
      {
        contractId: "openagents_desktop.sync.host_owned_sqlite.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Khala Sync local persistence",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Desktop opens the shared Khala Sync SQLite store inside Electron main, persists one installation identity across restart, migrates the supported legacy store without data loss, refuses a newer store before mutation with recovery guidance, and after native-session verification composes the shared HTTP/WebSocket session on exactly the server-derived owner's personal scope. Sparse event batches replay from the durable cursor; rotation is re-read host-side and the session closes before the store.",
        authorityBoundary:
          "The renderer receives only bounded phase and freshness. Owner refs, credentials, database path and handle, installation identity refs, rows, mutation queue, transport, and session remain host-only. The local database is a reconstructible cache/offline queue and never server authority; authenticated substrate is not an authoritative conversation projection.",
        evidenceRefs: [
          "apps/openagents-desktop/src/desktop-sync-host.ts",
          "apps/openagents-desktop/src/desktop-sync-store.ts",
          "packages/khala-sync-client/src/store-core.ts",
        ],
        oracles: [
          {
            id: "desktop_sync_host.lifecycle",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-sync-host.test.ts",
            description:
              "Proves restart-stable identity, private permissions, supported legacy migration, newer-version refusal, personal-scope selection, dynamic token lookup, live/freshness transition, session-before-store close, and reuse of the shared SQLite store.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the host lifecycle suite and real Electron gateway bootstrap.",
      },
      {
        contractId: "openagents_desktop.sync.native_conversation_continuity.v1",
        state: "enforced",
        surface: "openagents-desktop-and-mobile-hosts",
        productArea: "authoritative cross-device conversation continuity",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "A Desktop-created canonical chat_thread and first chat_message can be confirmed on mobile, mobile can append one canonical follow-up, and both native hosts converge on identical public-safe refs, confirmed entity versions, thread cursor, and live phase, then reconstruct the same state after restart without duplicates.",
        authorityBoundary:
          "Only server-confirmed chat_thread/chat_message rows enter the bounded conversation projection. Owner identity, credentials, store/session/overlay/transport objects, optimistic bodies, provider runtime events, and assistant-role inference remain host-only or explicitly outside this contract; denial and sign-out remove the conversation capability.",
        evidenceRefs: [
          "packages/khala-sync-client/src/chat.ts",
          "packages/khala-sync-client/src/conversation.ts",
          "apps/openagents-desktop/src/desktop-sync-host.ts",
          "apps/openagents-mobile/src/sync/mobile-sync-host-core.ts",
          "docs/sol/issues/native-conversation-continuation.md",
          "github:OpenAgentsInc/openagents#8668",
        ],
        oracles: [
          {
            id: "native_timeline_fault_convergence.e2e",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/native-timeline-fault-convergence.e2e.test.ts",
            description:
              "Feeds reordered/duplicate timeline rows and an authoritative gap snapshot through the Desktop and Expo/mobile SQLite adapters, proving byte-equivalent refs, versions, cursors, and retractions.",
          },
          {
            id: "native_conversation_continuation.e2e",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/native-conversation-continuation.e2e.test.ts",
            description:
              "Runs the real Desktop node:sqlite host and mobile Expo-SQLite host over the shared session/overlay protocol against a server-authoritative chat fake, then proves Desktop-to-mobile-to-Desktop convergence, exact versions/cursor, and restart reconstruction.",
          },
        ],
        verification:
          "The native conversation continuation e2e runs in the normal Desktop sweep; shared chat mutator/projection tests run in the khala-sync-client sweep and the collection package regression proves existing consumers use the same centralized mutators.",
      },
      {
        contractId: "openagents_desktop.seam.runtime_gateway_conversation.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "authoritative conversation Runtime Gateway",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "The signed Desktop renderer can query confirmed canonical conversation catalogs, threads, their current agent timeline, and durable command outcomes, and enqueue canonical create/append plus exact thread/message/run start or interrupt commands only through Runtime Gateway protocol v8. Enqueues return pending_reconcile or unknown_pending_reconcile with the durable mutation id, never optimistic completed; expired commands are terminal and never execute after reconnect.",
        authorityBoundary:
          "The seam carries public-safe thread/message/run/WorkContext refs, bounded canonical timeline items, timestamps, confirmed entity versions, exact scope phase/cursor, and pending count only. It carries no owner identity, credential, store/session/overlay/transport, generic IPC, raw provider stream, or process authority; not-live/read failure is typed and body-free.",
        seam: {
          client: "apps/openagents-desktop/src/preload.cts",
          server: "apps/openagents-desktop/src/runtime-gateway.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          "apps/openagents-desktop/src/main.ts",
          "packages/khala-sync-server/src/runtime-mutators.ts",
          "apps/pylon/src/orchestration/runtime-intent-enforcement.ts",
          "apps/openagents-mobile/src/conversation/mobile-conversation.ts",
          "docs/sol/issues/native-streamed-conversation-handoff.md",
          "docs/sol/issues/desktop-runtime-conversation.md",
          "github:OpenAgentsInc/openagents#8669",
        ],
        oracles: [
          {
            id: "runtime_gateway_conversation.e2e",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Round-trips confirmed catalog/thread/timeline/command-outcome projections and exact create/append/start/interrupt refs through protocol v8, proves pending-reconcile and terminal-expiry outcomes, unavailable fail-closed behavior, bounds, and schema rejection.",
          },
          {
            id: "runtime_agent_run_transactional_binding",
            kind: "bun-test",
            mode: "e2e",
            ref: "packages/khala-sync-server/src/runtime-mutators.test.ts",
            description:
              "Against real local Postgres, proves durable start admission transactionally creates the canonical agent run, exact semantic retry reconciles, conflicting reuse fails closed, WorkContext/repository snapshot stays immutable, and runtime events preserve exact thread/run refs.",
          },
          {
            id: "runtime_provider_single_generation_claim",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/pylon/src/orchestration/runtime-intent-enforcement.test.ts",
            description:
              "Races two independent consumers against one admitted turn and proves only the winner of the durable sequence-one event claim invokes Codex.",
          },
          {
            id: "mobile_same_thread_runtime_control",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/mobile-conversation.test.ts",
            description:
              "Proves mobile submits the same confirmed thread/message/run refs through the shared runtime builders, observes a later confirmed terminal projection, and interrupts only the exact confirmed run.",
          },
        ],
        verification:
          "Runtime Gateway e2e, Electron mechanical boundary, Desktop typecheck/build, and behavior-contract validation run in the normal Desktop sweep.",
      },
      {
        contractId: "openagents_desktop.seam.runtime_gateway_agent_timeline.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "confirmed provider-neutral agent timeline gateway",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Runtime Gateway protocol v8 accepts bounded agent.timeline by exact runRef, conversation.timeline by exact threadRef, and conversation.commandOutcome by exact stable intentId/threadRef, returning only confirmed server projections with bounded canonical timeline items; unavailable, not-found, and read failure remain typed and body-free.",
        authorityBoundary:
          "Electron main composes the shared confirmed timeline reader only behind authenticated live Sync. The server-projected agent_run.routeId is the sole route/thread binding; renderer code cannot derive it from runRef. The seam may expose bounded runtime/backend/WorkContext classification but never owner/objective/repository contents, provider source, raw payload, external callback, auth/store/session/transport, generic IPC, or process authority.",
        seam: {
          client: "apps/openagents-desktop/src/preload.cts",
          server: "apps/openagents-desktop/src/runtime-gateway.ts",
        },
        evidenceRefs: [
          "packages/khala-sync-client/src/agent-timeline.ts",
          "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          "apps/openagents-desktop/src/main.ts",
          "docs/sol/issues/desktop-runtime-agent-timeline.md",
          "github:OpenAgentsInc/openagents#8673",
        ],
        oracles: [
          {
            id: "runtime_gateway_agent_timeline.e2e",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Round-trips the exact agent.timeline query and confirmed run/route/event projection through both schema boundaries, proves server routeRef preservation, body-free non-live failure, public-field bounds, and invalid-ref rejection.",
          },
        ],
        verification:
          "Runtime Gateway e2e, Electron boundary, Desktop host/typecheck/build, shared timeline, and behavior-contract validation run in the normal sweeps.",
      },
      {
        contractId: "openagents_desktop.seam.runtime_gateway_live_agent_graph.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "confirmed live-agent graph gateway",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "Runtime Gateway protocol v8 emits server-confirmed openagents.live_agent_graph.v1 post-images inside the existing cursor-aware thread subscription. Exact resume emits the current bounded graph set, a proven cursor gap emits one authoritative-refetch snapshot, and interrupted or non-live scopes emit no cached graph authority.",
        authorityBoundary:
          "Electron main reads graph entities only from the authenticated canonical thread scope. Each event carries matching graphRefs plus at most eight validated post-images, with at most 2,000 nodes and 4,000 edges in aggregate. Provider-native history, raw callbacks, credentials, store/session/transport handles, and process authority never cross the renderer seam; unsupported graph facts remain explicit unknowns.",
        seam: {
          client: "apps/openagents-desktop/src/preload.cts",
          server: "apps/openagents-desktop/src/runtime-gateway.ts",
        },
        evidenceRefs: [
          "packages/khala-sync-client/src/live-agent-graph.ts",
          "packages/khala-sync-client/src/live-conversation.ts",
          "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          "apps/openagents-desktop/src/runtime-live-subscriptions.ts",
          "github:OpenAgentsInc/openagents#8691",
        ],
        oracles: [
          {
            id: "confirmed_live_agent_graph_read_model",
            kind: "bun-test",
            mode: "unit",
            ref: "packages/khala-sync-client/src/live-agent-graph.test.ts",
            description:
              "Reads only graph-valid post-images from the exact live thread scope, hides cached rows while non-live, and bounds a busy thread to the newest aggregate-safe graph set.",
          },
          {
            id: "runtime_gateway_live_agent_graph_reconnect",
            kind: "bun-test",
            mode: "e2e",
            ref: "packages/khala-sync-client/src/live-conversation.test.ts",
            description:
              "Proves graph refs and post-images ride confirmed resume and authoritative-refetch snapshots through the same durable cursor and bounded subscription path without polling.",
          },
          {
            id: "runtime_gateway_live_agent_graph_boundary",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Round-trips a bounded canonical graph post-image through both protocol-v8 schema boundaries and advertises agent-graph capability only with live Sync.",
          },
        ],
        verification:
          "Khala Sync client graph/live-conversation tests and typecheck plus Runtime Gateway e2e, Desktop typecheck/build, and behavior-contract validation run in the normal sweeps.",
      },
      {
        contractId: "openagents_desktop.chat.authoritative_sync_mode.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "visible authoritative conversation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "At boot, the Effect Native Desktop shell selects exactly one chat authority: Runtime Gateway v8 confirmed Sync when its catalog is live, otherwise the existing explicit local-only host. In Sync mode, visible threads/messages, durable command outcomes, and bounded assistant lifecycle items come from confirmed projections; create/append/start remain pending until exact refs and terminal state reconcile.",
        authorityBoundary:
          "Mode is selected once per renderer lifetime so local and account-linked conversations never mix. The adapter uses only the generic decoded Runtime Gateway call; it gets no owner/credential/native authority, does not add preload IPC, does not infer assistant roles, and reports an unconfirmed append as still pending rather than completed.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/runtime-conversation.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "docs/sol/issues/desktop-visible-sync-conversation.md",
          "github:OpenAgentsInc/openagents#8670",
        ],
        oracles: [
          {
            id: "desktop_authoritative_sync_mode.adapter",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-conversation.test.ts",
            description:
              "Proves one-time live-vs-local selection, confirmed catalog/transcript mapping, stable client refs, create/append exact-ref confirmation, and pending timeout honesty.",
          },
        ],
        verification:
          "The adapter, shell, Runtime Gateway, Electron boundary, typecheck, bundle, and behavior-contract validation run in the normal Desktop sweep.",
      },
      {
        contractId: "openagents_desktop.session.os_encrypted_custody.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "native OpenAgents session custody",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Desktop keeps the native OpenAgents access token, refresh token, and server-derived owner ref in one versioned Electron safeStorage-encrypted record under its private userData root; recovered credentials remain unverified until the server accepts them.",
        authorityBoundary:
          "OS encryption and private disk custody do not verify the credential, authorize Khala Sync rows or commands, create a device_session, or expose any credential field to preload, renderer, Runtime Gateway, logs, receipts, or public errors.",
        evidenceRefs: [
          "apps/openagents-desktop/src/desktop-session-vault.ts",
          "docs/sol/issues/desktop-session-vault.md",
          "github:OpenAgentsInc/openagents#8661",
        ],
        oracles: [
          {
            id: "desktop_session_vault.os_encrypted_custody",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-session-vault.test.ts",
            description:
              "Proves safeStorage encryption, private atomic persistence, encryption/backend refusal, malformed-record purge, bounded recovery, idempotent clear, and public-safe failures.",
          },
        ],
        verification:
          "The desktop-session-vault and Electron-boundary suites prove custody and renderer isolation; Desktop typecheck/build and behavior-contract validation gate the integration.",
      },
      {
        contractId:
          "openagents_desktop.session.recovered_validation_rotation.v1",
        state: "retired",
        surface: "openagents-desktop",
        productArea: "native OpenAgents session recovery",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Retired from ordinary startup on 2026-07-17: opening Desktop must not initialize OS credential custody or validate recovered native credentials. Secure custody remains reachable only after an explicit account action.",
        authorityBoundary:
          "The retained recovery module and its unit oracle do not authorize invocation during startup. Ordinary launch remains local-only and Keychain-free.",
        evidenceRefs: [
          "apps/openagents-desktop/src/desktop-session-recovery.ts",
          "docs/sol/issues/desktop-session-recovery.md",
          "github:OpenAgentsInc/openagents#8662",
        ],
        oracles: [
          {
            id: "desktop_session_recovery.validation_rotation",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-session-recovery.test.ts",
            description:
              "Proves exact native-session verification, rotation-before-ready, denial/owner-mismatch purge, transient retention, and tokenless results.",
          },
          {
            id: "desktop_session_recovery.gateway_projection",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Proves the renderer-facing Runtime Gateway projects only bounded verified readiness without owner or credential fields.",
          },
        ],
        verification:
          "Desktop recovery, Runtime Gateway, and Electron-boundary suites plus typecheck/build enforce both sides of the host-only validation seam.",
      },
      {
        contractId: "openagents_desktop.session.loopback_pkce_entry_exit.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "native OpenAgents session entry and exit",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Desktop main binds a temporary literal-loopback listener, launches the exact public-client GitHub code + S256 request, validates callback state, verifies the server owner before encrypted custody, and revokes both credential classes before local sign-out.",
        authorityBoundary:
          "The Runtime Gateway accepts only argument-free session entry/exit commands and returns bounded completed/cancelled/unavailable phase. No callback, authorize URL, state, code, verifier, owner, access token, or refresh token enters preload, renderer, logs, receipts, or public errors; verified session is not live Sync.",
        evidenceRefs: [
          "apps/openagents-desktop/src/desktop-session-pkce.ts",
          "docs/sol/issues/desktop-session-pkce.md",
          "github:OpenAgentsInc/openagents#8664",
        ],
        oracles: [
          {
            id: "desktop_session_pkce.loopback_entry_exit",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-session-pkce.test.ts",
            description:
              "Proves literal-loopback lifecycle, callback state, public-safe response, exact authorize/exchange tuples, server owner verification, immediate rotation, timeout/cancel, and dual revocation before clear.",
          },
          {
            id: "desktop_session_pkce.gateway_commands",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Proves argument-free session commands round-trip through the closed Runtime Gateway and return only bounded phase outcomes.",
          },
        ],
        verification:
          "Desktop PKCE, Runtime Gateway, and Electron-boundary suites plus typecheck/build enforce host composition without a live browser or GUI in tests.",
      },
      {
        contractId: "openagents_desktop.session.effect_native_controls.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "native OpenAgents session UI",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Desktop Settings renders the exact bounded OpenAgents session phase and routes visible sign-in/sign-out controls through typed Effect Native intents to argument-free Runtime Gateway commands, with honest in-flight and unavailable states.",
        authorityBoundary:
          "The renderer sees only signed-out, unverified, session-ready, denied, unavailable, or local authenticating presentation state. It receives no callback/authorize URL, state, code, verifier, owner, token, storage handle, or live Sync authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/settings.ts",
          "docs/sol/issues/desktop-session-controls.md",
          "github:OpenAgentsInc/openagents#8665",
        ],
        oracles: [
          {
            id: "desktop_session_controls.effect_native_intents",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/settings.test.ts",
            description:
              "Drives the real Effect Native Settings view and intent registry through signed-out, authenticating, session-ready, sign-in, and sign-out states with a tokenless fake bridge.",
          },
          {
            id: "desktop_session_controls.gateway_phase",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Proves bootstrap carries an explicit bounded session phase and session commands remain argument-free and single-flight.",
          },
        ],
        verification:
          "Settings view/intent, Runtime Gateway, and Electron-boundary suites plus Desktop typecheck/build enforce the visible tokenless path without GUI automation.",
      },
      {
        contractId: "openagents_desktop.settings.acp_peer_truth_and_recovery.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Grok and Cursor local Agent Client Protocol settings",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-16" },
        statement:
          "Desktop presents Grok CLI and Cursor Agent CLI as distinct Agent Client Protocol peers, derives controls from probe/profile/advertised capability truth, distinguishes authentication and session recovery outcomes, and never generalizes either peer's evidence to every ACP agent.",
        authorityBoundary:
          "Trusted peer runtimes retain executable identity, version/conformance admission, secret custody, authentication, workspace/process/session lifecycle, reverse authority, cancellation escalation, and recovery. The renderer accepts only a bounded schema-decoded projection and a closed provider/action pair, re-checks every action against the currently rendered state, and cannot infer authentication from files or environment variables. Filesystem and terminal are shown active only when their session brokers are active. The support projection structurally excludes executable paths, auth method payloads, environment, prompts, file/terminal content, and native events; release language remains matrix-gated by #8897.",
        evidenceRefs: [
          "apps/openagents-desktop/src/acp-provider-contract.ts",
          "apps/openagents-desktop/src/acp-provider-host.ts",
          "apps/openagents-desktop/src/renderer/acp-provider-settings.ts",
          "packages/agent-client-protocol/src/profiles/grok.ts",
          "packages/agent-client-protocol/src/profiles/cursor.ts",
          "github:OpenAgentsInc/openagents#8895",
        ],
        oracles: [
          {
            id: "acp_settings.peer_specific_state_and_controls",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/acp-provider-settings.test.ts",
            description:
              "Proves exact Grok/Cursor identity, Agent Client Protocol copy, advertised-auth-derived actions, Cursor login terminal states, Grok cached/API-key method representation, stable versus extension configuration provenance, authority withholding, cancellation escalation, recovery, accessibility labels, and no universal-agent claim.",
          },
          {
            id: "acp_settings.structural_support_bundle_redaction",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/acp-provider-host.test.ts",
            description:
              "Proves the main-owned support bundle carries only profile/version/state/capability/conformance and bounded receipt/evidence refs and excludes executable paths, auth methods and responses, configuration values, prompts, environment, and native content.",
          },
        ],
        verification:
          "Desktop typecheck and focused ACP provider host/settings, Settings intent-loop, UX contract, and design-conformance suites. Supported release labels remain blocked on the live compatibility matrix in #8897.",
      },
      {
        contractId: "openagents_desktop.chat.same_components_across_provider_lanes.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "provider-neutral local turn workbench",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-16" },
        statement:
          "Codex, Claude, ACP peers, and future agent lanes project plan, reasoning, tool/exec, approval/question, item-delta, and usage facts through one typed provider-lane envelope into the same renderer workbench components; a lane may not introduce a private transcript renderer or silently discard degradation.",
        authorityBoundary:
          "Electron main owns dispatch, durable journal state, host history, usage attribution, and restart disposition. The renderer receives only the existing bounded local-lane event envelope and gains no raw provider payload, credential, filesystem, process, transport, or provider-session authority. Facts without an exact shared-envelope counterpart stay on their typed sidecar/capability surface; degraded projection is always visible.",
        evidenceRefs: [
          "apps/openagents-desktop/src/provider-lane.ts",
          "apps/openagents-desktop/src/provider-lane-acp.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "github:OpenAgentsInc/openagents#8899",
        ],
        oracles: [
          {
            id: "provider_lanes.fixture_shared_dispatch_and_renderer",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/provider-lane.test.ts",
            description:
              "Runs a never-hand-wired fixture lane through the shared dispatcher, durable journal, exact usage attribution, restart refusal, and the real shared renderer projection while requiring every forwarded event to decode against the frozen envelope.",
          },
          {
            id: "provider_lanes.acp_canonical_mapping",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/provider-lane-acp.test.ts",
            description:
              "Proves ACP canonical text, reasoning, tool, plan, degradation, and exact usage facts map into the shared envelope without fabricated token fields or a provider-private renderer vocabulary.",
          },
        ],
        verification:
          "The Desktop test sweep runs the fixture-lane shared-dispatch/renderer oracle, ACP mapping oracle, local-turn recovery suite, both built-in lane runtime suites, and Desktop typecheck.",
      },
      {
        contractId: "openagents_desktop.chat.fable_local_lane_no_substitution.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local-mode composer harness lanes",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "In local (not-signed-in) mode, selecting Claude runs a real streaming turn on this machine with zero login: the default selection prefers the user's currently authenticated local Claude Code session, then falls back to ready isolated sibling Pylon Claude homes; explicitly selecting a named Pylon account pins it. Selecting a harness never routes to the cloud gateway or another provider; an unavailable lane renders a disabled chip with its reason and a Send that does not accept the action.",
        authorityBoundary:
          "The renderer receives only bounded, path-redacted typed stream events and typed availability/failure reasons — never tokens, account homes, credentials, raw SDK payloads, or provider error bodies. Main owns thread history; the renderer supplies only the new message.",
        evidenceRefs: [
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fable_local.runtime_isolation_and_rotation",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Proves current local Claude session precedence, isolated Pylon fallback discovery, read-only headless SDK options, bounded/redacted event mapping, same-lane account rotation only before content, and that no ready account yields a typed unavailable result with the SDK never loaded.",
          },
          {
            id: "fable_local.renderer_no_silent_substitution",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/local-harness.test.ts",
            description:
              "Proves fable sends stream progressively with trace lines, codex and unavailable-fable sends are typed refusals, and the legacy gateway sendMessage is reachable only by a laneless send.",
          },
          {
            id: "fable_local.evidence_gated_composer",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves unavailable lanes render disabled chips with visible captions, Send disables for the selected dead lane, submit refuses while keeping the draft, and selection auto-moves off a dead default.",
          },
        ],
        verification:
          "The Desktop verify gate runs the runtime/renderer/shell suites and the fixture-driven smoke journey (select Fable, send, observe progressive text, tool trace, finalize) inside built Electron.",
      },
      {
        contractId: "openagents_desktop.chat.fable_local_lane_no_substitution.v2",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local-mode composer harness lanes",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "MODEL PIN (user-facing brand is \"Claude\" per owner 2026-07-12, because Fable may not always be the available model; the pinned model is unchanged): the local Claude lane requests model claude-fable-5 with skills removed from the lane (disallowed, never offered-then-denied); if the SDK init reports an effective model outside the claude-fable family the turn fails typed as model_substituted naming requested vs effective, no substituted output is ever streamed or persisted as Claude, and the lane never rotates accounts on a model mismatch; the effective model is emitted as a typed event and displayed with the reply (e.g. Claude · claude-fable-5), never asserted from the brand alone.",
        authorityBoundary:
          "Model identity crossing the Electron boundary is only the bounded SDK-reported effective-model string and the typed model_substituted reason with a bounded requested-vs-effective detail — never raw SDK payloads, account homes, or provider error bodies. Model-level refusal grants no rotation, retry, or gateway-fallback authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fable_local.model_no_substitution_runtime",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Proves SDK options carry model claude-fable-5 plus disallowedTools Skill and skills off; an init reporting a non-Fable model yields a typed model_substituted failure naming requested vs effective with zero assistant deltas surfaced and no account rotation; a claude-fable init (including versioned IDs) streams normally and emits the effective model.",
          },
          {
            id: "fable_local.model_effective_caption",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/local-harness.test.ts",
            description:
              "Proves the renderer displays the SDK-reported effective model as the Claude · <model> transcript caption from the typed model_effective event, positioned with the trace lines the persisted thread also carries.",
          },
        ],
        verification:
          "The Desktop verify gate runs the runtime and renderer suites covering model-level substitution refusal and the effective-model caption; the fixture smoke journey streams with the fixture init reporting claude-fable-5.",
      },
      // =====================================================================
      // Lane C (#8712) — Codex delegation. Kept as a clearly separate block:
      // a parallel chat-UI lane also edits this registry; the coordinator
      // owns resolving the registry-version bump on merge.
      // =====================================================================
      {
        contractId: "openagents_desktop.seam.codex_delegation_no_substitution.v1",
        state: "retired",
        surface: "openagents-desktop",
        productArea: "Fable-to-Codex sub-agent delegation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "The local Fable lane may delegate bounded tasks to Codex sub-agents only through the typed mcp__codex__delegate tool: every child is requested pinned to model gpt-5.5 at medium reasoning effort as spawn-config truth (the codex exec --json stream does not echo model or effort, and every result and ledger row is labeled requested accordingly); children run read-only in isolated scratch workspaces on registry-isolated Codex account homes, never the default ~/.codex; a revoked-credential account is never silently skipped — rotation emits a typed account_reconnect_required event per skipped account, and when every registered account is revoked the delegation returns a typed unavailable result naming the reconnect need; at most 3 children run concurrently and 6 per turn, with over-cap calls refused typed before any spawn; exact per-child token usage from turn.completed (total = input + output + reasoning) rolls into the session usage ledger and the Fleet view's evidence-labeled Session usage section; and a session-observed revoked credential or failed usage probe supersedes the registry's presence-based ready with a typed reconnect-required readiness state.",
        authorityBoundary:
          "RETIRED 2026-07-11, superseded by v2 same day: (a) the owner full-access override replaced the read-only child sandbox with danger-full-access (owner sign-off, verbatim: 'disallowing bash is retarded, give them full tools full permissions etc'); (b) the live EP250 rotation miss (the SHORT auth variant 'Your access token could not be refreshed. Please log out and sign in again.' carried none of v1's markers, so no rotation happened) broadened the auth classifier, added typed pre-content rotation, and added the in-process account health ordering. Kept for history.",
        seam: {
          client: "apps/openagents-desktop/src/renderer/fleet-workspace.ts",
          server: "apps/openagents-desktop/src/codex-child-runtime.ts",
        },
        evidenceRefs: [
          "contract:openagents_desktop.seam.codex_delegation_no_substitution.v2",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_delegation.child_runtime_rotation_and_usage",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-child-runtime.test.ts",
            description:
              "HISTORICAL (retired state skips coverage): proved the v1 read-only spawn recipe and marker-set rotation. Replaced by the v2 oracles below.",
          },
        ],
        verification:
          "Retired — superseded by openagents_desktop.seam.codex_delegation_no_substitution.v2; see that contract's verification.",
      },
      {
        contractId: "openagents_desktop.seam.codex_delegation_no_substitution.v2",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Fable-to-Codex sub-agent delegation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "The local Fable lane may delegate bounded tasks to Codex sub-agents only through the typed mcp__codex__delegate tool: every child is requested pinned to model gpt-5.5 at medium reasoning effort as spawn-config truth (labeled requested; the codex exec --json stream does not echo model or effort); children run with the owner-local danger-full-access profile in isolated per-child scratch workspaces, preferring the ordinary authenticated local Codex session and using registry-isolated Codex homes as fallback, and the tool description tells Fable the child STARTS in an empty scratch directory so absolute paths must be included for anything it should read; a failing session is never silently skipped — auth-class failures (broadened marker set including the live SHORT variant 'Your access token could not be refreshed. Please log out and sign in again.') rotate with a typed account_reconnect_required event and demote the session in the in-process health memory, any other pre-content failure rotates with a typed pre_content_failure_rotated event, post-content failures and timeouts fail the child without rotation; candidate ordering per call is last-known-good first, then untried, then auth-failed last (a success clears the mark); when every session is exhausted the delegation returns a typed failure naming the reconnect need (all-auth) or the failure mix; at most 3 children run concurrently and 6 per turn, with over-cap calls refused typed before any spawn; exact per-child token usage from turn.completed (total = input + output + reasoning) rolls into the session usage ledger and the Fleet view's evidence-labeled Session usage section; and a session-observed auth failure or failed usage probe supersedes registry presence with a typed reconnect-required readiness state.",
        authorityBoundary:
          "The renderer receives only bounded typed child lifecycle events (childRef, account ref, public-safe summaries with the child workspace redacted, exact token counts, typed failure reasons/rotation activities) and the typed session-ledger snapshot — never prompts, raw JSONL, credentials, auth paths, or local paths beyond the <child-workspace> label. danger-full-access is the owner-local executor invariant (never a public wire field, never for untrusted labor/provider work). The health memory is in-process only (main-process lifetime, never persisted), and the spawn-config model pin is not presented as a provider echo.",
        seam: {
          client: "apps/openagents-desktop/src/renderer/fleet-workspace.ts",
          server: "apps/openagents-desktop/src/codex-child-runtime.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-child-contract.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/usage-ledger-contract.ts",
          "contract:openagents_desktop.chat.fable_local_owner_full_access.v1",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_delegation.child_runtime_rotation_and_usage",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-child-runtime.test.ts",
            description:
              "Drives the real JSONL parser: danger-full-access spawn recipe with pinned model/effort and isolated CODEX_HOME; exact usage totals; the verbatim SHORT auth variant classifying auth-class and rotating typed; generic pre-content failure rotating with pre_content_failure_rotated; post-content failure staying terminal; health ordering (last-good first, auth-failed demoted for the NEXT call, success clears); typed all-exhausted failures for the all-auth and mixed cases; host-side timeout; and concurrent isolated children.",
          },
          {
            id: "codex_delegation.fable_tool_caps_and_events",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Proves the delegate tool is auto-allowed under its fully-qualified name with the empty-scratch/absolute-paths guidance in its description, per-turn concurrency and total caps refuse typed without spawning, child lifecycle events (including both typed rotation activities) flow schema-valid through the FableLocalEvent envelope, and the tool result labels usage as requested spawn-config truth.",
          },
          {
            id: "codex_delegation.session_ledger_and_readiness_override",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/fleet-workspace.test.ts",
            description:
              "Proves the evidence-labeled Session usage section renders exact per-account rows with the requested codex model, and that ledger reconnect rows or failed probes supersede presence-based ready with the typed reconnect-required state.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the child-runtime, delegate, ledger, and fleet suites plus the fixture smoke journey where a fable fixture turn calls the delegate once (scripted child) and the transcript shows the tool_use/tool_result pair with the ledger row rendered in the Fleet view.",
      },
      {
        contractId: "openagents_desktop.agent_graph.pointer_keyboard_focus_equivalence.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "live agent supervision",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "khala-code-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "OpenAgents Desktop presents Runtime Gateway v8's confirmed canonical graph and schema-decoded desktop-local Claude/Fable and Codex graphs as one parent/subagent hierarchy in the chat's right context rail. Pointer activation, keyboard activation, and screen-reader buttons select the same typed agent ref; status, attention, current action, elapsed time, terminal reason, session, runtime, provider, and worktree facts remain inspectable; historical authority is labeled and never gains a live focus control; rapid replacement falls back deterministically and large graphs disclose their bound.",
        authorityBoundary:
          "The renderer accepts only graph post-images already schema-decoded by Runtime Gateway v8 or the frozen preload local-graph bridge and projects both through the shared client model. Agent selection is local inspection/focus state, not execution movement or provider/process authority. Historical projections remain inspectable with canControl=false. No provider payload, history heuristic, credential, path, store/session handle, or transport handle enters the view.",
        evidenceRefs: [
          "packages/khala-sync-client/src/live-agent-graph-presentation.ts",
          "apps/openagents-desktop/src/renderer/runtime-conversation.ts",
          "apps/openagents-desktop/src/renderer/runtime-agent-graph.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "docs/sol/2026-07-11-cut-12-live-agent-supervision-ui-receipt.md",
          "github:OpenAgentsInc/openagents#8692",
        ],
        oracles: [
          {
            id: "agent_graph.gateway_projection_and_no_poll",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-conversation.test.ts",
            description:
              "Drives a real Runtime Gateway live update carrying a validated root+child graph through the existing fenced subscription, asserts the thread receives the shared hierarchy projection, and retains the no-recurring-timeline-poll oracle.",
          },
          {
            id: "agent_graph.pointer_keyboard_screen_reader_view",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-agent-graph.test.ts",
            description:
              "Proves expand/select/focus/Escape all use typed Effect Native intent refs, selection exposes every required fact through ordinary accessible buttons and a region/table inspector, hierarchy depth is preserved, and historical authority removes live focus.",
          },
          {
            id: "agent_graph.shared_fault_and_scale_model",
            kind: "bun-test",
            mode: "unit",
            ref: "packages/khala-sync-client/src/live-agent-graph-presentation.test.ts",
            description:
              "Enforces deterministic hierarchy ordering, explicit missing facts, rapid-selection fallback, terminal/historical refusal, newest attachment/cursor selection, and exact large-graph remainder.",
          },
        ],
        verification:
          "pnpm exec vp test src/renderer/runtime-agent-graph.test.ts src/renderer/runtime-conversation.test.ts src/renderer/shell.test.ts plus typecheck, full test, build, and Electron smoke in apps/openagents-desktop; shared projection tests/typecheck run in packages/khala-sync-client.",
      },
      // =====================================================================
      // EP250 UI-owned reconnect lane — separate block: parallel lanes also
      // edit this registry; the coordinator owns the version bump on merge.
      // =====================================================================
      {
        contractId: "openagents_desktop.settings.ui_owned_codex_reconnect.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Settings Codex account reconnect",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-video-review",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "don't recommend me the CLI command for Fleet Connect. We're doing stuff with the UI now. Like, CLI stuff is nice, but the UI controls need to be working.",
        authorityBoundary:
          "The Settings Codex section owns connect AND per-account reconnect through the hardened device-auth bridge: the reconnect action carries exactly one grammar-validated account ref that main re-validates against its own registry listing before spawning the receipted per-ref re-auth (auth codex --account <ref> --force-device-login into the SAME isolated home; never ~/.codex). Fleet stays overview-only — its broken-credential rows navigate to Settings via the existing DesktopSettingsToggled intent and mutate nothing. No settings state renders copy instructing the user to run a CLI command, and no tokens, emails, or raw child output cross the bridge.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/settings.ts",
          "apps/openagents-desktop/src/codex-connect.ts",
          "apps/openagents-desktop/src/codex-connect-contract.ts",
          "apps/openagents-desktop/src/renderer/fleet-workspace.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "ui_owned_reconnect.settings_rows_and_no_cli_copy",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/settings.test.ts",
            description:
              "Proves credential-failed rows render a working per-account Reconnect button (ready rows none), the full intent loop drives the ref-targeted bridge through awaiting_browser to connected with the accounts projection re-listed so readiness flips without restart (including after a FAILED exit), and that NO settings state renders copy matching a CLI-command pattern.",
          },
          {
            id: "ui_owned_reconnect.per_ref_spawn_receipt",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/codex-connect.test.ts",
            description:
              "Proves startReconnect spawns exactly auth codex --account <ref> --force-device-login for a ref main itself listed, refuses unknown or malformed refs typed, holds single-flight across connect and reconnect, and surfaces the bounded public-safe pylon-auth failure detail.",
          },
          {
            id: "ui_owned_reconnect.fleet_fix_in_settings",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/fleet-workspace.test.ts",
            description:
              "Proves broken-credential fleet rows carry the Fix in Settings navigation over the existing DesktopSettingsToggled intent with no account mutation from Fleet, and that a successful probe this session clears a stale reconnect override (probe evidence rules).",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the settings, codex-connect, and fleet suites plus the Electron smoke journey asserting the revoked fixture account renders its Reconnect button in Settings.",
      },
      // -----------------------------------------------------------------------
      // EP250 local Fable lane permissions (#8712). HISTORY: the scoped-write
      // contract below bound the lane to workspace-contained Write/Edit with
      // no Bash/WebSearch. SUPERSEDED same-day by the owner full-access
      // override (next contract) — owner sign-off, verbatim: "disallowing
      // bash is retarded, give them full tools full permissions etc". Repo
      // law requires owner sign-off to weaken an oracle; that statement is
      // the sign-off, and the scoped-write oracles were REPLACED by the
      // full-access oracles rather than silently deleted.
      // -----------------------------------------------------------------------
      {
        contractId: "openagents_desktop.chat.fable_local_lane_scoped_write.v1",
        state: "retired",
        surface: "openagents-desktop",
        productArea: "chat local Fable lane permissions",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Writes are permitted only inside the turn/thread workspace; interactive-only tools are never offered to the model in this headless lane; out-of-scope denials say so honestly. The lane must never surface permission copy implying a grant flow exists (live incident: Write greetings.md failed with 'Claude requested permissions to write to <workspace>/greetings.md, but you haven't granted it yet.' — a grant no UI could give).",
        authorityBoundary:
          "RETIRED 2026-07-11, superseded by contract:openagents_desktop.chat.fable_local_owner_full_access.v1 on the owner's explicit sign-off ('disallowing bash is retarded, give them full tools full permissions etc'). The workspace boundary guard, out-of-scope denial copy, and Bash/WebSearch removal no longer apply; the honest-copy law (never imply a grant flow that does not exist) survives trivially because nothing is denied anymore. Kept for history.",
        evidenceRefs: [
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "contract:openagents_desktop.chat.fable_local_owner_full_access.v1",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fable_local_scoped_write.boundary_guard",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "HISTORICAL (retired state skips coverage): proved the PreToolUse guard, out-of-scope denial copy, and restricted tool set. Replaced by fable_local_owner_full_access oracles asserting the inverse posture.",
          },
        ],
        verification:
          "Retired — superseded by openagents_desktop.chat.fable_local_owner_full_access.v1; see that contract's verification.",
      },
      {
        contractId: "openagents_desktop.chat.fable_local_owner_full_access.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat local Fable lane permissions",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-codex-session", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "disallowing bash is retarded, give them full tools full permissions etc (owner override after live Claude subagents failed with 'Bash is not available in this lane.'; supersedes openagents_desktop.chat.fable_local_lane_scoped_write.v1)",
        authorityBoundary:
          "OWNER-LOCAL danger profile only — the same owner-local executor invariant as the Khala->Pylon runbook's danger-full-access/approval-never posture, never a public wire field and never applied to untrusted labor/provider work. The local Fable lane and its Agent children get the full SDK toolset (no allowedTools restriction beyond the delegate auto-allow, no PreToolUse workspace guard, no out-of-scope denial copy) with permissionMode 'default' plus an allow-all canUseTool — deliberately NOT bypassPermissions, which per sdk.d.ts bypasses all permission checks including the canUseTool handler the AskUserQuestion flow parks on. Skill and EnterPlanMode/ExitPlanMode stay disallowed as separately-decided UX noise (not permission bounds). Codex delegate children spawn with -s danger-full-access. The per-thread scratch workspace remains the default cwd only.",
        evidenceRefs: [
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/codex-child-runtime.ts",
          "contract:openagents_desktop.chat.fable_local_lane_scoped_write.v1",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fable_local_owner_full_access.full_toolset_and_question_flow",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Proves session options carry NO allowedTools restriction (only the delegate auto-allow when offered), NO PreToolUse hook, permissionMode 'default'; canUseTool allows Bash/out-of-workspace Write/WebSearch/Agent with no denial or scope copy; NotebookEdit is offered while Skill/plan-mode stay disallowed; and the AskUserQuestion regression: with the allow-all handler the question still parks pending and resolves through answerQuestion.",
          },
          {
            id: "fable_local_owner_full_access.codex_child_danger_sandbox",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-child-runtime.test.ts",
            description:
              "Proves the codex exec spawn recipe carries -s danger-full-access (owner-local profile) while keeping the isolated CODEX_HOME and per-child scratch cwd.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the fable-local runtime suite (full-toolset posture, allow-all canUseTool, question-flow regression) and the codex-child suite (danger-full-access spawn args).",
      },
      {
        contractId: "openagents_desktop.chat.fable_local_question_flow.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat local Fable lane questions",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "make the question UI too. Why not? proper effect native primitives and add some if needed. (AskUserQuestion must be a real affordance: on camera it surfaced as 'AskUserQuestion · failed · Answer questions?' with no way to answer.)",
        authorityBoundary:
          "This contract covers the runtime/IPC half: AskUserQuestion parks on the SDK canUseTool callback, emits a bounded path-redacted question_pending event (questionRef stable per invocation; question/header/options/multiSelect), and resolves with the user's answers via the answer-question invoke channel as canUseTool allow + updatedInput answers keyed by original question text (multi-select comma-separated — the SDK's documented mechanism). No answer within the window resolves a graceful typed deny with outcome timeout; turn interrupt/failure/dispose resolves outcome denied; unknown or settled questionRefs and unmatched answers are typed rejections that never throw and never burn a pending question. Multiple pending questions settle independently without deadlock. The renderer supplies only schema-checked answers; no tool execution, filesystem, or session authority crosses the channel.",
        evidenceRefs: [
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/preload.cts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fable_local_question_flow.answer_roundtrip",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Drives the real canUseTool path: question_pending with bounded questions, typed rejections for unknown/wrong-turn/unmatched answers, allow with updatedInput answers keyed by original question text, multiSelect comma-joined, timeout outcome with the turn continuing, interrupt outcome denied, and malformed input denied without parking a question.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the fable-local runtime suite covering the full question flow (answered, timeout, denied, typed rejections, multiSelect).",
      },
      {
        contractId: "openagents_desktop.chat.interactive_agent_questions.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat interactive agent questions",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-16" },
        statement:
          "When an agent asks me a question in chat, I see the options and can answer them",
        authorityBoundary:
          "A pending typed provider question opens the existing host-mediated decision surface with its bounded question text, single- or multi-select options, option descriptions, and an Other text answer. While it is pending the transcript says Waiting for your answer rather than Working. Selection and text remain renderer-local until explicit submission; the frozen schema-decoded answerQuestion bridge remains the only answer authority and carries one labels array per exact question. Runtime question_resolved remains the only resolution authority, and timeout, rejection, unavailable bridge, interruption, and stale refs stay visibly non-resolved. No raw tool input, arbitrary IPC, filesystem, provider credential, or session authority enters the renderer.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/react-composer.tsx",
          "apps/openagents-desktop/src/renderer/react-timeline.tsx",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "docs/receipts/2026-07-17-ask-user-question/README.md",
          "docs/receipts/2026-07-17-ask-user-question/01-pending-question.png",
          "docs/receipts/2026-07-17-ask-user-question/02-answer-round-trip.png",
          "github:OpenAgentsInc/openagents#8941",
        ],
        oracles: [
          {
            id: "interactive_agent_questions.react_answer_surface",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
            description:
              "Proves a provider question renders labeled options with descriptions, an Other text field, and an enabled explicit submit action that dispatches the exact question ref.",
          },
          {
            id: "interactive_agent_questions.typed_roundtrip",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves single-select, multi-select, and Other answers compile into the frozen one-entry-per-question labels-array shape and round-trip through the typed answer host without invented resolution.",
          },
          {
            id: "interactive_agent_questions.waiting_status",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx",
            description:
              "Proves a pending question replaces the generic Working indicator with Waiting for your answer and does not leave the timeline aria-busy.",
          },
        ],
        verification:
          "The normal Desktop test sweep runs the renderer answer-surface, typed round-trip, waiting-status, runtime parking/timeout, and built-Electron smoke oracles.",
      },
      {
        contractId: "openagents_desktop.coding_catalog.restart_safe_navigation.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "projects and coding sessions",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "khala-code-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "OpenAgents Desktop persists stable project, repository, worktree, coding-session, tab, route, and typed focus refs across restart. Adding a workspace creates or resumes one canonical session; duplicate opens collapse; missing worktrees recover explicitly; pointer and keyboard activation share the intent registry; and background coding-catalog state never treats a local path or renderer tab as authority. No Project home screen or route is exposed.",
        authorityBoundary:
          "Signed-out catalog rows live only under scope.device_local in the host-owned Sync SQLite local_entities table. Raw filesystem bindings live in a separate mode-0600 main-process file and never cross preload or enter canonical post-images. Hosted server projection and confirmed reads accept only user/team scopes. The renderer receives a bounded public-safe projection and can invoke only fixed schema-decoded choose/open/archive/recover actions.",
        evidenceRefs: [
          "packages/khala-sync/src/coding-session.ts",
          "apps/openagents-desktop/src/desktop-coding-catalog.ts",
          "apps/openagents-desktop/src/coding-catalog-contract.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "docs/sol/2026-07-11-cut-13-canonical-coding-session-catalog-receipt.md",
          "github:OpenAgentsInc/openagents#8693",
        ],
        oracles: [
          {
            id: "coding_catalog.host_restart_and_recovery",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-coding-catalog.test.ts",
            description:
              "Proves private path separation, stable refs across host-service reopen, duplicate-open collapse, recent sort, typed focus, archive selection, missing-worktree recovery, and owner-private file modes.",
          },
          {
            id: "coding_catalog.effect_native_action_equivalence",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves Project Home renders the bounded catalog without nested cards and choose/filter/open/archive dispatch through the same schema-checked Effect Native registry used by keyboard activation.",
          },
          {
            id: "coding_catalog.built_reload_restoration",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built Electron fixture writes through real local SQLite/private binding services, opens the Project Home with the stable current session, reloads the renderer, and observes the same restored catalog authority and selection.",
          },
        ],
        verification:
          "Focused catalog/shell/boundary tests, shared/server/client CUT-13 suites, Desktop typecheck and full test, production build, and OPENAGENTS_DESKTOP_SMOKE=1 Electron smoke.",
      },
      {
        contractId: "openagents_desktop.chrome.apps_sdk_chrome_design_language.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "application chrome design language",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "do a separate design pass of projects/repos/apps-sdk-ui and thats what i want to use for the rest of the app chrome, menus, etc, everything other than messages, but still harmonized to messages. we want that design language, ported to starcraft kinda, represented in EVERY other surface of the app",
        authorityBoundary:
          "Presentation only. The apps-sdk-ui chrome language (alpha-overlay state engine — hover/active/selected as translucent overlays of one base color, never new hues; elevation = lighter surface + hairline ring for floating overlays; 150/350/200ms motion; the trimmed 4-step control lattice; the three-level dim ladder) is expressed as shared @effect-native/tokens roles/groups, the vendored DOM renderer chrome base ruleset, typed token style objects in the renderer views, and a host stylesheet that resolves every color through --en-* custom properties. Our icon set stays; one uniform dark product theme only, now the owned Tokyo Night projection admitted by IDE-01 and mounted by IDE-03; no light theme, mutable theme marketplace, 24px composer radius, backdrop-blur popover variant, 9-step lattice, or donor icons. Message/tool cards keep the OpenCode geometry, harmonized onto the same shared scales.",
        evidenceRefs: [
          "apps/openagents.com/packages/effect-native-tokens/src/index.ts",
          "apps/openagents.com/packages/effect-native-render-dom/src/index.ts",
          "apps/openagents-desktop/src/renderer/theme.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/docs/design-ports.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "chrome_design.token_closure_and_scale_membership",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
            description:
              "Mechanical conformance: (a) zero raw hex/rgb/hsl color literals in renderer modules and the host stylesheet outside the theme module; (b) spacing/radius/type style values are members of the shared token scales with a small documented numeric-dimension allowlist; (c) per-surface structural recipes — sidebar rail sections, palette on surfaceOverlay+borderSubtle+xl with chord captions, composer radius cap + recessed segmented harness track, settings panel padding/hairline, fleet chrome, inspector rail scale, tool-card shimmer keys, 240px raw wells, context-group anatomy.",
          },
          {
            id: "chrome_design.theme_is_tokyo_night_canonical",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "The desktop theme uses the pinned Tokyo Night semantic colors over the canonical shared state-overlay, dim-ladder, motion, spacing, radius, and control groups; app-local palette drift remains forbidden.",
          },
          {
            id: "chrome_design.smoke_pixels",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke drives every restyled surface (shell+sidebar, palette, settings, fleet, inspector, composer) and captures pixel receipts when OPENAGENTS_DESKTOP_SMOKE_SHOTS is set.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the design-conformance sweep, the shell/theme suites, and the Electron smoke over the restyled surfaces.",
      },
      {
        contractId: "openagents_desktop.chat.new_chat_autofocuses_composer.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat composer focus",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement: "when i do new chat, clicking button or command N, auto focus the input.",
        authorityBoundary:
          "Focus behavior only. Every DesktopNewChat entry point — the workspace-new-chat dock button, the command palette chat.new row, and the Cmd+N/Ctrl+N chord (the canonical chat.new default binding, newly wired as a window keydown following the existing platform-modifier shortcut pattern; no Electron menu or OS accelerator collision exists) — dispatches the SAME typed intent, and the composer input receives focus AFTER the chat view mounts (retry across render commits, because a New chat from a loaded history page swaps the center view and a re-parented input loses focus). No new dispatch authority; the chord is suppressed inside editables like the other global shortcuts.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/desktop-command-contract.ts",
          "apps/openagents-desktop/src/renderer/command-registry.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "new_chat_focus.palette_chord_caption",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
            description:
              "The palette chat.new row surfaces its canonical chord caption (⌘N on darwin) from the single command registry entry — one registered command, all input paths dispatch the same intent.",
          },
          {
            id: "new_chat_focus.smoke_button_and_chord",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke proves BOTH paths end focused: the dock New-chat click from a loaded history page yields a fresh empty transcript with document.activeElement === the composer input, and a synthesized platform-modifier+N keydown from the fleet workspace does the same.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the palette-registry assertions and the Electron smoke new-chat + cmd-n focus steps.",
      },
      {
        contractId: "openagents_desktop.chat.startup_new_session_continuity.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "startup chat continuity",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-live-review", statedBy: "owner", statedOn: "2026-07-15" },
        statement:
          "When the app opens I can immediately type and send in the new chat. Loading the conversation catalog must never replace that draft by auto-opening the most recent conversation.",
        authorityBoundary:
          "The first mounted chat surface is a real New session draft, not a temporary facade and not a restored conversation. It stays focused and submittable without eagerly persisting empty chats; the first valid submission admits exactly one durable local thread through the same typed ChatHost used by DesktopNewChat and continues the original send unchanged. Startup history hydration may publish Codex metadata into the sidebar but may not select, restore, open, page, or hydrate any conversation detail; local thread-catalog arrival also preserves the null selection. Detail reads require an explicit user selection.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/react-composer.tsx",
          "apps/openagents-desktop/tests/startup-contract.test.ts",
          "apps/openagents-desktop/src/renderer/shell.test.ts",
          "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
        ],
        oracles: [
          {
            id: "startup_chat.no_history_autoselection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/startup-contract.test.ts",
            description:
              "Proves the shell mounts before history hydration, local catalog projection preserves the null selection, and rejects startup hydration containing restored selection, history paging, openThread, or hydrateThread calls.",
          },
          {
            id: "startup_chat.first_submit_is_durable",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Starts with a typed draft and no active thread, submits once, and proves exactly one durable local thread is created and the exact first message is sent on it.",
          },
          {
            id: "startup_chat.composer_send_enabled",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
            description:
              "Proves the focused startup composer keeps Send enabled for a valid draft before activeThreadId admission settles and dispatches DesktopNoteSubmitted unchanged.",
          },
        ],
        verification:
          "The normal Desktop test sweep runs the source-ordering falsifier, the typed handler first-submit test, and the React composer admission-race test.",
      },
      {
        contractId: "openagents_desktop.chat.per_conversation_composer_ownership.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat selection and composer ownership",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-live-review", statedBy: "owner", statedOn: "2026-07-16" },
        statement:
          "Composer state and turn controls belong to the selected chat. Switching chats must not move a draft, Send to another thread, or make Stop control a different thread.",
        authorityBoundary:
          "Every local chat owns an in-memory draft keyed by its exact durable thread ref; switching restores only that chat's draft and pending/failure projection. A turn freezes its originating thread ref before dispatch, and late updates or completion may update that thread's catalog entry but may never replace a newer selection or draft. Provider-history pages are read-only and mount no composer; a synthetic submit while history is selected fails closed and never falls through to newThread. Async selection is latest-intent-wins, and interruption checks the exact selected pending thread.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/react-composer.tsx",
          "apps/openagents-desktop/src/renderer/react-primitive-adapters.tsx",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
        ],
        oracles: [
          {
            id: "per_conversation_composer.draft_and_target_isolation",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves A/B draft restoration, blank New Chat isolation, quiet owner interruption, and that a selected provider-history page can never invoke newThread or sendMessage.",
          },
          {
            id: "per_conversation_composer.typed_interrupt_boundary",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/local-harness.test.ts",
            description:
              "Proves the provider-lane interrupted reason survives the dispatcher/renderer boundary as failureKind interrupted rather than generic failed.",
          },
        ],
        verification:
          "Desktop typecheck, behavior-contract validation, and the focused shell, local-harness, provider-lane, React composer, and workbench suites.",
      },
      {
        contractId: "openagents_desktop.chat.new_chat_always_exits_history.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat navigation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-live-review", statedBy: "owner", statedOn: "2026-07-12" },
        statement:
          "New Chat and Command-N must always leave the old chat and open a fresh chat.",
        authorityBoundary:
          "Every New Chat entry point dispatches the same DesktopNewChat intent. Creation is local-first and must not enter live Sync's pending-reconciliation path: the converging chat host creates through the app-owned durable local thread store, pins that ref to local authority, exits any loaded history page, mounts an empty transcript, and focuses the composer. The typed runtime host is attempted only when the local durable bridge cannot create. No fake success is projected without a real thread from one of the two typed durable hosts.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/runtime-conversation.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
        ],
        oracles: [
          {
            id: "new_chat_always.local_creation_bypasses_sync_reconciliation",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-conversation.test.ts",
            description:
              "Proves New Chat creates exactly once through the durable local store, never enters live Sync reconciliation, and pins the resulting thread ref to local authority.",
          },
          {
            id: "new_chat_always.button_and_command_n_exit_history",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke proves both the dock button and platform Command-N leave loaded history, mount a fresh empty transcript, and focus the composer.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the converging-host fallback unit test and both built-Electron New Chat paths.",
      },
      {
        contractId: "openagents_desktop.chrome.disabled_control_reason_popover.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "disabled-control affordances",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "i can't tell why the Codex option is disabled in the composer. for things like that you need to put a popover on hover over the disabled button explaining why.",
        authorityBoundary:
          "Presentation only, hover/focus only. A disabled control that carries a reason string is wrapped in the catalog Tooltip: pointer hover or keyboard focus reveals the reason as a small overlay on the shared overlay recipe (surfaceOverlay fill, overlay shadow + hairline ring, caption text, fast fade); leave/blur dismisses it. The popover reads whatever reason string the control state carries — never hardcoded copy — so a future lane that lights the Codex chip changes nothing here. The accessible label keeps carrying the reason for screen readers, and NO standing caption returns (openagents_desktop.chat.no_composer_disabled_caption.v1 stays intact: the bubble is [hidden] at rest and excluded from visible-text checks). Applied to the composer harness chips, the unavailable-lane Send button, and the settings Reconnect control while another device-auth flow is live.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/settings.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "disabled_reason_popover.typed_wrapping",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
            description:
              "A disabled harness chip with a reason is wrapped in a Tooltip whose content equals the exact lane reason; the unavailable-lane Send button gets the same wrap; available controls render bare (no popover, no caption).",
          },
          {
            id: "disabled_reason_popover.smoke_hover_reveal",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke proves the disabled Codex chip's popover is hidden at rest, matches the accessible reason exactly, reveals on pointerenter, and dismisses on pointerleave — while the standing-caption ban assertion keeps passing on visible text.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the popover unit assertions and the Electron smoke hover-reveal step.",
      },
      {
        contractId: "openagents_desktop.window.fullscreen_hotkey.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "window controls",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement: "add a hotkey for maximizing (command+something) to fullscreen like command f",
        authorityBoundary:
          "Window presentation only. Cmd+F/Ctrl+F is the canonical window.fullscreen_toggle default binding. The native Window menu exposes Electron's togglefullscreen role with that effective accelerator, so visible menu activation and the shortcut operate directly on the focused BrowserWindow even before renderer readiness. The renderer command remains a command-palette and DOM-keyboard fallback: DesktopFullscreenToggled calls the window host seam and main toggles the sender BrowserWindow's fullscreen state. Deliberately no editable-guard (no find-in-page exists yet; rebind review when find lands). No renderer window-handle authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/desktop-command-contract.ts",
          "apps/openagents-desktop/src/window-contract.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fullscreen_hotkey.contract_binding",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "The command contract carries window.fullscreen_toggle with Meta+F/Control+F defaults bound to DesktopFullscreenToggled, and dispatching the intent through the registry invokes the injected window host toggle exactly once.",
          },
          {
            id: "fullscreen_hotkey.native_window_menu",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-command-host.test.ts",
            description:
              "The production menu excludes fullscreen from generic Commands and exposes Electron's native togglefullscreen role under Window with the effective canonical binding.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the native Window menu and shell registry fallback assertions.",
      },
      {
        contractId: "openagents_desktop.shell.no_sidebar_brand_row.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "sidebar chrome",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement: "remove the \"OpenAgents\" with icon top left ins idebar",
        authorityBoundary:
          "Compatibility-renderer presentation only: its typed workspace dock renders no redundant brand row above the dock. This historical rule does not govern the admitted React workbench, whose current Codex-shaped identity row is specified by openagents_desktop.sidebar.codex_shaped_react_anatomy.v1.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "no_sidebar_brand.absence",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "shellSidebar's rendered view contains no sidebar-brand or sidebar-brand-icon nodes and no literal 'OpenAgents' text node in the sidebar tree.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the sidebar absence assertion.",
      },
      {
        contractId: "openagents_desktop.chat.codex_first_class_local_lane.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer Codex local chat lane",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement: "yeah i need codex and claude both first class",
        authorityBoundary:
          "The Codex chip in local mode uses the app-server protocol from the user's validated installed Codex executable against the ordinary authenticated local Codex session — never a Codex binary packaged by OpenAgents, never the cloud gateway, and never an inherited or registry-selected CODEX_HOME. Provider-originated server requests park at the main-process runtime in a correlated pending registry and emit bounded typed decision state. React can answer only through the installed intent/bridge path; the runtime returns the method-correct response to that exact provider request, and the turn continues only after the provider accepts it. On a reused supervised connection, notifications received before the new provider thread and turn identities are bound are quarantined with a fixed bound and replayed only through the exact identity fence: stale identities and unaffiliated transcript content cannot enter the new chat, while explicitly connection-scoped telemetry and compatibility notices remain non-transcript exceptions. The lane reuses the frozen fable-local event envelope so turns render through the same transcript cards, and persists the provider thread id for same-session resume. The legacy `codex exec --json` parser is retained only for compatibility fixtures and delegate children remain ephemeral. No renderer authority is widened: the bridge carries only bounded, redacted typed events.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-local-runtime.ts",
          "apps/openagents-desktop/src/codex-local-contract.ts",
          "apps/openagents-desktop/src/codex-app-server-turn.ts",
          "apps/openagents-desktop/src/codex-app-server-smoke-fixture.ts",
          "apps/openagents-desktop/src/codex-app-server-smoke-fixture.test.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/main.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_first_class.lane_runtime",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-local-runtime.test.ts",
            description:
              "Exercises the production app-server runtime, including a provider-originated command approval that blocks the turn until the exact request id receives the method-correct decision. It also retains explicit compatibility coverage for the legacy JSONL parser, bounded history, event mapping, interruption, and typed no-account failures.",
          },
          {
            id: "codex_first_class.turn_stream_identity",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-app-server-turn.test.ts",
            description:
              "Reproduces a reused app-server publishing previous-chat text before the new provider identities bind, then proves quarantined stale and unaffiliated text are discarded while the exact current thread/turn answer is the only emitted assistant delta.",
          },
          {
            id: "codex_first_class.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The production-built React Electron smoke injects a protocol-speaking provider peer at the app-server spawn seam. Request 91 opens a real pending command decision, the installed React dialog dispatches Approve through Effect intent and IPC, the provider records the exact correlated accept response, and only then emits command completion, assistant output, usage, and turn completion. Reload restoration and zero-owner teardown are asserted after reconciliation; the separate compatibility smoke retains the legacy parser fixture.",
          },
          {
            id: "codex_first_class.live_proof",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/live-proof.ts",
            description:
              "The EP250 live-proof driver's codex-chip and codex-turn steps pass with a verified account: a real gpt-5.5 turn streamed in the transcript with mid-stream capture, journaled honestly when no account verifies.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the codex-local runtime suite and the Electron smoke codex-local step; OPENAGENTS_DESKTOP_LIVE_PROOF=1 exercises the real lane.",
      },
      {
        contractId: "openagents_desktop.reliability.codex_connection_signature_corpus.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Codex connection reliability regression corpus",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "add thorough fucking tests or whatever to prevent this category of codex connection error PLEASE I HATE ALL THEF UCKING SPEEDBUMPS HERE",
        authorityBoundary:
          "The corpus is table-driven over checked-in verbatim failure fixtures (live-captured where available): LONG revoked, SHORT auth variant, 401 token_invalidated, refresh_token_invalidated, missing auth.json, malformed auth.json, quota/429, network-refused, timeout, and Codex configuration parser failures. Before spending a probe turn, the user's validated installed Codex parser validates the current account configuration. One narrowly provable repair is automatic: a disabled MCP server stanza whose only field is `enabled = false` and therefore has no transport; the original file is backed up, only that inert stanza is removed, and Codex must parse successfully afterward. Every other configuration failure is left untouched and crosses the typed availability projection as exact path, line, column, and bounded parser message for the React status notice. Renderer code never reads or writes the config file.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-connection-signatures.test.ts",
          "apps/openagents-desktop/src/codex-preflight.ts",
          "apps/openagents-desktop/src/codex-config-health.ts",
          "apps/openagents-desktop/src/codex-config-health.test.ts",
          "apps/openagents-desktop/src/codex-child-runtime.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_signature_corpus.table",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-connection-signatures.test.ts",
            description:
              "One row per failure signature through the REAL parser/classifier/rotation path, asserting all five row dimensions, plus the chip lifecycle state machine (boot-probe to verified to enabled; revoke-mid-session demotes while another verified account keeps the chip; a reconnect probe clears; none-verified disables with the reconnect reason).",
          },
          {
            id: "codex_signature_corpus.preflight",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-preflight.test.ts",
            description:
              "Proves the receipted minimal probe recipe, configuration-invalid classification, exact diagnostic projection, verified/reconnect/rate-limit/missing/failed classification, the credentials_missing no-spawn fast path, the host-side timeout bound, health + ledger feeds, and ensureProbed session-cache semantics. The companion config-health suite proves backup, narrow repair, re-parse, and ambiguous-file refusal.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the signature corpus and preflight suites in the normal sweep; the live-proof journey journals the real per-account probe round as step 0.",
      },
      {
        contractId: "openagents_desktop.seam.codex_local_lane_no_substitution.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Codex local lane substitution law",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Selecting Codex routes to the local Codex lane only: the requested model and reasoning effort are spawn-config truth (gpt-5.5, medium — the exec stream echoes nothing back), every projected model string is labeled (requested), account rotation is typed and visible in the transcript, and no send is ever silently rerouted to another account, lane, or model.",
        authorityBoundary:
          "When no PROBE-VERIFIED Codex account exists, Send refuses with the chip reason and the message goes nowhere. Rotation is bounded by the registry, announced per skip via typed lane_notice events, and post-content failures are terminal (a partial reply never double-runs). The ledger and message metadata record the requested model as spawn-config truth, never as a provider echo.",
        seam: {
          client: "apps/openagents-desktop/src/renderer/local-harness.ts",
          server: "apps/openagents-desktop/src/codex-local-runtime.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-local-runtime.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/codex-local-contract.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_local_no_substitution.runtime",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-local-runtime.test.ts",
            description:
              "Asserts the pinned -m/-c spawn args, the (requested) model caption event, typed visible rotation with health demotion, terminal post-content failures, and typed refusals that name that no other lane was used.",
          },
          {
            id: "codex_local_no_substitution.renderer_refusal",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/local-harness.test.ts",
            description:
              "A codex send without verified availability is an explicit typed refusal that never reaches the legacy gateway or the fable bridge.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the codex-local runtime and local-harness suites in the normal sweep.",
      },
      {
        contractId: "openagents_desktop.chat.codex_chip_verified_evidence.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer Codex chip evidence gating",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "The composer Codex chip is enabled only when a PROBE-VERIFIED Codex account exists this session; registry auth.json presence is never validity. Unavailable reads Codex — no verified account · Reconnect in Settings; when the only obstacle is quota it reads Codex — accounts rate-limited · retry later or connect another account (reconnecting never restores quota); while the session probe is still running it reads Codex — verifying accounts…; the shell never blocks first mount on the probe round.",
        authorityBoundary:
          "Verification evidence is a real bounded minimal `codex exec` probe turn per account (read-only sandbox, ~30s host bound), session-scoped with observedAt, re-run on boot/fleet-Refresh/reconnect-completion and lazily before first dispatch. Probe results feed the shared account-health ordering (verified first), the fleet readiness projection via the ledger typed reconnectRequired flag (probe evidence supersedes presence-based ready; a fresh verified probe clears a stale flag), and the chip state. The reason string lives in the chip state for the chrome disabled-reason popover to render; enabling the chip grants no new authority beyond the existing typed start channel.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-preflight.ts",
          "apps/openagents-desktop/src/codex-local-contract.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/usage-ledger.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_chip_verified_evidence.lifecycle",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-connection-signatures.test.ts",
            description:
              "The chip lifecycle state machine: pending probe renders verifying disabled; verified enables; revoke-mid-session keeps the chip on the other verified account; a reconnect probe clears; none verified disables with the reconnect reason.",
          },
          {
            id: "codex_chip_verified_evidence.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke asserts the codex chip stays disabled (with its popover reason) until the gated fixture preflight verifies an account, then lights for the streamed codex-local turn.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the lifecycle suite and the Electron smoke chip assertions; the live-proof journal records the real per-account probe round.",
      },
      {
        contractId: "openagents_desktop.chat.composer_shift_tab_harness_toggle.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer keyboard harness toggle",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "i want shift+tab to togle between modes in composer (fable / codex) in this case",
        authorityBoundary:
          "The gesture exists only while the composer input has focus: Shift+Tab there toggles the selected harness both directions through the SAME typed DesktopHarnessSelected intent the chips dispatch, with preventDefault so focus never moves. Shift+Tab anywhere else keeps normal reverse focus navigation, and plain Tab is untouched. Toggling TO an unavailable lane is allowed — selection moves and the disabled-reason popover / evidence-gated Send explain why it cannot act; the gesture is never silently blocked and grants no send authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/composer-shortcuts.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "composer_shift_tab_toggle.handler",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/composer-shortcuts.test.ts",
            description:
              "Proves focused-composer Shift+Tab toggles both directions and preventDefaults, focus-elsewhere and plain Tab are untouched, already-consumed events are left alone, and availability is never consulted (unavailable-lane toggles allowed).",
          },
          {
            id: "composer_shift_tab_toggle.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke synthesizes Shift+Tab on the focused composer input, asserts the segmented selection flips to codex (a disabled lane) and back with dispatchEvent reporting preventDefault, and that Shift+Tab on a non-composer target neither toggles nor gets consumed.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the composer-shortcuts suite and the Electron smoke composer-gestures step.",
      },
      {
        contractId: "openagents_desktop.chat.composer_icon_only_send.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer send control",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-12" },
        statement:
          "put a microphone button there left of the submit button. make submit button an up arrow like codex has there. clicking microphone button toggles a voice mode we havent implemented yet. just put a text thing there for like disabled or something",
        authorityBoundary:
          "The composer renders exactly one Submit control as the shared catalog IconButton with ArrowUp inside and no visible Send label. Immediately left is a compact shared Mic IconButton. Mic dispatches only DesktopVoiceModeToggled into renderer presentation state: selected state shows the literal `Voice unavailable` badge and starts no microphone, permission prompt, transcription, network, provider, or device action. Pending turns disable Mic and its handler also refuses programmatic toggles. Submit keeps its accessible name, disabled-reason wrapper, and DesktopNoteSubmitted intent.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "composer_icon_only_send.view",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the composer renders exactly one ArrowUp Submit IconButton, places the compact Mic control immediately before it, toggles the honest Voice unavailable badge through the typed intent, and refuses that toggle while pending.",
          },
          {
            id: "composer_icon_only_send.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke asserts the rendered send control carries the icon variant with an inline glyph and aria-label, exactly one send control exists, and no visible Send text or freestanding icon remains in the composer.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the shell composer suite and the Electron smoke composer-gestures step.",
      },
      {
        contractId: "openagents_desktop.sidebar.connected_accounts_usage_box.v1",
        state: "retired",
        surface: "openagents-desktop",
        productArea: "sidebar connected-accounts usage box",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "in the left sidebar, in a bottom box, like letting the chats flex up but show up to 5 connected accounts with a progress bar showing remaining weekly/hourly usage (grayed out if we dont have that data).",
        authorityBoundary:
          "Read-only presentation over the existing fleet accounts projection and its per-account usage entries: the box never probes providers, adds no polling loop, and mutates nothing. The bar renders a MEASURED remaining value only when a decoded usage entry carries real provider rate-limit windows (pylon truth.provider.snapshots, codex-rs RateLimitSnapshot 5h/weekly lineage); every other account renders the grayed borderSubtle track with zero fill and the honest reason ('no usage-window data for this provider') in the tooltip and accessible label — never a fake bar. At most five accounts render (ready first, then provider, then ref); overflow is a dim '+N more' row deep-linking to the Fleet workspace through the existing DesktopWorkspaceSelected intent. Zero connected accounts render no box.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/sidebar-accounts.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/provider-accounts.ts",
          "apps/openagents-desktop/src/provider-accounts-contract.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "sidebar_accounts_box.view_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/sidebar-accounts.test.ts",
            description:
              "Proves the five-account cap with the '+N more' Fleet deep-link row, ready-first ordering, measured bar math (Meter value = tightest window remainingPercent / 100 with all windows in the accessible label), the grayed no-data bar (zero fill, reduced opacity, borderSubtle track, honest tooltip/aria reason), and the absent box at zero accounts.",
          },
          {
            id: "sidebar_accounts_box.pinned_structure",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the sidebar column keeps the chats list flexing (NavRail flex:1/minHeight:0) while the accounts box renders as the LAST sidebar child (pinned bottom, hairline-topped) when accounts exist, and does not render at all when none do.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the sidebar-accounts view suite, the shell pinning assertions, and the design-conformance token oracle over the new module.",
      },
      {
        contractId: "openagents_desktop.history.markdown_prose_rendering.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "codex history workspace message prose",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "i see assistant messages showing raw markdown what the fuck. need to use the same markdown renderer we use elsewhere",
        authorityBoundary:
          "Presentation only. Assistant, user, and agent-message prose in the history workspace (center transcript AND the right-rail item inspector) renders through the SAME bounded markdown projector chat assistant bodies use (renderer markdown.ts -> catalog Markdown/CodeBlock/Divider) — no second parser, links stay safe text with the path visible, no navigation from historical content. Loss-accounted completeness (#8674/#8675) is untouched: every retained source item still projects exactly once (prose OR event row), gap/redaction notices stay plain styled text outside the markdown projection, and the completeness equation counts exactly what it counted before.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "apps/openagents-desktop/src/renderer/markdown.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "history_markdown.prose_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
            description:
              "An assistant item carrying headings/bold/links/inline+fenced code renders typed Markdown/CodeBlock views (h3 + strong + code present, no literal ### or ** in text nodes, link label plus visible path as safe text); user and agent-message prose route through the same projector; gap/redaction notices stay plain event rows; the inspector body uses the projector for prose and plain text for notices; rendered-once accounting holds.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the history workspace suite plus the codex-history completeness suites.",
      },
      {
        contractId: "openagents_desktop.history.agent_roster_shortcut_traversal.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "codex history workspace keyboard navigation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "just like command up and down scrolsl thru chats, have command shift up and down go up and down the agents of a convo.",
        authorityBoundary:
          "Cmd+Shift+Up/Down (Ctrl+Shift off-macOS) moves the selection through the SAME visible agent roster the right-rail Agents tree renders, dispatching the SAME typed HistoryAgentSelected intent the tree rows dispatch — no parallel selection path. Ends clamp exactly like the unshifted conversation shortcut; the unshifted chord keeps traversing conversations; no-op when no history conversation is open or the roster is empty; editable targets are never intercepted.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "history_agent_traversal.roster_walk",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
            description:
              "Traversal walks the exact visible roster (collapsed subtrees skipped) both directions, clamps at both ends, no-ops without an open page or roster, targets resolve to the same HistoryAgentSelected intent the tree rows carry, and the shifted chord discriminates from the unshifted conversation chord per platform.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the history workspace traversal suite.",
      },
      {
        contractId: "openagents_desktop.history.humanized_tool_cards.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "codex history workspace tool cards",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "spawn agent card is still showing a fucking json object in the card tool thing, not good",
        authorityBoundary:
          "Display only. Historical tool_call rows humanize through the SAME table chat tool cards use (renderer tool-cards.ts humanizeToolInvocation — never a second table): spawn_agent shows 'Spawn agent' with the task name and a dim fork-turns meta; terminal/file/search/web items show their command, path, or query; unknown items show a prettified name plus a bounded key:value summary. Raw JSON never renders as the default card body, and opaque base64-class continuation/message blobs NEVER appear in any card body — the raw input (blob included) stays reachable only through the item inspector, bounded as before. Loss-accounting completeness is untouched.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/tool-cards.ts",
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "history_tool_cards.humanized_no_blob",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
            description:
              "A spawn_agent fixture with a base64-class continuation blob renders 'Spawn agent — issue_audit · fork turns: 3' with no blob and no braces in the card detail while the inspector still exposes the raw input; table-driven cases cover exec/shell/web_search/read_file/grep/apply_patch/mcp and the prettified unknown fallback.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the history workspace and tool-card humanization suites.",
      },
      {
        contractId: "openagents_desktop.history.bottom_anchored_autoload.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "codex history workspace pagination",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "you need to show the most recent messages, starting at bottom, and auto load them as i scroll up, smartly loading before the cursor",
        authorityBoundary:
          "Presentation and fetch-order only. A history conversation opens at its END (tail window, newest items visible, scrolled to bottom); the Previous/Next pager is gone. Scrolling up auto-loads the previous page ~1.5 viewports before the top edge and preserves the reader's scroll anchor by the exact prepended height; scrolling down auto-loads newer pages. A thin textFaint loading row / position caption marks the fetching edge. Overlapping fetches never double-count an item. Loss-accounted completeness (#8674/#8675) is untouched: source/rendered/redactions/gaps and totalItems stay whole-conversation truth as the loaded WINDOW changes — only fetch order changed. Restoring a saved item selection reopens the window around that item and scrolls to it; otherwise restore opens at the end.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/renderer/history-restore.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "history_autoload.window_math",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
            description:
              "Tail offset opens at the last page; scroll-up merge prepends older items and moves the offset back with no dupes; scroll-down appends newer items; prefetch predicates fire ~1.5 viewports before each edge and only while idle; prepend preserves the scroll anchor by the growth; no pager renders and the loading edge shows a thin textFaint row/caption; the restore plan reopens a saved item's window or opens at the end; completeness stays whole-conversation as the window grows.",
          },
          {
            id: "history_autoload.smoke_scroll",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "WRITTEN, pending owner (movie): the built-Electron trace acceptance opens a conversation at its tail (bottom-anchored, no pager), asserts scrolling to the top auto-prepends the previous window with the scroll anchor preserved and the position caption advancing, discovers tool/handoff items against the same tail window the UI renders, and the reload driver restores the saved window. NOT executed this session.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the windowed-loading unit suite; the Electron smoke bottom-anchored + prefetch steps are written but were not executed this session (owner watching a movie) — the coordinator runs the visual/smoke gate on integration.",
      },
      {
        contractId: "openagents_desktop.history.claude_import.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local coding-history import",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "capability-audit", statedBy: "agent", statedOn: "2026-07-11" },
        statement:
          "From the daily-coding capability audit (docs/fable/2026-07-11-daily-coding-capability-audit.md §4 H3): History import/browse must import ~/.claude alongside ~/.codex — the desktop imported ~/.codex history only, with no ~/.claude importer, even though the window held 2,243 Claude files. OpenAgents Desktop discovers Claude Code parent sessions and subagent/workflow sidechains, reconstructs the same loss-accounted parent/child agent graph (edge = the invoking Agent tool call's structured toolUseResult.agentId, NOT parentUuid), and surfaces Claude sessions in the SAME history catalog as Codex, tagged by source.",
        authorityBoundary:
          "Read-only, owner-local, additive. The Claude importer (claude-history.ts) and the merged surface (merged-history.ts) return only the same schema-bounded, credential-redacted CodexHistory catalog/page projections the Codex importer returns, namespaced `claude:` so page/search routing is unambiguous. Import NEVER changes what counts: each source keeps its whole-conversation completeness equation source = rendered + redactions + gaps, and the ~3% Claude orphan class (a child file with no recoverable structured edge) is shown and counted as an explicit topology gap, never silently hidden. Raw prompts, thinking, tool arguments, command output, credentials, and file contents never reach the renderer beyond the existing bounded/redacted item fields; no filesystem authority, resume, or provider runtime authority enters the renderer.",
        evidenceRefs: [
          "apps/openagents-desktop/src/claude-history.ts",
          "apps/openagents-desktop/src/merged-history.ts",
          "apps/openagents-desktop/src/codex-history-contract.ts",
          "docs/teardowns/2026-07-10-claude-subagents-rendering-analysis.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "claude_import.graph_and_loss_accounting",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/claude-history.test.ts",
            description:
              "Reconstructs the Agent-edge graph (source-tagged, namespaced refs), projects rich redacted items, links subagent previews, represents an unlinked child as an explicit orphan/topology gap, pages without overlap/omission, and holds source = rendered + redactions + gaps.",
          },
          {
            id: "claude_import.merged_catalog_and_scale",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/history-search.test.ts",
            description:
              "A mixed Codex+Claude catalog shows both sources tagged and routes pages by ref; the 100MiB/100-child scale oracle (claude-history-performance.e2e.test.ts) keeps discovery bounded and the completeness equation intact; the capability-evals headless oracle drives the merged catalog/page.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the Claude importer unit suite, the merged-catalog + search suite, the scale oracle, and the capability-evals headless H3 oracle.",
      },
      {
        contractId: "openagents_desktop.history.free_text_search.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local coding-history search",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "capability-audit", statedBy: "agent", statedOn: "2026-07-11" },
        statement:
          "From the daily-coding capability audit (docs/fable/2026-07-11-daily-coding-capability-audit.md §4 H4 / §6 item 11): Session search must go beyond the structured project:/repository:/state: catalog grammar — the daily reality is 'find that conversation where…' across 3,262 session files, and nothing searched transcript content. OpenAgents Desktop adds free-text matching over session TITLES and a bounded per-session CONTENT index across Codex AND Claude sessions, ranked by match then recency, with a search box in the history workspace whose results open the session at the matching item.",
        authorityBoundary:
          "The content index is a REBUILDABLE LOCAL CACHE, never authority (per the audit's History/Discovery/Memory split, indexes are caches). It ranks over titles (always available from the loss-accounted catalog) and a bounded content projection of the most-recent sessions (bounded item cap; first search never blocks on the whole archive; response reports indexedSessions/truncated honestly). Search NEVER mutates catalog/page truth or the completeness equation. A content result carries the exact matching item ref so opening it windows the session on that item, reusing the bottom-anchored restore-to-item flow; a title result opens at the end. Matching is deterministic substring filtering of an owner-local corpus — not ad-hoc intent routing — and surfaces only the same bounded/redacted item text the projector already exposes.",
        evidenceRefs: [
          "apps/openagents-desktop/src/history-search.ts",
          "apps/openagents-desktop/src/merged-history.ts",
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "free_text_search.ranking_and_open_at_item",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/history-search.test.ts",
            description:
              "Title match, content match with the exact open-at-item ref, cross-source recency ranking, empty/no-match returns nothing, and the index is rebuildable (a fresh build yields identical ranked results); the pure ranking core proves titles outrank content and recency breaks ties.",
          },
          {
            id: "free_text_search.ui",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
            description:
              "The search field renders a query-bound TextField; a non-blank query activates search; result rows carry the source badge and dispatch HistorySearchResultOpened with the threadRef; a content result opens at its matching item while a title result opens at the end.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the search ranking/open-at-item suite, the history-workspace search UI suite, and the capability-evals headless H4 oracle.",
      },
      {
        contractId: "openagents_desktop.chat.fable_local_runtime_capabilities.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "fable-local runtime capability substrate",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "capability-audit",
          statedBy: "agent",
          statedOn: "2026-07-11",
        },
        statement:
          "From the daily-coding capability audit (docs/fable/2026-07-11-daily-coding-capability-audit.md §4/§6): the local Fable lane must surface plan/task progress (J2 plan mode/plan review + J4 task/todo progress — both providers externalize plans constantly, 1,617 update_plan observations, yet the app rendered none of it and plan mode was disallowed); it must be able to steer or stop a running child (G4 steer/message running children — the app could spawn children but could not talk to or stop one); it must let a user queue a follow-up while a turn runs (A3 message queueing during a turn — steer-by-queueing is habitual in both CLIs); and it must load user-configured MCP servers (I2 — ~858 calls across Stripe/Expo/Apollo/docs/design servers with no config surface, only the internal delegate server).",
        authorityBoundary:
          "This contract binds the RUNTIME SUBSTRATE only (typed FableLocalEvent kinds + control channels + programmatic oracles); the renderer that draws these is a separate wave-2 lane. Everything is additive and default-off so current turn behavior is unchanged: plan mode is opt-in (default permissionMode 'default'); TodoWrite (never disallowed) emits plan_updated additionally to its tool_use trace; steerChild interrupts a running Codex delegate child (message is honestly 'unsupported' — codex exec is non-interactive and the SDK Agent tool exposes no per-subagent message API); queueFollowup is queue-until-idle (the single-string-prompt turn cannot inject mid-stream, so the queued message is promoted at the idle boundary, not steered mid-stream); user MCP servers are bounded/validated and merged next to the internal 'codex' server (reserved), and a failed or invalid server emits a typed mcp_server_unavailable without ever crashing the turn. The owner-local full-access posture (no allowedTools restriction, danger-full-access children) is preserved.",
        evidenceRefs: [
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "docs/fable/2026-07-11-daily-coding-capability-audit.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fable_local_runtime_capabilities.plan_todo",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime-caps.test.ts",
            description:
              "A TodoWrite tool call emits plan_updated with mapped {step,status} entries in addition to the raw tool_use; a non-TodoWrite tool never emits plan_updated and unknown todo status coerces to pending; the default turn uses permissionMode 'default' with ExitPlanMode disallowed, and opt-in planMode switches to permissionMode 'plan' and allows ExitPlanMode while Skill/EnterPlanMode stay disallowed.",
          },
          {
            id: "fable_local_runtime_capabilities.child_steer",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime-caps.test.ts",
            description:
              "steerChild reaches a running delegate child: interrupt emits child_steered(interrupted) and the turn completes; message is honestly unsupported and a later interrupt still ends the turn; an unknown child or turn mismatch returns not_found with no event; and a whole-turn interrupt also aborts the running child.",
          },
          {
            id: "fable_local_runtime_capabilities.queue_followup",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime-caps.test.ts",
            description:
              "queueFollowup during a streaming turn emits followup_queued (with position) and, at turn end, followup_promoted with the queued message, ordered after turn_completed; two queued follow-ups take positions 1 and 2 with only the first promoted per turn end (FIFO); and a queue with no live turn returns no_active_turn and emits nothing.",
          },
          {
            id: "fable_local_runtime_capabilities.user_mcp_servers",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime-caps.test.ts",
            description:
              "An enabled stdio/http server is merged into Options.mcpServers and its mcp__name__tool is allow-listed via canUseTool (no delegate auto-allow when no delegate); an SDK-reported failed server and an invalid config (bad name / reserved codex / missing transport field) each emit mcp_server_unavailable while the turn still completes; and the frozen FableLocalMcpServerConfig schema enforces its bounds (cap, transport enum, boolean enabled) with normalization rejecting bad/reserved/duplicate/missing-field entries.",
          },
          {
            id: "fable_local_runtime_capabilities.renderer_surface",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-cards.test.ts",
            description:
              "WAVE-2 landed for plan/todo, child steer/stop, and the queued-follow-up chip (see openagents_desktop.chat.runtime_capability_cards.v1). The MCP-server settings/status surface (I2) remains a separate wave-2 settings lane.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the fable-local runtime capability suite (src/fable-local-runtime-caps.test.ts) as programmatic oracles; the wave-2 renderer surfaces are proven by runtime-cards.test.ts + the smoke.",
      },
      {
        contractId: "openagents_desktop.chat.runtime_capability_cards.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript runtime-capability cards",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "capability-audit",
          statedBy: "agent",
          statedOn: "2026-07-11",
        },
        statement:
          "WAVE-2 renderer for the wave-1 runtime substrate (audit §4/§6 J2/J4 plan-todo progress, G4 steer/stop a running child, A3 queue a follow-up while a turn runs): the desktop transcript must render a compact task-progress checklist that updates in place, let the user Interrupt a running delegate child, retain each delegated child's bounded exact prompt and answer, show that conversation when its card or agent-row is selected, and let the user queue a follow-up while a turn streams.",
        authorityBoundary:
          "Renderer/presentation only over the bounded additive FableLocalEvent stream + control channels — no new authority is granted. child_started carries the exact bounded instruction and child_completed carries the exact bounded answer; the renderer retains both on the keyed child card and projects them as the selected child's user/Codex transcript above secondary runtime metadata. Older persisted cards without transcript remain schema-valid. plan_updated renders ONE compact plan card per turn (a status glyph per entry from the exact pending/in_progress/completed enum, the in_progress row emphasized) that replace-renders in place as new plan_updated events arrive (latest wins). A running delegate child offers a single Interrupt control that dispatches DesktopChildInterruptRequested -> fableLocal.steerChild(action:'interrupt') by exact { turnRef, childRef }; MESSAGE-ing an in-flight child is NOT offered. The composer stays usable while a turn streams; a mid-turn submit calls fableLocal.queueFollowup instead of starting a new turn, renders a 'Queued follow-up (#N)' chip, and clears it on followup_promoted. Token styling only; a host without a local streaming lane simply no-ops.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/runtime-cards.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "docs/fable/2026-07-11-daily-coding-capability-audit.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "runtime_capability_cards.plan_todo",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-cards.test.ts",
            description:
              "Proves plan_updated projects ONE plan card (keyed) that updates in place as entries change (latest wins), each row carrying the status glyph for the exact pending/in_progress/completed enum with the in_progress row emphasized, and the progress summary counting done/in-progress.",
          },
          {
            id: "runtime_capability_cards.child_steer",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-cards.test.ts",
            description:
              "Proves a running child card renders an Interrupt control (and NO message control), the control dispatches DesktopChildInterruptRequested with the exact { turnRef, childRef }, the child_steered outcome renders as a compact line, and a completed/failed or already-interrupted child no longer offers Interrupt.",
          },
          {
            id: "runtime_capability_cards.queue_followup",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-cards.test.ts",
            description:
              "Proves followup_queued renders a 'Queued follow-up (#N)' chip, followup_promoted clears it, the composer stays usable while pending with a queue placeholder, a mid-turn submit routes to queueFollowup (not sendMessage), and the promoted follow-up starts as the next turn.",
          },
          {
            id: "runtime_capability_cards.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke fable fixture turn scripts a TodoWrite plan_updated and asserts the plan/todo card renders in the transcript with status glyphs (no raw JSON, no SYSTEM label).",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs runtime-cards.test.ts + the local-harness projection suite as the renderer oracles and the Electron smoke plan-card step.",
      },
      {
        contractId: "openagents_desktop.seam.typed_git_github_surface.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Git/GitHub review surface",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "capability-audit", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Commit, push, branch, and GitHub issue/PR flows work from OpenAgents Desktop through a TYPED Git/GitHub surface with receipts — not only via an agent's Bash tool. The review workspace has a Git panel: a status header (branch, ahead/behind, dirty state), a changed-files list with stage toggles, a commit box that shows the resulting SHA, a Push button with its pushed-ref receipt and typed failure classes, a branch switcher, and an issues/PRs section that can list and create (returning the item url). (EP250 capability audit §4E, §6.3, ranks E2–E5 commit/push/PR/issue as a daily habit with zero typed UI.)",
        authorityBoundary:
          "The renderer never supplies argv: the host (git-github-host.ts) runs a FIXED, closed operation set over the active canonicalized workspace root, with user strings only reaching git/gh as validated path/ref/message values that cannot be reinterpreted as flags. Every request and result is Effect-Schema decoded on both sides (git-github-contract.ts). Results are public-safe — a commit SHA, a pushed ref, an issue/PR number+url — never tokens, credentials, raw stderr, or absolute paths; failures are typed classes (no_upstream, non_fast_forward, auth_failed, blocked_by_hook, dirty_tree, gh_unavailable, gh_unauthenticated, …). Commit refuses an empty message or nothing-staged; checkout refuses a dirty tree; push applies a bounded fetch→rebase→push retry; gh operations never trigger an auth prompt. This is owner-local executor authority (this is the owner's machine); it adds no untrusted-labor or provider authority.",
        seam: {
          client: "apps/openagents-desktop/src/git-github-contract.ts",
          server: "apps/openagents-desktop/src/git-github-host.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/git-github-host.ts",
          "apps/openagents-desktop/src/git-github-contract.ts",
          "apps/openagents-desktop/src/renderer/git-panel.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/preload.cts",
          "docs/fable/2026-07-11-daily-coding-capability-audit.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "git_github_surface.host_real_repo",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/git-github-host.test.ts",
            description:
              "Against a REAL temp git repo (and a REAL local bare remote): status structure, stage/unstage, commit returns the real HEAD SHA, empty-message and nothing-staged refusals, branch list/create/checkout, dirty-tree checkout refusal, push returns the pushed ref/sha and advances the remote, no-upstream refusal, and the non-fast-forward fetch→rebase→push retry (success on non-conflict, typed non_fast_forward after aborting a conflicting rebase). The gh path is boundary-unit-tested plus one guarded real gh --version/auth-status test that skips honestly when gh is unavailable.",
          },
          {
            id: "git_github_surface.contract_both_sides",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/git-github-contract.test.ts",
            description:
              "The closed operation set and every result variant (incl. the typed error) round-trip through decode on both sides; unknown ops and malformed params are rejected and excess keys stripped.",
          },
          {
            id: "git_github_surface.panel_intent_loop",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/git-panel.test.ts",
            description:
              "The panel renders the status header, changed-files stage toggles, commit box, Push control, branch switcher, and issues/PRs section; disabled controls (commit with nothing staged / empty message, Push without upstream, gh Create when gh is unavailable) carry the hover-only Tooltip reason; and the real intent loop stages, commits (showing the SHA receipt), pushes (ref receipt), checks out branches, marks gh unavailable with a reason, and creates an issue (url receipt) — never a fabricated success.",
          },
          {
            id: "git_github_surface.smoke_real_status",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke routes to the review workspace and asserts the typed Git panel rendered REAL read-only status of the app's own repo (status header, commit box, Push, branch switcher, issues/PRs section all present; branch label resolves to a real branch or detached HEAD) without committing or pushing.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the host real-repo suite, the contract both-sides suite, the panel intent-loop suite, and the Electron smoke git-review step.",
      },
      // =====================================================================
      // I2 (#8712 EP250 wave-2) — the MCP-config SETTINGS surface, landing the
      // renderer half that the runtime-capabilities contract left as a planned
      // wave-2 oracle. Realizes audit gap §6.5.
      // =====================================================================
      {
        contractId: "openagents_desktop.settings.mcp_servers.v1",
        state: "retired",
        surface: "openagents-desktop",
        productArea: "settings — user-configured MCP servers",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "capability-audit",
          statedBy: "agent",
          statedOn: "2026-07-11",
        },
        statement:
          "User-configured MCP servers — ~858 calls across Stripe/Expo/Apollo/docs/design servers. No config surface; only the internal delegate server exists. (missing, I2)",
        authorityBoundary:
          "The Settings screen gains an 'MCP servers' section (apps-sdk chrome + shared token styles only — the design-conformance oracle) that lists configured servers (name, transport chip, enable/disable Toggle, Remove) and an Add form whose stdio/http fields switch on a transport RadioGroup. Client-side validation mirrors the FROZEN FableLocalMcpServerConfig bounds (name charset/length, reserved 'codex', duplicate names, transport-specific required field, arg/env/header value bounds) and shows a single inline error before anything crosses to main. Persistence is a main-process host writing a private JSON file under userData mode 0600; env/header/arg VALUES are user-provided and may be sensitive, so they are persisted and handed to the fable-local runtime's userMcpServers getter (main-only) but are NEVER logged and NEVER cross back to the renderer — the projection carries name/transport/enabled/command/url and arg/env/header COUNTS only. Stored rows are re-validated against the frozen schema on read; invalid rows are dropped and counted, never crashing. A runtime-reported mcp_server_unavailable status renders a warn chip when threaded to settings; until that thread lands the section shows config state. This surface adds no new primitive: it composes the shared catalog TextField/RadioGroup/Toggle/Button/Badge only.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/settings.ts",
          "apps/openagents-desktop/src/mcp-config-host.ts",
          "apps/openagents-desktop/src/mcp-config-contract.ts",
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "docs/fable/2026-07-11-daily-coding-capability-audit.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "mcp_servers.settings_render_and_intent_loop",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/settings.test.ts",
            description:
              "The MCP servers section lists configured servers with a transport badge, enable/disable Toggle, and Remove; the Add form shows stdio vs http fields by transport; client-side validation rejects empty/invalid/reserved/duplicate names and missing transport fields with an inline error; and the add/toggle/remove intent loop drives a fake bridge, resets the draft on success, and ignores names not displayed.",
          },
          {
            id: "mcp_servers.persistence_host",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/mcp-config-host.test.ts",
            description:
              "The persistence host round-trips configs to a mode-0600 file, drops-and-counts schema-invalid stored rows without crashing, enforces the reserved/duplicate/transport/list-cap rules on add, and returns a renderer projection that never contains secret env/header values.",
          },
          {
            id: "mcp_servers.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke opens Settings, asserts the MCP servers section renders, adds a fixture stdio server through the real Add form + typed IPC, and asserts it persists and lists — without spawning a real MCP server (the fable query is a fixture in smoke).",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the settings renderer suite and the persistence-host suite as programmatic/UI oracles, plus the Electron smoke MCP add-and-list step.",
      },
      {
        contractId: "openagents_desktop.terminal.workspace_bounded_pty.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "workspace terminal / execution",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "audit", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Interactive terminal / stdin steering — the audit's #1 daily-coding gap (8,333 write_stdin observations, capability D3). Ordinary build/test/dev-server work must run in scoped Desktop terminals with an explicit local-preview lifecycle and no renderer ambient process authority. (CUT-20, #8700)",
        authorityBoundary:
          "The renderer holds no shell and no process: every terminal operation is a typed intent (create/input/resize/interrupt/restart/close/preview-open) schema-decoded on both sides of the sandbox. Main alone binds each session to the currently authorized workspace root + a bounded environment; the renderer sends a session ref and, for input/resize, bounded data / integer geometry — never a shell, argv, cwd, or env, so a compromised renderer can steer stdin but never chooses WHAT is spawned or WHERE. Output crossing to the renderer is BOUNDED (a byte-capped ring, loss-accounted with a gap flag) and REDACTED (secret-named/secret-shaped env VALUES and token-shaped literals are scrubbed in main before any chunk is sent). On project/workspace close the OWNED process tree is killed exactly once (SIGTERM then SIGKILL against the process group; a second close is a no-op). A bounded tail persists (mode 0600) and is reloaded as an explicitly recovered, gap-marked session after an app restart. Local preview discovers an EXPLICIT announced port parsed from the session's OWN output (never a port scan), shows readiness, and stops with its owning session; opening it is out-of-process (external browser) behind a confirmation — never arbitrary in-app navigation. The shipped backend is a child-process-group terminal (zero native deps, runs under pnpm exec vp test AND Electron); node-pty pseudo-TTY + xterm.js are a documented TerminalBackend swap deferred to the #8574 packaging lane. The terminal UI (bounded monospace output + a typed input line + interrupt/restart) composes only shared catalog primitives on the design-conformance token scales.",
        evidenceRefs: [
          "apps/openagents-desktop/src/terminal-host.ts",
          "apps/openagents-desktop/src/terminal-contract.ts",
          "apps/openagents-desktop/src/renderer/terminal-workspace.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/preload.cts",
          "docs/fable/2026-07-11-daily-coding-capability-audit.md",
          "github:OpenAgentsInc/openagents#8700",
        ],
        oracles: [
          {
            id: "terminal.adversarial_and_built_host_receipt",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/terminal-host.test.ts",
            description:
              "The adversarial PTY suite — shell injection (renderer input never becomes argv; the spawn argv stays fixed and workspace-bound), secret environment (bound secret values redacted in the streamed chunk AND the tail), runaway output (ring buffer holds the byte cap and marks gap), orphan children (closing a session reaps a REAL backgrounded grandchild via process-group kill — the disposal evidence), duplicate start, port collision (typed error on a second live session claiming the same announced port), and revoked grants — plus the built-host receipt (a real /bin/sh runs a stdin command, output captured, exit code observed, tree disposed) and the dev-preview receipt (a real server's announced port is detected + reachable, then freed when the session stops).",
          },
          {
            id: "terminal.renderer_intent_loop",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/terminal-workspace.test.ts",
            description:
              "The terminal workspace transitions (ready/output/exit/preview/closed/error + snapshot recovery) and the typed intent loop through the real registry with a fake bridge: create adds the returned session, submit writes the input line to stdin with a newline and clears the field, interrupt/restart target the active session, and a failed preview-open surfaces a typed notice — all on the design-conformance token scales.",
          },
          {
            id: "terminal.smoke_built_electron_pty_receipt",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke routes to the terminal workspace through the canonical workspace.terminal command, then runs a REAL bounded command through the real preload bridge + real main PTY host (bound to the app's own repo in smoke), asserts the ready + redacted output events, closes the session, and the lifecycle-teardown asserts zero live terminal sessions remain.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the adversarial PTY host suite and the renderer intent-loop suite as programmatic/UI oracles, plus the built-Electron smoke terminal PTY receipt step and its lifecycle-teardown disposal check.",
      },
      {
        contractId: "openagents_desktop.preferences.typed_durable_migratable_schemas.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "app preferences & operability",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "issue", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Theme, density, font, reduced motion, keybindings, provider defaults, privacy, notifications, and update preferences have typed durable schemas/migrations.",
        authorityBoundary:
          "A single versioned, migratable preferences document (<userData>/preferences.json, mode 0600) owns density, font, reduced-motion, built-in Vim enablement, provider-defaults, privacy, notifications, and update preferences. Theme is intentionally NOT mutable: the app mounts the fixed pinned Tokyo Night projection; keybindings keep their existing typed store (desktop-command-bindings). Density and font resize the app through a scaled theme, reduced-motion resolves to a root attribute, and Vim is off by default but persists when toggled. The migrator is total: a missing, corrupt, partial, legacy, or future-versioned file always resolves to a valid current document and never throws.",
        evidenceRefs: [
          "https://github.com/OpenAgentsInc/openagents/issues/8704",
          "apps/openagents-desktop/src/desktop-preferences-contract.ts",
          "apps/openagents-desktop/src/desktop-preferences-host.ts",
          "apps/openagents-desktop/src/desktop-preferences-effects.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "apps/openagents-desktop/src/renderer/boot.ts",
        ],
        oracles: [
          {
            id: "preferences.migration_and_host_and_effects",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-preferences.test.ts",
            description:
              "The migration chain (current/defaults/legacy_v0/merged/downgraded) is total and value-preserving, the host round-trips to a mode-0600 file and self-heals legacy bytes, and the effects module scales the theme (font/density) and maps reduced-motion to the root attribute with defaults returning the identity theme.",
          },
          {
            id: "preferences.durable_ipc_round_trip.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke round-trips preferences over the real IPC (update density→compact, read back compact, reset→comfortable) in the diagnostics-and-preferences step.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the preferences migration/host/effects suite and the Electron smoke preferences round-trip.",
      },
      {
        contractId: "openagents_desktop.accessibility.core_flows_meet_wcag_aa.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "accessibility",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "issue", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Desktop/mobile meet keyboard, focus, screen-reader, contrast, dynamic type, target-size, and reduced-motion acceptance for core coding flows.",
        authorityBoundary:
          "This contract binds the DESKTOP surface only. It guarantees: WCAG 2.1 AA text contrast on the theme's primary/secondary/status text roles across all four surfaces; a high-contrast, clearly-visible focus ring (focus token ≈ 7.9:1 on background); reduced-motion honored both via the OS prefers-reduced-motion media query AND an explicit in-app override; and accessible names on every interactive node in the diagnostics/preferences operability surfaces. Disabled text is treated per the WCAG 1.4.3 exemption. Mobile accessibility for core coding flows is a SEPARATE app (apps/openagents-mobile) and is NOT covered by this contract — it remains the named residual on #8704.",
        evidenceRefs: [
          "https://github.com/OpenAgentsInc/openagents/issues/8704",
          "apps/openagents-desktop/tests/accessibility.test.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "apps/openagents-desktop/docs/2026-07-11-cut24-accessibility-audit.md",
        ],
        oracles: [
          {
            id: "accessibility.contrast_and_reduced_motion",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/accessibility.test.ts",
            description:
              "Computes WCAG relative-luminance contrast for the theme text roles and asserts AA (primary/secondary/status ≥ 4.5:1, faint/accent ≥ 3:1, focus ring high-contrast); asserts app.css honors both the OS prefers-reduced-motion media query and the explicit data-en-reduce-motion override.",
          },
          {
            id: "accessibility.diagnostics_accessible_names",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/diagnostics.test.ts",
            description:
              "Every interactive control in the diagnostics panel carries a non-empty accessible name (Button label), each health row is a labelled group region, and no rendered text leaks a path/url/token.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the accessibility contrast/reduced-motion suite and the diagnostics accessible-name suite. Mobile a11y is tracked separately as the #8704 residual.",
      },
      {
        contractId: "openagents_desktop.notifications.refs_only_and_authoritative_clear.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "notifications & attention",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "issue", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Notifications carry stable authorized refs and never prompt/code/secrets; attention clears only after authoritative acknowledgement.",
        authorityBoundary:
          "The desktop notification-analog is the confirmed live-agent-graph attention projection (attentionCount / attentionLabel) plus the typed notification preference payload. Attention surfaces only enum-derived labels and public-safe refs (never the underlying question/approval prompt text), and it reflects the newest confirmed graph cursor only — a stale lower-cursor snapshot can neither raise nor clear attention optimistically. The notification preference payload is boolean-only, with no content field a prompt/secret could ride in on. The in-transcript question/approval CARD (which does carry prompt text) is a separate detail surface and is never routed into a notification payload.",
        evidenceRefs: [
          "https://github.com/OpenAgentsInc/openagents/issues/8704",
          "apps/openagents-desktop/tests/notification-attention.test.ts",
          "apps/openagents-desktop/src/renderer/runtime-interactions.ts",
          "apps/openagents-desktop/src/agent-graph-presentation.ts",
        ],
        oracles: [
          {
            id: "notifications.refs_only_authoritative_clear",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/notification-attention.test.ts",
            description:
              "An approval/question attention projects an enum-derived label (not the prompt), the serialized projection carries no prompt/secret text, newestLiveAgentGraph picks the higher-cursor confirmed graph so attention clears only on the authoritative snapshot (and a stale snapshot never overrides it), and the notification preference payload rejects any content string.",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the notification/attention suite; the authoritative-clearing of interactive decisions is additionally exercised by runtime-interactions.test.ts.",
      },
      {
        contractId: "openagents_desktop.diagnostics.watchdog_redacted_export_and_recovery.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "diagnostics & recovery",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "issue", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Diagnostics/watchdog show provider, Runtime Gateway, Sync, workspace, PTY, and extension health with redacted export, restart, and recovery actions.",
        authorityBoundary:
          "The diagnostics panel projects public-safe health for all six domains (provider, Runtime Gateway, Sync, workspace, PTY, extensions). Health rows carry only a bounded domain/level enum, a short public-safe summary, and public-safe refs — never a path, email, prompt, token, or url (structural privacy). The export is ALWAYS redacted before it touches disk (a secret-pattern scrubber runs even if an upstream builder regresses), and the returned notice never carries the saved path. Recovery actions map only to safe typed paths: provider re-probe re-checks accounts, and refresh/refresh_workspace/reload_extensions re-gather fresh sources. PTY health is honestly 'unavailable' until the CUT-20 (#8700) PTY host merges, and restart_runtime/reconnect_sync report 'no recovery action available' until a safe typed restart exists — surfaced honestly, never faked.",
        evidenceRefs: [
          "https://github.com/OpenAgentsInc/openagents/issues/8704",
          "apps/openagents-desktop/src/diagnostics-contract.ts",
          "apps/openagents-desktop/src/diagnostics-report.ts",
          "apps/openagents-desktop/src/diagnostics-host.ts",
          "apps/openagents-desktop/src/renderer/diagnostics.ts",
        ],
        oracles: [
          {
            id: "diagnostics.builder_redaction_and_host",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/diagnostics.test.ts",
            description:
              "The report builder maps each domain to the right level under fault injection (provider outage, closed/unobserved sync, gateway lifecycle/capability degradation, git-unavailable workspace, dropped MCP rows) and never emits a path/token; redaction scrubs a leaked secret and keeps the report schema-valid + export-safe; and the host writes an owner-only redacted bundle whose notice carries no saved path.",
          },
          {
            id: "diagnostics.view_and_handler_loop",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/diagnostics.test.ts",
            description:
              "The panel renders a row + level badge per domain, refresh/export/recovery intents drive a fake bridge (a successful recovery re-gathers), a corrupt gather resolves to unavailable (never a throw), and no rendered text leaks a path/token.",
          },
          {
            id: "diagnostics.renders_and_exports.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke opens Settings, asserts all six diagnostics health rows render with level badges, asserts no rendered diagnostics text is secret-like, clicks Export and asserts a public-safe notice appears (no saved path).",
          },
        ],
        verification:
          "pnpm --dir apps/openagents-desktop run verify runs the diagnostics builder/redaction/host suite, the diagnostics view/handler suite, and the Electron smoke diagnostics-and-preferences step.",
      },
      {
        contractId: "openagents_desktop.microinteraction.owner_review_register.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "micro-interaction do/don't register",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "I want the ones that we have for that for this app to include micro interactions and things that I do and don't want to see. There are some things I don't want to see, such as long streams of text where icons should be.",
        authorityBoundary:
          "This contract is the REGISTER for owner-stated micro-interaction and visual do/don't rules on this app: every future 'things I do and don't want to see' statement lands here as its own versioned contract with a real oracle in the same change (house law 2026-07-03), never conversation-only. The register grants no rendering authority and does not make any individual rule true — each concrete rule (starting with openagents_desktop.microinteraction.icon_slot_no_raw_text.v1 and openagents_desktop.typography.approved_fonts_only.v1) carries its own oracle and its own honest state. AssuranceSpec obligations may cite these contractIds via contract_refs for environment-bound pixel evidence later; this registry stays the single source of the rule text and AssuranceSpec never duplicates it (docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md).",
        evidenceRefs: [
          "contract:openagents_desktop.microinteraction.icon_slot_no_raw_text.v1",
          "contract:openagents_desktop.typography.approved_fonts_only.v1",
          "docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md",
          "apps/openagents-desktop/tests/owner-ux-rules.test.ts",
        ],
        oracles: [
          {
            id: "microinteraction_register.rules_present_and_documented",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/owner-ux-rules.test.ts",
            description:
              "Proves both concrete owner rules exist in this registry as enforced contracts whose oracle refs point at the real enforcing suite, that both owner statements are recorded verbatim, and that docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md exists and names all three contractIds so the next owner rule has a documented home.",
          },
        ],
        verification:
          "pnpm exec vp test apps/openagents-desktop/tests/owner-ux-rules.test.ts runs in the normal desktop sweep; it fails if either concrete rule is removed, renamed, downgraded from enforced, detached from its oracle file, or undocumented in the assurance clarification doc.",
      },
      {
        contractId: "openagents_desktop.microinteraction.icon_slot_no_raw_text.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "icon-slot micro-interactions",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "I want the ones that we have for that for this app to include micro interactions and things that I do and don't want to see. There are some things I don't want to see, such as long streams of text where icons should be.",
        authorityBoundary:
          "Icon slots are the closed-catalog glyph positions: sidebar dock items, icon-only action controls (IconButton and any icon-carrying node), and status indicator glyphs. Each must resolve a glyph name in the closed @effect-native/core iconNames catalog — never a raw string rendered where the glyph belongs — and dock labels stay bounded single-line micro-copy (24-char bound). Accessible labels are announced, not painted, and stay full-length. ENFORCED SUBSET (honest): the oracle proves the structural rule on the real typed view trees across sampled shell states — every workspace dock item carries a catalog glyph plus bounded single-line label, every icon-carrying node's glyph resolves in the closed catalog, and every IconButton is glyph-plus-accessible-label with no rendered text content — and demonstrates sensitivity against known-bad fixtures. The fully general 'no long text ever appears where an icon was designed' claim over arbitrary rendered pixels is NOT mechanically expressible on typed trees today; it lands later as an AssuranceSpec obligation with technique visual citing this contractId (ASSURANCE_SPEC.md §5), and this contract does not claim that pixel evidence exists yet.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents.com/packages/effect-native-core/src/index.ts",
          "docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md",
        ],
        oracles: [
          {
            id: "icon_slots.closed_catalog_glyphs_and_bounded_labels",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/owner-ux-rules.test.ts",
            description:
              "On the real desktopShellView trees across sampled states: every workspace dock item carries a closed-catalog glyph, a non-empty single-line label within the 24-char micro-copy bound, and an accessible label; every node carrying an icon prop and every Icon node resolves in the closed iconNames catalog; every IconButton carries a glyph and accessible label and no rendered text content. The falsifier test proves the validators reject an unknown glyph, an empty accessible label, rendered IconButton text, and a long-stream dock label.",
          },
        ],
        verification:
          "pnpm exec vp test apps/openagents-desktop/tests/owner-ux-rules.test.ts runs in the normal desktop sweep and enforces the structural subset on real view trees; the residual pixel-level generalization is deferred to an AssuranceSpec visual-technique obligation referencing this contractId, per docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md.",
      },
      {
        contractId: "openagents_desktop.typography.approved_fonts_only.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "typography",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "I want all that enforced in the assurance pieces. I want to be able to specify rules there. For example, I don't want to see certain things like strings where icons should be, certain fonts, and that must be specified",
        authorityBoundary:
          "The shared @openagentsinc/ui typography authority self-hosts Inter as the primary body, UI, heading, navigation, and conversation family, followed by the host system stack as resilient fallbacks. Zalando Sans is demoted to the explicit --oa-font-sans-accent token for occasional marketing copy and must never become an implicit product-UI family. The authority self-hosts Disket Mono as the primary code and metadata family, followed by the approved system monospace stack. Web, docs, splash, and Desktop consume the same --oa-font-sans and --oa-font-mono tokens. The @effect-native/tokens type scale deliberately carries size/weight only — no competing family tokens — so no renderer module, stylesheet, or typed style object may declare an unapproved family, and the CSS font shorthand stays exactly the form-control 'font: inherit' reset so a family cannot ride past the family checks.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/app.css",
          "apps/openagents.com/packages/effect-native-tokens/src/index.ts",
          "docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md",
        ],
        oracles: [
          {
            id: "typography.approved_font_stack_only",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/owner-ux-rules.test.ts",
            description:
              "Scans every non-test .ts/.cts/.css file under apps/openagents-desktop/src and packages/ui/src recursively: every CSS font-family declaration and every TypeScript fontFamily value must resolve to the shared Inter, opt-in Zalando Sans accent, or Disket Mono tokens and approved fallback families, every CSS font shorthand must be exactly 'inherit', and the shared token declarations must remain consumed by app.css and desktop-workbench.css. The falsifier test proves a rogue family (Comic Sans MS / Papyrus / a font shorthand smuggle) is rejected while the approved stacks pass.",
          },
        ],
        verification:
          "pnpm exec vp test apps/openagents-desktop/tests/owner-ux-rules.test.ts runs in the normal desktop sweep; adding any stray font family anywhere under apps/openagents-desktop/src fails it.",
      },
      {
        contractId: "openagents_desktop.design.khala_autopilot_foldin.v1",
        state: "retired",
        surface: "openagents-desktop",
        productArea: "product theme / palette",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-16" },
        statement:
          "Some recent Autopilot UI change changed my blue background to gray, undo that. The Autopilot palette needs to be kinda folded INTO Khala: use its mono/condensed ideas and generally its colors, but do not override or conflict with Khala.",
        authorityBoundary:
          "Retired on 2026-07-19 by the owner's newer IDE roadmap directive selecting Tokyo Night as the initial theme for everyone. The one-theme-many-hosts, shared-scale, no-Autopilot-override, and no-mutable-theme invariants remain; only the former Khala color identity is superseded by openagents_desktop.ide_monaco_document_runtime.v1.",
        evidenceRefs: [
          "apps/openagents.com/packages/effect-native-tokens/src/index.ts",
          "apps/openagents-desktop/src/renderer/theme.ts",
          "apps/openagents-desktop/index.html",
          "apps/openagents.com/apps/start/src/routes/-splash-page.tsx",
          "apps/openagents.com/apps/start/src/routes/-components-storybook-page.tsx",
          "docs/fable/autopilot-ui-design-spec.md",
          "INVARIANTS.md",
          "github:OpenAgentsInc/openagents#8858",
          "github:OpenAgentsInc/effect-native#102",
        ],
        oracles: [
          {
            id: "khala_autopilot_foldin.theme_restoration",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/owner-ux-rules.test.ts",
            description:
              "Historical oracle now proves the successor Tokyo Night projection is mounted over the shared scales and no desktop module mounts the temporary autopilotTheme.",
          },
          {
            id: "khala_autopilot_foldin.theme_is_canonical",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "The theme-parity suite pins the successor Tokyo Night colors while retaining the shared radius, motion, and control scales.",
          },
        ],
        verification:
          "pnpm exec vp test apps/openagents-desktop/tests/owner-ux-rules.test.ts plus the shell theme-parity, design-conformance, and startup boot-frame suites in the normal desktop sweep.",
      },
      {
        contractId: "openagents_desktop.startup.window_first_no_blank_frame.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "startup / boot process",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-incident", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "Opening the openagents app, via our new oa command or in dev, shows a blank/brown screen for ~5 seconds before opening the UI. This is unacceptable. I thought we had a UX contract somewhere about the need to show initial codex chats in <50 ms. That should be timed from startup. Go look up our ways of testing load times. Startup and everything else. Write full analysis of current situation in openagents/docs/fable/ new doc. This is an incident. Very bad. Need good bootup process. No brown screen. If any loading, show beautiful starcraft version of it, or something. Time to seeing stuff and then interactable elements on bootup is extremely important. Analyze, fix, update analysis, push.",
        authorityBoundary:
          "Ordinary launch uses a non-persistent in-memory Chromium partition and never resolves Electron safeStorage, reads/decrypts native credential custody, or performs session network verification; only an explicit account command may initialize secure custody. The BrowserWindow is created before local database work. The renderer paints a static branded boot frame (product-theme literals mechanically synced to @effect-native/tokens) with the first HTML parse, mounts the interactable shell BEFORE the local coding-history scan, and publishes the Codex-only top-level metadata catalog without requesting any selected-thread detail. Closed overlays perform zero catalog projection work and recent-only projections inspect a fixed-size prefix, never the full loss-accounted catalog. Hydration streams behind an explicit 'Scanning coding history…' sidebar state — the 'No local Codex history found.' claim renders only after the scan settles. This contract governs boot ordering and honest loading presentation; it does not change the separate post-selection thread_first_content_under_50ms.v1 projection budget, and it does not promise a wall-clock bound for full history hydration on arbitrary ~/.codex sizes (bounding the scan itself is follow-up work, now off the critical path).",
        evidenceRefs: [
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/index.html",
          "apps/openagents-desktop/tests/startup-contract.test.ts",
          "apps/openagents-desktop/scripts/startup-bench.ts",
          "apps/openagents-desktop/benchmarks/startup/2026-07-13-window-first-boot-frame.json",
          "docs/fable/2026-07-13-desktop-startup-incident.md",
          "docs/transcripts/248.md",
        ],
        oracles: [
          {
            id: "startup.window_first_ordering",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/startup-contract.test.ts",
            description:
              "Proves ordinary launch uses an in-memory renderer session and contains no persistent-session, safeStorage, credential-recovery, or session-verification access; secure custody is confined to explicit account commands and falsifier fixtures reject delayed as well as pre-window recovery.",
          },
          {
            id: "startup.shell_mounts_before_hydration",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/startup-contract.test.ts",
            description:
              "Proves the renderer mounts the shell before the coding-history hydration effect runs, publishes catalog metadata without selected-thread detail, keeps the MVP history host Codex-only, and removes the boot frame after mount. A known-bad startup detail-autoload fixture is rejected.",
          },
          {
            id: "startup.boot_frame_token_sync",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/startup-contract.test.ts",
            description:
              "Proves the branded boot frame exists in index.html and every color literal in it is an exact Tokyo Night projection value — no off-palette frame can paint — and BrowserWindow backgroundColor stays the projected background.",
          },
          {
            id: "startup.sidebar_scanning_honesty",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/startup-contract.test.ts",
            description:
              "Proves the typed shell view renders the scanning row while history hydration is pending and the empty-history claim only after hydration settles.",
          },
          {
            id: "startup.bench_budgets",
            kind: "script",
            mode: "headless",
            ref: "apps/openagents-desktop/scripts/startup-bench.ts",
            description:
              "The fixture-mode startup bench asserts median windowReadyToShow < 1500 ms and median shellMounted < 2500 ms and writes a timings-only receipt to benchmarks/startup/; the real-wiring OPENAGENTS_DESKTOP_STARTUP_TRACE mode records the same milestone chain (plus historyHydrated) against a real profile.",
          },
        ],
        verification:
          "Desktop typecheck, tests/startup-contract.test.ts in the normal sweep, scripts/startup-bench.ts receipts (real-profile before: shellMounted 5.4–7.0 s; after: ~0.7 s), and smoke screenshot receipts of the boot frame and mounted shell.",
      },
      {
        contractId: "openagents_desktop.workbench.turn_checkpoints.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Coding workbench turn checkpoints",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "Desktop coding turns mutate the workspace with no cheap per-turn restore point. Capture at turn start/completion via an isolated temp GIT_INDEX_FILE, write hidden refs (refs/openagents/checkpoints/<thread>/<turn>), bounded to tracked + non-ignored files with size exclusions; never touches user branches/index. Typed revert as an explicit command with an irreversible-effects statement: stage, inspect, commit/clear; refuse on dirty conflicting state.",
        authorityBoundary:
          "The host-side turn-checkpoint service owns hidden-ref capture, the typed turn-over-turn diff query, and the staged revert command. Every snapshot is built through an isolated temporary GIT_INDEX_FILE: user branches, HEAD, the user index, stashes, and (during capture) the worktree are never written. Capture is bounded to tracked plus non-ignored untracked files with a per-file size exclusion and a total-file refusal. Revert only ever runs as stage then inspect then explicit commit; every staged revert carries the irreversible-effects statement, refuses dirty conflicting state, and retains a pre-revert baseline snapshot. Checkpoint refs and snapshots stay in the local repository only — they can contain secrets and never enter Sync projections, renderer state, or push surfaces. Thread checkpoint deletion removes every hidden ref for that thread. This is an internal post-MVP substrate: it authorizes no visible renderer affordance under the MVP surface allowlist.",
        evidenceRefs: [
          "apps/openagents-desktop/src/turn-checkpoint-contract.ts",
          "apps/openagents-desktop/src/turn-checkpoint-host.ts",
          "apps/openagents-desktop/src/main.ts",
          "docs/teardowns/2026-07-13-t3-code-teardown.md",
          "docs/teardowns/2026-07-10-opencode-v2-architecture-teardown.md",
          "github:OpenAgentsInc/openagents#8781",
        ],
        oracles: [
          {
            id: "turn_checkpoints.capture_hidden_ref_untouched_user_state",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/turn-checkpoints.test.ts",
            description:
              "Against a real fixture repository: capture on a fixture turn creates the hidden ref (ignored and oversized files excluded, typed completion signal emitted) with byte-identical user branches, HEAD, index, status, and stashes before and after.",
          },
          {
            id: "turn_checkpoints.staged_revert_exact_bytes_and_transitions",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/turn-checkpoints.test.ts",
            description:
              "Proves the typed diff query reports real turn-over-turn changes; stage/inspect/commit restores exact text and binary bytes (deleting later-turn artifacts) while inspect carries the irreversible-effects statement; clear abandons without mutation; double-stage, commit-without-stage, dirty stage, and post-stage drift all refuse typed; thread deletion removes that thread's refs only.",
          },
        ],
        verification:
          "Desktop typecheck and tests/turn-checkpoints.test.ts in the normal sweep; the suite also proves main.ts wires capture at turn_start and turn_completed on both local lanes.",
      },
      {
        contractId: "openagents_desktop.settings.harness_maintenance_one_click.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Settings harness maintenance",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-issue", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "fleet(MAINT-1): one-click provider install/update with ledger pinning and provenance receipts — typed per-harness maintenance actions: detect installed version + channel, resolve latest per channel, execute update via the harness's native path, then RE-PROBE the harness before reporting success; version pinning (record expected version/hash before update; refuse silent channel jumps); a provenance receipt for the swapped binary; one click in Desktop Settings per connected harness driving the typed action through the existing command path; CLI parity via pylon command; NEVER touch the default ~/.codex login home during update flows.",
        authorityBoundary:
          "Desktop Codex projects the immutable installed-Codex CodexRuntimeResolution used by turns. OpenAgents never packages, copies, re-signs, installs, or mutates that executable. Discovery is bounded to documented/native absolute locations and the launch PATH; the selected identity is pinned for the process lifetime. Missing or incompatible Codex projects an install/update-and-restart instruction, while the user's normal CODEX_HOME and authentication remain untouched. The renderer receives only bounded state/provenance/versions/recovery — never paths, tokens, homes, or raw output. Signed makes prove that no @openai/codex package or native Codex executable enters the staged closure.",
        evidenceRefs: [
          "packages/pylon-core/src/custody/harness-maintenance.ts",
          "packages/pylon-core/src/custody/harness-maintenance.test.ts",
          "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          "apps/openagents-desktop/src/runtime-gateway.ts",
          "apps/openagents-desktop/src/renderer/settings.ts",
          "apps/openagents-desktop/src/provider-runtime-host.ts",
          "apps/openagents-desktop/src/provider-runtime-host.test.ts",
          "apps/openagents-desktop/tests/package-macos.test.ts",
          "apps/pylon/tests/accounts-maintenance-cli.test.ts",
          "docs/fable/2026-07-13-chatgpt-codex-launch-failure-analysis.md",
          "docs/receipts/2026-07-14-harness-maintenance/README.md",
          "github:OpenAgentsInc/openagents#8785",
        ],
        oracles: [
          {
            id: "harness_maintenance.engine_round_trip_and_guards",
            kind: "bun-test",
            mode: "unit",
            ref: "packages/pylon-core/src/custody/harness-maintenance.test.ts",
            description:
              "Fixture-harness round trip with REAL spawned fixture binaries: detect → pin → update → re-probe → receipt; failure paths (update fails, post-update probe fails, version unchanged) keep the previous state intact in the receipt; channel jumps are refused without execution; the fixture ~/.codex/auth.json is byte-identical after every flow and auth-flow arguments are refused.",
          },
          {
            id: "harness_maintenance.cli_parity",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/pylon/tests/accounts-maintenance-cli.test.ts",
            description:
              "`pylon accounts maintenance --json` projects version/channel/advisory and `--update --harness codex` runs the same engine end to end against fixture binaries and a local registry: provenance receipt persisted under the Pylon home, channel jump refused with non-zero exit, fixture ~/.codex untouched.",
          },
          {
            id: "harness_maintenance.desktop_gateway_and_settings",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/harness-maintenance.test.ts",
            description:
              "The maintenance query/command decode through the versioned gateway contract (unknown harnesses refused), dispatch to injected host actions with per-harness single-flight, and Settings renders version/channel truth with the one-click update affordance driving the typed intent; failure and channel-jump-refusal outcomes surface honestly.",
          },
        ],
        verification:
          "Desktop typecheck, the three oracle suites in the normal sweep, and the built-host smoke settings capture (docs/receipts/2026-07-14-harness-maintenance/) showing the rendered harness rows.",
      },
      {
        contractId: "openagents_desktop.composer.focused_on_open.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat composer / window open",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-incident", statedBy: "owner", statedOn: "2026-07-14" },
        statement:
          "the text input should be focused immediately on open. so i can start typing right away.",
        authorityBoundary:
          "On window open — fresh launch and macOS re-activate with an existing window — keyboard focus lands in the message composer at SHELL-INTERACTABLE (the moment the shell mounts under the branded boot frame, composing with window_first_no_blank_frame.v1's boot ordering), so the first keystroke enters the composer with zero clicks. Background history hydration must never steal that focus; conversely the automatic settle passes (post-hydration, window re-activate) claim only UNOWNED focus (document.activeElement at body/root) and never move focus the user placed elsewhere. When the restored workspace renders no composer (a loaded history page), nothing is force-focused.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/composer-focus.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/main.ts",
          "github:OpenAgentsInc/openagents#8787",
        ],
        oracles: [
          {
            id: "composer_focus.dom_focus_and_no_steal",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/composer-focus.test.ts",
            description:
              "DOM-level: the focuser lands document.activeElement on the composer at mount (and across late commits), a keystroke at the active element routes to the composer, the settle pass claims unowned focus, and it NEVER steals focus the user placed in another input.",
          },
          {
            id: "composer_focus.built_electron_first_keystroke",
            kind: "script",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "Built-Electron smoke steps composer-focused-on-open and first-keystroke-lands-in-composer: before ANY pointer event, document.activeElement is the composer at shell-mount and still after hydration settles, then a real Chromium keyboard event sent from the main process appears as typed text in the composer.",
          },
        ],
        verification:
          "Desktop typecheck, src/renderer/composer-focus.test.ts in the normal sweep, and the built-host smoke's composer-focused-on-open + first-keystroke-lands-in-composer steps.",
      },
      {
        contractId: "openagents_desktop.history.session_search_filters.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "sidebar session search",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-incident", statedBy: "owner", statedOn: "2026-07-14" },
        statement:
          "The search doesn't seem to fucking work at all. One of the chats is titled Assurance, but when I start typing in the first few letters there, it does not show it.",
        authorityBoundary:
          "Typing in the sidebar session search filters the list with case-insensitive substring matching over session titles and workspace labels — bounded deterministic field matching over the owner-local corpus (the semantic-routing invariant's bounded-field exception), never keyword intent routing. The search operates over the FULL loss-accounted catalog store (every root, including beyond the sidebar's ten recent rows), not just rendered rows: instant title matches come straight from the hydrated catalog cache, and the host content index (itself now byte-bounded per session, so a multi-GB rollout can no longer crash or starve it) merges in when it settles. While the host response is in flight the empty state says 'Searching…'; 'No sessions match.' renders only once settled; clearing the query restores the bounded recent list. The index remains a rebuildable cache, never catalog/page authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/history-search.ts",
          "apps/openagents-desktop/src/merged-history.ts",
          "github:OpenAgentsInc/openagents#8788",
        ],
        oracles: [
          {
            id: "session_search.filters_full_catalog",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Through the real intent registry on a 45-root fixture catalog: a title prefix ('Ass') filters to the matching session even though it sits beyond the ten recent rows; the no-match state is explicit; clearing restores the recent-ten list; a deferred host response shows 'Searching…' (never a false no-match) and merges content results when it settles.",
          },
          {
            id: "session_search.instant_and_merge_helpers",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
            description:
              "historyImmediateSearchResults prefix-matches the full catalog case-insensitively including beyond-page roots; mergeHistorySearchResults dedupes by threadRef, keeps host content matches, ranks by score, and degrades to instant matches on a null host response.",
          },
          {
            id: "session_search.workspace_label_and_bounded_index",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/history-catalog-scale.test.ts",
            description:
              "The host index carries the session's workspace label (cwd basename) as a searchable title-tier field, and its per-session content read is item- and byte-bounded rather than a whole-file read.",
          },
          {
            id: "session_search.built_electron_filter_journey",
            kind: "script",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "Built-Electron smoke types a title prefix into the sidebar search and asserts the fixture session row appears, then a no-match query shows the explicit empty state and clearing restores the full list — with a pixel receipt of the filtered sidebar.",
          },
        ],
        verification:
          "Desktop typecheck, the shell/history-workspace/history-catalog-scale suites, and the built-host smoke's session-search steps with screenshot receipts.",
      },
      {
        contractId: "openagents_desktop.history.recent_ten_search_all.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "sidebar recent chats and full-history search",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-16" },
        statement:
          "in openagents, I only want to see the most recent ten chats in the sidebar. Search should search through all chats, but on the recent, only show the most recent ten.",
        authorityBoundary:
          "The unfiltered Desktop sidebar projects exactly the ten newest-created unique Codex chats across app-local threads and the recent-first loss-accounted Codex catalog, or every chat when fewer than ten exist. Later activity in an older chat does not reorder it. The recent list has no load-more affordance and cannot expand through the legacy catalog window. This is presentation-only: the full catalog remains hydrated and searchable, instant title matching reads every catalog root, and the bounded host content index may return matching chats outside the recent ten. Clearing search returns to the recent-ten projection. Search does not become catalog authority and no session identity, persistence, or execution authority changes.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "apps/openagents-desktop/src/renderer/shell.test.ts",
        ],
        oracles: [
          {
            id: "recent_chats.exact_ten_without_paging",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Projects a mixed local/catalog history larger than ten, proves the normal sidebar renders exactly the ten newest unique chats, exposes no load-more row, and cannot expand when the legacy visible-root count changes.",
          },
          {
            id: "recent_chats.search_reaches_full_catalog",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Searches a 45-root fixture for a title outside the recent ten, proves the result appears through the real intent registry, then clears search and proves the sidebar returns to exactly ten recent rows without the out-of-window match.",
          },
        ],
        verification:
          "Desktop shell tests and typecheck enforce the recent-ten projection and preserve full-catalog search semantics.",
      },
      {
        contractId: "openagents_desktop.history.sidebar_header_truthful_scope.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "sidebar coding-history header / catalog scope",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-incident", statedBy: "owner", statedOn: "2026-07-14" },
        statement:
          "That says coding history all time, but it only has five chats, so that's definitely not all time.",
        authorityBoundary:
          "The sidebar header's scope claim must match the projection's real semantics: 'Recent chats · scanning…' before hydration settles and 'Recent chats · N' after hydration, where N is the exact number of rows rendered and never exceeds ten. The header never claims all-time disclosure; the full loss-accounted catalog remains reachable through search. Catalog title scans, page reads, and search-index content reads remain byte-bounded/streaming so an oversized session degrades to a fallback title instead of taking down catalog or search.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/codex-history.ts",
          "apps/openagents-desktop/src/merged-history.ts",
          "apps/openagents-desktop/tests/history-catalog-scale.test.ts",
          "docs/teardowns/2026-07-10-openagents-product-adaptation-analysis.md",
          "github:OpenAgentsInc/openagents#8789",
          "github:OpenAgentsInc/openagents#8674",
        ],
        oracles: [
          {
            id: "history_header.counted_disclosure_truth",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "On a catalog larger than ten: the header reads 'scanning…' pre-hydration and then the exact bounded recent count, never exposes a load-more row, never double-counts local threads that are also catalogued, and keeps the searchable total separate from the visible count.",
          },
          {
            id: "history_header.catalog_survives_oversized_rollouts",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/history-catalog-scale.test.ts",
            description:
              "A >page-size multi-workspace store catalogues every root (children excluded, never lost) with no silent truncation; a session whose authored title lies beyond the bounded head scan degrades to the fallback title while the rest of the catalog survives; the streaming page read keeps whole-conversation totals and loss accounting while returning only the requested window.",
          },
          {
            id: "history_header.built_electron_header",
            kind: "script",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "Built-Electron smoke asserts the sidebar header states the fixture catalog's true recent scope ('Recent chats · 1') — an untrue all-time claim fails the smoke.",
          },
        ],
        verification:
          "Desktop typecheck, shell + history-catalog-scale suites, the built-host smoke header assertion, and the real-store diagnosis receipt (1,289 roots from 1,582 sessions in ~1.9 s where the pre-fix build ENOMEMed).",
      },
      {
        contractId: "openagents_desktop.chat.provider_lane_capability_honesty.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer provider-lane capability truth",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "capability-audit", statedBy: "issue-8900", statedOn: "2026-07-16" },
        statement:
          "Every active provider lane renders a composer derived from its admitted capability projection; unsupported controls are absent or honestly disabled, and an over-claiming lane is quarantined before submission.",
        authorityBoundary:
          "ProviderLane.capabilities is the observed lane report. Electron main intersects it with the native static declaration or trusted ACP peer-profile/conformance allowlist and sends only the bounded projection through preload. The renderer may present that projection but cannot add models, modes, approvals, interactions, attachments, extensions, Full Auto, interrupt, queue, or steer authority. A model, feature, or extension outside the allowlist quarantines the whole lane with a public-safe reason and removes submission authority. Switching threads/providers re-derives the controls from the newly active lane; per-provider selections cannot leak Codex-only reasoning or Full Auto affordances into another lane.",
        evidenceRefs: [
          "apps/openagents-desktop/src/provider-lane.ts",
          "apps/openagents-desktop/src/provider-lane-capabilities.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/preload.cts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/react-composer.tsx",
          "github:OpenAgentsInc/openagents#8900",
        ],
        oracles: [
          {
            id: "provider_lane_capabilities.policy_intersection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/provider-lane-capabilities.test.ts",
            description:
              "Proves distinct Codex/Claude projections and fail-closed quarantine of an ACP fixture that advertises a feature and vendor extension absent from its trusted profile.",
          },
          {
            id: "provider_lane_capabilities.composer_switch",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
            description:
              "Switches the active composer fixture from Codex to Claude and proves models, reasoning, permission mode, Full Auto, attachments, and submission authority follow only the active admitted projection; a quarantined lane cannot send.",
          },
        ],
        verification:
          "Desktop focused ProviderLane/capability/composer suites, behavior-contract registry validation, and Desktop typecheck.",
      },
      {
        contractId: "openagents_desktop.chat.durable_automatic_titles.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "conversation sidebar titles",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-16" },
        statement:
          "Non-empty chats in the Desktop sidebar must have useful durable titles instead of remaining New chat or untitled; show Codex-native names when available and otherwise derive a safe title automatically.",
        authorityBoundary:
          "One bounded deterministic policy replaces only known placeholders with normalized authored message text. The local atomic store and the existing chat.appendMessage Sync transaction apply the same policy, so a failed or long-running model turn cannot strand the row and confirmed clients converge without a new mutation or schema. Existing manual/native names always win. App-server Thread.name remains authoritative for Codex history; only a missing name falls back to the bounded first-user preview, and transport envelopes are excluded. Live confirmed metadata updates replace the existing row in place without inventing recency, renderer title authority, cloud-task credentials, or prompt logging.",
        evidenceRefs: [
          "packages/khala-sync/src/chat.ts",
          "packages/khala-sync-client/src/chat.ts",
          "packages/khala-sync-server/src/chat-mutators.ts",
          "apps/openagents-desktop/src/thread-store.ts",
          "apps/openagents-desktop/src/codex-thread-lifecycle.ts",
          "apps/openagents-desktop/src/renderer/runtime-conversation.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8940",
        ],
        oracles: [
          {
            id: "automatic_titles.shared_atomic_policy",
            kind: "bun-test",
            mode: "unit",
            ref: "packages/khala-sync/src/chat.test.ts",
            description:
              "Proves placeholder recognition, whitespace normalization, the title bound, explicit-title precedence, and rejection of environment/plugin transport envelopes.",
          },
          {
            id: "automatic_titles.local_restart_persistence",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/thread-store.test.ts",
            description:
              "Persists a first authored title through the real upsert path and reopen while preserving a later owner rename.",
          },
          {
            id: "automatic_titles.codex_native_preview_precedence",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-thread-lifecycle.test.ts",
            description:
              "Projects an unnamed app-server thread with its bounded first-user preview while existing native names retain precedence.",
          },
          {
            id: "automatic_titles.live_sidebar_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Merges confirmed same-thread title metadata into the rail/header row in place during a live projection.",
          },
        ],
        verification:
          "Shared Sync policy/client/server tests, Desktop store/lifecycle/runtime/shell tests, behavior-contract registry validation, Desktop typecheck/build, and built Electron smoke.",
      },
      {
        contractId: "openagents_desktop.chat.local_title_rename.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local chat sidebar rename",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-16" },
        statement:
          "In OpenAgents Desktop, right-clicking a chat in the conversation sidebar should open a native-feeling shadcn context menu with a Rename action. Rename should support keyboard interaction, validate the title, persist through the existing thread store/host boundary, and update the visible chat row without requiring a restart.",
        authorityBoundary:
          "Only app-local Desktop threads are renameable. The accessible shadcn-styled context menu and focused dialog collect a bounded title, trim and reject empty input before dispatch, and send only the exact local thread ref plus title through a schema-decoded preload channel. Electron main re-validates the request and the private atomic thread store persists it; provider-owned history remains read-only. Renderer state updates the sidebar and active header only after host success. Cancel/Escape and every validation or host failure retain the previous durable and visible title, with an inline failure message for a rejected save.",
        evidenceRefs: [
          "apps/openagents-desktop/src/components/ui/context-menu.tsx",
          "apps/openagents-desktop/src/renderer/react-primitive-adapters.tsx",
          "packages/ui/src/workbench/rail.tsx",
          "apps/openagents-desktop/src/chat-contract.ts",
          "apps/openagents-desktop/src/preload.cts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/thread-store.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8929",
        ],
        oracles: [
          {
            id: "local_chat_rename.accessible_renderer_interaction",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx",
            description:
              "Opens the real local sidebar row menu by keyboard and right-click, activates Rename, proves the current title is focused and selected, refuses blank input inline, and dispatches only the trimmed exact thread/title payload.",
          },
          {
            id: "local_chat_rename.host_success_and_failure_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Runs the real typed intent handler, updates the visible thread title only after a successful host result, and retains the prior title plus explicit failure state when persistence rejects the rename.",
          },
          {
            id: "local_chat_rename.atomic_persistence",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/thread-store.test.ts",
            description:
              "Reopens the private JSON store after rename to prove durability, trimming, and unchanged persisted truth after an empty-title failure.",
          },
        ],
        verification:
          "Desktop renderer, shell, and thread-store oracle suites; repository behavior-contract validation; Desktop typecheck and build.",
      },
      {
        contractId: "openagents_desktop.chat.provider_lane_registry_and_switch_honesty.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "multi-lane thread selection and switching",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "issue-8903", statedOn: "2026-07-16" },
        statement:
          "Every configured provider lane remains visible with its authentication, admission, and capability truth; a thread's selected lane is durable, and switching either carries bounded host-owned history into a compatible lane or returns an exact typed refusal without changing selection.",
        authorityBoundary:
          "Electron main owns the durable lane registry, thread existence, history read, capability requirements, authentication evidence, peer admission, and selection write. The renderer can request only a bounded threadRef/laneRef pair and receives a public-safe outcome. Missing authentication, an unadmitted peer, an unknown lane, a missing thread, and a capability mismatch are distinct refusals; none may silently fall back to another provider or discard transcript history.",
        evidenceRefs: [
          "apps/openagents-desktop/src/provider-lane-registry.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/preload.cts",
          "apps/openagents-desktop/src/full-auto-control-server.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8903",
        ],
        oracles: [
          {
            id: "provider_lane_registry.durable_selection_and_typed_switch",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/provider-lane-registry.test.ts",
            description:
              "Reopens durable per-thread selection, proves unavailable and unadmitted lanes stay explicit, refuses incompatible switches without mutation, and carries a bounded host-read history window on a compatible switch.",
          },
          {
            id: "provider_lane_registry.control_route_parity",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/full-auto-control-server.test.ts",
            description:
              "Keeps GET /v1/lanes in the shared control route/OpenAPI parity table and behind the same loopback bearer boundary as every control operation.",
          },
        ],
        verification:
          "Provider lane registry, Full Auto control route parity, renderer composer/shell suites, Desktop typecheck, and repository behavior-contract validation.",
      },
      {
        contractId: "openagents_desktop.full_auto_dedicated_launcher.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Desktop Full Auto launch surface",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-17" },
        statement:
          "Full Auto is a named, durable autonomous run launched separately from ordinary chat, with an explicit objective/done condition, Play/Pause/Stop semantics, a read-only running view, run-level liveness/reporting, and provider-handoff evidence.",
        authorityBoundary:
          "This binds only the launch entry point (a lightning-bolt Full Auto action beside/under New session in the left rail). It does not itself define the run's lifecycle state machine (see full_auto_play_pause_stop_lifecycle.v1) or the read-only run view's contents (see full_auto_read_only_run_view.v1), and it grants no release or public-claim authority.",
        evidenceRefs: [
          "specs/desktop/full-auto.product-spec.md",
          "docs/fable/2026-07-17-full-auto-implementation-audit.md",
          "github:OpenAgentsInc/openagents#8968",
          "github:OpenAgentsInc/openagents#8974",
        ],
        oracles: [
          {
            id: "openagents_desktop.full_auto_dedicated_launcher.dom",
            kind: "bun-test",
            mode: "dom",
            ref: "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx",
            description:
              "FA-UX-01 (#8974) landed: real-DOM component oracles prove the left rail renders a dedicated Full Auto launcher action beside New session, that the launcher form collects title/objective/done-condition/workspace/provider-lane/turn-cap and Start stays disabled until every required field is present, and that Start routes through the same startFullAutoRunAction the opt-in HTTP control API uses.",
          },
          {
            id: "openagents_desktop.full_auto_dedicated_launcher.e2e_residual",
            kind: "planned",
            mode: "e2e",
            ref: "github:OpenAgentsInc/openagents#8974",
            description:
              "Real-Chromium/Electron e2e visual smoke across launch/running/paused/stalled/terminal states remains a residual for a follow-up issue.",
          },
        ],
        verification:
          "FA-UX-01 (#8974) landed: pnpm --dir apps/openagents-desktop run test runs react-full-auto-surface.test.tsx and full-auto-workspace.test.ts in the normal Desktop sweep, proving Start is disabled until required fields are present and refuses on a workspace mismatch/active-run-conflict exactly like the existing control-API start.",
      },
      {
        contractId: "openagents_desktop.full_auto_read_only_run_view.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Desktop Full Auto run view",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-17" },
        statement:
          "Full Auto is a named, durable autonomous run launched separately from ordinary chat, with an explicit objective/done condition, Play/Pause/Stop semantics, a read-only running view, run-level liveness/reporting, and provider-handoff evidence.",
        authorityBoundary:
          "This binds only the visible run view while a Full Auto run is active: pinned objective/workspace, explicit lifecycle state, an inspectable per-turn transcript, and the absence of the ordinary chat composer. It does not grant live token-streaming, steering, or any release/public-claim authority, and it does not itself define Play/Pause/Stop transition legality (see full_auto_play_pause_stop_lifecycle.v1).",
        evidenceRefs: [
          "specs/desktop/full-auto.product-spec.md",
          "docs/fable/2026-07-17-full-auto-implementation-audit.md",
          "github:OpenAgentsInc/openagents#8968",
          "github:OpenAgentsInc/openagents#8974",
        ],
        oracles: [
          {
            id: "openagents_desktop.full_auto_read_only_run_view.dom",
            kind: "bun-test",
            mode: "dom",
            ref: "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx",
            description:
              "FA-UX-01 (#8974) landed: real-DOM component oracles prove that once a run Starts, the main canvas renders a dedicated read-only run view (pinned objective/workspace/provider/cap, explicit lifecycle state across all ten named states, and an inspectable per-turn transcript) and that the ordinary chat composer and its retired Full Auto toggle are absent while the run is active.",
          },
          {
            id: "openagents_desktop.full_auto_read_only_run_view.e2e_residual",
            kind: "planned",
            mode: "e2e",
            ref: "github:OpenAgentsInc/openagents#8974",
            description:
              "Real-Chromium/Electron e2e visual smoke across launch/running/paused/stalled/terminal states remains a residual for a follow-up issue.",
          },
        ],
        verification:
          "FA-UX-01 (#8974) landed: the read-only run view renders explicit lifecycle state across Draft/Running/Pausing/Paused/Retrying/Stalled/Completed/Failed/Stopped/Cap-reached, and the ordinary composer/toggle/badge are retired from the chat surface, proven in the normal Desktop test sweep.",
      },
      {
        contractId: "openagents_desktop.full_auto_play_pause_stop_lifecycle.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Desktop Full Auto run lifecycle",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-17" },
        statement:
          "Full Auto is a named, durable autonomous run launched separately from ordinary chat, with an explicit objective/done condition, Play/Pause/Stop semantics, a read-only running view, run-level liveness/reporting, and provider-handoff evidence.",
        authorityBoundary:
          "This binds the lifecycle state machine and its Play/Pause/Stop (Resume/Pause/Stop) transition legality and attribution. It grants no autonomous provider-selection, mid-run steering, or concurrent multi-run authority, and it does not itself verify that a run's stated done condition was actually satisfied -- Completed remains a self-reported, owner-reviewable disposition.",
        evidenceRefs: [
          "specs/desktop/full-auto.product-spec.md",
          "docs/fable/2026-07-17-full-auto-implementation-audit.md",
          "github:OpenAgentsInc/openagents#8968",
          "github:OpenAgentsInc/openagents#8969",
          "github:OpenAgentsInc/openagents#8974",
        ],
        oracles: [
          {
            id: "openagents_desktop.full_auto_play_pause_stop_lifecycle.run_model",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/full-auto-run-registry.test.ts",
            description:
              "FA-RUN-01 (#8969) landed: unit coverage over the full Draft/Running/Pausing/Paused/Retrying/Stalled/Completed/Failed/Stopped/Cap-reached lifecycle state machine and its exhaustive legal-transition matrix.",
          },
          {
            id: "openagents_desktop.full_auto_play_pause_stop_lifecycle.ui_wiring",
            kind: "bun-test",
            mode: "dom",
            ref: "apps/openagents-desktop/src/renderer/full-auto-workspace.test.ts",
            description:
              "FA-UX-01 (#8974) landed: the read-only run view's Pause/Resume/Stop/Retry-now controls are wired to full-auto-run-actions.ts -- the same shared action functions the opt-in HTTP control server uses -- via a dedicated renderer IPC bridge, with per-state control visibility proven through the real Effect intent registry.",
          },
        ],
        verification:
          "FA-RUN-01 (#8969) landed the durable FullAutoRun lifecycle state machine and its control-API Pause/Resume/Stop routes. FA-UX-01 (#8974) landed wiring those exact typed transitions into the read-only run view's Pause/Resume/Stop/Retry-now controls through a dedicated renderer IPC bridge sharing the same action functions as the control API.",
      },
      {
        contractId: "openagents_desktop.full_auto_run_view_canonical_timeline.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Desktop Full Auto run view",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-screenshot-review", statedBy: "owner", statedOn: "2026-07-17" },
        statement:
          "this looks like shit. why the fuck doesnt this use normal thread component view",
        authorityBoundary:
          "The Full Auto run view renders the bound thread's conversation with the SAME canonical ConversationTimeline component every ordinary chat uses, hydrated through the shell's canonical local-session selection path -- never a parallel mini-renderer -- and stays read-only (no composer, per the read-only run view contract). Run chrome is a proper header: a styled state badge, objective and done-condition, workspace/provider/cap metadata rows, and real button components for Pause/Resume/Retry/Stop/Refresh. Turn history rows are formatted (provider chip, disposition summary, relative time plus duration), never raw ISO concatenation. This binds presentation composition only; run lifecycle legality and dispatch authority remain with the existing lifecycle and reconcile contracts.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/react-full-auto-surface.tsx",
          "apps/openagents-desktop/src/renderer/full-auto-workspace.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/react-timeline.tsx",
          "github:OpenAgentsInc/openagents#8997",
        ],
        oracles: [
          {
            id: "openagents_desktop.full_auto_run_view_canonical_timeline.dom",
            kind: "bun-test",
            mode: "dom",
            ref: "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx",
            description:
              "Real-DOM oracles prove the run view composes the canonical ConversationTimeline (the message-scroller element ordinary chats render) for the bound thread's notes, renders the styled state badge and real Pause/Resume/Retry/Stop/Refresh buttons, and formats turn rows as provider chip + disposition + relative time/duration with no raw ISO concatenation.",
          },
          {
            id: "openagents_desktop.full_auto_run_view_canonical_timeline.selection_wiring",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/full-auto-workspace.test.ts",
            description:
              "Opening a Full Auto run selects its bound thread through the injected canonical thread-selection path (the shell wires commitLocalSession) before re-asserting the full-auto workspace, so state.notes carries the run's real conversation for the canonical timeline.",
          },
        ],
        verification:
          "Desktop renderer full-auto workspace/surface suites, behavior-contract validation, and Desktop typecheck in the normal test sweep.",
      },
      {
        contractId: "openagents_desktop.workspace.files_primary_sidebar_mode_toggle.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Desktop current-worktree Files mode",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-18" },
        statement:
          "In OpenAgents Desktop, Command-E replaces the existing left sidebar with the current working directory file tree and reuses the existing top bar; it adds no new page chrome.",
        authorityBoundary:
          "The canonical workspace.files command owns Meta+E on macOS and Control+E elsewhere. It enters a bounded Files workspace mode for the currently selected coding session and its already-admitted WorkContext: the existing primary rail replaces sessions/projects with the Pierre tree, the existing conversation header carries Files search/refresh/exit controls, and the existing main region carries the editor. Files is excluded from the renderer-local right-panel catalog, so no right sidebar, parallel tab strip, resize rail, or additional application shell is mounted. Repeating the command exits Files and restores the ordinary session rail and chat workspace. The shortcut, palette row, header control, and workspace state converge on typed Desktop intents. The owned Pierre adapter projects only canonical relative refs already admitted by Desktop state; Pierre receives no bridge, filesystem, absolute root, grant, Git/process authority, rename/drag-and-drop authority, or ambient cwd. This adds no Monaco dependency or new workspace grant.",
        evidenceRefs: [
          "docs/ide/2026-07-18-openagents-desktop-basic-ide-vscode-pierre-plan.md",
          "github:OpenAgentsInc/openagents#9006",
          "github:OpenAgentsInc/openagents#9007",
          "github:OpenAgentsInc/openagents#9008",
          "github:OpenAgentsInc/openagents#9009",
        ],
        oracles: [
          {
            id: "openagents_desktop.workspace.files_primary_sidebar_mode_toggle.registry",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves workspace.files remains the canonical palette command and carries Meta+E/Control+E defaults.",
          },
          {
            id: "openagents_desktop.workspace.files_primary_sidebar_mode_toggle.transition",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the canonical command enters and exits the Effect-owned Files workspace mode without a renderer-local Files-panel request.",
          },
          {
            id: "openagents_desktop.workspace.files_primary_sidebar_mode_toggle.no_right_panel",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/surface-layout.test.ts",
            description:
              "Proves Files is not an admitted right-side surface and legacy persisted Files-panel state is decoded away while Review, Terminal, and Preview remain bounded panel surfaces.",
          },
          {
            id: "openagents_desktop.workspace.files_primary_sidebar_mode_toggle.dom",
            kind: "bun-test",
            mode: "dom",
            ref: "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx",
            description:
              "Proves Files replaces the existing Sessions rail, reuses the existing conversation header for Files controls, mounts no right panel or Files tab strip, renders admitted relative paths through the real Pierre shadow tree, expands/collapses a directory, opens its child through typed Desktop intents, and restores the ordinary shell on exit.",
          },
          {
            id: "openagents_desktop.workspace.files_primary_sidebar_mode_toggle.pierre_boundary",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/pierre-tree-package.test.ts",
            description:
              "Pins the audited Pierre Trees beta, confines package imports to the owned adapter, rejects the private path store and unsafe CSS, and proves the installed Apache license/NOTICE remain in the package closure.",
          },
          {
            id: "openagents_desktop.workspace.files_primary_sidebar_mode_toggle.literal_paths",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/workspace-service.test.ts",
            description:
              "Proves Git ignore classification passes arbitrary admitted relative filenames through NUL-delimited stdin as literal paths, so a valid leading-colon name cannot erase the visible workspace tree.",
          },
          {
            id: "openagents_desktop.workspace.files_primary_sidebar_mode_toggle.effective_binding",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/command-shortcuts.test.ts",
            description:
              "Proves platform defaults, user overrides, conflicts, editable targets, prevented events, and key repeat are handled without a second shortcut authority.",
          },
          {
            id: "openagents_desktop.workspace.files_primary_sidebar_mode_toggle.electron",
            kind: "visual-smoke",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built Electron smoke synthesizes the effective Command-E/Control-E chord, proves the existing primary rail and top bar enter Files mode with no right panel, renders non-empty relative root entries, expands a directory, opens a text document, withholds the absolute root, and repeats the chord to restore Sessions and Chat.",
          },
        ],
        verification:
          "Issues #9006 through #9009 land the registry, guarded effective-binding matcher, Effect workspace state, primary-rail/top-bar takeover, right-panel exclusion, literal-path ignore classification, audited Pierre projection, current-worktree boundary proof, and built Electron enter/expand/open/exit-restoration smoke in the normal Desktop verification gate.",
      },
      {
        contractId: "openagents_desktop.macos.code_document_open_with.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Desktop macOS system integration",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-18" },
        statement:
          "Register OpenAgents with macOS so Finder recommends it in Open With for Markdown and common code files, and make the packaged result locally testable.",
        authorityBoundary:
          "Only the packaged macOS bundle advertises document support through CFBundleDocumentTypes. It claims the Editor role at Alternate rank for a bounded source/text UTI and extension set, so it is recommended without silently replacing the user's default application. Electron main subscribes to open-file before ready. The explicit OS-selected regular file may admit only its containing directory as the WorkContext; main reduces the selection to one validated relative filename before the deferred typed command crosses into the renderer. The renderer reuses the existing Files mode, workspace browser grant, and document-open intent. Unsupported, relative, directory, revoked, secret-shaped, binary, oversized, or invalid selections remain rejected by main or the existing workspace service.",
        evidenceRefs: [
          "apps/openagents-desktop/src/macos-document-open.ts",
          "apps/openagents-desktop/forge.config.ts",
          "github:OpenAgentsInc/openagents#9010",
        ],
        oracles: [
          {
            id: "openagents_desktop.macos.code_document_open_with.bundle",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/package-macos.test.ts",
            description:
              "Proves Forge emits Editor/Alternate document declarations, supports Markdown/JavaScript/JSX/TypeScript/TSX, and installs the open-file listener before Electron ready.",
          },
          {
            id: "openagents_desktop.macos.code_document_open_with.reduction",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/macos-document-open.test.ts",
            description:
              "Proves an explicit absolute selection reduces to its containing directory and one validated relative path while unsupported, relative, and non-file inputs fail closed.",
          },
          {
            id: "openagents_desktop.macos.code_document_open_with.editor_transition",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the typed system-document intent enters Files, resolves the current workspace grant, and opens the relative document through the existing editor bridge.",
          },
        ],
        verification:
          "Issue #9010 owns Forge configuration, pre-ready Electron delivery, relative-path reduction, the existing Files/editor transition, and inspection of a real packaged macOS Info.plist before Launch Services registration.",
      },
      {
        contractId: "openagents_desktop.macos.document_open_editor_first_startup.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Desktop macOS document-open startup performance",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-18" },
        statement:
          "So it does work, except it stays on the original loading screen for about six seconds and then spends like one or two seconds showing the usual chat view before it pops over to the editor. So this is like way too slow. Every part of that needs to be dramatically sped up. Fix it.",
        authorityBoundary:
          "A validated pre-ready macOS open-file selection contributes only its already-reduced relative filename to the sandboxed renderer launch context. That bounded hint selects Files and an honest loading tree for the first shell paint; it does not carry the absolute path, grant a workspace, or open a document. Main remains the sole workspace authority and delivers the existing typed system-document command after admitting the selected file's containing directory. The renderer drains that command before history hydration, opens the tree and requested document before refreshing secondary coding-catalog metadata, and defers chat-host, provider-capability, Fable-availability, and voice metadata probes until after the editor-first shell mounts. Ordinary chat startup and unsupported selections keep their existing behavior.",
        evidenceRefs: [
          "apps/openagents-desktop/src/desktop-launch-context.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#9011",
        ],
        oracles: [
          {
            id: "macos.document_open_startup.relative_launch_context",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/desktop-launch-context.test.ts",
            description:
              "Proves the renderer launch argument round-trips one relative filename and rejects absolute, nested, and malformed inputs.",
          },
          {
            id: "macos.document_open_startup.editor_first_ordering",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/startup-contract.test.ts",
            description:
              "Proves a document launch initializes Files/loading, drains commands before history hydration, and bypasses chat/provider probes before mount.",
          },
          {
            id: "macos.document_open_startup.packaged_timing",
            kind: "visual-smoke",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The packaged startup trace waits for the real documentEditorReady mark and records process-start-to-editor timing without waiting for chat-history hydration.",
          },
        ],
        verification:
          "Issue #9011 owns the exact-revision macOS package, cold Finder-open startup trace, no-chat-flash oracle, Desktop verification gate, and local RC replacement for owner testing.",
      },
    ],
  };
