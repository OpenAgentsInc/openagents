import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts";
export const openAgentsMobileUxContractRegistry: BehaviorContractRegistryDocument =
  {
    schemaVersion: BehaviorContractSchemaVersion,
    version: "2026-07-11.2",
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
    ],
  };
