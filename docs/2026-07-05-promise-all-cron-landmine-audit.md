# 2026-07-05 — repo-wide `Promise.all` unrelated-task-coupling audit

Follow-up audit to the #8409 severe write-loss incident (root cause and fix
documented in `docs/khala-sync/RUNBOOK.md`, "2026-07-05 severe write-loss
incident — ROOT CAUSE FOUND AND FIXED (#8409)", commits `2c9ce44bcb` /
`e38570eaaf`). Cross-posted from epic
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282).

## The pattern being hunted

`Promise.all([...])` (or `Promise.all(items.map(...))`) rejects and abandons
visibility into every other array entry the instant ANY single entry
rejects — even though the other promises keep running to completion
server-side (JS doesn't cancel promises), the *caller* never learns their
outcome, and if the rejecting `Promise.all` is what a
`scheduled()`/`fetch()`/`alarm()` handler directly awaits (or is what gets
handed to `ctx.waitUntil`), the Cloudflare Workers runtime can tear down the
whole invocation, abandoning still-in-flight sibling work mid-execution.
That mechanism, applied to the main Worker's shared per-minute
`Promise.all([...~25 unrelated tasks])`, caused real cross-domain data loss
(#8409): a known-flaky tips-buffer invariant check and a D1-overload error in
an unrelated billing sweep intermittently killed a completely unrelated
Postgres mirror write.

This audit searched the rest of the monorepo for the same **shape**: a bare
`Promise.all` whose array holds independently-failable, unrelated (or only
loosely related) operations, with no per-item error isolation, such that one
failure silently swallows visibility into (or, worse, prevents/aborts) the
others.

**Important:** the already-shipped #8409 fix (`Promise.allSettled` at the
main `scheduled()` handler, `apps/openagents.com/workers/api/src/index.ts`
~line 15622) is the accepted emergency stopgap and is explicitly OUT OF
SCOPE here — it is not touched by this audit. Per owner direction, any *new*
fixes coming out of this audit should use Effect structured concurrency
(`Effect.all`/`Effect.forEach` with per-item error handling), not more raw
`Promise.allSettled`. **This audit is documentation only — no code was
changed.** A separate follow-up pass implements the fixes.

## Method

- Grepped the whole repo for `Promise.all(` (not `Promise.allSettled`),
  excluding `*.test.ts(x)`, `node_modules`, `dist`.
- For every call site: read the enclosing function and the definitions of
  whatever runs inside the array/map, to determine whether each item can
  actually reject, and if so what breaks when it does.
- Classified each site as either a **genuine landmine** (independently
  failable + unrelated + uncaught + real consequence) or **safe /
  correctly-coupled** (either a genuine all-or-nothing operation where
  partial failure should abort the whole thing, or every item already has
  its own internal try/catch so nothing can actually reject the outer
  `Promise.all`).
- Three lanes of work: (1) the main production Worker
  (`apps/openagents.com/workers/api/src`, ~55 non-test files, done directly
  in this pass), (2) Pylon + backend packages (delegated to a research
  sub-agent), (3) desktop/mobile/CLI client apps (delegated to a research
  sub-agent).

**Total sites examined across all three lanes: ~202** (≈90 call sites in the
main Worker, 56 in Pylon/backend packages, 56 in client apps).
**Genuine landmines found: 29** (8 in the main Worker, 9 in Pylon/backend
packages, 12 in client apps).

---

## Lane 1 — Main production Worker (`apps/openagents.com/workers/api/src`)

Reviewed directly, ~90 non-test call sites across ~55 files.

### Genuine landmines (8)

1. **`apps/openagents.com/workers/api/src/runtime.ts:140`**
   (`notifySyncScopesPromise`) fanned out via
   **`apps/openagents.com/workers/api/src/index.ts:6551`**
   (`enforceOutOfCreditsPolicy`).
   ```ts
   // runtime.ts:140
   await Promise.all(
     [...new Set(scopes)].map(scope => notifySyncScopePromise(namespace, scope)),
   )
   // index.ts:6551
   await Promise.all(
     canceledRuns.map(item => notifyAgentRunSyncScopes(env, item.run.id)),
   )
   ```
   `notifySyncScopePromise` does an uncaught Durable Object `.fetch()` per
   sync scope (personal workroom / team / agent-run / thread scopes — several
   independent DOs per run). `notifyAgentRunSyncScopes` itself does an
   uncaught D1 read (`readAgentRunSyncScopes`) before that. **Real
   consequence:** in `enforceOutOfCreditsPolicy`, this fan-out over all of a
   user's just-canceled runs sits *before* the SHC (Sandboxed Host Compute)
   cleanup dispatch and the out-of-credits email send. If even ONE canceled
   run's sync-scope notify throws (flaky DO fetch, D1 hiccup), the function
   throws right there and **both of the following are silently skipped for
   ALL of that user's canceled runs**: `cleanupCanceledAgentRunOnShc` (which
   actually stops the runaway compute — meaning the user keeps burning
   compute after DB says billing-suspended) and
   `sendOutOfCreditsNotificationOnce` (the user never gets told they're out
   of credits). This is arguably a more severe real-world consequence than
   #8409's own trigger, since it can block an actual billing-enforcement
   action, not just an observability side effect. Fix candidate: isolate the
   sync-notify fan-out from the SHC-cleanup/email dispatch (they should not
   share fate), and isolate per-run/per-scope notify failures with logging.

2. **`apps/openagents.com/workers/api/src/relay-health.ts:642`**
   (`runRelayHealthProbeTick`).
   ```ts
   await Promise.all([
     input.store.pruneProbesBefore(...),
     input.store.pruneTransitionsBefore(...),
   ])
   ```
   Two independent retention-prune D1 writes on two different tables
   (`relay_health_probes`, `relay_health_transitions`), both raw `.run()`
   calls with no internal catch. One failing rejects the whole tick function
   with no indication of which table's prune succeeded. Low severity: this
   tick is one of the ~24 tasks now correctly isolated by the fixed outer
   `Promise.allSettled` in `index.ts`, and pruning is self-healing (retried
   next minute) — but it is still a real, uncaught, two-independent-writes
   coupling with zero per-item logging.

3. **`apps/openagents.com/workers/api/src/pylon-capacity-funnel-live-routes.ts:829` and `:832`**
   (`recordPylonCapacityFunnelSnapshots`, wired into the scheduled handler at
   `index.ts:15685`).
   ```ts
   await Promise.all(snapshots.map(snapshot => input.snapshotStore.upsertSnapshot(snapshot)))
   await Promise.all((['hourly', 'daily'] as const).map(bucketKind => input.snapshotStore.pruneSnapshotsBefore({...})))
   ```
   Same shape as #2: raw D1 `.run()` writes for the `hourly` and `daily`
   buckets, no per-item catch. One bucket's write/prune failure masks the
   other's outcome. Same low-severity/self-healing profile, same
   already-isolated-by-the-outer-fix blast radius.

4. **`apps/openagents.com/workers/api/src/treasury-routes.ts:421`**
   (`reconcilePendingTreasuryTransactions`, wired into the scheduled handler
   at `index.ts:16043`).
   ```ts
   const results = await Promise.all(
     records.map(record => reconcileTreasuryTransactionRecord(dependencies, record)),
   )
   ```
   `reconcileTreasuryTransactionRecord` does uncaught network calls
   (`readTreasuryPaymentStatus`) and uncaught D1 writes
   (`transactionStore.settle`/`.fail`) per pending outbound treasury
   transaction. Nuance: because JS doesn't cancel promises, each individual
   record's `.settle()`/`.fail()` DOES still get applied even if a sibling
   record's reconcile throws — so this is NOT settlement-state data loss.
   What IS lost is the batch-level reporting (`blocked`/`checked`/`failed`
   counts) for that cron tick, and the whole
   `reconcilePendingTreasuryTransactions` call surfaces as one generic
   failure in the (already-fixed) outer allSettled log rather than a
   detailed per-transaction outcome. In a financially-sensitive domain
   (treasury payout reconciliation), losing the granular per-cycle
   visibility is worth closing even though money isn't actually lost.

5. **`apps/openagents.com/workers/api/src/operator-provider-account-routes.ts:2265` / `:2268`**
   (`mapWithConcurrency` + `runSanityCheckForAccount`, operator bulk
   provider-account sanity-check endpoint).
   ```ts
   const mapWithConcurrency = async (values, concurrency, mapper) => {
     const results = []
     for (let index = 0; index < values.length; index += concurrency) {
       results.push(...(await Promise.all(values.slice(index, index + concurrency).map(mapper))))
     }
     return results
   }
   ```
   `runSanityCheckForAccount` does uncaught D1 writes
   (`recordProviderAccountHealth`, `recordSanityCheck`) and network probes
   per provider account, with no wrapping try/catch. **Real consequence:**
   this backs an operator-triggered "run sanity check across N provider
   accounts" bulk action. If any account in a chunk throws, that chunk's
   `Promise.all` rejects, the `for` loop's `await` throws out of
   `mapWithConcurrency` entirely, and **every chunk not yet reached never
   runs at all** — the operator gets a generic 500 with no indication which
   accounts were checked, which weren't reached, and results for
   already-succeeded accounts in earlier chunks are lost from the response.
   This is a real operational blind spot for a fleet-wide health-check
   utility.

6. **`apps/openagents.com/workers/api/src/forge-control-plane-routes.ts:1431`**
   (Forge-to-GitHub promotion mirror endpoint).
   ```ts
   const mirrorReceipts = await Promise.all(
     promotions.map(promotion => mirrorPromotionToGitHub(dependencies, env, promotion)),
   )
   ```
   `mirrorPromotionToGitHub` does uncaught D1 reads/writes and (further down,
   not shown) a GitHub API call per promotion, no wrapping try/catch. One
   promotion's mirror failing (e.g. transient GitHub API error) discards the
   whole `mirrorReceipts` array — including successful mirror results for
   every OTHER promotion in the same batch — and the endpoint's
   `mirroredCount`/`refusedCount`/`failedCount` summary never gets computed
   or returned to the operator.

7. **`apps/openagents.com/workers/api/src/agent-definition-run-routes.ts:1157` and `:1719`**
   (`revokeAgentDefinitionRunForgeGitTokensForAssignment` and its sibling for
   a completed run).
   ```ts
   await Promise.all(
     run.forgeGitTokenRefs.map(tokenRef =>
       dependencies.forgeGitAuthStore.revokeGitAccessToken(run.forgeTenantRef, tokenRef, input.nowIso),
     ),
   )
   if (run.forgeGitTokenRefs.length > 0) {
     await dependencies.runStore.upsertRun({ ...run, evidenceRefs: [...,'evidence.agent_definition_run.forge_git_tokens_revoked'], ... })
   }
   ```
   `revokeGitAccessToken` (`forge-domain-store.ts:980`) does an uncaught D1
   write plus a dual-write mirror call, per Forge git access token. Security-
   sensitive cleanup: if revoking token B throws while token A and C
   succeeded, the follow-up `upsertRun` that records the
   `forge_git_tokens_revoked` evidence ref **never runs for any of them** —
   so a run whose tokens were (mostly) actually revoked never gets the audit
   trail entry saying so, and a naive retry could attempt to re-revoke
   already-revoked tokens.

8. **`apps/openagents.com/workers/api/src/tassadar-settled-feed-sync.ts:268`**
   (`publishSettledFeedEvents`).
   ```ts
   await Promise.all(
     safeEvents.map(event => store.appendChange({ ...,  id: event.eventRef, op: 'put', scope, value: event })),
   )
   ```
   Batch-publishes independent real settlement events to the public
   settled-feed sync outbox (D1). One event's `appendChange` throwing (D1
   write failure) rejects the whole publish call with no per-event
   try/catch or logging, and the caller has no way to know which of
   potentially several unrelated settlement events in the batch actually
   got published vs silently dropped. This is exactly the "webhook/event
   batch processing of independent items" shape called out as a specific
   risk area.

### Reviewed and correctly left alone — safe / correctly-coupled (main Worker)

- **Already-fixed incident site** — `index.ts` `scheduled:` handler (~line
  15622), `Promise.allSettled` + per-task `.entries()` logging loop. Left
  untouched per instructions.
- **Every-item-already-caught pattern** (the dominant safe pattern in this
  codebase — confirmed repeatedly): `index.ts:6294`
  (`sendPendingReviewReadySiteNotifications`), `index.ts:6538`
  (`sendPendingReviewReadyArtifactNotifications`), `index.ts:6588`
  (`cleanupCanceledAgentRunOnShc` fan-out — internal try/catch),
  `index.ts:7649` (`makeTokensServedProjectionObserver`, both
  "BestEffort"-suffixed functions individually try/catch internally,
  confirmed in `khala-sync-public-tokens-served.ts:134` and
  `khala-sync-public-tokens-served-mix.ts:164`),
  `index.ts:15264`/`broadcastScope` (`replaySocket` wraps its whole body in
  try/catch and never rethrows), `artanis-operator-pylon-job-status.ts:368`
  (per-pylon-ref `.catch(() => [])`), `agent-registration.ts:818`
  (`mirrorProfileAndCredential` — both calls route through the
  `guarded()`-wrapped `mirrorRowsByPk`, confirmed non-throwing in
  `agent-runtime-remainder-store.ts:400`), `relay-health.ts:373`
  (`executeRelayHealthProbe`'s two probes — both `probeRelayNip11` and
  `probeRelayWebsocketEose` are internally caught, return typed
  outcome objects, never throw), `artanis-situational-awareness.ts:333`
  (every entry wrapped in the local `safeRead()` helper),
  `artanis-token-pace.ts:432` (every entry wrapped in the local
  `safeJson()` helper).
- **Genuinely coupled, all-or-nothing, single unit of work** (correct to
  fail together): `index.ts:3730` (GitHub OAuth — need both `user` and
  `emails` to build one auth subject), `cs336-a4-crawl-shard-plan.ts:251` /
  `cs336-a4-crawl-shard-assignment.ts:243` (all shards needed to form one
  valid crawl plan), `forge-git-canonical-store.ts:1113`/`:1118` and the
  Postgres-store twin (all refs/objects for one incoming git push's intake
  result), `agent-registration.ts:365` (`timingSafeEqual` — trivial, no
  real failure mode), `artanis-fleet-overseer-tick.ts:627` (both reads
  needed together to form one coherent tick decision).
- **Single-request read-aggregation for one response/page/report** — a very
  common pattern in this codebase (build one JSON response or page context
  out of N independent D1/Postgres reads). Reviewed and judged a
  **different, lower-severity risk class** than the cron/background
  landmine this audit targets: failure here means one HTTP request 500s
  (retry-safe, no cross-domain data loss, no unrelated background job
  killed) rather than silently abandoning concurrently-running unrelated
  work. Left alone as out-of-scope for this pass (a legitimate future
  resilience improvement, not the #8409 bug class): `index.ts:4041`
  (`readAuthenticatedPageContext`), `billing.ts:698`/`:904`/`:1204`,
  `agent-scoped-grant-routes.ts:504`, `agent-home-routes.ts:166`,
  `khala-trace-review-routes.ts:318`, `site-referral-inspection.ts:567`,
  `prefilled-workspace.ts:421`, `http/forum-social-preview.ts:349`,
  `public-tassadar-run-summary-routes.ts:351`/`:422`,
  `public-activity-timeline.ts:584`/`:1015`,
  `hosted-gemini-promise-readiness-routes.ts:60`,
  `github-write-connections.ts:1007`, `provider-account-service.ts:74`,
  `provider-account-usage-routes.ts:241`,
  `provider-account-pool-routes.ts:418`,
  `operator-provider-account-routes.ts:1310`,
  `operator-pro-status-routes.ts:387`,
  `x-claim-reward-eligibility-routes.ts:135`,
  `forge-control-plane-routes.ts:502`, `crm-email-domain-store.ts:161`,
  `token-usage.ts:189`, `team-repository.ts:117`, `billing-store.ts:418`,
  `omni-handlers.ts:801`/`:1279`/`:1393`, `omni-runs.ts:1581`/`:1599`,
  `khala-mcp.ts:703`/`:1276`/`:1282`, `pylon-api-routes.ts:2349`/`:2361`,
  `public-pylon-stats.ts:794`, `email-sequence-authoring.ts:171`/`:212`/`:360`
  and `email-onboarding-drip.ts:130`/`:210` (batch email-campaign-step
  inserts for one campaign definition — same reasoning: single admin/signup
  request building one coherent campaign object, not a background sweep).
- **`auth/email-otp-hardening.ts:149`/`:173`** — rate-limit bucket
  read/write fan-out for one OTP request. Noted as a security nuance (a
  partial bucket-write failure could under-count one rate-limit scope) but
  judged out of the primary landmine class (single-request, not a
  background job killing unrelated domains); flagged for awareness only.
- **`public-pylon-stats.ts:955`/`:957`** — payout-reconciliation-receipt
  read fan-out (nested `Promise.all` per receipt). Read-only reporting
  aggregation; same reasoning as the general read-aggregation bucket above,
  called out separately here because it sits in the financially-sensitive
  payout-reconciliation surface — worth a resilience pass later, not
  flagged as a #8409-class landmine since no data is lost, only visibility
  into an operator report.

---

## Lane 2 — Pylon + backend packages (sub-agent research pass)

Directories: `apps/pylon/src`, `apps/pylon/scripts`,
`apps/pylon/packages/runtime/src/benchmark`, `packages/khala-tools/src`,
`packages/khala-sync-server/src` + `scripts`, `packages/tassadar-executor/src`,
`packages/probe/packages/runtime/src/benchmark`, `packages/khala-qa-harness/src`,
`packages/ai-sdk-sandbox-local/src`, `packages/agent-readiness/src`.
**56 sites examined, 9 landmines, 47 safe.**

### Genuine landmines (9)

1. **`apps/pylon/src/coordinator/coordinator-runtime.ts:124`** — highest
   priority in this lane. `Promise.all(refs.map(ref => deps.sessionState(ref)))`
   inside `reconcile(intentId)`; `deps.sessionState` calls `sessions.list()`
   with no catch. `reconcile()` is called from a `for...of` loop in `tick()`
   (lines 167–184) iterating **all queued intents** each cycle, driven by
   `setInterval(() => void tick(), intervalMs)` — a genuine background
   polling loop. One flaky session-state read on intent A aborts
   dispatch/reconcile for every other unrelated intent still in that tick
   (delay via retry-next-tick, not permanent loss, but structurally the
   closest match in the repo to the #8409 shape). Fix candidate:
   `.catch(() => null)` per `sessionState` call (matching the existing
   "not yet observable" null-handling elsewhere), or isolate at the tick
   level.
2. **`packages/agent-readiness/src/index.ts:1558`** — worker-pool over
   independent domains; `scanAgentReadinessDomain` catches network/parse
   errors internally, but its tail (findings/score computation, final
   schema decode) is uncaught — a throw there for one domain discards
   already-computed reports for every other domain. Currently only exercised
   by tests, no live caller yet — a landmine waiting for its first
   production caller.
3. **`packages/tassadar-executor/src/linked-dense-module-runtime.ts:617`** —
   per-"bank" conformance verdicts; the execution call is try/caught but
   subsequent verdict-construction steps are not, so one bank's crash
   discards verdicts for all banks. Foreground benchmark/conformance
   runner, low severity, undermines report completeness.
4. **`apps/pylon/scripts/concurrent-checkout-proof.ts:151`** — fans out N
   (default 12) independent OS-process workers with no per-worker
   try/catch; a harness-level throw aborts visibility into the other
   in-flight workers, undermining the script's own stated goal of
   asserting zero checkout failures across all workers. Foreground CLI
   proof script.
5. **`apps/pylon/scripts/multi-session-run.ts:813`** — mostly safe
   (`runOneSession` wraps almost its whole body in try/catch and always
   returns an outcome object), but the very first `appendHeartbeat(...)`
   call per session runs before that try block and is uncaught; a
   disk-full/permission error there aborts the whole worker loop.
6. **`apps/pylon/src/orchestration/work-planner.ts:415`** —
   `Promise.all([gh(issueArgs), githubPullRequestCandidates(source, gh)])`,
   two independent uncaught `gh` CLI calls; one failing discards the
   other's already-fetched results. Currently dead code (no production
   caller confirmed) — worth fixing before it's wired into a live backlog
   loop.
7. **`apps/pylon/src/provider-nip90.ts:1154`** — `Promise.all(transports.map(t => t.close?.()))`
   in a `finally` shutdown block; one rejecting close hides whether
   siblings closed cleanly. Foreground/one-time shutdown, minor/cosmetic.
8. **`packages/khala-sync-server/scripts/backfill-agent-runtime-remainder.ts:269`** —
   independent D1-vs-Postgres scalar tallies per table, uncaught; one bad
   tally crashes the whole verify run for that table. Manually-invoked CLI
   script, blast radius is "operator reruns it."
9. **`packages/khala-qa-harness/src/lag-profiling-sweep.ts:556`** — loads N
   independent snapshot files; one bad/missing file kills the whole sweep
   with no report even for valid files. Possibly run unattended in CI.

### Safe (47)

Atomic subprocess I/O triples (stdout/stderr/exit-code of one spawned
process — genuinely all-or-nothing), intentionally fail-fast atomic
construction (paired imports, migration file sets, deny-list resolution),
and every-item-already-caught patterns across: `apps/pylon/src/wallet.ts`,
`dev-loop.ts`, `fleet-run-live-smoke.ts`, `khala-requester.ts` (flagged as a
minor foreground UX-polish opportunity, not a landmine),
`workspace-materializer.ts`, `dev-doctor.ts`, `codex-pr-publisher.ts`,
`labor.ts`, `codex-agent-executor.ts`, `assignment.ts`, `khala-spawn.ts`,
`forge-verification-runner.ts`, `account-connect.ts`, `labor-market.ts`,
`claude-agent-executor.ts`, `auth.ts`, `index.ts`,
`node/sessions-batch.ts`; `apps/pylon/scripts/recover-assignment-prs.ts`,
`nip90-provider-serve.ts`, `packaged-runtime-task-smoke.ts`,
`concurrent-checkout-proof.ts` (its other two sites),
`multi-session-run.ts` (its other site), `packaged-live-network-smoke.ts`;
`apps/pylon/packages/runtime/src/benchmark/closeout-writer.ts`;
`packages/khala-tools/src/glob.ts`, `process-sandbox-macos.ts` (deny-list
resolution is fail-closed by design — security-critical, correct as-is),
`index.ts`, `session-rollout.ts`, `redaction.ts`;
`packages/khala-sync-server/src/migrate.ts` (deploy-time CLI, correct
fail-fast), `load-test.ts`; `packages/probe/packages/runtime/src/benchmark/closeout-writer.ts`;
`packages/khala-qa-harness/src/lag-profiling-sweep.ts` (its other site);
`packages/ai-sdk-sandbox-local/src/index.ts`.

---

## Lane 3 — Desktop/mobile/CLI client apps (sub-agent research pass)

Directories: `clients/khala-code-desktop/src/bun` + `/ui` + `/scripts`,
`clients/khala-cli/src`, `clients/khala-mobile/src/auth` + `/native`,
`apps/autopilot-desktop/src/bun` + `/scripts`, `apps/qa-runner/src`,
`apps/oa-updates/src` + `/scripts`. **56 sites examined, 12 landmines, 44
safe.**

### Genuine landmines (12)

1. **`clients/khala-code-desktop/src/bun/codex-token-usage-telemetry.ts:665`** —
   three independent ledger-file reads feeding a token-usage summary polled
   every 2s during streaming turns; `readJsonLines` swallows `ENOENT` but
   rethrows other errors. One bad file blanks the whole on-screen counter
   with no indication which file failed.
2. **`clients/khala-code-desktop/src/bun/fleet-run-supervisor.ts:618`** —
   fan-out over independent fleet accounts/work units; only the
   `runner.dispatch(...)` call is try/catch'd per item, post-dispatch
   bookkeeping (`recordWorkerDone`, `updateWorkClaimState`, `onLifecycle`
   emit) is not. Production's live loop currently calls with
   `awaitDispatches: false` (fire-and-forget with a silent
   `.catch(() => undefined)` — itself a separate, zero-logging landmine),
   but the awaited branch is exported/reachable too.
3. **`clients/khala-code-desktop/src/bun/khala-fleet-tools.ts:3350`** —
   multi-account fan-out with no per-item catch; currently safe only
   because the command runner never actually rejects today — fragile, not
   currently broken. A future runner change (or custom runner) would drop
   every other account's spawn result silently.
4. **`clients/khala-code-desktop/src/ui/main.ts:2221, 3183, 3243`**
   (`steerFollowUpDraft`, `handleDiffReviewSubmit`,
   `handleSourceControlActionSubmit`) — each maps an independent RPC over
   potentially multiple concurrent active turn ids; one turn's rejection
   throws before the per-turn `results.find(...)` reporting logic runs,
   collapsing success/failure into one opaque generic error.
5. **`clients/khala-code-desktop/src/ui/main.ts:3353`**
   (`imageAttachmentsForSubmit`) — runs before `submitComposer`'s own try
   block, and `submitComposer()` is invoked bare (`void submitComposer()`,
   no `.catch`). One stale/revoked attachment URL throwing is an unhandled
   rejection with zero user feedback.
6. **`clients/khala-code-desktop/src/ui/codex-settings-panel.ts:695`**
   (`refreshSettings`) — three independent IPC fetches; one rejecting
   hides a successful settings fetch behind a generic error banner with
   stale data. Lower severity (foreground, user-watched, no data loss).
7. **`clients/khala-cli/src/fleet-run.ts:377, 392, 420`**
   (`runSupervisorLoop` round/replenishment dispatch, `runOneRound`) —
   `dispatchFleetSlot` intentionally throws a `NEEDS-OWNER` reauth error for
   one locked-out account, and unexpected spawn errors propagate too. Lines
   377/392 sit inside an unguarded `for (;;)` supervisor loop — one bad
   account's failure kills the **entire perpetual supervisor process**,
   aborting every other account's dispatch for the rest of the run. This is
   the closest structural analog to the #8409 incident found in the client
   lane.
8. **`clients/khala-mobile/src/native/modules.ts:10`**
   (`readNativeReadiness`) — two unrelated native capability checks; native
   bodies never throw today, but the Expo JS/native bridge can reject
   independently (a real, explicitly-supported failure mode when a native
   binary is out of sync with an OTA-updated bundle). One bridge hiccup
   blanks visibility into the other unrelated capability.

### Safe (44)

Every-item-already-caught patterns, atomic subprocess I/O for one spawned
process, and legitimately-coupled single objects (e.g. one paired credential
split across two SecureStore keys). Notably, `apps/oa-updates` (the
update-checker/publisher) and all of `apps/autopilot-desktop/src/bun/pylon-control.ts`
(which manages multiple independent pylon/fleet accounts) came back entirely
clean — that code already follows the self-guarding-helper pattern this
whole audit is chasing. Full file-by-file list: `khala-fleet-tools.ts:989/996/2069/2286/2418`,
`rpc-handlers.ts:658/685/1697/1768/2867`, `session-catalog.ts:321`,
`claude-app-sdk-chat-runtime.ts:437`, `main.ts:3427`, `plans-panel.ts:419`,
`forum-panel.ts:313/329/347`, `fleet-status.ts:1936`,
`composer-visual-smoke.ts:289`, `live-two-codex-readonly-smoke.ts:107/123`,
`fleet-run.ts:509` (khala-cli), `spawn.ts:260`, `fleet.ts:439/583`,
`khala-mobile-pairing-core.ts:117`, `khala-auth-store.ts:18/27/34`,
`promise-surfacing.ts:189`, `verse-turn.ts:275`,
`pylon-control.ts:216/295/865/1150/1425`, `training-runs.ts:1386/1643`,
`swarm-view-proof.ts:146`, `qa-runner/shard.ts:65` (verified — the
already-correct bounded worker-pool pattern this whole audit is chasing
toward), `qa-runner/control.ts:535`, `oa-updates/publish.ts:39`,
`export-reader.ts:68`, `publish-desktop-release.ts:166`.

---

## Grand totals

| Lane | Sites examined | Landmines | Safe |
|---|---|---|---|
| Main production Worker | ~90 | 8 | ~82 |
| Pylon + backend packages | 56 | 9 | 47 |
| Desktop/mobile/CLI clients | 56 | 12 | 44 |
| **Total** | **~202** | **29** | **~173** |

(Plus the one already-fixed #8409 incident site, left untouched.)

## Priority ranking for the follow-up Effect-based fix pass

**Tier 1 — real production consequence, fix first:**
- `index.ts:6551` / `runtime.ts:140` (blocks SHC compute cleanup + billing
  email on an unrelated sync-notify failure)
- `clients/khala-cli/src/fleet-run.ts:377/392/420` (one account kills the
  entire perpetual fleet supervisor process)
- `apps/pylon/src/coordinator/coordinator-runtime.ts:124` (background
  polling loop, closest structural match to #8409)
- `operator-provider-account-routes.ts:2265/2268` (bulk operator action
  silently drops un-run chunks)
- `forge-control-plane-routes.ts:1431` (GitHub mirror receipts lost for
  unrelated promotions)
- `tassadar-settled-feed-sync.ts:268` (public settlement-feed events
  silently dropped)

**Tier 2 — real but lower severity / self-healing / narrow blast radius:**
- `relay-health.ts:642`, `pylon-capacity-funnel-live-routes.ts:829/832`
  (nested inside the already-isolated outer cron fix; self-healing next
  cycle)
- `treasury-routes.ts:421` (money itself isn't lost, only per-cycle
  reconciliation visibility)
- `agent-definition-run-routes.ts:1157/1719` (security audit-trail gap)
- `packages/agent-readiness/src/index.ts:1558`,
  `packages/tassadar-executor/src/linked-dense-module-runtime.ts:617`
  (report-completeness gaps, foreground/no live caller yet)
- `clients/khala-code-desktop/src/bun/fleet-run-supervisor.ts:618`,
  `khala-fleet-tools.ts:3350`, and the `main.ts`/`codex-settings-panel.ts`
  UI clusters (UX/observability degradation, not data loss)
- `clients/khala-mobile/src/native/modules.ts:10`

**Tier 3 — cosmetic / CLI-script / foreground-only, fix opportunistically:**
- everything else listed under "genuine landmines" in Lanes 2 and 3 above
  (concurrent-checkout-proof.ts, multi-session-run.ts, work-planner.ts,
  provider-nip90.ts, backfill-agent-runtime-remainder.ts,
  lag-profiling-sweep.ts)

## What NOT to "fix"

Do not touch the already-shipped #8409 `Promise.allSettled` fix in
`index.ts`'s `scheduled:` handler — it's the accepted emergency stopgap.
Do not convert the large "single-request read-aggregation" bucket
(dashboards/page-context loaders building one response out of N independent
reads) to per-item isolation as part of this specific audit's fix pass —
that's a legitimate but separate resilience improvement with a different
(much lower) severity profile than the cron/background-job data-loss class
this audit targets. Do not touch the "genuinely coupled, all-or-nothing"
sites listed above (GitHub OAuth user+email, crawl shard plans, git
push intake, fleet-overseer tick context) — partial failure there SHOULD
abort the whole operation.
