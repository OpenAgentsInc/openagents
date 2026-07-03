import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "./contract"

export const BACKGROUND_AGENTS_CONTRACT_DOC_PATH =
  "docs/fable/background-agent-behavior-contracts.md"

const pendingOracleBlocker = (task: string): string =>
  `blocker.background_agents.${task}.oracle_not_landed`

/**
 * Background-agent behavior contracts.
 *
 * These entries record the headline invariants from
 * docs/fable/ROADMAP_BACKGROUND_AGENTS.md and issue #8218 before the owning
 * implementation tasks land their oracles. Sibling task PRs should replace the
 * matching pending entry with an enforced one in the same change that adds the
 * oracle test.
 */
export const backgroundAgentsContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract binds dispatch budget enforcement for background-agent definitions. It does not authorize public budget or reliability claims until the BA-B4 dispatch oracles exist and run in the normal sweep.",
      blockerRefs: [pendingOracleBlocker("ba_b4")],
      contractId: "background_agents.dispatch.budget_caps_enforced.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8196",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
      ],
      oracles: [],
      productArea: "background agent dispatch",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "pending",
      statement:
        "Auto-pause after N consecutive failures; maxRunsPerDay / maxRunSeconds / maxCreditsPerDay enforced at dispatch with typed refusals - a buggy background watcher must never be a money pump.",
      surface: "openagents.com-worker",
      verification:
        "Pending BA-B4: add dispatch budget and auto-pause tests, then flip this contract to enforced with those tests as bun-test oracles in the relevant sweep.",
    },
    {
      authorityBoundary:
        "This contract binds compiled background-agent tool policy at the local-lane and Forge git-token boundaries. It does not widen any runtime tool authority beyond the compiled policy.",
      blockerRefs: [],
      contractId: "background_agents.toolset.compiled_policy_enforced.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8192",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "INVARIANTS.md",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
        "packages/agent-runtime-schema/src/index.test.ts",
        "packages/khala-tools/src/dispatcher.test.ts",
        "apps/openagents.com/workers/api/src/forge-tenant-git-auth-store.test.ts",
      ],
      oracles: [
        {
          description:
            "Shared agent-definition policy compiler preserves deny precedence, ask escalation, allow, and default-deny semantics.",
          id: "background_agents.toolset.schema_policy",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/agent-runtime-schema/src/index.test.ts",
        },
        {
          description:
            "Khala local-lane dispatcher enforces compiled name/authority policy before tool execution and routes ask decisions to approval.",
          id: "background_agents.toolset.khala_local_lane",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/khala-tools/src/dispatcher.test.ts",
        },
        {
          description:
            "Forge git token scope compilation permits allowed scopes, escalates ask scopes, and rejects denied token mints.",
          id: "background_agents.toolset.forge_git_scopes",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/forge-tenant-git-auth-store.test.ts",
        },
      ],
      productArea: "background agent tool policy",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "Definition toolset compiles to the ADR-0012 tool-runtime policy object (local lane) and to Forge tenant-token scopes for git access; ask entries route to escalation instead of failing; no lane may reach tools outside compiled policy.",
      surface: "openagents.com-worker",
      verification:
        "BA-A5 is enforced by the agent-runtime-schema compiler test, the packages/khala-tools dispatcher test, and the openagents.com Worker Forge git-token scope test in their normal bun test sweeps.",
    },
    {
      authorityBoundary:
        "This contract binds worker workspace credential hygiene only. It does not claim that owner subscription custody or provider-account refresh flows are complete.",
      blockerRefs: [pendingOracleBlocker("ba_d3")],
      contractId: "background_agents.credentials.no_long_lived_tokens_in_workspaces.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8202",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
      ],
      oracles: [],
      productArea: "background agent credentials",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "pending",
      statement:
        "No long-lived SCM tokens exist in worker workspaces/homes across materialize/run/closeout.",
      surface: "pylon-worker",
      verification:
        "Pending BA-D3: BA-D3's tests are this contract's oracle; flip this contract to enforced only when those tests exist and run in the Pylon sweep.",
    },
    {
      authorityBoundary:
        "This contract binds harness portability for unchanged background-agent definitions. It does not claim semantic parity between all provider outputs beyond the parity fixture's asserted behavior.",
      blockerRefs: [pendingOracleBlocker("ba_a4")],
      contractId: "background_agents.definitions.harness_swap.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8191",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
      ],
      oracles: [],
      productArea: "background agent definitions",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "pending",
      statement:
        "One unchanged background-agent definition runs on Codex and Claude; harness is a field, never load-bearing.",
      surface: "pylon-worker",
      verification:
        "Pending BA-A4: add the parity fixture proving one unchanged definition runs on both harnesses, then flip this contract to enforced with that fixture as the oracle.",
    },
    {
      authorityBoundary:
        "This contract binds truthfulness of run-status indicators in the Khala Code Agents panel. It does not prescribe the final panel layout, copy, or visual treatment.",
      blockerRefs: [pendingOracleBlocker("ba_g4")],
      contractId: "background_agents.agents_panel.run_status_indicators_truthful.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8211",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "contract:khala_code.chat.sidebar_spinner_streaming_only.v1",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
      ],
      oracles: [],
      productArea: "Khala Code Agents panel",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "pending",
      statement:
        "Agents panel run-status indicators must be truthful: an in-progress, queued, failed, complete, or blocked indicator means exactly that run state and nothing else.",
      surface: "khala-code-desktop",
      verification:
        "Pending BA-G4: write the indicator-truthfulness contract before the Agents panel ships, with DOM or source oracles in the Khala Code Desktop sweep.",
    },
    {
      authorityBoundary:
        "This contract binds the warm-on-intent pre-materialize signal path. It does not require every lane to implement a warm path.",
      blockerRefs: [pendingOracleBlocker("ba_e3")],
      contractId: "background_agents.warm_dispatch.honest_no_op_without_warm_path.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8205",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
      ],
      oracles: [],
      productArea: "warm dispatch",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "pending",
      statement:
        "Khala Code composer emits a typed, debounced, owner-scoped pre-materialize signal while a fleet/background run is being composed; honest no-op when the target lane has no warm path.",
      surface: "khala-code-desktop",
      verification:
        "Pending BA-E3: add debounce and gating tests for the composer pre-materialize signal, then flip this contract to enforced with those tests as bun-test oracles.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-03.3",
}
