---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.openagents.desktop.codex.workroom.mvp"
assurance_revision: 4
title: "OpenAgents Desktop Codex Workroom MVP AssuranceSpec"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "OpenAgents"
---

## Assurance Objective

### Revision 4 Expected-Working Surface Congruence (2026-07-14, proposed)

Owner decision (verbatim, 2026-07-14): "The AssuranceSpec needs to be fully
covering everything that we should expect to actually work, and then nothing
else, and ensure that that is specified in the contract."

This proposed revision makes the UX-4 expected-working dock allowlist and its
interaction proof coverage congruent. The checked-in matrix at
`docs/mvp/openagents-codex-workroom-mvp-assurance-coverage-matrix.md` maps
every allowed surface interaction to its ProductSpec criterion, assurance
item, behavior contract, and executable oracle. The normal Desktop sweep
diffs that matrix against the machine-readable allowlist and fails for both
missing coverage and coverage of a non-MVP surface. It explicitly incorporates
UX-1 composer focus, UX-2 full-catalog search, and UX-3 truthful history scope.

This is an AssuranceSpec revision, not a ProductSpec intent revision: the
accepted product criteria remain byte-bound to ProductSpec revision 6. This
proposal cannot admit itself or retarget any accepted run or receipt.

### Revision 3 Supersession Reconciliation (carried forward)

Owner decision (verbatim, 2026-07-14): "khala-code-desktop must itself be
deprecated and all relevant promises removed (OpenAgents desktop supercedes
it). ditto for apps/autopilot-desktop. sarah get rid of that too etc - i
dont give a shit wut u do just get that shit cleared out".

This proposed revision reconciles the assurance environment with the
owner-directed supersession removals: `apps/autopilot-desktop`,
`packages/sarah-take-scoreboard`, and `.agents/skills/khala-fleet` were
deleted from the workspace (recover any path with
`git show c7044f5a2870110b331c5a7288caceb85488290a:<path>`).
`clients/khala-code-desktop` was subsequently removed under the same owner
supersession direction. `packages/autopilot-ui` remains as an internal package.
it does not authorize an additional OpenAgents Desktop visible surface.

The full-sweep `test` command of record for this revision is the
post-removal root `package.json` aggregate, which no longer names
`test:sarah-take-scoreboard`, and the root manifest no longer declares the
`verify:autopilot-desktop:{training,deploy,if-changed}` or
`test:sarah-take-scoreboard` scripts:

```text
bun run test:sol-docs && bun run test:qa-pre-push-smoke && bun run test:qa-async-gce-trigger && bun run test:qa-nightly-matrix && bun run test:qa-visual-smoke-gate && bun run test:github-issue-triage && bun run test:khala-sync-runtime-dogfood-evidence && bun run test:ui-velocity-receipt && bun run test:bun-api-perimeter && bun run test:sqlite-runtime && bun run test:forge && bun run test:forum && bun run test:pylon && bun run test:pylon-core && bun run test:probe && bun run test:qa-runner && bun run test:khala-cli && bun run test:khala-mobile && bun run test:openagents-mobile && bun run test:khala-qa-harness && bun run test:khala-ai-sdk-core && bun run test:ai-sdk-sandbox-local && bun run test:ai-sdk-sandbox-openagents && bun run test:behavior-contracts && bun run test:assurance-spec && bun run test:agent-readiness && bun run test:nip90 && bun run test:arbiter-effect && bun run test:public-activity-timeline && bun run test:input-bindings && bun run test:design-tokens && bun run test:ui && bun run test:composer-state && bun run test:agent-runtime-schema && bun run test:khala-fleet-intents && bun run test:grok-harness && bun run test:harness-conformance && bun run test:reactor-contracts && bun run test:provider-account-schema && bun run test:effect-boundary && bun run test:effect-start && bun run test:khala-sync-db-collection && bun run test:blueprint-contracts && bun run test:connector-sidecar && bun run test:pipeline-signals && bun run test:khala-tools && bun run test:mcp-contract && bun run test:portable-session-contract && bun run test:environment-auth && bun run test:forge-protocol && bun run test:world-contract && bun run test:world-client && bun run test:openagents-world && bun run test:autopilot-ui && bun run test:oa-updates && bun run test:khala-capture && bun run test:khala-live-hub && bun run test:nostr-relay && bun run test:durable-stream && bun run test:openagents-desktop && bun run test:cloud-contract && bun run test:oa-infra && bun run test:oa-queue-worker && bun run test:aiur
```

The embedded `assurancespec-environments` repository inventory below is the
admitted-time machine-generated snapshot (head
`376d98fa4cc973af334b09a61e7ecba0dcae127a`, before the removals) and is
retained byte-for-byte: it is generated evidence bound to that head and
digest, so it is not hand-edited. It still names the removed
`apps/autopilot-desktop/**` candidate artifacts and
`apps/autopilot-desktop/package.json` /
`packages/sarah-take-scoreboard/package.json` declared scripts as
historical enumeration. It must be regenerated against the post-removal
head as part of admitting this revision. The admitted revision-2 document,
its evidence index, and its receipts remain byte-stable historical proof
and are not retargeted. Admitting this revision is a subsequent owner/gate
act per `specs/CONVENTIONS.md` and `docs/assurance/ASSURANCE_SPEC.md`.
Full removal record: `docs/refactor/2026-07-14-mvp-prune-ledger.md`
(Part 2) and `docs/promises/2026-07-14-owner-supersession-removals.md`.

Prove the accepted first-deployable OpenAgents Desktop Codex workroom against all eighteen frozen ProductSpec criteria. The run retains criterion-local candidate and falsifier observations, the signed/notarized installed journey, and the full current Desktop regression gate without collapsing their distinct authority or evidence tiers.

## Subject

This admitted assurance revision remains byte-bound to ProductSpec revision 6 and its legacy CW-AC identities. A later ProductSpec identity migration must create a new admission and may not retarget these receipts or rewrite this historical proof chain.

```assurancespec-subject
{
  "product_spec": {
    "criterion_refs": [
      "CW-AC-01",
      "CW-AC-02",
      "CW-AC-03",
      "CW-AC-04",
      "CW-AC-05",
      "CW-AC-06",
      "CW-AC-07",
      "CW-AC-08",
      "CW-AC-09",
      "CW-AC-10",
      "CW-AC-11",
      "CW-AC-12",
      "CW-AC-13",
      "CW-AC-14",
      "CW-AC-15",
      "CW-AC-16",
      "CW-AC-17",
      "CW-AC-18"
    ],
    "document_digest": "sha256:fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1",
    "path": "docs/mvp/openagents-codex-workroom-mvp.product-spec.md",
    "profile": "openagents_executable_v0.1_exact_document",
    "spec_format_version": "0.1",
    "spec_revision": 6
  }
}
```

## Risk Model

The proof design treats runtime compatibility, ordinary Codex-session custody, durable work identity, authority containment, restart safety, privacy, release lifecycle fidelity, and visible-surface proof congruence as separate risks. Candidate evidence is never sufficient without a named falsifier, an exact environment, independent review, and current immutable bindings.

```assurancespec-risks
{
  "risks": [
    {
      "id": "RISK-RUNTIME-CUSTODY",
      "statement": "A second engine, inherited CODEX_HOME, or account selector could violate ordinary logged-in Codex-session custody."
    },
    {
      "id": "RISK-WORK-AUTHORITY",
      "statement": "Agent prose or evidence presence could be mistaken for admission, verification, owner acceptance, or release authority."
    },
    {
      "id": "RISK-DURABILITY",
      "statement": "Reload, restart, retry, or update could duplicate work, flatten causal history, or silently retarget intent."
    },
    {
      "id": "RISK-PUBLIC-SAFETY",
      "statement": "Private native reports, credentials, paths, prompts, or repository content could leak into a public projection."
    },
    {
      "id": "RISK-SURFACE-CONGRUENCE",
      "statement": "An expected-working MVP interaction could lack an executable assurance item, or assurance coverage could preserve a removed non-MVP surface."
    }
  ],
  "source_digest": "sha256:2597133237cb20832d7c0f6f932548e666fa10ca4bf8d07b5f492750635f748a",
  "source_snapshot": "- Codex app-server evolves quickly. Compatibility must fail explicitly and\n  remain tied to a tested app/runtime set instead of silently parsing a changed\n  provider protocol.\n- The Runtime Gateway can accidentally become a second engine or database.\n  Any alternate model/tool loop or independent session truth blocks launch.\n- The built-in skill can accidentally become hidden authority. All durable\n  spec, plan, work-packet, criterion, and evidence transitions must remain\n  host-validated typed operations that the workroom can inspect.\n- ProductSpec ceremony can slow small work. The guided draft must be quick, and\n  the workroom must not require a spec for mechanical tasks outside the repo's\n  consequential-work threshold.\n- A polished timeline can hide data loss. Completeness, explicit gaps, durable\n  admission, and restart fault receipts remain acceptance requirements.\n- Reusing the ordinary Codex session can inherit stale or missing auth. The\n  host clears inherited `CODEX_HOME`, probes the exact current session, and\n  reports a precise sign-in or quota prerequisite without rotating elsewhere.\n- Read-only review may be too narrow for repeated daily use. That is a\n  falsifiable post-launch result, not permission to add editor/PTY/Git breadth\n  before the first complete workroom is accepted.\n- Opt-in metrics can bias toward expert dogfood users. Segment and consent\n  provenance must remain visible; no prompt, path, account, or machine identity\n  is collected to improve the number.\n- A raw Codex escape can hide an OpenAgents defect if external completion is\n  reported as workroom success. Every fallback must remain visible, and only an\n  exact OpenAgents rerun may convert that packet into OpenAgents-native proof.\n- Fleet capacity can be mistaken for available work. Concurrency is bounded by\n  distinct admitted packets, non-overlapping paths and hot contracts, and\n  review capacity—not connected accounts or idle workers.\n- Closed broader issues can tempt a premature claim. Only the exact current\n  artifact and MVP journey prove this spec; CUT-27 and portable/mobile/Fleet\n  claims retain their own gates."
}
```

## Assurance Scope

Every executable ProductSpec criterion is required and has exactly one criterion obligation in this MVP run. UX-5 additionally requires every expected-working dock interaction to map to those obligations, enforced behavior contracts, and executable oracles in the checked-in coverage matrix. The congruence sweep fails both omissions and non-MVP additions. No criterion is deferred or marked not applicable. Release and public-promise authority remain outside the execution grant even after all observations are confirmed.

## Environments

Execution uses the admitted first-party macOS ARM64 Bun environment with network and credential access forbidden. Native JUnit remains private. Normalized receipts expose only digests and bounded references. The historical signed RC9 receipt supplies release-artifact evidence and is not regenerated or published by this run.

```assurancespec-environments
{
  "profiles": [
    {
      "id": "ENV-OA-DESKTOP-MVP-BUN-1",
      "status": "admitted"
    }
  ],
  "repository_inventory": {
    "candidate_artifact_refs": [
      "apps/acceptance-runner/src/daemon.test.ts",
      "apps/acceptance-runner/src/e2e-local-proof.test.ts",
      "apps/aiur/src/admin-credits-proxy.test.ts",
      "apps/aiur/src/auth/access-route.test.ts",
      "apps/aiur/src/auth/access.test.ts",
      "apps/aiur/src/auth/owner-gate.test.ts",
      "apps/aiur/src/auth/routes.test.ts",
      "apps/aiur/src/cloudrun/server.test.ts",
      "apps/aiur/src/cloudrun/static.test.ts",
      "apps/aiur/src/credits/credits-action-state.test.ts",
      "apps/aiur/src/credits/credits-api-client.test.ts",
      "apps/aiur/src/credits/credits-console.test.tsx",
      "apps/aiur/src/dashboard/recent-users-panel.test.tsx",
      "apps/aiur/src/dashboard/tokens-served-sync.test.ts",
      "apps/aiur/src/khala-sync-proxy.test.ts",
      "apps/aiur/src/lib/relative-time-core.test.ts",
      "apps/aiur/src/ops/crm-batch-api-client.test.ts",
      "apps/aiur/src/ops/crm-batch-console.test.tsx",
      "apps/aiur/src/ops/crm-batch-selection.test.ts",
      "apps/aiur/src/ops/ops-api-client.test.ts",
      "apps/aiur/src/ops/ops-console.test.tsx",
      "apps/aiur/src/server.test.ts",
      "apps/aiur/vitest.config.ts",
      "apps/autopilot-desktop/src/shared/character-creation-onboarding.test.ts",
      "apps/autopilot-desktop/src/shared/chat-world-game-layer.test.ts",
      "apps/autopilot-desktop/src/shared/chat-world-multiplayer.test.ts",
      "apps/autopilot-desktop/src/shared/chat-world-scene.test.ts",
      "apps/autopilot-desktop/tests/account-management.test.ts",
      "apps/autopilot-desktop/tests/agent-onboarding.test.ts",
      "apps/autopilot-desktop/tests/agent-stream-projection.test.ts",
      "apps/autopilot-desktop/tests/ambient-browser-automation.test.ts",
      "apps/autopilot-desktop/tests/app-replica.test.ts",
      "apps/autopilot-desktop/tests/apple-fm-loopback-integration.test.ts",
      "apps/autopilot-desktop/tests/apple-fm-packaging.test.ts",
      "apps/autopilot-desktop/tests/approval-decision-ui.test.ts",
      "apps/autopilot-desktop/tests/artifact-receipt-browser.test.ts",
      "apps/autopilot-desktop/tests/asset-ingestion.test.ts",
      "apps/autopilot-desktop/tests/auto-approve-surface.test.ts",
      "apps/autopilot-desktop/tests/auto-update.test.ts",
      "apps/autopilot-desktop/tests/autonomous-loop.test.ts",
      "apps/autopilot-desktop/tests/black-screen-guard.test.ts",
      "apps/autopilot-desktop/tests/blueprint-chat-live.test.ts",
      "apps/autopilot-desktop/tests/builtin-agent.test.ts",
      "apps/autopilot-desktop/tests/capture-scene-headless.test.ts",
      "apps/autopilot-desktop/tests/chat-world-character.test.ts",
      "apps/autopilot-desktop/tests/chat-world-cloudflare.test.ts",
      "apps/autopilot-desktop/tests/chat-world-forum-activity.test.ts",
      "apps/autopilot-desktop/tests/chat-world-scene.test.ts",
      "apps/autopilot-desktop/tests/chat-world-subscriptions.test.ts",
      "apps/autopilot-desktop/tests/chat-world-visualization.test.ts",
      "apps/autopilot-desktop/tests/cl-53-foldkit.test.ts",
      "apps/autopilot-desktop/tests/cl-53-sanitize.test.ts",
      "apps/autopilot-desktop/tests/code-mode-account-routing.test.ts",
      "apps/autopilot-desktop/tests/code-mode-sync.test.ts",
      "apps/autopilot-desktop/tests/composer-workspace.test.ts",
      "apps/autopilot-desktop/tests/composer.test.ts",
      "apps/autopilot-desktop/tests/conformance.test.ts",
      "apps/autopilot-desktop/tests/control-verbs.test.ts",
      "apps/autopilot-desktop/tests/crackling-arc-pixel-regression.test.ts",
      "apps/autopilot-desktop/tests/desktop-style-palette.test.ts",
      "apps/autopilot-desktop/tests/deterministic-env.test.ts",
      "apps/autopilot-desktop/tests/diff-artifacts-pane.test.ts",
      "apps/autopilot-desktop/tests/diff-transcript.test.ts",
      "apps/autopilot-desktop/tests/electrobun-config.test.ts",
      "apps/autopilot-desktop/tests/forum-intro.test.ts",
      "apps/autopilot-desktop/tests/forum-loop-bounds.test.ts",
      "apps/autopilot-desktop/tests/forum-tip-recipient.test.ts",
      "apps/autopilot-desktop/tests/forum-tipping-multiplayer-integration.test.ts",
      "apps/autopilot-desktop/tests/forum-work-search.test.ts",
      "apps/autopilot-desktop/tests/full-input-path-harness.test.ts",
      "apps/autopilot-desktop/tests/harnesses/chat-world-integration-harness.ts",
      "apps/autopilot-desktop/tests/headless/.gitignore",
      "apps/autopilot-desktop/tests/headless/capture-verse-arc-headless.ts",
      "apps/autopilot-desktop/tests/headless/verse-game-screen.real-scene.headless.png",
      "apps/autopilot-desktop/tests/headless/verse-spawned-arc-harness.ts",
      "apps/autopilot-desktop/tests/headless/verse-spawned-arc.after-movement.headless.png",
      "apps/autopilot-desktop/tests/headless/verse-spawned-arc.headless.png",
      "apps/autopilot-desktop/tests/headless/verse-spawned-arc.html",
      "apps/autopilot-desktop/tests/headless/verse-spawned-arc.real-scene.headless.png",
      "apps/autopilot-desktop/tests/host-diagnostics-projection.test.ts",
      "apps/autopilot-desktop/tests/hud-status-projection.test.ts",
      "apps/autopilot-desktop/tests/identity-choice.test.ts",
      "apps/autopilot-desktop/tests/inference-gateway.test.ts",
      "apps/autopilot-desktop/tests/inference-routing-model.test.ts",
      "apps/autopilot-desktop/tests/inference-routing.test.ts",
      "apps/autopilot-desktop/tests/install-readiness.test.ts",
      "apps/autopilot-desktop/tests/isolated-scene-render-gate.test.ts",
      "apps/autopilot-desktop/tests/isolated-scene-runner.test.ts",
      "apps/autopilot-desktop/tests/khala-cockpit.test.ts",
      "apps/autopilot-desktop/tests/loopback-preview.test.ts",
      "apps/autopilot-desktop/tests/managed-worktree-resolve.test.ts",
      "apps/autopilot-desktop/tests/mcp-contract-import.test.ts",
      "apps/autopilot-desktop/tests/nav-shell.test.ts",
      "apps/autopilot-desktop/tests/node-home.test.ts",
      "apps/autopilot-desktop/tests/node-launcher.test.ts",
      "apps/autopilot-desktop/tests/node-state-poll.test.ts",
      "apps/autopilot-desktop/tests/notifier.test.ts",
      "apps/autopilot-desktop/tests/onboarding-status.test.ts",
      "apps/autopilot-desktop/tests/onboarding-wizard.test.ts",
      "apps/autopilot-desktop/tests/pane-layer.test.ts",
      "apps/autopilot-desktop/tests/pdf-production.test.ts",
      "apps/autopilot-desktop/tests/preferences.test.ts",
      "apps/autopilot-desktop/tests/preload.ts",
      "apps/autopilot-desktop/tests/promise-surfacing.test.ts",
      "apps/autopilot-desktop/tests/public-activity-timeline.test.ts",
      "apps/autopilot-desktop/tests/pylon-base-scene.test.ts",
      "apps/autopilot-desktop/tests/pylon-control.test.ts",
      "apps/autopilot-desktop/tests/pylon-fleet-reconciliation.test.ts",
      "apps/autopilot-desktop/tests/pylon-network-glow-regression.test.ts",
      "apps/autopilot-desktop/tests/pylon-network-scene.test.ts",
      "apps/autopilot-desktop/tests/pylon-network-stats.test.ts",
      "apps/autopilot-desktop/tests/pylon-network-visualization.test.ts",
      "apps/autopilot-desktop/tests/session-event-stream.test.ts",
      "apps/autopilot-desktop/tests/session-pane.test.ts",
      "apps/autopilot-desktop/tests/shell-turn.test.ts",
      "apps/autopilot-desktop/tests/stream-render.test.ts",
      "apps/autopilot-desktop/tests/stylex-removal-headless-mount.test.ts",
      "apps/autopilot-desktop/tests/stylex-removal-theme-preserving.test.ts",
      "apps/autopilot-desktop/tests/swarm-batch.test.ts",
      "apps/autopilot-desktop/tests/swarm.test.ts",
      "apps/autopilot-desktop/tests/terminal-log-pane.test.ts",
      "apps/autopilot-desktop/tests/theme.test.ts",
      "apps/autopilot-desktop/tests/training-runs.test.ts",
      "apps/autopilot-desktop/tests/update-feed.test.ts",
      "apps/autopilot-desktop/tests/verse-bulletin-board.test.ts",
      "apps/autopilot-desktop/tests/verse-code-dock.test.ts",
      "apps/autopilot-desktop/tests/verse-game-screen.test.ts",
      "apps/autopilot-desktop/tests/verse-hud-action-model.test.ts",
      "apps/autopilot-desktop/tests/verse-khala-effect.test.ts",
      "apps/autopilot-desktop/tests/verse-launch-checklist.test.ts",
      "apps/autopilot-desktop/tests/verse-progress-diagnostics-model.test.ts",
      "apps/autopilot-desktop/tests/verse-run-hud.test.ts",
      "apps/autopilot-desktop/tests/verse-scene-helpers.test.ts",
      "apps/autopilot-desktop/tests/verse-spawned-scene.test.ts",
      "apps/autopilot-desktop/tests/verse-toggle.test.ts",
      "apps/autopilot-desktop/tests/verse-training-visualization.test.ts",
      "apps/autopilot-desktop/tests/verse-turn.test.ts",
      "apps/autopilot-desktop/tests/zero-base-shell.test.ts",
      "apps/forge/src/index.test.ts",
      "apps/forum/src/index.test.ts",
      "apps/khala-capture/src/deploy-contract.test.ts",
      "apps/khala-live-hub/src/credit-balance-live-delivery.test.ts",
      "apps/khala-live-hub/src/scope-hub.test.ts",
      "apps/khala-live-hub/src/server.test.ts",
      "apps/khala-live-hub/src/service.test.ts",
      "apps/nostr-relay/src/general-policy.test.ts",
      "apps/nostr-relay/src/market-policy.test.ts",
      "apps/oa-queue-worker/src/pump.test.ts",
      "apps/oa-updates/src/asset-store.test.ts",
      "apps/oa-updates/src/code-signing.test.ts",
      "apps/oa-updates/src/desktop-release.test.ts",
      "apps/oa-updates/src/desktop-seed.test.ts",
      "apps/oa-updates/src/export-reader.test.ts",
      "apps/oa-updates/src/legacy-desktop-lockout.test.ts",
      "apps/oa-updates/src/manifest-resolver.test.ts",
      "apps/oa-updates/src/manifest-validate.test.ts",
      "apps/oa-updates/src/multipart-body.test.ts",
      "apps/oa-updates/src/node-registry.test.ts",
      "apps/oa-updates/src/openagents-desktop-publish-serve.test.ts",
      "apps/oa-updates/src/openagents-desktop-release.test.ts",
      "apps/oa-updates/src/openagents-desktop-seed.test.ts",
      "apps/oa-updates/src/publish-builder.test.ts",
      "apps/oa-updates/src/publish.test.ts",
      "apps/oa-updates/src/pylon-release.test.ts",
      "apps/oa-updates/src/pylon-seed.test.ts",
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
      "apps/openagents-desktop/src/builtin-productspec-skill.test.ts",
      "apps/openagents-desktop/src/chat-service.test.ts",
      "apps/openagents-desktop/src/codex-app-server-client.test.ts",
      "apps/openagents-desktop/src/codex-child-runtime.test.ts",
      "apps/openagents-desktop/src/codex-connection-signatures.test.ts",
      "apps/openagents-desktop/src/codex-handoff-host.test.ts",
      "apps/openagents-desktop/src/codex-handoff-integration.test.ts",
      "apps/openagents-desktop/src/codex-handoff.test.ts",
      "apps/openagents-desktop/src/codex-history-host.test.ts",
      "apps/openagents-desktop/src/codex-history-utility.test.ts",
      "apps/openagents-desktop/src/codex-local-runtime.test.ts",
      "apps/openagents-desktop/src/codex-preflight.test.ts",
      "apps/openagents-desktop/src/desktop-host-lifecycle.test.ts",
      "apps/openagents-desktop/src/desktop-operation-context.test.ts",
      "apps/openagents-desktop/src/desktop-renderer-location.test.ts",
      "apps/openagents-desktop/src/desktop-runtime-workspace.test.ts",
      "apps/openagents-desktop/src/desktop-worker-location.test.ts",
      "apps/openagents-desktop/src/extension-lifecycle-contract.test.ts",
      "apps/openagents-desktop/src/fable-local-runtime-caps.test.ts",
      "apps/openagents-desktop/src/fable-local-runtime.test.ts",
      "apps/openagents-desktop/src/git-github-contract.test.ts",
      "apps/openagents-desktop/src/git-github-host.test.ts",
      "apps/openagents-desktop/src/git-review-corpus.node.test.ts",
      "apps/openagents-desktop/src/history-thread-actions.test.ts",
      "apps/openagents-desktop/src/live-agent-graph-host.test.ts",
      "apps/openagents-desktop/src/live-agent-graph-local.test.ts",
      "apps/openagents-desktop/src/live-proof.test.ts",
      "apps/openagents-desktop/src/local-runtime-event-persistence.test.ts",
      "apps/openagents-desktop/src/local-turn-journal.test.ts",
      "apps/openagents-desktop/src/local-turn-text-persistence.test.ts",
      "apps/openagents-desktop/src/macos-update-applier.test.ts",
      "apps/openagents-desktop/src/mcp-config-host.test.ts",
      "apps/openagents-desktop/src/mvp-proof.test.ts",
      "apps/openagents-desktop/src/product-spec-app-server-tools.test.ts",
      "apps/openagents-desktop/src/product-spec-workroom.test.ts",
      "apps/openagents-desktop/src/provider-runtime-compatibility.test.ts",
      "apps/openagents-desktop/src/provider-runtime-host.test.ts",
      "apps/openagents-desktop/src/provider-runtime-target.test.ts",
      "apps/openagents-desktop/src/renderer/command-notice.test.ts",
      "apps/openagents-desktop/src/renderer/composer-images.test.ts",
      "apps/openagents-desktop/src/renderer/composer-shortcuts.test.ts",
      "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
      "apps/openagents-desktop/src/renderer/diagnostics.test.ts",
      "apps/openagents-desktop/src/renderer/fleet-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/git-panel.test.ts",
      "apps/openagents-desktop/src/renderer/history-restore.test.ts",
      "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/local-harness.test.ts",
      "apps/openagents-desktop/src/renderer/markdown.test.ts",
      "apps/openagents-desktop/src/renderer/product-spec-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/runtime-agent-graph.test.ts",
      "apps/openagents-desktop/src/renderer/runtime-cards.test.ts",
      "apps/openagents-desktop/src/renderer/runtime-conversation.test.ts",
      "apps/openagents-desktop/src/renderer/runtime-interactions.test.ts",
      "apps/openagents-desktop/src/renderer/runtime-live-client.test.ts",
      "apps/openagents-desktop/src/renderer/settings.test.ts",
      "apps/openagents-desktop/src/renderer/shell.test.ts",
      "apps/openagents-desktop/src/renderer/sidebar-accounts.test.ts",
      "apps/openagents-desktop/src/renderer/skill-invocation.test.ts",
      "apps/openagents-desktop/src/renderer/terminal-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/tool-cards.test.ts",
      "apps/openagents-desktop/src/renderer/voice-actions.test.ts",
      "apps/openagents-desktop/src/renderer/voice-mode.test.ts",
      "apps/openagents-desktop/src/renderer/workspace-browser.test.ts",
      "apps/openagents-desktop/src/renderer/workspace-editor.test.ts",
      "apps/openagents-desktop/src/runtime-live-subscriptions.test.ts",
      "apps/openagents-desktop/src/terminal-host.test.ts",
      "apps/openagents-desktop/src/thread-store.test.ts",
      "apps/openagents-desktop/src/update-staging-host.test.ts",
      "apps/openagents-desktop/src/update-staging-integration.test.ts",
      "apps/openagents-desktop/src/usage-ledger.test.ts",
      "apps/openagents-desktop/src/voice-host.test.ts",
      "apps/openagents-desktop/src/voice-native-helper.test.ts",
      "apps/openagents-desktop/src/voice-permission-policy.test.ts",
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
      "apps/openagents-desktop/tests/diagnostics.test.ts",
      "apps/openagents-desktop/tests/electron-boundary.test.ts",
      "apps/openagents-desktop/tests/electron-trace-acceptance.test.ts",
      "apps/openagents-desktop/tests/fixtures/claude-smoke/projects/openagents-desktop/11111111-2222-3333-4444-555555555555.jsonl",
      "apps/openagents-desktop/tests/fixtures/codex-smoke/session_index.jsonl",
      "apps/openagents-desktop/tests/fixtures/codex-smoke/sessions/2026/07/11/smoke-child.jsonl",
      "apps/openagents-desktop/tests/fixtures/codex-smoke/sessions/2026/07/11/smoke-root.jsonl",
      "apps/openagents-desktop/tests/fixtures/provider-accounts/accounts-list.json",
      "apps/openagents-desktop/tests/fleet-control.test.ts",
      "apps/openagents-desktop/tests/git-fixture.ts",
      "apps/openagents-desktop/tests/history-search.test.ts",
      "apps/openagents-desktop/tests/isolated-app-proof.test.ts",
      "apps/openagents-desktop/tests/local-first-identity.e2e.test.ts",
      "apps/openagents-desktop/tests/local-turn-restart.e2e.test.ts",
      "apps/openagents-desktop/tests/native-conversation-continuation.e2e.test.ts",
      "apps/openagents-desktop/tests/native-timeline-fault-convergence.e2e.test.ts",
      "apps/openagents-desktop/tests/notification-attention.test.ts",
      "apps/openagents-desktop/tests/package-macos.test.ts",
      "apps/openagents-desktop/tests/plugin-config.test.ts",
      "apps/openagents-desktop/tests/provider-accounts.test.ts",
      "apps/openagents-desktop/tests/publish-release.test.ts",
      "apps/openagents-desktop/tests/release-preflight.test.ts",
      "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
      "apps/openagents-desktop/tests/service-topology.test.ts",
      "apps/openagents-desktop/tests/update-contract.test.ts",
      "apps/openagents-desktop/tests/update-rollback.test.ts",
      "apps/openagents-desktop/tests/voice-boundary.test.ts",
      "apps/openagents-desktop/tests/voice-runtime-gateway.test.ts",
      "apps/openagents-desktop/tests/workspace-scale.e2e.test.ts",
      "apps/openagents-desktop/tests/workspace-service.test.ts",
      "apps/openagents-mobile/tests/app-identity.test.ts",
      "apps/openagents-mobile/tests/authoritative-home.test.ts",
      "apps/openagents-mobile/tests/component-sharing.test.ts",
      "apps/openagents-mobile/tests/home-shell-core.test.ts",
      "apps/openagents-mobile/tests/khala-surface.test.ts",
      "apps/openagents-mobile/tests/local-first-identity.e2e.test.ts",
      "apps/openagents-mobile/tests/mobile-accessibility.test.ts",
      "apps/openagents-mobile/tests/mobile-account-control.test.ts",
      "apps/openagents-mobile/tests/mobile-agent-graph.test.ts",
      "apps/openagents-mobile/tests/mobile-coding-attachment-delivery.test.ts",
      "apps/openagents-mobile/tests/mobile-coding-attachment-picker.test.ts",
      "apps/openagents-mobile/tests/mobile-coding-composer.test.ts",
      "apps/openagents-mobile/tests/mobile-coding-navigation.test.ts",
      "apps/openagents-mobile/tests/mobile-conversation.test.ts",
      "apps/openagents-mobile/tests/mobile-execution-targets.test.ts",
      "apps/openagents-mobile/tests/mobile-experience-reconciler.test.ts",
      "apps/openagents-mobile/tests/mobile-sync-host.test.ts",
      "apps/openagents-mobile/tests/native-coding-target-delivery.test.ts",
      "apps/openagents-mobile/tests/native-session-pkce.test.ts",
      "apps/openagents-mobile/tests/native-session-recovery.test.ts",
      "apps/openagents-mobile/tests/native-session-vault.test.ts",
      "apps/openagents-mobile/tests/ota-polling.test.ts",
      "apps/openagents-world/src/bridge.test.ts",
      "apps/openagents-world/src/commands.test.ts",
      "apps/openagents-world/src/expiry.test.ts",
      "apps/openagents-world/src/moderation.test.ts",
      "apps/openagents-world/src/protocol.test.ts",
      "apps/openagents-world/src/replay-buffer.test.ts",
      "apps/openagents-world/src/subscriptions.test.ts",
      "apps/openagents.com/apps/start/src/forum-entry.test.ts",
      "apps/openagents.com/apps/start/src/khala-sync-proxy.test.ts",
      "apps/openagents.com/apps/start/src/routes/-activity.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-artanis-accounts.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-artanis-console.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-artanis-traces.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-business-kpi.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-chat-sync-web-core.test.ts",
      "apps/openagents.com/apps/start/src/routes/-chat-sync.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-clients-preview.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-code.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-components.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-download-effect-native.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-forum-markdown.test.ts",
      "apps/openagents.com/apps/start/src/routes/-forum-tips.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-forum.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-funnel.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-gym.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-index.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-khala-effect-native.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-landing-en.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-landing-preview.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-login.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-mirrorcode.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-onboarding.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-portal.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-privacy-effect-native.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-promises.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-public-agent.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-pylon-codex-assignment-status.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-pylons.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-run.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-sales-landing.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-share.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-site-checkout-demo.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-stage1-effect-native.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-stats.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-tassadar-effect-native.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-terms.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-training-runs-deprecated.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-training-runs.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-workspace-invite.test.tsx",
      "apps/openagents.com/apps/start/src/routes/code/-retirement.test.ts",
      "apps/openagents.com/apps/start/src/server-agent-surfaces.test.ts",
      "apps/openagents.com/apps/start/src/server.test.ts",
      "apps/openagents.com/apps/start/src/test/bun-builtin-stub.ts",
      "apps/openagents.com/apps/start/vitest.config.ts",
      "apps/openagents.com/apps/web/src/autopilot-route.test.ts",
      "apps/openagents.com/apps/web/src/business-route.test.ts",
      "apps/openagents.com/apps/web/src/client-server-route-agreement.test.ts",
      "apps/openagents.com/apps/web/src/commands/session.test.ts",
      "apps/openagents.com/apps/web/src/demo-legal-route.test.ts",
      "apps/openagents.com/apps/web/src/docs-blog-route.test.ts",
      "apps/openagents.com/apps/web/src/download-route.test.ts",
      "apps/openagents.com/apps/web/src/forum-route.test.ts",
      "apps/openagents.com/apps/web/src/forum-theme.test.ts",
      "apps/openagents.com/apps/web/src/gym-route.test.ts",
      "apps/openagents.com/apps/web/src/icon-policy.test.ts",
      "apps/openagents.com/apps/web/src/index-html.test.ts",
      "apps/openagents.com/apps/web/src/json-boundary.test.ts",
      "apps/openagents.com/apps/web/src/khala-code-download-route.test.ts",
      "apps/openagents.com/apps/web/src/main.test.ts",
      "apps/openagents.com/apps/web/src/mcp-contract-import.test.ts",
      "apps/openagents.com/apps/web/src/navigation-policy.test.ts",
      "apps/openagents.com/apps/web/src/new-landing-route.test.ts",
      "apps/openagents.com/apps/web/src/page/artanisTraceTree.test.ts",
      "apps/openagents.com/apps/web/src/page/autopilot-onboarding/component-catalog.test.ts",
      "apps/openagents.com/apps/web/src/page/autopilot-onboarding/component-renderer.test.ts",
      "apps/openagents.com/apps/web/src/page/autopilot-onboarding/flow.test.ts"
    ],
    "declared_scripts": [
      {
        "command": "playwright install --with-deps chromium",
        "manifest_path": "apps/acceptance-runner/package.json",
        "name": "playwright:install"
      },
      {
        "command": "bun run src/run-once.ts",
        "manifest_path": "apps/acceptance-runner/package.json",
        "name": "run-once"
      },
      {
        "command": "bun run src/service.ts",
        "manifest_path": "apps/acceptance-runner/package.json",
        "name": "serve"
      },
      {
        "command": "bun test src/daemon.test.ts",
        "manifest_path": "apps/acceptance-runner/package.json",
        "name": "test"
      },
      {
        "command": "vite build --logLevel warn",
        "manifest_path": "apps/aiur/package.json",
        "name": "build"
      },
      {
        "command": "vite build --config vite.config.cloudrun.ts --logLevel warn && bun build src/cloudrun/server.ts --target=bun --outdir dist/cloudrun --entry-naming server.js",
        "manifest_path": "apps/aiur/package.json",
        "name": "build:cloudrun"
      },
      {
        "command": "wrangler types",
        "manifest_path": "apps/aiur/package.json",
        "name": "cf:typegen"
      },
      {
        "command": "bun run build && wrangler deploy",
        "manifest_path": "apps/aiur/package.json",
        "name": "deploy"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh",
        "manifest_path": "apps/aiur/package.json",
        "name": "deploy:cloudrun"
      },
      {
        "command": "vite dev",
        "manifest_path": "apps/aiur/package.json",
        "name": "dev"
      },
      {
        "command": "vite preview",
        "manifest_path": "apps/aiur/package.json",
        "name": "preview"
      },
      {
        "command": "vitest run",
        "manifest_path": "apps/aiur/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/aiur/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun run build:css && bun run build:pylon-node && electrobun build",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "build"
      },
      {
        "command": "bun run build:css && bun run build:pylon-node && electrobun build --env=canary",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "build:canary"
      },
      {
        "command": "bun scripts/build-css.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "build:css"
      },
      {
        "command": "bun build ../pylon/src/index.ts --target bun --outfile resources/pylon-node/index.js",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "build:pylon-node"
      },
      {
        "command": "bun run build:css && bun run build:pylon-node && electrobun build --env=stable",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "build:stable"
      },
      {
        "command": "bun scripts/capture-scene-headless.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "capture-scene-headless"
      },
      {
        "command": "bun scripts/capture-scene-headless.ts verse-arc tests/headless/verse-arc.capture.png",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "capture-scene:verse-arc"
      },
      {
        "command": "bun run build:css && electrobun dev",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "dev"
      },
      {
        "command": "bash scripts/notarize-macos.sh",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "notarize:macos"
      },
      {
        "command": "bun scripts/account-picker-proof.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "proof:account-picker"
      },
      {
        "command": "bun run build:css && bun test tests/app-replica.test.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "proof:app-replica"
      },
      {
        "command": "bun scripts/auto-onboarding-headless-proof.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "proof:auto-onboarding"
      },
      {
        "command": "bun scripts/composer-loop-proof.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "proof:composer"
      },
      {
        "command": "bun scripts/crackling-arc-pixel-regression.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "proof:crackling-arc-pixels"
      },
      {
        "command": "bun --preload ./tests/preload.ts scripts/shell-control-proof.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "proof:shell-control"
      },
      {
        "command": "bun scripts/swarm-view-proof.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "proof:swarm"
      },
      {
        "command": "bun scripts/transcript-proof.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "proof:transcript"
      },
      {
        "command": "bun tests/headless/capture-verse-arc-headless.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "proof:verse-arc"
      },
      {
        "command": "bun scripts/run-bounded.ts 480000 -- bun scripts/verse-launch-smoke.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "proof:verse-coding-overlay"
      },
      {
        "command": "bun scripts/run-isolated-scene.ts pylon-network",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "scene:pylon-network"
      },
      {
        "command": "bun scripts/run-isolated-scene.ts verse-arc",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "scene:verse-arc"
      },
      {
        "command": "bun scripts/apple-fm-live-smoke.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "smoke:apple-fm-local"
      },
      {
        "command": "bun scripts/auto-onboarding-e2e-smoke.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "smoke:auto-onboarding-e2e"
      },
      {
        "command": "bun test tests/forum-tipping-multiplayer-integration.test.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "smoke:forum-tipping-multiplayer"
      },
      {
        "command": "bun scripts/forum-verse-reflection-smoke.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "smoke:forum-verse-reflection"
      },
      {
        "command": "bun scripts/khala-cockpit-smoke.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "smoke:khala-cockpit"
      },
      {
        "command": "bun scripts/training-scene-canvas-smoke.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "smoke:training-scene"
      },
      {
        "command": "bun scripts/run-bounded.ts 480000 -- bun scripts/verse-launch-smoke.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "smoke:verse-launch"
      },
      {
        "command": "bun scripts/verse-launch-smoke.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "smoke:verse-launch:unbounded"
      },
      {
        "command": "bash scripts/run-tests.sh",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "test"
      },
      {
        "command": "bun test tests/verse-launch-checklist.test.ts tests/verse-toggle.test.ts tests/chat-world-visualization.test.ts tests/chat-world-cloudflare.test.ts tests/chat-world-subscriptions.test.ts tests/forum-tipping-multiplayer-integration.test.ts tests/verse-training-visualization.test.ts tests/verse-bulletin-board.test.ts tests/verse-scene-helpers.test.ts tests/verse-turn.test.ts",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "test:verse-launch"
      },
      {
        "command": "tsc -p tsconfig.typecheck.json --noEmit",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun run typecheck && bun test tests/electrobun-config.test.ts && bun run verify:training && bun run test:verse-launch && bun run build && bun scripts/verify-packaged-moksha-asset.ts && bun run smoke:verse-launch",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "verify:deploy"
      },
      {
        "command": "bun test tests/cl-53-foldkit.test.ts tests/cl-53-sanitize.test.ts && bun run build:css && bun build src/ui/main.ts --outdir /tmp/autopilot-desktop-ui-build --target browser && bun build src/bun/index.ts --outdir /tmp/autopilot-desktop-bun-build --target bun && bun run smoke:training-scene",
        "manifest_path": "apps/autopilot-desktop/package.json",
        "name": "verify:training"
      },
      {
        "command": "wrangler deploy",
        "manifest_path": "apps/forge/package.json",
        "name": "deploy"
      },
      {
        "command": "wrangler dev --port 8792",
        "manifest_path": "apps/forge/package.json",
        "name": "dev"
      },
      {
        "command": "bun test src",
        "manifest_path": "apps/forge/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/forge/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun src/index.ts",
        "manifest_path": "apps/forum/package.json",
        "name": "dev"
      },
      {
        "command": "bun test",
        "manifest_path": "apps/forum/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/forum/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun build src/server.ts --target=bun --outdir dist --entry-naming server.js",
        "manifest_path": "apps/khala-capture/package.json",
        "name": "build:cloudrun"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh",
        "manifest_path": "apps/khala-capture/package.json",
        "name": "deploy:cloudrun"
      },
      {
        "command": "bun --watch src/server.ts",
        "manifest_path": "apps/khala-capture/package.json",
        "name": "dev"
      },
      {
        "command": "bun src/server.ts",
        "manifest_path": "apps/khala-capture/package.json",
        "name": "start"
      },
      {
        "command": "bun test",
        "manifest_path": "apps/khala-capture/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/khala-capture/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun build src/server.ts --target=bun --outdir dist --entry-naming server.js",
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "build:cloudrun"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh",
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "deploy:cloudrun"
      },
      {
        "command": "bun --watch src/server.ts",
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "dev"
      },
      {
        "command": "bun src/server.ts",
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "start"
      },
      {
        "command": "bun test",
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "typecheck"
      },
      {
        "command": "wrangler deploy",
        "manifest_path": "apps/nostr-relay/package.json",
        "name": "deploy"
      },
      {
        "command": "wrangler dev --port 8787",
        "manifest_path": "apps/nostr-relay/package.json",
        "name": "dev"
      },
      {
        "command": "bun scripts/smoke.ts",
        "manifest_path": "apps/nostr-relay/package.json",
        "name": "smoke"
      },
      {
        "command": "bun test src",
        "manifest_path": "apps/nostr-relay/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/nostr-relay/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun build src/main.ts --target=bun --outdir dist --entry-naming server.js",
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "build"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh",
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "deploy"
      },
      {
        "command": "bun run src/main.ts",
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "serve"
      },
      {
        "command": "bun test src",
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun run scripts/publish-desktop-release.ts",
        "manifest_path": "apps/oa-updates/package.json",
        "name": "desktop:publish"
      },
      {
        "command": "bun run src/server.ts",
        "manifest_path": "apps/oa-updates/package.json",
        "name": "serve"
      },
      {
        "command": "bun test",
        "manifest_path": "apps/oa-updates/package.json",
        "name": "test"
      },
      {
        "command": "bun build src/cloudrun.ts --target=bun --outfile=dist/server.js",
        "manifest_path": "apps/openagents-audio-edge/package.json",
        "name": "build"
      },
      {
        "command": "bash deploy-cloudrun.sh",
        "manifest_path": "apps/openagents-audio-edge/package.json",
        "name": "deploy"
      },
      {
        "command": "bun build src/main.ts --target=bun --outdir dist --entry-naming server.js",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "build:cloudrun"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "deploy:cloudrun"
      },
      {
        "command": "bun scripts/live-barge-smoke.ts",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "smoke:barge:live"
      },
      {
        "command": "bun scripts/live-retention-smoke.ts",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "smoke:live"
      },
      {
        "command": "bun scripts/live-tts-smoke.ts",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "smoke:tts:live"
      },
      {
        "command": "bun src/main.ts",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "start"
      },
      {
        "command": "bun test src test",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun scripts/build.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "build"
      },
      {
        "command": "bun scripts/build.ts && electron .",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "dev"
      },
      {
        "command": "bun scripts/build.ts && bun scripts/run-live-proof.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "live-proof"
      },
      {
        "command": "bun scripts/prepare-macos-maker.ts && electron-forge make --platform=darwin --arch=arm64",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "make:mac"
      },
      {
        "command": "bun scripts/build.ts && bun scripts/run-mvp-proof.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "mvp-proof"
      },
      {
        "command": "electron-forge package --platform=darwin --arch=arm64",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "package:mac"
      },
      {
        "command": "bun scripts/build.ts && OPENAGENTS_DESKTOP_SMOKE=1 electron .",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke"
      },
      {
        "command": "bun scripts/startup-bench.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "startup-bench"
      },
      {
        "command": "bun test",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun run typecheck && bun test && bun run build && OPENAGENTS_DESKTOP_SMOKE=1 bun run smoke",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "verify"
      },
      {
        "command": "expo start",
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "dev"
      },
      {
        "command": "expo prebuild",
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "prebuild"
      },
      {
        "command": "expo prebuild --platform android",
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "prebuild:android"
      },
      {
        "command": "expo prebuild --platform ios",
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "prebuild:ios"
      },
      {
        "command": "bash ../../apps/oa-updates/scripts/publish-ota.sh",
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "publish:ota"
      },
      {
        "command": "bun test",
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "typecheck"
      },
      {
        "command": "wrangler deploy --env=\"\"",
        "manifest_path": "apps/openagents-world/package.json",
        "name": "deploy"
      },
      {
        "command": "wrangler dev --port 8791",
        "manifest_path": "apps/openagents-world/package.json",
        "name": "dev"
      },
      {
        "command": "bun test src",
        "manifest_path": "apps/openagents-world/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents-world/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun run build && bun run src/routes/-funnel-budget.ts",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "budget"
      },
      {
        "command": "vite build --logLevel warn",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "build"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh stage1",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "deploy"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "deploy:cloudrun"
      },
      {
        "command": "vite dev",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "dev"
      },
      {
        "command": "vite preview",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "preview"
      },
      {
        "command": "vitest run",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "typecheck"
      },
      {
        "command": "vite build && vite build --config vite.lander3.config.ts",
        "manifest_path": "apps/openagents.com/apps/web/package.json",
        "name": "build"
      },
      {
        "command": "bun run scripts/capture-khala-tokens-history.ts",
        "manifest_path": "apps/openagents.com/apps/web/package.json",
        "name": "capture:khala-tokens-history"
      },
      {
        "command": "bun run scripts/capture-khala-tokens-pill.ts",
        "manifest_path": "apps/openagents.com/apps/web/package.json",
        "name": "capture:khala-tokens-pill"
      },
      {
        "command": "bun run scripts/capture-landing.ts",
        "manifest_path": "apps/openagents.com/apps/web/package.json",
        "name": "capture:landing"
      },
      {
        "command": "vite",
        "manifest_path": "apps/openagents.com/apps/web/package.json",
        "name": "dev"
      },
      {
        "command": "vite preview",
        "manifest_path": "apps/openagents.com/apps/web/package.json",
        "name": "preview"
      },
      {
        "command": "vitest run",
        "manifest_path": "apps/openagents.com/apps/web/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/apps/web/package.json",
        "name": "typecheck"
      },
      {
        "command": "node scripts/artanis-production-readiness.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "artanis:readiness"
      },
      {
        "command": "bun run build:web && bun run build:api",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "build"
      },
      {
        "command": "cd workers/api && bun run build",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "build:api"
      },
      {
        "command": "node scripts/sync-live-agent-doc.mjs && cd apps/web && bun run build",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "build:web"
      },
      {
        "command": "bun scripts/check-live-agent-doc-links.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:agent-doc-links"
      },
      {
        "command": "bun scripts/check-zero-debt-architecture.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:architecture"
      },
      {
        "command": "bun scripts/check-conflict-markers.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:conflict-markers"
      },
      {
        "command": "bun scripts/check-contract-drift.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:contract-drift"
      },
      {
        "command": "bun run check:conflict-markers && bun run check:no-github-actions && bun run test:cloudrun-env-origins && bun run check:effect-topology && bun run check:agent-doc-links && bun run check:architecture && bun run check:contract-drift && bun run check:public-projection-freshness && bun run --cwd ../../packages/agent-readiness test && bun run --cwd ../../packages/agent-readiness typecheck && bun run --cwd ../../packages/autopilot-control-protocol typecheck && bun run --cwd ../../apps/pylon typecheck && bun run typecheck:api-pylon-integration && bun run --cwd ../../apps/pylon test -- tests/security-adversarial-harness.test.ts && bun run --cwd ../../packages/khala-sync-server test:pending-migrations-guard && bun run --cwd ../../packages/khala-sync-client test && bun run typecheck:web && bun run typecheck:api && bun run test:conflict-markers-guard && bun run test:effect-native-vendor-guard && bun run test:command-composer-privacy-guard && bun run test:contract-drift-guard && bun run test:pending-migrations-guard && bun run test:predeploy-parallel-dispatch-smoke && bun run test:predeploy-khala-sync-live-seam-smoke && vitest run scripts/site-speed-landing.test.ts && bun run --cwd apps/web test -- src/route.test.ts src/route-coverage.test.ts src/navigation-policy.test.ts src/icon.test.ts src/icon-policy.test.ts src/routing/startup.test.ts src/client-server-route-agreement.test.ts src/subscriptions.test.ts src/main.test.ts src/update.test.ts src/page/demo/update.test.ts src/page/demo/playback.test.ts src/page/loggedOut/gym/terminalBenchReplay.test.ts src/page/loggedOut/page/gym.test.ts src/page/loggedOut/page/login.scene.test.ts src/page/loggedOut/page/onboarding.story.test.ts src/page/loggedIn/view.scene.test.ts src/scene/gymOssSceneElement.test.ts src/page/loggedIn/gymOss/gymOss.test.ts src/page/loggedIn/gymOss/stream.test.ts src/page/loggedOut/khala-tokens-served-countup.test.ts src/page/loggedOut/khala-tokens-served-countup-controller.test.ts && bun run --cwd workers/api test -- src/lander-css-policy.test.ts src/worker-routes.test.ts src/redirect-policy.test.ts src/client-server-route-agreement.test.ts src/mullet/routes.test.ts src/product-promises.test.ts src/model-custody-lead-gen.test.ts src/reactor-need-to-know-access.test.ts src/reactor-data-liberation.test.ts src/reactor-improvement-ladder.test.ts src/wasm-plugin-marketplace.test.ts src/qualified-contributor-methodology.test.ts src/public-forum-activity-routes.test.ts src/inference/inference-privacy-receipt-routes.test.ts src/inference/gym/terminal-bench-khala-orchestration.test.ts src/tassadar-settled-feed-sync.test.ts src/khala-sync-public-settled-feed.test.ts src/public-settled-feed-routes.test.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:deploy"
      },
      {
        "command": "bun run scripts/check-deploy-from-main.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:deploy-from-main"
      },
      {
        "command": "bun scripts/check-effect-native-vendor-freshness.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:effect-native-vendor"
      },
      {
        "command": "bun scripts/check-effect-topology.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:effect-topology"
      },
      {
        "command": "bun scripts/check-effect-upgrade-metadata.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:effect-upgrade-metadata"
      },
      {
        "command": "bun run scripts/check-no-github-actions.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:no-github-actions"
      },
      {
        "command": "bun run scripts/check-pending-migrations.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:pending-migrations"
      },
      {
        "command": "bun scripts/check-public-projection-freshness.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:public-projection-freshness"
      },
      {
        "command": "bun scripts/d1-zero-reference-sweep.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "d1:zero-reference-sweep"
      },
      {
        "command": "bun run dev:web",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "dev"
      },
      {
        "command": "cd workers/api && bun run dev",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "dev:api"
      },
      {
        "command": "cd apps/web && bun run dev",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "dev:web"
      },
      {
        "command": "prettier -w .",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "format"
      },
      {
        "command": "node scripts/forum.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "forum"
      },
      {
        "command": "bun scripts/gym-harbor-full-trace-archive.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "gym:harbor-full-trace-archive"
      },
      {
        "command": "bun scripts/gym-harbor-progress-push.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "gym:harbor-progress-push"
      },
      {
        "command": "bun scripts/khala-code-headless-harness.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "khala-code:verify"
      },
      {
        "command": "eslint .",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "lint"
      },
      {
        "command": "node scripts/khala-production-readiness-monitor.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "monitor:khala:production-readiness"
      },
      {
        "command": "node scripts/predeploy-khala-sync-live-seam-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "predeploy:khala-sync-live-seam-smoke"
      },
      {
        "command": "node scripts/predeploy-parallel-dispatch-smoke.mjs --approve-staging-mutation",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "predeploy:parallel-dispatch-smoke"
      },
      {
        "command": "node scripts/patch-effect-language-service.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "prepare"
      },
      {
        "command": "cd apps/web && bun run preview",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "preview"
      },
      {
        "command": "node scripts/private-workspace-setup-check.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "private-workspace:setup-check"
      },
      {
        "command": "node scripts/public-activity-proof-links-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:activity:proof-links"
      },
      {
        "command": "node scripts/mdk-forum-readiness-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:forum:mdk-readiness"
      },
      {
        "command": "node scripts/forum-tip-payout-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:forum:tip-payout"
      },
      {
        "command": "node scripts/forum-tip-wallet-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:forum:tip-wallet"
      },
      {
        "command": "node scripts/forum-void-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:forum:void"
      },
      {
        "command": "node scripts/gpt-oss20b-production-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:gpt-oss20b-production"
      },
      {
        "command": "node scripts/khala-gateway-readiness-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:khala:gateway-readiness"
      },
      {
        "command": "node scripts/khala-glm-reap-production-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:khala:glm-reap"
      },
      {
        "command": "node scripts/khala-production-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:khala:production"
      },
      {
        "command": "node scripts/pylon-install-to-bitcoin-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:pylon:install-to-bitcoin"
      },
      {
        "command": "node scripts/visibility-browser-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:visibility:browser"
      },
      {
        "command": "bun scripts/sync-fireball-icons.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "sync:icons"
      },
      {
        "command": "bun run test:packages && bun run test:web && bun run test:api",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test"
      },
      {
        "command": "cd workers/api && bun run test",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:api"
      },
      {
        "command": "bun test workers/api/scripts/cloudrun/render-env-yaml.test.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:cloudrun-env-origins"
      },
      {
        "command": "vitest run scripts/check-command-composer-privacy-fixtures.test.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:command-composer-privacy-guard"
      },
      {
        "command": "vitest run scripts/check-conflict-markers.test.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:conflict-markers-guard"
      },
      {
        "command": "vitest run scripts/check-contract-drift.test.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:contract-drift-guard"
      },
      {
        "command": "vitest run scripts/check-effect-native-vendor.test.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:effect-native-vendor-guard"
      },
      {
        "command": "cd packages/email-templates && bun run test && cd ../mullet-schema && bun run test && cd ../mullet-sim && bun run test && cd ../sync-schema && bun run test && cd ../sync-client && bun run test && cd ../sync-worker && bun run test",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:packages"
      },
      {
        "command": "vitest run scripts/check-pending-migrations.test.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:pending-migrations-guard"
      },
      {
        "command": "vitest run scripts/predeploy-khala-sync-live-seam-smoke.test.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:predeploy-khala-sync-live-seam-smoke"
      },
      {
        "command": "vitest run scripts/predeploy-parallel-dispatch-smoke.test.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:predeploy-parallel-dispatch-smoke"
      },
      {
        "command": "cd apps/web && bun run test",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:web"
      },
      {
        "command": "bun run typecheck:packages && bun run typecheck:web && bun run typecheck:api",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck"
      },
      {
        "command": "cd workers/api && bun run typecheck",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:api"
      },
      {
        "command": "tsc -p workers/api/tsconfig.pylon-api-routes.test.json --noEmit",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:api-pylon-integration"
      },
      {
        "command": "cd packages/email-templates && bun run typecheck && cd ../mullet-schema && bun run typecheck && cd ../mullet-sim && bun run typecheck && cd ../sync-schema && bun run typecheck && cd ../sync-client && bun run typecheck && cd ../sync-worker && bun run typecheck",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:packages"
      },
      {
        "command": "cd apps/web && bun run typecheck",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:web"
      },
      {
        "command": "tsc -b",
        "manifest_path": "apps/openagents.com/packages/effect-native-core/package.json",
        "name": "typecheck"
      },
      {
        "command": "tsc -b",
        "manifest_path": "apps/openagents.com/packages/effect-native-render-dom/package.json",
        "name": "typecheck"
      },
      {
        "command": "tsc -b",
        "manifest_path": "apps/openagents.com/packages/effect-native-render-rn/package.json",
        "name": "typecheck"
      },
      {
        "command": "tsc -b",
        "manifest_path": "apps/openagents.com/packages/effect-native-tokens/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun run src/preview.ts",
        "manifest_path": "apps/openagents.com/packages/email-templates/package.json",
        "name": "preview"
      },
      {
        "command": "vitest run",
        "manifest_path": "apps/openagents.com/packages/email-templates/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/packages/email-templates/package.json",
        "name": "typecheck"
      },
      {
        "command": "vitest run",
        "manifest_path": "apps/openagents.com/packages/mullet-schema/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/packages/mullet-schema/package.json",
        "name": "typecheck"
      },
      {
        "command": "vitest run",
        "manifest_path": "apps/openagents.com/packages/mullet-sim/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/packages/mullet-sim/package.json",
        "name": "typecheck"
      },
      {
        "command": "vitest run",
        "manifest_path": "apps/openagents.com/packages/sync-client/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/packages/sync-client/package.json",
        "name": "typecheck"
      },
      {
        "command": "vitest run",
        "manifest_path": "apps/openagents.com/packages/sync-schema/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/packages/sync-schema/package.json",
        "name": "typecheck"
      },
      {
        "command": "vitest run",
        "manifest_path": "apps/openagents.com/packages/sync-worker/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/packages/sync-worker/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun scripts/smoke.mjs",
        "manifest_path": "apps/openagents.com/services/mdk-sidecar/package.json",
        "name": "smoke"
      },
      {
        "command": "bun src/server.mjs",
        "manifest_path": "apps/openagents.com/services/mdk-sidecar/package.json",
        "name": "start"
      },
      {
        "command": "bun scripts/smoke.mjs",
        "manifest_path": "apps/openagents.com/services/mdk-tips-buffer/package.json",
        "name": "smoke"
      },
      {
        "command": "bun src/server.mjs",
        "manifest_path": "apps/openagents.com/services/mdk-tips-buffer/package.json",
        "name": "start"
      },
      {
        "command": "bun scripts/smoke.mjs",
        "manifest_path": "apps/openagents.com/services/mdk-treasury/package.json",
        "name": "smoke"
      },
      {
        "command": "bun src/server.mjs",
        "manifest_path": "apps/openagents.com/services/mdk-treasury/package.json",
        "name": "start"
      },
      {
        "command": "bun run scripts/agent-readiness-fleet-report-run.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "agent-readiness:fleet-run"
      },
      {
        "command": "node --experimental-strip-types scripts/apollo-wave-runner.ts dry-run",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "apollo-wave:dry-run"
      },
      {
        "command": "node --experimental-strip-types scripts/apollo-wave-runner.ts live",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "apollo-wave:live"
      },
      {
        "command": "node --experimental-strip-types scripts/apollo-wave-runner.ts print-fixture",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "apollo-wave:print-fixture"
      },
      {
        "command": "wrangler deploy --dry-run --containers-rollout=none --assets ../../apps/web/dist",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "build"
      },
      {
        "command": "bun run deploy:safe",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "deploy"
      },
      {
        "command": "cd ../.. && bun run check:deploy-from-main && bun run check:deploy && cd workers/api && wrangler d1 migrations apply openagents-autopilot-staging --env staging --remote && cd ../.. && bun run build:web && cd workers/api && wrangler deploy --env staging --containers-rollout=none --assets ../../apps/web/dist && cd ../.. && bun run predeploy:parallel-dispatch-smoke && bun run predeploy:khala-sync-live-seam-smoke && cd workers/api && wrangler d1 migrations apply openagents-autopilot --remote && cd ../.. && bun run check:pending-migrations && bun run --cwd ../../packages/khala-sync-server check:pending-migrations && cd workers/api && wrangler deploy --containers-rollout=none --assets ../../apps/web/dist",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "deploy:safe"
      },
      {
        "command": "cd ../.. && bun run check:deploy && cd workers/api && wrangler d1 migrations apply openagents-autopilot-staging --env staging --remote && cd ../.. && bun run build:web && cd workers/api && wrangler deploy --env staging --containers-rollout=none --assets ../../apps/web/dist",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "deploy:staging"
      },
      {
        "command": "wrangler dev",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "dev"
      },
      {
        "command": "bun run scripts/khala-glm-fleet-durability.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "glm-fleet:durability"
      },
      {
        "command": "bun run scripts/marching-orders-agent.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "marching-orders"
      },
      {
        "command": "wrangler d1 migrations apply openagents-autopilot-staging --env staging --remote",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "migrate:staging"
      },
      {
        "command": "bun run scripts/khala-glm-nvfp4-pilot.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "pilot:glm-nvfp4"
      },
      {
        "command": "vitest run src/autopilot-work-routes.test.ts -t \"claude_agent_task git_checkout\"",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:autopilot-coder:claude-agent-git-checkout"
      },
      {
        "command": "vitest run src/autopilot-work-routes.test.ts -t \"no-spend Autopilot Coder end-to-end smoke\"",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:autopilot-coder:no-spend"
      },
      {
        "command": "vitest run src/autopilot-work-routes.test.ts -t \"paid Autopilot Coder end-to-end smoke\"",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:autopilot-coder:paid"
      },
      {
        "command": "vitest run src/autopilot-rate-limit-rotation-smoke.test.ts src/provider-account-lease-policy.test.ts src/provider-account-failover-policy.test.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:autopilot-coder:rate-limit-rotation"
      },
      {
        "command": "vitest run src/cs336-a1-homework.test.ts src/cs336-a1-homework-workload.test.ts src/cs336-a1-real-gradient-workload.test.ts src/training-real-gradient-evidence.test.ts src/training-verification.test.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:cs336-a1:no-spend"
      },
      {
        "command": "vitest run src/training-device-capability.test.ts src/training-device-admission-gates.test.ts src/training-run-window-routes.test.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:cs336-a2:device-capability"
      },
      {
        "command": "vitest run src/training-scaling-sweep.test.ts src/training-run-window-routes.test.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:cs336-a3:isoflop"
      },
      {
        "command": "vitest run src/cs336-a4-data-refinery.test.ts src/cs336-a4-refinery-workload.test.ts src/training-data-refinery.test.ts src/training-run-window-routes.test.ts src/training-leaderboards.test.ts src/training-verification.test.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:cs336-a4:data-refinery"
      },
      {
        "command": "vitest run src/cs336-a5-alignment-homework.test.ts src/cs336-a5-rollout-workload.test.ts src/training-alignment-evals.test.ts src/training-run-window-routes.test.ts src/training-leaderboards.test.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:cs336-a5:alignment"
      },
      {
        "command": "bun scripts/probe-gepa-stage0-no-spend-campaign.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:probe-gepa-stage0"
      },
      {
        "command": "bun scripts/qwen-remote-pylon-live-training.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:qwen-remote-training"
      },
      {
        "command": "vitest run src/tassadar-executor-trace-homework.test.ts src/training-verification.test.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:tassadar:executor-trace"
      },
      {
        "command": "vitest run src/training-leaderboards.test.ts src/training-run-window-routes.test.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:training-leaderboards"
      },
      {
        "command": "vitest run src/training-run-window-routes.test.ts src/training-run-window-authority.test.ts src/training-run-public-copy-gate.test.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:training-runs:public"
      },
      {
        "command": "vitest run src/training-validator-assignments.test.ts src/training-verification.test.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:training-validator:no-spend"
      },
      {
        "command": "bun --preload ./src/cloudrun/preload.ts ./src/cloudrun/server.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "start:cloudrun"
      },
      {
        "command": "vitest run",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "typecheck"
      },
      {
        "command": "tsc -p tsconfig.cloudrun.json --noEmit",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "typecheck:cloudrun"
      },
      {
        "command": "bash swift/foundation-bridge/build.sh",
        "manifest_path": "apps/pylon/package.json",
        "name": "build:apple-fm-bridge"
      },
      {
        "command": "bash scripts/build-rc-binaries.sh",
        "manifest_path": "apps/pylon/package.json",
        "name": "build:rc-binaries"
      },
      {
        "command": "bun scripts/check-supervisor-store-bypass.mjs",
        "manifest_path": "apps/pylon/package.json",
        "name": "check:supervisor-store"
      },
      {
        "command": "bun scripts/nip90-provider-serve.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "provider:serve"
      },
      {
        "command": "bash scripts/release-gate.sh",
        "manifest_path": "apps/pylon/package.json",
        "name": "release:gate"
      },
      {
        "command": "bun --cwd packages/runtime src/cli.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "runtime"
      },
      {
        "command": "bun run --cwd packages/runtime test",
        "manifest_path": "apps/pylon/package.json",
        "name": "runtime:test"
      },
      {
        "command": "bun scripts/claude-agent-task-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:claude-agent-task"
      },
      {
        "command": "bun scripts/claude-owner-local-permission-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:claude-owner-local-permission"
      },
      {
        "command": "bun scripts/codex-agent-task-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:codex-agent-task"
      },
      {
        "command": "rm -f /tmp/pylon-default-start.log; perl -e 'alarm 3; $ENV{PYLON_DISABLE_OPENCODE_STARTUP}=1; exec @ARGV' bun src/index.ts > /tmp/pylon-default-start.log 2>&1; code=$?; if [ \"$code\" -ne 142 ] && [ \"$code\" -ne 0 ]; then cat /tmp/pylon-default-start.log; exit \"$code\"; fi; if rg -n 'TypeError|Effect\\.(fork|catchAll)|is not a function|\\[ERROR\\]' /tmp/pylon-default-start.log; then exit 1; fi; printf 'default startup reached persistent mode without startup API errors\\n'",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:default-start"
      },
      {
        "command": "bun scripts/fleet-run-live-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:fleet-run-live"
      },
      {
        "command": "bun scripts/fleet-run-sustained-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:fleet-run-sustained"
      },
      {
        "command": "bash scripts/smoke-local-package-install.sh",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:install:local"
      },
      {
        "command": "bun scripts/live-worker-loop-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:live-worker-loop"
      },
      {
        "command": "bun scripts/mixed-harness-fleet-run-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:mixed-harness-fleet-run"
      },
      {
        "command": "bun scripts/nip90-provider-loop-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:nip90-provider"
      },
      {
        "command": "bun scripts/packaged-live-network-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:packaged-network"
      },
      {
        "command": "bun scripts/packaged-runtime-task-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:packaged-runtime-task"
      },
      {
        "command": "bun scripts/stranger-probe-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:stranger-probe"
      },
      {
        "command": "npx --yes @moneydevkit/agent-wallet@latest balance | bun -e 'const input = await new Response(Bun.stdin.stream()).text(); const json = JSON.parse(input); if (typeof json.balance_sats !== \"number\") { console.error(input.trim()); process.exit(1); } console.log(`wallet balance: ${json.balance_sats} sats`);'",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:wallet"
      },
      {
        "command": "bun src/index.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "start"
      },
      {
        "command": "bun run check:supervisor-store && bun test --max-concurrency=1",
        "manifest_path": "apps/pylon/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/pylon/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun src/cli.ts",
        "manifest_path": "apps/pylon/packages/runtime/package.json",
        "name": "runtime"
      },
      {
        "command": "bun test",
        "manifest_path": "apps/pylon/packages/runtime/package.json",
        "name": "test"
      },
      {
        "command": "bun run src/daemon.ts --api",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "api"
      },
      {
        "command": "bun run src/atif-emit.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "atif:emit"
      },
      {
        "command": "bun run scripts/build.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "build"
      },
      {
        "command": "bun run src/codex-to-atif.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "codex:to-atif"
      },
      {
        "command": "bun run src/compose/cli.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "compose"
      },
      {
        "command": "bun run src/byo.ts run --fake-model --url https://example.test --out ./runs/byo-fake",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "demo:byo"
      },
      {
        "command": "bun run src/demo-khala.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "demo:khala"
      },
      {
        "command": "bun run src/demo-login.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "demo:login"
      },
      {
        "command": "bun run src/evals-run.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "evals"
      },
      {
        "command": "bun run src/khala-sync-once.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "khala-sync-once"
      },
      {
        "command": "bun run src/khala-flagship-demo.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "khala:flagship-demo"
      },
      {
        "command": "bun run src/khala-packaged-native-smoke.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "khala:packaged-native-smoke"
      },
      {
        "command": "playwright install --with-deps chromium",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "playwright:install"
      },
      {
        "command": "bun run src/pr-comment-run.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "pr-comment"
      },
      {
        "command": "bun run scripts/build.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "prepack"
      },
      {
        "command": "bun run src/byo.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "qa"
      },
      {
        "command": "node dist/qa.js",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "qa:dist"
      },
      {
        "command": "bun run src/run-once.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "run-once"
      },
      {
        "command": "bun run src/run-targets.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "run-targets"
      },
      {
        "command": "bun run src/daemon.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "serve"
      },
      {
        "command": "bun run src/terminal-once.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "terminal-once"
      },
      {
        "command": "bun test src/byo-model.test.ts src/byo.test.ts src/runner.test.ts src/runner-hardening.test.ts src/timeouts.test.ts src/shard.test.ts src/public-safety.test.ts src/brain.test.ts src/backend.test.ts src/terminal-backend.test.ts src/khala-sync-transport-backend.test.ts src/mobile-nightly.test.ts src/container-backend.test.ts src/native-desktop-backend.test.ts src/khala-desktop-backend.test.ts src/khala-action.test.ts src/khala-driver.test.ts src/khala-config.test.ts src/khala-openrouter.test.ts src/session-trace.test.ts src/distiller.test.ts src/skill-candidate.test.ts src/receipt.test.ts src/run-settlement.test.ts src/khala-session.test.ts src/compose/build-plan.test.ts src/compose/ffmpeg.test.ts src/evals.test.ts src/pr-comment.test.ts src/control-auth.test.ts src/artifacts.test.ts src/control.test.ts src/api-server.test.ts src/failure-learning.test.ts src/failure-learning-gepa.test.ts src/target-registry.test.ts src/target-registry-run.test.ts src/target-adapter.test.ts src/qs7-rhys-sales-motion.test.ts src/cf-browser-backend.test.ts src/cf-browser-video.test.ts src/cf-sandbox-backend.test.ts src/atif.test.ts src/atif-html.test.ts src/codex-to-atif.test.ts src/redaction.test.ts src/claude-code-to-atif.test.ts src/publish-trace.test.ts src/trace-fixture.test.ts src/publish-trace-e2e.verify.test.ts generated/khala-code-packaged-seeded-bug.e2e.test.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "test"
      },
      {
        "command": "bun run src/trace-fixture.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "trace:fixture"
      },
      {
        "command": "tsc --noEmit -p tsconfig.json",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun build src/index.ts --target=node --format=esm --outdir=dist",
        "manifest_path": "clients/khala-cli/package.json",
        "name": "build"
      },
      {
        "command": "bun src/index.ts",
        "manifest_path": "clients/khala-cli/package.json",
        "name": "khala"
      },
      {
        "command": "bun run build",
        "manifest_path": "clients/khala-cli/package.json",
        "name": "prepack"
      },
      {
        "command": "bun test src",
        "manifest_path": "clients/khala-cli/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "clients/khala-cli/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun scripts/backfill-codex-message-token-audit.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "backfill:message-token-audit"
      },
      {
        "command": "bun scripts/thread-switch-benchmark.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "bench:thread-switch"
      },
      {
        "command": "bun run build:ui && electrobun build",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "build"
      },
      {
        "command": "bun run build:ui && electrobun build --env=rc",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "build:rc"
      },
      {
        "command": "bun run build:ui && electrobun build --env=stable",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "build:stable"
      },
      {
        "command": "vite build",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "build:ui"
      },
      {
        "command": "bun run build:ui && electrobun dev",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "dev"
      },
      {
        "command": "concurrently \"vite --port 5173 --strictPort\" \"bun run dev\"",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "dev:hmr"
      },
      {
        "command": "bun scripts/retired-release-guard.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "release:macos"
      },
      {
        "command": "bun scripts/retired-release-guard.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "release:plan"
      },
      {
        "command": "bun scripts/architecture-scan.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "scan:architecture"
      },
      {
        "command": "bun scripts/claude-live-smoke.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "smoke:claude-live"
      },
      {
        "command": "bun scripts/cockpit-visual-smoke.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "smoke:cockpit-visual"
      },
      {
        "command": "bun scripts/codex-parity-live-smoke.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "smoke:codex-parity-live"
      },
      {
        "command": "bun scripts/live-two-codex-readonly-smoke.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "smoke:codex-spawn-live"
      },
      {
        "command": "bun scripts/composer-visual-smoke.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "smoke:composer-visual"
      },
      {
        "command": "bun scripts/composer-visual-smoke.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "smoke:composer-visual-preview"
      },
      {
        "command": "bun scripts/diagnostics-recovery-visual-smoke.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "smoke:diagnostics-recovery-visual"
      },
      {
        "command": "bun scripts/editor-visual-smoke.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "smoke:editor-visual"
      },
      {
        "command": "bun scripts/part2-fleet-gym-visual-smoke.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "smoke:part2-fleet-gym-visual"
      },
      {
        "command": "bun scripts/part2-ui-recording-smoke.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "smoke:part2-ui"
      },
      {
        "command": "bun test tests/*.test.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.typecheck.json --noEmit",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun run scan:architecture && bun run typecheck && bun test tests/*.test.ts && bun run build:ui && bun build src/bun/index.ts --outdir /tmp/khala-code-desktop-bun-build --target bun",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "verify"
      },
      {
        "command": "bun scripts/verify-packaged-apple-fm-bridge.ts",
        "manifest_path": "clients/khala-code-desktop/package.json",
        "name": "verify:apple-fm-bridge"
      },
      {
        "command": "depcruise index.tsx src tests --config .dependency-cruiser.cjs",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "architecture:check"
      },
      {
        "command": "bash scripts/generate-assets.sh",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "assets:generate"
      },
      {
        "command": "./android/gradlew -p android :app:assembleDebug",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "build:android:local"
      },
      {
        "command": "xcodebuild -workspace ios/KhalaCode.xcworkspace -scheme KhalaCode -configuration Debug -destination 'generic/platform=iOS Simulator' build",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "build:ios:local"
      },
      {
        "command": "expo start",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "dev"
      },
      {
        "command": "expo export --platform android --output-dir ../../apps/oa-updates/dist",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "export:ota:android"
      },
      {
        "command": "expo export --platform ios --output-dir ../../apps/oa-updates/dist",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "export:ota:ios"
      },
      {
        "command": "bun run scripts/generate.ts",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "generate"
      },
      {
        "command": "expo prebuild",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "prebuild"
      },
      {
        "command": "expo prebuild --platform android",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "prebuild:android"
      },
      {
        "command": "expo prebuild --platform ios",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "prebuild:ios"
      },
      {
        "command": "bash ../../apps/oa-updates/scripts/publish-ota.sh",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "publish:ota"
      },
      {
        "command": "bash scripts/android-emulator-test-run.sh",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "qa:android:emulator"
      },
      {
        "command": "bun run scripts/qa-mobile-gate.ts",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "qa:mobile:gate"
      },
      {
        "command": "STORYBOOK_ENABLED=true EXPO_PUBLIC_STORYBOOK_ENABLED=true expo start",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "storybook"
      },
      {
        "command": "sb-rn-get-stories",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "storybook-generate"
      },
      {
        "command": "STORYBOOK_ENABLED=true EXPO_PUBLIC_STORYBOOK_ENABLED=true expo start --android",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "storybook:android"
      },
      {
        "command": "STORYBOOK_ENABLED=true EXPO_PUBLIC_STORYBOOK_ENABLED=true expo start --ios",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "storybook:ios"
      },
      {
        "command": "bun test",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "clients/khala-mobile/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "docs/khala/fixtures/artanis-as-a-service-smoke-repo/package.json",
        "name": "test"
      },
      {
        "command": "bun run check:sol-docs && bun run test:sol-docs && bun run --cwd apps/openagents.com check:deploy",
        "manifest_path": "package.json",
        "name": "check:deploy"
      },
      {
        "command": "bun scripts/generate-sol-doc-manifest.ts --check",
        "manifest_path": "package.json",
        "name": "check:sol-doc-manifest"
      },
      {
        "command": "bun scripts/check-sol-docs.ts",
        "manifest_path": "package.json",
        "name": "check:sol-docs"
      },
      {
        "command": "bun run --cwd apps/aiur deploy",
        "manifest_path": "package.json",
        "name": "deploy:aiur"
      },
      {
        "command": "bun run --cwd apps/forge deploy",
        "manifest_path": "package.json",
        "name": "deploy:forge"
      },
      {
        "command": "bun run --cwd apps/nostr-relay deploy",
        "manifest_path": "package.json",
        "name": "deploy:nostr-relay"
      },
      {
        "command": "bun run --cwd apps/openagents-world deploy",
        "manifest_path": "package.json",
        "name": "deploy:openagents-world"
      },
      {
        "command": "bun run --cwd apps/aiur dev",
        "manifest_path": "package.json",
        "name": "dev:aiur"
      },
      {
        "command": "bun run --cwd apps/forge dev",
        "manifest_path": "package.json",
        "name": "dev:forge"
      },
      {
        "command": "bun run --cwd apps/forum dev",
        "manifest_path": "package.json",
        "name": "dev:forum"
      },
      {
        "command": "bun run --cwd clients/khala-code-desktop dev",
        "manifest_path": "package.json",
        "name": "dev:khala-code-desktop"
      },
      {
        "command": "bun run --cwd clients/khala-mobile dev",
        "manifest_path": "package.json",
        "name": "dev:khala-mobile"
      },
      {
        "command": "bun run --cwd apps/nostr-relay dev",
        "manifest_path": "package.json",
        "name": "dev:nostr-relay"
      },
      {
        "command": "bun run --cwd apps/openagents-desktop dev",
        "manifest_path": "package.json",
        "name": "dev:openagents-desktop"
      },
      {
        "command": "bun run --cwd apps/openagents-mobile dev",
        "manifest_path": "package.json",
        "name": "dev:openagents-mobile"
      },
      {
        "command": "bun run --cwd apps/openagents-world dev",
        "manifest_path": "package.json",
        "name": "dev:openagents-world"
      },
      {
        "command": "bun run --cwd apps/openagents.com dev",
        "manifest_path": "package.json",
        "name": "dev:openagents.com"
      },
      {
        "command": "bun run --cwd apps/pylon start",
        "manifest_path": "package.json",
        "name": "dev:pylon"
      },
      {
        "command": "bun scripts/generate-sol-doc-manifest.ts",
        "manifest_path": "package.json",
        "name": "generate:sol-doc-manifest"
      },
      {
        "command": "bun run --cwd clients/khala-cli khala",
        "manifest_path": "package.json",
        "name": "khala"
      },
      {
        "command": "bun run scripts/ui-velocity-receipt.ts",
        "manifest_path": "package.json",
        "name": "perf:ui-velocity"
      },
      {
        "command": "effect-language-service patch",
        "manifest_path": "package.json",
        "name": "prepare"
      },
      {
        "command": "bun scripts/qa-nightly-matrix.ts",
        "manifest_path": "package.json",
        "name": "qa:nightly"
      },
      {
        "command": "bun run scripts/effect-authority-boundary-scan.ts",
        "manifest_path": "package.json",
        "name": "scan:effect-authority-boundaries"
      },
      {
        "command": "bun run --cwd apps/nostr-relay smoke",
        "manifest_path": "package.json",
        "name": "smoke:nostr-relay"
      },
      {
        "command": "bun run test:sol-docs && bun run test:qa-pre-push-smoke && bun run test:qa-async-gce-trigger && bun run test:qa-nightly-matrix && bun run test:qa-visual-smoke-gate && bun run test:github-issue-triage && bun run test:khala-sync-runtime-dogfood-evidence && bun run test:ui-velocity-receipt && bun run test:forge && bun run test:forum && bun run test:pylon && bun run test:pylon-core && bun run test:probe && bun run test:qa-runner && bun run test:khala-cli && bun run test:khala-code-desktop && bun run test:khala-mobile && bun run test:openagents-mobile && bun run test:khala-qa-harness && bun run test:khala-ai-sdk-core && bun run test:ai-sdk-sandbox-local && bun run test:ai-sdk-sandbox-openagents && bun run test:behavior-contracts && bun run test:assurance-spec && bun run test:agent-readiness && bun run test:nip90 && bun run test:arbiter-effect && bun run test:public-activity-timeline && bun run test:input-bindings && bun run test:design-tokens && bun run test:ui && bun run test:composer-state && bun run test:agent-runtime-schema && bun run test:khala-fleet-intents && bun run test:grok-harness && bun run test:harness-conformance && bun run test:reactor-contracts && bun run test:provider-account-schema && bun run test:effect-boundary && bun run test:effect-start && bun run test:khala-sync-db-collection && bun run test:blueprint-contracts && bun run test:connector-sidecar && bun run test:khala-tools && bun run test:mcp-contract && bun run test:forge-protocol && bun run test:world-contract && bun run test:world-client && bun run test:openagents-world && bun run test:autopilot-ui && bun run test:oa-updates && bun run test:khala-capture && bun run test:khala-live-hub && bun run test:nostr-relay && bun run test:durable-stream && bun run test:sarah-take-scoreboard && bun run test:openagents-desktop && bun run test:cloud-contract && bun run test:oa-infra && bun run test:oa-queue-worker && bun run test:aiur",
        "manifest_path": "package.json",
        "name": "test"
      },
      {
        "command": "bun run --cwd packages/agent-readiness test",
        "manifest_path": "package.json",
        "name": "test:agent-readiness"
      },
      {
        "command": "bun run --cwd packages/agent-runtime-schema test",
        "manifest_path": "package.json",
        "name": "test:agent-runtime-schema"
      },
      {
        "command": "cd packages/ai-sdk-sandbox-local && bun run test",
        "manifest_path": "package.json",
        "name": "test:ai-sdk-sandbox-local"
      },
      {
        "command": "cd packages/ai-sdk-sandbox-openagents && bun run test",
        "manifest_path": "package.json",
        "name": "test:ai-sdk-sandbox-openagents"
      },
      {
        "command": "bun run --cwd apps/aiur test",
        "manifest_path": "package.json",
        "name": "test:aiur"
      },
      {
        "command": "bun run --cwd packages/arbiter-effect test",
        "manifest_path": "package.json",
        "name": "test:arbiter-effect"
      },
      {
        "command": "bun run --cwd packages/assurance-spec test",
        "manifest_path": "package.json",
        "name": "test:assurance-spec"
      },
      {
        "command": "bun run --cwd packages/audio-contract test",
        "manifest_path": "package.json",
        "name": "test:audio-contract"
      },
      {
        "command": "bun run --cwd packages/autopilot-ui test",
        "manifest_path": "package.json",
        "name": "test:autopilot-ui"
      },
      {
        "command": "bun run --cwd packages/behavior-contracts test",
        "manifest_path": "package.json",
        "name": "test:behavior-contracts"
      },
      {
        "command": "bun run --cwd packages/blueprint-contracts test",
        "manifest_path": "package.json",
        "name": "test:blueprint-contracts"
      },
      {
        "command": "bun run --cwd packages/cloud-contract test",
        "manifest_path": "package.json",
        "name": "test:cloud-contract"
      },
      {
        "command": "cargo test --workspace",
        "manifest_path": "package.json",
        "name": "test:cloud-crates"
      },
      {
        "command": "bun run --cwd packages/composer-state test",
        "manifest_path": "package.json",
        "name": "test:composer-state"
      },
      {
        "command": "bun run --cwd packages/connector-sidecar test",
        "manifest_path": "package.json",
        "name": "test:connector-sidecar"
      },
      {
        "command": "bun run --cwd packages/design-tokens test",
        "manifest_path": "package.json",
        "name": "test:design-tokens"
      },
      {
        "command": "bun run --cwd packages/durable-stream test",
        "manifest_path": "package.json",
        "name": "test:durable-stream"
      },
      {
        "command": "bun run --cwd packages/effect-boundary test",
        "manifest_path": "package.json",
        "name": "test:effect-boundary"
      },
      {
        "command": "bun run --cwd packages/effect-start test",
        "manifest_path": "package.json",
        "name": "test:effect-start"
      },
      {
        "command": "bun run --cwd apps/forge test",
        "manifest_path": "package.json",
        "name": "test:forge"
      },
      {
        "command": "bun run --cwd packages/forge-protocol test",
        "manifest_path": "package.json",
        "name": "test:forge-protocol"
      },
      {
        "command": "bun run --cwd apps/forum test",
        "manifest_path": "package.json",
        "name": "test:forum"
      },
      {
        "command": "bun test scripts/github-issue-triage.test.ts",
        "manifest_path": "package.json",
        "name": "test:github-issue-triage"
      },
      {
        "command": "bun run --cwd packages/grok-harness test",
        "manifest_path": "package.json",
        "name": "test:grok-harness"
      },
      {
        "command": "bun run --cwd packages/harness-conformance test",
        "manifest_path": "package.json",
        "name": "test:harness-conformance"
      },
      {
        "command": "bun run --cwd packages/input-bindings test",
        "manifest_path": "package.json",
        "name": "test:input-bindings"
      },
      {
        "command": "bun run --cwd packages/khala-ai-sdk-core test",
        "manifest_path": "package.json",
        "name": "test:khala-ai-sdk-core"
      },
      {
        "command": "bun run --cwd apps/khala-capture test",
        "manifest_path": "package.json",
        "name": "test:khala-capture"
      },
      {
        "command": "bun run --cwd clients/khala-cli test",
        "manifest_path": "package.json",
        "name": "test:khala-cli"
      },
      {
        "command": "bun run --cwd clients/khala-code-desktop test",
        "manifest_path": "package.json",
        "name": "test:khala-code-desktop"
      },
      {
        "command": "bun run --cwd packages/khala-fleet-intents test",
        "manifest_path": "package.json",
        "name": "test:khala-fleet-intents"
      },
      {
        "command": "bun run --cwd apps/khala-live-hub test",
        "manifest_path": "package.json",
        "name": "test:khala-live-hub"
      },
      {
        "command": "bun run --cwd clients/khala-mobile test",
        "manifest_path": "package.json",
        "name": "test:khala-mobile"
      },
      {
        "command": "bun run --cwd packages/khala-qa-harness test",
        "manifest_path": "package.json",
        "name": "test:khala-qa-harness"
      },
      {
        "command": "bun run --cwd packages/khala-sync-db-collection test",
        "manifest_path": "package.json",
        "name": "test:khala-sync-db-collection"
      },
      {
        "command": "bun test scripts/validate-khala-sync-runtime-dogfood-evidence.test.ts",
        "manifest_path": "package.json",
        "name": "test:khala-sync-runtime-dogfood-evidence"
      },
      {
        "command": "bun run --cwd packages/khala-tools test",
        "manifest_path": "package.json",
        "name": "test:khala-tools"
      },
      {
        "command": "bun run --cwd packages/mcp-contract test",
        "manifest_path": "package.json",
        "name": "test:mcp-contract"
      },
      {
        "command": "bun run --cwd packages/nip90 test",
        "manifest_path": "package.json",
        "name": "test:nip90"
      },
      {
        "command": "bun run --cwd apps/nostr-relay test",
        "manifest_path": "package.json",
        "name": "test:nostr-relay"
      },
      {
        "command": "bun run --cwd packages/oa-infra test",
        "manifest_path": "package.json",
        "name": "test:oa-infra"
      },
      {
        "command": "bun run --cwd apps/oa-queue-worker test",
        "manifest_path": "package.json",
        "name": "test:oa-queue-worker"
      },
      {
        "command": "bun run --cwd apps/oa-updates test",
        "manifest_path": "package.json",
        "name": "test:oa-updates"
      },
      {
        "command": "bun run --cwd apps/openagents-audio test",
        "manifest_path": "package.json",
        "name": "test:openagents-audio"
      },
      {
        "command": "bun run --cwd apps/openagents-desktop verify",
        "manifest_path": "package.json",
        "name": "test:openagents-desktop"
      },
      {
        "command": "bun run --cwd apps/openagents-mobile test",
        "manifest_path": "package.json",
        "name": "test:openagents-mobile"
      },
      {
        "command": "bun run --cwd apps/openagents-world test",
        "manifest_path": "package.json",
        "name": "test:openagents-world"
      },
      {
        "command": "bun run --cwd apps/openagents.com test",
        "manifest_path": "package.json",
        "name": "test:openagents.com"
      },
      {
        "command": "bun run --cwd packages/probe test",
        "manifest_path": "package.json",
        "name": "test:probe"
      },
      {
        "command": "bun run --cwd packages/provider-account-schema test",
        "manifest_path": "package.json",
        "name": "test:provider-account-schema"
      },
      {
        "command": "bun run --cwd packages/public-activity-timeline test",
        "manifest_path": "package.json",
        "name": "test:public-activity-timeline"
      },
      {
        "command": "bun run --cwd apps/pylon test",
        "manifest_path": "package.json",
        "name": "test:pylon"
      },
      {
        "command": "bun run --cwd packages/pylon-core test",
        "manifest_path": "package.json",
        "name": "test:pylon-core"
      },
      {
        "command": "bun test scripts/qa-async-gce-trigger.test.ts",
        "manifest_path": "package.json",
        "name": "test:qa-async-gce-trigger"
      },
      {
        "command": "bun test scripts/qa-nightly-matrix.test.ts",
        "manifest_path": "package.json",
        "name": "test:qa-nightly-matrix"
      },
      {
        "command": "bun test scripts/qa-pre-push-smoke.test.ts",
        "manifest_path": "package.json",
        "name": "test:qa-pre-push-smoke"
      },
      {
        "command": "bun run --cwd apps/qa-runner test",
        "manifest_path": "package.json",
        "name": "test:qa-runner"
      },
      {
        "command": "bun test scripts/qa-visual-smoke-gate.test.ts",
        "manifest_path": "package.json",
        "name": "test:qa-visual-smoke-gate"
      },
      {
        "command": "bun run --cwd packages/reactor-contracts test",
        "manifest_path": "package.json",
        "name": "test:reactor-contracts"
      },
      {
        "command": "bun run --cwd packages/sarah-take-scoreboard test",
        "manifest_path": "package.json",
        "name": "test:sarah-take-scoreboard"
      },
      {
        "command": "bun test scripts/check-sol-docs.test.ts",
        "manifest_path": "package.json",
        "name": "test:sol-docs"
      },
      {
        "command": "bun run --cwd packages/ui test",
        "manifest_path": "package.json",
        "name": "test:ui"
      },
      {
        "command": "bun test scripts/ui-velocity-receipt.test.ts",
        "manifest_path": "package.json",
        "name": "test:ui-velocity-receipt"
      },
      {
        "command": "bun run --cwd packages/world-client test",
        "manifest_path": "package.json",
        "name": "test:world-client"
      },
      {
        "command": "bun run --cwd packages/world-contract test",
        "manifest_path": "package.json",
        "name": "test:world-contract"
      },
      {
        "command": "bun run scripts/github-issue-triage.ts",
        "manifest_path": "package.json",
        "name": "triage:issues"
      },
      {
        "command": "bun run typecheck:forge && bun run typecheck:forum && bun run typecheck:nostr-relay && bun run typecheck:openagents-world && bun run typecheck:khala-cli && bun run typecheck:khala-code-desktop && bun run typecheck:khala-mobile && bun run typecheck:khala-qa-harness && bun run typecheck:khala-ai-sdk-core && bun run typecheck:ai-sdk-sandbox-local && bun run typecheck:ai-sdk-sandbox-openagents && bun run typecheck:behavior-contracts && bun run typecheck:assurance-spec && bun run typecheck:agent-readiness && bun run typecheck:nip90 && bun run typecheck:arbiter-effect && bun run typecheck:public-activity-timeline && bun run typecheck:input-bindings && bun run typecheck:design-tokens && bun run typecheck:composer-state && bun run typecheck:ui && bun run typecheck:agent-runtime-schema && bun run typecheck:khala-fleet-intents && bun run typecheck:grok-harness && bun run typecheck:harness-conformance && bun run typecheck:reactor-contracts && bun run typecheck:provider-account-schema && bun run typecheck:effect-boundary && bun run typecheck:effect-start && bun run typecheck:khala-sync-db-collection && bun run typecheck:blueprint-contracts && bun run typecheck:connector-sidecar && bun run typecheck:khala-tools && bun run typecheck:mcp-contract && bun run typecheck:forge-protocol && bun run typecheck:world-contract && bun run typecheck:world-client && bun run typecheck:autopilot-ui && bun run typecheck:durable-stream && bun run typecheck:oa-infra && bun run typecheck:oa-queue-worker",
        "manifest_path": "package.json",
        "name": "typecheck"
      },
      {
        "command": "bun run --cwd packages/agent-readiness typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:agent-readiness"
      },
      {
        "command": "bun run --cwd packages/agent-runtime-schema typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:agent-runtime-schema"
      },
      {
        "command": "cd packages/ai-sdk-sandbox-local && bun run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:ai-sdk-sandbox-local"
      },
      {
        "command": "cd packages/ai-sdk-sandbox-openagents && bun run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:ai-sdk-sandbox-openagents"
      },
      {
        "command": "bun run --cwd packages/arbiter-effect typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:arbiter-effect"
      },
      {
        "command": "bun run --cwd packages/assurance-spec typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:assurance-spec"
      },
      {
        "command": "bun run --cwd packages/audio-contract typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:audio-contract"
      },
      {
        "command": "bun run --cwd packages/autopilot-ui typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:autopilot-ui"
      },
      {
        "command": "bun run --cwd packages/behavior-contracts typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:behavior-contracts"
      },
      {
        "command": "bun run --cwd packages/blueprint-contracts typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:blueprint-contracts"
      },
      {
        "command": "bun run --cwd packages/composer-state typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:composer-state"
      },
      {
        "command": "bun run --cwd packages/connector-sidecar typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:connector-sidecar"
      },
      {
        "command": "bun run --cwd packages/design-tokens typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:design-tokens"
      },
      {
        "command": "bun run --cwd packages/durable-stream typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:durable-stream"
      },
      {
        "command": "bun run --cwd packages/effect-boundary typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:effect-boundary"
      },
      {
        "command": "bun run --cwd packages/effect-start typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:effect-start"
      },
      {
        "command": "bun run --cwd apps/forge typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:forge"
      },
      {
        "command": "bun run --cwd packages/forge-protocol typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:forge-protocol"
      },
      {
        "command": "bun run --cwd apps/forum typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:forum"
      },
      {
        "command": "bun run --cwd packages/grok-harness typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:grok-harness"
      },
      {
        "command": "bun run --cwd packages/harness-conformance typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:harness-conformance"
      },
      {
        "command": "bun run --cwd packages/input-bindings typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:input-bindings"
      },
      {
        "command": "bun run --cwd packages/khala-ai-sdk-core typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-ai-sdk-core"
      },
      {
        "command": "bun run --cwd clients/khala-cli typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-cli"
      },
      {
        "command": "bun run --cwd clients/khala-code-desktop typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-code-desktop"
      },
      {
        "command": "bun run --cwd packages/khala-fleet-intents typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-fleet-intents"
      },
      {
        "command": "bun run --cwd clients/khala-mobile typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-mobile"
      },
      {
        "command": "bun run --cwd packages/khala-qa-harness typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-qa-harness"
      },
      {
        "command": "bun run --cwd packages/khala-sync-db-collection typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-sync-db-collection"
      },
      {
        "command": "bun run --cwd packages/khala-tools typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-tools"
      },
      {
        "command": "bun run --cwd packages/mcp-contract typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:mcp-contract"
      },
      {
        "command": "bun run --cwd packages/nip90 typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:nip90"
      },
      {
        "command": "bun run --cwd apps/nostr-relay typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:nostr-relay"
      },
      {
        "command": "bun run --cwd packages/oa-infra typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:oa-infra"
      },
      {
        "command": "bun run --cwd apps/oa-queue-worker typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:oa-queue-worker"
      },
      {
        "command": "bun run --cwd apps/openagents-audio typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:openagents-audio"
      },
      {
        "command": "bun run --cwd apps/openagents-world typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:openagents-world"
      },
      {
        "command": "bun run --cwd packages/provider-account-schema typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:provider-account-schema"
      },
      {
        "command": "bun run --cwd packages/public-activity-timeline typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:public-activity-timeline"
      },
      {
        "command": "bun run --cwd packages/reactor-contracts typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:reactor-contracts"
      },
      {
        "command": "bun run --cwd packages/ui typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:ui"
      },
      {
        "command": "bun run --cwd packages/world-client typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:world-client"
      },
      {
        "command": "bun run --cwd packages/world-contract typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:world-contract"
      },
      {
        "command": "bun run --cwd apps/autopilot-desktop verify:deploy",
        "manifest_path": "package.json",
        "name": "verify:autopilot-desktop:deploy"
      },
      {
        "command": "bun run --cwd apps/autopilot-desktop run scripts/run-if-desktop-changed.ts -- bun run verify:deploy",
        "manifest_path": "package.json",
        "name": "verify:autopilot-desktop:if-changed"
      },
      {
        "command": "bun run --cwd apps/autopilot-desktop verify:training",
        "manifest_path": "package.json",
        "name": "verify:autopilot-desktop:training"
      },
      {
        "command": "specs/run-tlc.sh",
        "manifest_path": "package.json",
        "name": "verify:tla"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/agent-readiness/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/agent-readiness/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/agent-runtime-schema/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/agent-runtime-schema/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/ai-sdk-sandbox-local/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/ai-sdk-sandbox-local/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/ai-sdk-sandbox-openagents/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/ai-sdk-sandbox-openagents/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/arbiter-effect/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/arbiter-effect/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/assurance-spec/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/assurance-spec/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/atif/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/atif/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/audio-contract/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/audio-contract/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/autopilot-control-protocol/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/autopilot-control-protocol/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/autopilot-ui/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/autopilot-ui/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test src/*.test.ts",
        "manifest_path": "packages/behavior-contracts/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/behavior-contracts/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/blueprint-contracts/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/blueprint-contracts/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/cloud-contract/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/cloud-contract/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/composer-state/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/composer-state/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/connector-sidecar/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/connector-sidecar/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/design-tokens/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/design-tokens/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/durable-stream/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/durable-stream/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/effect-boundary/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/effect-boundary/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/effect-start/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/effect-start/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/forge-protocol/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/forge-protocol/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun scripts/live-acp-smoke.ts",
        "manifest_path": "packages/grok-harness/package.json",
        "name": "live-acp-smoke"
      },
      {
        "command": "bun scripts/mock-acp-stdio.ts",
        "manifest_path": "packages/grok-harness/package.json",
        "name": "mock-acp"
      },
      {
        "command": "bun scripts/rl-probe.ts",
        "manifest_path": "packages/grok-harness/package.json",
        "name": "rl-probe"
      },
      {
        "command": "bun scripts/rl4-worktree-probe.ts",
        "manifest_path": "packages/grok-harness/package.json",
        "name": "rl4-worktree-probe"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/grok-harness/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/grok-harness/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test src/*.test.ts",
        "manifest_path": "packages/harness-conformance/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/harness-conformance/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/input-bindings/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/input-bindings/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/khala-ai-sdk-core/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-ai-sdk-core/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test src/*.test.ts",
        "manifest_path": "packages/khala-fleet-intents/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-fleet-intents/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun src/lag-profiling-sweep.ts",
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "lag:sweep"
      },
      {
        "command": "bun src/monkey-night.ts --runs 100 --steps 64",
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "monkey:night"
      },
      {
        "command": "bun test src/monkey-explorer.test.ts --test-name-pattern 'bounded fixture smoke'",
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "monkey:smoke"
      },
      {
        "command": "bun src/architect-coder-judge-live-smoke.ts",
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "smoke:architect-coder-judge-live"
      },
      {
        "command": "bun src/real-bridge-smoke.ts",
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "smoke:real-bridge"
      },
      {
        "command": "bun test src/*.test.ts",
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "typecheck"
      },
      {
        "command": "node scripts/check-test-import-coverage.mjs",
        "manifest_path": "packages/khala-sync-client/package.json",
        "name": "check:test-import-coverage"
      },
      {
        "command": "bun test && node scripts/check-test-import-coverage.mjs",
        "manifest_path": "packages/khala-sync-client/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-sync-client/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/khala-sync-db-collection/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-sync-db-collection/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun scripts/capture.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "capture"
      },
      {
        "command": "bun scripts/check-pending-migrations.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "check:pending-migrations"
      },
      {
        "command": "bun scripts/compact.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "compact"
      },
      {
        "command": "bun scripts/load-test.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "load-test"
      },
      {
        "command": "bun scripts/migrate.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "migrate"
      },
      {
        "command": "bun scripts/query-compare-soak.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "query-compare-soak"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "test"
      },
      {
        "command": "bun test scripts/check-pending-migrations.test.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "test:pending-migrations-guard"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/khala-sync/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-sync/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/khala-tools/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-tools/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/mcp-contract/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/mcp-contract/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/nip90/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/nip90/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun scripts/migrate.ts",
        "manifest_path": "packages/oa-infra/package.json",
        "name": "migrate"
      },
      {
        "command": "bun test src",
        "manifest_path": "packages/oa-infra/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/oa-infra/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/portable-session-contract/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/portable-session-contract/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun run --cwd packages/runtime test",
        "manifest_path": "packages/probe/package.json",
        "name": "test"
      },
      {
        "command": "bun src/cli.ts",
        "manifest_path": "packages/probe/packages/runtime/package.json",
        "name": "probe"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/probe/packages/runtime/package.json",
        "name": "test"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/product-spec/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/product-spec/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/provider-account-schema/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/provider-account-schema/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/public-activity-timeline/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/public-activity-timeline/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/pylon-core/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/pylon-core/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun scripts/dogfood-smoke.ts",
        "manifest_path": "packages/reactor-contracts/package.json",
        "name": "smoke:dogfood"
      },
      {
        "command": "bun scripts/install-smoke.ts",
        "manifest_path": "packages/reactor-contracts/package.json",
        "name": "smoke:install"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/reactor-contracts/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/reactor-contracts/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun src/cli.ts",
        "manifest_path": "packages/sarah-take-scoreboard/package.json",
        "name": "score-take"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/sarah-take-scoreboard/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/sarah-take-scoreboard/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun run src/replay-cli.ts",
        "manifest_path": "packages/tassadar-executor/package.json",
        "name": "replay"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/tassadar-executor/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/tassadar-executor/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/ui/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/ui/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test test/react-edition.test.tsx",
        "manifest_path": "packages/ui/package.json",
        "name": "visual-smoke"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/world-client/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/world-client/package.json",
        "name": "typecheck"
      },
      {
        "command": "bun test",
        "manifest_path": "packages/world-contract/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/world-contract/package.json",
        "name": "typecheck"
      }
    ],
    "diagnostics": [
      "candidate_artifact_inventory_truncated",
      "repository_candidates_unmapped"
    ],
    "head": "376d98fa4cc973af334b09a61e7ecba0dcae127a",
    "inventory_digest": "sha256:933968124c7b907eae4656f6e4b6d1d7d7862e486591f52d84970e551b19b524",
    "repository_label": "openagents-wt-252-notes",
    "state": "clean",
    "tracked_file_count": 10560,
    "tree": "7094983971df54aed2796787b94ee22326e16338",
    "truncated": true
  }
}
```

## Obligations

Each obligation binds one criterion to a criterion-local contract oracle and a deterministic missing-anchor falsifier. The UX-5 coverage matrix is the interaction-level projection of those items. It adds the explicit UX-1/UX-2/UX-3 contract and oracle links and rejects any visible surface not justified by this exact scope. The complete Desktop suite and installed RC9 journey are required companion evidence, so a narrow contract result cannot independently authorize release or a public completion claim.

```assurancespec-obligations
[
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-01"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-01-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-01 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:96254dc85e649218554d1cdb9ec7ec36a0813f7696204b48df6119025b35cf7c",
    "source_claim_snapshot": "A signed/notarized release candidate installs and launches without a source\ncheckout, resolves only its pinned compatible Codex runtime path, and reports\nmissing or incompatible runtime state explicitly.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-01"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-02"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-02-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-02 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:9956af177e2962268ada34b1eb0f7b6c5e9a495115c68d72b67b1b8b5c51283f",
    "source_claim_snapshot": "Local-first mode can reach the first useful Codex workroom without an\nOpenAgents account or hosted service. It uses only the user's ordinary\nlogged-in Codex session, clears any inherited `CODEX_HOME`, and exposes no\nnamed-Pylon account linking, isolated device-auth, or account rotation in the\nMVP workroom.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-02"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-03"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-03-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-03 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:520b3de4719ed980c5df9a8f56a963eefb3c426ed70c044566c0d7e991fd45f8",
    "source_claim_snapshot": "Granting one repository creates a stable WorkContext and product session ref\nthat do not derive from a path, process, port, machine, or provider thread ID.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-03"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-04"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-04-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-04 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:ea6517af8f157701b183991dbcedbbe2e603b9ec28dc53109659063d1850ecba",
    "source_claim_snapshot": "From one guided conversation, the workroom creates a validator-clean\nProductSpec v0.1 draft or opens an existing spec. Validation failures identify\nthe exact section. An unlabeled legacy spec remains viewable, but executable\ncriteria require unique author-visible IDs and no work starts while any ID is\nmissing or duplicated.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-04"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-05"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-05-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-05 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:af4e081e296d638acc389a976a2adab0cd4f232e8f7e94f06bb4eb94356cf814",
    "source_claim_snapshot": "The workroom shows the exact ProductSpec digest and `spec_revision`, previews\nevery intent-changing edit as a diff, requires user confirmation plus a\nrevision bump, and retains the prior revision for already admitted work.\nRetained criterion IDs may map across revisions; changed or removed IDs\nrequire explicit reconciliation.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-05"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-06"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-06-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-06 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:d7099615d7a3c38b037a9b79556edf9cadc04e39b6c32037d1c01926458ad9c2",
    "source_claim_snapshot": "A user-accepted execution plan contains at least two durable work packets.\nEvery packet cites the exact spec revision and one or more criterion refs;\nat least one packet can be allocated to a child agent and opened from both\nthe criterion board and causal timeline. Before execution, every criterion is\nmapped or explicitly deferred, every mutating packet has at most one active\nexecution lease, and duplicate or cyclic work packets refuse.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-06"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-07"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-07-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-07 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:0fc9b7aa277bc5ee0556fae35b13ab8041c83d25a2307fb5f3533bc56e080025",
    "source_claim_snapshot": "The product-owned `productspec-work` skill ships hash-pinned in the signed\ncompatibility set, is registered from the app-owned resource root into the\ncurrent Codex session through the native app-server surface, and can refine, decompose,\nallocate, and report through typed host tools. Removing, corrupting, or\nversion-mismatching it produces an explicit incompatible workflow state; it\nnever falls back to an ambient/user-installed skill and never copies itself\ninto the default Codex home.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-07"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-08"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-08-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-08 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:4350ae8c8ff59199e024df4462387b3d8c0a994c7f99e4a1166cb6d26815a4bb",
    "source_claim_snapshot": "Skill or agent prose cannot approve a spec edit, admit a work packet, change\nthe pinned revision, or mark a criterion verified. Evidence-present and\nverified remain distinct. Verification requires linked test/verifier output,\nbehavior/Eval oracle, artifact or diff review, or receipt; owner acceptance\nor waiver remains a separate typed disposition.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-08"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-09"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-09-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-09 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:07e64e1ead0897b6da45a95871ae61f27c2a45b66b29f47af75f82fde8eb137c",
    "source_claim_snapshot": "A spec revision/digest change while work is active produces a typed mismatch.\nNew dispatch stops until the user reconciles, supersedes, or cancels the old\nplan; active work is never silently retargeted and no evidence crosses\nrevisions without an explicit mapping.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-09"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-10"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-10-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-10 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:202d24f1116fff61ca338f6458bc4cf68caa529cb2b146fe916bab19ad7e5104",
    "source_claim_snapshot": "The session rail paints bounded metadata before transcript hydration, lists\nonly top-level sessions, pages without an age ceiling, and preserves stable\ntitles, status, attention, ordering, and selected session through restart.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-10"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-11"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-11-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-11 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:e29a2fd859224fe11e32bbf56f89c70ec90bec4e3e9888da559fc3d40bb25896",
    "source_claim_snapshot": "One real Codex task is durably admitted before dispatch and renders typed\ntext plus at least one non-text plan, tool, patch/file-change, usage, blocker,\nor lifecycle item and exactly one terminal disposition.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-11"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-12"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-12-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-12 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:111e82a689f475b833dceadfde912b40ca663335b56fac363c97196d067ca730",
    "source_claim_snapshot": "Exact retry reconciles to the admitted intent; conflicting reuse refuses.\nSend, stop, steer, queue, question, approval, and plan-review actions use the\nsame registered command identities across direct, keyboard, palette, and\nnative-menu entry points.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-12"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-13"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-13-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-13 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:2cb93d68bef3e709f3e0a42c3f2a79c563396ca8d96a183a135a6454a273d2be",
    "source_claim_snapshot": "The complete child graph retains exact parentage and lifecycle. A causal\ninline card opens one child's independent transcript; reload/reconnect never\nflattens, duplicates, re-roots, or leaks a child into the top-level catalog.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-13"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-14"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-14-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-14 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:fb91c82fe2706512e4285fac6725ea32925403058f84449fe0a7e90d74a1c024",
    "source_claim_snapshot": "The granted repository exposes a bounded file tree, Git status, and exact\ndiff correlated to timeline item refs. Revocation and post-image conflict\nfail visibly without exposing general filesystem or Git mutation authority.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-14"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-15"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-15-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-15 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:9b603d9c1b3a91f3a77e8e59c41a81b2a103acda23388c697c9e0ca7f9014f73",
    "source_claim_snapshot": "Renderer reload does not stop or duplicate host-owned work. App-process\nrestart restores the exact persisted prefix and either continues the\nrecorded Codex thread at most once or records an explicit interrupted\nterminal outcome; it never silently reruns the task. Open in Codex is offered\nonly after the OpenAgents attempt is quiescent or authoritatively reconciled,\npreserves the admitted packet identity, and labels exact-thread continuation\nseparately from repository-state handoff and transcript-gap recovery.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-15"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-16"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-16-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-16 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:e9b73d46b15a925a64e6149d36d724d32ca45e9cd8fbaaa04d6ee4dd9f19b8ff",
    "source_claim_snapshot": "Lost acknowledgement, duplicate/out-of-order frame, cursor gap, stale\ngeneration, revoked grant, quota exhaustion, rate limit, auth revocation,\nand policy denial converge to distinct typed states. Durable repair precedes\nlive resubscription.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-16"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-17"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-17-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-17 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:618c2b5880038c5c6526b0071ac4f5d0c363a0ef2df808039f57114711236687",
    "source_claim_snapshot": "Diagnostics and non-content renderer control envelopes contain no\ncredential, account identity, loopback URL/secret, raw provider event,\nprompt/transcript body, repository content, absolute root, generic IPC,\nprocess handle, or general filesystem handle. Content views receive only\nbounded transcript and repository projections admitted for the selected\nwork context; they never receive raw provider payloads, credentials,\nabsolute roots, or general process/filesystem authority.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-17"
  },
  {
    "activation_gate": "GATE-MVP-FULL-ASSURANCE",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"
    ],
    "criterion_refs": [
      "CW-AC-18"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workroom",
      "release_artifact"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-MVP-BUN-1"
    ],
    "evidence": {
      "proof_rung": "reviewed_release_plus_current_regression",
      "required_kinds": [
        "native_junit",
        "assurance_receipt",
        "oracle_sensitivity_receipt",
        "installed_release_receipt",
        "full_desktop_gate"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_required_anchor",
      "ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"
    },
    "id": "AO-CW-AC-18-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "statement": "The exact CW-AC-18 implementation/release anchors remain present and the criterion-local candidate test passes."
    },
    "source_claim_digest": "sha256:c0a258b64639d90909f16e2d5db2720371cb0f89f0e3cb240d7b4f3317a815a0",
    "source_claim_snapshot": "The exact release candidate passes install, launch, one real Codex workroom\ntask, renderer reload, app restart, interrupted update, rollback/downgrade\nrefusal, diagnostics export, uninstall/reinstall, and cleanup receipts.",
    "technique": "criterion_contract_with_sensitivity",
    "title": "Assure CW-AC-18"
  }
]
```

## Gates

The MVP assurance gate passes only when exact admission and environment bindings are current, every candidate is CONFIRMED, every falsifier is REFUTED, infrastructure is ready, observations are stable, independent review accepts each candidate, no exception remains, and the full Desktop regression gate is green.

```assurancespec-gates
[
  {
    "expression": "admitted && executable && candidate=CONFIRMED && falsifier=REFUTED && infrastructure=ready && stability=stable && freshness=current && disposition=accepted && exception=none && full_desktop_gate=green",
    "id": "GATE-MVP-FULL-ASSURANCE"
  }
]
```

## Evidence Policy

Links remain evidence locations rather than verdicts. Native output stays private, normalized receipts are reviewed public-safe projections, and missing or stale artifacts remain INCONCLUSIVE. Candidate, sensitivity, installed-release, and full-regression evidence must all remain independently visible.

```assurancespec-evidence-policy
{
  "links_are_verdicts": false,
  "missing_evidence_verdict": "INCONCLUSIVE",
  "policy_state": "designed",
  "required_for_ready_obligation": [
    "oracle_observation",
    "falsifier_observation",
    "environment_binding",
    "independent_review",
    "installed_release_receipt",
    "full_desktop_gate"
  ]
}
```

## Authority Boundaries

The owner admits this exact proof design and has accepted the installed ProductSpec-native journey and its read-only review boundary. The runner may execute and report only. It cannot alter owner acceptance, publish RC9, change registries or promises, waive failures, or infer authority from prose or green tests.

```assurancespec-authority
{
  "admitted_roles": [
    "openagents.owner"
  ],
  "policy_state": "designed",
  "proposal_may_change_public_promises": false,
  "proposal_may_execute": false,
  "proposal_may_release": false,
  "proposal_may_self_admit": false,
  "proposal_may_verify": false,
  "release_roles": [
    "openagents.owner"
  ],
  "verifier_roles": [
    "openagents.assurance_reviewer"
  ]
}
```
