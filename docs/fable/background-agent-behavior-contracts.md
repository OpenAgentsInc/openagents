Registry version: `2026-07-04.5` (schema `openagents.behavior_contracts.v1`)

### `background_agents.dispatch.budget_caps_enforced.v1` — ENFORCED

- **Surface:** openagents.com-worker (background agent dispatch)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Auto-pause after 3 consecutive failures; maxRunsPerDay / maxRunSeconds / maxCreditsPerDay enforced at dispatch with typed refusals - a buggy background watcher must never be a money pump.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.dispatch.definition_budget_caps` (bun-test, unit): Definition dispatch refuses exhausted daily run and credit budgets, rejects invalid run-second budgets, reserves zero credits for current own-Pylon no-spend dispatch, and writes the capped timeout into the Pylon assignment. — `apps/openagents.com/workers/api/src/agent-definition-run-routes.test.ts`
- **Oracle** `background_agents.dispatch.trigger_auto_pause` (bun-test, unit): Trigger rows auto-pause after three consecutive failures, preserve the pause reason, leave due-trigger scans empty while paused, and reset the failure streak on explicit enable. — `apps/openagents.com/workers/api/src/agent-definition-trigger-store.test.ts`
- **Verification:** BA-B4 is enforced by the openagents.com Worker definition-run route tests and trigger-store tests in the normal bun test sweep.
- **Authority boundary:** This contract binds dispatch budget enforcement for background-agent definitions at the openagents.com Worker dispatch boundary. It does not authorize public budget or reliability claims beyond the tested definition-run and trigger-store oracles.

### `background_agents.dispatch.lane_account_breaker.v1` — ENFORCED

- **Surface:** pylon-worker (background agent dispatch)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Dispatch failures are classified as transient or permanent; per-account/lane breakers cool or quarantine failed lanes and feed delegate readiness/capacity instead of repeatedly dispatching into known failures.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.dispatch.orchestration_store_breaker` (bun-test, unit): The local orchestration store classifies transient and permanent dispatch failures, persists per-account/lane breaker rows, cools transient failures, and quarantines permanent credential failures. — `apps/pylon/src/orchestration/supervisor-orchestration.test.ts`
- **Oracle** `background_agents.dispatch.khala_spawn_breaker` (bun-test, unit): Khala spawn planning zeroes advertised capacity for cooled account/lane breakers, skips broken accounts, and projects timeout failures into typed transient dispatch classifications. — `apps/pylon/tests/khala-spawn.test.ts`
- **Oracle** `background_agents.dispatch.khala_dispatch_breaker` (bun-test, unit): Khala dispatch planning filters cooled Codex account/lane breakers before selecting candidate slots. — `apps/pylon/tests/khala-dispatch.test.ts`
- **Oracle** `background_agents.dispatch.khala_burndown_breaker` (bun-test, unit): Khala burndown planning skips cooled account/lane breakers while assigning issue slots. — `apps/pylon/tests/khala-burndown.test.ts`
- **Verification:** BA-F1 is enforced by the Pylon orchestration store test plus Khala spawn, dispatch, and burndown planner tests in the normal Pylon bun test sweep.
- **Authority boundary:** This contract binds Pylon delegate dispatch admission and local orchestration-store breaker state only. It does not claim provider-account custody, payment settlement, or public availability guarantees beyond the tested planner/store behavior.

### `background_agents.toolset.compiled_policy_enforced.v1` — ENFORCED

- **Surface:** openagents.com-worker (background agent tool policy)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Definition toolset compiles to the ADR-0012 tool-runtime policy object (local lane) and to Forge tenant-token scopes for git access; ask entries route to escalation instead of failing; no lane may reach tools outside compiled policy.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.toolset.schema_policy` (bun-test, unit): Shared agent-definition policy compiler preserves deny precedence, ask escalation, allow, and default-deny semantics. — `packages/agent-runtime-schema/src/index.test.ts`
- **Oracle** `background_agents.toolset.khala_local_lane` (bun-test, unit): Khala local-lane dispatcher enforces compiled name/authority policy before tool execution and routes ask decisions to approval. — `packages/khala-tools/src/dispatcher.test.ts`
- **Oracle** `background_agents.toolset.forge_git_scopes` (bun-test, unit): Forge git token scope compilation permits allowed scopes, escalates ask scopes, and rejects denied token mints. — `apps/openagents.com/workers/api/src/forge-tenant-git-auth-store.test.ts`
- **Verification:** BA-A5 is enforced by the agent-runtime-schema compiler test, the packages/khala-tools dispatcher test, and the openagents.com Worker Forge git-token scope test in their normal bun test sweeps.
- **Authority boundary:** This contract binds compiled background-agent tool policy at the local-lane and Forge git-token boundaries. It does not widen any runtime tool authority beyond the compiled policy.

### `lead_gen_agent.drafting_only_toolset.v1` — ENFORCED

- **Surface:** openagents.com-worker (Autopilot Lead Gen)
- **Stated by:** owner via issue on 2026-07-04
- **Statement:** Autopilot Lead Gen v0 is a drafting-only standing background agent: target discovery, agent-readiness analysis, report drafts, sequence-entry drafts, receipt writing, and operator-inbox escalation are permitted; outreach send/activation tools are denied and absent from allow/ask.
- **Enforcement tier:** test-sweep
- **Oracle** `lead_gen_agent.drafting_only_toolset` (bun-test, unit): The Autopilot Lead Gen definition has no send tools in allow/ask, explicitly denies email/Apollo send and activation refs, allows only drafting/analyzer/operator-inbox work plus the Forge receive-pack dispatch scope, and compiles as dispatchable. — `apps/openagents.com/workers/api/src/autopilot-lead-gen-agent-definition.test.ts`
- **Verification:** LG-7 is enforced by the openagents.com Worker lead-gen agent-definition test in the normal bun test sweep.
- **Authority boundary:** This contract binds only the v0 Autopilot Lead Gen background-agent definition and its compiled toolset. It allows the Forge receive-pack scope needed for owner-Pylon dispatch but grants no Apollo send, email send, contact campaign activation, spend, payout, settlement, or customer-result claim authority.

### `lead_gen_agent.no_send_without_approval_receipt.v1` — ENFORCED

- **Surface:** openagents.com-worker (Autopilot Lead Gen)
- **Stated by:** owner via issue on 2026-07-04
- **Statement:** Autopilot Lead Gen may produce reports and sequence drafts, but no outreach leaves the system unless a separate LG-4 approval receipt exists; the v0 dogfood receipt records sendAuthority.allowed=false.
- **Enforcement tier:** test-sweep
- **Oracle** `lead_gen_agent.no_send_without_approval_receipt` (bun-test, unit): The run payload and OpenAgents dogfood receipt record sendAuthority.allowed=false, keep drafted reports/sequences in review state, name the operator inbox, and require a separate LG-4 approval receipt before any send. — `apps/openagents.com/workers/api/src/autopilot-lead-gen-agent-definition.test.ts`
- **Verification:** LG-7 is enforced by the openagents.com Worker lead-gen agent-definition test in the normal bun test sweep.
- **Authority boundary:** This contract binds only the public-safe v0 dogfood run receipt and run payload shape for Autopilot Lead Gen. It does not authorize live sends, Apollo credential use, contact reveal, customer delivery claims, or marketing copy that the product is available.

### `background_agents.credentials.brokered_scm_helper.v1` — ENFORCED

- **Surface:** pylon-worker + openagents.com-worker (background agent SCM credentials)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Worker-side background-agent Git credentials are brokered: dispatch sends only ref metadata, and the Pylon materializer installs a per-task Git credential helper that scopes requests by protocol, host, and path, uses a bounded cache, and never reads embedded SCM credentials from the workspace.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.credentials.dispatch_broker_refs` (bun-test, unit): Definition dispatch attaches scmAuthBroker metadata with Forge token refs to Pylon git_checkout assignments and never includes raw oa_forge_git_ token material. — `apps/openagents.com/workers/api/src/agent-definition-run-routes.test.ts`
- **Oracle** `background_agents.credentials.pylon_helper_install` (bun-test, unit): The Pylon workspace materializer validates broker metadata, rejects raw/malformed broker shapes, writes helper config under Git admin state, configures credential.useHttpPath, fails closed, and stores no raw SCM token in the generated config/script. — `apps/pylon/tests/workspace-materializer.test.ts`
- **Oracle** `background_agents.credentials.brokered_writeback` (bun-test, unit): The Pylon PR publisher resolves the brokered Git credential for GitHub API calls, never puts the token in command args, pushes scoped assignment branches without a force refspec, and maps GitHub permission failures to typed refs. — `apps/pylon/tests/codex-pr-publisher.test.ts`
- **Verification:** BA-D2 is enforced by the agent-definition run route test plus the Pylon workspace materializer test in their normal sweeps.
- **Authority boundary:** This contract proves the brokered helper shape and ref-only dispatch boundary. The broader no-long-lived-token runtime sweep is enforced by background_agents.credentials.no_long_lived_tokens_in_workspaces.v1.

### `background_agents.credentials.no_long_lived_tokens_in_workspaces.v1` — ENFORCED

- **Surface:** pylon-worker (background agent credentials)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** No long-lived SCM tokens exist in worker workspaces/homes across materialize/run/closeout. Short-lived helper cache entries may exist only under Git admin state; GitHub PATs, raw Forge git tokens, credentialed Git URLs, and Git extraheader authorization material are rejected in the bounded checkout or selected isolated account home.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.credentials.long_lived_scm_scanner` (bun-test, unit): scanLongLivedScmCredentials detects GitHub PATs / raw Forge git tokens / credentialed Git URLs in worker roots while allowing bounded helper cache entries. — `apps/pylon/tests/workspace-materializer.test.ts`
- **Oracle** `background_agents.credentials.closeout_cleanup` (bun-test, unit): Workspace lease cleanup removes a dirty workspace when the dirty content contains long-lived SCM credential material instead of retaining it. — `apps/pylon/tests/workspace-worktree.test.ts`
- **Oracle** `background_agents.credentials.codex_runtime_sweep` (bun-test, unit): Codex git-checkout runs scan the bounded workspace plus selected CODEX_HOME, refuse with scm_credential_policy_failed, and clean the lease on detection. — `apps/pylon/tests/codex-agent-executor.test.ts`
- **Oracle** `background_agents.credentials.claude_runtime_sweep` (bun-test, unit): Claude git-checkout runs scan the bounded workspace plus selected CLAUDE_CONFIG_DIR, refuse with scm_credential_policy_failed, and clean the lease on detection. — `apps/pylon/tests/claude-agent-executor.test.ts`
- **Verification:** BA-D3 is enforced by the Pylon materializer, worktree, Codex executor, and Claude executor tests in the normal Pylon bun test sweep.
- **Authority boundary:** This contract binds worker workspace/account-home credential hygiene only. It does not claim that owner subscription custody or provider-account refresh flows are complete.

### `background_agents.warm_dispatch.prepared_worktree_cache.v1` — ENFORCED

- **Surface:** pylon-worker (warm dispatch)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Prepared-worktree cache in the Pylon workspace materializer: typed reuse reasons (post-completion snapshot, restore = quick sync + reset), cache keyed by repo+baseline, integrity checks, bounded disk budget with eviction.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.warm_dispatch.prepared_cache_key` (bun-test, unit): preparedWorktreeCacheKeyFor is stable for one repository+baseline pair and changes across repository names or baseline commits. — `apps/pylon/tests/workspace-worktree.test.ts`
- **Oracle** `background_agents.warm_dispatch.prepared_cache_snapshot_restore` (bun-test, unit): A clean closeout records a post_completion_snapshot prepared entry, and the next matching repo+baseline materialization restores with restore_quick_sync_reset without contacting the remote. — `apps/pylon/tests/workspace-worktree.test.ts`
- **Oracle** `background_agents.warm_dispatch.prepared_cache_integrity_budget` (bun-test, unit): Prepared cache integrity rejects dirty/stale entries and the byte budget evicts oldest prepared entries while retaining the newest fitting entry. — `apps/pylon/tests/workspace-worktree.test.ts`
- **Verification:** BA-E1 is enforced by the Pylon workspace-worktree test suite in the normal Pylon bun test sweep.
- **Authority boundary:** This contract binds the Pylon materializer prepared-worktree source cache only. It does not claim prebuilt dependency baselines or Khala Code warm-on-intent dispatch, which remain BA-E2/BA-E3 scope.

### `background_agents.warm_dispatch.prebuilt_baseline_cache.v1` — ENFORCED

- **Surface:** pylon-worker (warm dispatch)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Prebuilt baselines in the Pylon workspace materializer use a staleness-checked upstream refresh cadence, start matching cold dispatches from a setup-prepared baseline, and keep registry rows with honest hit/miss metrics.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.warm_dispatch.prebuilt_baseline_key` (bun-test, unit): prebuiltBaselineCacheKeyFor is stable for one repository+branch pair and changes across repository names or branches. — `apps/pylon/tests/workspace-worktree.test.ts`
- **Oracle** `background_agents.warm_dispatch.prebuilt_baseline_hit` (bun-test, unit): A cold materialization builds the newest upstream prebuilt baseline, runs setup once, restores later workspaces with setup artifacts preserved, and records registry hit counts. — `apps/pylon/tests/workspace-worktree.test.ts`
- **Oracle** `background_agents.warm_dispatch.prebuilt_baseline_refresh_metrics` (bun-test, unit): A requested commit that is newer than the cached prebuild before the refresh cadence records an honest miss and falls back to normal materialization, then a due cadence refresh advances to the newest upstream baseline. — `apps/pylon/tests/workspace-worktree.test.ts`
- **Verification:** BA-E2 is enforced by the Pylon workspace-worktree test suite in the normal Pylon bun test sweep.
- **Authority boundary:** This contract binds local Pylon prebuilt baseline cache selection, refresh, and metrics only. It does not claim post-completion exact prepared snapshots or Khala Code warm-on-intent dispatch.

### `background_agents.integrations.forum_trigger_callback.v1` — ENFORCED

- **Surface:** openagents.com-worker (background agent integrations)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Forum-triggered background-agent runs follow one integration template: signed source event, verified Forum source post, bounded normalization, owner-scoped definition dispatch, and a completion callback that can post only back to the stored source Forum thread through Forum write authority.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.integrations.forum_event_normalization` (bun-test, unit): Forum webhook normalization emits only bounded event, actor, forum, topic, post, source URL, and source-ref fields that trigger conditions can match without exposing raw Forum body text. — `packages/agent-runtime-schema/src/webhooks.test.ts`
- **Oracle** `background_agents.integrations.forum_dispatch_callback` (bun-test, unit): The Forum webhook route verifies the signed source event before dispatch, uses the shared bot-integration trigger template, stores a Forum completion callback descriptor on the run trigger payload, and the completion route posts only through the stored run callback plus Forum writer policy. — `apps/openagents.com/workers/api/src/agent-definition-webhook-routes.test.ts`
- **Verification:** BA-G1 is enforced by agent-runtime-schema Forum webhook normalization tests and openagents.com Worker Forum webhook/completion route tests in the normal bun test sweep.
- **Authority boundary:** This contract binds the background-agent bot integration template for Forum-triggered runs only. It does not authorize arbitrary Forum writes, raw Forum body payloads in model-visible trigger context, or non-Forum provider callbacks beyond their own future source-specific verification.

### `background_agents.integrations.github_mention_callback.v1` — ENFORCED

- **Surface:** openagents.com-worker (background agent integrations)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** GitHub @mention background-agent runs follow the shared integration template: signed issue_comment.created event, configured mention extraction, bounded normalization, owner-scoped definition dispatch, and a result comment posted only back to the stored source issue or PR thread without loose issue spam.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.integrations.github_mention_normalization` (bun-test, unit): GitHub issue-comment webhook normalization upgrades only configured @mentions to issue_comment.created.mention, emits bounded repository/subject/comment/mention refs, and excludes raw comment body text from trigger payloads. — `packages/agent-runtime-schema/src/webhooks.test.ts`
- **Oracle** `background_agents.integrations.github_mention_dispatch_callback` (bun-test, unit): The GitHub webhook route verifies signatures before dispatch, stores a subject-bound GitHub completion callback on mention-triggered runs, skips ordinary comments, and the completion route posts through the stored callback with app-owned idempotency. — `apps/openagents.com/workers/api/src/agent-definition-webhook-routes.test.ts`
- **Verification:** BA-G2 is enforced by agent-runtime-schema GitHub mention normalization tests and openagents.com Worker GitHub webhook/completion route tests in the normal bun test sweep.
- **Authority boundary:** This contract binds GitHub issue-comment @mention integration for background-agent definitions only. It does not authorize arbitrary GitHub writes, issue creation, raw GitHub body payloads in model-visible trigger context, or completion targets supplied by callback callers.

### `background_agents.inbox.event_ledger_owner_scoped_private.v1` — ENFORCED

- **Surface:** openagents.com-worker (background agent inbox)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** event_ledger.v1 ingests matched GitHub source events through Queues into owner-scoped private D1 rows, orders and dedupes through a per-owner Durable Object, stores refs and bounded summaries rather than raw content, and is never training data or cross-account projection material.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.inbox.event_ledger_private_rows` (bun-test, unit): The event ledger projects signed GitHub webhook events into owner-scoped queue messages and D1 rows with source, externalRef, actor, contentRef, subjectRef, timestamps, false training consent, no raw comment/title content, and per-owner deduped ordering. — `apps/openagents.com/workers/api/src/event-ledger.test.ts`
- **Oracle** `background_agents.inbox.github_webhook_enqueue` (bun-test, unit): The GitHub webhook route enqueues exactly one event-ledger message per matched owner trigger while preserving the existing signed normalization and owner-scoped dispatch path. — `apps/openagents.com/workers/api/src/agent-definition-webhook-routes.test.ts`
- **Verification:** BA-H1 is enforced by the openagents.com Worker event-ledger and GitHub webhook route tests in the normal bun test sweep.
- **Authority boundary:** This contract binds private event-ledger ingest for matched owner-scoped background-agent GitHub triggers. It does not authorize public projection, training use, cross-owner reads, handled-state mutation, Slack ingest, or model-visible raw source payloads.

### `background_agents.inbox.event_ledger_handled_gateway_redacted.v1` — ENFORCED

- **Surface:** openagents.com-worker (background agent inbox)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** event_ledger.v1 treats handled-state as first-class (`open`, `handled`, `responded`, `ignored`), records the definition run that touched an entry, and exposes owner-scoped definition reads only through a toolset-gated gateway that redacts according to secretPolicy.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.inbox.event_ledger_handled_state` (bun-test, unit): The event ledger persists first-class handled-state, records the touching run and definition, filters by state, and refuses cross-owner handled-state updates. — `apps/openagents.com/workers/api/src/event-ledger.test.ts`
- **Oracle** `background_agents.inbox.event_ledger_gateway_redaction` (bun-test, unit): The definition event-ledger gateway authenticates the owner, enforces the definition toolset, verifies same-definition touching runs, and redacts reads according to secretPolicy. — `apps/openagents.com/workers/api/src/agent-definition-event-ledger-routes.test.ts`
- **Verification:** BA-H2 is enforced by the openagents.com Worker event-ledger store and definition event-ledger gateway route tests in the normal bun test sweep.
- **Authority boundary:** This contract binds the private event-ledger handled-state and definition gateway read path. It does not authorize public projection, raw provider payload disclosure, cross-owner reads, unredacted model context, Slack ingest, or handled-state updates from unrelated runs.

### `background_agents.inbox.slack_event_ledger_ingest.v1` — ENFORCED

- **Surface:** openagents.com-worker (background agent inbox)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Slack webhook events enter event_ledger.v1 only after Slack signature verification and typed normalization, then become owner-scoped private rows with refs and bounded summaries rather than raw message text.
- **Enforcement tier:** test-sweep
- **Oracle** `background_agents.inbox.slack_event_normalization` (bun-test, unit): Slack webhook normalization emits only bounded team, channel, actor, message timestamp, event, and source-ref fields that trigger conditions can match without exposing raw Slack message text or legacy verification tokens. — `packages/agent-runtime-schema/src/webhooks.test.ts`
- **Oracle** `background_agents.inbox.slack_webhook_enqueue` (bun-test, unit): The Slack webhook route verifies Slack HMAC signatures and replay windows before dispatch, matches owner-scoped Slack triggers, and enqueues exactly one event-ledger message for the matched owner. — `apps/openagents.com/workers/api/src/agent-definition-webhook-routes.test.ts`
- **Oracle** `background_agents.inbox.slack_event_ledger_private_rows` (bun-test, unit): The event ledger accepts Slack source rows through the widened D1 source contract, preserves owner-scoped ordering, fixes training consent false, and stores refs plus bounded summaries rather than raw message text. — `apps/openagents.com/workers/api/src/event-ledger.test.ts`
- **Verification:** BA-H3 is enforced by agent-runtime-schema Slack normalization tests plus openagents.com Worker Slack webhook route and event-ledger store tests in the normal bun test sweep.
- **Authority boundary:** This contract binds private Slack event-ledger ingest for matched owner-scoped background-agent Slack triggers. It does not authorize outbound Slack writes, public projection, training use, cross-owner reads, raw message disclosure, or unredacted model context.

### `background_agents.definitions.harness_swap.v1` — PENDING

- **Surface:** pylon-worker (background agent definitions)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** One unchanged background-agent definition runs on Codex and Claude; harness is a field, never load-bearing.
- **Enforcement tier:** unenforced
- **Verification:** Pending BA-A4: add the parity fixture proving one unchanged definition runs on both harnesses, then flip this contract to enforced with that fixture as the oracle.
- **Blockers:** `blocker.background_agents.ba_a4.oracle_not_landed`
- **Authority boundary:** This contract binds harness portability for unchanged background-agent definitions. It does not claim semantic parity between all provider outputs beyond the parity fixture's asserted behavior.

### `background_agents.agents_panel.run_status_indicators_truthful.v1` — PENDING

- **Surface:** khala-code-desktop (Khala Code Agents panel)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Agents panel run-status indicators must be truthful: an in-progress, queued, failed, complete, or blocked indicator means exactly that run state and nothing else.
- **Enforcement tier:** unenforced
- **Verification:** Pending BA-G4: write the indicator-truthfulness contract before the Agents panel ships, with DOM or source oracles in the Khala Code Desktop sweep.
- **Blockers:** `blocker.background_agents.ba_g4.oracle_not_landed`
- **Authority boundary:** This contract binds truthfulness of run-status indicators in the Khala Code Agents panel. It does not prescribe the final panel layout, copy, or visual treatment.

### `background_agents.warm_dispatch.honest_no_op_without_warm_path.v1` — PENDING

- **Surface:** khala-code-desktop (warm dispatch)
- **Stated by:** owner via issue_list on 2026-07-03
- **Statement:** Khala Code composer emits a typed, debounced, owner-scoped pre-materialize signal while a fleet/background run is being composed; honest no-op when the target lane has no warm path.
- **Enforcement tier:** unenforced
- **Verification:** Pending BA-E3: add debounce and gating tests for the composer pre-materialize signal, then flip this contract to enforced with those tests as bun-test oracles.
- **Blockers:** `blocker.background_agents.ba_e3.oracle_not_landed`
- **Authority boundary:** This contract binds the warm-on-intent pre-materialize signal path. It does not require every lane to implement a warm path.
