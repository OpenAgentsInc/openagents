import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts";
export const openAgentsMobileUxContractRegistry: BehaviorContractRegistryDocument =
  {
    schemaVersion: BehaviorContractSchemaVersion,
    version: "2026-07-17.1",
    contracts: [
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
        seam: {
          client: "apps/openagents-mobile/src/screens/home-core.ts",
          server: "apps/openagents-mobile/src/effect-native/effect-native-host.tsx",
        },
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
