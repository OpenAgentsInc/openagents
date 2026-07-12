import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts";
export const openAgentsMobileUxContractRegistry: BehaviorContractRegistryDocument =
  {
    schemaVersion: BehaviorContractSchemaVersion,
    version: "2026-07-12.2",
    contracts: [
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
    ],
  };
