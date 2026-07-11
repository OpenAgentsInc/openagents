import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts";
export const openAgentsMobileUxContractRegistry: BehaviorContractRegistryDocument =
  {
    schemaVersion: BehaviorContractSchemaVersion,
    version: "2026-07-10.1",
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
    ],
  };
