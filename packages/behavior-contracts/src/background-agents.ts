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
        "This contract binds dispatch budget enforcement for background-agent definitions at the openagents.com Worker dispatch boundary. It does not authorize public budget or reliability claims beyond the tested definition-run and trigger-store oracles.",
      blockerRefs: [],
      contractId: "background_agents.dispatch.budget_caps_enforced.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8196",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "INVARIANTS.md",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
        "apps/openagents.com/workers/api/src/agent-definition-run-routes.test.ts",
        "apps/openagents.com/workers/api/src/agent-definition-trigger-store.test.ts",
        "apps/openagents.com/workers/api/migrations/0282_agent_definition_run_budget_credits.sql",
      ],
      oracles: [
        {
          description:
            "Definition dispatch refuses exhausted daily run and credit budgets, rejects invalid run-second budgets, reserves zero credits for current own-Pylon no-spend dispatch, and writes the capped timeout into the Pylon assignment.",
          id: "background_agents.dispatch.definition_budget_caps",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/agent-definition-run-routes.test.ts",
        },
        {
          description:
            "Trigger rows auto-pause after three consecutive failures, preserve the pause reason, leave due-trigger scans empty while paused, and reset the failure streak on explicit enable.",
          id: "background_agents.dispatch.trigger_auto_pause",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/agent-definition-trigger-store.test.ts",
        },
      ],
      productArea: "background agent dispatch",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "Auto-pause after 3 consecutive failures; maxRunsPerDay / maxRunSeconds / maxCreditsPerDay enforced at dispatch with typed refusals - a buggy background watcher must never be a money pump.",
      surface: "openagents.com-worker",
      verification:
        "BA-B4 is enforced by the openagents.com Worker definition-run route tests and trigger-store tests in the normal bun test sweep.",
    },
    {
      authorityBoundary:
        "This contract binds Pylon delegate dispatch admission and local orchestration-store breaker state only. It does not claim provider-account custody, payment settlement, or public availability guarantees beyond the tested planner/store behavior.",
      blockerRefs: [],
      contractId: "background_agents.dispatch.lane_account_breaker.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8206",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "INVARIANTS.md",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
        "apps/pylon/src/orchestration/supervisor-orchestration.test.ts",
        "apps/pylon/tests/khala-spawn.test.ts",
        "apps/pylon/tests/khala-dispatch.test.ts",
        "apps/pylon/tests/khala-burndown.test.ts",
      ],
      oracles: [
        {
          description:
            "The local orchestration store classifies transient and permanent dispatch failures, persists per-account/lane breaker rows, cools transient failures, and quarantines permanent credential failures.",
          id: "background_agents.dispatch.orchestration_store_breaker",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/src/orchestration/supervisor-orchestration.test.ts",
        },
        {
          description:
            "Khala spawn planning zeroes advertised capacity for cooled account/lane breakers, skips broken accounts, and projects timeout failures into typed transient dispatch classifications.",
          id: "background_agents.dispatch.khala_spawn_breaker",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/khala-spawn.test.ts",
        },
        {
          description:
            "Khala dispatch planning filters cooled Codex account/lane breakers before selecting candidate slots.",
          id: "background_agents.dispatch.khala_dispatch_breaker",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/khala-dispatch.test.ts",
        },
        {
          description:
            "Khala burndown planning skips cooled account/lane breakers while assigning issue slots.",
          id: "background_agents.dispatch.khala_burndown_breaker",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/khala-burndown.test.ts",
        },
      ],
      productArea: "background agent dispatch",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "Dispatch failures are classified as transient or permanent; per-account/lane breakers cool or quarantine failed lanes and feed delegate readiness/capacity instead of repeatedly dispatching into known failures.",
      surface: "pylon-worker",
      verification:
        "BA-F1 is enforced by the Pylon orchestration store test plus Khala spawn, dispatch, and burndown planner tests in the normal Pylon bun test sweep.",
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
        "This contract proves the brokered helper shape and ref-only dispatch boundary. The broader no-long-lived-token runtime sweep is enforced by background_agents.credentials.no_long_lived_tokens_in_workspaces.v1.",
      blockerRefs: [],
      contractId: "background_agents.credentials.brokered_scm_helper.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8201",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "INVARIANTS.md",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
        "apps/openagents.com/workers/api/src/agent-definition-run-routes.test.ts",
        "apps/pylon/tests/workspace-materializer.test.ts",
      ],
      oracles: [
        {
          description:
            "Definition dispatch attaches scmAuthBroker metadata with Forge token refs to Pylon git_checkout assignments and never includes raw oa_forge_git_ token material.",
          id: "background_agents.credentials.dispatch_broker_refs",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/agent-definition-run-routes.test.ts",
        },
        {
          description:
            "The Pylon workspace materializer validates broker metadata, rejects raw/malformed broker shapes, writes helper config under Git admin state, configures credential.useHttpPath, fails closed, and stores no raw SCM token in the generated config/script.",
          id: "background_agents.credentials.pylon_helper_install",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/workspace-materializer.test.ts",
        },
      ],
      productArea: "background agent SCM credentials",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "Worker-side background-agent Git credentials are brokered: dispatch sends only ref metadata, and the Pylon materializer installs a per-task Git credential helper that scopes requests by protocol, host, and path, uses a bounded cache, and never reads embedded SCM credentials from the workspace.",
      surface: "pylon-worker + openagents.com-worker",
      verification:
        "BA-D2 is enforced by the agent-definition run route test plus the Pylon workspace materializer test in their normal sweeps.",
    },
    {
      authorityBoundary:
        "This contract binds worker workspace/account-home credential hygiene only. It does not claim that owner subscription custody or provider-account refresh flows are complete.",
      blockerRefs: [],
      contractId: "background_agents.credentials.no_long_lived_tokens_in_workspaces.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8202",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "INVARIANTS.md",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
        "apps/pylon/tests/workspace-materializer.test.ts",
        "apps/pylon/tests/workspace-worktree.test.ts",
        "apps/pylon/tests/codex-agent-executor.test.ts",
        "apps/pylon/tests/claude-agent-executor.test.ts",
      ],
      oracles: [
        {
          description:
            "scanLongLivedScmCredentials detects GitHub PATs / raw Forge git tokens / credentialed Git URLs in worker roots while allowing bounded helper cache entries.",
          id: "background_agents.credentials.long_lived_scm_scanner",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/workspace-materializer.test.ts",
        },
        {
          description:
            "Workspace lease cleanup removes a dirty workspace when the dirty content contains long-lived SCM credential material instead of retaining it.",
          id: "background_agents.credentials.closeout_cleanup",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/workspace-worktree.test.ts",
        },
        {
          description:
            "Codex git-checkout runs scan the bounded workspace plus selected CODEX_HOME, refuse with scm_credential_policy_failed, and clean the lease on detection.",
          id: "background_agents.credentials.codex_runtime_sweep",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/codex-agent-executor.test.ts",
        },
        {
          description:
            "Claude git-checkout runs scan the bounded workspace plus selected CLAUDE_CONFIG_DIR, refuse with scm_credential_policy_failed, and clean the lease on detection.",
          id: "background_agents.credentials.claude_runtime_sweep",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/claude-agent-executor.test.ts",
        },
      ],
      productArea: "background agent credentials",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "No long-lived SCM tokens exist in worker workspaces/homes across materialize/run/closeout. Short-lived helper cache entries may exist only under Git admin state; GitHub PATs, raw Forge git tokens, credentialed Git URLs, and Git extraheader authorization material are rejected in the bounded checkout or selected isolated account home.",
      surface: "pylon-worker",
      verification:
        "BA-D3 is enforced by the Pylon materializer, worktree, Codex executor, and Claude executor tests in the normal Pylon bun test sweep.",
    },
    {
      authorityBoundary:
        "This contract binds the Pylon materializer prepared-worktree source cache only. It does not claim prebuilt dependency baselines or Khala Code warm-on-intent dispatch, which remain BA-E2/BA-E3 scope.",
      blockerRefs: [],
      contractId: "background_agents.warm_dispatch.prepared_worktree_cache.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8203",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "INVARIANTS.md",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
        "apps/pylon/docs/workspace-materializer.md",
        "apps/pylon/tests/workspace-worktree.test.ts",
      ],
      oracles: [
        {
          description:
            "preparedWorktreeCacheKeyFor is stable for one repository+baseline pair and changes across repository names or baseline commits.",
          id: "background_agents.warm_dispatch.prepared_cache_key",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/workspace-worktree.test.ts",
        },
        {
          description:
            "A clean closeout records a post_completion_snapshot prepared entry, and the next matching repo+baseline materialization restores with restore_quick_sync_reset without contacting the remote.",
          id: "background_agents.warm_dispatch.prepared_cache_snapshot_restore",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/workspace-worktree.test.ts",
        },
        {
          description:
            "Prepared cache integrity rejects dirty/stale entries and the byte budget evicts oldest prepared entries while retaining the newest fitting entry.",
          id: "background_agents.warm_dispatch.prepared_cache_integrity_budget",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/workspace-worktree.test.ts",
        },
      ],
      productArea: "warm dispatch",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "Prepared-worktree cache in the Pylon workspace materializer: typed reuse reasons (post-completion snapshot, restore = quick sync + reset), cache keyed by repo+baseline, integrity checks, bounded disk budget with eviction.",
      surface: "pylon-worker",
      verification:
        "BA-E1 is enforced by the Pylon workspace-worktree test suite in the normal Pylon bun test sweep.",
    },
    {
      authorityBoundary:
        "This contract binds local Pylon prebuilt baseline cache selection, refresh, and metrics only. It does not claim post-completion exact prepared snapshots or Khala Code warm-on-intent dispatch.",
      blockerRefs: [],
      contractId: "background_agents.warm_dispatch.prebuilt_baseline_cache.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8204",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "INVARIANTS.md",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
        "apps/pylon/docs/workspace-materializer.md",
        "apps/pylon/tests/workspace-worktree.test.ts",
      ],
      oracles: [
        {
          description:
            "prebuiltBaselineCacheKeyFor is stable for one repository+branch pair and changes across repository names or branches.",
          id: "background_agents.warm_dispatch.prebuilt_baseline_key",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/workspace-worktree.test.ts",
        },
        {
          description:
            "A cold materialization builds the newest upstream prebuilt baseline, runs setup once, restores later workspaces with setup artifacts preserved, and records registry hit counts.",
          id: "background_agents.warm_dispatch.prebuilt_baseline_hit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/workspace-worktree.test.ts",
        },
        {
          description:
            "A requested commit that is newer than the cached prebuild before the refresh cadence records an honest miss and falls back to normal materialization, then a due cadence refresh advances to the newest upstream baseline.",
          id: "background_agents.warm_dispatch.prebuilt_baseline_refresh_metrics",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/pylon/tests/workspace-worktree.test.ts",
        },
      ],
      productArea: "warm dispatch",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "Prebuilt baselines in the Pylon workspace materializer use a staleness-checked upstream refresh cadence, start matching cold dispatches from a setup-prepared baseline, and keep registry rows with honest hit/miss metrics.",
      surface: "pylon-worker",
      verification:
        "BA-E2 is enforced by the Pylon workspace-worktree test suite in the normal Pylon bun test sweep.",
    },
    {
      authorityBoundary:
        "This contract binds the background-agent bot integration template for Forum-triggered runs only. It does not authorize arbitrary Forum writes, raw Forum body payloads in model-visible trigger context, or non-Forum provider callbacks beyond their own future source-specific verification.",
      blockerRefs: [],
      contractId: "background_agents.integrations.forum_trigger_callback.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8208",
        "https://github.com/OpenAgentsInc/openagents/issues/8218",
        "INVARIANTS.md",
        "docs/fable/ROADMAP_BACKGROUND_AGENTS.md",
        "packages/agent-runtime-schema/src/webhooks.test.ts",
        "apps/openagents.com/workers/api/src/agent-definition-webhook-routes.test.ts",
      ],
      oracles: [
        {
          description:
            "Forum webhook normalization emits only bounded event, actor, forum, topic, post, source URL, and source-ref fields that trigger conditions can match without exposing raw Forum body text.",
          id: "background_agents.integrations.forum_event_normalization",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/agent-runtime-schema/src/webhooks.test.ts",
        },
        {
          description:
            "The Forum webhook route verifies the signed source event before dispatch, uses the shared bot-integration trigger template, stores a Forum completion callback descriptor on the run trigger payload, and the completion route posts only through the stored run callback plus Forum writer policy.",
          id: "background_agents.integrations.forum_dispatch_callback",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/agent-definition-webhook-routes.test.ts",
        },
      ],
      productArea: "background agent integrations",
      source: {
        channel: "issue_list",
        statedBy: "owner",
        statedOn: "2026-07-03",
      },
      state: "enforced",
      statement:
        "Forum-triggered background-agent runs follow one integration template: signed source event, verified Forum source post, bounded normalization, owner-scoped definition dispatch, and a completion callback that can post only back to the stored source Forum thread through Forum write authority.",
      surface: "openagents.com-worker",
      verification:
        "BA-G1 is enforced by agent-runtime-schema Forum webhook normalization tests and openagents.com Worker Forum webhook/completion route tests in the normal bun test sweep.",
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
  version: "2026-07-03.8",
}
