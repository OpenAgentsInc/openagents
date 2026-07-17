import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts";
export const openAgentsMobileUxContractRegistry: BehaviorContractRegistryDocument =
  {
    schemaVersion: BehaviorContractSchemaVersion,
    version: "2026-07-17.14",
    contracts: [
      {
        contractId: "openagents_mobile.workspace_native_input.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile native workspace input",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "accepted-owner-plan", statedBy: "owner", statedOn: "2026-07-17" },
        statement:
          "Native workspace rows use a thresholded horizontal PanResponder driver for declared full-swipe actions while retaining press actions, and the app host maps a closed hardware-key set to new-task, navigation, detail, and dismiss commands.",
        authorityBoundary:
          "Pan gestures dispatch only an exact declared action on the matching side after horizontal-axis and distance checks, then settle. Keyboard input requires Command/Control except Escape; unknown keys do nothing. Layout/focus commands never change transcript authority, and physical-device focus traversal remains a release receipt rather than a simulated claim.",
        evidenceRefs: [
          "apps/openagents.com/packages/effect-native-render-rn/src/index.ts",
          "apps/openagents-mobile/src/screens/mobile-workspace-keyboard.ts",
          "apps/openagents-mobile/src/screens/home-screen.tsx",
          "apps/openagents-mobile/src/screens/home-core.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-c22b",
        ],
        oracles: [{
          id: "mobile_native_gesture_and_keyboard",
          kind: "bun-test",
          mode: "e2e",
          ref: "apps/openagents-mobile/tests/mobile-workspace-keyboard.test.ts",
          description: "Proves the closed keyboard map and authority-preserving layout dispatch; the paired RN renderer suite proves exact swipe threshold/axis/side resolution.",
        }],
        verification:
          "Keyboard/workspace, RN renderer, authoritative Home, accessibility, behavior-contract, mobile/RN typechecks, and repository checks; physical keyboard and screen-reader receipts remain T3M-F2.",
      },
      {
        contractId: "openagents_mobile.workspace_row_actions.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile workspace navigation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "accepted-owner-plan",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "Workspace rows expose valid archive/restore/delete actions through the typed swipe-item grammar and an accessible press fallback; compact context and lifecycle controls render in one native-lowered bottom sheet.",
        authorityBoundary:
          "Only reversible archive or restore may be the full-swipe default. Delete always opens the existing explicit confirmation and remains server-authoritative. Action IDs are validated against the exact current active/archived thread sets; foreign, stale, invalid, and in-flight actions are refused, and sheet dismissal cannot interrupt writeback.",
        evidenceRefs: [
          "apps/openagents-mobile/src/screens/home-core.ts",
          "apps/openagents.com/packages/effect-native-core/src/index.ts",
          "apps/openagents.com/packages/effect-native-render-rn/src/index.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-c22a",
        ],
        oracles: [{
          id: "mobile_workspace_sheet_and_row_actions",
          kind: "bun-test",
          mode: "e2e",
          ref: "apps/openagents-mobile/tests/mobile-workspace-actions.test.ts",
          description:
            "Proves sheet presentation/dismissal, reversible full-swipe policy, accessible action fallback, stale-action refusal, explicit delete confirmation, and confirmed lifecycle writeback.",
        }],
        verification:
          "Workspace action, authoritative Home, accessibility, Sheet/Swipeable RN renderer, behavior-contract, mobile typecheck, and repository checks; genuine gesture driver, keyboard shortcuts, host focus, and physical evidence remain T3M-C2.2b/T3M-F2.",
      },
      {
        contractId: "openagents_mobile.adaptive_workspace.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile adaptive workspace shell",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "accepted-owner-plan",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "Phone width presents workspace navigation and detail as exclusive routes; tablet width keeps one bounded navigation pane and the sole transcript/detail authority mounted together through the typed Effect Native split-pane contract.",
        authorityBoundary:
          "Viewport width selects only layout, never conversation authority. Sidebar resize/collapse accepts typed bounded values; the detail transcript is mounted exactly once, route-aware navigation copy reflects the current layout, and each transition projects a serializable navigation-or-transcript focus-return identity.",
        evidenceRefs: [
          "apps/openagents-mobile/src/screens/mobile-adaptive-workspace.ts",
          "apps/openagents-mobile/src/screens/home-screen.tsx",
          "apps/openagents-mobile/src/screens/home-core.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-c21",
          "reference:t3code@8b5469863ae1dd696e696de30240ec3da607962d:apps/mobile/src/components/AdaptiveWorkspaceLayout.tsx",
        ],
        oracles: [{
          id: "mobile_adaptive_workspace_matrix",
          kind: "bun-test",
          mode: "e2e",
          ref: "apps/openagents-mobile/tests/mobile-adaptive-workspace.test.ts",
          description:
            "Proves compact/regular thresholds, bounded resizing, phone exclusivity, single tablet detail authority, route-aware chrome, and focus-return identity.",
        }],
        verification:
          "Adaptive workspace, authoritative Home, accessibility, native SplitPane renderer, behavior-contract, mobile/package typechecks, and repository checks; native sheets/swipes/shortcuts and physical-device focus evidence remain T3M-C2.2/T3M-F2.",
      },
      {
        contractId: "openagents_mobile.workspace_navigation.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile workspace navigation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "accepted-owner-plan",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "The mobile workspace drawer uses one compact project-aware row grammar for confirmed conversations, coding sessions, pending attention, and archived threads, with bounded search, status/project filters, row-local lifecycle actions, and exact causal jumps.",
        authorityBoundary:
          "Navigation projects only already-confirmed conversation, coding-directory, and personal-attention state. Exact refs remain in typed intents but outside primary labels; invalid attention is withheld, foreign project filters are refused, cached-withheld coding rows remain counted but hidden, and lifecycle writeback remains server-authoritative.",
        evidenceRefs: [
          "apps/openagents-mobile/src/screens/mobile-workspace-navigation.ts",
          "apps/openagents-mobile/src/screens/home-core.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-c1",
          "reference:t3code@8b5469863ae1dd696e696de30240ec3da607962d:apps/mobile/src/features/sidebar",
        ],
        oracles: [
          {
            id: "mobile_workspace_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/mobile-workspace-navigation.test.ts",
            description:
              "Proves exact project/session joins, bounded labels and filters, archive isolation, invalid-attention withholding, and causal target identity.",
          },
          {
            id: "mobile_workspace_actions_and_causal_navigation",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/authoritative-home.test.ts",
            description:
              "Proves typed row selection, attention jumps, row-local lifecycle controls, explicit deletion, and confirmed writeback.",
          },
        ],
        verification:
          "Workspace projection, authoritative Home, lifecycle, attention, accessibility, behavior-contract, mobile typecheck, and repository checks; adaptive native shell and physical-device evidence remain T3M-C2/T3M-F2.",
      },
      {
        contractId: "openagents_mobile.composer.provider_neutral_queue.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile coding composer",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "accepted-owner-plan",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "Text submitted during a confirmed running or waiting turn queues a durable follow-up after that turn. The draft clears only after confirmed admission, and a compact receipt keeps admission distinct from pending delivery and promotion.",
        authorityBoundary:
          "Mobile mints an exact provider-neutral turn.queue control bound to the confirmed thread, run generation, durable message ref, ordering key, origin, idempotency key, and deadline. The adapter lowers that semantic only through Pylon's proven queue-until-idle message.append path; it never relabels queue as steer or starts a concurrent turn. Stop remains bound to the parent run. Cross-restart delivery observation remains T3M-F1.",
        evidenceRefs: [
          "apps/openagents-mobile/src/conversation/mobile-runtime-queue.ts",
          "apps/openagents-mobile/src/conversation/mobile-conversation.ts",
          "apps/openagents-mobile/src/screens/mobile-composer-run-control.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-b23b",
          "apps/pylon/src/orchestration/runtime-intent-enforcement.ts",
        ],
        oracles: [
          {
            id: "mobile_provider_neutral_queue_control",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/mobile-runtime-queue.test.ts",
            description:
              "Proves exact queue identity, replay classification, and separate admission, delivery, and terminal axes.",
          },
          {
            id: "mobile_queue_adapter_and_receipt",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/authoritative-home.test.ts",
            description:
              "Proves queue-only active-run lowering, no concurrent start, confirmed draft clearing, and honest pending-delivery receipt beside Stop.",
          },
        ],
        verification:
          "Queue schema, conversation adapter, Home receipt, run-control, accessibility, behavior-contract, mobile typecheck, and repository checks; cross-restart promotion and physical-device evidence remain T3M-F1/T3M-F2.",
      },
      {
        contractId: "openagents_mobile.composer.active_run_admission_and_stop.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile coding composer",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "accepted-owner-plan",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "The composer names queued, running, waiting, confirmation, and stop-pending state beside the transcript. Running or waiting text queues a durable follow-up after the exact current run; an empty composer action becomes Stop and requires explicit confirmation.",
        authorityBoundary:
          "Stop request, confirmation, and dispatch are bound to the exact current thread/run and confirmed runtime-control availability; stale or foreign confirmation cannot dispatch. Active text mints provider-neutral turn.queue identity and lowers through Pylon's queue-until-idle message.append adapter; it cannot fall through to a concurrent start or be mislabeled as mid-stream steering.",
        evidenceRefs: [
          "apps/openagents-mobile/src/screens/mobile-composer-run-control.ts",
          "apps/openagents-mobile/src/screens/khala-core.ts",
          "apps/openagents-mobile/src/conversation/mobile-conversation.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-b23a",
          "reference:t3code@8b5469863ae1dd696e696de30240ec3da607962d:apps/mobile/src/features/threads/ThreadComposer.tsx",
        ],
        oracles: [
          {
            id: "mobile_composer_active_run_admission",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/authoritative-home.test.ts",
            description:
              "Proves composer-local active status, exact Stop confirmation/refusal, pending preservation, and confirmed terminal replacement.",
          },
          {
            id: "mobile_waiting_run_exact_queue",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/mobile-conversation.test.ts",
            description:
              "Proves waiting-for-input follow-up queues after the exact current run and never dispatches startTurn.",
          },
          {
            id: "effect_native_mobile_composer_stop_action",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents.com/packages/effect-native-render-rn/src/index.test.ts",
            description:
              "Proves empty Stop, non-empty exact submit, accessible consequences, and duplicate Stop suppression in the native renderer.",
          },
        ],
        verification:
          "Run-control, authoritative Home, mobile conversation, native renderer, accessibility, behavior-contract, package/mobile typechecks, and repository checks; queue admission/delivery semantics are covered by the paired queue contract and physical-device evidence remains T3M-F2.",
      },
      {
        contractId: "openagents_mobile.composer.repository_path_context.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile coding composer",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "accepted-owner-plan",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "An explicit trailing @ token opens bounded repository-path autocomplete with loading, empty, unavailable, and failed states; selecting a current result inserts only that safe relative path mention into the persisted draft.",
        authorityBoundary:
          "Queries are scoped to the exact repository and worktree refs already bound to the composer. Results must echo that scope and query, carry safe relative path and revision identity, and survive duplicate, traversal, and stale-completion checks. The current mobile app names a missing environment search transport instead of manufacturing repository contents; connecting that real provider remains a T3M-D1/T3M-F1 release dependency.",
        evidenceRefs: [
          "apps/openagents-mobile/src/coding/mobile-composer-path-context.ts",
          "apps/openagents-mobile/src/screens/mobile-composer-discovery.ts",
          "apps/openagents-mobile/src/screens/home-core.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-b22b",
          "reference:t3code@8b5469863ae1dd696e696de30240ec3da607962d:apps/mobile/src/features/threads/ThreadComposer.tsx",
        ],
        oracles: [{
          id: "mobile_composer_repository_path_context",
          kind: "bun-test",
          mode: "e2e",
          ref: "apps/openagents-mobile/tests/mobile-composer-path-context.test.ts",
          description:
            "Proves exact repository/worktree search scope, bounded safe result decoding, stale-race suppression, current-result-only insertion, foreign selection refusal, and honest missing-transport presentation.",
        }],
        verification:
          "Path context, composer discovery, authoritative Home, accessibility, behavior-contract, mobile typecheck, and repository checks; a paired live environment transport and physical-device evidence remain T3M-D1/T3M-F1/T3M-F2.",
      },
      {
        contractId: "openagents_mobile.composer.typed_slash_commands.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile coding composer",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "accepted-owner-plan",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "An explicit trailing slash token opens bounded composer-local command autocomplete with typed labels, descriptions, availability reasons, filtering, empty state, and native keyboard/touch selection.",
        authorityBoundary:
          "The parser activates only after an explicit slash token and selection accepts only a closed schema-decoded command id. Each id reuses an existing Home authority path for new chat, target picker, attachment picker, or exact active-turn cancel; arbitrary text never becomes tool or runtime routing.",
        evidenceRefs: [
          "apps/openagents-mobile/src/screens/mobile-composer-discovery.ts",
          "apps/openagents-mobile/src/screens/home-core.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-b22a",
          "reference:t3code@8b5469863ae1dd696e696de30240ec3da607962d:apps/mobile/src/features/threads/ThreadComposer.tsx",
        ],
        oracles: [{
          id: "mobile_composer_typed_slash_discovery",
          kind: "bun-test",
          mode: "e2e",
          ref: "apps/openagents-mobile/tests/mobile-composer-discovery.test.ts",
          description:
            "Proves explicit-trigger parsing, bounded filtering, honest disabled/empty states, draft-preserving query replacement, exact available dispatch, and unavailable refusal.",
        }],
        verification:
          "Discovery, authoritative Home, composer toolbar/attachments, accessibility, RN Composer/Combobox, behavior-contract, mobile typecheck, and repository checks; physical keyboard evidence remains T3M-F2.",
      },
      {
        contractId: "openagents_mobile.composer.attachment_editing.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile coding composer",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "accepted-owner-plan",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "Device-local draft attachments render as compact image previews or file cards with exact state, size, and type; the user can remove an exact item and retry a failed item without losing the text, target, other attachments, or transcript.",
        authorityBoundary:
          "Removal is a canonical persisted composer transaction scoped to the exact active draft. Retry is admitted only for an existing failed attachment and can return to ready only after the host re-reads the managed device-local bytes and proves the stored size and SHA-256 digest. This does not add binary runtime delivery or remote upload authority.",
        evidenceRefs: [
          "apps/openagents-mobile/src/screens/mobile-composer-attachments.ts",
          "apps/openagents-mobile/src/coding/mobile-coding-composer.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-b21",
          "reference:t3code@8b5469863ae1dd696e696de30240ec3da607962d:apps/mobile/src/features/threads/ThreadComposer.tsx",
        ],
        oracles: [
          {
            id: "mobile_composer_attachment_projection_and_intents",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/mobile-composer-attachments.test.ts",
            description:
              "Proves image/file presentation, exact states and errors, Dynamic Type actions, foreign/stale refusal, active-draft remove/retry, transcript preservation, and honest feedback.",
          },
          {
            id: "mobile_composer_attachment_canonical_transactions",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/mobile-coding-composer.test.ts",
            description:
              "Proves canonical removal preserves text and other items while verified retry rejects mismatched proof and clears failure only for matching device-local bytes.",
          },
        ],
        verification:
          "Composer attachment, canonical draft, picker/delivery, authoritative Home, accessibility, registry, mobile typecheck, and repository checks; physical image preview and screen-reader evidence remain T3M-F2.",
      },
      {
        contractId: "openagents_mobile.composer.authoritative_target_toolbar.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile coding composer",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "accepted-owner-plan",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "The coding composer presents repository/worktree, current target/model, and Code mode as one compact local toolbar; its searchable native picker groups authenticated targets by provider and explains selected, ready, unavailable, revoked, offline, empty-search, and missing-catalog states.",
        authorityBoundary:
          "Target/model remain one authoritative catalog selection persisted through the existing composer draft mutation; the UI cannot construct a target, split a model from its account/lane, or select a non-ready row. Code is the only visible mode until T3M-B2 supplies typed command admission; this packet does not pretend shell mode works.",
        evidenceRefs: [
          "apps/openagents-mobile/src/screens/mobile-composer-toolbar.ts",
          "apps/openagents-mobile/src/coding/mobile-execution-targets.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-b1",
          "reference:t3code@8b5469863ae1dd696e696de30240ec3da607962d:apps/mobile/src/features/threads/ThreadComposer.tsx",
        ],
        oracles: [
          {
            id: "mobile_composer_target_toolbar",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/mobile-composer-toolbar.test.ts",
            description:
              "Proves provider grouping/search, compact current target/model/mode, complete readiness copy, Dynamic Type targets, empty state, exact ready selection, persistence, dismissal, and foreign/non-ready refusal.",
          },
          {
            id: "mobile_composer_target_authoritative_home",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/authoritative-home.test.ts",
            description:
              "Proves the selected persisted target remains the exact runtime command target used on submission and catalog loss preserves the draft while withholding send authority.",
          },
        ],
        verification:
          "Composer-toolbar, authoritative Home, accessibility, registry, mobile typecheck, and repository checks; physical keyboard/sheet focus restoration remains T3M-F2.",
      },
      {
        contractId: "openagents_mobile.transcript.media_and_history.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile transcript media and history",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "accepted-owner-plan",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "The mobile transcript renders confirmed images with explicit loading/failure/retry and a dismissable contain-fit viewer, keeps stable active rows keyed in place, suspends auto-pin when the user scrolls away, marks unread updates inline, offers jump-to-latest, and paginates retained history without moving the visible keyed anchor.",
        authorityBoundary:
          "Image, scroll, disclosure, viewer, retry, and retained-page state is device-local presentation. Attachment callbacks are accepted only for an exact attachment in the current confirmed transcript; the image-only wire schema is not widened. Pagination reveals only already-retained confirmed entries and names server/device omissions instead of claiming they were loaded.",
        evidenceRefs: [
          "apps/openagents-mobile/src/screens/mobile-transcript-attachment.ts",
          "apps/openagents-mobile/src/screens/mobile-transcript-history.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-a4",
          "reference:t3code@8b5469863ae1dd696e696de30240ec3da607962d:apps/mobile/src/features/threads/ThreadFeed.tsx",
        ],
        oracles: [
          {
            id: "mobile_transcript_attachment_viewer",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/mobile-transcript-attachment.test.ts",
            description:
              "Proves exact attachment lifecycle callbacks, honest failure/retry, foreign-callback refusal, ready-only viewer opening, contain-fit media, and dismissal.",
          },
          {
            id: "mobile_transcript_history_scroll_state",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/mobile-transcript-history.test.ts",
            description:
              "Proves 60-row retained pages, omission accounting, stable in-place replacement, pin suspension, unread boundary, jump recovery, virtualization, and anchor retention intent.",
          },
          {
            id: "effect_native_transcript_media_renderer",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents.com/packages/effect-native-render-rn/src/index.test.ts",
            description:
              "Proves native Image lifecycle/press dispatch plus FlatList keyed target scrolling, visible-position retention, real pin transitions, and end restoration.",
          },
        ],
        verification:
          "Attachment, history/scroll, authoritative Home, accessibility, RN renderer, behavior-contract, package/mobile typecheck, and repository checks; physical zoom gestures and screen-reader evidence remain T3M-F2.",
      },
      {
        contractId: "openagents_mobile.transcript.runtime_interaction_cards.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile transcript interactions",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "accepted-owner-plan",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "Provider questions, tool approvals, and plan reviews appear inline as distinct compact transcript cards with clear pending, submitting, resolved, expired, and revoked states; exact request-scoped actions; accessible touch targets; selected-answer and validation feedback; and bounded rich plan rendering.",
        authorityBoundary:
          "The cards present confirmed exact-thread interaction authority and dispatch the existing typed decision intents only. Tool permission is request-scoped allow-once or deny; no session grant is invented. Questions support only the choices provided by authority, and free text is absent unless a future authority schema explicitly permits it. Terminal cards are read-only.",
        evidenceRefs: [
          "apps/openagents-mobile/src/screens/mobile-interaction-card.ts",
          "apps/openagents-mobile/src/screens/khala-core.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-a3",
          "reference:t3code@8b5469863ae1dd696e696de30240ec3da607962d:apps/mobile/src/features/threads/PendingApprovalCard.tsx",
          "reference:t3code@8b5469863ae1dd696e696de30240ec3da607962d:apps/mobile/src/features/threads/PendingUserInputCard.tsx",
        ],
        oracles: [
          {
            id: "mobile_runtime_interaction_card_matrix",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/mobile-interaction-card.test.ts",
            description:
              "Proves request-scoped approval, question modes/descriptions/selections/validation/submission, plan Markdown and exact outcomes, 44pt targets, and read-only terminal states.",
          },
          {
            id: "mobile_runtime_interaction_card_authority",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/authoritative-home.test.ts",
            description:
              "Proves inline cards retain confirmed-only decision settlement and exact answer transport through the Home program.",
          },
        ],
        verification:
          "Interaction-card and authoritative Home tests, accessibility oracle, mobile typecheck, behavior-contract checks, and repository checks; physical screen-reader evidence remains T3M-F2.",
      },
      {
        contractId: "openagents_mobile.transcript.grouped_runtime_work.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile coding transcript",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "accepted-owner-plan",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "Confirmed runtime activity in the mobile transcript is one compact causal work log: it groups the exact run's reasoning, connection, tools, plan steps, usage, failures, and terminal state; names running or settled status, derivable elapsed time, and runtime identity; keeps the latest five useful rows when collapsed; names bounded omissions; and reveals full selectable detail only through typed local disclosure.",
        authorityBoundary:
          "The work log is a presentation of the already-confirmed exact-thread agent timeline. Group and row disclosure are device-local Effect Native state and cannot issue runtime, tool, plan, navigation, or movement actions. Runtime-event schemas, ordering authority, private thread scope, and consequential controls remain unchanged; elapsed time is omitted when confirmed timestamps cannot prove it.",
        evidenceRefs: [
          "apps/openagents-mobile/src/screens/home-core.ts",
          "apps/openagents-mobile/src/screens/khala-core.ts",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#active-packet--t3m-a2",
          "reference:t3code@8b5469863ae1dd696e696de30240ec3da607962d:apps/mobile/src/features/threads/thread-work-log.tsx",
        ],
        oracles: [
          {
            id: "mobile_grouped_runtime_work_log",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/mobile-work-log.test.ts",
            description:
              "Proves event compaction, causal identity, running/settled state, elapsed time, exact collapsed and safety-bound counts, typed group/item disclosure, selectable detail, and removal of generic tool rows.",
          },
          {
            id: "mobile_grouped_runtime_work_authoritative_home",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/authoritative-home.test.ts",
            description:
              "Proves a confirmed running timeline enters the transcript as one work group while the safe follow-up composer remains available.",
          },
        ],
        verification:
          "Mobile work-log, authoritative Home, accessibility, behavior-contract, typecheck, and repository checks; physical screen-reader evidence remains T3M-F2.",
      },
      {
        contractId: "openagents_mobile.t3_code_full_mobile_parity.v1",
        state: "pending",
        surface: "openagents-mobile",
        productArea: "T3 Code mobile parity",
        enforcementTier: "unenforced",
        blockerRefs: [
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#ordered-program",
        ],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-17",
        },
        statement:
          "i want full mobile parity, do the breakdown then start churning thru it",
        authorityBoundary:
          "Parity adapts T3 Code's complete mobile component and interaction grammar to OpenAgents styles while preserving one Effect Native application authority, exact confirmed refs, fail-closed target readiness, local credential custody, bounded private material, portable-session receipts, and server-authoritative consequential actions. It does not authorize release signing, deployment, credentials, or a screenshot-only parity claim.",
        evidenceRefs: [
          "docs/teardowns/2026-07-17-t3-code-openagents-mobile-component-gap-analysis.md",
          "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md",
          "reference:t3code@8b5469863ae1dd696e696de30240ec3da607962d",
          "apps/openagents-mobile/tests/mobile-transcript-content.test.ts",
        ],
        oracles: [
          {
            id: "mobile_t3_parity_transcript_a1",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/mobile-transcript-content.test.ts",
            description:
              "First enforced rung: bounded rich assistant Markdown, fenced code, safe links, and native clipboard actions without changing transcript authority.",
          },
          {
            id: "mobile_t3_full_parity_physical_matrix",
            kind: "planned",
            mode: "e2e",
            ref: "docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md#epic-f--connections-and-native-finish",
            description:
              "Pending complete component census, compact/regular layouts, physical iOS/Android journeys, VoiceOver/TalkBack traversal, signed build evidence, and owner acceptance.",
          },
        ],
        verification:
          "T3M-A1 focused tests plus mobile typecheck and repository checks; full parity remains pending through T3M-F2.",
      },
      {
        contractId: "openagents_mobile.seam.identity.local_first_account_link.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "two-tier native identity",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Mobile boots to a usable device-local identity without an account. Linking a server-verified OpenAgents account adds cross-device Sync; unlink, denial, failure, and restart preserve local-authority rows and return to local-only UX.",
        authorityBoundary:
          "The Expo host owns identity/link/local tables and credentials. Effect Native receives bounded local/account phases only; private identity, owner, token, store, transport, and rows never enter the view program.",
        seam: {
          client: "apps/openagents-mobile/src/screens/home-core.ts",
          server: "apps/openagents-mobile/src/sync/mobile-sync-host-core.ts",
        },
        evidenceRefs: [
          "packages/khala-sync/src/local-authority.ts",
          "packages/khala-sync-client/src/store-core.ts",
          "apps/openagents-mobile/src/app.tsx",
          "github:OpenAgentsInc/openagents#8666",
        ],
        oracles: [
          {
            id: "mobile_local_first_identity",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/local-first-identity.e2e.test.ts",
            description:
              "Proves Expo/Bun parity for stable local identity, verified account link, unlink retention, and local-first projection.",
          },
        ],
        verification:
          "Mobile sync-host, Home, session, typecheck, and shared local-authority suites.",
      },
      {
        contractId: "openagents_mobile.seam.runtime_authoritative_interactions.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "runtime questions and approvals",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "sol-roadmap",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "Mobile renders grouped provider questions, tool approvals, and plan reviews from confirmed exact-thread authority; actions disable while reconciling and only confirmed replacement can show resolved, expired, or revoked state.",
        authorityBoundary:
          "Effect Native selection is local view state. Every consequential decision carries exact interaction/thread/turn and stable decision/idempotency refs through runtime.decideInteraction; cached, late, foreign, revoked, and unconfirmed outcomes never become visible authority.",
        seam: {
          client: "apps/openagents-mobile/src/screens/home-core.ts",
          server: "packages/khala-sync-server/src/runtime-mutators.ts",
        },
        evidenceRefs: [
          "apps/openagents-mobile/src/conversation/mobile-conversation.ts",
          "packages/khala-sync-client/src/runtime-interactions.ts",
          "docs/sol/2026-07-11-cut-16-composer-runtime-interactions-receipt.md",
          "github:OpenAgentsInc/openagents#8696",
        ],
        oracles: [
          {
            id: "mobile_authoritative_runtime_interactions",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/authoritative-home.test.ts",
            description:
              "Proves grouped selection, disabled reconciliation, exact decisions, confirmed resolution, and terminal expired/revoked rendering.",
          },
        ],
        verification:
          "Mobile conversation, authoritative Home, sync-host, full app, and typecheck suites; physical screen-reader and device receipts remain open on #8696.",
      },
      {
        contractId: "openagents_mobile.seam.coding_authenticated_navigation.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "authenticated coding continuity",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "sol-roadmap",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "Mobile lists only live authorized repositories and recent coding sessions, restores the exact stable thread after verified reconnect, and switches through one typed Effect Native action with a generation-fenced live lease.",
        authorityBoundary:
          "Hosted catalog rows remain hidden outside the exact live owner scope. A device-local selection stores refs only; every directory, restored, deep-link, or notification target is revalidated before a conversation can render.",
        seam: {
          client: "apps/openagents-mobile/src/screens/home-core.ts",
          server: "apps/openagents-mobile/src/sync/mobile-sync-host-core.ts",
        },
        evidenceRefs: [
          "apps/openagents-mobile/src/coding/mobile-coding-navigation.ts",
          "apps/openagents-mobile/src/coding/native-coding-target-delivery.ts",
          "apps/openagents-mobile/src/conversation/mobile-conversation.ts",
          "docs/sol/2026-07-11-cut-14-mobile-authenticated-catalog-receipt.md",
          "github:OpenAgentsInc/openagents#8694",
        ],
        oracles: [
          {
            id: "mobile_authenticated_coding_navigation",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/mobile-coding-navigation.test.ts",
            description:
              "Proves live-only projection, target rejection, real SQLite restore, and concurrent-selection fencing.",
          },
          {
            id: "mobile_coding_directory_effect_native",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/authoritative-home.test.ts",
            description:
              "Proves the confirmed directory and session selection use the typed Effect Native intent registry.",
          },
          {
            id: "mobile_native_coding_target_delivery",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/native-coding-target-delivery.test.ts",
            description:
              "Proves bounded reconnect queuing, terminal stale rejection, exact activation, and production native-listener teardown.",
          },
        ],
        verification:
          "Mobile coding, conversation, Home, sync-host, full app, and typecheck suites; physical iOS/Android receipts remain open on #8694.",
      },
      {
        contractId: "openagents_mobile.seam.accessibility_core_flows.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "mobile accessibility",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "sol-roadmap",
          statedBy: "owner",
          statedOn: "2026-07-12",
        },
        statement:
          "Mobile Home and Khala coding flows carry platform font-scale and reduced-motion state into the Effect Native view program, keep primary touch targets at least 44pt and larger under Dynamic Type, expose non-empty labels/roles for chrome, transcript, composer, runtime questions, approvals, and drawer navigation, and avoid app-owned animation in these core flows.",
        authorityBoundary:
          "React Native reads OS accessibility signals only through AccessibilityInfo and useWindowDimensions, then projects bounded booleans/numbers into serializable Effect Native state. No prompt, credential, provider payload, file path, or private Sync row is included in accessibility metadata; physical device screen-reader proof is not claimed by this deterministic oracle.",
        seam: {
          client: "apps/openagents-mobile/src/screens/home-core.ts",
          server: "apps/openagents-mobile/src/screens/home-screen.tsx",
        },
        evidenceRefs: [
          "apps/openagents-mobile/src/screens/home-screen.tsx",
          "apps/openagents-mobile/src/screens/home-core.ts",
          "apps/openagents-mobile/src/screens/khala-core.ts",
          "apps/openagents-mobile/tests/mobile-accessibility.test.ts",
          "github:OpenAgentsInc/openagents#8704",
        ],
        oracles: [
          {
            id: "mobile_accessibility_core_flows",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/mobile-accessibility.test.ts",
            description:
              "Proves bounded font-scale/reduced-motion projection, enlarged touch targets, transcript/composer/runtime-control accessibility metadata, and absence of app-owned animation in mobile core coding flows.",
          },
        ],
        verification:
          "Mobile accessibility oracle, Home/Khala focused suites, mobile typecheck, and app test sweep. Manual VoiceOver/TalkBack and physical device receipts remain intentionally unclaimed.",
      },
      {
        contractId: "openagents_mobile.seam.coding_offline_cache_accounting.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "authenticated coding continuity",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "sol-roadmap",
          statedBy: "owner",
          statedOn: "2026-07-12",
        },
        statement:
          "While hosted coding authority is withheld, the mobile directory loss-accounts the device-local confirmed cache: it names exactly how many confirmed repository and session rows stay cached-but-hidden for the current owner scope plus the durable cursor they were confirmed through, exposes none of the cached row content, and signed-out state stays explicitly unaccounted so no owner's cache is read without a live owner-scope handle.",
        authorityBoundary:
          "Accounting reads only confirmed rows and the durable cursor of the currently authenticated owner scope through the shared Sync store and shared catalog decoders. Counts and cursor are the only projection; refs, names, paths, threads, and bodies of withheld rows never reach the view program, and an offline directory never renders as an empty account without the withheld counts.",
        seam: {
          client: "apps/openagents-mobile/src/screens/home-core.ts",
          server: "apps/openagents-mobile/src/coding/mobile-coding-navigation.ts",
        },
        evidenceRefs: [
          "apps/openagents-mobile/src/coding/mobile-coding-navigation.ts",
          "apps/openagents-mobile/src/screens/home-core.ts",
          "github:OpenAgentsInc/openagents#8694",
        ],
        oracles: [
          {
            id: "mobile_coding_offline_cache_accounting",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/mobile-coding-navigation.test.ts",
            description:
              "Proves real-SQLite withheld/live/signed-out cache accounting with exact counts and cursor, cross-owner and malformed row exclusion, and no cached ref leakage.",
          },
          {
            id: "mobile_coding_offline_cache_drawer",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/authoritative-home.test.ts",
            description:
              "Proves the drawer renders the loss-accounted withheld cache line, distinguishes denial from reconnect wording, and hides it when live or unaccounted.",
          },
        ],
        verification:
          "Mobile coding navigation, authoritative Home, sync-host, full app, and typecheck suites; physical iOS/Android receipts remain open on #8694.",
      },
      {
        contractId: "openagents_mobile.seam.agent_graph_inline_supervision.v1",
        state: "enforced",
        surface: "openagents-mobile",
        productArea: "live agent supervision",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "sol-roadmap",
          statedBy: "owner",
          statedOn: "2026-07-12",
        },
        statement:
          "The mobile conversation renders the confirmed canonical live-agent hierarchy inline above the transcript: root turns, delegate children, lifecycle status, current action, elapsed time, terminal reason, attention state, and per-node token attribution that is exact only when reported and loss-accounted otherwise. Attention auto-opens the stack, a tap selects/inspects the exact typed agent ref locally, at most 40 rows render with the exact hidden remainder named, and historical authority is labeled and never issues live controls.",
        authorityBoundary:
          "Rows come only from confirmed `openagents.live_agent_graph.v1` post-images in the exact live thread scope through the shared provider-neutral presentation model; no parallel graph shape exists. Selection and expansion are local view state; no graph row can dispatch runtime-control or execution-movement intents, and token truth is never synthesized from missing usage.",
        seam: {
          client: "apps/openagents-mobile/src/screens/khala-core.ts",
          server: "apps/openagents-mobile/src/sync/mobile-sync-host-core.ts",
        },
        evidenceRefs: [
          "packages/khala-sync-client/src/live-agent-graph-presentation.ts",
          "apps/openagents-mobile/src/conversation/mobile-conversation.ts",
          "apps/openagents-mobile/src/screens/home-core.ts",
          "docs/sol/2026-07-11-cut-12-live-agent-supervision-ui-receipt.md",
          "github:OpenAgentsInc/openagents#8692",
        ],
        oracles: [
          {
            id: "mobile_agent_graph_inline_supervision",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-mobile/tests/mobile-agent-graph.test.ts",
            description:
              "Proves confirmed hierarchy projection, attention auto-open, tap select/inspect with deterministic replacement fallback, the named 40-row bound, historical control refusal, and exact/loss-accounted token attribution.",
          },
        ],
        verification:
          "Mobile agent-graph oracle, shared presentation suite, mobile typecheck, and app test sweep; physical iOS/Android receipts remain open on #8692.",
      },
    ],
  };
