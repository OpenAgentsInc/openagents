# Background-Agent Behavior Contracts

This document is the human rendering of the background-agent contract registry
in `packages/behavior-contracts/src/background-agents.ts` (schema:
`packages/behavior-contracts`, `@openagentsinc/behavior-contracts`).

Issue #8218 registers the headline invariants from
`docs/fable/ROADMAP_BACKGROUND_AGENTS.md` before their sibling implementation
oracles land. Entries remain `pending` until the owning task adds its oracle
test and flips that exact contract to `enforced`; BA-B4 and BA-A5 are the
first enforced background-agent contracts in this registry.

Registry version: `2026-07-03.4` (schema `openagents.behavior_contracts.v1`)

### `background_agents.dispatch.budget_caps_enforced.v1` - ENFORCED

- **Surface:** openagents.com-worker (background agent dispatch)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Auto-pause after 3 consecutive failures; maxRunsPerDay / maxRunSeconds / maxCreditsPerDay enforced at dispatch with typed refusals - a buggy background watcher must never be a money pump.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.dispatch.definition_budget_caps` (bun-test, unit): Definition dispatch refuses exhausted daily run and credit budgets, rejects invalid run-second budgets, reserves zero credits for current own-Pylon no-spend dispatch, and writes the capped timeout into the Pylon assignment. - `apps/openagents.com/workers/api/src/agent-definition-run-routes.test.ts`
- **Oracle** `background_agents.dispatch.trigger_auto_pause` (bun-test, unit): Trigger rows auto-pause after three consecutive failures, preserve the pause reason, leave due-trigger scans empty while paused, and reset the failure streak on explicit enable. - `apps/openagents.com/workers/api/src/agent-definition-trigger-store.test.ts`
- **Verification:** BA-B4 is enforced by the openagents.com Worker definition-run route tests and trigger-store tests in the normal bun test sweep.
- **Authority boundary:** This contract binds dispatch budget enforcement for background-agent definitions at the openagents.com Worker dispatch boundary. It does not authorize public budget or reliability claims beyond the tested definition-run and trigger-store oracles.

### `background_agents.toolset.compiled_policy_enforced.v1` - ENFORCED

- **Surface:** openagents.com-worker (background agent tool policy)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Definition toolset compiles to the ADR-0012 tool-runtime policy object (local lane) and to Forge tenant-token scopes for git access; ask entries route to escalation instead of failing; no lane may reach tools outside compiled policy.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.toolset.schema_policy` (bun-test, unit): Shared agent-definition policy compiler preserves deny precedence, ask escalation, allow, and default-deny semantics. - `packages/agent-runtime-schema/src/index.test.ts`
- **Oracle** `background_agents.toolset.khala_local_lane` (bun-test, unit): Khala local-lane dispatcher enforces compiled name/authority policy before tool execution and routes ask decisions to approval. - `packages/khala-tools/src/dispatcher.test.ts`
- **Oracle** `background_agents.toolset.forge_git_scopes` (bun-test, unit): Forge git token scope compilation permits allowed scopes, escalates ask scopes, and rejects denied token mints. - `apps/openagents.com/workers/api/src/forge-tenant-git-auth-store.test.ts`
- **Verification:** BA-A5 is enforced by the agent-runtime-schema compiler test, the packages/khala-tools dispatcher test, and the openagents.com Worker Forge git-token scope test in their normal bun test sweeps.
- **Authority boundary:** This contract binds compiled background-agent tool policy at the local-lane and Forge git-token boundaries. It does not widen any runtime tool authority beyond the compiled policy.

### `background_agents.credentials.no_long_lived_tokens_in_workspaces.v1` - PENDING

- **Surface:** pylon-worker (background agent credentials)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** No long-lived SCM tokens exist in worker workspaces/homes across materialize/run/closeout.
- **Enforcement tier:** unenforced
- **Verification:** Pending BA-D3: BA-D3's tests are this contract's oracle; flip this contract to enforced only when those tests exist and run in the Pylon sweep.
- **Blockers:** `blocker.background_agents.ba_d3.oracle_not_landed`
- **Authority boundary:** This contract binds worker workspace credential hygiene only. It does not claim that owner subscription custody or provider-account refresh flows are complete.

### `background_agents.definitions.harness_swap.v1` - PENDING

- **Surface:** pylon-worker (background agent definitions)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** One unchanged background-agent definition runs on Codex and Claude; harness is a field, never load-bearing.
- **Enforcement tier:** unenforced
- **Verification:** Pending BA-A4: add the parity fixture proving one unchanged definition runs on both harnesses, then flip this contract to enforced with that fixture as the oracle.
- **Blockers:** `blocker.background_agents.ba_a4.oracle_not_landed`
- **Authority boundary:** This contract binds harness portability for unchanged background-agent definitions. It does not claim semantic parity between all provider outputs beyond the parity fixture's asserted behavior.

### `background_agents.agents_panel.run_status_indicators_truthful.v1` - PENDING

- **Surface:** khala-code-desktop (Khala Code Agents panel)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Agents panel run-status indicators must be truthful: an in-progress, queued, failed, complete, or blocked indicator means exactly that run state and nothing else.
- **Enforcement tier:** unenforced
- **Verification:** Pending BA-G4: write the indicator-truthfulness contract before the Agents panel ships, with DOM or source oracles in the Khala Code Desktop sweep.
- **Blockers:** `blocker.background_agents.ba_g4.oracle_not_landed`
- **Authority boundary:** This contract binds truthfulness of run-status indicators in the Khala Code Agents panel. It does not prescribe the final panel layout, copy, or visual treatment.

### `background_agents.warm_dispatch.honest_no_op_without_warm_path.v1` - PENDING

- **Surface:** khala-code-desktop (warm dispatch)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Khala Code composer emits a typed, debounced, owner-scoped pre-materialize signal while a fleet/background run is being composed; honest no-op when the target lane has no warm path.
- **Enforcement tier:** unenforced
- **Verification:** Pending BA-E3: add debounce and gating tests for the composer pre-materialize signal, then flip this contract to enforced with those tests as bun-test oracles.
- **Blockers:** `blocker.background_agents.ba_e3.oracle_not_landed`
- **Authority boundary:** This contract binds the warm-on-intent pre-materialize signal path. It does not require every lane to implement a warm path.
