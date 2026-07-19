---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.full.auto.codex.composer.loop"
assurance_revision: 5
title: "Full Auto Autonomous-Run Assurance Spec"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "OpenAgents"
---

## Assurance Objective

This revision reconciles the Full Auto AssuranceSpec to ProductSpec rev 14, including MemoHarness's released bundle identity, frozen pre-run adaptation, terminal-only experience compilation, independent Blueprint release gate, and Effect/Rust ownership boundary. It preserves rev 3's complete FA-AC-01..68 subject set and adds one explicit obligation for each new FA-AC-69..76 criterion. It establishes what confidence this document is designed to support and what it explicitly does not yet establish.

**What this revision DOES establish:** a real risk model (Risk Model section), extended with MemoHarness privacy, evaluation-leakage, self-promotion, authority-expansion, and provenance-drift risks; explicit local/dev/packaged/owner-real environment profiles with honest capability gaps (Environments section); a criterion-to-obligation map covering every one of the 76 FA-AC-\* criteria with no criterion silently dropped from the subject binding (Obligations section, `uncovered_acceptance_criterion` is structurally impossible here); and retention of the 76 DESIGNED obligations with their exact existing evidence tier -- not a fabricated claim that old Full Auto tests prove new learning behavior.

**What this revision explicitly does NOT establish:** (1) Admission. `lifecycle_state` stays `proposed`; no producer may self-admit (Law 10, and #8978's own text: "the implementation/analyzer cannot self-admit"). (2) Execution completeness. All 76 criteria now have complete proof designs, but a designed oracle is not a passing observation. The Observation axis remains unmet wherever no source-bound receipt is named. (3) Release or public-claim authority. Design readiness and the owner-real development receipt do not authorize a release or public claim. (4) A signed/notarized packaged-build proof for restart/resume (FA-AC-07/08/29). The two-OS-process smoke can fall back to unsigned dev-mode Electron; this revision does not upgrade that evidence to release-artifact proof. (5) Any MemoHarness optimizer, experience bank, adapted run, held-out evaluation, Blueprint promotion, privacy deletion, or authority-immunity observation. FA-AC-69..76 now have explicit proof plans, but their referenced production/evidence seam does not yet exist and their observations remain `INCONCLUSIVE`. (6) Independent admission of the owner-real receipt. `docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json` proves the six named rows and same-pass rotation at source `3123d926a3` in an owner-real development profile; it is neither independently admitted by this producer nor evidence for a signed package.

## Subject

This revision binds the subject to the exact ProductSpec rev-14 bytes at `specs/desktop/full-auto.product-spec.md`. All 76 `FA-AC-01` through `FA-AC-76` criterion refs are bound below. The eight MemoHarness criteria receive exact, separately gated contract/model obligations and do not borrow evidence from the pre-MemoHarness run loop.

A future rebind to ProductSpec rev 15+ must repeat this same digest/criterion-set update; per ASSURANCE_SPEC.md Law/§13, any ProductSpec revision or intent change stales this AssuranceSpec until explicit reconciliation.

```assurancespec-subject
{
  "product_spec": {
    "criterion_refs": [
      "FA-AC-01",
      "FA-AC-02",
      "FA-AC-03",
      "FA-AC-04",
      "FA-AC-05",
      "FA-AC-06",
      "FA-AC-07",
      "FA-AC-08",
      "FA-AC-09",
      "FA-AC-10",
      "FA-AC-11",
      "FA-AC-12",
      "FA-AC-13",
      "FA-AC-14",
      "FA-AC-15",
      "FA-AC-16",
      "FA-AC-17",
      "FA-AC-18",
      "FA-AC-19",
      "FA-AC-20",
      "FA-AC-21",
      "FA-AC-22",
      "FA-AC-23",
      "FA-AC-24",
      "FA-AC-25",
      "FA-AC-26",
      "FA-AC-27",
      "FA-AC-28",
      "FA-AC-29",
      "FA-AC-30",
      "FA-AC-31",
      "FA-AC-32",
      "FA-AC-33",
      "FA-AC-34",
      "FA-AC-35",
      "FA-AC-36",
      "FA-AC-37",
      "FA-AC-38",
      "FA-AC-39",
      "FA-AC-40",
      "FA-AC-41",
      "FA-AC-42",
      "FA-AC-43",
      "FA-AC-44",
      "FA-AC-45",
      "FA-AC-46",
      "FA-AC-47",
      "FA-AC-48",
      "FA-AC-49",
      "FA-AC-50",
      "FA-AC-51",
      "FA-AC-52",
      "FA-AC-53",
      "FA-AC-54",
      "FA-AC-55",
      "FA-AC-56",
      "FA-AC-57",
      "FA-AC-58",
      "FA-AC-59",
      "FA-AC-60",
      "FA-AC-61",
      "FA-AC-62",
      "FA-AC-63",
      "FA-AC-64",
      "FA-AC-65",
      "FA-AC-66",
      "FA-AC-67",
      "FA-AC-68",
      "FA-AC-69",
      "FA-AC-70",
      "FA-AC-71",
      "FA-AC-72",
      "FA-AC-73",
      "FA-AC-74",
      "FA-AC-75",
      "FA-AC-76"
    ],
    "document_digest": "sha256:1ec816bd58dce62b71060381188e2a82307d4e50baa3ba86ee2d0f8a827857ef",
    "path": "specs/desktop/full-auto.product-spec.md",
    "profile": "openagents_executable_v0.1_exact_document",
    "spec_format_version": "0.1",
    "spec_revision": 14
  }
}
```

## Risk Model

The bound ProductSpec has no `## Risks` section (confirmed by the mechanical proposal's `source_snapshot`), so no risk is inferred from ProductSpec prose (ASSURANCE_SPEC.md §1: an AssuranceSpec risk model is reviewer-authored, never auto-derived from product intent text). The 19 risks below are authored directly from (a) the nine highest-risk clusters #8978 itself names, (b) the concrete 2026-07-17 six-hour-stall incident (`docs/fable/2026-07-17-full-auto-implementation-audit.md`) that motivated ProductSpec rev 10, (c) root `INVARIANTS.md`'s Full Auto Authority Boundaries entry, and (d) ProductSpec rev 14's MemoHarness privacy, evaluation, promotion, authority, and provenance boundaries. Each risk below is referenced by at least one designed obligation's motivation; the `custom-evidence-tier-ledger` section maps risks to the obligations that address them where that mapping is not already obvious from the risk statement.

```assurancespec-risks
{
  "risks": [
    {
      "id": "RISK-FA-01",
      "statement": "Silent duplicate dispatch: overlapping reconciliation passes (startup, post-turn, control-triggered continue-now, and provider-rotation retry) could double-dispatch a continuation into the same thread absent the durable per-thread lease, wasting provider budget or producing divergent duplicate turns."
    },
    {
      "id": "RISK-FA-02",
      "statement": "Workspace/objective drift: a continuation could dispatch into the wrong repository, or lose its owner-authored objective/done-condition across a provider switch, restart, or truncated transcript, and silently pursue the wrong goal."
    },
    {
      "id": "RISK-FA-03",
      "statement": "Silent stall misclassified as healthy: thread/session-store eviction under multi-chat pressure (the exact 2026-07-17 incident) could leave a run appearing to be running normally for hours with no owner-visible diagnosis or actionable notice."
    },
    {
      "id": "RISK-FA-04",
      "statement": "Terminal-state corruption: a Stopped or otherwise terminal run could be resumed, or Pause/Stop could be applied out of the legal order, producing an owner-unrecoverable record or a run that keeps running when the owner believes it stopped."
    },
    {
      "id": "RISK-FA-05",
      "statement": "Late-provider write-after-fence: a stale in-flight turn from a provider lane the run has since switched away from, paused, or stopped could still land a write (commit, file change) after the run's authority moved on."
    },
    {
      "id": "RISK-FA-06",
      "statement": "Private-evidence leakage: the FullAutoRunReport or its public-safe projection could leak raw prompts, tool output, transcript text, workspace paths, or credentials into a public or lower-trust surface."
    },
    {
      "id": "RISK-FA-07",
      "statement": "Overclaimed provider support: the provider picker could offer a lane that is not actually admitted/authenticated/eligible, or product copy could imply that provider-private session state itself transfers on a switch when it does not."
    },
    {
      "id": "RISK-FA-08",
      "statement": "Guardrail bypass via configuration: the owner-configurable guardrail surface (maxWallClockMs, maxTurns, maxPerTurnFailures, tokenBudgetRef) could be structured so a config or environment value relaxes the NON-OVERRIDABLE core guardrails (workspace binding, own-capacity-only lane admission, no rate-limit-reset triggering)."
    },
    {
      "id": "RISK-FA-09",
      "statement": "Registry eviction drops an active run: the bounded 128-record registry eviction policy could drop a non-terminal (still-active) run record, silently ending an in-progress unattended run with no owner notice."
    },
    {
      "id": "RISK-FA-10",
      "statement": "Unsigned/dev-mode resume claimed as packaged proof: restart/resume evidence gathered from an unsigned local Forge package or the dev-mode Electron fallback could be reported as equivalent to a signed, notarized, distributable build's resume behavior."
    },
    {
      "id": "RISK-FA-11",
      "statement": "Fixture-tier dogfood evidence claimed as real-provider proof: the #8976 six-test headless fixture harness could be cited as satisfying the real-provider owner-dogfood acceptance bar it is explicitly designed NOT to satisfy (its own schema names a distinct fixture vs owner_real profileClass)."
    },
    {
      "id": "RISK-FA-12",
      "statement": "Legacy migration invents or drops intent: migrating a pre-rev-10 enabled:true per-thread registry row into a FullAutoRun could fabricate an objective the owner never authored, or silently drop a row that loses the one-active-run-per-profile race instead of preserving it as a Draft."
    },
    {
      "id": "RISK-FA-13",
      "statement": "Rotation/failure-budget miscounting: a typed rotation-eligible failure could be miscounted against the ordinary FA-H5 failure budget (or vice versa), causing a run to disable early or never disable when it should, or to rotate on a failure class that should never rotate (owner interrupts, model substitution)."
    },
    {
      "id": "RISK-FA-14",
      "statement": "Claimed commit evidence taken as verified: a report's claimed commit SHA (extracted from the local turn journal) could be presented as independently verified against real git state when the current implementation only extracts the claim and never re-verifies it."
    },
    {
      "id": "RISK-FA-15",
      "statement": "Private-experience leakage or cross-scope influence: adaptation, pattern extraction, a safe projection, or deletion handling could expose raw prompts/transcripts/tool output/embeddings/secrets or retrieve evidence outside the admitted tenant, workspace, visibility, consent, retention, and tombstone scope."
    },
    {
      "id": "RISK-FA-16",
      "statement": "Evaluation leakage and mid-run policy drift: a run could learn from its own labels or feedback, retrieve from a bank that changes during execution, or alter its effective harness between continuations, making the outcome irreproducible and the evaluation circular."
    },
    {
      "id": "RISK-FA-17",
      "statement": "Optimizer self-promotion: the component that produces a candidate could verify or activate it without independent held-out evidence and Blueprint review, converting an experimental policy into production through the same authority that benefits from the claim."
    },
    {
      "id": "RISK-FA-18",
      "statement": "Authority expansion through adaptation: a harness delta could change workspace, placement, provider/account admission, tools, approvals, guardrails, budgets, done condition, release authority, or external-effect permissions even though learning is not an action-authority grant."
    },
    {
      "id": "RISK-FA-19",
      "statement": "Provenance or compatibility drift: a report could omit or misstate the base/effective bundle, module versions, bank snapshot, evaluator/environment, cache state, or release decision, or silently reuse an adapted bundle after a provider/model/toolset change makes it incompatible."
    }
  ],
  "source_digest": "sha256:1084a9edb589ef44d32854a76620d88fe517c9cce4ceba15b537b96bf839849c",
  "source_snapshot": "The source ProductSpec contains no Risks section. Assurance risk modeling remains required."
}
```

## Assurance Scope

Every one of the 76 executable FA-AC-\* criteria from ProductSpec rev 14 is in assurance scope (see Subject). No criterion carries a `not_applicable` disposition in this revision -- rev 10's own Criterion Disposition Map already resolved FA-AC-01..37 (`changed-superseded`, `retained`, or `deferred`) and none of those dispositions map to "does not need assurance"; a superseded criterion's assurance obligation is superseded by its replacement criterion's obligation, not dropped. In scope: local unit/module proof (Vite Plus `vp test`), in-process HTTP control-server contract proof, two-OS-process dev-mode Electron smoke proof, the landed owner-real development receipt for #8976's six rows, and future MemoHarness unit/contract/privacy/evaluation/release/architecture evidence explicitly required by FA-AC-69..76. Out of scope for THIS revision (named explicitly rather than silently omitted): a genuinely signed/notarized packaged-build resume proof (no signing runbook was executed in this pass); a new TLA+ formal model (the repository has none for Full Auto -- see Formal-model note below and the `custom-evidence-tier-ledger`); independent re-verification of claimed commit SHAs in the FullAutoRunReport against live git state; independent admission of the owner-real receipt; and any execution claim for the not-yet-implemented MemoHarness experience, retrieval, adaptation, optimization, deletion, or Blueprint release seams.

## Environments

Four Environment Profiles are declared below, matching the real evidence tiers this codebase actually produces today (no profile claims a capability the repository does not demonstrate). `ENV-FA-LOCAL-UNIT-1` covers the overwhelming majority of cited evidence: real production module composition (real `full-auto-registry.ts`, `full-auto-run-registry.ts`, `full-auto-reconcile.ts`, `thread-store.ts`, `local-turn-journal.ts`, and real in-process Effect HTTP control servers), in-process, with no live Electron process and no live provider. `ENV-FA-DEV-TWO-PROCESS-1` covers `pnpm run smoke:full-auto-restart` / `smoke:full-auto-control`: two real OS processes launching a real Electron app, but conditionally falling back to unsigned dev-mode `electron .` when no local Forge package exists. `ENV-FA-PACKAGED-UNSIGNED-1` names the conditional Forge-packaged path the smoke script prefers when `out/OpenAgents-darwin-arm64/OpenAgents.app` exists locally; it is marked `signing_unverified`, so it is not release-grade. `ENV-FA-OWNER-REAL-SIDEBAR-1` now has exact development-tier evidence in `docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json`: source `3123d926a3`, owner-real macOS arm64 development build, real Codex app-server and Claude Agent SDK lanes, all six named rows PASS, and one same-pass rotation PASS. The profile remains `status: "proposed"` because this AssuranceSpec has not been independently admitted; proposed status no longer means the evidence is absent.

```assurancespec-environments
{
  "profiles": [
    {
      "id": "ENV-FA-LOCAL-UNIT-1",
      "status": "proposed"
    },
    {
      "id": "ENV-FA-DEV-TWO-PROCESS-1",
      "status": "proposed"
    },
    {
      "id": "ENV-FA-PACKAGED-UNSIGNED-1",
      "status": "proposed"
    },
    {
      "id": "ENV-FA-OWNER-REAL-SIDEBAR-1",
      "status": "proposed"
    }
  ],
  "repository_inventory": {
    "state": "clean",
    "repository_label": "work",
    "head": "78f2230e2c1578cc70b36ee426e5287b4b2b105c",
    "tree": "0f1f86880586f6eca17c34d9cc292582691b800f",
    "tracked_file_count": 9814,
    "candidate_artifact_refs": [
      "apps/acceptance-runner/src/daemon.test.ts",
      "apps/acceptance-runner/src/e2e-local-proof.test.ts",
      "apps/aiur/src/auth/access-route.test.ts",
      "apps/aiur/src/auth/access.test.ts",
      "apps/aiur/src/auth/owner-gate.test.ts",
      "apps/aiur/src/auth/routes.test.ts",
      "apps/aiur/src/cloudrun/server.test.ts",
      "apps/aiur/src/cloudrun/static.test.ts",
      "apps/aiur/src/dashboard/tokens-served-sync.test.ts",
      "apps/aiur/src/effect-native-theme.test.ts",
      "apps/aiur/src/khala-sync-proxy.test.ts",
      "apps/aiur/src/lib/relative-time-core.test.ts",
      "apps/aiur/src/ops/crm-batch-api-client.test.ts",
      "apps/aiur/src/ops/crm-batch-console.test.tsx",
      "apps/aiur/src/ops/crm-batch-selection.test.ts",
      "apps/aiur/src/ops/ops-api-client.test.ts",
      "apps/aiur/src/ops/ops-console.test.tsx",
      "apps/aiur/src/server.test.ts",
      "apps/aiur/vitest.config.ts",
      "apps/forum/src/index.test.ts",
      "apps/khala-capture/src/deploy-contract.test.ts",
      "apps/khala-live-hub/src/credit-balance-live-delivery.test.ts",
      "apps/khala-live-hub/src/scope-hub.test.ts",
      "apps/khala-live-hub/src/server.test.ts",
      "apps/khala-live-hub/src/service.test.ts",
      "apps/oa-queue-worker/src/pump.test.ts",
      "apps/oa-updates/src/asset-store.test.ts",
      "apps/oa-updates/src/code-signing.test.ts",
      "apps/oa-updates/src/deploy-cloudrun.test.ts",
      "apps/oa-updates/src/desktop-release.test.ts",
      "apps/oa-updates/src/desktop-seed.test.ts",
      "apps/oa-updates/src/desktop-staging-feed-e2e.test.ts",
      "apps/oa-updates/src/export-reader.test.ts",
      "apps/oa-updates/src/legacy-desktop-lockout.test.ts",
      "apps/oa-updates/src/manifest-resolver.test.ts",
      "apps/oa-updates/src/manifest-validate.test.ts",
      "apps/oa-updates/src/multipart-body.test.ts",
      "apps/oa-updates/src/node-registry.test.ts",
      "apps/oa-updates/src/openagents-desktop-publish-serve.test.ts",
      "apps/oa-updates/src/openagents-desktop-release.test.ts",
      "apps/oa-updates/src/openagents-desktop-seed.test.ts",
      "apps/oa-updates/src/production-entrypoint.test.ts",
      "apps/oa-updates/src/publish-builder.test.ts",
      "apps/oa-updates/src/publish-ota-contract.test.ts",
      "apps/oa-updates/src/publish.test.ts",
      "apps/oa-updates/src/pylon-release.test.ts",
      "apps/oa-updates/src/pylon-seed.test.ts",
      "apps/oa-updates/src/release-set-artifact-verifier.test.ts",
      "apps/oa-updates/src/release-set-feed.test.ts",
      "apps/oa-updates/src/release-set-gcs-store.test.ts",
      "apps/oa-updates/src/serve.test.ts",
      "apps/oa-updates/src/server.test.ts",
      "apps/oa-updates/src/signed-response.test.ts",
      "apps/oa-updates/src/update-channel-config.test.ts",
      "apps/openagents-audio/src/auth.test.ts",
      "apps/openagents-audio/src/media.test.ts",
      "apps/openagents-audio/src/server.test.ts",
      "apps/openagents-audio/src/session.test.ts",
      "apps/openagents-audio/src/tts.test.ts",
      "apps/openagents-audio/test/live-script-contracts.test.ts",
      "apps/openagents-audio/test/privacy.test.ts",
      "apps/openagents-audio/test/retention.test.ts",
      "apps/openagents-desktop/scripts/check-ide-boundaries.test.ts",
      "apps/openagents-desktop/src/acp-provider-host.test.ts",
      "apps/openagents-desktop/src/acp-provider-path-store.test.ts",
      "apps/openagents-desktop/src/assurance-receipt-bridge.test.ts",
      "apps/openagents-desktop/src/builtin-productspec-skill.test.ts",
      "apps/openagents-desktop/src/chat-service.test.ts",
      "apps/openagents-desktop/src/codex-app-server-client.test.ts",
      "apps/openagents-desktop/src/codex-app-server-smoke-fixture.test.ts",
      "apps/openagents-desktop/src/codex-app-server-supervisor.test.ts",
      "apps/openagents-desktop/src/codex-app-server-turn.test.ts",
      "apps/openagents-desktop/src/codex-child-runtime.test.ts",
      "apps/openagents-desktop/src/codex-config-health.test.ts",
      "apps/openagents-desktop/src/codex-conformance.test.ts",
      "apps/openagents-desktop/src/codex-connection-signatures.test.ts",
      "apps/openagents-desktop/src/codex-control-plane.test.ts",
      "apps/openagents-desktop/src/codex-durable-queue.test.ts",
      "apps/openagents-desktop/src/codex-ecosystem.test.ts",
      "apps/openagents-desktop/src/codex-experimental-contract.test.ts",
      "apps/openagents-desktop/src/codex-experimental-runtime.test.ts",
      "apps/openagents-desktop/src/codex-handoff-host.test.ts",
      "apps/openagents-desktop/src/codex-handoff-integration.test.ts",
      "apps/openagents-desktop/src/codex-handoff.test.ts",
      "apps/openagents-desktop/src/codex-history-host.test.ts",
      "apps/openagents-desktop/src/codex-history-plan-projection.test.ts",
      "apps/openagents-desktop/src/codex-history-utility.test.ts",
      "apps/openagents-desktop/src/codex-history.test.ts",
      "apps/openagents-desktop/src/codex-host-contract.test.ts",
      "apps/openagents-desktop/src/codex-host-services.test.ts",
      "apps/openagents-desktop/src/codex-local-contract.test.ts",
      "apps/openagents-desktop/src/codex-local-runtime.test.ts",
      "apps/openagents-desktop/src/codex-native-event-plane.test.ts",
      "apps/openagents-desktop/src/codex-preflight.test.ts",
      "apps/openagents-desktop/src/codex-release-notes.test.ts",
      "apps/openagents-desktop/src/codex-reverse-rpc-arbiter.test.ts",
      "apps/openagents-desktop/src/codex-thread-lifecycle.test.ts",
      "apps/openagents-desktop/src/codex-turn-state.test.ts",
      "apps/openagents-desktop/src/composer-admission.test.ts",
      "apps/openagents-desktop/src/desktop-codex-usage-outbox.test.ts",
      "apps/openagents-desktop/src/desktop-codex-usage-reporter.test.ts",
      "apps/openagents-desktop/src/desktop-host-lifecycle.test.ts",
      "apps/openagents-desktop/src/desktop-launch-context.test.ts",
      "apps/openagents-desktop/src/desktop-launch-workspace.test.ts",
      "apps/openagents-desktop/src/desktop-operation-context.test.ts",
      "apps/openagents-desktop/src/desktop-renderer-location.test.ts",
      "apps/openagents-desktop/src/desktop-runtime-workspace.test.ts",
      "apps/openagents-desktop/src/desktop-worker-location.test.ts",
      "apps/openagents-desktop/src/extension-lifecycle-contract.test.ts",
      "apps/openagents-desktop/src/fable-local-runtime-caps.test.ts",
      "apps/openagents-desktop/src/fable-local-runtime.test.ts",
      "apps/openagents-desktop/src/full-auto-acceptance-driver.test.ts",
      "apps/openagents-desktop/src/full-auto-acceptance.test.ts",
      "apps/openagents-desktop/src/full-auto-control-server.test.ts",
      "apps/openagents-desktop/src/full-auto-followup.test.ts",
      "apps/openagents-desktop/src/full-auto-hydration.integration.test.ts",
      "apps/openagents-desktop/src/full-auto-lane.test.ts",
      "apps/openagents-desktop/src/full-auto-liveness.test.ts",
      "apps/openagents-desktop/src/full-auto-mission.test.ts",
      "apps/openagents-desktop/src/full-auto-provider-handoff.test.ts",
      "apps/openagents-desktop/src/full-auto-retry-rotation-model.test.ts",
      "apps/openagents-desktop/src/full-auto-routing.test.ts",
      "apps/openagents-desktop/src/full-auto-run-analyzer.test.ts",
      "apps/openagents-desktop/src/full-auto-run-control-server.test.ts",
      "apps/openagents-desktop/src/full-auto-run-handoff-control-server.test.ts",
      "apps/openagents-desktop/src/full-auto-run-liveness-control-server.test.ts",
      "apps/openagents-desktop/src/full-auto-run-report-control-server.test.ts",
      "apps/openagents-desktop/src/full-auto-run-report.test.ts",
      "apps/openagents-desktop/src/git-github-contract.test.ts",
      "apps/openagents-desktop/src/git-github-host.test.ts",
      "apps/openagents-desktop/src/git-review-corpus.node.test.ts",
      "apps/openagents-desktop/src/history-thread-actions.test.ts",
      "apps/openagents-desktop/src/ide/baseline-contract.test.ts",
      "apps/openagents-desktop/src/ide/project-contract.test.ts",
      "apps/openagents-desktop/src/ide/project-service.test.ts",
      "apps/openagents-desktop/src/isolated-app-proof.test.ts",
      "apps/openagents-desktop/src/live-agent-graph-host.test.ts",
      "apps/openagents-desktop/src/live-agent-graph-local.test.ts",
      "apps/openagents-desktop/src/live-proof.test.ts",
      "apps/openagents-desktop/src/local-runtime-event-persistence.test.ts",
      "apps/openagents-desktop/src/local-turn-journal.test.ts",
      "apps/openagents-desktop/src/local-turn-recovery.test.ts",
      "apps/openagents-desktop/src/local-turn-text-persistence.test.ts",
      "apps/openagents-desktop/src/macos-document-open.test.ts",
      "apps/openagents-desktop/src/macos-update-applier.test.ts",
      "apps/openagents-desktop/src/mcp-config-host.test.ts",
      "apps/openagents-desktop/src/mvp-assurance-bridge.test.ts",
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "apps/openagents-desktop/src/mvp-proof.test.ts",
      "apps/openagents-desktop/src/product-spec-app-server-tools.test.ts",
      "apps/openagents-desktop/src/product-spec-workroom.test.ts",
      "apps/openagents-desktop/src/provider-lane-acp.test.ts",
      "apps/openagents-desktop/src/provider-lane-capabilities.test.ts",
      "apps/openagents-desktop/src/provider-lane-registry.test.ts",
      "apps/openagents-desktop/src/provider-lane.test.ts",
      "apps/openagents-desktop/src/provider-runtime-compatibility.test.ts",
      "apps/openagents-desktop/src/provider-runtime-host.test.ts",
      "apps/openagents-desktop/src/provider-runtime-target.test.ts",
      "apps/openagents-desktop/src/react-conversation-assurance.test.ts",
      "apps/openagents-desktop/src/renderer/acp-provider-settings.test.ts",
      "apps/openagents-desktop/src/renderer/assurance-spec-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/branding.test.ts",
      "apps/openagents-desktop/src/renderer/command-notice.test.ts",
      "apps/openagents-desktop/src/renderer/command-shortcuts.test.ts",
      "apps/openagents-desktop/src/renderer/composer-focus.test.ts",
      "apps/openagents-desktop/src/renderer/composer-image-acquisition.test.ts",
      "apps/openagents-desktop/src/renderer/composer-images.test.ts",
      "apps/openagents-desktop/src/renderer/composer-shortcuts.test.ts",
      "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
      "apps/openagents-desktop/src/renderer/diagnostics.test.ts",
      "apps/openagents-desktop/src/renderer/fleet-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/full-auto-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/git-panel.test.ts",
      "apps/openagents-desktop/src/renderer/history-restore.test.ts",
      "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/ide/pierre-tree-adapter.test.ts",
      "apps/openagents-desktop/src/renderer/latest-only-queue.test.ts",
      "apps/openagents-desktop/src/renderer/local-harness.test.ts",
      "apps/openagents-desktop/src/renderer/markdown.test.ts",
      "apps/openagents-desktop/src/renderer/mvp-assurance-congruence.test.ts",
      "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.test.ts",
      "apps/openagents-desktop/src/renderer/navigation-history.test.ts",
      "apps/openagents-desktop/src/renderer/product-spec-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
      "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx",
      "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx",
      "apps/openagents-desktop/src/renderer/react-review-sheet.test.tsx",
      "apps/openagents-desktop/src/renderer/react-review.test.tsx",
      "apps/openagents-desktop/src/renderer/react-timeline.test.tsx",
      "apps/openagents-desktop/src/renderer/remote-connect.test.ts",
      "apps/openagents-desktop/src/renderer/runtime-agent-graph.test.ts",
      "apps/openagents-desktop/src/renderer/runtime-cards.test.ts",
      "apps/openagents-desktop/src/renderer/runtime-conversation.test.ts",
      "apps/openagents-desktop/src/renderer/runtime-interactions.test.ts",
      "apps/openagents-desktop/src/renderer/runtime-live-client.test.ts",
      "apps/openagents-desktop/src/renderer/settings.test.ts",
      "apps/openagents-desktop/src/renderer/shell.test.ts",
      "apps/openagents-desktop/src/renderer/sidebar-accounts.test.ts",
      "apps/openagents-desktop/src/renderer/sidebar-destinations.test.ts",
      "apps/openagents-desktop/src/renderer/skill-invocation.test.ts",
      "apps/openagents-desktop/src/renderer/surface-layout.test.ts",
      "apps/openagents-desktop/src/renderer/terminal-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/tool-cards.test.ts",
      "apps/openagents-desktop/src/renderer/visual-baseline-fixtures.test.ts",
      "apps/openagents-desktop/src/renderer/visual-baseline-workbench.test.tsx",
      "apps/openagents-desktop/src/renderer/voice-actions.test.ts",
      "apps/openagents-desktop/src/renderer/voice-mode.test.ts",
      "apps/openagents-desktop/src/renderer/workspace-browser.test.ts",
      "apps/openagents-desktop/src/renderer/workspace-editor.test.ts",
      "apps/openagents-desktop/src/runtime-control-outcome-store.test.ts",
      "apps/openagents-desktop/src/runtime-live-subscriptions.test.ts",
      "apps/openagents-desktop/src/spec-lane-workflow.test.ts",
      "apps/openagents-desktop/src/t3-component-census.test.ts",
      "apps/openagents-desktop/src/terminal-host.test.ts",
      "apps/openagents-desktop/src/thread-event-authority-relation-ledger.test.ts",
      "apps/openagents-desktop/src/thread-event-search-artifact-source.test.ts",
      "apps/openagents-desktop/src/thread-event-search-bridge-contract.test.ts",
      "apps/openagents-desktop/src/thread-event-search-electron-host.test.ts",
      "apps/openagents-desktop/src/thread-event-search-host-runtime.test.ts",
      "apps/openagents-desktop/src/thread-event-search-main-handler.test.ts",
      "apps/openagents-desktop/src/thread-event-search-receipt-catalog.test.ts",
      "apps/openagents-desktop/src/thread-export-artifact-store.test.ts",
      "apps/openagents-desktop/src/thread-export-bridge-contract.test.ts",
      "apps/openagents-desktop/src/thread-export-command.test.ts",
      "apps/openagents-desktop/src/thread-export-confirmed-timeline-command.test.ts",
      "apps/openagents-desktop/src/thread-export-confirmed-timeline-evidence.test.ts",
      "apps/openagents-desktop/src/thread-export-create-bridge-contract.test.ts",
      "apps/openagents-desktop/src/thread-export-create-main-handler.test.ts",
      "apps/openagents-desktop/src/thread-export-electron-host.test.ts",
      "apps/openagents-desktop/src/thread-export-file-transport.test.ts",
      "apps/openagents-desktop/src/thread-export-host-runtime.test.ts",
      "apps/openagents-desktop/src/thread-export-main-composition.test.ts",
      "apps/openagents-desktop/src/thread-export-main-handler.test.ts",
      "apps/openagents-desktop/src/thread-export-terminal-authority-overlay.test.ts",
      "apps/openagents-desktop/src/thread-export-workflow.test.ts",
      "apps/openagents-desktop/src/thread-store.test.ts",
      "apps/openagents-desktop/src/thread-visibility-audience-authorization.test.ts",
      "apps/openagents-desktop/src/thread-visibility-bridge-contract.test.ts",
      "apps/openagents-desktop/src/thread-visibility-main-composition.test.ts",
      "apps/openagents-desktop/src/thread-visibility-main-handler.test.ts",
      "apps/openagents-desktop/src/thread-visibility-policy-store.test.ts",
      "apps/openagents-desktop/src/thread-visibility-publication-transport.test.ts",
      "apps/openagents-desktop/src/thread-visibility-sync-authority.test.ts",
      "apps/openagents-desktop/src/thread-visibility-workspace-publication-transport.test.ts",
      "apps/openagents-desktop/src/update-feed-config.test.ts",
      "apps/openagents-desktop/src/update-migration-evidence.test.ts",
      "apps/openagents-desktop/src/update-runtime-drain.test.ts",
      "apps/openagents-desktop/src/update-staging-host.test.ts",
      "apps/openagents-desktop/src/update-staging-integration.test.ts",
      "apps/openagents-desktop/src/usage-ledger.test.ts",
      "apps/openagents-desktop/src/visual-baseline-baselines.test.ts",
      "apps/openagents-desktop/src/visual-baseline-diff.test.ts",
      "apps/openagents-desktop/src/visual-baseline-swarm-contract.test.ts",
      "apps/openagents-desktop/src/voice-host.test.ts",
      "apps/openagents-desktop/src/voice-native-helper.test.ts",
      "apps/openagents-desktop/src/voice-permission-policy.test.ts",
      "apps/openagents-desktop/src/workbench-item-contract.test.ts",
      "apps/openagents-desktop/src/workspace-search-host.test.ts",
      "apps/openagents-desktop/src/workspace-search-registry.test.ts",
      "apps/openagents-desktop/tests/accessibility.test.ts",
      "apps/openagents-desktop/tests/build.test.ts",
      "apps/openagents-desktop/tests/capability-evals.test.ts",
      "apps/openagents-desktop/tests/claude-history-performance.e2e.test.ts",
      "apps/openagents-desktop/tests/claude-history.test.ts",
      "apps/openagents-desktop/tests/codex-connect.test.ts",
      "apps/openagents-desktop/tests/codex-history-performance.e2e.test.ts",
      "apps/openagents-desktop/tests/codex-history.e2e.test.ts",
      "apps/openagents-desktop/tests/codex-history.test.ts",
      "apps/openagents-desktop/tests/codex-subagent-history.test.ts",
      "apps/openagents-desktop/tests/desktop-coding-catalog.test.ts",
      "apps/openagents-desktop/tests/desktop-command-bindings.test.ts",
      "apps/openagents-desktop/tests/desktop-command-contract.test.ts",
      "apps/openagents-desktop/tests/desktop-command-host.test.ts",
      "apps/openagents-desktop/tests/desktop-preferences.test.ts",
      "apps/openagents-desktop/tests/desktop-session-pkce.test.ts",
      "apps/openagents-desktop/tests/desktop-session-recovery.test.ts",
      "apps/openagents-desktop/tests/desktop-session-vault.test.ts",
      "apps/openagents-desktop/tests/desktop-sync-host.test.ts",
      "apps/openagents-desktop/tests/dev-server.test.ts",
      "apps/openagents-desktop/tests/diagnostics.test.ts",
      "apps/openagents-desktop/tests/electron-boundary.test.ts",
      "apps/openagents-desktop/tests/electron-trace-acceptance.test.ts",
      "apps/openagents-desktop/tests/fixtures/claude-smoke/projects/openagents-desktop/11111111-2222-3333-4444-555555555555.jsonl",
      "apps/openagents-desktop/tests/fixtures/codex-smoke/session_index.jsonl",
      "apps/openagents-desktop/tests/fixtures/codex-smoke/sessions/2026/07/11/smoke-child.jsonl",
      "apps/openagents-desktop/tests/fixtures/codex-smoke/sessions/2026/07/11/smoke-root.jsonl",
      "apps/openagents-desktop/tests/fixtures/provider-accounts/accounts-list.json",
      "apps/openagents-desktop/tests/fixtures/release-set-v1.json",
      "apps/openagents-desktop/tests/fixtures/release-set-v2.json",
      "apps/openagents-desktop/tests/fixtures/working-indicator-motion-probe.cjs",
      "apps/openagents-desktop/tests/fleet-control.test.ts",
      "apps/openagents-desktop/tests/full-auto-guardrails.test.ts",
      "apps/openagents-desktop/tests/full-auto-liveness.test.ts",
      "apps/openagents-desktop/tests/full-auto-registry.test.ts",
      "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts",
      "apps/openagents-desktop/tests/full-auto-run-control-intent-consumer.test.ts",
      "apps/openagents-desktop/tests/full-auto-run-projection-publisher.test.ts",
      "apps/openagents-desktop/tests/full-auto-run-registry.test.ts",
      "apps/openagents-desktop/tests/full-auto-soak-harness.ts",
      "apps/openagents-desktop/tests/full-auto-soak.e2e.test.ts",
      "apps/openagents-desktop/tests/full-auto-thread-pressure.e2e.test.ts",
      "apps/openagents-desktop/tests/git-fixture.ts",
      "apps/openagents-desktop/tests/harness-maintenance.test.ts",
      "apps/openagents-desktop/tests/history-catalog-scale.test.ts",
      "apps/openagents-desktop/tests/history-search.test.ts",
      "apps/openagents-desktop/tests/isolated-app-proof.test.ts",
      "apps/openagents-desktop/tests/launch-receipt.test.ts",
      "apps/openagents-desktop/tests/local-first-identity.e2e.test.ts",
      "apps/openagents-desktop/tests/local-turn-restart.e2e.test.ts",
      "apps/openagents-desktop/tests/macos-gatekeeper.test.ts",
      "apps/openagents-desktop/tests/native-conversation-continuation.e2e.test.ts",
      "apps/openagents-desktop/tests/native-timeline-fault-convergence.e2e.test.ts",
      "apps/openagents-desktop/tests/notification-attention.test.ts",
      "apps/openagents-desktop/tests/oa-dev-supervisor.test.ts",
      "apps/openagents-desktop/tests/owner-ux-rules.test.ts",
      "apps/openagents-desktop/tests/package-macos.test.ts",
      "apps/openagents-desktop/tests/pierre-tree-package.test.ts",
      "apps/openagents-desktop/tests/plugin-config.test.ts",
      "apps/openagents-desktop/tests/provider-accounts.test.ts",
      "apps/openagents-desktop/tests/publish-release.test.ts",
      "apps/openagents-desktop/tests/release-preflight.test.ts",
      "apps/openagents-desktop/tests/release-set-contract.test.ts",
      "apps/openagents-desktop/tests/release-staging.test.ts",
      "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
      "apps/openagents-desktop/tests/service-topology.test.ts",
      "apps/openagents-desktop/tests/startup-contract.test.ts",
      "apps/openagents-desktop/tests/turn-checkpoints.test.ts",
      "apps/openagents-desktop/tests/update-contract.test.ts",
      "apps/openagents-desktop/tests/update-rollback.test.ts",
      "apps/openagents-desktop/tests/voice-boundary.test.ts",
      "apps/openagents-desktop/tests/voice-runtime-gateway.test.ts",
      "apps/openagents-desktop/tests/working-indicator-motion.e2e.test.ts",
      "apps/openagents-desktop/tests/workspace-scale.e2e.test.ts",
      "apps/openagents-desktop/tests/workspace-service.test.ts",
      "apps/openagents-mobile/tests/app-identity.test.ts",
      "apps/openagents-mobile/tests/authoritative-home.test.ts",
      "apps/openagents-mobile/tests/component-sharing.test.ts",
      "apps/openagents-mobile/tests/full-auto-run-control-intent.test.ts",
      "apps/openagents-mobile/tests/full-auto-run-header.test.ts",
      "apps/openagents-mobile/tests/full-auto-run-projection-source.test.ts",
      "apps/openagents-mobile/tests/full-auto-run-projection.test.ts",
      "apps/openagents-mobile/tests/home-shell-core.test.ts",
      "apps/openagents-mobile/tests/khala-surface.test.ts",
      "apps/openagents-mobile/tests/local-first-identity.e2e.test.ts",
      "apps/openagents-mobile/tests/mobile-accessibility.test.ts",
      "apps/openagents-mobile/tests/mobile-account-control.test.ts",
      "apps/openagents-mobile/tests/mobile-adaptive-workspace.test.ts",
      "apps/openagents-mobile/tests/mobile-agent-graph.test.ts",
      "apps/openagents-mobile/tests/mobile-attention-target.test.ts",
      "apps/openagents-mobile/tests/mobile-coding-attachment-delivery.test.ts",
      "apps/openagents-mobile/tests/mobile-coding-attachment-picker.test.ts",
      "apps/openagents-mobile/tests/mobile-coding-composer.test.ts",
      "apps/openagents-mobile/tests/mobile-coding-navigation.test.ts",
      "apps/openagents-mobile/tests/mobile-composer-attachments.test.ts",
      "apps/openagents-mobile/tests/mobile-composer-discovery.test.ts",
      "apps/openagents-mobile/tests/mobile-composer-path-context.test.ts",
      "apps/openagents-mobile/tests/mobile-composer-run-control.test.ts",
      "apps/openagents-mobile/tests/mobile-composer-toolbar.test.ts",
      "apps/openagents-mobile/tests/mobile-controller-directory.test.ts",
      "apps/openagents-mobile/tests/mobile-controller-session-detail.test.ts",
      "apps/openagents-mobile/tests/mobile-controller-shell.test.ts",
      "apps/openagents-mobile/tests/mobile-conversation.test.ts",
      "apps/openagents-mobile/tests/mobile-execution-targets.test.ts",
      "apps/openagents-mobile/tests/mobile-experience-reconciler.test.ts",
      "apps/openagents-mobile/tests/mobile-interaction-card.test.ts",
      "apps/openagents-mobile/tests/mobile-native-feedback.test.ts",
      "apps/openagents-mobile/tests/mobile-portable-controls-ui.test.ts",
      "apps/openagents-mobile/tests/mobile-portable-session-controls.test.ts",
      "apps/openagents-mobile/tests/mobile-repository-environment-client.test.ts",
      "apps/openagents-mobile/tests/mobile-repository-files.test.ts",
      "apps/openagents-mobile/tests/mobile-repository-git.test.ts",
      "apps/openagents-mobile/tests/mobile-repository-review.test.ts",
      "apps/openagents-mobile/tests/mobile-repository-terminal.test.ts",
      "apps/openagents-mobile/tests/mobile-runtime-queue.test.ts",
      "apps/openagents-mobile/tests/mobile-settings.test.ts",
      "apps/openagents-mobile/tests/mobile-sync-host.test.ts",
      "apps/openagents-mobile/tests/mobile-transcript-attachment.test.ts",
      "apps/openagents-mobile/tests/mobile-transcript-content.test.ts",
      "apps/openagents-mobile/tests/mobile-transcript-history.test.ts",
      "apps/openagents-mobile/tests/mobile-work-log.test.ts",
      "apps/openagents-mobile/tests/mobile-workspace-actions.test.ts",
      "apps/openagents-mobile/tests/mobile-workspace-keyboard.test.ts",
      "apps/openagents-mobile/tests/mobile-workspace-navigation.test.ts",
      "apps/openagents-mobile/tests/native-attention-target-delivery.test.ts",
      "apps/openagents-mobile/tests/native-coding-target-delivery.test.ts",
      "apps/openagents-mobile/tests/native-session-pkce.test.ts",
      "apps/openagents-mobile/tests/native-session-recovery.test.ts",
      "apps/openagents-mobile/tests/native-session-vault.test.ts",
      "apps/openagents-mobile/tests/ota-polling.test.ts",
      "apps/openagents-mobile/tests/sarah-owner-orchestrator.test.ts",
      "apps/openagents-mobile/tests/sarah-speech-client.test.ts",
      "apps/openagents-mobile/tests/t3-mobile-component-census.test.ts",
      "apps/openagents.com/apps/start/src/desktop-download-resolver.server.test.ts",
      "apps/openagents.com/apps/start/src/docs/docs-content.test.ts",
      "apps/openagents.com/apps/start/src/docs/docs-layout-contract.test.ts",
      "apps/openagents.com/apps/start/src/forum-entry.test.ts",
      "apps/openagents.com/apps/start/src/khala-sync-proxy.test.ts",
      "apps/openagents.com/apps/start/src/qa-board-projection.server.test.ts",
      "apps/openagents.com/apps/start/src/routes/-activity.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-artanis-accounts.test.tsx"
    ],
    "declared_scripts": [
      {
        "manifest_path": "apps/acceptance-runner/package.json",
        "name": "playwright:install",
        "command": "playwright install --with-deps chromium"
      },
      {
        "manifest_path": "apps/acceptance-runner/package.json",
        "name": "run-once",
        "command": "node --import tsx src/run-once.ts"
      },
      {
        "manifest_path": "apps/acceptance-runner/package.json",
        "name": "serve",
        "command": "node --import tsx src/service.ts"
      },
      {
        "manifest_path": "apps/acceptance-runner/package.json",
        "name": "test",
        "command": "vp test --run src/daemon.test.ts"
      },
      {
        "manifest_path": "apps/ai-sdk-harness-poc/package.json",
        "name": "dev",
        "command": "node --import tsx src/server.ts"
      },
      {
        "manifest_path": "apps/ai-sdk-harness-poc/package.json",
        "name": "start",
        "command": "node --import tsx src/server.ts"
      },
      {
        "manifest_path": "apps/ai-sdk-harness-poc/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/aiur/package.json",
        "name": "build",
        "command": "pnpm run build:cloudrun"
      },
      {
        "manifest_path": "apps/aiur/package.json",
        "name": "build:cloudrun",
        "command": "vp build --config vite.config.cloudrun.ts --logLevel warn && vp pack src/cloudrun/server.ts --format esm --platform node --out-dir dist/cloudrun"
      },
      {
        "manifest_path": "apps/aiur/package.json",
        "name": "deploy",
        "command": "pnpm run deploy:cloudrun"
      },
      {
        "manifest_path": "apps/aiur/package.json",
        "name": "deploy:cloudrun",
        "command": "bash scripts/deploy-cloudrun.sh"
      },
      {
        "manifest_path": "apps/aiur/package.json",
        "name": "dev",
        "command": "vp dev --config vite.config.cloudrun.ts"
      },
      {
        "manifest_path": "apps/aiur/package.json",
        "name": "preview",
        "command": "vp preview --config vite.config.cloudrun.ts"
      },
      {
        "manifest_path": "apps/aiur/package.json",
        "name": "test",
        "command": "vitest run"
      },
      {
        "manifest_path": "apps/aiur/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/electron-ai-sdk-test/package.json",
        "name": "dev",
        "command": "electron-forge start"
      },
      {
        "manifest_path": "apps/electron-ai-sdk-test/package.json",
        "name": "make",
        "command": "electron-forge make"
      },
      {
        "manifest_path": "apps/electron-ai-sdk-test/package.json",
        "name": "package",
        "command": "electron-forge package"
      },
      {
        "manifest_path": "apps/electron-ai-sdk-test/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/forum/package.json",
        "name": "dev",
        "command": "node --import tsx src/index.ts"
      },
      {
        "manifest_path": "apps/forum/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "apps/forum/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/khala-capture/package.json",
        "name": "build:cloudrun",
        "command": "vp pack src/server.ts --format esm --platform node --out-dir dist"
      },
      {
        "manifest_path": "apps/khala-capture/package.json",
        "name": "deploy:cloudrun",
        "command": "bash scripts/deploy-cloudrun.sh"
      },
      {
        "manifest_path": "apps/khala-capture/package.json",
        "name": "dev",
        "command": "node --watch --import tsx src/server.ts"
      },
      {
        "manifest_path": "apps/khala-capture/package.json",
        "name": "start",
        "command": "node --import tsx src/server.ts"
      },
      {
        "manifest_path": "apps/khala-capture/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "apps/khala-capture/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "build:cloudrun",
        "command": "vp pack src/server.ts --format esm --platform node --out-dir dist"
      },
      {
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "deploy:cloudrun",
        "command": "bash scripts/deploy-cloudrun.sh"
      },
      {
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "dev",
        "command": "node --watch --import tsx src/server.ts"
      },
      {
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "start",
        "command": "node --import tsx src/server.ts"
      },
      {
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "build",
        "command": "vp pack src/main.ts --format esm --platform node --out-dir dist"
      },
      {
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "deploy",
        "command": "bash scripts/deploy-cloudrun.sh"
      },
      {
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "serve",
        "command": "node --import tsx src/main.ts"
      },
      {
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "test",
        "command": "vp test --run src"
      },
      {
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/oa-updates/package.json",
        "name": "build:server",
        "command": "vp pack src/serve.ts --out-dir dist-server --format esm --platform node --target node24 --minify"
      },
      {
        "manifest_path": "apps/oa-updates/package.json",
        "name": "desktop:publish",
        "command": "node --import tsx scripts/publish-desktop-release.ts"
      },
      {
        "manifest_path": "apps/oa-updates/package.json",
        "name": "serve",
        "command": "node --import tsx src/serve.ts"
      },
      {
        "manifest_path": "apps/oa-updates/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "apps/oa-updates/package.json",
        "name": "typecheck",
        "command": "node scripts/verify-test-typecheck.mjs"
      },
      {
        "manifest_path": "apps/openagents-audio-edge/package.json",
        "name": "build",
        "command": "vp pack src/cloudrun.ts --format esm --platform node --out-dir dist"
      },
      {
        "manifest_path": "apps/openagents-audio-edge/package.json",
        "name": "deploy",
        "command": "bash deploy-cloudrun.sh"
      },
      {
        "manifest_path": "apps/openagents-audio-edge/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "build:cloudrun",
        "command": "vp pack src/main.ts --format esm --platform node --out-dir dist"
      },
      {
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "deploy:cloudrun",
        "command": "bash scripts/deploy-cloudrun.sh"
      },
      {
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "smoke:barge:live",
        "command": "node --import tsx scripts/live-barge-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "smoke:live",
        "command": "node --import tsx scripts/live-retention-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "smoke:tts:live",
        "command": "node --import tsx scripts/live-tts-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "start",
        "command": "node --import tsx src/main.ts"
      },
      {
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "test",
        "command": "vp test --run src test"
      },
      {
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "build",
        "command": "node --import tsx scripts/build.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "check:ide-boundaries",
        "command": "node --import tsx scripts/check-ide-boundaries.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "dev",
        "command": "node --import tsx scripts/dev.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "dev:preview",
        "command": "node --import tsx scripts/dev.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "full-auto",
        "command": "node --import tsx scripts/full-auto-cli.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "generate:codex-conformance",
        "command": "node --import tsx scripts/generate-codex-conformance-report.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "ide-baseline",
        "command": "node --import tsx scripts/ide-baseline.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "live-proof",
        "command": "node --import tsx scripts/build.ts && node --import tsx scripts/run-live-proof.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "make:mac",
        "command": "node --import tsx scripts/prepare-macos-maker.ts && node --import tsx scripts/stage-and-package.ts --target darwin-arm64 --mode make"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "mvp-proof",
        "command": "node --import tsx scripts/build.ts && node --import tsx scripts/run-mvp-proof.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "package:mac",
        "command": "node --import tsx scripts/stage-and-package.ts --target darwin-arm64 --mode package"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "qa:visual",
        "command": "node --import tsx scripts/build.ts && node --import tsx scripts/visual-baseline-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "release-acceptance",
        "command": "node --import tsx scripts/run-release-acceptance.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke",
        "command": "node --import tsx scripts/build.ts && OPENAGENTS_DESKTOP_SMOKE=1 electron ."
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:acp-release",
        "command": "node --import tsx scripts/acp-packaged-release-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-control-plane",
        "command": "node --import tsx scripts/codex-control-plane-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-ecosystem",
        "command": "node --import tsx scripts/codex-ecosystem-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-experimental",
        "command": "node --import tsx scripts/codex-experimental-runtime-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-host-services",
        "command": "node --import tsx scripts/codex-host-services-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-supervisor",
        "command": "node --import tsx scripts/codex-app-server-supervisor-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-thread-lifecycle",
        "command": "node --import tsx scripts/codex-thread-lifecycle-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-turn-control",
        "command": "node --import tsx scripts/codex-turn-control-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:full-auto-control",
        "command": "node --import tsx scripts/full-auto-control-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:full-auto-restart",
        "command": "node --import tsx scripts/full-auto-restart-smoke.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:headed",
        "command": "node --import tsx scripts/build.ts && OPENAGENTS_DESKTOP_SMOKE=1 OPENAGENTS_DESKTOP_HEADED=1 electron ."
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:react",
        "command": "node --import tsx scripts/build.ts && OPENAGENTS_DESKTOP_SMOKE=1 OPENAGENTS_DESKTOP_SMOKE_REACT=1 electron ."
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "soak:full-auto",
        "command": "node --import tsx scripts/full-auto-soak.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "stage:target",
        "command": "node --import tsx scripts/stage-target.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "startup-bench",
        "command": "node --import tsx scripts/startup-bench.ts"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "test",
        "command": "vp test --run --max-concurrency 1 --root ../.. apps/openagents-desktop"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "verify",
        "command": "pnpm run typecheck && pnpm run test && pnpm run build && OPENAGENTS_DESKTOP_SMOKE=1 pnpm run smoke && pnpm run smoke:react"
      },
      {
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "dev",
        "command": "expo start"
      },
      {
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "prebuild",
        "command": "expo prebuild"
      },
      {
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "prebuild:android",
        "command": "expo prebuild --platform android"
      },
      {
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "prebuild:ios",
        "command": "expo prebuild --platform ios"
      },
      {
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "publish:ota",
        "command": "bash ../../apps/oa-updates/scripts/publish-ota.sh"
      },
      {
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "test",
        "command": "vp test --run --root ../.. apps/openagents-mobile"
      },
      {
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "budget",
        "command": "pnpm run build && node --import tsx src/routes/-funnel-budget.ts"
      },
      {
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "build",
        "command": "pnpm run generate:docs && vp build --logLevel warn && vp pack cloudrun/server.mjs --out-dir dist/cloudrun --format esm --platform node --target node24 --minify"
      },
      {
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "check:docs",
        "command": "node --import tsx scripts/generate-docs.ts --check"
      },
      {
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "deploy",
        "command": "bash scripts/deploy-cloudrun.sh stage1"
      },
      {
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "deploy:cloudrun",
        "command": "bash scripts/deploy-cloudrun.sh"
      },
      {
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "dev",
        "command": "pnpm run generate:docs && vp dev"
      },
      {
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "generate:docs",
        "command": "node --import tsx scripts/generate-docs.ts"
      },
      {
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "preview",
        "command": "vp preview"
      },
      {
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "test",
        "command": "pnpm run generate:docs && vp test --root ../../../.. --run --project @openagentsinc/openagents-com-start"
      },
      {
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "typecheck",
        "command": "pnpm run generate:docs && tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "artanis:readiness",
        "command": "node scripts/artanis-production-readiness.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "build",
        "command": "pnpm run build:start && pnpm run build:api"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "build:api",
        "command": "pnpm --dir workers/api run build"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "build:start",
        "command": "node scripts/sync-live-agent-doc.mjs && pnpm --dir apps/start run build"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:agent-doc-links",
        "command": "node --import tsx scripts/check-live-agent-doc-links.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:architecture",
        "command": "node --import tsx scripts/check-zero-debt-architecture.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:conflict-markers",
        "command": "node --import tsx scripts/check-conflict-markers.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:contract-drift",
        "command": "node --import tsx scripts/check-contract-drift.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:deploy",
        "command": "pnpm run check:conflict-markers && pnpm run check:no-github-actions && pnpm run check:effect-topology && pnpm run check:agent-doc-links && pnpm run check:architecture && pnpm run check:contract-drift && pnpm run check:public-projection-freshness && pnpm --dir ../../packages/agent-readiness run test && pnpm --dir ../../packages/agent-readiness run typecheck && pnpm --dir ../../packages/autopilot-control-protocol run typecheck && pnpm --dir ../../apps/pylon run typecheck && pnpm run typecheck:api-pylon-integration && pnpm --dir ../../apps/pylon run test tests/security-adversarial-harness.test.ts && pnpm --dir ../../packages/khala-sync-server run test:pending-migrations-guard && pnpm --dir ../../packages/khala-sync-client run test && pnpm run typecheck:start && pnpm run typecheck:api && pnpm run test:conflict-markers-guard && pnpm run test:effect-native-vendor-guard && pnpm run test:contract-drift-guard && pnpm --dir apps/start run test && pnpm --dir workers/api run test src/lander-css-policy.test.ts src/worker-routes.test.ts src/redirect-policy.test.ts src/client-server-route-agreement.test.ts src/mullet/routes.test.ts src/product-promises.test.ts src/model-custody-lead-gen.test.ts src/reactor-need-to-know-access.test.ts src/reactor-data-liberation.test.ts src/reactor-improvement-ladder.test.ts src/wasm-plugin-marketplace.test.ts src/qualified-contributor-methodology.test.ts src/public-forum-activity-routes.test.ts src/inference/inference-privacy-receipt-routes.test.ts src/inference/gym/terminal-bench-khala-orchestration.test.ts src/tassadar-settled-feed-sync.test.ts src/khala-sync-public-settled-feed.test.ts src/public-settled-feed-routes.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:deploy-from-main",
        "command": "node --import tsx scripts/check-deploy-from-main.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:effect-native-vendor",
        "command": "node --import tsx scripts/check-effect-native-vendor-freshness.ts"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:effect-topology",
        "command": "node --import tsx scripts/check-effect-topology.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:effect-upgrade-metadata",
        "command": "node --import tsx scripts/check-effect-upgrade-metadata.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:no-github-actions",
        "command": "node --import tsx scripts/check-no-github-actions.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:public-projection-freshness",
        "command": "node --import tsx scripts/check-public-projection-freshness.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "dev",
        "command": "pnpm run dev:start"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "dev:api",
        "command": "pnpm --dir workers/api run dev"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "dev:start",
        "command": "pnpm --dir apps/start run dev"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "format",
        "command": "prettier -w ."
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "forum",
        "command": "node scripts/forum.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "gym:harbor-full-trace-archive",
        "command": "node --import tsx scripts/gym-harbor-full-trace-archive.ts"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "gym:harbor-progress-push",
        "command": "node --import tsx scripts/gym-harbor-progress-push.ts"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "khala-code:verify",
        "command": "node --import tsx scripts/khala-code-headless-harness.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "lint",
        "command": "eslint ."
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "monitor:khala:production-readiness",
        "command": "node scripts/khala-production-readiness-monitor.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "predeploy:khala-sync-live-seam-smoke",
        "command": "node scripts/predeploy-khala-sync-live-seam-smoke.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "predeploy:parallel-dispatch-smoke",
        "command": "node scripts/predeploy-parallel-dispatch-smoke.mjs --approve-staging-mutation"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "prepare",
        "command": "node scripts/patch-effect-language-service.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "preview",
        "command": "pnpm --dir apps/start run preview"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:activity:proof-links",
        "command": "node scripts/public-activity-proof-links-smoke.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:forum:void",
        "command": "node scripts/forum-void-smoke.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:gpt-oss20b-production",
        "command": "node scripts/gpt-oss20b-production-smoke.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:khala:gateway-readiness",
        "command": "node scripts/khala-gateway-readiness-smoke.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:khala:glm-reap",
        "command": "node scripts/khala-glm-reap-production-smoke.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:khala:production",
        "command": "node scripts/khala-production-smoke.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:visibility:browser",
        "command": "node scripts/visibility-browser-smoke.mjs"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test",
        "command": "pnpm run test:packages && pnpm run test:start && pnpm run test:api"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:api",
        "command": "pnpm --dir workers/api run test"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:command-composer-privacy-guard",
        "command": "vitest run scripts/check-command-composer-privacy-fixtures.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:conflict-markers-guard",
        "command": "vitest run scripts/check-conflict-markers.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:contract-drift-guard",
        "command": "vitest run scripts/check-contract-drift.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:effect-native-vendor-guard",
        "command": "vitest run scripts/check-effect-native-vendor.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:packages",
        "command": "pnpm --dir packages/email-templates run test && pnpm --dir packages/mullet-schema run test && pnpm --dir packages/mullet-sim run test && pnpm --dir packages/sync-schema run test && pnpm --dir packages/sync-client run test && pnpm --dir packages/sync-worker run test"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:predeploy-khala-sync-live-seam-smoke",
        "command": "vitest run scripts/predeploy-khala-sync-live-seam-smoke.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:predeploy-parallel-dispatch-smoke",
        "command": "vitest run scripts/predeploy-parallel-dispatch-smoke.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:start",
        "command": "pnpm --dir apps/start run test"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck",
        "command": "pnpm run typecheck:packages && pnpm run typecheck:start && pnpm run typecheck:api"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:api",
        "command": "pnpm --dir workers/api run typecheck"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:api-pylon-integration",
        "command": "tsc -p workers/api/tsconfig.pylon-api-routes.test.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:packages",
        "command": "pnpm --dir packages/email-templates run typecheck && pnpm --dir packages/mullet-schema run typecheck && pnpm --dir packages/mullet-sim run typecheck && pnpm --dir packages/sync-schema run typecheck && pnpm --dir packages/sync-client run typecheck && pnpm --dir packages/sync-worker run typecheck"
      },
      {
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:start",
        "command": "pnpm --dir apps/start run typecheck"
      },
      {
        "manifest_path": "apps/openagents.com/packages/effect-native-core/package.json",
        "name": "typecheck",
        "command": "tsc -b"
      },
      {
        "manifest_path": "apps/openagents.com/packages/effect-native-gallery/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/packages/effect-native-khala-ui/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/packages/effect-native-render-canvas/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/packages/effect-native-render-dom/package.json",
        "name": "typecheck",
        "command": "tsc -b"
      },
      {
        "manifest_path": "apps/openagents.com/packages/effect-native-render-rn/package.json",
        "name": "typecheck",
        "command": "tsc -b"
      },
      {
        "manifest_path": "apps/openagents.com/packages/effect-native-tokens/package.json",
        "name": "typecheck",
        "command": "tsc -b"
      },
      {
        "manifest_path": "apps/openagents.com/packages/email-templates/package.json",
        "name": "preview",
        "command": "node --import tsx src/preview.ts"
      },
      {
        "manifest_path": "apps/openagents.com/packages/email-templates/package.json",
        "name": "test",
        "command": "vitest run"
      },
      {
        "manifest_path": "apps/openagents.com/packages/email-templates/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/packages/mullet-schema/package.json",
        "name": "test",
        "command": "vitest run"
      },
      {
        "manifest_path": "apps/openagents.com/packages/mullet-schema/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/packages/mullet-sim/package.json",
        "name": "test",
        "command": "vitest run"
      },
      {
        "manifest_path": "apps/openagents.com/packages/mullet-sim/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/packages/sync-client/package.json",
        "name": "test",
        "command": "vitest run"
      },
      {
        "manifest_path": "apps/openagents.com/packages/sync-client/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/packages/sync-schema/package.json",
        "name": "test",
        "command": "vitest run"
      },
      {
        "manifest_path": "apps/openagents.com/packages/sync-schema/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/packages/sync-worker/package.json",
        "name": "test",
        "command": "vitest run"
      },
      {
        "manifest_path": "apps/openagents.com/packages/sync-worker/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "agent-readiness:fleet-run",
        "command": "node --import tsx scripts/agent-readiness-fleet-report-run.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "apollo-wave:dry-run",
        "command": "node --experimental-strip-types scripts/apollo-wave-runner.ts dry-run"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "apollo-wave:live",
        "command": "node --experimental-strip-types scripts/apollo-wave-runner.ts live"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "apollo-wave:print-fixture",
        "command": "node --experimental-strip-types scripts/apollo-wave-runner.ts print-fixture"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "build",
        "command": "pnpm --dir ../../apps/start run build && vp pack src/cloudrun/server.ts --out-dir dist-cloudrun --format esm --platform node --target node24"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "deploy",
        "command": "bash scripts/deploy-cloudrun.sh production"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "deploy:staging",
        "command": "bash scripts/deploy-cloudrun.sh staging"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "dev",
        "command": "node --import tsx src/cloudrun/server.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "glm-fleet:durability",
        "command": "node --import tsx scripts/khala-glm-fleet-durability.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "marching-orders",
        "command": "node --import tsx scripts/marching-orders-agent.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "pilot:glm-nvfp4",
        "command": "node --import tsx scripts/khala-glm-nvfp4-pilot.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:autopilot-coder:claude-agent-git-checkout",
        "command": "vitest run src/autopilot-work-routes.test.ts -t \"claude_agent_task git_checkout\""
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:autopilot-coder:no-spend",
        "command": "vitest run src/autopilot-work-routes.test.ts -t \"no-spend Autopilot Coder end-to-end smoke\""
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:autopilot-coder:paid",
        "command": "vitest run src/autopilot-work-routes.test.ts -t \"paid Autopilot Coder end-to-end smoke\""
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:autopilot-coder:rate-limit-rotation",
        "command": "vitest run src/autopilot-rate-limit-rotation-smoke.test.ts src/provider-account-lease-policy.test.ts src/provider-account-failover-policy.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:cs336-a1:no-spend",
        "command": "vitest run src/cs336-a1-homework.test.ts src/cs336-a1-homework-workload.test.ts src/cs336-a1-real-gradient-workload.test.ts src/training-real-gradient-evidence.test.ts src/training-verification.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:cs336-a2:device-capability",
        "command": "vitest run src/training-device-capability.test.ts src/training-device-admission-gates.test.ts src/training-run-window-routes.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:cs336-a3:isoflop",
        "command": "vitest run src/training-scaling-sweep.test.ts src/training-run-window-routes.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:cs336-a4:data-refinery",
        "command": "vitest run src/cs336-a4-data-refinery.test.ts src/cs336-a4-refinery-workload.test.ts src/training-data-refinery.test.ts src/training-run-window-routes.test.ts src/training-leaderboards.test.ts src/training-verification.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:cs336-a5:alignment",
        "command": "vitest run src/cs336-a5-alignment-homework.test.ts src/cs336-a5-rollout-workload.test.ts src/training-alignment-evals.test.ts src/training-run-window-routes.test.ts src/training-leaderboards.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:probe-gepa-stage0",
        "command": "node --import tsx scripts/probe-gepa-stage0-no-spend-campaign.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:qwen-remote-training",
        "command": "node --import tsx scripts/qwen-remote-pylon-live-training.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:tassadar:executor-trace",
        "command": "vitest run src/tassadar-executor-trace-homework.test.ts src/training-verification.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:training-leaderboards",
        "command": "vitest run src/training-leaderboards.test.ts src/training-run-window-routes.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:training-runs:public",
        "command": "vitest run src/training-run-window-routes.test.ts src/training-run-window-authority.test.ts src/training-run-public-copy-gate.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:training-validator:no-spend",
        "command": "vitest run src/training-validator-assignments.test.ts src/training-verification.test.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "start:cloudrun",
        "command": "node --import tsx ./src/cloudrun/server.ts"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "test",
        "command": "vp test --root ../../../.. --run --project @openagentsinc/api-worker"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "typecheck:cloudrun",
        "command": "tsc -p tsconfig.cloudrun.json --noEmit"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "build:apple-fm-bridge",
        "command": "bash swift/foundation-bridge/build.sh"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "build:rc-binaries",
        "command": "bash scripts/build-rc-binaries.sh"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "check:supervisor-store",
        "command": "node --import tsx scripts/check-supervisor-store-bypass.mjs"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "release:gate",
        "command": "bash scripts/release-gate.sh"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "runtime",
        "command": "node --import tsx packages/runtime/src/cli.ts"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "runtime:test",
        "command": "pnpm --dir packages/runtime run test"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:claude-agent-task",
        "command": "node --import tsx scripts/claude-agent-task-smoke.ts"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:claude-owner-local-permission",
        "command": "node --import tsx scripts/claude-owner-local-permission-smoke.ts"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:codex-agent-task",
        "command": "node --import tsx scripts/codex-agent-task-smoke.ts"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:default-start",
        "command": "rm -f /tmp/pylon-default-start.log; perl -e 'alarm 3; $ENV{PYLON_DISABLE_OPENCODE_STARTUP}=1; exec @ARGV' node --import tsx src/index.ts > /tmp/pylon-default-start.log 2>&1; code=$?; if [ \"$code\" -ne 142 ] && [ \"$code\" -ne 0 ]; then cat /tmp/pylon-default-start.log; exit \"$code\"; fi; if rg -n 'TypeError|Effect\\.(fork|catchAll)|is not a function|\\[ERROR\\]' /tmp/pylon-default-start.log; then exit 1; fi; printf 'default startup reached persistent mode without startup API errors\\n'"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:fleet-run-live",
        "command": "node --import tsx scripts/fleet-run-live-smoke.ts"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:fleet-run-sustained",
        "command": "node --import tsx scripts/fleet-run-sustained-smoke.ts"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:install:local",
        "command": "bash scripts/smoke-local-package-install.sh"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:live-worker-loop",
        "command": "node --import tsx scripts/live-worker-loop-smoke.ts"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:mixed-harness-fleet-run",
        "command": "node --import tsx scripts/mixed-harness-fleet-run-smoke.ts"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:packaged-network",
        "command": "node --import tsx scripts/packaged-live-network-smoke.ts"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:packaged-runtime-task",
        "command": "node --import tsx scripts/packaged-runtime-task-smoke.ts"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "start",
        "command": "node --import tsx src/index.ts"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "test",
        "command": "vp test --run --root ../.. apps/pylon/scripts/typecheck-tests.test.mjs && pnpm run check:supervisor-store && vp test --run --max-concurrency=1 --root ../.. apps/pylon"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "typecheck:tests:baseline",
        "command": "node scripts/typecheck-tests.mjs"
      },
      {
        "manifest_path": "apps/pylon/package.json",
        "name": "typecheck:tests:update-baseline",
        "command": "node scripts/typecheck-tests.mjs --update-baseline"
      },
      {
        "manifest_path": "apps/pylon/packages/runtime/package.json",
        "name": "runtime",
        "command": "node --import tsx src/cli.ts"
      },
      {
        "manifest_path": "apps/pylon/packages/runtime/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "api",
        "command": "node --import tsx src/daemon.ts --api"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "atif:emit",
        "command": "node --import tsx src/atif-emit.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "build",
        "command": "node --import tsx scripts/build.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "codex:to-atif",
        "command": "node --import tsx src/codex-to-atif.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "compose",
        "command": "node --import tsx src/compose/cli.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "demo:byo",
        "command": "node --import tsx src/byo.ts run --fake-model --url https://example.test --out ./runs/byo-fake"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "demo:khala",
        "command": "node --import tsx src/demo-khala.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "demo:login",
        "command": "node --import tsx src/demo-login.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "evals",
        "command": "node --import tsx src/evals-run.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "khala-sync-once",
        "command": "node --import tsx src/khala-sync-once.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "playwright:install",
        "command": "playwright install --with-deps chromium"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "pr-comment",
        "command": "node --import tsx src/pr-comment-run.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "prepack",
        "command": "node --import tsx scripts/build.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "qa",
        "command": "node --import tsx src/byo.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "qa:dist",
        "command": "node dist/qa.js"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "run-once",
        "command": "node --import tsx src/run-once.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "run-targets",
        "command": "node --import tsx src/run-targets.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "serve",
        "command": "node --import tsx src/daemon.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "terminal-once",
        "command": "node --import tsx src/terminal-once.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "test",
        "command": "vp test --run src/assurance-swarm.test.ts src/byo-model.test.ts src/byo.test.ts src/runner.test.ts src/runner-hardening.test.ts src/timeouts.test.ts src/shard.test.ts src/public-safety.test.ts src/brain.test.ts src/backend.test.ts src/terminal-backend.test.ts src/khala-sync-transport-backend.test.ts src/container-backend.test.ts src/native-desktop-backend.test.ts src/khala-action.test.ts src/khala-driver.test.ts src/khala-config.test.ts src/khala-openrouter.test.ts src/session-trace.test.ts src/distiller.test.ts src/discovery-regression-lifecycle.test.ts src/skill-candidate.test.ts src/receipt.test.ts src/run-settlement.test.ts src/khala-session.test.ts src/compose/build-plan.test.ts src/compose/ffmpeg.test.ts src/evals.test.ts src/pr-comment.test.ts src/control-auth.test.ts src/artifacts.test.ts src/control.test.ts src/api-server.test.ts src/failure-learning.test.ts src/failure-learning-gepa.test.ts src/target-registry.test.ts src/target-registry-run.test.ts src/target-adapter.test.ts src/qs7-rhys-sales-motion.test.ts src/atif.test.ts src/atif-html.test.ts src/codex-to-atif.test.ts src/redaction.test.ts src/claude-code-to-atif.test.ts src/publish-trace.test.ts src/trace-fixture.test.ts src/publish-trace-e2e.verify.test.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "trace:fixture",
        "command": "node --import tsx src/trace-fixture.ts"
      },
      {
        "manifest_path": "apps/qa-runner/package.json",
        "name": "typecheck",
        "command": "tsc --noEmit -p tsconfig.json"
      },
      {
        "manifest_path": "docs/khala/fixtures/artanis-as-a-service-smoke-repo/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "package.json",
        "name": "build",
        "command": "vp run --concurrency-limit 1 -r build"
      },
      {
        "manifest_path": "package.json",
        "name": "changelog",
        "command": "node --import tsx scripts/changelog.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "check",
        "command": "pnpm run fmt:check && vp lint --quiet"
      },
      {
        "manifest_path": "package.json",
        "name": "check:agent-client-protocol",
        "command": "pnpm --dir packages/agent-client-protocol run check:generated"
      },
      {
        "manifest_path": "package.json",
        "name": "check:agent-client-protocol-conformance",
        "command": "pnpm --dir packages/agent-client-protocol-conformance run check:artifacts && pnpm --dir packages/agent-client-protocol-conformance run check:release"
      },
      {
        "manifest_path": "package.json",
        "name": "check:codex-app-server-protocol",
        "command": "pnpm --dir packages/codex-app-server-protocol run check:generated"
      },
      {
        "manifest_path": "package.json",
        "name": "check:deploy",
        "command": "pnpm run check:google-cloud-authority && pnpm run check:sol-docs && pnpm run test:sol-docs && pnpm --dir apps/openagents.com run check:deploy"
      },
      {
        "manifest_path": "package.json",
        "name": "check:fast",
        "command": "vp lint --quiet && pnpm run check:agent-client-protocol && pnpm run check:agent-client-protocol-conformance && pnpm run check:codex-app-server-protocol && node scripts/vp1-retired-money-surface-guard.mjs . && node scripts/zero-supported-bun-guard.mjs . && node scripts/google-cloud-authority-guard.mjs"
      },
      {
        "manifest_path": "package.json",
        "name": "check:fast-follow-spec",
        "command": "pnpm --dir packages/fast-follow-spec run typecheck && pnpm --dir packages/fast-follow-spec run test && pnpm --dir packages/fast-follow-spec run verify:distribution"
      },
      {
        "manifest_path": "package.json",
        "name": "check:google-cloud-authority",
        "command": "node scripts/google-cloud-authority-guard.mjs"
      },
      {
        "manifest_path": "package.json",
        "name": "check:node-vp-freeze",
        "command": "node scripts/node-vp-cutover-inventory.mjs --check"
      },
      {
        "manifest_path": "package.json",
        "name": "check:sol-doc-manifest",
        "command": "node --import tsx scripts/generate-sol-doc-manifest.ts --check"
      },
      {
        "manifest_path": "package.json",
        "name": "check:sol-docs",
        "command": "node --import tsx scripts/check-sol-docs.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "check:vp1-retirement",
        "command": "node scripts/vp1-retired-money-surface-guard.mjs ."
      },
      {
        "manifest_path": "package.json",
        "name": "check:vp2-node-runtime",
        "command": "node scripts/vp2-node-runtime-guard.mjs"
      },
      {
        "manifest_path": "package.json",
        "name": "check:zero-supported-bun",
        "command": "node scripts/zero-supported-bun-guard.mjs ."
      },
      {
        "manifest_path": "package.json",
        "name": "deploy:aiur",
        "command": "pnpm --dir apps/aiur run deploy"
      },
      {
        "manifest_path": "package.json",
        "name": "dev:aiur",
        "command": "pnpm --dir apps/aiur run dev"
      },
      {
        "manifest_path": "package.json",
        "name": "dev:forum",
        "command": "pnpm --dir apps/forum run dev"
      },
      {
        "manifest_path": "package.json",
        "name": "dev:openagents-desktop",
        "command": "pnpm --dir apps/openagents-desktop run dev"
      },
      {
        "manifest_path": "package.json",
        "name": "dev:openagents-mobile",
        "command": "pnpm --dir apps/openagents-mobile run dev"
      },
      {
        "manifest_path": "package.json",
        "name": "dev:openagents.com",
        "command": "pnpm --dir apps/openagents.com run dev"
      },
      {
        "manifest_path": "package.json",
        "name": "dev:pylon",
        "command": "pnpm --dir apps/pylon run start"
      },
      {
        "manifest_path": "package.json",
        "name": "fmt",
        "command": "vp fmt"
      },
      {
        "manifest_path": "package.json",
        "name": "fmt:check",
        "command": "vp fmt --check package.json pnpm-workspace.yaml vite.config.ts '**/package.json' packages/oxlint-plugin-openagents/src"
      },
      {
        "manifest_path": "package.json",
        "name": "generate:sol-doc-manifest",
        "command": "node --import tsx scripts/generate-sol-doc-manifest.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "lint",
        "command": "vp lint --report-unused-disable-directives"
      },
      {
        "manifest_path": "package.json",
        "name": "pack",
        "command": "node --max-old-space-size=8192 scripts/build-public-cli-artifacts.mjs"
      },
      {
        "manifest_path": "package.json",
        "name": "perf:ui-velocity",
        "command": "node --import tsx scripts/ui-velocity-receipt.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "prepare",
        "command": "effect-language-service patch && vp config --no-agent --no-hooks && bash scripts/enable-git-hooks.sh"
      },
      {
        "manifest_path": "package.json",
        "name": "qa:nightly",
        "command": "node --import tsx scripts/qa-nightly-matrix.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "qa:observer",
        "command": "node --import tsx scripts/qa-observer.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "qa:swarm:desktop",
        "command": "pnpm --dir apps/openagents-desktop run qa:visual"
      },
      {
        "manifest_path": "package.json",
        "name": "qa:verify",
        "command": "node --import tsx scripts/qa-verify.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "release",
        "command": "node --import tsx scripts/release.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "release:communicate",
        "command": "node --import tsx scripts/release-communications.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "release:feedback",
        "command": "node --import tsx scripts/release-feedback.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "release:github",
        "command": "node --import tsx scripts/github-release.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "release:impact",
        "command": "node --import tsx scripts/release-impact.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "scan:effect-authority-boundaries",
        "command": "node --import tsx scripts/effect-authority-boundary-scan.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "package.json",
        "name": "test:agent-client-protocol",
        "command": "pnpm --dir packages/agent-client-protocol run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:agent-client-protocol-conformance",
        "command": "pnpm --dir packages/agent-client-protocol-conformance run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:agent-readiness",
        "command": "pnpm --dir packages/agent-readiness run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:agent-runtime-schema",
        "command": "pnpm --dir packages/agent-runtime-schema run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:agent-stdio-transport",
        "command": "pnpm --dir packages/agent-stdio-transport run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:ai-sdk-sandbox-local",
        "command": "pnpm --dir packages/ai-sdk-sandbox-local run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:ai-sdk-sandbox-openagents",
        "command": "pnpm --dir packages/ai-sdk-sandbox-openagents run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:aiur",
        "command": "pnpm --dir apps/aiur run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:arbiter-effect",
        "command": "pnpm --dir packages/arbiter-effect run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:assurance-spec",
        "command": "pnpm --dir packages/assurance-spec run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:audio-contract",
        "command": "pnpm --dir packages/audio-contract run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:authority-delegation",
        "command": "vp test --run scripts/check-authority-delegation.test.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "test:behavior-contracts",
        "command": "pnpm --dir packages/behavior-contracts run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:blueprint-contracts",
        "command": "pnpm --dir packages/blueprint-contracts run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:cloud-contract",
        "command": "pnpm --dir packages/cloud-contract run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:cloud-crates",
        "command": "cargo test --workspace"
      },
      {
        "manifest_path": "package.json",
        "name": "test:codex-app-server-protocol",
        "command": "pnpm --dir packages/codex-app-server-protocol run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:composer-state",
        "command": "pnpm --dir packages/composer-state run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:connector-sidecar",
        "command": "pnpm --dir packages/connector-sidecar run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:durable-stream",
        "command": "pnpm --dir packages/durable-stream run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:effect-boundary",
        "command": "pnpm --dir packages/effect-boundary run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:effect-start",
        "command": "pnpm --dir packages/effect-start run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:environment-auth",
        "command": "pnpm --dir packages/environment-auth run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:fast-follow",
        "command": "vp test --run scripts/check-fast-follow.test.ts && pnpm --dir packages/fast-follow-spec run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:forge-protocol",
        "command": "pnpm --dir packages/forge-protocol run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:forum",
        "command": "pnpm --dir apps/forum run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:github-issue-triage",
        "command": "vp test --run scripts/github-issue-triage.test.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "test:grok-harness",
        "command": "pnpm --dir packages/grok-harness run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:harness-conformance",
        "command": "pnpm --dir packages/harness-conformance run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:input-bindings",
        "command": "pnpm --dir packages/input-bindings run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:khala-ai-sdk-core",
        "command": "pnpm --dir packages/khala-ai-sdk-core run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:khala-capture",
        "command": "pnpm --dir apps/khala-capture run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:khala-fleet-intents",
        "command": "pnpm --dir packages/khala-fleet-intents run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:khala-live-hub",
        "command": "pnpm --dir apps/khala-live-hub run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:khala-qa-harness",
        "command": "pnpm --dir packages/khala-qa-harness run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:khala-sync-db-collection",
        "command": "pnpm --dir packages/khala-sync-db-collection run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:khala-sync-runtime-dogfood-evidence",
        "command": "vp test --run scripts/validate-khala-sync-runtime-dogfood-evidence.test.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "test:khala-tools",
        "command": "pnpm --dir packages/khala-tools run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:mcp-contract",
        "command": "pnpm --dir packages/mcp-contract run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:nip90",
        "command": "pnpm --dir packages/nip90 run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:node-vp-inventory",
        "command": "node --test scripts/node-vp-cutover-inventory.test.mjs"
      },
      {
        "manifest_path": "package.json",
        "name": "test:oa-infra",
        "command": "pnpm --dir packages/oa-infra run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:oa-queue-worker",
        "command": "pnpm --dir apps/oa-queue-worker run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:oa-updates",
        "command": "pnpm --dir apps/oa-updates run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:openagents-audio",
        "command": "pnpm --dir apps/openagents-audio run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:openagents-desktop",
        "command": "pnpm --dir apps/openagents-desktop run verify"
      },
      {
        "manifest_path": "package.json",
        "name": "test:openagents-mobile",
        "command": "pnpm --dir apps/openagents-mobile run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:openagents.com",
        "command": "pnpm --dir apps/openagents.com run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:pipeline-signals",
        "command": "pnpm --dir packages/pipeline-signals run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:portable-session-contract",
        "command": "pnpm --dir packages/portable-session-contract run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:probe",
        "command": "pnpm --dir packages/probe run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:provider-account-schema",
        "command": "pnpm --dir packages/provider-account-schema run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:public-activity-timeline",
        "command": "pnpm --dir packages/public-activity-timeline run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:pylon",
        "command": "pnpm --dir apps/pylon run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:pylon-core",
        "command": "pnpm --dir packages/pylon-core run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:qa-async-gce-trigger",
        "command": "vp test --run scripts/qa-async-gce-trigger.test.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "test:qa-nightly-matrix",
        "command": "vp test --run scripts/qa-nightly-matrix.test.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "test:qa-pre-push-smoke",
        "command": "vp test --run scripts/qa-pre-push-smoke.test.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "test:qa-runner",
        "command": "pnpm --dir apps/qa-runner run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:qa-visual-smoke-gate",
        "command": "vp test --run scripts/qa-visual-smoke-gate.test.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "test:reactor-contracts",
        "command": "pnpm --dir packages/reactor-contracts run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:retired-clients",
        "command": "vp test --run scripts/retired-clients-removal.test.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "test:sol-docs",
        "command": "vp test --run scripts/check-sol-docs.test.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "test:sqlite-runtime",
        "command": "pnpm --dir packages/sqlite-runtime run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:ui-velocity-receipt",
        "command": "vp test --run scripts/ui-velocity-receipt.test.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "test:vp2-node",
        "command": "node --test packages/runtime-platform/src/runtime-platform.node-suite.ts packages/sqlite-runtime/src/node-database.node-suite.ts scripts/public-cli-artifacts.node.test.mjs scripts/vp2-node-runtime-guard.test.mjs scripts/vp2-retained-service.node.test.mjs"
      },
      {
        "manifest_path": "package.json",
        "name": "test:world-client",
        "command": "pnpm --dir packages/world-client run test"
      },
      {
        "manifest_path": "package.json",
        "name": "test:world-contract",
        "command": "pnpm --dir packages/world-contract run test"
      },
      {
        "manifest_path": "package.json",
        "name": "triage:issues",
        "command": "node --import tsx scripts/github-issue-triage.ts"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck",
        "command": "vp run --concurrency-limit 2 --filter './**' --filter '!./packages/probe' --filter '!./packages/probe/**' typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:agent-client-protocol",
        "command": "pnpm --dir packages/agent-client-protocol run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:agent-client-protocol-conformance",
        "command": "pnpm --dir packages/agent-client-protocol-conformance run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:agent-readiness",
        "command": "pnpm --dir packages/agent-readiness run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:agent-runtime-schema",
        "command": "pnpm --dir packages/agent-runtime-schema run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:agent-stdio-transport",
        "command": "pnpm --dir packages/agent-stdio-transport run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:ai-sdk-sandbox-local",
        "command": "pnpm --dir packages/ai-sdk-sandbox-local run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:ai-sdk-sandbox-openagents",
        "command": "pnpm --dir packages/ai-sdk-sandbox-openagents run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:arbiter-effect",
        "command": "pnpm --dir packages/arbiter-effect run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:assurance-spec",
        "command": "pnpm --dir packages/assurance-spec run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:audio-contract",
        "command": "pnpm --dir packages/audio-contract run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:behavior-contracts",
        "command": "pnpm --dir packages/behavior-contracts run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:blueprint-contracts",
        "command": "pnpm --dir packages/blueprint-contracts run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:codex-app-server-protocol",
        "command": "pnpm --dir packages/codex-app-server-protocol run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:composer-state",
        "command": "pnpm --dir packages/composer-state run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:connector-sidecar",
        "command": "pnpm --dir packages/connector-sidecar run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:durable-stream",
        "command": "pnpm --dir packages/durable-stream run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:effect-boundary",
        "command": "pnpm --dir packages/effect-boundary run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:effect-start",
        "command": "pnpm --dir packages/effect-start run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:environment-auth",
        "command": "pnpm --dir packages/environment-auth run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:forge-protocol",
        "command": "pnpm --dir packages/forge-protocol run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:forum",
        "command": "pnpm --dir apps/forum run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:grok-harness",
        "command": "pnpm --dir packages/grok-harness run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:harness-conformance",
        "command": "pnpm --dir packages/harness-conformance run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:input-bindings",
        "command": "pnpm --dir packages/input-bindings run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:khala-ai-sdk-core",
        "command": "pnpm --dir packages/khala-ai-sdk-core run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:khala-fleet-intents",
        "command": "pnpm --dir packages/khala-fleet-intents run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:khala-qa-harness",
        "command": "pnpm --dir packages/khala-qa-harness run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:khala-sync-db-collection",
        "command": "pnpm --dir packages/khala-sync-db-collection run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:khala-tools",
        "command": "pnpm --dir packages/khala-tools run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:mcp-contract",
        "command": "pnpm --dir packages/mcp-contract run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:nip90",
        "command": "pnpm --dir packages/nip90 run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:oa-infra",
        "command": "pnpm --dir packages/oa-infra run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:oa-queue-worker",
        "command": "pnpm --dir apps/oa-queue-worker run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:oa-updates",
        "command": "pnpm --dir apps/oa-updates run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:openagents-audio",
        "command": "pnpm --dir apps/openagents-audio run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:pipeline-signals",
        "command": "pnpm --dir packages/pipeline-signals run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:portable-session-contract",
        "command": "pnpm --dir packages/portable-session-contract run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:provider-account-schema",
        "command": "pnpm --dir packages/provider-account-schema run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:public-activity-timeline",
        "command": "pnpm --dir packages/public-activity-timeline run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:reactor-contracts",
        "command": "pnpm --dir packages/reactor-contracts run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:sqlite-runtime",
        "command": "pnpm --dir packages/sqlite-runtime run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:world-client",
        "command": "pnpm --dir packages/world-client run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "typecheck:world-contract",
        "command": "pnpm --dir packages/world-contract run typecheck"
      },
      {
        "manifest_path": "package.json",
        "name": "verify:tla",
        "command": "specs/run-tlc.sh"
      },
      {
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "check:artifacts",
        "command": "node --import tsx scripts/generate-artifacts.ts --check"
      },
      {
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "check:release",
        "command": "node --import tsx scripts/check-release-matrix.ts"
      },
      {
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "generate",
        "command": "node --import tsx scripts/generate-artifacts.ts"
      },
      {
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "live:cursor",
        "command": "node --import tsx scripts/live-probe.ts cursor"
      },
      {
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "live:grok",
        "command": "node --import tsx scripts/live-probe.ts grok"
      },
      {
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "live:release",
        "command": "node --import tsx scripts/live-release-suite.ts"
      },
      {
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "report",
        "command": "node --import tsx scripts/run-conformance-report.ts"
      },
      {
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "test",
        "command": "vp test --run packages/agent-client-protocol-conformance/src/conformance.test.ts packages/agent-client-protocol-conformance/src/release-evidence.test.ts"
      },
      {
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/agent-client-protocol/package.json",
        "name": "check:generated",
        "command": "node scripts/check-generated.ts"
      },
      {
        "manifest_path": "packages/agent-client-protocol/package.json",
        "name": "generate",
        "command": "node scripts/generate.ts"
      },
      {
        "manifest_path": "packages/agent-client-protocol/package.json",
        "name": "test",
        "command": "vp test --run packages/agent-client-protocol/src/protocol.test.ts packages/agent-client-protocol/src/profiles/profiles.test.ts"
      },
      {
        "manifest_path": "packages/agent-client-protocol/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/agent-client-protocol/package.json",
        "name": "update:upstream",
        "command": "node scripts/update-upstream.ts"
      },
      {
        "manifest_path": "packages/agent-client-runtime-bridge/package.json",
        "name": "test",
        "command": "vp test --run packages/agent-client-runtime-bridge/src"
      },
      {
        "manifest_path": "packages/agent-client-runtime-bridge/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/agent-readiness/package.json",
        "name": "test",
        "command": "vp test --root ../.. --run --project node packages/agent-readiness/src/index.test.ts"
      },
      {
        "manifest_path": "packages/agent-readiness/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/agent-runtime-schema/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/agent-runtime-schema/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/agent-stdio-transport/package.json",
        "name": "test",
        "command": "vp test --run packages/agent-stdio-transport/src/transport.test.ts"
      },
      {
        "manifest_path": "packages/agent-stdio-transport/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/ai-sdk-sandbox-local/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/ai-sdk-sandbox-local/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/ai-sdk-sandbox-openagents/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/ai-sdk-sandbox-openagents/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/arbiter-effect/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/arbiter-effect/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/assurance-spec/package.json",
        "name": "assure:mvp",
        "command": "node --import tsx scripts/run-mvp-assurance.ts"
      },
      {
        "manifest_path": "packages/assurance-spec/package.json",
        "name": "pack:public",
        "command": "node --import tsx scripts/pack-public.ts"
      },
      {
        "manifest_path": "packages/assurance-spec/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/assurance-spec/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/assurance-spec/package.json",
        "name": "verify:distribution",
        "command": "node --import tsx scripts/verify-distribution.ts"
      },
      {
        "manifest_path": "packages/atif/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/atif/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/audio-contract/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/audio-contract/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/authority/package.json",
        "name": "test",
        "command": "vp test --run src/*.test.ts"
      },
      {
        "manifest_path": "packages/authority/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/autopilot-control-protocol/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/autopilot-control-protocol/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/behavior-contracts/package.json",
        "name": "test",
        "command": "vp test --run src/*.test.ts"
      },
      {
        "manifest_path": "packages/behavior-contracts/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/blueprint-contracts/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/blueprint-contracts/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/cloud-contract/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/cloud-contract/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/codex-app-server-protocol/package.json",
        "name": "check:generated",
        "command": "node scripts/check-generated.ts"
      },
      {
        "manifest_path": "packages/codex-app-server-protocol/package.json",
        "name": "fixtures:generate",
        "command": "node scripts/generate-notification-fixtures.ts"
      },
      {
        "manifest_path": "packages/codex-app-server-protocol/package.json",
        "name": "generate",
        "command": "node scripts/generate-all.ts"
      },
      {
        "manifest_path": "packages/codex-app-server-protocol/package.json",
        "name": "smoke:binary",
        "command": "node scripts/real-binary-smoke.ts"
      },
      {
        "manifest_path": "packages/codex-app-server-protocol/package.json",
        "name": "test",
        "command": "vp test --run packages/codex-app-server-protocol/src/protocol.test.ts"
      },
      {
        "manifest_path": "packages/codex-app-server-protocol/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/composer-state/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/composer-state/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/connector-sidecar/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/connector-sidecar/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/cursor-agent-runtime/package.json",
        "name": "live-smoke",
        "command": "node --import tsx scripts/live-smoke.ts"
      },
      {
        "manifest_path": "packages/cursor-agent-runtime/package.json",
        "name": "test",
        "command": "vp test --run packages/cursor-agent-runtime/src"
      },
      {
        "manifest_path": "packages/cursor-agent-runtime/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/durable-stream/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/durable-stream/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/effect-boundary/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/effect-boundary/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/effect-start/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/effect-start/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/environment-auth/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/environment-auth/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/fast-follow-spec/package.json",
        "name": "test",
        "command": "vp test --run packages/fast-follow-spec/test/fast-follow-spec.test.ts packages/fast-follow-spec/test/manifest.test.ts"
      },
      {
        "manifest_path": "packages/fast-follow-spec/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/fast-follow-spec/package.json",
        "name": "verify:distribution",
        "command": "node --import tsx scripts/verify-distribution.ts"
      },
      {
        "manifest_path": "packages/forge-protocol/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/forge-protocol/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/grok-harness/package.json",
        "name": "live-acp-smoke",
        "command": "node --import tsx scripts/live-acp-smoke.ts"
      },
      {
        "manifest_path": "packages/grok-harness/package.json",
        "name": "mock-acp",
        "command": "node --import tsx scripts/mock-acp-stdio.ts"
      },
      {
        "manifest_path": "packages/grok-harness/package.json",
        "name": "rl-probe",
        "command": "node --import tsx scripts/rl-probe.ts"
      },
      {
        "manifest_path": "packages/grok-harness/package.json",
        "name": "rl4-worktree-probe",
        "command": "node --import tsx scripts/rl4-worktree-probe.ts"
      },
      {
        "manifest_path": "packages/grok-harness/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/grok-harness/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/harness-conformance/package.json",
        "name": "test",
        "command": "vp test --run src/*.test.ts"
      },
      {
        "manifest_path": "packages/harness-conformance/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/input-bindings/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/input-bindings/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/khala-ai-sdk-core/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/khala-ai-sdk-core/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/khala-fleet-intents/package.json",
        "name": "test",
        "command": "vp test --run src/*.test.ts"
      },
      {
        "manifest_path": "packages/khala-fleet-intents/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "lag:sweep",
        "command": "node --import tsx src/lag-profiling-sweep.ts"
      },
      {
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "monkey:night",
        "command": "node --import tsx src/monkey-night.ts --runs 100 --steps 64"
      },
      {
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "monkey:smoke",
        "command": "vp test --run src/monkey-explorer.test.ts --test-name-pattern 'bounded fixture smoke'"
      },
      {
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "smoke:architect-coder-judge-live",
        "command": "node --import tsx src/architect-coder-judge-live-smoke.ts"
      },
      {
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "test",
        "command": "vp test --run src/*.test.ts"
      },
      {
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/khala-sync-client/package.json",
        "name": "check:test-import-coverage",
        "command": "node scripts/check-test-import-coverage.mjs"
      },
      {
        "manifest_path": "packages/khala-sync-client/package.json",
        "name": "test",
        "command": "vp test --root ../.. --run --project node packages/khala-sync-client/src && node scripts/check-test-import-coverage.mjs"
      },
      {
        "manifest_path": "packages/khala-sync-client/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/khala-sync-db-collection/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/khala-sync-db-collection/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "capture",
        "command": "node --import tsx scripts/capture.ts"
      },
      {
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "check:pending-migrations",
        "command": "node --import tsx scripts/check-pending-migrations.ts"
      },
      {
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "compact",
        "command": "node --import tsx scripts/compact.ts"
      },
      {
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "load-test",
        "command": "node --import tsx scripts/load-test.ts"
      },
      {
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "migrate",
        "command": "node --import tsx scripts/migrate.ts"
      },
      {
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "test:pending-migrations-guard",
        "command": "vp test --run scripts/check-pending-migrations.test.ts"
      },
      {
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit && tsc -p tsconfig.port03-production-driver.json --noEmit"
      },
      {
        "manifest_path": "packages/khala-sync/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/khala-sync/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/khala-tools/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/khala-tools/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/khala-tools/package.json",
        "name": "typecheck:tsgo",
        "command": "tsgo -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/khala-tools/package.json",
        "name": "typecheck:tsgo:patch",
        "command": "effect-tsgo patch"
      },
      {
        "manifest_path": "packages/mcp-contract/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/mcp-contract/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/nip90/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/nip90/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/oa-infra/package.json",
        "name": "migrate",
        "command": "node --import tsx scripts/migrate.ts"
      },
      {
        "manifest_path": "packages/oa-infra/package.json",
        "name": "test",
        "command": "vp test --run src"
      },
      {
        "manifest_path": "packages/oa-infra/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/oxlint-plugin-openagents/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/oxlint-plugin-openagents/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/pipeline-signals/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/pipeline-signals/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/portable-session-contract/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/portable-session-contract/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/postgres-runtime/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/probe/package.json",
        "name": "test",
        "command": "pnpm --dir packages/runtime run test"
      },
      {
        "manifest_path": "packages/probe/package.json",
        "name": "typecheck",
        "command": "npm --prefix packages/runtime run typecheck"
      },
      {
        "manifest_path": "packages/probe/packages/runtime/package.json",
        "name": "probe",
        "command": "node --import tsx src/cli.ts"
      },
      {
        "manifest_path": "packages/probe/packages/runtime/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/probe/packages/runtime/package.json",
        "name": "typecheck",
        "command": "node scripts/verify-test-typecheck.mjs"
      },
      {
        "manifest_path": "packages/probe/packages/runtime/package.json",
        "name": "typecheck:baseline:update",
        "command": "node scripts/verify-test-typecheck.mjs --update-baseline"
      },
      {
        "manifest_path": "packages/product-spec/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/product-spec/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/provider-account-schema/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/provider-account-schema/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/public-activity-timeline/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/public-activity-timeline/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/pylon-core/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/pylon-core/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/qa-swarm-contract/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/qa-swarm-contract/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/reactor-contracts/package.json",
        "name": "smoke:dogfood",
        "command": "node --import tsx scripts/dogfood-smoke.ts"
      },
      {
        "manifest_path": "packages/reactor-contracts/package.json",
        "name": "smoke:install",
        "command": "node --import tsx scripts/install-smoke.ts"
      },
      {
        "manifest_path": "packages/reactor-contracts/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/reactor-contracts/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/runtime-platform/package.json",
        "name": "test:node",
        "command": "node --test src/runtime-platform.node-suite.ts"
      },
      {
        "manifest_path": "packages/runtime-platform/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/sarah/package.json",
        "name": "test",
        "command": "vp test --run src/*.test.ts"
      },
      {
        "manifest_path": "packages/sarah/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/sqlite-runtime/package.json",
        "name": "test",
        "command": "vp test --run && pnpm run test:node"
      },
      {
        "manifest_path": "packages/sqlite-runtime/package.json",
        "name": "test:node",
        "command": "node --test src/node-database.node-suite.ts"
      },
      {
        "manifest_path": "packages/sqlite-runtime/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/tassadar-executor/package.json",
        "name": "replay",
        "command": "node --import tsx src/replay-cli.ts"
      },
      {
        "manifest_path": "packages/tassadar-executor/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/tassadar-executor/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/ui/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/ui/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/world-client/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/world-client/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      },
      {
        "manifest_path": "packages/world-contract/package.json",
        "name": "test",
        "command": "vp test --run"
      },
      {
        "manifest_path": "packages/world-contract/package.json",
        "name": "typecheck",
        "command": "tsc -p tsconfig.json --noEmit"
      }
    ],
    "truncated": true,
    "diagnostics": [
      "candidate_artifact_inventory_truncated",
      "repository_candidates_unmapped"
    ],
    "inventory_digest": "sha256:c8b2d01ec6d7b363f7326244a0cfec32d44cb75049e315d28cbd60cd78dfddbd",
    "candidates_not_proof": true
  }
}
```

## Obligations

All 76 criteria carry a fully DESIGNED obligation in this revision (domains, technique, environment_refs, oracle, falsifier, evidence, independence, and activation_gate all present). Rev 3's 31 designed obligations are preserved and the former 45-item backlog is now reconciled against ProductSpec rev 14. No criterion remains in a silent `needs_design` bucket. Design readiness remains separate from execution and admission: MemoHarness obligations name executable local contract/model oracles but remain unobserved until their candidate suite runs, and release-grade obligations remain gated at their exact environment rung. No designed obligation in this revision claims the `seam` domain: every cited evidence file is in-process (real production modules composed together, or a real in-process Effect HTTP server) rather than a genuine two-real-process wire connection with independently qualifying evidence on both sides, so claiming `domains: ["seam"]` here would trip `false_green_mocked_seam` honestly. The two-OS-process `smoke:full-auto-restart` harness is real cross-process evidence but is modeled under the `resilience` technique / `ENV-FA-DEV-TWO-PROCESS-1` environment rather than `seam`, because its in-app probe mode makes it a smoke fixture, not two independently-authored real sides meeting at an ordinary production boundary.

```assurancespec-obligations
[
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/react-composer.test.tsx"
    ],
    "criterion_refs": [
      "FA-AC-01"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-01-01",
    "source_claim_digest": "sha256:5249b07e544314383070e43f5b8552b924fa4715fe5e32ca7c294ae802e59a1b",
    "source_claim_snapshot": "The composer renders exactly one `Full Auto` toggle\n(`shell-full-auto-toggle`), off by default, with `aria-pressed` reflecting\nstate. No other new screen or review surface ships with this spec.\nProof: `react-composer.test.tsx` \"Full Auto (#8852): renders as an\noff-by-default composer toggle and reports DesktopFullAutoToggled\".",
    "title": "Assure FA-AC-01",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_01_negative_control",
      "ref": "apps/openagents-desktop/src/renderer/react-composer.test.tsx"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
      "statement": "The exact FA-AC-01 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/codex-local-runtime.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-02"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-02-01",
    "source_claim_digest": "sha256:cb9866496df8854341bd1f37fd970b32ec998cc714b87d18f1b75b433531c4dd",
    "source_claim_snapshot": "A Codex-lane turn started with Full Auto on sends\n`approvalPolicy: \"never\"` on both `thread/start` and `turn/start`, and its\nprompt is prefixed with the Full Auto instruction; an ordinary turn keeps\n`approvalPolicy: \"on-request\"` and an unprefixed prompt.\nProof: `codex-local-runtime.test.ts` \"Full Auto (#8852) forces\napprovalPolicy never and prefixes the turn prompt...\" and \"an ordinary\n(non-Full-Auto) app-server turn keeps approvalPolicy on-request...\".",
    "title": "Assure FA-AC-02",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_02_negative_control",
      "ref": "apps/openagents-desktop/src/codex-local-runtime.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/codex-local-runtime.test.ts",
      "statement": "The exact FA-AC-02 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/shell.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-03"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-03-01",
    "source_claim_digest": "sha256:13add3dccc6a5307434b19ee3920e3e6bc545427e15b5f7178a6f1a743af136d",
    "source_claim_snapshot": "A completed Full-Auto turn sends `fullAuto: true`\nexactly once from the renderer; the renderer never loops. Continuation is\ndecided in main by `reconcileFullAutoThreads`, called both right after that\nturn completes and once at app startup.\nProof: `shell.test.ts` \"a flagged turn sends fullAuto:true exactly once --\nmain, not the renderer, decides whether to continue\"; `main.ts`'s\n`dispatchCodexLocalTurn` calling `runFullAutoReconciliation()` after a\nsuccessful Full-Auto turn (code-reviewed; main.ts has no direct unit-test\nharness, see Receipts for the isolated-module proof used instead).",
    "title": "Assure FA-AC-03",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_03_negative_control",
      "ref": "apps/openagents-desktop/src/renderer/shell.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/shell.test.ts",
      "statement": "The exact FA-AC-03 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-04"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-04-01",
    "source_claim_digest": "sha256:c629cf55289ec3cb3f95bc3dab1527f5d37c1c83cc140663d7786463bed06f90",
    "source_claim_snapshot": "Toggling Full Auto off persists to main immediately\n(`CodexLocalFullAutoSetChannel`), independent of whether a turn is in\nflight, so a toggle-off durably stops the loop even if the app quits before\nthe next turn would have started.\nProof: `shell.test.ts` \"DesktopFullAutoToggled flips the flag and persists\nit to main immediately\"; `full-auto-restart.e2e.test.ts` \"toggling off\nbefore restart durably stops it -- Runtime B never dispatches\".",
    "title": "Assure FA-AC-04",
    "activation_gate": "GATE-DEV-TWO-PROCESS",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-DEV-TWO-PROCESS-1"
    ],
    "evidence": {
      "proof_rung": "local_dev_two_process_unsigned",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_04_negative_control",
      "ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts",
      "statement": "The exact FA-AC-04 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_e2e"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/shell.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-05"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-05-01",
    "source_claim_digest": "sha256:da1b0266659d27ba0851e38614447a6334690794861843f6068c51b2fc57c224",
    "source_claim_snapshot": "When Full Auto is off, an ordinary turn sends `fullAuto`\nundefined (not `false`) and never resubmits automatically.\nProof: `shell.test.ts` \"toggled off, an ordinary Codex turn sends fullAuto\nundefined and never resubmits\".",
    "title": "Assure FA-AC-05",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_05_negative_control",
      "ref": "apps/openagents-desktop/src/renderer/shell.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/shell.test.ts",
      "statement": "The exact FA-AC-05 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-registry.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-06"
    ],
    "disposition": "required",
    "domains": [
      "exactly_once",
      "cap_semantics"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "invalid_reset_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-registry.test.ts"
    },
    "id": "AO-FA-AC-06-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-registry.test.ts",
      "statement": "20 consecutive continuations self-disable durably; only a successful dispatch increments the counter; the counter resets only on an explicit toggle-off."
    },
    "source_claim_digest": "sha256:d170e89e8e52dee7023aa1809393d65ee48fcc60d556617a98328578342286ec",
    "source_claim_snapshot": "A run of 20 consecutive automatic continuations turns Full\nAuto off durably (registry, not renderer state) and appends an explanatory\nsystem note, rather than continuing unbounded -- and this holds even if a\nrestart happens partway through the count. The consecutive-continuation\ncounter resets only when Full Auto is toggled off for that thread; a manual\nsend while the toggle stays on does NOT reset it, and re-enabling an\nalready-enabled thread preserves the count. Since rev 4 the counter\nincrements only on a SUCCESSFUL dispatch: a failed dispatch consumes\nfailure/backoff budget (FA-AC-16), never a cap slot.\nProof: `full-auto-restart.e2e.test.ts` \"a genuinely stuck loop self-disables\nat the continuation cap across restarts, rather than continuing unbounded\"\nand \"failed dispatches never consume cap slots: fail once then succeed ->\ncontinuationCount is exactly 1\"; `full-auto-registry.test.ts`\n\"continuationCount resets ONLY on toggle-off: a manual send leaves it\nunchanged; off-then-on zeroes it\".",
    "technique": "unit",
    "title": "Assure FA-AC-06"
  },
  {
    "activation_gate": "GATE-DEV-TWO-PROCESS",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/scripts/full-auto-restart-smoke.ts",
      "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-07"
    ],
    "disposition": "required",
    "domains": [
      "resume",
      "restart_recovery"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1",
      "ENV-FA-DEV-TWO-PROCESS-1"
    ],
    "evidence": {
      "proof_rung": "local_dev_two_process_unsigned",
      "required_kinds": [
        "execution_trace",
        "two_process_smoke_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "no_resume_after_relaunch_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    },
    "id": "AO-FA-AC-07-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/scripts/full-auto-restart-smoke.ts",
      "statement": "A thread left enabled with no turn in flight resumes automatically on next launch. Two-OS-process evidence exists via smoke:full-auto-restart's happy-path seed/resume phase pair, but that script conditionally falls back to unsigned dev-mode Electron when no local Forge package exists (RISK-FA-10) -- cited here at that real tier, not as a signed-build proof."
    },
    "source_claim_digest": "sha256:2254ce441c65d583c1a4cbf7940ad440914370d88a5d733b13a633131878a801",
    "source_claim_snapshot": "A thread left enabled with no turn in flight when\nthe app quits resumes its next continuation on its own at the next launch,\nwith no user action beyond the original toggle.\nProof: `full-auto-restart.e2e.test.ts` \"a thread left enabled by Runtime A\nresumes on Runtime B with no manual re-toggle or re-send\".",
    "technique": "resilience",
    "title": "Assure FA-AC-07"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-08"
    ],
    "disposition": "required",
    "domains": [
      "resume",
      "restart_recovery"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1",
      "ENV-FA-DEV-TWO-PROCESS-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "in_flight_double_dispatch_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    },
    "id": "AO-FA-AC-08-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts",
      "statement": "A thread with a turn in flight at quit is left alone by Full Auto until interrupted-turn recovery resolves it -- never double-dispatched into on relaunch."
    },
    "source_claim_digest": "sha256:cb34dc2b34869318843b7a396111fdaaea0afed8ea1b1a8d5f51138c7bd84524",
    "source_claim_snapshot": "A thread whose turn was still in flight when the\napp quit is left alone by Full Auto reconciliation until existing\ninterrupted-turn recovery resolves it -- Full Auto never races or\nduplicates that recovery.\nProof: `full-auto-restart.e2e.test.ts` \"a thread with a turn still in\nflight at restart is left alone until that turn resolves\"; the real\nwiring sequences `runFullAutoReconciliation()` after `localTurnRecovery`\nresolves, and computes `nonterminalThreadRefs` from the same\n`localTurnJournal.nonterminal()` that recovery itself owns.",
    "technique": "resilience",
    "title": "Assure FA-AC-08"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/shell.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-09"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-09-01",
    "source_claim_digest": "sha256:e0928b54f48543be4384febe0697ca5995463691bab1060e1537e2bf3b7406d2",
    "source_claim_snapshot": "A brand new thread (no id yet when the user\ntoggles Full Auto on) persists its enabled state to main once it actually\ngets a real thread id, rather than silently dropping the toggle's intent.\nProof: `shell.test.ts` \"a brand new thread persists its enabled state to\nmain once it has a real id\".",
    "title": "Assure FA-AC-09",
    "activation_gate": "GATE-DEV-TWO-PROCESS",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-DEV-TWO-PROCESS-1"
    ],
    "evidence": {
      "proof_rung": "local_dev_two_process_unsigned",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_09_negative_control",
      "ref": "apps/openagents-desktop/src/renderer/shell.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/shell.test.ts",
      "statement": "The exact FA-AC-09 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_e2e"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-lane.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-10"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-10-01",
    "source_claim_digest": "sha256:d1daf49b0dffa4a3db43527a9d08ceac7b8c450c367d16f34c3d54fa6b5b66f7",
    "source_claim_snapshot": "No Full Auto packet performs a direct commit, merge, or push;\nCodex proposes changes exactly as every other Desktop Codex turn already\ndoes. (Unchanged existing boundary; no new authority was added.)",
    "title": "Assure FA-AC-10",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_10_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-lane.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-lane.test.ts",
      "statement": "The exact FA-AC-10 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-registry.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-11"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-11-01",
    "source_claim_digest": "sha256:c4801546b6e401d8e1f127f2d3bb0c94521df237aab304b8f0f84f9b5c17e6ca",
    "source_claim_snapshot": "A corrupt or schema-invalid registry file never\nblocks Desktop main initialization. Opening it fails closed for the feature\nand open for the app: the bad file is quarantined beside the registry\n(best-effort rename to `registry.json.quarantined-<ISO timestamp>` with an\nowner-visible console diagnostic naming the quarantine path), the registry\nstarts empty (Full Auto disabled for all threads), and subsequent writes\npersist normally.\nProof: `full-auto-registry.test.ts` \"a corrupt registry file is quarantined\nand the registry opens empty instead of throwing\" and \"a schema-invalid (but\nvalid JSON) registry file is also quarantined rather than thrown\".",
    "title": "Assure FA-AC-11",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_11_negative_control",
      "ref": "apps/openagents-desktop/tests/full-auto-registry.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-registry.test.ts",
      "statement": "The exact FA-AC-11 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-registry.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-12"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-12-01",
    "source_claim_digest": "sha256:27450f70ca5382f43098d7df09b20a74f7ecd2370302593d44cce1f89f8a9564",
    "source_claim_snapshot": "Registry record eviction never drops an\n`enabled: true` record. All enabled records are kept; only the disabled tail\nis bounded, filling remaining capacity (up to 128 total) with the\nmost-recently-updated disabled records. An owner-enabled thread therefore\nalways survives to the next restart, no matter how many other records were\ntouched more recently.\nProof: `full-auto-registry.test.ts` \"eviction never drops an enabled record:\nthe oldest enabled thread survives while old disabled records are evicted\".",
    "title": "Assure FA-AC-12",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_12_negative_control",
      "ref": "apps/openagents-desktop/tests/full-auto-registry.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-registry.test.ts",
      "statement": "The exact FA-AC-12 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-13"
    ],
    "disposition": "required",
    "domains": [
      "fail_closed",
      "workspace_binding"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "workspace_mismatch_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    },
    "id": "AO-FA-AC-13-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts",
      "statement": "Enable binds the resolved workspace; a continuation resolved against a mismatched workspace never dispatches and blocks with the typed reason workspace_mismatch."
    },
    "source_claim_digest": "sha256:0c0fc5b541489fe29a468ba48824663fad47343b2d0e61639f918b8e0d80915a",
    "source_claim_snapshot": "Enabling Full Auto binds the currently resolved workspace onto\nthe durable record -- resolved by main from the exact same source of truth\ncodex-local turns execute against, never a renderer-supplied path. A\ncontinuation whose currently-resolved workspace differs from the recorded\nbinding does NOT dispatch: the record is disabled with\n`blockedReason: \"workspace_mismatch\"` and an owner-visible system note\nexplains that Full Auto was turned off because the granted workspace no\nlonger matches.\nProof: `full-auto-restart.e2e.test.ts` \"enable on workspace A, resolve\nworkspace B at reconcile -> no dispatch, record disabled with\nworkspace_mismatch, block reported\"; `main.ts` binds via\n`resolveDesktopLocalWorkspaceRoot()` in the `CodexLocalFullAutoSetChannel`\nhandler and passes the same resolver into reconciliation (code-reviewed;\nmain.ts has no direct unit-test harness).",
    "technique": "unit",
    "title": "Assure FA-AC-13"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-14"
    ],
    "disposition": "required",
    "domains": [
      "fail_closed",
      "workspace_binding"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "unbound_workspace_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    },
    "id": "AO-FA-AC-14-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts",
      "statement": "A pre-upgrade v1 record with no workspace binding fails closed with the typed reason workspace_unbound rather than dispatching into an unresolved workspace."
    },
    "source_claim_digest": "sha256:d2d82d33d573b2bc82414f00aaa1866434b3f2c15b624f36da6cdc1eb9bfcb36",
    "source_claim_snapshot": "An enabled record with NO recorded workspace (a pre-upgrade v1\nrow) fails CLOSED at dispatch: it is never silently adopted onto the current\nworkspace -- the record is disabled with\n`blockedReason: \"workspace_unbound\"` and an owner-visible note. The binding\nis (re)established only by a successful ENABLE, which always records the\nthen-current workspace.\nProof: `full-auto-restart.e2e.test.ts` \"an enabled record with NO workspace\nbinding (pre-upgrade v1 row) fails CLOSED: no dispatch, disabled with\nworkspace_unbound\".",
    "technique": "unit",
    "title": "Assure FA-AC-14"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-15"
    ],
    "disposition": "required",
    "domains": [
      "exactly_once",
      "concurrency"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "overlapping_reconcile_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    },
    "id": "AO-FA-AC-15-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts",
      "statement": "Two overlapping reconcile passes against the same thread dispatch exactly once via the durable per-thread lease keyed to the turn ref; only the STARTUP pass clears a stale lease."
    },
    "source_claim_digest": "sha256:fa5c6e534d0a1fed8ff7e5ef7b6b9970b28b6442bfedbc401f5691fe09a115ff",
    "source_claim_snapshot": "Continuation dispatch is exactly-once. All reconciliation\ntriggers in main serialize through a promise-chain mutex, and before\ndispatching a thread the reconciler durably claims a per-thread lease\ncarrying the exact continuation turn ref (the lease identity and the\ndispatched turn identity are the same value). Two overlapping reconcile\npasses dispatch an enabled thread at most once. The lease releases on\ndispatch completion (success or failure). Only the STARTUP pass clears a\nstale lease -- one whose turn ref has no nonterminal local-turn journal row\n(a dispatch that crashed before its turn was accepted); a mid-session pass\ntreats a held lease as in-flight and skips. As defense in depth, main's\ndispatch adapter refuses to start a continuation when the local-turn\njournal already holds a nonterminal turn on that thread.\nProof: `full-auto-restart.e2e.test.ts` \"audit probe (a): two overlapping\nreconcile passes against one enabled thread dispatch it exactly ONCE\n(durable lease), and continuationCount increments by exactly 1\", \"the\nserial task queue serializes overlapping reconciliation triggers...\", \"a\nstale lease (crashed mid-dispatch: no journal row for its turn ref) is\ncleared ONLY by the startup pass...\", and \"a lease whose turn IS still\nnonterminal in the journal is NOT cleared at startup...\";\n`full-auto-registry.test.ts` \"claimPending holds the lease exactly once\nuntil cleared; a missing record can never be claimed\".",
    "technique": "component",
    "title": "Assure FA-AC-15"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-retry-rotation-model.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-16"
    ],
    "disposition": "required",
    "domains": [
      "exactly_once",
      "retry_backoff"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "exhaustive_enumeration_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "broken_backoff_schedule_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-retry-rotation-model.test.ts"
    },
    "id": "AO-FA-AC-16-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-retry-rotation-model.test.ts",
      "statement": "A failed dispatch is typed/visible; bounded exponential backoff min(2^n*30s,30min) is exhaustively monotonic-nondecreasing and bounded over failures 0..11 against the real exported fullAutoFailureBackoffMs; the 5th consecutive failure disables durably."
    },
    "source_claim_digest": "sha256:12427f5bc832f0d4b1a77615435d03c066a911354893db08f228bec1aeccaa3e",
    "source_claim_snapshot": "A failed continuation dispatch -- thrown OR `{ ok: false }` --\nis a typed, owner-visible outcome, never a silently dormant enabled record.\nFailure persists `consecutiveFailures`, `lastFailureAt`, and a bounded\n`blockedReason` on the record, releases the lease, and appends an\nowner-visible system note. Retries respect bounded exponential backoff:\ndispatch is skipped while the record is within\n`min(2^consecutiveFailures * 30s, 30min)` of `lastFailureAt`. The 5th\nconsecutive failure disables the record durably (with the failure reason as\n`blockedReason`) and a final note says so. A successful dispatch clears all\nfailure state.\nProof: `full-auto-restart.e2e.test.ts` \"audit probe (b): an { ok: false }\ndispatch is a typed, visible failure...\", \"a thrown dispatch is the same\ntyped failure outcome as ok:false\", \"the bounded backoff window skips\ndispatch after a failure, then allows it once the window has passed\", and\n\"the 5th consecutive failure disables the record with a blockedReason and\nreports disabled: true\"; `full-auto-registry.test.ts` \"recordFailure\nincrements and stamps typed failure state (releasing the lease);\nrecordSuccess clears all of it\".",
    "technique": "property",
    "title": "Assure FA-AC-16"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-17"
    ],
    "disposition": "required",
    "domains": [
      "exactly_once",
      "profile_continuity"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "profile_drift_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    },
    "id": "AO-FA-AC-17-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts",
      "statement": "A continuation replays the bound execution profile (account/model/effort) exactly; images/attachments/extensions reset each continuation."
    },
    "source_claim_digest": "sha256:fad71b282b1060216a6e6d231714dcc37bc290b15534152555660bd5450db41c",
    "source_claim_snapshot": "Automatic continuations preserve the initiating turn's\nexecution profile. When a renderer-initiated turn carries\n`fullAuto: true`, main binds its effective account target, model, and\nreasoning effort onto the durable record; every continuation (including a\npost-restart resume) replays that bound profile, revalidated against the\nlive contract enums (a field that no longer decodes falls back to lane\ndefaults instead of failing the loop). Fields that deliberately RESET on a\ncontinuation: images, explicit context attachments, and extension\nselection -- a continuation is a fresh instruction, not a replay of the\ninitiating turn's payload.\nProof: `full-auto-restart.e2e.test.ts` \"a continuation dispatch carries the\nprofile bound by the initiating flagged turn (account, model, effort) --\nincluding across a restart\" and \"decodeCodexLocalContinuationProfile\nrevalidates stored strings against the live contract...\".",
    "technique": "unit",
    "title": "Assure FA-AC-17"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-registry.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-18"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-18-01",
    "source_claim_digest": "sha256:1a81b9e9b1821120f34aa3c4ddcbf11bbc2992e916e3718b2f4e9898a9d05ed8",
    "source_claim_snapshot": "The wave-2 registry schema upgrade is strictly additive: every\nnew record field (workspace binding, profile, lease, failure state) is\noptional, and an existing v1 registry file decodes without quarantine so no\nuser's enabled state is lost by upgrading.\nProof: `full-auto-registry.test.ts` \"an existing v1 registry file (no\nwave-2 fields) still decodes -- the schema upgrade never quarantines a\nuser's state\".",
    "title": "Assure FA-AC-18",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_18_negative_control",
      "ref": "apps/openagents-desktop/tests/full-auto-registry.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-registry.test.ts",
      "statement": "The exact FA-AC-18 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx"
    ],
    "criterion_refs": [
      "FA-AC-19"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-19-01",
    "source_claim_digest": "sha256:9fe946aaddb3c1c7217a2d42784eaefaaaba48f6230ff55ebf32135fc4fd7d37",
    "source_claim_snapshot": "A background (main-initiated) continuation is rendered as a\ncoarse, typed, per-thread in-flight state, not silence until completion.\nMain owns an in-memory live-state map (idle | turn_running |\nturn_completed | turn_failed | cap_reached | blocked; blocked carries the\ntyped blockedReason as bounded detail) and broadcasts every transition to\nall windows over `CodexLocalFullAutoStateChannel`: turn_running with the\nlease turn ref at dispatch start, turn_completed on success, turn_failed\nwith the typed reason on an ordinary failure, cap_reached at the cap, and\nblocked on a workspace or failure-limit disable. Terminal states persist\nuntil the next transition. The extended get channel additively returns\n`{ state, turnRef }` beside `enabled`, and while the active thread's state\nis turn_running the composer renders a \"Full Auto running…\" status badge.\nToken-by-token streaming remains deliberately out of scope.\nProof: `shell.test.ts` \"FA-H4 (#8877): withFullAutoLiveState projects a\nlive-state event per thread and activeFullAutoTurnRunning reads only the\nACTIVE thread\"; `react-composer.test.tsx` \"FA-H4 (#8877): a running\nbackground Full Auto turn renders the status badge and the Stop\naffordance; idle renders neither\"; `main.ts` wires the transitions around\nthe existing `runFullAutoReconciliation` dispatch adapter and callbacks\n(code-reviewed; main.ts has no direct unit-test harness).",
    "title": "Assure FA-AC-19",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_19_negative_control",
      "ref": "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx",
      "statement": "The exact FA-AC-19 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/shell.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-20"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-20-01",
    "source_claim_digest": "sha256:e5bb0ecede90a05283555057a9d8d8b5197ce004bdbb0836e6294e1d87bf2887",
    "source_claim_snapshot": "A working stop targets the ACTUAL background turn. While the\nactive thread's live state is turn_running (renderer non-pending), the\ncomposer's Stop control dispatches the same interrupt intent, whose\nhandler calls the thread-scoped `CodexLocalFullAutoInterruptChannel` with\nonly `{ threadRef }`; main resolves the live running turn ref itself and\nsignals the exact same `codexLocal.interrupt` runtime path the existing\nturn-interrupt channel uses, answering `{ ok: boolean }`. While the\nrenderer's OWN turn is pending, Stop keeps signalling the active streaming\nturn unchanged. The interrupted background turn terminates through the\nexisting FA-H5 typed-failure path; the toggle remains the durable\nloop-level stop.\nProof: `shell.test.ts` \"FA-H4 (#8877): DesktopTurnInterrupted with a\nrunning BACKGROUND turn (not pending) calls fullAutoHost.interrupt with\nthe active threadRef\" and \"FA-H4 (#8877): while renderer-pending, Stop\nkeeps signalling the ACTIVE streaming turn (chat.interruptActive), not the\nbackground channel\"; `react-composer.test.tsx` (Stop affordance case\nabove); `main.ts` interrupt handler (code-reviewed).",
    "title": "Assure FA-AC-20",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_20_negative_control",
      "ref": "apps/openagents-desktop/src/renderer/shell.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/shell.test.ts",
      "statement": "The exact FA-AC-20 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/react-composer.test.tsx"
    ],
    "criterion_refs": [
      "FA-AC-21"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-21-01",
    "source_claim_digest": "sha256:96242f50d322c4619643e0da9d9d5496f93d97bcdc52f85239c3bef7b0b8e743",
    "source_claim_snapshot": "A manual send while a background Full Auto turn owns the\nthread is excluded, never run silently concurrently. When the active\nthread's live state is turn_running, `runNoteSubmission` refuses to start\na manual turn: it sets the transient notice \"Full Auto is running a turn\non this thread. Stop it first or wait for it to finish.\" and keeps the\ncomposer draft. Once the live state is terminal, the same submit goes\nthrough normally.\nProof: `shell.test.ts` \"FA-H4 (#8877): a manual send while a background\nFull Auto turn runs is FENCED -- sendMessage is never called, a notice\nsays why, and the draft is kept\".",
    "title": "Assure FA-AC-21",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_21_negative_control",
      "ref": "apps/openagents-desktop/src/renderer/react-composer.test.tsx"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
      "statement": "The exact FA-AC-21 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-22"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-22-01",
    "source_claim_digest": "sha256:0f0f2018b26d60b1402371861ad8755b89b17097b98bf7eff2e9d9387997b89a",
    "source_claim_snapshot": "The programmatic control surface is opt-in and off by\ndefault, loopback-only, and bearer-gated. Desktop main constructs the\ncontrol server ONLY when `OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1`; the\nlistener binds 127.0.0.1 exclusively (ephemeral or env-pinned port); every\nrequest -- the OpenAPI document included -- requires the per-process scoped\nbearer credential (scopes drawn from `@openagentsinc/environment-auth`'s\nnarrowing-only exchange, verified with a constant-time comparison) or is\nrefused 401. Connection info is written mode-0600 to\n`full-auto/control.json` under userData and removed on stop.\nProof: `full-auto-control-server.test.ts` \"off by default: main's guard\nrequires OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1 exactly\", \"credential mint\nuses the environment-auth narrowing-only exchange...\", \"auth: no bearer and\na wrong bearer are 401 on every route...\", and \"the connection file is\nwritten mode 0600...\"; `main.ts` wraps the entire server wiring in\n`isFullAutoControlEnabled(process.env)` (code-reviewed; main.ts has no\ndirect unit-test harness -- the guard function itself is the tested unit).",
    "title": "Assure FA-AC-22",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_22_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts",
      "statement": "The exact FA-AC-22 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-23"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-23-01",
    "source_claim_digest": "sha256:8e0ec08187c8ca2db29385a7622cd9af60cc26aa5313af4173bcfd6568a1f62c",
    "source_claim_snapshot": "Programmatic enable NAMES the workspace the caller expects\nand enforces it: the request body requires `workspaceRef`, the server\nresolves the current workspace itself via the same\n`resolveDesktopLocalWorkspaceRoot` codex-local turns execute against, and\nany difference is a 409 `workspace_mismatch` refusal with the registry left\nuntouched -- never a silent redirect. Programmatic enable can never grant a\nnew, previously-ungranted workspace; on success it binds exactly the\nresolved workspace, the same path as the IPC set handler.\nProof: `full-auto-control-server.test.ts` \"enable with a mismatched\nworkspaceRef is a 409 typed refusal and the registry is untouched\" and\n\"enable with the matching workspaceRef enables + binds the record...\".",
    "title": "Assure FA-AC-23",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_23_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts",
      "statement": "The exact FA-AC-23 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-24"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-24-01",
    "source_claim_digest": "sha256:30a72566ec674ecb4397f6f1088a73fad8e68d8e024065fccc28cc46a485442b",
    "source_claim_snapshot": "Every mutating control-API call (enable, disable,\ncontinue-now) appends a durable, distinctly-attributed system note to the\nthread through the existing `appendFullAutoSystemNote` (naming the\nprogrammatic path and caller `control-api`), plus a public-safe console\naudit line, so the owner can always tell a programmatic action from their\nown click.\nProof: `full-auto-control-server.test.ts` attribution assertions inside the\nenable, disable, and continue-now cases (note text contains \"programmatically\"\nand \"control-api\").",
    "title": "Assure FA-AC-24",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_24_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts",
      "statement": "The exact FA-AC-24 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-25"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-25-01",
    "source_claim_digest": "sha256:7abfea21c68f5feefe7da620209c18ddc291b890cb51b133c2a8843d2f7340a8",
    "source_claim_snapshot": "continue-now is a new TRIGGER into the shared serialized\nreconciliation path, never a new dispatch mechanism: the handler invokes\nthe exact injected reconciliation trigger (main passes\n`runFullAutoReconciliation`, the same FA-H3 promise-chain mutex + durable\nlease every other trigger point uses) exactly once and returns\n`{ scheduled: true }` immediately; dispatch remains subject to lease,\nworkspace binding, backoff, and cap policy. An unknown threadRef is 404 and\nnever touches the trigger.\nProof: `full-auto-control-server.test.ts` \"continue-now invokes the\ninjected reconcile trigger exactly once and returns { scheduled: true }\"\n(spy on the injected trigger) and \"continue-now on an unknown threadRef is\na 404 and never touches the trigger\"; `main.ts` passes\n`() => runFullAutoReconciliation()` as that capability (code-reviewed).",
    "title": "Assure FA-AC-25",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_25_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts",
      "statement": "The exact FA-AC-25 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-26"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-26-01",
    "source_claim_digest": "sha256:d841490075cc6fc71f4f7368ad6244f539f0f77f395e28cd13cf4a013f46e0af",
    "source_claim_snapshot": "The served surface and the published OpenAPI 3.1 document\ncannot drift: `GET /v1/openapi.json` serves the hand-authored document, and\na structural parity test asserts every route in the shared\n`FULL_AUTO_CONTROL_ROUTES` table appears in the document (path, method,\noperationId) AND every operation in the document is a served route.\nResponse bodies decode against the Effect Schemas in\n`full-auto-control-contract.ts`, whose bounds mirror the IPC contract.\nProjections stay public-safe: records expose only\nthreadRef/enabled/continuationCount/updatedAt/workspaceRef/blockedReason/\nlive state plus accountRef (never model/effort/raw profile material), and\nturns expose identity/phase/disposition/timestamps for at most the last 20\nFull Auto turns -- never transcript text.\nProof: `full-auto-control-server.test.ts` \"GET /v1/openapi.json serves the\ndocument, and the document <-> served routes agree in both directions\",\n\"list and status match the contract schemas... expose no profile material\nbeyond accountRef\", and \"turns returns a bounded, most-recent-first Full\nAuto projection with no transcript text\".",
    "title": "Assure FA-AC-26",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_26_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts",
      "statement": "The exact FA-AC-26 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-27"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-27-01",
    "source_claim_digest": "sha256:a379ac228a58d621c4498bc9f579070a44fe338d59d598b45551ef863bac629c",
    "source_claim_snapshot": "The MCP server and CLI are thin pass-through clients of the\none control surface: both discover the server from `full-auto/control.json`\n(with `--user-data` / `OPENAGENTS_DESKTOP_USER_DATA` overrides), attach the\nbearer, call the HTTP API, and return the server's JSON verbatim -- no\nclient-side policy and no second schema vocabulary. Both fail with a clear\n\"server not enabled\" message when the connection file is missing. The MCP\nserver exposes `full_auto_list` / `full_auto_status` / `full_auto_enable` /\n`full_auto_disable` / `full_auto_continue_now` / `full_auto_turns` over the\nrepo's public MCP protocol revision (2025-06-18).\nProof: `scripts/full-auto-cli.ts` and `scripts/full-auto-mcp.ts`\n(pass-through by construction over the shared\n`scripts/full-auto-control-client.ts`); live end-to-end receipt in the\nrev 6 entry under Receipts (`pnpm run smoke:full-auto-control` exercises\nthe real CLI as a second OS process against the real running Electron\nmain).",
    "title": "Assure FA-AC-27",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_27_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts",
      "statement": "The exact FA-AC-27 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-28"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-28-01",
    "source_claim_digest": "sha256:bfadbd7c3bc0f39bdda56240e27fd763503abb7e944c2e60d6ade253ef6efa6a",
    "source_claim_snapshot": "The control surface can BOOTSTRAP Full Auto with no existing\nthread: `POST /v1/full-auto/start` (OpenAPI `startFullAuto`, MCP\n`full_auto_start`, CLI `start --workspace <path> [--title <t>]`) mints a\nbrand-new local thread in main's own thread store (main names the ref --\nthe caller never supplies one), binds the resolved workspace, enables the\nrecord through the same `registry.set` path as the composer toggle,\nappends the distinctly-attributed `(caller: control-api)` system note, and\nschedules the shared serialized reconcile pass so the first continuation\ndispatches without a separate continue-now call -- the reconcile\ndispatcher then opens a brand-new provider conversation because the\nminted thread has no session continuity. start obeys the exact enable\nauthority rule: the caller MUST name the workspace it expects, and on any\ndifference from the currently resolved workspace the call refuses with\n409 `workspace_mismatch` with NO thread minted, NO record written, and NO\nnote appended -- never a redirect, never a new grant.\nProof: `src/full-auto-control-server.test.ts` (\"start with the matching\nworkspaceRef mints a thread...\", \"start with a mismatched workspaceRef is\na 409 typed refusal: NO thread minted...\", \"start discipline: bodyless\nstart is 400...\", plus the doc <-> route parity test covering\n`startFullAuto`).",
    "title": "Assure FA-AC-28",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_28_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts",
      "statement": "The exact FA-AC-28 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "activation_gate": "GATE-DEV-TWO-PROCESS",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/scripts/full-auto-restart-smoke.ts",
      "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-29"
    ],
    "disposition": "required",
    "domains": [
      "resume",
      "provider_continuity"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1",
      "ENV-FA-DEV-TWO-PROCESS-1"
    ],
    "evidence": {
      "proof_rung": "local_dev_two_process_unsigned",
      "required_kinds": [
        "execution_trace",
        "two_process_smoke_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "lane_lost_on_restart_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts"
    },
    "id": "AO-FA-AC-29-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/scripts/full-auto-restart-smoke.ts",
      "statement": "A durable execution profile carries an optional ProviderLane ref and survives restart; legacy rows still decode on codex-local. The smoke harness's Claude-lane seed-claude/resume-claude phase pair is real two-OS-process evidence at the unsigned/dev-mode tier (RISK-FA-10)."
    },
    "source_claim_digest": "sha256:95a4f3578438a53c7032f9e110b29e4202cb40695533f6792bbb6b092a360945",
    "source_claim_snapshot": "The durable execution profile carries an optional ProviderLane\nref. A rev-7 registry row with no lane still decodes and continues on\n`codex-local`; a selected `fable-local` row survives a Runtime A → Runtime B\nreopen and reaches the shared dispatch seam with the same lane/account/model.\nProof: `full-auto-restart.e2e.test.ts` \"a Claude lane selection survives\nRuntime A -> Runtime B...\" plus the retained legacy-file registry tests;\n`pnpm run smoke:full-auto-restart` launches real Electron OS processes for\n`seed-claude` → `resume-claude` and receipts `dispatchedLane:fable-local`.",
    "technique": "resilience",
    "title": "Assure FA-AC-29"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/provider-lane.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-30"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-30-01",
    "source_claim_digest": "sha256:a9b55a40165284d3dce1b35a96082a6a59d8fd6ad9f9ce15c77b73050a65fdcd",
    "source_claim_snapshot": "Reconciliation dispatches through the L1 ProviderLane SPI and\nfails closed for any lane that is unknown, L2-quarantined, does not advertise\nFull Auto, or lacks safe background-question settlement. Workspace binding,\nexactly-once lease, backoff, cap, and attribution behavior are unchanged.\nProof: `main.ts` lane selection + `projectProviderLaneCapabilities` gate;\nfocused Full Auto regression suites.",
    "title": "Assure FA-AC-30",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_30_negative_control",
      "ref": "apps/openagents-desktop/src/provider-lane.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/provider-lane.test.ts",
      "statement": "The exact FA-AC-30 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-lane.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-31"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-31-01",
    "source_claim_digest": "sha256:4c079b7e3e08a8adad86a2918fff6937a69a26010d2306c4e3bcf404bf155219",
    "source_claim_snapshot": "Codex and Claude Full Auto turns use the single lane-keyed\ninstruction policy. A background Claude `AskUserQuestion` never parks: it is\ndenied immediately with guidance to make a reasonable judgment and proceed,\nwhile an interactive ordinary Claude turn retains the existing real question\nUI flow.\nProof: `fable-local-runtime.test.ts` \"background Full Auto denies\nAskUserQuestion immediately...\" and the retained interactive question tests.",
    "title": "Assure FA-AC-31",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_31_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-lane.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-lane.test.ts",
      "statement": "The exact FA-AC-31 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-32"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-32-01",
    "source_claim_digest": "sha256:4f022a210eada46d961d2fb53d39763565d354d6465328d5cda3bb511253e1cd",
    "source_claim_snapshot": "`start` and `enable` accept an optional lane ref (default\n`codex-local`) through the shared control contract, served OpenAPI document,\nMCP tools, and CLI `--lane`; status/list expose the public-safe selected lane.\nAn ineligible lane returns typed 409 `lane_not_eligible` without mutating the\nregistry.\nProof: `full-auto-control-server.test.ts` \"enable accepts an admitted lane\nselector...\" plus document/route/schema parity.",
    "title": "Assure FA-AC-32",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_32_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-control-server.test.ts",
      "statement": "The exact FA-AC-32 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json"
    ],
    "criterion_refs": [
      "FA-AC-33"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-33-01",
    "source_claim_digest": "sha256:d13b2fc55b4284f1e5b47327c615a43b87c98d3c71736701129b26fdd82c9b96",
    "source_claim_snapshot": "A real bounded Claude Code Full Auto run must be retained as a\nrelease receipt. ACP peer proof remains conditional on #8893/#8894 admission\nand must not be inferred from fixture coverage.\nProof: owner/dogfood receipt linked from #8901; until captured this criterion\nremains an explicit residual, not a release claim.",
    "title": "Assure FA-AC-33",
    "activation_gate": "GATE-OWNER-REAL-PROVIDER",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-OWNER-REAL-SIDEBAR-1"
    ],
    "evidence": {
      "proof_rung": "owner_real",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_33_negative_control",
      "ref": "docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json",
      "statement": "The exact FA-AC-33 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "owner_real_replay"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/spec-lane-workflow.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-34"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-34-01",
    "source_claim_digest": "sha256:8050c77a9576607875b9d2a9af869a0ea159297be5f349a1d0697810424c6247",
    "source_claim_snapshot": "Every ProviderLane dispatch receives the same main-owned spec\nprojection when the granted workspace contains `specs/**`. The projection is\nbounded to 32 files, 512,000 bytes per file, 64 snapshot criteria per\nProductSpec, 128 snapshot obligations, 12 prompt criteria per ProductSpec,\n12 prompt obligations, and 8,000 prompt characters; truncation is explicit.\nProof: `spec-lane-workflow.test.ts` projection-bound case plus the shared\n`makeProviderLaneDispatcher` seam.",
    "title": "Assure FA-AC-34",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_34_negative_control",
      "ref": "apps/openagents-desktop/src/spec-lane-workflow.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/spec-lane-workflow.test.ts",
      "statement": "The exact FA-AC-34 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-lane.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-35"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-35-01",
    "source_claim_digest": "sha256:b8635e7309d8465621d38435efe0e9ec02511e2af8fbc92820c7605f70463776",
    "source_claim_snapshot": "Full Auto's shared lane instruction explicitly names unmet\nProductSpec/AssuranceSpec obligations as candidate work while preserving the\none-concrete-step contract and denying provider-owned verdict authority.\nProof: `full-auto-lane.ts` shared instruction and focused Full Auto tests.",
    "title": "Assure FA-AC-35",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_35_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-lane.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-lane.test.ts",
      "statement": "The exact FA-AC-35 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/spec-lane-workflow.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-36"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-36-01",
    "source_claim_digest": "sha256:482ee08b1bc9704f87e430c5c664700af0859e5a8bee1a4726d55abe1360688a",
    "source_claim_snapshot": "After a dispatched turn, main re-reads the workspace through\nthe ProductSpec/AssuranceSpec packages and appends a bounded system note with\nchanged and remaining unmet obligation state. Missing, malformed, stale,\nflaky, inconclusive, unreviewed, or excepted evidence never rounds green.\nProof: `spec-lane-workflow.test.ts` malformed-index and axis-revalidation\ncases.",
    "title": "Assure FA-AC-36",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_36_negative_control",
      "ref": "apps/openagents-desktop/src/spec-lane-workflow.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/spec-lane-workflow.test.ts",
      "statement": "The exact FA-AC-36 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/spec-lane-workflow.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-37"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-37-01",
    "source_claim_digest": "sha256:9f6f83a0851f2e17620d86e15f1a71ff32e081b69c846d09f2022ddd79a287c5",
    "source_claim_snapshot": "The identical bounded projection and revalidation path works\nthrough at least two distinct ProviderLane refs without importing a provider\ninto the spec module or moving admission, verification, release, or\npublic-claim authority into a lane.\nProof: the two-lane dispatcher fixture in `provider-lane.test.ts` and the\nCodex/Claude note assertions in `spec-lane-workflow.test.ts`.",
    "title": "Assure FA-AC-37",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_37_negative_control",
      "ref": "apps/openagents-desktop/src/spec-lane-workflow.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/spec-lane-workflow.test.ts",
      "statement": "The exact FA-AC-37 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-38"
    ],
    "disposition": "required",
    "domains": [
      "schema",
      "run_identity"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "malformed_run_record_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    },
    "id": "AO-FA-AC-38-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts",
      "statement": "A FullAutoRun record has a stable runRef independent of threadRef plus title/objective/doneCondition/workspace/profile/turnCap, Effect-Schema-decoded."
    },
    "source_claim_digest": "sha256:b189ba028f022fc45379a81cd5b78fa5f29cdb9c8b52f44e658b9510334a424b",
    "source_claim_snapshot": "A `FullAutoRun` record carries a stable `runRef` distinct from\nand independent of any `threadRef` it is currently bound to, plus title,\nobjective, explicit done condition, workspace, provider profile\n(lane/account/model/effort), and turn cap as first-class durable fields\ndecoded by an Effect Schema.\nProof: planned, owned by FA-RUN-01 (#8969); regression target\n`full-auto-run-registry.test.ts` (module TBD by #8969).",
    "technique": "unit",
    "title": "Assure FA-AC-38"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-39"
    ],
    "disposition": "required",
    "domains": [
      "exactly_once",
      "concurrency"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "ninth_active_run_capacity_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    },
    "id": "AO-FA-AC-39-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts",
      "statement": "Two active-run starts mint distinct runRef/threadRef identities and remain independently controllable; the ninth concurrent start refuses before minting a thread, and per-thread lease tests still forbid duplicate dispatch."
    },
    "source_claim_digest": "sha256:8d01cdf671b3c4bab025ef31871271502057b69939fbfbfab1338a3fb250f8a5",
    "source_claim_snapshot": "Starting another active run while other runs are active mints a\ndistinct `runRef` and `threadRef` and admits it without mutating, pausing, or\nsilently queueing any existing run, up to the explicit eight-active-run\nlocal capacity. A ninth start refuses before minting a thread. The monitor\nand authenticated control surfaces list and control each run by exact\n`runRef`. The existing durable\nper-thread lease still permits at most one in-flight Full Auto turn for a\ngiven thread, while different run threads may be in flight concurrently.\nProof: `full-auto-run-registry.test.ts` concurrent-active case,\n`full-auto-run-control-server.test.ts` two-run bootstrap case, and\n`react-full-auto-surface.test.tsx` multi-run monitor/open/stop case.",
    "technique": "unit",
    "title": "Assure FA-AC-39"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-40"
    ],
    "disposition": "required",
    "domains": [
      "exactly_once"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "terminal_mutation_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    },
    "id": "AO-FA-AC-40-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts",
      "statement": "Starting a new run from a terminal run's launcher always mints a new, distinct runRef and never mutates the terminal record."
    },
    "source_claim_digest": "sha256:88175449407486953a579bfeec384e8374e851b491a5db69c5c3491c8b157e2b",
    "source_claim_snapshot": "Starting a new run from a terminal run's launcher always mints\na new distinct `runRef` and never mutates the terminal record's fields or\nstate; an optional predecessor-run reference may be carried for report\ncontinuity only, never for authority or objective inheritance.\nProof: planned, owned by FA-RUN-01 (#8969).",
    "technique": "unit",
    "title": "Assure FA-AC-40"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-41"
    ],
    "disposition": "required",
    "domains": [
      "migration",
      "fail_closed"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "duplicate_or_invented_objective_migration_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    },
    "id": "AO-FA-AC-41-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts",
      "statement": "Legacy enabled:true rows migrate additively/idempotently to exactly one Running FullAutoRun with objectiveSource legacy_migration and the exact prior generic instruction (never an invented objective); disabled rows never migrate; a second concurrently-enabled row that loses the one-active-run race is preserved as Draft, never dropped; a second migration pass is a no-op."
    },
    "source_claim_digest": "sha256:50fd3b6e7fcb75657f0d0803843491b69c0c08d53e04da40e09d06b5518e34fd",
    "source_claim_snapshot": "On first startup after the FullAutoRun model ships, every\nexisting `enabled: true` legacy thread-keyed registry row migrates\nadditively to exactly one `FullAutoRun` record (Running or Paused per its\nprior live state) with no data loss; a second startup performs no duplicate\nmigration; an `enabled: false` legacy row does not migrate to an active run.\nProof: planned, owned by FA-RUN-01 (#8969).",
    "technique": "unit",
    "title": "Assure FA-AC-41"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-thread-pressure.e2e.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-42"
    ],
    "disposition": "required",
    "domains": [
      "fail_closed",
      "stall_detection"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace",
        "oracle_bite_verified_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "silent_reattach_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-thread-pressure.e2e.test.ts"
    },
    "id": "AO-FA-AC-42-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-thread-pressure.e2e.test.ts",
      "statement": "A run whose bound thread/session is missing or orphaned transitions to typed Stalled/Failed via settleFullAutoRunFromThreadState, never silently reattaches; composed against the real thread-pressure incident replay this never reproduces the 6-hour silent stall."
    },
    "source_claim_digest": "sha256:256fa9704359981b50a9ad2509b19ea7b919b39fedcae30f9e3b53bc8324c926",
    "source_claim_snapshot": "A run whose bound `threadRef` or provider session is missing\nor orphaned at reconciliation transitions to a typed Stalled or Failed\ndisposition with an owner-visible reason; it never silently reattaches to\nan unrelated thread and never reproduces the six-hour silent stall recorded\nin the 2026-07-17 audit.\nProof: planned, owned by FA-RUN-02 (#8970) (thread-pressure replay) and\nFA-RUN-03 (#8971) (stall classification); regression target: a deterministic\nreplay of the exact incident composition (long turn, concurrent chats,\nbounded mutable-thread eviction, gap-to-next-reconciliation) plus a real\nFull Auto pressure run.",
    "technique": "component",
    "title": "Assure FA-AC-42"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-43"
    ],
    "disposition": "required",
    "domains": [
      "lifecycle",
      "terminal_semantics"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "exhaustive_enumeration_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "illegal_transition_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    },
    "id": "AO-FA-AC-43-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts",
      "statement": "The full lifecycle is exactly Draft/Running/Pausing/Paused/Retrying/Stalled/Completed/Failed/Stopped/Cap-reached; exhaustively enumerated over the real exported isLegalFullAutoRunTransition across all 10x10 from/to pairs: terminal states have zero outgoing edges, and every legal transition persists actor/timestamp/typed reason."
    },
    "source_claim_digest": "sha256:61b0d3afb3632842c2728e7ec18ff4cadeaa1c639ac042d00d5cd013442b4903",
    "source_claim_snapshot": "The full run lifecycle state machine is exactly Draft,\nRunning, Pausing, Paused, Retrying, Stalled, Completed, Failed, Stopped, and\nCap-reached; every transition between these states persists actor (owner\nUI, control-api, or a named system policy such as workspace_guard or\ncontinuation_cap), a UTC timestamp, and a typed reason, extending the\nexisting `disabledBy` attribution pattern to the complete graph. An illegal\ntransition (for example Resume from a non-Paused state) is refused with a\ntyped error and never silently coerced.\nProof: planned, owned by FA-RUN-01 (#8969).",
    "technique": "property",
    "title": "Assure FA-AC-43"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-run-control-server.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-44"
    ],
    "disposition": "required",
    "domains": [
      "lifecycle",
      "pause_resume"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_contract_http",
      "required_kinds": [
        "http_contract_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "resume_from_non_paused_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-run-control-server.test.ts"
    },
    "id": "AO-FA-AC-44-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-run-control-server.test.ts",
      "statement": "Pause with an active turn goes Running->Pausing->Paused only once the turn resolves; Pause with no turn goes directly to Paused; Resume is legal only from Paused, re-validates workspace admission, and is exactly-once via the FA-H3 lease."
    },
    "source_claim_digest": "sha256:b18d5a2e4de51021cbe67b97e8f396d87fd80eb2c6628ca0ed1cb714c6e0ff4e",
    "source_claim_snapshot": "Pause with an active provider turn transitions the run to\nPausing immediately and to Paused only once that turn resolves (completes\nnormally or is interrupted); Pause with no turn in flight transitions\ndirectly to Paused. Resume is legal only from Paused, dispatches exactly\nonce through the existing FA-H3 serialized-mutex-plus-lease path (FA-AC-15,\nretained unchanged), and a run cannot be resumed twice concurrently.\nProof: planned, owned by FA-RUN-01 (#8969).",
    "technique": "contract",
    "title": "Assure FA-AC-44"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-run-control-server.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-45"
    ],
    "disposition": "required",
    "domains": [
      "lifecycle",
      "terminal_semantics"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_contract_http",
      "required_kinds": [
        "http_contract_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "resume_after_stop_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-run-control-server.test.ts"
    },
    "id": "AO-FA-AC-45-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-run-control-server.test.ts",
      "statement": "Stop is terminal and legal from any non-terminal state, distinct from Pause; a stopped run refuses a second Stop and refuses Resume."
    },
    "source_claim_digest": "sha256:e8ab443bcece89cb71ffa64d0cd863ddfc31275a77dd66169733ff9eefaddc16",
    "source_claim_snapshot": "Stop is a terminal transition legal from any non-terminal\nstate (Draft, Running, Pausing, Paused, Retrying, Stalled) and is distinct\nfrom Pause: a Stopped run is never resumed, and starting new work requires\nthe rerun path (FA-AC-40), never a mutation of the stopped record.\nProof: planned, owned by FA-RUN-01 (#8969).",
    "technique": "contract",
    "title": "Assure FA-AC-45"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-run-report.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-46"
    ],
    "disposition": "required",
    "domains": [
      "report_truth"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "auto_verified_completion_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-run-report.test.ts"
    },
    "id": "AO-FA-AC-46-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-run-report.test.ts",
      "statement": "Completed is self-reported and owner-reviewable, backed by the bounded FullAutoRunReport; every terminal disposition renders with typed stop attribution -- never automatically-verified truth."
    },
    "source_claim_digest": "sha256:090c81128f575f59befe1288e55ac55f97b9694059cb1a75af5e7f3837b56da9",
    "source_claim_snapshot": "A run's Completed disposition is a self-reported,\nowner-reviewable claim backed by the bounded `FullAutoRunReport`\n(FA-AC-51), not an automatically verified assertion that the objective/done\ncondition was actually satisfied; the product never presents Completed as\nverified truth. Automatic done-condition verification is explicitly cut\n(CUT-FA-04) past this revision.\nProof: planned, owned by FA-RUN-01 (#8969) for the state itself and\nFA-RUN-04 (#8972) for the report it is backed by; UX copy proof owned by\nFA-UX-01 (#8974).",
    "technique": "unit",
    "title": "Assure FA-AC-46"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-liveness.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-47"
    ],
    "disposition": "required",
    "domains": [
      "stall_detection",
      "liveness"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "auto_unstall_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-liveness.test.ts"
    },
    "id": "AO-FA-AC-47-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-liveness.test.ts",
      "statement": "Run-level liveness is distinct from single-turn duration: a run stays live while a turn executes or within the backoff-derived SLO window of last dispatch; otherwise Stalled with a typed cause and ETA. Stalled is sticky and never auto-reclassifies to Running."
    },
    "source_claim_digest": "sha256:41571473c6af933ae616457f9812297a3554cf079bc533a2c455a7503a64f7fb",
    "source_claim_snapshot": "Run-level liveness is computed distinctly from a single\nhealthy long-running provider turn: a run is live while a turn is genuinely\nexecuting OR while elapsed time since the last dispatch is within a defined\nSLO window; outside that window the run transitions to Stalled with an\nexplicit owner-visible cause and a retry ETA, never silence.\nProof: planned, owned by FA-RUN-03 (#8971).",
    "technique": "unit",
    "title": "Assure FA-AC-47"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-run-liveness-control-server.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-48"
    ],
    "disposition": "required",
    "domains": [
      "stall_detection",
      "recovery"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_contract_http",
      "required_kinds": [
        "http_contract_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "retry_nonstalled_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-run-liveness-control-server.test.ts"
    },
    "id": "AO-FA-AC-48-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-run-liveness-control-server.test.ts",
      "statement": "A Stalled run exposes an explicit recovery affordance (retry-now or Stop, never a generic banner); retry-now refuses when not Stalled and refuses a nonrecoverable cause with stop_only."
    },
    "source_claim_digest": "sha256:8102beda95251856a365f25a4b5af631ee98155230bf9242666ec6aeadeb0dc8",
    "source_claim_snapshot": "A stalled run exposes an explicit, owner-actionable recovery\naffordance (at minimum: retry now, or Stop) rather than requiring the owner\nto infer the situation from a generic failure banner, closing the exact\nobservability gap the 2026-07-17 audit recorded.\nProof: planned, owned by FA-RUN-03 (#8971) (detection/recovery) and\nFA-UX-01 (#8974) (affordance).",
    "technique": "contract",
    "title": "Assure FA-AC-48"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-49"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-49-01",
    "source_claim_digest": "sha256:0c24d8e40c4c3e3267aa1ae5c25a8e0cbe11a0296963c2d345f0a6e0a135f97f",
    "source_claim_snapshot": "Objective and done-condition text are durable fields on the\n`FullAutoRun` record itself, never dependent solely on provider-native\nsession continuity or the bounded transcript-note window; a provider\nswitch or a bounded-history truncation cannot cause the objective to be\nlost from Desktop's own state.\nProof: planned, owned by FA-RUN-01 (#8969); handoff-path proof owned by\nFA-HO-01 (#8975).",
    "title": "Assure FA-AC-49",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_49_negative_control",
      "ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts",
      "statement": "The exact FA-AC-49 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-50"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-50-01",
    "source_claim_digest": "sha256:012eaead2d43da7ded04b3439ea531732a4ad7e124fb6bec29a9fd7486195ced",
    "source_claim_snapshot": "Registry eviction never drops a non-terminal (Running,\nPausing, Paused, Retrying, Stalled) run record, extending the existing\nenabled-record eviction protection (FA-AC-12, retained with stronger proof)\nto the full FullAutoRun state set.\nProof: planned, owned by FA-RUN-01 (#8969).",
    "title": "Assure FA-AC-50",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_50_negative_control",
      "ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-run-registry.test.ts",
      "statement": "The exact FA-AC-50 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-run-report.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-51"
    ],
    "disposition": "required",
    "domains": [
      "report_truth",
      "bounds"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "dropped_or_duplicated_turn_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-run-report.test.ts"
    },
    "id": "AO-FA-AC-51-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-run-report.test.ts",
      "statement": "Every run produces a bounded private FullAutoRunReport (turns, provider transitions, claimed commits, failures, liveness gaps, progress, transcript findings, raw-evidence pointer); claimed commit SHAs are extracted from the local turn journal and deduplicated/turn-attributed, but NOT independently re-verified against live git state -- this obligation does not claim stronger verification than the code performs (RISK-FA-14)."
    },
    "source_claim_digest": "sha256:c799b959326a2d0a01780f9eff41ed6fb7aeb92cf855291a4584aa7872785e1c",
    "source_claim_snapshot": "Every run produces a bounded, private `FullAutoRunReport`\ncontaining: run ref, thread ref, title, objective, workspace, started/\nstopped timestamps; provider/lane per turn and every provider transition;\nper-turn disposition, duration, selected packet/issue, and bounded outcome\nsummary; commits/receipts claimed by the agent (verified independently\nwhere possible); failure classification, retry/backoff, recovery action,\nand disabled reason; liveness gaps over threshold; objective/acceptance\nprogress and remaining work; transcript-analysis findings; and a pointer to\nprivate raw evidence rather than raw transcript contents. The report never\ncontains raw prompts, raw provider tool output, secrets, or credentials.\nProof: planned, owned by FA-RUN-04 (#8972).",
    "technique": "unit",
    "title": "Assure FA-AC-51"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-run-report.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-52"
    ],
    "disposition": "required",
    "domains": [
      "privacy",
      "redaction"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "adversarial_redaction_walk_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-run-report.test.ts"
    },
    "id": "AO-FA-AC-52-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-run-report.test.ts",
      "statement": "The public-safe control/receipt projection exposes only bounded non-transcript fields; an adversarial walk of the entire derived receipt tree asserts no secret-bearing field (reasons/objective/doneCondition/workspace path/title/account/session refs/assistant text) ever leaks, and the schema structurally has no unbounded free-text field."
    },
    "source_claim_digest": "sha256:f2498af24ce8448f5e697d9dbd952b36e93b1c7bcf76063bd733fcfcbb98ecc7",
    "source_claim_snapshot": "A public-safe control/receipt projection of the run report\nexposes only bounded, non-transcript fields (extending the existing\nFA-AC-26 public-safety bound), and any raw-evidence pointer it carries\nresolves only to owner-private storage, never a public route.\nProof: planned, owned by FA-RUN-04 (#8972).",
    "technique": "unit",
    "title": "Assure FA-AC-52"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-run-analyzer.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-53"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-53-01",
    "source_claim_digest": "sha256:07f60230698e3f0598ebdd35fab637962cc0ff3a9685bdd18cf975f493700300",
    "source_claim_snapshot": "An offline/private transcript-analysis pass runs against a\ncompleted run's report and evidence pointer, reusing #8911's default-off\nDesktop usage/telemetry consent boundary rather than a parallel collection\npath, and produces measurable, comparable findings (duplicated setup,\ndrift, stalls, unclear UI state, false completion claims) across at least\ntwo runs of the same named test.\nProof: planned, owned by FA-RUN-05 (#8973).",
    "title": "Assure FA-AC-53",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_53_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-run-analyzer.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-run-analyzer.test.ts",
      "statement": "The exact FA-AC-53 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx"
    ],
    "criterion_refs": [
      "FA-AC-54"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-54-01",
    "source_claim_digest": "sha256:ee1cccbf29975d95ae29de67376f4841375283a56074368a89efd62c4b29b595",
    "source_claim_snapshot": "A dedicated **Full Auto** launcher action appears beside/\nunder **New session** in the left rail and collects title (auto-suggested\nfrom the objective, editable), objective, explicit done condition,\nworkspace, provider/lane, and a bounded turn cap (default 20, clearly\nshown) before Start is enabled; Start applies the same workspace-authority\nrefusal rule as the existing control-API `start` (FA-AC-28, retained with\nstronger proof).\nProof: planned, owned by FA-UX-01 (#8974).",
    "title": "Assure FA-AC-54",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_54_negative_control",
      "ref": "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx",
      "statement": "The exact FA-AC-54 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx"
    ],
    "criterion_refs": [
      "FA-AC-55"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-55-01",
    "source_claim_digest": "sha256:98f144ef0a2f0da51baab539b19a2608b04a3a5ebebb0835a6cbbaee6931e39b",
    "source_claim_snapshot": "After Start, the main canvas renders a dedicated read-only run\nview for v1: objective and workspace remain pinned at the top; current\nstate is one of Running, Pausing, Paused, Retrying, Stalled, Completed,\nFailed, Stopped, or Cap-reached, rendered explicitly (not inferred from a\ngeneric banner); Pause/Resume is the primary control depending on state;\nStop is a distinct, always-available terminal control while non-terminal;\nthe per-turn transcript (provider, duration, outcome, artifacts) is\ninspectable; and the ordinary chat composer is absent from this view while\nthe run is active.\nProof: planned, owned by FA-UX-01 (#8974).",
    "title": "Assure FA-AC-55",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_55_negative_control",
      "ref": "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx",
      "statement": "The exact FA-AC-55 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx"
    ],
    "criterion_refs": [
      "FA-AC-56"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-56-01",
    "source_claim_digest": "sha256:c7a1f7b04852fb39e8c05e99072b3f0056e0570f9c746e7451683c36f05ecc33",
    "source_claim_snapshot": "The composer-embedded Full Auto toggle, badge, and\nmanual-send fencing (FA-AC-01, FA-AC-19, FA-AC-21) are removed from the\nordinary chat composer once the dedicated launcher and run view ship; an\nordinary chat thread never exposes Full Auto controls inline again.\nProof: planned, owned by FA-UX-01 (#8974).",
    "title": "Assure FA-AC-56",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_56_negative_control",
      "ref": "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx",
      "statement": "The exact FA-AC-56 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx"
    ],
    "criterion_refs": [
      "FA-AC-57"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-57-01",
    "source_claim_digest": "sha256:fad1f7faefa66f7654341795cb8774ef69c3b1aeca5ec4951eb9c27302cea100",
    "source_claim_snapshot": "The left-rail sidebar entry for an active or recently-terminal\nrun displays its objective-derived title (never a raw first-message title)\nand a live-state-derived status indicator, and remains reachable via the\nsame search/navigation affordances as an ordinary thread.\nProof: planned, owned by FA-UX-01 (#8974); reuses generic-title work\ntracked at #8940.",
    "title": "Assure FA-AC-57",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_57_negative_control",
      "ref": "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx",
      "statement": "The exact FA-AC-57 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-provider-handoff.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-58"
    ],
    "disposition": "required",
    "domains": [
      "handoff",
      "truncation_disclosure"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "silent_truncation_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-provider-handoff.test.ts"
    },
    "id": "AO-FA-AC-58-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-provider-handoff.test.ts",
      "statement": "A manual or Pause->switch->Resume provider switch projects a host-owned bounded history (never raw session state) and appends a visible transition receipt (from/to/actor/time/reason/truncation flag); exceeding the shared message bound sets contextTruncated while the objective survives regardless."
    },
    "source_claim_digest": "sha256:489548c43875c0443257176e48f727abf6e30fa9c1bbda27252f422c51959a70",
    "source_claim_snapshot": "A manual same-thread provider switch (Codex<->Claude) and an\nexplicit Pause -> switch provider -> Resume sequence both project a\nhost-owned, objective-priority bounded history (never raw provider-private\nsession state) to the target provider, and both append a visible\ntransition receipt to the thread and the run report carrying from/to/\nactor/time/reason and an explicit truncation flag when the projection was\nbounded.\nProof: planned, owned by FA-HO-01 (#8975).",
    "technique": "unit",
    "title": "Assure FA-AC-58"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-run-handoff-control-server.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-59"
    ],
    "disposition": "required",
    "domains": [
      "fencing",
      "rollback"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_contract_http",
      "required_kinds": [
        "http_contract_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "partial_state_after_refusal_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-run-handoff-control-server.test.ts"
    },
    "id": "AO-FA-AC-59-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-run-handoff-control-server.test.ts",
      "statement": "A provider-switch/handoff request re-checks the target lane's admission/auth/eligibility before switching; on refusal the run's lane and profile are left untouched (explicit rollback), never a partial state change. This is the real coverage for #8978's named 'late-provider fencing' -- no literally-named fencing test exists; the rollback-on-refusal guarantee is the closest and most direct real coverage."
    },
    "source_claim_digest": "sha256:79c55f78740562d47e14ce35d2197118e86e749a23a165a1f49bd4293dc84965",
    "source_claim_snapshot": "A provider switch (manual or Pause -> switch -> Resume)\nre-checks the target lane's L2 capability admission, auth, and Full Auto/\nbackground-question eligibility before switching; on refusal the run\nremains on its current provider/lane with no partial state change (an\nexplicit rollback, not a redirect).\nProof: planned, owned by FA-HO-01 (#8975).",
    "technique": "contract",
    "title": "Assure FA-AC-59"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-provider-handoff.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-60"
    ],
    "disposition": "required",
    "domains": [
      "handoff",
      "privacy"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "implied_session_transfer_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-provider-handoff.test.ts"
    },
    "id": "AO-FA-AC-60-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-provider-handoff.test.ts",
      "statement": "The handoff envelope always carries an explicit provider_private_never_transferred omission marker; no copy states or implies that provider-private session state itself transfers on switch."
    },
    "source_claim_digest": "sha256:a192e0189eaa81a1895b1816559db66780274f4afb87d2f423a1c41445eff1d0",
    "source_claim_snapshot": "No documentation, UI copy, or public claim states or implies\nthat a provider switch transfers provider-private session state; copy is\nrestricted to what is actually true: a host-owned bounded projection of\nDesktop-visible thread history.\nProof: planned, owned by FA-HO-01 (#8975).",
    "technique": "unit",
    "title": "Assure FA-AC-60"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/react-composer.test.tsx"
    ],
    "criterion_refs": [
      "FA-AC-61"
    ],
    "disposition": "required",
    "domains": [
      "claim_freshness",
      "provider_support"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "quarantined_lane_offered_fixture",
      "ref": "apps/openagents-desktop/src/renderer/react-composer.test.tsx"
    },
    "id": "AO-FA-AC-61-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
      "statement": "Provider support/picker eligibility is derived from proven L2 admission plus proven safe background-question policy, never mere code presence: an admitted ACP lane is a real selectable first-class provider through the same picker chip; a quarantined/unadmitted lane is never offered."
    },
    "source_claim_digest": "sha256:d5c82128a69bae75650d700552adec91c4737e4a615b9298ed378ef01ffc8482",
    "source_claim_snapshot": "Provider support and picker eligibility are derived from\nexact admitted evidence (proven L2 capability admission plus a proven safe\nbackground-question policy), never presented merely because a lane adapter\nexists in code. An admitted ACP lane that has not cleared this bar is not\nexposed as a first-class Full Auto or handoff picker option.\nProof: planned, owned by FA-HO-01 (#8975); repairs the picker gap tracked\nat #8977.",
    "technique": "component",
    "title": "Assure FA-AC-61"
  },
  {
    "activation_gate": "GATE-OWNER-REAL-PROVIDER",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-acceptance-driver.test.ts",
      "docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json"
    ],
    "criterion_refs": [
      "FA-AC-62"
    ],
    "disposition": "required",
    "domains": [
      "handoff",
      "real_provider_dogfood"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1",
      "ENV-FA-OWNER-REAL-SIDEBAR-1"
    ],
    "evidence": {
      "proof_rung": "owner_real_development",
      "required_kinds": [
        "fixture_execution_trace",
        "owner_real_dogfood_receipt"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "broken_marker_retention_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-acceptance-driver.test.ts"
    },
    "id": "AO-FA-AC-62-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json",
      "statement": "The source-bound owner-real development receipt records PASS for TEST 01 and TEST 02 with real Codex and Claude lanes, explicit bidirectional transitions, non-null report/analysis digests, and null terminal failure classifications. The headless driver's broken-marker falsifier remains the independent oracle-sensitivity requirement; this producer does not self-admit either artifact."
    },
    "source_claim_digest": "sha256:8d83c9b227a3ed7376281cfe80448af2a02b9d1d217cd89d399f08338a818b40",
    "source_claim_snapshot": "Real-provider execution of `TEST 01` and `TEST 02` from the\n2026-07-17 audit's sidebar test batch (Codex establishes a marker and a\ntwo-step task, switches the same thread to Claude, Claude states the\nmarker and performs step two; and the reverse) is captured as a named,\nretained receipt in the owner's real Desktop sidebar, not inferred from\nfixture coverage.\nProof: planned, owned by FA-QA-01 (#8976).",
    "technique": "component",
    "title": "Assure FA-AC-62"
  },
  {
    "activation_gate": "GATE-OWNER-REAL-PROVIDER",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-acceptance.ts",
      "apps/openagents-desktop/src/full-auto-acceptance-driver.test.ts",
      "docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json"
    ],
    "criterion_refs": [
      "FA-AC-63"
    ],
    "disposition": "required",
    "domains": [
      "real_provider_dogfood",
      "objective_retention"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1",
      "ENV-FA-OWNER-REAL-SIDEBAR-1"
    ],
    "evidence": {
      "proof_rung": "owner_real_development",
      "required_kinds": [
        "fixture_execution_trace",
        "owner_real_dogfood_receipt"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "objective_lost_under_pressure_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-acceptance-driver.test.ts"
    },
    "id": "AO-FA-AC-63-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json",
      "statement": "The source-bound owner-real development receipt records PASS for TEST 03 with a real Codex-to-Claude transition, objective-retention artifact digests, non-null report/analysis digests, and a null terminal failure classification. The fixture negative control remains required to demonstrate oracle sensitivity; the receipt remains development-tier and unadmitted by this producer."
    },
    "source_claim_digest": "sha256:e3a8b7580a13d0884b5cddf1a12c554546bb5d8b2ad16c4ecc526c225a65ddae",
    "source_claim_snapshot": "Real-provider execution of `TEST 03` (objective retention\nunder context/notes pressure -- either the target provider states the\noriginal objective and acceptance rule correctly, or the product visibly\nreports truncation and requires confirmation rather than silently\nfabricating continuity) is captured as a named, retained receipt.\nProof: planned, owned by FA-QA-01 (#8976).",
    "technique": "component",
    "title": "Assure FA-AC-63"
  },
  {
    "activation_gate": "GATE-OWNER-REAL-PROVIDER",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-acceptance-driver.test.ts",
      "docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json"
    ],
    "criterion_refs": [
      "FA-AC-64"
    ],
    "disposition": "required",
    "domains": [
      "real_provider_dogfood"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1",
      "ENV-FA-OWNER-REAL-SIDEBAR-1"
    ],
    "evidence": {
      "proof_rung": "owner_real_development",
      "required_kinds": [
        "fixture_execution_trace",
        "owner_real_dogfood_receipt"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fixture_verdict_mismatch_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-acceptance-driver.test.ts"
    },
    "id": "AO-FA-AC-64-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json",
      "statement": "The source-bound owner-real development receipt records PASS for TEST 04 (three-turn unattended Codex) and TEST 05 (Claude restart), each with three artifact digests, non-null report/analysis digests, and null terminal failure classifications. This is not signed-package restart proof and is not self-admitted by this producer."
    },
    "source_claim_digest": "sha256:4b958dfe66f7ac9c2074aa7f6d8d2e19e3e116eeec506502db5d93ae321b1135",
    "source_claim_snapshot": "Real-provider execution of `TEST 04` (a three-turn unattended\nCodex run with no manual message between turns, visible progress, and an\nexplicit stop reason) and `TEST 05` (a Claude run surviving a Desktop\nrestart with the same objective/lane and no duplicate turn) are captured as\nnamed, retained receipts.\nProof: planned, owned by FA-QA-01 (#8976); reuses FA-RUN-02 (#8970) restart\ninfrastructure.",
    "technique": "component",
    "title": "Assure FA-AC-64"
  },
  {
    "activation_gate": "GATE-OWNER-REAL-PROVIDER",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-acceptance-driver.test.ts",
      "apps/openagents-desktop/tests/full-auto-thread-pressure.e2e.test.ts",
      "docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json"
    ],
    "criterion_refs": [
      "FA-AC-65"
    ],
    "disposition": "required",
    "domains": [
      "real_provider_dogfood",
      "stall_detection"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1",
      "ENV-FA-OWNER-REAL-SIDEBAR-1"
    ],
    "evidence": {
      "proof_rung": "owner_real_development",
      "required_kinds": [
        "fixture_execution_trace",
        "oracle_bite_verified_trace",
        "owner_real_dogfood_receipt"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "incident_reproduction_fixture",
      "ref": "apps/openagents-desktop/tests/full-auto-thread-pressure.e2e.test.ts"
    },
    "id": "AO-FA-AC-65-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json",
      "statement": "The source-bound owner-real development receipt records PASS for TEST 06 after greater-than-five-chat pressure, with three artifact digests, non-null report/analysis digests, and a null terminal failure classification. The deterministic pressure regression remains the falsifier/oracle-sensitivity evidence; this producer does not independently admit the owner-real result."
    },
    "source_claim_digest": "sha256:65fa6e7fe479a8388a5a0a26f50e545f6892dd2497df2ff0c3c1900f4764c3fe",
    "source_claim_snapshot": "`TEST 06` -- the real replay of the 2026-07-17 incident\n(launch Full Auto, then create/open more than five other chats while its\nturn runs) -- passes with the autonomous thread remaining addressable and\nthe next continuation starting, both as an automated deterministic\nregression and as a real-provider dogfood receipt.\nProof: planned, owned by FA-RUN-02 (#8970) (automated regression) and\nFA-QA-01 (#8976) (real-provider receipt).",
    "technique": "component",
    "title": "Assure FA-AC-65"
  },
  {
    "candidate_artifact_refs": [
      "packages/assurance-spec/test/assurance-spec.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-66"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-66-01",
    "source_claim_digest": "sha256:374fcf720d23b8182c1460e8f333c559eba287a9c5e75956a448fb956dd58453",
    "source_claim_snapshot": "The Full Auto AssuranceSpec is reconciled to this revision:\nevery FA-AC-01..37 obligation carries its Criterion Disposition Map\noutcome (retired obligations marked accordingly, changed obligations\nrebound to their rev-10 criterion text) and every FA-AC-38..65 obligation\nhas a designed proof rung -- no obligation may round green by omission or\nsilent carry-forward of the rev-9 needs_design set.\nProof: planned, owned by FA-AS-01 (#8978).",
    "title": "Assure FA-AC-66",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_66_negative_control",
      "ref": "packages/assurance-spec/test/assurance-spec.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "packages/assurance-spec/test/assurance-spec.test.ts",
      "statement": "The exact FA-AC-66 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "activation_gate": "GATE-LOCAL-UNIT",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-retry-rotation-model.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-67"
    ],
    "disposition": "required",
    "domains": [
      "rotation",
      "routing",
      "exactly_once"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "exhaustive_enumeration_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "unrecognized_reason_never_rotates_fixture",
      "ref": "apps/openagents-desktop/src/full-auto-retry-rotation-model.test.ts"
    },
    "id": "AO-FA-AC-67-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-retry-rotation-model.test.ts",
      "statement": "Every FullAutoRotationReasonSchema literal (account_exhausted/rate_limited/provider_error) classifies exhaustively via the real exported classifyFullAutoDispatchFailure over its documented direct-reason and detail-substring marker sets, rotating to the next admitted candidate under a fresh exactly-once lease in the same reconciliation pass; a full unsuccessful cycle consumes exactly one FA-H5 budget step."
    },
    "source_claim_digest": "sha256:c9621aee478959a6b57cc7530930f670dd46d7c67c68776cc9504cb5a04020fb",
    "source_claim_snapshot": "With an ordered routing policy of admitted lane/account\ncandidates bound on the durable record, a continuation dispatch that fails\nwith a typed account_exhausted, rate_limited, or provider_error class\nrotates to the next admitted candidate IN THE SAME reconciliation pass,\nunder a fresh exactly-once lease per attempt, persisting a typed rotation\nrecord (fromLane, toLane, reason, at) -- the run never enters failure\nbackoff while an untried admitted candidate remains. A full unsuccessful\ncycle through every candidate consumes exactly one FA-H5 failure-budget\nstep; untyped failures, records without a policy, and every existing\ncap/disable/backoff semantic behave exactly as before (proven by retained\nregression tests). Policies are validated fail-closed at bind time:\nunknown, unadmitted, or Full-Auto-ineligible lanes refuse the whole policy\nat validation, never at dispatch, and the loop can never rotate outside\nthe owner-admitted candidate set. Rotation history surfaces through the\ncontrol-API status projections as public-safe typed fields only (lane\nrefs, typed reason, timestamp -- never prompts, models, paths, or\nsecrets), and a v1/v2-era registry file without the new optional fields\ndecodes and behaves exactly as single-lane.\nProof: FA-RT-01 (#8987); `full-auto-restart.e2e.test.ts` \"Full Auto\nmulti-lane never-halt rotation (FA-RT-01 #8987)\" cases (same-pass rotation\nper typed class, one-budget-step full cycle, untyped/legacy regression,\nbound-lane start order, restart survival), `full-auto-registry.test.ts`\n\"Full Auto multi-lane routing-policy fields (FA-RT-01 #8987)\" cases\n(legacy fixture decode, bind/clear/bounds, capped oldest-evicted history,\npublic-safe projection), and `full-auto-routing.test.ts` (fail-closed\npolicy validation and the control-record rotationHistory projection\nbound).",
    "technique": "property",
    "title": "Assure FA-AC-67"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/tests/full-auto-guardrails.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-68"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-68-01",
    "source_claim_digest": "sha256:42198d0dc4dd0e1dc88785133fb9636e22677ee468114b2ae42a6105dc057af4",
    "source_claim_snapshot": "Owner-configurable guardrails and the confidence gate bound\nevery unattended run. Each configured guardrail class provably halts a\nsynthetic run with its typed reason and durable attribution: maxWallClockMs\n(against the durable `enabledAt` anchor, failing CLOSED when a\nguardrail-bearing record lacks the anchor) and maxTurns terminate with\n`guardrail_max_wall_clock`/`guardrail_max_turns` and\n`disabledBy: \"guardrail\"`; maxPerTurnFailures tightens the FA-H5 budget\nwhile keeping its `dispatch_failure_limit` attribution; tokenBudgetRef is\ncarried durably as an owner-visible ref and never fabricated into\nenforcement. Absent guardrails preserve the existing 20-cap and 5-failure\nsemantics byte-for-byte. Every between-turn decision persists as a typed,\nbounded, oldest-evicted decision record (continue / rotate /\npause_low_confidence / stop_guardrail with reason and remaining budget)\nthat survives restart and disable/enable, with a public-safe explicit\nfield-by-field projection. A deterministic no-progress detector (three\nconsecutive settled failed/interrupted_by_restart turns after the\nlastResumedAt ?? enabledAt anchor) transitions the record to a durable\n`pausedReason`-carrying paused state instead of continuing blind; the\npause survives restart, only an explicit attributed resume\n(`resumeFullAuto`) clears it, and pre-resume evidence can never\nimmediately re-pause the resumed loop. The non-overridable core set --\nworkspace binding, own-capacity-only admission, no rate-limit-reset\ntriggering -- is enforced in code with no config/env surface: unknown\nguardrail keys are dropped at decode, and no environment variable or\nhand-edited durable field relaxes any of the three (proven by the\nimmunity test). Guardrail terminations flow into the FA-RUN-04 report's\nthreadFailureHistory and settle the bound FullAutoRun to Stopped with the\ntyped `guardrail` actor. Legacy registry files without the new optional\nfields decode unquarantined and behave exactly as before.\nProof: FA-GD-01 (#8991); `tests/full-auto-guardrails.test.ts` (guardrail\nschema/legacy decode, decision records, per-class typed halts, durable\npause + restart survival + explicit resume, non-overridable immunity, run\nreport pickup); control-server/OpenAPI/UI wiring for bind-guardrails,\nresume, and the decision-history projection is an explicit named\nfollow-up seam, not claimed by this revision.",
    "title": "Assure FA-AC-68",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "contract",
      "regression"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_68_negative_control",
      "ref": "apps/openagents-desktop/tests/full-auto-guardrails.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/tests/full-auto-guardrails.test.ts",
      "statement": "The exact FA-AC-68 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-69"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-69-01",
    "source_claim_digest": "sha256:cd15ea0f2c2a7bdd8c9a58b880a7fe6935997e511b0028c24bfbeea45492c233",
    "source_claim_snapshot": "Before a Full Auto run is admitted, the host resolves exactly\none released `HarnessPolicyBundle` and persists its immutable digest plus\nthe six independently versioned dimension-policy refs (context assembly,\ntool interaction, generation control, orchestration, memory management,\noutput processing). Compatibility is checked against the\nexact engine protocol, provider/model/toolset, execution profile,\nevaluator, and environment; an unknown, candidate-only, revoked, or\nincompatible bundle fails closed before a thread or first turn dispatch.\nProof: planned; new MemoHarness/Blueprint implementation authority is\nrequired before dispatch.",
    "title": "Assure FA-AC-69",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "memo_harness",
      "authority_preservation"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_contract_model",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_69_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts",
      "statement": "The exact FA-AC-69 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "bounded_contract_model"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-70"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-70-01",
    "source_claim_digest": "sha256:07bf96fb6a114bfe4ef69473312c4a9542d831d451b9fc274b9fdfa00fffcb7b",
    "source_claim_snapshot": "When adaptation is enabled, run admission freezes an eligible\nexperience-bank snapshot before the first dispatch. Retrieval is semantic\nthrough the central typed selector and is filtered by tenant, workspace,\nvisibility, retention, consent, evaluator compatibility, deletion, and\ntombstone state; it never uses ad hoc keyword intent routing and never\nretrieves raw cross-tenant or ineligible evidence. The snapshot identity,\nsafe experience/pattern refs, scores, filters, and cache/cost facts are\nrecorded in the adaptation receipt, while raw prompts, transcripts, tool\noutput, embeddings, secrets, credentials, and filesystem paths remain\nprivate.\nProof: planned; the retrieval, privacy, and deletion-aware snapshot suites\ndo not yet exist.",
    "title": "Assure FA-AC-70",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "memo_harness",
      "authority_preservation"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_contract_model",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_70_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts",
      "statement": "The exact FA-AC-70 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "bounded_contract_model"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-71"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-71-01",
    "source_claim_digest": "sha256:8e56e60b71e97c71a5cb015289d48230cdc544575cfa3a585a142d4d29481bde",
    "source_claim_snapshot": "Per-case adaptation happens at most once, after the snapshot\nfreezes and before the first provider turn. It applies only released,\ncompatible, bounded module patches and emits a `HarnessAdaptationReceipt`\ncontaining base/result bundle digests, snapshot, selected patch refs,\ncompatibility and risk decisions, and explicit no-current-run-label and\nno-current-run-feedback facts. The effective digest is immutable through\nevery continuation, restart, Pause/Resume, retry, and provider handoff;\nan incompatible result fails closed rather than silently falling back or\nchanging policy mid-run.\nProof: planned; run-start adaptation and restart/handoff invariance tests do\nnot yet exist.",
    "title": "Assure FA-AC-71",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "memo_harness",
      "authority_preservation"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_contract_model",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_71_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts",
      "statement": "The exact FA-AC-71 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "bounded_contract_model"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-72"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-72-01",
    "source_claim_digest": "sha256:6b3028616af9475e8941d26e70b77442926d93d106735aaa53546abbc06b5db2",
    "source_claim_snapshot": "The authority manifest before and after adaptation is\nidentical for objective/done condition, workspace grant, execution\nplacement/profile, provider/account candidate set and order, tool scopes,\napproval policy, guardrails, budgets, release authority, action authority,\nand external effects. The adapter schema cannot express those fields, and\na malformed or hand-edited delta that attempts to change one refuses before\ndispatch with a typed reason.\nProof: planned; a bounded authority-immunity model and regression suite do\nnot yet exist.",
    "title": "Assure FA-AC-72",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "memo_harness",
      "authority_preservation"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_contract_model",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_72_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts",
      "statement": "The exact FA-AC-72 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "bounded_contract_model"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-73"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-73-01",
    "source_claim_digest": "sha256:b665295b430e6b5690fe058f4832ba3533f0bd5e22029b88998d5b5b01427f55",
    "source_claim_snapshot": "Every `FullAutoRunReport`, private control record, and\npublic-safe projection records the effective execution tuple of provider,\nmodel, harness bundle digest, toolset, evaluator, and environment, plus the\nadaptation policy/receipt refs and static/global/adapted classification.\nPublic-safe projections are explicit-field allowlists and never include raw\nexperience content, retrieval queries, embeddings, private scores, prompts,\ntranscripts, tool output, secrets, credentials, or filesystem paths.\nProof: planned; report and cross-surface projection schemas require an\nadditive implementation revision.",
    "title": "Assure FA-AC-73",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "memo_harness",
      "authority_preservation"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_contract_model",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_73_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts",
      "statement": "The exact FA-AC-73 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "bounded_contract_model"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-74"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-74-01",
    "source_claim_digest": "sha256:64d472a8991b10c07decb6948452d157710cf7dbcaa9ab3f872b7e69755dd412",
    "source_claim_snapshot": "A run can become a `HarnessExecutionExperience` only through a\nseparate Effect-owned compiler after the run is terminal and its report is\nimmutable. The experience records source run/report/evaluator/provenance,\noutcome and quality facts, visibility, retention, and retrieval/training\neligibility; it cannot alter the source run, its effective bundle, or the\nfrozen snapshot that source run used. Export, deletion, and tombstone flows\nprevent deleted evidence from re-entering later snapshots or candidates.\nProof: planned; no terminal-run experience compiler or deletion receipt\nsuite is claimed by this revision.",
    "title": "Assure FA-AC-74",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "memo_harness",
      "authority_preservation"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_contract_model",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_74_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts",
      "statement": "The exact FA-AC-74 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "bounded_contract_model"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-75"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-75-01",
    "source_claim_digest": "sha256:91ced3de6e86519d789498743b436deaff1d70113f49318ddd1b60e34d174831",
    "source_claim_snapshot": "Offline global optimization and pattern extraction consume\nonly admitted immutable experience/evaluation snapshots and produce\ncontent-addressed candidate module versions with complete lineage. A\ncandidate cannot become production by optimizer, executor, or run action:\nit must pass held-out quality, regression, compatibility, privacy, and\nsafety evidence, then cross an independent Blueprint release gate. Shadow/\ndogfood execution is explicit; production Full Auto resolves only released\ncompatible versions and records promotion or rollback receipts.\nProof: planned; the optimizer, evidence packet, and Blueprint BP-MH release\ngate are not implemented or admitted by this document.",
    "title": "Assure FA-AC-75",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "memo_harness",
      "authority_preservation"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_contract_model",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_75_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts",
      "statement": "The exact FA-AC-75 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "bounded_contract_model"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    ],
    "criterion_refs": [
      "FA-AC-76"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-76-01",
    "source_claim_digest": "sha256:b0f9b83855ae085ad89e1032193d087c33b68b1e64a2137f185782e8745555ec",
    "source_claim_snapshot": "MemoHarness application and control-plane code is TypeScript\non Effect: Effect Schema contracts, services/layers, structured errors,\nsemantic retrieval, storage policy, optimization orchestration, adaptation,\nretention/deletion, and release resolution. Any Rust is an isolated native\nhelper for containment, PTY/process primitives, or local inference behind a\ngenerated Effect-owned contract; no Rust MemoHarness daemon, policy engine,\nrelease authority, metadata database, or second source of truth is admitted.\nProof: planned; architecture conformance tests must reject ownership drift\nwhen implementation begins.",
    "title": "Assure FA-AC-76",
    "activation_gate": "GATE-LOCAL-UNIT",
    "domains": [
      "memo_harness",
      "authority_preservation"
    ],
    "environment_refs": [
      "ENV-FA-LOCAL-UNIT-1"
    ],
    "evidence": {
      "proof_rung": "local_contract_model",
      "required_kinds": [
        "execution_trace",
        "negative_control_trace"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fa_ac_76_negative_control",
      "ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts"
    },
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/full-auto-harness-policy.test.ts",
      "statement": "The exact FA-AC-76 source claim must be observed at the declared proof rung, and its named negative control must be rejected without weakening runtime policy."
    },
    "technique": "bounded_contract_model"
  }
]
```

## Gates

Four gates, one per Environment Profile, express exactly when each evidence tier is genuinely armed -- never a blended "some evidence exists" signal (ASSURANCE_SPEC.md Law 7). `GATE-LOCAL-UNIT` is armed by default under the owned runner (never GitHub-hosted CI, per root `INVARIANTS.md` and this repo's CLAUDE.md). `GATE-DEV-TWO-PROCESS` requires an explicit local invocation of the named smoke script on owned infrastructure. `GATE-PACKAGED-RELEASE` stays BLOCKED until a signed/notarized build and its runbook receipt both exist -- it is not armed by this revision. `GATE-OWNER-REAL-PROVIDER` is evidence-armed only for the exact owner-real development candidate in the retained receipt; it does not arm any newer source revision, packaged build, release, or admission decision.

```assurancespec-gates
[
  {
    "expression": "Armed by default under the owned runner (pnpm run check / vp test) on owned infrastructure; never armed by GitHub-hosted CI per root INVARIANTS.md.",
    "id": "GATE-LOCAL-UNIT"
  },
  {
    "expression": "Armed only by an explicit local invocation of pnpm run smoke:full-auto-restart or smoke:full-auto-control on owned infrastructure with a free loopback port and an isolated temporary user-data directory; never armed automatically or in GitHub-hosted CI.",
    "id": "GATE-DEV-TWO-PROCESS"
  },
  {
    "expression": "BLOCKED by default. Arms only once a packaged build exists at out/OpenAgents-darwin-arm64/OpenAgents.app AND apps/oa-updates/docs/release-signing-runbook.md has produced a signed/notarized receipt for that exact build. The smoke harness's conditional fallback to unsigned dev-mode Electron does NOT arm this gate.",
    "id": "GATE-PACKAGED-RELEASE"
  },
  {
    "expression": "Armed only for source revision 3123d926a3, build main-3123d926a3, test definition 77ab05baed3c3ab4974787b4c37d66720eae3eadfd90bcf5f5fea3e4935f8c78, and the owner_real development receipt at docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json. It is not armed for another revision, a packaged build, release admission, or a public claim.",
    "id": "GATE-OWNER-REAL-PROVIDER"
  }
]
```

## Evidence Policy

Links are pointers, never verdicts (Law 13): a `candidate_artifact_refs` entry or an `oracle.evaluator_ref` identifies the artifact a verifier must inspect; its mere presence is not admission. Missing, stale, or unexecuted evidence stays `INCONCLUSIVE` (schema-enforced: `missing_evidence_verdict` is a literal `"INCONCLUSIVE"`). `required_for_ready_obligation` below names the four axes every obligation's design must resolve before it can be considered execution-ready: an oracle observation, a falsifier observation demonstrating sensitivity, an environment binding, and independent (non-producer) review. This revision resolves the DESIGN of those four axes for 76 obligations. It additionally binds FA-AC-62..65 to the exact owner-real development receipt, but does not independently admit that receipt. Evidence tier must never round up: the owner-real development receipt is not signed packaged evidence; unsigned dev-mode two-process evidence (FA-AC-07/08/09/29) is not a signed packaged-release proof; and existing Full Auto lifecycle/report tests provide zero MemoHarness observation for FA-AC-69..76 until the designed oracles, falsifiers, environments, and independent review are actually run.

```assurancespec-evidence-policy
{
  "links_are_verdicts": false,
  "missing_evidence_verdict": "INCONCLUSIVE",
  "policy_state": "designed",
  "required_for_ready_obligation": [
    "oracle_observation",
    "falsifier_observation",
    "environment_binding",
    "independent_review"
  ]
}
```

## Authority Boundaries

This document cannot admit itself (`proposal_may_self_admit: false`), execute (`proposal_may_execute: false`), verify (`proposal_may_verify: false`), release (`proposal_may_release: false`), or change a public promise (`proposal_may_change_public_promises: false`) -- all four are schema-literal `false` and cannot be flipped by editing this file. `admitted_roles` names who CAN admit this revision: the existing authorized review boundary named in #8978's own text -- concretely, the owner or an explicitly owner-designated independent reviewer distinct from the obligation's producer (this repo's agents, including the one authoring this revision, are producers, never admitters, of their own obligations). `verifier_roles` requires that an obligation's verifier differ from its evidence producer (mirrors `independence.producer_may_verify: false` set on every designed obligation below). `release_roles` stays the owner alone; this AssuranceSpec's existence, however complete, never itself authorizes a release or public claim -- the product-promise registry remains the sole authority for that. The same separation applies inside MemoHarness: candidate production, evaluation, verification, Blueprint release, and production activation are distinct roles, and neither an optimizer nor the run that supplied an experience may self-promote its output.

```assurancespec-authority
{
  "admitted_roles": [
    "owner",
    "owner_designated_independent_reviewer"
  ],
  "policy_state": "designed",
  "proposal_may_change_public_promises": false,
  "proposal_may_execute": false,
  "proposal_may_release": false,
  "proposal_may_self_admit": false,
  "proposal_may_verify": false,
  "release_roles": [
    "owner"
  ],
  "verifier_roles": [
    "independent_reviewer_not_producer"
  ]
}
```

## custom-criterion-coverage-ledger

Per-criterion disposition for this revision. "designed (ready)" means the obligation's oracle/falsifier/technique/environment/evidence/independence/activation_gate are all present, mechanically verified by `assurance-spec coverage`. It does not mean the oracle has passed, the evidence is fresh for a later source revision, or an independent reviewer has admitted it. All 76 criteria are designed; zero remain `needs_design`.

| Criterion | Disposition (this revision) |
| --------- | --------------------------- |
| FA-AC-01  | designed (ready)            |
| FA-AC-02  | designed (ready)            |
| FA-AC-03  | designed (ready)            |
| FA-AC-04  | designed (ready)            |
| FA-AC-05  | designed (ready)            |
| FA-AC-06  | designed (ready)            |
| FA-AC-07  | designed (ready)            |
| FA-AC-08  | designed (ready)            |
| FA-AC-09  | designed (ready)            |
| FA-AC-10  | designed (ready)            |
| FA-AC-11  | designed (ready)            |
| FA-AC-12  | designed (ready)            |
| FA-AC-13  | designed (ready)            |
| FA-AC-14  | designed (ready)            |
| FA-AC-15  | designed (ready)            |
| FA-AC-16  | designed (ready)            |
| FA-AC-17  | designed (ready)            |
| FA-AC-18  | designed (ready)            |
| FA-AC-19  | designed (ready)            |
| FA-AC-20  | designed (ready)            |
| FA-AC-21  | designed (ready)            |
| FA-AC-22  | designed (ready)            |
| FA-AC-23  | designed (ready)            |
| FA-AC-24  | designed (ready)            |
| FA-AC-25  | designed (ready)            |
| FA-AC-26  | designed (ready)            |
| FA-AC-27  | designed (ready)            |
| FA-AC-28  | designed (ready)            |
| FA-AC-29  | designed (ready)            |
| FA-AC-30  | designed (ready)            |
| FA-AC-31  | designed (ready)            |
| FA-AC-32  | designed (ready)            |
| FA-AC-33  | designed (ready)            |
| FA-AC-34  | designed (ready)            |
| FA-AC-35  | designed (ready)            |
| FA-AC-36  | designed (ready)            |
| FA-AC-37  | designed (ready)            |
| FA-AC-38  | designed (ready)            |
| FA-AC-39  | designed (ready)            |
| FA-AC-40  | designed (ready)            |
| FA-AC-41  | designed (ready)            |
| FA-AC-42  | designed (ready)            |
| FA-AC-43  | designed (ready)            |
| FA-AC-44  | designed (ready)            |
| FA-AC-45  | designed (ready)            |
| FA-AC-46  | designed (ready)            |
| FA-AC-47  | designed (ready)            |
| FA-AC-48  | designed (ready)            |
| FA-AC-49  | designed (ready)            |
| FA-AC-50  | designed (ready)            |
| FA-AC-51  | designed (ready)            |
| FA-AC-52  | designed (ready)            |
| FA-AC-53  | designed (ready)            |
| FA-AC-54  | designed (ready)            |
| FA-AC-55  | designed (ready)            |
| FA-AC-56  | designed (ready)            |
| FA-AC-57  | designed (ready)            |
| FA-AC-58  | designed (ready)            |
| FA-AC-59  | designed (ready)            |
| FA-AC-60  | designed (ready)            |
| FA-AC-61  | designed (ready)            |
| FA-AC-62  | designed (ready)            |
| FA-AC-63  | designed (ready)            |
| FA-AC-64  | designed (ready)            |
| FA-AC-65  | designed (ready)            |
| FA-AC-66  | designed (ready)            |
| FA-AC-67  | designed (ready)            |
| FA-AC-68  | designed (ready)            |
| FA-AC-69  | designed (ready)            |
| FA-AC-70  | designed (ready)            |
| FA-AC-71  | designed (ready)            |
| FA-AC-72  | designed (ready)            |
| FA-AC-73  | designed (ready)            |
| FA-AC-74  | designed (ready)            |
| FA-AC-75  | designed (ready)            |
| FA-AC-76  | designed (ready)            |

The former 45-item design backlog is resolved at the proof-plan layer. The old rev-9 criteria now point to exact existing contract or component suites; launcher, report-retention, transcript-analysis, sidebar, and guardrail criteria have explicit oracles and negative controls; FA-AC-66 checks the exact rev-14 binding and complete coverage mechanically; and FA-AC-69..76 name the bounded MemoHarness contract/model suite that must exist before execution can pass. The last group is deliberately design-complete but observation-incomplete: a future file path is an executable implementation target, not evidence that the target already exists.

## custom-evidence-tier-ledger

Evidence-tier honesty table for every DESIGNED obligation, so no reader has to infer tier from the environment_refs alone.

| Criterion      | Real cited evidence                                                                                                      | Tier                                                                                                                   | Oracle-bite verified in this pass?                                                                                                                                                                                                                    |
| -------------- | ------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| FA-AC-06       | apps/openagents-desktop/tests/full-auto-registry.test.ts                                                                 | local_unit                                                                                                             | Not independently re-run in this authoring pass; cited from prior landed suite.                                                                                                                                                                       |
| FA-AC-15       | apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts                                                              | local_unit (real module composition, no live Electron/provider)                                                        | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-16       | apps/openagents-desktop/src/full-auto-retry-rotation-model.test.ts (NEW, added by this pass)                             | local_unit (bounded exhaustive enumeration over real exported pure functions)                                          | YES -- run directly against a standalone import of the real functions during authoring; see this pass's status comment for the observed values.                                                                                                       |
| FA-AC-17       | apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts                                                              | local_unit                                                                                                             | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-39/40    | apps/openagents-desktop/tests/full-auto-run-registry.test.ts                                                             | local_unit                                                                                                             | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-67       | apps/openagents-desktop/src/full-auto-retry-rotation-model.test.ts (NEW)                                                 | local_unit (bounded exhaustive enumeration)                                                                            | YES -- same standalone run as FA-AC-16.                                                                                                                                                                                                               |
| FA-AC-13/14    | apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts                                                              | local_unit                                                                                                             | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-38/41    | apps/openagents-desktop/tests/full-auto-run-registry.test.ts                                                             | local_unit                                                                                                             | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-42       | apps/openagents-desktop/tests/full-auto-thread-pressure.e2e.test.ts                                                      | local_unit (real thread-store/registry/journal composition)                                                            | Per the suite's own docstring: YES, reverting thread-store.ts's write() sort to compareDesktopThreadsByCreatedAt makes the retention tests fail -- oracle-bite recorded in the suite's own documentation, not independently re-verified by this pass. |
| FA-AC-43       | apps/openagents-desktop/tests/full-auto-run-registry.test.ts                                                             | local_unit (exhaustive 10x10 enumeration)                                                                              | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-44/45    | apps/openagents-desktop/src/full-auto-run-control-server.test.ts                                                         | local_contract_http (real Effect HTTP server in-process)                                                               | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-59       | apps/openagents-desktop/src/full-auto-run-handoff-control-server.test.ts                                                 | local_contract_http                                                                                                    | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-47       | apps/openagents-desktop/tests/full-auto-liveness.test.ts                                                                 | local_unit                                                                                                             | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-48       | apps/openagents-desktop/src/full-auto-run-liveness-control-server.test.ts                                                | local_contract_http                                                                                                    | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-46/51/52 | apps/openagents-desktop/src/full-auto-run-report.test.ts                                                                 | local_unit (52's redaction test is adversarial)                                                                        | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-58/60    | apps/openagents-desktop/src/full-auto-provider-handoff.test.ts                                                           | local_unit                                                                                                             | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-07/29    | apps/openagents-desktop/scripts/full-auto-restart-smoke.ts + apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts | local_dev_two_process_unsigned (conditionally falls back to unsigned dev-mode electron .; NOT proven signed/notarized) | Not executed in this authoring pass (would require a live two-process Electron launch on owned infrastructure).                                                                                                                                       |
| FA-AC-08       | apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts                                                              | local_unit                                                                                                             | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-61       | apps/openagents-desktop/src/renderer/react-composer.test.tsx                                                             | local_unit (React Testing Library component tier)                                                                      | Not independently re-run in this authoring pass.                                                                                                                                                                                                      |
| FA-AC-62-65    | docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json + acceptance driver negative controls                  | owner_real development, exact source `3123d926a3`; not signed/package/release evidence                                 | Six named rows and same-pass rotation are retained with public-safe digests; independent admission remains outstanding.                                                                                                                               |

"Not independently re-run in this authoring pass" means: the cited file was located, read, and its described behavior matches the claim above (dossier-verified), but this authoring pass did not itself execute `vp test` against the full suite as part of writing this document -- that execution is `pnpm run check`, reported separately in the issue status comment, not fabricated here as a receipt. FA-AC-16/67's new retry-rotation model test WAS run standalone during authoring (see above) precisely because it is new and unverified evidence, not carried-forward evidence from a previously-landed suite.

## custom-formal-model-status

The repository has NO TLA+ specification for Full Auto. `specs/` contains TLA+ models only for `khala-fleet-delegate`, `approval-protocol`, and `session-thread-mapping` (plus four mutation-testing variants under `specs/mutations/`) -- none reference `FullAutoRun`, leases, retry, or provider rotation. No `fast-check` (or equivalent property-testing library) dependency exists anywhere under `apps/openagents-desktop`.

What DOES exist, both real and bounded: (1) `apps/openagents-desktop/tests/full-auto-run-registry.test.ts`'s exhaustive enumeration over the real exported `isLegalFullAutoRunTransition` across the full 10x10 `FullAutoRunState` from/to product (FA-AC-43's oracle above). (2) This pass's new `apps/openagents-desktop/src/full-auto-retry-rotation-model.test.ts`, which extends bounded exhaustive coverage to the backoff schedule (`fullAutoFailureBackoffMs` over failures 0..11) and the rotation-reason classifier (`classifyFullAutoDispatchFailure` over its full real literal set and documented detail markers), composed against real exported production functions rather than a reimplementation (FA-AC-16/67's oracle above).

What remains genuinely unmodeled: the COMPOSED reachable state space of {lifecycle state x lease-claimed-boolean x retry-attempt-count x routing-candidate-index}. No artifact in this repository -- before or after this pass -- exhaustively checks that composition; the existing e2e scenario tests (apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts, apps/openagents-desktop/tests/full-auto-thread-pressure.e2e.test.ts) cover hand-picked overlapping scenarios, not the full reachable product. This is a named, disclosed residual, not a closed obligation: a future well-scoped follow-up could add a genuine TLA+ spec under `specs/desktop/full-auto-run/` mirroring the existing `specs/khala-fleet-delegate/` pattern (a supervisor process model with an explicit lease/retry/rotation state variable set and `specs/run-tlc.sh`-style pass/fail mutation configs), but building that model was judged out of safe scope for this single pass given the risk of a hastily-authored formal model asserting properties beyond what was actually verified against production code (ASSURANCE_SPEC.md Law 9: "a passing model... cannot... override contradictory runtime evidence" -- and an under-verified model is worse than an honestly absent one).

MemoHarness adds a second explicitly unmodeled bounded product:
{bundle release state x snapshot frozen x adaptation count x run terminality x
candidate release state x authority-delta-valid}. A future model should prove
at least: adaptation count never exceeds one; current-run labels/feedback are
unreachable before terminal state; effective bundle identity is immutable
after first dispatch; deleted/tombstoned experiences are ineligible; forbidden
authority deltas never reach dispatch; and candidate production activation is
unreachable without an independent release decision. This revision names the
model boundary but does not claim a model or implementation exists.

## custom-owner-gates

What this AssuranceSpec cannot self-provide and needs an explicit owner (or owner-designated independent reviewer) action:

1. **Admission.** This document must stay `lifecycle_state: "proposed"` until the owner or an owner-designated independent reviewer distinct from its producer reviews and admits it. No agent, including the one that authored this revision, may flip that field.
2. **Independent review of the owner-real development receipt (FA-AC-62..65).** The six rows and same-pass rotation exist at `docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json`; a reviewer distinct from this design producer must verify the candidate/test-definition binding, privacy properties, and dispositions. A newer source or packaged candidate needs a new receipt.
3. **Signed/notarized packaged-build resume proof (gates FA-AC-07/29 at release tier).** Requires running `apps/oa-updates/docs/release-signing-runbook.md` against a real packaged build, then re-running `pnpm run smoke:full-auto-restart` against that signed artifact specifically.
4. **Execution of design-complete but unobserved obligations.** No `needs_design` criterion remains. FA-AC-69..76 specifically require the named MemoHarness production seam and bounded contract/model suite, followed by independent privacy/release review; old run-loop evidence cannot close them.
5. **A future composed formal model (optional, not blocking).** See `custom-formal-model-status` above.
6. **MemoHarness policy and release decisions.** The owner or an explicitly designated independent reviewer must admit default experience retention/retrieval/training policy, any cross-workspace aggregate sharing, production adaptation policy, and candidate promotion beyond shadow/dogfood. The optimizer, executor, and source run are never eligible to supply their own release acceptance.
