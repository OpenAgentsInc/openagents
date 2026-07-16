---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.full.auto.codex.composer.loop"
assurance_revision: 1
title: "Full Auto Provider-Lane Composer Loop Assurance Spec"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "OpenAgents"
---

## Assurance Objective

This proposed AssuranceSpec creates exact criterion-to-obligation coverage without claiming that proof design, execution, evidence, admission, or release is complete.

## Subject

The proposal is bound to the exact ProductSpec bytes, revision, path, and stable criterion identifiers below.

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
      "FA-AC-33"
    ],
    "document_digest": "sha256:60a5efeba64a4a83314c7e9fe8910b3505d795cdcf87915e8844762ffc4a9557",
    "path": "specs/desktop/full-auto.product-spec.md",
    "profile": "openagents_executable_v0.1_exact_document",
    "spec_format_version": "0.1",
    "spec_revision": 8
  }
}
```

## Risk Model

No risk objects are inferred from ProductSpec prose. Reviewers must design the applicable risk model.

```assurancespec-risks
{
  "risks": [],
  "source_digest": "sha256:1084a9edb589ef44d32854a76620d88fe517c9cce4ceba15b537b96bf839849c",
  "source_snapshot": "The source ProductSpec contains no Risks section. Assurance risk modeling remains required."
}
```

## Assurance Scope

Every executable ProductSpec criterion is in assurance scope. No criterion is silently excluded or marked not applicable.

## Environments

Repository facts are proposal context only. No Environment Profile, adapter, capability, or permission is selected by inventory.

```assurancespec-environments
{
  "profiles": [],
  "repository_inventory": {
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
      "apps/oa-updates/src/production-entrypoint.test.ts",
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
      "apps/openagents-desktop/src/codex-local-runtime.test.ts",
      "apps/openagents-desktop/src/codex-native-event-plane.test.ts",
      "apps/openagents-desktop/src/codex-preflight.test.ts",
      "apps/openagents-desktop/src/codex-release-notes.test.ts",
      "apps/openagents-desktop/src/codex-reverse-rpc-arbiter.test.ts",
      "apps/openagents-desktop/src/codex-thread-lifecycle.test.ts",
      "apps/openagents-desktop/src/codex-turn-state.test.ts",
      "apps/openagents-desktop/src/composer-admission.test.ts",
      "apps/openagents-desktop/src/desktop-codex-usage-reporter.test.ts",
      "apps/openagents-desktop/src/desktop-host-lifecycle.test.ts",
      "apps/openagents-desktop/src/desktop-launch-workspace.test.ts",
      "apps/openagents-desktop/src/desktop-operation-context.test.ts",
      "apps/openagents-desktop/src/desktop-renderer-location.test.ts",
      "apps/openagents-desktop/src/desktop-runtime-workspace.test.ts",
      "apps/openagents-desktop/src/desktop-worker-location.test.ts",
      "apps/openagents-desktop/src/extension-lifecycle-contract.test.ts",
      "apps/openagents-desktop/src/fable-local-runtime-caps.test.ts",
      "apps/openagents-desktop/src/fable-local-runtime.test.ts",
      "apps/openagents-desktop/src/full-auto-control-server.test.ts",
      "apps/openagents-desktop/src/git-github-contract.test.ts",
      "apps/openagents-desktop/src/git-github-host.test.ts",
      "apps/openagents-desktop/src/git-review-corpus.node.test.ts",
      "apps/openagents-desktop/src/history-thread-actions.test.ts",
      "apps/openagents-desktop/src/live-agent-graph-host.test.ts",
      "apps/openagents-desktop/src/live-agent-graph-local.test.ts",
      "apps/openagents-desktop/src/live-proof.test.ts",
      "apps/openagents-desktop/src/local-runtime-event-persistence.test.ts",
      "apps/openagents-desktop/src/local-turn-journal.test.ts",
      "apps/openagents-desktop/src/local-turn-recovery.test.ts",
      "apps/openagents-desktop/src/local-turn-text-persistence.test.ts",
      "apps/openagents-desktop/src/macos-update-applier.test.ts",
      "apps/openagents-desktop/src/mcp-config-host.test.ts",
      "apps/openagents-desktop/src/mvp-assurance-bridge.test.ts",
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "apps/openagents-desktop/src/mvp-proof.test.ts",
      "apps/openagents-desktop/src/product-spec-app-server-tools.test.ts",
      "apps/openagents-desktop/src/product-spec-workroom.test.ts",
      "apps/openagents-desktop/src/provider-lane-acp.test.ts",
      "apps/openagents-desktop/src/provider-lane-capabilities.test.ts",
      "apps/openagents-desktop/src/provider-lane.test.ts",
      "apps/openagents-desktop/src/provider-runtime-compatibility.test.ts",
      "apps/openagents-desktop/src/provider-runtime-host.test.ts",
      "apps/openagents-desktop/src/provider-runtime-target.test.ts",
      "apps/openagents-desktop/src/react-conversation-assurance.test.ts",
      "apps/openagents-desktop/src/renderer/assurance-spec-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/branding.test.ts",
      "apps/openagents-desktop/src/renderer/command-notice.test.ts",
      "apps/openagents-desktop/src/renderer/composer-focus.test.ts",
      "apps/openagents-desktop/src/renderer/composer-image-acquisition.test.ts",
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
      "apps/openagents-desktop/src/renderer/mvp-assurance-congruence.test.ts",
      "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.test.ts",
      "apps/openagents-desktop/src/renderer/navigation-history.test.ts",
      "apps/openagents-desktop/src/renderer/product-spec-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
      "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx",
      "apps/openagents-desktop/src/renderer/react-review-sheet.test.tsx",
      "apps/openagents-desktop/src/renderer/react-review.test.tsx",
      "apps/openagents-desktop/src/renderer/react-timeline.test.tsx",
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
      "apps/openagents-desktop/src/renderer/terminal-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/tool-cards.test.ts",
      "apps/openagents-desktop/src/renderer/visual-baseline-fixtures.test.ts",
      "apps/openagents-desktop/src/renderer/visual-baseline-workbench.test.tsx",
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
      "apps/openagents-desktop/tests/fleet-control.test.ts",
      "apps/openagents-desktop/tests/full-auto-registry.test.ts",
      "apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts",
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
      "apps/openagents-desktop/tests/owner-ux-rules.test.ts",
      "apps/openagents-desktop/tests/package-macos.test.ts",
      "apps/openagents-desktop/tests/plugin-config.test.ts",
      "apps/openagents-desktop/tests/provider-accounts.test.ts",
      "apps/openagents-desktop/tests/publish-release.test.ts",
      "apps/openagents-desktop/tests/release-preflight.test.ts",
      "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
      "apps/openagents-desktop/tests/service-topology.test.ts",
      "apps/openagents-desktop/tests/startup-contract.test.ts",
      "apps/openagents-desktop/tests/turn-checkpoints.test.ts",
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
      "apps/openagents.com/apps/start/src/docs/docs-content.test.ts",
      "apps/openagents.com/apps/start/src/docs/docs-layout-contract.test.ts",
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
      "apps/openagents.com/apps/start/src/routes/-components-workbench-page.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-components.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-forum-khala-motion.test.ts",
      "apps/openagents.com/apps/start/src/routes/-forum-markdown.test.ts",
      "apps/openagents.com/apps/start/src/routes/-forum.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-funnel.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-gym.test.tsx",
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
      "apps/openagents.com/apps/start/src/routes/-public-site.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-pylon-codex-assignment-status.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-pylons.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-retired-app.test.ts",
      "apps/openagents.com/apps/start/src/routes/-run.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-sales-landing.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-share-fetch.test.ts",
      "apps/openagents.com/apps/start/src/routes/-share.test.tsx",
      "apps/openagents.com/apps/start/src/routes/-splash-khala-canvas.test.ts",
      "apps/openagents.com/apps/start/src/routes/-splash.test.tsx",
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
      "apps/openagents.com/apps/start/src/test/vitest-cwd-setup.ts",
      "apps/openagents.com/apps/start/src/typography-contract.test.ts",
      "apps/openagents.com/apps/start/vitest.config.ts",
      "apps/openagents.com/packages/effect-native-render-dom/tests/index.test.ts",
      "apps/openagents.com/packages/effect-native-render-dom/tests/react.test.ts",
      "apps/openagents.com/packages/effect-native-render-rn/src/index.test.ts",
      "apps/openagents.com/packages/email-templates/src/index.test.ts",
      "apps/openagents.com/packages/mullet-schema/src/index.test.ts",
      "apps/openagents.com/packages/mullet-sim/src/index.test.ts",
      "apps/openagents.com/packages/sync-client/src/index.test.ts",
      "apps/openagents.com/packages/sync-schema/src/json-boundary.test.ts",
      "apps/openagents.com/packages/sync-schema/src/runner-event.test.ts",
      "apps/openagents.com/packages/sync-schema/src/token-usage.test.ts",
      "apps/openagents.com/packages/sync-worker/src/index.test.ts",
      "apps/openagents.com/scripts/artanis-production-readiness.test.ts",
      "apps/openagents.com/scripts/check-command-composer-privacy-fixtures.test.ts",
      "apps/openagents.com/scripts/check-conflict-markers.test.ts",
      "apps/openagents.com/scripts/check-contract-drift.test.ts",
      "apps/openagents.com/scripts/check-effect-native-vendor.test.ts",
      "apps/openagents.com/scripts/check-public-projection-freshness.test.ts",
      "apps/openagents.com/scripts/conversation-bundle-redaction.test.ts",
      "apps/openagents.com/scripts/customer-one-cohort-recorder.test.ts",
      "apps/openagents.com/scripts/d1-zero-reference-sweep.test.ts",
      "apps/openagents.com/scripts/forum.test.ts",
      "apps/openagents.com/scripts/gpt-oss20b-production-smoke.test.ts",
      "apps/openagents.com/scripts/gym-harbor-progress-push.test.ts",
      "apps/openagents.com/scripts/khala-gateway-readiness-smoke.test.ts",
      "apps/openagents.com/scripts/khala-glm-reap-production-smoke.test.ts",
      "apps/openagents.com/scripts/khala-production-readiness-monitor.test.ts",
      "apps/openagents.com/scripts/khala-production-smoke.test.ts",
      "apps/openagents.com/scripts/predeploy-khala-sync-live-seam-smoke.test.ts",
      "apps/openagents.com/scripts/predeploy-parallel-dispatch-smoke.test.ts",
      "apps/openagents.com/scripts/provider-chatgpt-device-login.test.ts",
      "apps/openagents.com/scripts/public-activity-proof-links-smoke.test.ts",
      "apps/openagents.com/scripts/site-speed-landing.test.ts",
      "apps/openagents.com/scripts/tassadar-live-page-smoke.test.ts",
      "apps/openagents.com/scripts/visibility-browser-smoke.test.ts",
      "apps/openagents.com/scripts/visibility-freshness-smoke.test.ts",
      "apps/openagents.com/workers/api/scripts/cloudrun/deploy-bundle-contract.test.ts",
      "apps/openagents.com/workers/api/src/accepted-outcomes-per-kwh.test.ts",
      "apps/openagents.com/workers/api/src/adjutant-assignments.test.ts",
      "apps/openagents.com/workers/api/src/adjutant-enrichment-ledger.test.ts",
      "apps/openagents.com/workers/api/src/adjutant-enrichment-operations.test.ts",
      "apps/openagents.com/workers/api/src/adjutant-enrichment-planner.test.ts",
      "apps/openagents.com/workers/api/src/adjutant-public-activity.test.ts",
      "apps/openagents.com/workers/api/src/adjutant-public-source-refs.test.ts",
      "apps/openagents.com/workers/api/src/adjutant-research-briefs.test.ts",
      "apps/openagents.com/workers/api/src/adjutant-research-policies.test.ts",
      "apps/openagents.com/workers/api/src/adjutant-usage-receipts.test.ts",
      "apps/openagents.com/workers/api/src/admin-access.test.ts",
      "apps/openagents.com/workers/api/src/admin-ops-routes.test.ts",
      "apps/openagents.com/workers/api/src/admin-overview-routes.test.ts",
      "apps/openagents.com/workers/api/src/agent-claim-reward-ledger.test.ts",
      "apps/openagents.com/workers/api/src/agent-claim-reward-policy.test.ts",
      "apps/openagents.com/workers/api/src/agent-definition-event-ledger-routes.test.ts",
      "apps/openagents.com/workers/api/src/agent-definition-routes.test.ts",
      "apps/openagents.com/workers/api/src/agent-definition-run-routes.test.ts",
      "apps/openagents.com/workers/api/src/agent-definition-scheduler.test.ts",
      "apps/openagents.com/workers/api/src/agent-definition-trigger-store.test.ts",
      "apps/openagents.com/workers/api/src/agent-definition-webhook-routes.test.ts",
      "apps/openagents.com/workers/api/src/agent-goal-hardening.test.ts",
      "apps/openagents.com/workers/api/src/agent-goal-routes.test.ts",
      "apps/openagents.com/workers/api/src/agent-goal-runtime.test.ts",
      "apps/openagents.com/workers/api/src/agent-goals.test.ts",
      "apps/openagents.com/workers/api/src/agent-home-routes.test.ts",
      "apps/openagents.com/workers/api/src/agent-owner-claim-routes.test.ts",
      "apps/openagents.com/workers/api/src/agent-proposal-routes.test.ts",
      "apps/openagents.com/workers/api/src/agent-readiness-public-report-routes.test.ts",
      "apps/openagents.com/workers/api/src/agent-readiness-public-report-store.test.ts",
      "apps/openagents.com/workers/api/src/agent-registration-postgres-fallback.test.ts",
      "apps/openagents.com/workers/api/src/agent-registration-routes.test.ts",
      "apps/openagents.com/workers/api/src/agent-registration.test.ts",
      "apps/openagents.com/workers/api/src/agent-runtime-kernel.test.ts",
      "apps/openagents.com/workers/api/src/agent-runtime-repository.contract.test.ts",
      "apps/openagents.com/workers/api/src/agent-runtime-store.test.ts",
      "apps/openagents.com/workers/api/src/agent-scoped-grant-routes.test.ts",
      "apps/openagents.com/workers/api/src/agent-search-routes.test.ts",
      "apps/openagents.com/workers/api/src/agentic-labor-product-claim-upgrade.test.ts",
      "apps/openagents.com/workers/api/src/agentic-labor-product-demand.test.ts",
      "apps/openagents.com/workers/api/src/agentic-labor-product-routes.test.ts",
      "apps/openagents.com/workers/api/src/agentic-labor-product-settlement.test.ts",
      "apps/openagents.com/workers/api/src/agentic-labor-product.test.ts",
      "apps/openagents.com/workers/api/src/agentic-npm-composition-runtime.test.ts",
      "apps/openagents.com/workers/api/src/artanis-activity-routes.test.ts",
      "apps/openagents.com/workers/api/src/artanis-administrator-corpus.test.ts",
      "apps/openagents.com/workers/api/src/artanis-administrator-labor-tick.test.ts",
      "apps/openagents.com/workers/api/src/artanis-approval-gates.test.ts"
    ],
    "declared_scripts": [
      {
        "command": "playwright install --with-deps chromium",
        "manifest_path": "apps/acceptance-runner/package.json",
        "name": "playwright:install"
      },
      {
        "command": "node --import tsx src/run-once.ts",
        "manifest_path": "apps/acceptance-runner/package.json",
        "name": "run-once"
      },
      {
        "command": "node --import tsx src/service.ts",
        "manifest_path": "apps/acceptance-runner/package.json",
        "name": "serve"
      },
      {
        "command": "vp test --run src/daemon.test.ts",
        "manifest_path": "apps/acceptance-runner/package.json",
        "name": "test"
      },
      {
        "command": "pnpm run build:cloudrun",
        "manifest_path": "apps/aiur/package.json",
        "name": "build"
      },
      {
        "command": "vp build --config vite.config.cloudrun.ts --logLevel warn && vp pack src/cloudrun/server.ts --format esm --platform node --out-dir dist/cloudrun",
        "manifest_path": "apps/aiur/package.json",
        "name": "build:cloudrun"
      },
      {
        "command": "pnpm run deploy:cloudrun",
        "manifest_path": "apps/aiur/package.json",
        "name": "deploy"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh",
        "manifest_path": "apps/aiur/package.json",
        "name": "deploy:cloudrun"
      },
      {
        "command": "vp dev --config vite.config.cloudrun.ts",
        "manifest_path": "apps/aiur/package.json",
        "name": "dev"
      },
      {
        "command": "vp preview --config vite.config.cloudrun.ts",
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
        "command": "node --import tsx src/index.ts",
        "manifest_path": "apps/forum/package.json",
        "name": "dev"
      },
      {
        "command": "vp test --run",
        "manifest_path": "apps/forum/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/forum/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp pack src/server.ts --format esm --platform node --out-dir dist",
        "manifest_path": "apps/khala-capture/package.json",
        "name": "build:cloudrun"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh",
        "manifest_path": "apps/khala-capture/package.json",
        "name": "deploy:cloudrun"
      },
      {
        "command": "node --watch --import tsx src/server.ts",
        "manifest_path": "apps/khala-capture/package.json",
        "name": "dev"
      },
      {
        "command": "node --import tsx src/server.ts",
        "manifest_path": "apps/khala-capture/package.json",
        "name": "start"
      },
      {
        "command": "vp test --run",
        "manifest_path": "apps/khala-capture/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/khala-capture/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp pack src/server.ts --format esm --platform node --out-dir dist",
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "build:cloudrun"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh",
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "deploy:cloudrun"
      },
      {
        "command": "node --watch --import tsx src/server.ts",
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "dev"
      },
      {
        "command": "node --import tsx src/server.ts",
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "start"
      },
      {
        "command": "vp test --run",
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/khala-live-hub/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp pack src/main.ts --format esm --platform node --out-dir dist",
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "build"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh",
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "deploy"
      },
      {
        "command": "node --import tsx src/main.ts",
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "serve"
      },
      {
        "command": "vp test --run src",
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/oa-queue-worker/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp pack src/serve.ts --out-dir dist-server --format esm --platform node --target node24 --minify",
        "manifest_path": "apps/oa-updates/package.json",
        "name": "build:server"
      },
      {
        "command": "node --import tsx scripts/publish-desktop-release.ts",
        "manifest_path": "apps/oa-updates/package.json",
        "name": "desktop:publish"
      },
      {
        "command": "node --import tsx src/serve.ts",
        "manifest_path": "apps/oa-updates/package.json",
        "name": "serve"
      },
      {
        "command": "vp test --run",
        "manifest_path": "apps/oa-updates/package.json",
        "name": "test"
      },
      {
        "command": "node scripts/verify-test-typecheck.mjs",
        "manifest_path": "apps/oa-updates/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp pack src/cloudrun.ts --format esm --platform node --out-dir dist",
        "manifest_path": "apps/openagents-audio-edge/package.json",
        "name": "build"
      },
      {
        "command": "bash deploy-cloudrun.sh",
        "manifest_path": "apps/openagents-audio-edge/package.json",
        "name": "deploy"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents-audio-edge/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp pack src/main.ts --format esm --platform node --out-dir dist",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "build:cloudrun"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "deploy:cloudrun"
      },
      {
        "command": "node --import tsx scripts/live-barge-smoke.ts",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "smoke:barge:live"
      },
      {
        "command": "node --import tsx scripts/live-retention-smoke.ts",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "smoke:live"
      },
      {
        "command": "node --import tsx scripts/live-tts-smoke.ts",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "smoke:tts:live"
      },
      {
        "command": "node --import tsx src/main.ts",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "start"
      },
      {
        "command": "vp test --run src test",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents-audio/package.json",
        "name": "typecheck"
      },
      {
        "command": "node --import tsx scripts/build.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "build"
      },
      {
        "command": "node --import tsx scripts/dev.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "dev"
      },
      {
        "command": "node --import tsx scripts/full-auto-cli.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "full-auto"
      },
      {
        "command": "node --import tsx scripts/generate-codex-conformance-report.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "generate:codex-conformance"
      },
      {
        "command": "node --import tsx scripts/build.ts && node --import tsx scripts/run-live-proof.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "live-proof"
      },
      {
        "command": "node --import tsx scripts/prepare-macos-maker.ts && electron-forge make --platform=darwin --arch=arm64",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "make:mac"
      },
      {
        "command": "node --import tsx scripts/build.ts && node --import tsx scripts/run-mvp-proof.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "mvp-proof"
      },
      {
        "command": "electron-forge package --platform=darwin --arch=arm64",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "package:mac"
      },
      {
        "command": "node --import tsx scripts/build.ts && node --import tsx scripts/visual-baseline-smoke.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "qa:visual"
      },
      {
        "command": "node --import tsx scripts/run-release-acceptance.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "release-acceptance"
      },
      {
        "command": "node --import tsx scripts/build.ts && OPENAGENTS_DESKTOP_SMOKE=1 electron .",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke"
      },
      {
        "command": "node --import tsx scripts/codex-runtime-artifact-smoke.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:artifact:codex-runtime"
      },
      {
        "command": "node --import tsx scripts/codex-binary-manifest-smoke.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-binary-manifest"
      },
      {
        "command": "node --import tsx scripts/codex-control-plane-smoke.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-control-plane"
      },
      {
        "command": "node --import tsx scripts/codex-ecosystem-smoke.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-ecosystem"
      },
      {
        "command": "node --import tsx scripts/codex-experimental-runtime-smoke.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-experimental"
      },
      {
        "command": "node --import tsx scripts/codex-host-services-smoke.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-host-services"
      },
      {
        "command": "node --import tsx scripts/codex-app-server-supervisor-smoke.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-supervisor"
      },
      {
        "command": "node --import tsx scripts/codex-thread-lifecycle-smoke.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-thread-lifecycle"
      },
      {
        "command": "node --import tsx scripts/codex-turn-control-smoke.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:codex-turn-control"
      },
      {
        "command": "node --import tsx scripts/full-auto-control-smoke.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:full-auto-control"
      },
      {
        "command": "node --import tsx scripts/full-auto-restart-smoke.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:full-auto-restart"
      },
      {
        "command": "node --import tsx scripts/build.ts && OPENAGENTS_DESKTOP_SMOKE=1 OPENAGENTS_DESKTOP_HEADED=1 electron .",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:headed"
      },
      {
        "command": "node --import tsx scripts/build.ts && OPENAGENTS_DESKTOP_SMOKE=1 OPENAGENTS_DESKTOP_SMOKE_REACT=1 electron .",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "smoke:react"
      },
      {
        "command": "node --import tsx scripts/startup-bench.ts",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "startup-bench"
      },
      {
        "command": "vp test --run --max-concurrency 1 --root ../.. apps/openagents-desktop",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents-desktop/package.json",
        "name": "typecheck"
      },
      {
        "command": "pnpm run typecheck && pnpm run test && pnpm run build && OPENAGENTS_DESKTOP_SMOKE=1 pnpm run smoke && pnpm run smoke:react",
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
        "command": "vp test --run",
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents-mobile/package.json",
        "name": "typecheck"
      },
      {
        "command": "pnpm run build && node --import tsx src/routes/-funnel-budget.ts",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "budget"
      },
      {
        "command": "pnpm run generate:docs && vp build --logLevel warn && vp pack cloudrun/server.mjs --out-dir dist/cloudrun --format esm --platform node --target node24 --minify",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "build"
      },
      {
        "command": "node --import tsx scripts/generate-docs.ts --check",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "check:docs"
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
        "command": "pnpm run generate:docs && vp dev",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "dev"
      },
      {
        "command": "node --import tsx scripts/generate-docs.ts",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "generate:docs"
      },
      {
        "command": "vp preview",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "preview"
      },
      {
        "command": "pnpm run generate:docs && vp test --root ../../../.. --run --project @openagentsinc/openagents-com-start",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "test"
      },
      {
        "command": "pnpm run generate:docs && tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "typecheck"
      },
      {
        "command": "node scripts/artanis-production-readiness.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "artanis:readiness"
      },
      {
        "command": "pnpm run build:start && pnpm run build:api",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "build"
      },
      {
        "command": "pnpm --dir workers/api run build",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "build:api"
      },
      {
        "command": "node scripts/sync-live-agent-doc.mjs && pnpm --dir apps/start run build",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "build:start"
      },
      {
        "command": "node --import tsx scripts/check-live-agent-doc-links.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:agent-doc-links"
      },
      {
        "command": "node --import tsx scripts/check-zero-debt-architecture.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:architecture"
      },
      {
        "command": "node --import tsx scripts/check-conflict-markers.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:conflict-markers"
      },
      {
        "command": "node --import tsx scripts/check-contract-drift.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:contract-drift"
      },
      {
        "command": "pnpm run check:conflict-markers && pnpm run check:no-github-actions && pnpm run check:effect-topology && pnpm run check:agent-doc-links && pnpm run check:architecture && pnpm run check:contract-drift && pnpm run check:public-projection-freshness && pnpm --dir ../../packages/agent-readiness run test && pnpm --dir ../../packages/agent-readiness run typecheck && pnpm --dir ../../packages/autopilot-control-protocol run typecheck && pnpm --dir ../../apps/pylon run typecheck && pnpm run typecheck:api-pylon-integration && pnpm --dir ../../apps/pylon run test tests/security-adversarial-harness.test.ts && pnpm --dir ../../packages/khala-sync-server run test:pending-migrations-guard && pnpm --dir ../../packages/khala-sync-client run test && pnpm run typecheck:start && pnpm run typecheck:api && pnpm run test:conflict-markers-guard && pnpm run test:effect-native-vendor-guard && pnpm run test:contract-drift-guard && pnpm --dir apps/start run test && pnpm --dir workers/api run test src/lander-css-policy.test.ts src/worker-routes.test.ts src/redirect-policy.test.ts src/client-server-route-agreement.test.ts src/mullet/routes.test.ts src/product-promises.test.ts src/model-custody-lead-gen.test.ts src/reactor-need-to-know-access.test.ts src/reactor-data-liberation.test.ts src/reactor-improvement-ladder.test.ts src/wasm-plugin-marketplace.test.ts src/qualified-contributor-methodology.test.ts src/public-forum-activity-routes.test.ts src/inference/inference-privacy-receipt-routes.test.ts src/inference/gym/terminal-bench-khala-orchestration.test.ts src/tassadar-settled-feed-sync.test.ts src/khala-sync-public-settled-feed.test.ts src/public-settled-feed-routes.test.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:deploy"
      },
      {
        "command": "node --import tsx scripts/check-deploy-from-main.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:deploy-from-main"
      },
      {
        "command": "node --import tsx scripts/check-effect-native-vendor-freshness.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:effect-native-vendor"
      },
      {
        "command": "node --import tsx scripts/check-effect-topology.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:effect-topology"
      },
      {
        "command": "node --import tsx scripts/check-effect-upgrade-metadata.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:effect-upgrade-metadata"
      },
      {
        "command": "node --import tsx scripts/check-no-github-actions.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:no-github-actions"
      },
      {
        "command": "node --import tsx scripts/check-public-projection-freshness.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "check:public-projection-freshness"
      },
      {
        "command": "pnpm run dev:start",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "dev"
      },
      {
        "command": "pnpm --dir workers/api run dev",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "dev:api"
      },
      {
        "command": "pnpm --dir apps/start run dev",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "dev:start"
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
        "command": "node --import tsx scripts/gym-harbor-full-trace-archive.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "gym:harbor-full-trace-archive"
      },
      {
        "command": "node --import tsx scripts/gym-harbor-progress-push.ts",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "gym:harbor-progress-push"
      },
      {
        "command": "node --import tsx scripts/khala-code-headless-harness.mjs",
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
        "command": "pnpm --dir apps/start run preview",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "preview"
      },
      {
        "command": "node scripts/public-activity-proof-links-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:activity:proof-links"
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
        "command": "node scripts/visibility-browser-smoke.mjs",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "smoke:visibility:browser"
      },
      {
        "command": "pnpm run test:packages && pnpm run test:start && pnpm run test:api",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test"
      },
      {
        "command": "pnpm --dir workers/api run test",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:api"
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
        "command": "pnpm --dir packages/email-templates run test && pnpm --dir packages/mullet-schema run test && pnpm --dir packages/mullet-sim run test && pnpm --dir packages/sync-schema run test && pnpm --dir packages/sync-client run test && pnpm --dir packages/sync-worker run test",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:packages"
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
        "command": "pnpm --dir apps/start run test",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:start"
      },
      {
        "command": "pnpm run typecheck:packages && pnpm run typecheck:start && pnpm run typecheck:api",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck"
      },
      {
        "command": "pnpm --dir workers/api run typecheck",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:api"
      },
      {
        "command": "tsc -p workers/api/tsconfig.pylon-api-routes.test.json --noEmit",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:api-pylon-integration"
      },
      {
        "command": "pnpm --dir packages/email-templates run typecheck && pnpm --dir packages/mullet-schema run typecheck && pnpm --dir packages/mullet-sim run typecheck && pnpm --dir packages/sync-schema run typecheck && pnpm --dir packages/sync-client run typecheck && pnpm --dir packages/sync-worker run typecheck",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:packages"
      },
      {
        "command": "pnpm --dir apps/start run typecheck",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:start"
      },
      {
        "command": "tsc -b",
        "manifest_path": "apps/openagents.com/packages/effect-native-core/package.json",
        "name": "typecheck"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/packages/effect-native-gallery/package.json",
        "name": "typecheck"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/packages/effect-native-khala-ui/package.json",
        "name": "typecheck"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/openagents.com/packages/effect-native-render-canvas/package.json",
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
        "command": "node --import tsx src/preview.ts",
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
        "command": "node --import tsx scripts/agent-readiness-fleet-report-run.ts",
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
        "command": "pnpm --dir ../../apps/start run build && vp pack src/cloudrun/server.ts --out-dir dist-cloudrun --format esm --platform node --target node24",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "build"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh production",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "deploy"
      },
      {
        "command": "bash scripts/deploy-cloudrun.sh staging",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "deploy:staging"
      },
      {
        "command": "node --import tsx src/cloudrun/server.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "dev"
      },
      {
        "command": "node --import tsx scripts/khala-glm-fleet-durability.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "glm-fleet:durability"
      },
      {
        "command": "node --import tsx scripts/marching-orders-agent.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "marching-orders"
      },
      {
        "command": "node --import tsx scripts/khala-glm-nvfp4-pilot.ts",
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
        "command": "node --import tsx scripts/probe-gepa-stage0-no-spend-campaign.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "smoke:probe-gepa-stage0"
      },
      {
        "command": "node --import tsx scripts/qwen-remote-pylon-live-training.ts",
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
        "command": "node --import tsx ./src/cloudrun/server.ts",
        "manifest_path": "apps/openagents.com/workers/api/package.json",
        "name": "start:cloudrun"
      },
      {
        "command": "vp test --root ../../../.. --run --project @openagentsinc/api-worker",
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
        "command": "node --import tsx scripts/check-supervisor-store-bypass.mjs",
        "manifest_path": "apps/pylon/package.json",
        "name": "check:supervisor-store"
      },
      {
        "command": "bash scripts/release-gate.sh",
        "manifest_path": "apps/pylon/package.json",
        "name": "release:gate"
      },
      {
        "command": "node --import tsx packages/runtime/src/cli.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "runtime"
      },
      {
        "command": "pnpm --dir packages/runtime run test",
        "manifest_path": "apps/pylon/package.json",
        "name": "runtime:test"
      },
      {
        "command": "node --import tsx scripts/claude-agent-task-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:claude-agent-task"
      },
      {
        "command": "node --import tsx scripts/claude-owner-local-permission-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:claude-owner-local-permission"
      },
      {
        "command": "node --import tsx scripts/codex-agent-task-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:codex-agent-task"
      },
      {
        "command": "rm -f /tmp/pylon-default-start.log; perl -e 'alarm 3; $ENV{PYLON_DISABLE_OPENCODE_STARTUP}=1; exec @ARGV' node --import tsx src/index.ts > /tmp/pylon-default-start.log 2>&1; code=$?; if [ \"$code\" -ne 142 ] && [ \"$code\" -ne 0 ]; then cat /tmp/pylon-default-start.log; exit \"$code\"; fi; if rg -n 'TypeError|Effect\\.(fork|catchAll)|is not a function|\\[ERROR\\]' /tmp/pylon-default-start.log; then exit 1; fi; printf 'default startup reached persistent mode without startup API errors\\n'",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:default-start"
      },
      {
        "command": "node --import tsx scripts/fleet-run-live-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:fleet-run-live"
      },
      {
        "command": "node --import tsx scripts/fleet-run-sustained-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:fleet-run-sustained"
      },
      {
        "command": "bash scripts/smoke-local-package-install.sh",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:install:local"
      },
      {
        "command": "node --import tsx scripts/live-worker-loop-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:live-worker-loop"
      },
      {
        "command": "node --import tsx scripts/mixed-harness-fleet-run-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:mixed-harness-fleet-run"
      },
      {
        "command": "node --import tsx scripts/packaged-live-network-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:packaged-network"
      },
      {
        "command": "node --import tsx scripts/packaged-runtime-task-smoke.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "smoke:packaged-runtime-task"
      },
      {
        "command": "node --import tsx src/index.ts",
        "manifest_path": "apps/pylon/package.json",
        "name": "start"
      },
      {
        "command": "vp test --run --root ../.. apps/pylon/scripts/typecheck-tests.test.mjs && pnpm run check:supervisor-store && vp test --run --max-concurrency=1 --root ../.. apps/pylon",
        "manifest_path": "apps/pylon/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "apps/pylon/package.json",
        "name": "typecheck"
      },
      {
        "command": "node scripts/typecheck-tests.mjs",
        "manifest_path": "apps/pylon/package.json",
        "name": "typecheck:tests:baseline"
      },
      {
        "command": "node scripts/typecheck-tests.mjs --update-baseline",
        "manifest_path": "apps/pylon/package.json",
        "name": "typecheck:tests:update-baseline"
      },
      {
        "command": "node --import tsx src/cli.ts",
        "manifest_path": "apps/pylon/packages/runtime/package.json",
        "name": "runtime"
      },
      {
        "command": "vp test --run",
        "manifest_path": "apps/pylon/packages/runtime/package.json",
        "name": "test"
      },
      {
        "command": "node --import tsx src/daemon.ts --api",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "api"
      },
      {
        "command": "node --import tsx src/atif-emit.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "atif:emit"
      },
      {
        "command": "node --import tsx scripts/build.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "build"
      },
      {
        "command": "node --import tsx src/codex-to-atif.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "codex:to-atif"
      },
      {
        "command": "node --import tsx src/compose/cli.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "compose"
      },
      {
        "command": "node --import tsx src/byo.ts run --fake-model --url https://example.test --out ./runs/byo-fake",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "demo:byo"
      },
      {
        "command": "node --import tsx src/demo-khala.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "demo:khala"
      },
      {
        "command": "node --import tsx src/demo-login.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "demo:login"
      },
      {
        "command": "node --import tsx src/evals-run.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "evals"
      },
      {
        "command": "node --import tsx src/khala-sync-once.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "khala-sync-once"
      },
      {
        "command": "playwright install --with-deps chromium",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "playwright:install"
      },
      {
        "command": "node --import tsx src/pr-comment-run.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "pr-comment"
      },
      {
        "command": "node --import tsx scripts/build.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "prepack"
      },
      {
        "command": "node --import tsx src/byo.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "qa"
      },
      {
        "command": "node dist/qa.js",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "qa:dist"
      },
      {
        "command": "node --import tsx src/run-once.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "run-once"
      },
      {
        "command": "node --import tsx src/run-targets.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "run-targets"
      },
      {
        "command": "node --import tsx src/daemon.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "serve"
      },
      {
        "command": "node --import tsx src/terminal-once.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "terminal-once"
      },
      {
        "command": "vp test --run src/assurance-swarm.test.ts src/byo-model.test.ts src/byo.test.ts src/runner.test.ts src/runner-hardening.test.ts src/timeouts.test.ts src/shard.test.ts src/public-safety.test.ts src/brain.test.ts src/backend.test.ts src/terminal-backend.test.ts src/khala-sync-transport-backend.test.ts src/container-backend.test.ts src/native-desktop-backend.test.ts src/khala-action.test.ts src/khala-driver.test.ts src/khala-config.test.ts src/khala-openrouter.test.ts src/session-trace.test.ts src/distiller.test.ts src/discovery-regression-lifecycle.test.ts src/skill-candidate.test.ts src/receipt.test.ts src/run-settlement.test.ts src/khala-session.test.ts src/compose/build-plan.test.ts src/compose/ffmpeg.test.ts src/evals.test.ts src/pr-comment.test.ts src/control-auth.test.ts src/artifacts.test.ts src/control.test.ts src/api-server.test.ts src/failure-learning.test.ts src/failure-learning-gepa.test.ts src/target-registry.test.ts src/target-registry-run.test.ts src/target-adapter.test.ts src/qs7-rhys-sales-motion.test.ts src/atif.test.ts src/atif-html.test.ts src/codex-to-atif.test.ts src/redaction.test.ts src/claude-code-to-atif.test.ts src/publish-trace.test.ts src/trace-fixture.test.ts src/publish-trace-e2e.verify.test.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "test"
      },
      {
        "command": "node --import tsx src/trace-fixture.ts",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "trace:fixture"
      },
      {
        "command": "tsc --noEmit -p tsconfig.json",
        "manifest_path": "apps/qa-runner/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "docs/khala/fixtures/artanis-as-a-service-smoke-repo/package.json",
        "name": "test"
      },
      {
        "command": "vp run --concurrency-limit 1 -r build",
        "manifest_path": "package.json",
        "name": "build"
      },
      {
        "command": "pnpm run fmt:check && vp lint --quiet",
        "manifest_path": "package.json",
        "name": "check"
      },
      {
        "command": "pnpm --dir packages/agent-client-protocol run check:generated",
        "manifest_path": "package.json",
        "name": "check:agent-client-protocol"
      },
      {
        "command": "pnpm --dir packages/agent-client-protocol-conformance run check:artifacts",
        "manifest_path": "package.json",
        "name": "check:agent-client-protocol-conformance"
      },
      {
        "command": "pnpm --dir packages/codex-app-server-protocol run check:generated",
        "manifest_path": "package.json",
        "name": "check:codex-app-server-protocol"
      },
      {
        "command": "pnpm run check:google-cloud-authority && pnpm run check:sol-docs && pnpm run test:sol-docs && pnpm --dir apps/openagents.com run check:deploy",
        "manifest_path": "package.json",
        "name": "check:deploy"
      },
      {
        "command": "vp lint --quiet && pnpm run check:agent-client-protocol && pnpm run check:agent-client-protocol-conformance && pnpm run check:codex-app-server-protocol && node scripts/vp1-retired-money-surface-guard.mjs . && node scripts/zero-supported-bun-guard.mjs . && node scripts/google-cloud-authority-guard.mjs",
        "manifest_path": "package.json",
        "name": "check:fast"
      },
      {
        "command": "node scripts/google-cloud-authority-guard.mjs",
        "manifest_path": "package.json",
        "name": "check:google-cloud-authority"
      },
      {
        "command": "node scripts/node-vp-cutover-inventory.mjs --check",
        "manifest_path": "package.json",
        "name": "check:node-vp-freeze"
      },
      {
        "command": "node --import tsx scripts/generate-sol-doc-manifest.ts --check",
        "manifest_path": "package.json",
        "name": "check:sol-doc-manifest"
      },
      {
        "command": "node --import tsx scripts/check-sol-docs.ts",
        "manifest_path": "package.json",
        "name": "check:sol-docs"
      },
      {
        "command": "node scripts/vp1-retired-money-surface-guard.mjs .",
        "manifest_path": "package.json",
        "name": "check:vp1-retirement"
      },
      {
        "command": "node scripts/vp2-node-runtime-guard.mjs",
        "manifest_path": "package.json",
        "name": "check:vp2-node-runtime"
      },
      {
        "command": "node scripts/zero-supported-bun-guard.mjs .",
        "manifest_path": "package.json",
        "name": "check:zero-supported-bun"
      },
      {
        "command": "pnpm --dir apps/aiur run deploy",
        "manifest_path": "package.json",
        "name": "deploy:aiur"
      },
      {
        "command": "pnpm --dir apps/aiur run dev",
        "manifest_path": "package.json",
        "name": "dev:aiur"
      },
      {
        "command": "pnpm --dir apps/forum run dev",
        "manifest_path": "package.json",
        "name": "dev:forum"
      },
      {
        "command": "pnpm --dir apps/openagents-desktop run dev",
        "manifest_path": "package.json",
        "name": "dev:openagents-desktop"
      },
      {
        "command": "pnpm --dir apps/openagents-mobile run dev",
        "manifest_path": "package.json",
        "name": "dev:openagents-mobile"
      },
      {
        "command": "pnpm --dir apps/openagents.com run dev",
        "manifest_path": "package.json",
        "name": "dev:openagents.com"
      },
      {
        "command": "pnpm --dir apps/pylon run start",
        "manifest_path": "package.json",
        "name": "dev:pylon"
      },
      {
        "command": "vp fmt",
        "manifest_path": "package.json",
        "name": "fmt"
      },
      {
        "command": "vp fmt --check package.json pnpm-workspace.yaml vite.config.ts '**/package.json' packages/oxlint-plugin-openagents/src",
        "manifest_path": "package.json",
        "name": "fmt:check"
      },
      {
        "command": "node --import tsx scripts/generate-sol-doc-manifest.ts",
        "manifest_path": "package.json",
        "name": "generate:sol-doc-manifest"
      },
      {
        "command": "vp lint --report-unused-disable-directives",
        "manifest_path": "package.json",
        "name": "lint"
      },
      {
        "command": "node --max-old-space-size=8192 scripts/build-public-cli-artifacts.mjs",
        "manifest_path": "package.json",
        "name": "pack"
      },
      {
        "command": "node --import tsx scripts/ui-velocity-receipt.ts",
        "manifest_path": "package.json",
        "name": "perf:ui-velocity"
      },
      {
        "command": "effect-language-service patch && vp config --no-agent",
        "manifest_path": "package.json",
        "name": "prepare"
      },
      {
        "command": "node --import tsx scripts/qa-nightly-matrix.ts",
        "manifest_path": "package.json",
        "name": "qa:nightly"
      },
      {
        "command": "node --import tsx scripts/qa-observer.ts",
        "manifest_path": "package.json",
        "name": "qa:observer"
      },
      {
        "command": "pnpm --dir apps/openagents-desktop run qa:visual",
        "manifest_path": "package.json",
        "name": "qa:swarm:desktop"
      },
      {
        "command": "node --import tsx scripts/qa-verify.ts",
        "manifest_path": "package.json",
        "name": "qa:verify"
      },
      {
        "command": "node --import tsx scripts/effect-authority-boundary-scan.ts",
        "manifest_path": "package.json",
        "name": "scan:effect-authority-boundaries"
      },
      {
        "command": "vp test --run",
        "manifest_path": "package.json",
        "name": "test"
      },
      {
        "command": "pnpm --dir packages/agent-client-protocol run test",
        "manifest_path": "package.json",
        "name": "test:agent-client-protocol"
      },
      {
        "command": "pnpm --dir packages/agent-client-protocol-conformance run test",
        "manifest_path": "package.json",
        "name": "test:agent-client-protocol-conformance"
      },
      {
        "command": "pnpm --dir packages/agent-readiness run test",
        "manifest_path": "package.json",
        "name": "test:agent-readiness"
      },
      {
        "command": "pnpm --dir packages/agent-runtime-schema run test",
        "manifest_path": "package.json",
        "name": "test:agent-runtime-schema"
      },
      {
        "command": "pnpm --dir packages/agent-stdio-transport run test",
        "manifest_path": "package.json",
        "name": "test:agent-stdio-transport"
      },
      {
        "command": "pnpm --dir packages/ai-sdk-sandbox-local run test",
        "manifest_path": "package.json",
        "name": "test:ai-sdk-sandbox-local"
      },
      {
        "command": "pnpm --dir packages/ai-sdk-sandbox-openagents run test",
        "manifest_path": "package.json",
        "name": "test:ai-sdk-sandbox-openagents"
      },
      {
        "command": "pnpm --dir apps/aiur run test",
        "manifest_path": "package.json",
        "name": "test:aiur"
      },
      {
        "command": "pnpm --dir packages/arbiter-effect run test",
        "manifest_path": "package.json",
        "name": "test:arbiter-effect"
      },
      {
        "command": "pnpm --dir packages/assurance-spec run test",
        "manifest_path": "package.json",
        "name": "test:assurance-spec"
      },
      {
        "command": "pnpm --dir packages/audio-contract run test",
        "manifest_path": "package.json",
        "name": "test:audio-contract"
      },
      {
        "command": "pnpm --dir packages/behavior-contracts run test",
        "manifest_path": "package.json",
        "name": "test:behavior-contracts"
      },
      {
        "command": "pnpm --dir packages/blueprint-contracts run test",
        "manifest_path": "package.json",
        "name": "test:blueprint-contracts"
      },
      {
        "command": "pnpm --dir packages/cloud-contract run test",
        "manifest_path": "package.json",
        "name": "test:cloud-contract"
      },
      {
        "command": "cargo test --workspace",
        "manifest_path": "package.json",
        "name": "test:cloud-crates"
      },
      {
        "command": "pnpm --dir packages/codex-app-server-protocol run test",
        "manifest_path": "package.json",
        "name": "test:codex-app-server-protocol"
      },
      {
        "command": "pnpm --dir packages/composer-state run test",
        "manifest_path": "package.json",
        "name": "test:composer-state"
      },
      {
        "command": "pnpm --dir packages/connector-sidecar run test",
        "manifest_path": "package.json",
        "name": "test:connector-sidecar"
      },
      {
        "command": "pnpm --dir packages/durable-stream run test",
        "manifest_path": "package.json",
        "name": "test:durable-stream"
      },
      {
        "command": "pnpm --dir packages/effect-boundary run test",
        "manifest_path": "package.json",
        "name": "test:effect-boundary"
      },
      {
        "command": "pnpm --dir packages/effect-start run test",
        "manifest_path": "package.json",
        "name": "test:effect-start"
      },
      {
        "command": "pnpm --dir packages/environment-auth run test",
        "manifest_path": "package.json",
        "name": "test:environment-auth"
      },
      {
        "command": "pnpm --dir packages/forge-protocol run test",
        "manifest_path": "package.json",
        "name": "test:forge-protocol"
      },
      {
        "command": "pnpm --dir apps/forum run test",
        "manifest_path": "package.json",
        "name": "test:forum"
      },
      {
        "command": "vp test --run scripts/github-issue-triage.test.ts",
        "manifest_path": "package.json",
        "name": "test:github-issue-triage"
      },
      {
        "command": "pnpm --dir packages/grok-harness run test",
        "manifest_path": "package.json",
        "name": "test:grok-harness"
      },
      {
        "command": "pnpm --dir packages/harness-conformance run test",
        "manifest_path": "package.json",
        "name": "test:harness-conformance"
      },
      {
        "command": "pnpm --dir packages/input-bindings run test",
        "manifest_path": "package.json",
        "name": "test:input-bindings"
      },
      {
        "command": "pnpm --dir packages/khala-ai-sdk-core run test",
        "manifest_path": "package.json",
        "name": "test:khala-ai-sdk-core"
      },
      {
        "command": "pnpm --dir apps/khala-capture run test",
        "manifest_path": "package.json",
        "name": "test:khala-capture"
      },
      {
        "command": "pnpm --dir packages/khala-fleet-intents run test",
        "manifest_path": "package.json",
        "name": "test:khala-fleet-intents"
      },
      {
        "command": "pnpm --dir apps/khala-live-hub run test",
        "manifest_path": "package.json",
        "name": "test:khala-live-hub"
      },
      {
        "command": "pnpm --dir packages/khala-qa-harness run test",
        "manifest_path": "package.json",
        "name": "test:khala-qa-harness"
      },
      {
        "command": "pnpm --dir packages/khala-sync-db-collection run test",
        "manifest_path": "package.json",
        "name": "test:khala-sync-db-collection"
      },
      {
        "command": "vp test --run scripts/validate-khala-sync-runtime-dogfood-evidence.test.ts",
        "manifest_path": "package.json",
        "name": "test:khala-sync-runtime-dogfood-evidence"
      },
      {
        "command": "pnpm --dir packages/khala-tools run test",
        "manifest_path": "package.json",
        "name": "test:khala-tools"
      },
      {
        "command": "pnpm --dir packages/mcp-contract run test",
        "manifest_path": "package.json",
        "name": "test:mcp-contract"
      },
      {
        "command": "pnpm --dir packages/nip90 run test",
        "manifest_path": "package.json",
        "name": "test:nip90"
      },
      {
        "command": "node --test scripts/node-vp-cutover-inventory.test.mjs",
        "manifest_path": "package.json",
        "name": "test:node-vp-inventory"
      },
      {
        "command": "pnpm --dir packages/oa-infra run test",
        "manifest_path": "package.json",
        "name": "test:oa-infra"
      },
      {
        "command": "pnpm --dir apps/oa-queue-worker run test",
        "manifest_path": "package.json",
        "name": "test:oa-queue-worker"
      },
      {
        "command": "pnpm --dir apps/oa-updates run test",
        "manifest_path": "package.json",
        "name": "test:oa-updates"
      },
      {
        "command": "pnpm --dir apps/openagents-audio run test",
        "manifest_path": "package.json",
        "name": "test:openagents-audio"
      },
      {
        "command": "pnpm --dir apps/openagents-desktop run verify",
        "manifest_path": "package.json",
        "name": "test:openagents-desktop"
      },
      {
        "command": "pnpm --dir apps/openagents-mobile run test",
        "manifest_path": "package.json",
        "name": "test:openagents-mobile"
      },
      {
        "command": "pnpm --dir apps/openagents.com run test",
        "manifest_path": "package.json",
        "name": "test:openagents.com"
      },
      {
        "command": "pnpm --dir packages/pipeline-signals run test",
        "manifest_path": "package.json",
        "name": "test:pipeline-signals"
      },
      {
        "command": "pnpm --dir packages/portable-session-contract run test",
        "manifest_path": "package.json",
        "name": "test:portable-session-contract"
      },
      {
        "command": "pnpm --dir packages/probe run test",
        "manifest_path": "package.json",
        "name": "test:probe"
      },
      {
        "command": "pnpm --dir packages/provider-account-schema run test",
        "manifest_path": "package.json",
        "name": "test:provider-account-schema"
      },
      {
        "command": "pnpm --dir packages/public-activity-timeline run test",
        "manifest_path": "package.json",
        "name": "test:public-activity-timeline"
      },
      {
        "command": "pnpm --dir apps/pylon run test",
        "manifest_path": "package.json",
        "name": "test:pylon"
      },
      {
        "command": "pnpm --dir packages/pylon-core run test",
        "manifest_path": "package.json",
        "name": "test:pylon-core"
      },
      {
        "command": "vp test --run scripts/qa-async-gce-trigger.test.ts",
        "manifest_path": "package.json",
        "name": "test:qa-async-gce-trigger"
      },
      {
        "command": "vp test --run scripts/qa-nightly-matrix.test.ts",
        "manifest_path": "package.json",
        "name": "test:qa-nightly-matrix"
      },
      {
        "command": "vp test --run scripts/qa-pre-push-smoke.test.ts",
        "manifest_path": "package.json",
        "name": "test:qa-pre-push-smoke"
      },
      {
        "command": "pnpm --dir apps/qa-runner run test",
        "manifest_path": "package.json",
        "name": "test:qa-runner"
      },
      {
        "command": "vp test --run scripts/qa-visual-smoke-gate.test.ts",
        "manifest_path": "package.json",
        "name": "test:qa-visual-smoke-gate"
      },
      {
        "command": "pnpm --dir packages/reactor-contracts run test",
        "manifest_path": "package.json",
        "name": "test:reactor-contracts"
      },
      {
        "command": "vp test --run scripts/retired-clients-removal.test.ts",
        "manifest_path": "package.json",
        "name": "test:retired-clients"
      },
      {
        "command": "vp test --run scripts/check-sol-docs.test.ts",
        "manifest_path": "package.json",
        "name": "test:sol-docs"
      },
      {
        "command": "pnpm --dir packages/sqlite-runtime run test",
        "manifest_path": "package.json",
        "name": "test:sqlite-runtime"
      },
      {
        "command": "vp test --run scripts/ui-velocity-receipt.test.ts",
        "manifest_path": "package.json",
        "name": "test:ui-velocity-receipt"
      },
      {
        "command": "node --test packages/runtime-platform/src/runtime-platform.node-suite.ts packages/sqlite-runtime/src/node-database.node-suite.ts scripts/public-cli-artifacts.node.test.mjs scripts/vp2-node-runtime-guard.test.mjs scripts/vp2-retained-service.node.test.mjs",
        "manifest_path": "package.json",
        "name": "test:vp2-node"
      },
      {
        "command": "pnpm --dir packages/world-client run test",
        "manifest_path": "package.json",
        "name": "test:world-client"
      },
      {
        "command": "pnpm --dir packages/world-contract run test",
        "manifest_path": "package.json",
        "name": "test:world-contract"
      },
      {
        "command": "node --import tsx scripts/github-issue-triage.ts",
        "manifest_path": "package.json",
        "name": "triage:issues"
      },
      {
        "command": "vp run --concurrency-limit 2 --filter './**' --filter '!./packages/probe' --filter '!./packages/probe/**' typecheck",
        "manifest_path": "package.json",
        "name": "typecheck"
      },
      {
        "command": "pnpm --dir packages/agent-client-protocol run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:agent-client-protocol"
      },
      {
        "command": "pnpm --dir packages/agent-client-protocol-conformance run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:agent-client-protocol-conformance"
      },
      {
        "command": "pnpm --dir packages/agent-readiness run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:agent-readiness"
      },
      {
        "command": "pnpm --dir packages/agent-runtime-schema run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:agent-runtime-schema"
      },
      {
        "command": "pnpm --dir packages/agent-stdio-transport run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:agent-stdio-transport"
      },
      {
        "command": "pnpm --dir packages/ai-sdk-sandbox-local run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:ai-sdk-sandbox-local"
      },
      {
        "command": "pnpm --dir packages/ai-sdk-sandbox-openagents run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:ai-sdk-sandbox-openagents"
      },
      {
        "command": "pnpm --dir packages/arbiter-effect run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:arbiter-effect"
      },
      {
        "command": "pnpm --dir packages/assurance-spec run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:assurance-spec"
      },
      {
        "command": "pnpm --dir packages/audio-contract run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:audio-contract"
      },
      {
        "command": "pnpm --dir packages/behavior-contracts run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:behavior-contracts"
      },
      {
        "command": "pnpm --dir packages/blueprint-contracts run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:blueprint-contracts"
      },
      {
        "command": "pnpm --dir packages/codex-app-server-protocol run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:codex-app-server-protocol"
      },
      {
        "command": "pnpm --dir packages/composer-state run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:composer-state"
      },
      {
        "command": "pnpm --dir packages/connector-sidecar run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:connector-sidecar"
      },
      {
        "command": "pnpm --dir packages/durable-stream run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:durable-stream"
      },
      {
        "command": "pnpm --dir packages/effect-boundary run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:effect-boundary"
      },
      {
        "command": "pnpm --dir packages/effect-start run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:effect-start"
      },
      {
        "command": "pnpm --dir packages/environment-auth run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:environment-auth"
      },
      {
        "command": "pnpm --dir packages/forge-protocol run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:forge-protocol"
      },
      {
        "command": "pnpm --dir apps/forum run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:forum"
      },
      {
        "command": "pnpm --dir packages/grok-harness run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:grok-harness"
      },
      {
        "command": "pnpm --dir packages/harness-conformance run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:harness-conformance"
      },
      {
        "command": "pnpm --dir packages/input-bindings run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:input-bindings"
      },
      {
        "command": "pnpm --dir packages/khala-ai-sdk-core run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-ai-sdk-core"
      },
      {
        "command": "pnpm --dir packages/khala-fleet-intents run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-fleet-intents"
      },
      {
        "command": "pnpm --dir packages/khala-qa-harness run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-qa-harness"
      },
      {
        "command": "pnpm --dir packages/khala-sync-db-collection run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-sync-db-collection"
      },
      {
        "command": "pnpm --dir packages/khala-tools run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:khala-tools"
      },
      {
        "command": "pnpm --dir packages/mcp-contract run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:mcp-contract"
      },
      {
        "command": "pnpm --dir packages/nip90 run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:nip90"
      },
      {
        "command": "pnpm --dir packages/oa-infra run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:oa-infra"
      },
      {
        "command": "pnpm --dir apps/oa-queue-worker run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:oa-queue-worker"
      },
      {
        "command": "pnpm --dir apps/oa-updates run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:oa-updates"
      },
      {
        "command": "pnpm --dir apps/openagents-audio run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:openagents-audio"
      },
      {
        "command": "pnpm --dir packages/pipeline-signals run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:pipeline-signals"
      },
      {
        "command": "pnpm --dir packages/portable-session-contract run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:portable-session-contract"
      },
      {
        "command": "pnpm --dir packages/provider-account-schema run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:provider-account-schema"
      },
      {
        "command": "pnpm --dir packages/public-activity-timeline run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:public-activity-timeline"
      },
      {
        "command": "pnpm --dir packages/reactor-contracts run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:reactor-contracts"
      },
      {
        "command": "pnpm --dir packages/sqlite-runtime run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:sqlite-runtime"
      },
      {
        "command": "pnpm --dir packages/world-client run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:world-client"
      },
      {
        "command": "pnpm --dir packages/world-contract run typecheck",
        "manifest_path": "package.json",
        "name": "typecheck:world-contract"
      },
      {
        "command": "specs/run-tlc.sh",
        "manifest_path": "package.json",
        "name": "verify:tla"
      },
      {
        "command": "node --import tsx scripts/generate-artifacts.ts --check",
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "check:artifacts"
      },
      {
        "command": "node --import tsx scripts/generate-artifacts.ts",
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "generate"
      },
      {
        "command": "node --import tsx scripts/live-probe.ts cursor",
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "live:cursor"
      },
      {
        "command": "node --import tsx scripts/live-probe.ts grok",
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "live:grok"
      },
      {
        "command": "node --import tsx scripts/run-conformance-report.ts",
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "report"
      },
      {
        "command": "vp test --run packages/agent-client-protocol-conformance/src/conformance.test.ts",
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/agent-client-protocol-conformance/package.json",
        "name": "typecheck"
      },
      {
        "command": "node scripts/check-generated.ts",
        "manifest_path": "packages/agent-client-protocol/package.json",
        "name": "check:generated"
      },
      {
        "command": "node scripts/generate.ts",
        "manifest_path": "packages/agent-client-protocol/package.json",
        "name": "generate"
      },
      {
        "command": "vp test --run packages/agent-client-protocol/src/protocol.test.ts packages/agent-client-protocol/src/profiles/profiles.test.ts",
        "manifest_path": "packages/agent-client-protocol/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/agent-client-protocol/package.json",
        "name": "typecheck"
      },
      {
        "command": "node scripts/update-upstream.ts",
        "manifest_path": "packages/agent-client-protocol/package.json",
        "name": "update:upstream"
      },
      {
        "command": "vp test --run packages/agent-client-runtime-bridge/src",
        "manifest_path": "packages/agent-client-runtime-bridge/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/agent-client-runtime-bridge/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --root ../.. --run --project node packages/agent-readiness/src/index.test.ts",
        "manifest_path": "packages/agent-readiness/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/agent-readiness/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/agent-runtime-schema/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/agent-runtime-schema/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run packages/agent-stdio-transport/src/transport.test.ts",
        "manifest_path": "packages/agent-stdio-transport/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/agent-stdio-transport/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/ai-sdk-sandbox-local/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/ai-sdk-sandbox-local/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/ai-sdk-sandbox-openagents/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/ai-sdk-sandbox-openagents/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/arbiter-effect/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/arbiter-effect/package.json",
        "name": "typecheck"
      },
      {
        "command": "node --import tsx scripts/run-mvp-assurance.ts",
        "manifest_path": "packages/assurance-spec/package.json",
        "name": "assure:mvp"
      },
      {
        "command": "node --import tsx scripts/pack-public.ts",
        "manifest_path": "packages/assurance-spec/package.json",
        "name": "pack:public"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/assurance-spec/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/assurance-spec/package.json",
        "name": "typecheck"
      },
      {
        "command": "node --import tsx scripts/verify-distribution.ts",
        "manifest_path": "packages/assurance-spec/package.json",
        "name": "verify:distribution"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/atif/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/atif/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/audio-contract/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/audio-contract/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/autopilot-control-protocol/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/autopilot-control-protocol/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run src/*.test.ts",
        "manifest_path": "packages/behavior-contracts/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/behavior-contracts/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/blueprint-contracts/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/blueprint-contracts/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/cloud-contract/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/cloud-contract/package.json",
        "name": "typecheck"
      },
      {
        "command": "node scripts/check-generated.ts",
        "manifest_path": "packages/codex-app-server-protocol/package.json",
        "name": "check:generated"
      },
      {
        "command": "node scripts/generate-notification-fixtures.ts",
        "manifest_path": "packages/codex-app-server-protocol/package.json",
        "name": "fixtures:generate"
      },
      {
        "command": "node scripts/generate-all.ts",
        "manifest_path": "packages/codex-app-server-protocol/package.json",
        "name": "generate"
      },
      {
        "command": "node scripts/real-binary-smoke.ts",
        "manifest_path": "packages/codex-app-server-protocol/package.json",
        "name": "smoke:binary"
      },
      {
        "command": "vp test --run packages/codex-app-server-protocol/src/protocol.test.ts",
        "manifest_path": "packages/codex-app-server-protocol/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/codex-app-server-protocol/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/composer-state/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/composer-state/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/connector-sidecar/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/connector-sidecar/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/durable-stream/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/durable-stream/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/effect-boundary/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/effect-boundary/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/effect-start/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/effect-start/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/environment-auth/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/environment-auth/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/forge-protocol/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/forge-protocol/package.json",
        "name": "typecheck"
      },
      {
        "command": "node --import tsx scripts/live-acp-smoke.ts",
        "manifest_path": "packages/grok-harness/package.json",
        "name": "live-acp-smoke"
      },
      {
        "command": "node --import tsx scripts/mock-acp-stdio.ts",
        "manifest_path": "packages/grok-harness/package.json",
        "name": "mock-acp"
      },
      {
        "command": "node --import tsx scripts/rl-probe.ts",
        "manifest_path": "packages/grok-harness/package.json",
        "name": "rl-probe"
      },
      {
        "command": "node --import tsx scripts/rl4-worktree-probe.ts",
        "manifest_path": "packages/grok-harness/package.json",
        "name": "rl4-worktree-probe"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/grok-harness/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/grok-harness/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run src/*.test.ts",
        "manifest_path": "packages/harness-conformance/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/harness-conformance/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/input-bindings/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/input-bindings/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/khala-ai-sdk-core/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-ai-sdk-core/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run src/*.test.ts",
        "manifest_path": "packages/khala-fleet-intents/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-fleet-intents/package.json",
        "name": "typecheck"
      },
      {
        "command": "node --import tsx src/lag-profiling-sweep.ts",
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "lag:sweep"
      },
      {
        "command": "node --import tsx src/monkey-night.ts --runs 100 --steps 64",
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "monkey:night"
      },
      {
        "command": "vp test --run src/monkey-explorer.test.ts --test-name-pattern 'bounded fixture smoke'",
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "monkey:smoke"
      },
      {
        "command": "node --import tsx src/architect-coder-judge-live-smoke.ts",
        "manifest_path": "packages/khala-qa-harness/package.json",
        "name": "smoke:architect-coder-judge-live"
      },
      {
        "command": "vp test --run src/*.test.ts",
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
        "command": "vp test --root ../.. --run --project node packages/khala-sync-client/src && node scripts/check-test-import-coverage.mjs",
        "manifest_path": "packages/khala-sync-client/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-sync-client/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/khala-sync-db-collection/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-sync-db-collection/package.json",
        "name": "typecheck"
      },
      {
        "command": "node --import tsx scripts/capture.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "capture"
      },
      {
        "command": "node --import tsx scripts/check-pending-migrations.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "check:pending-migrations"
      },
      {
        "command": "node --import tsx scripts/compact.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "compact"
      },
      {
        "command": "node --import tsx scripts/load-test.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "load-test"
      },
      {
        "command": "node --import tsx scripts/migrate.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "migrate"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "test"
      },
      {
        "command": "vp test --run scripts/check-pending-migrations.test.ts",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "test:pending-migrations-guard"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit && tsc -p tsconfig.port03-production-driver.json --noEmit",
        "manifest_path": "packages/khala-sync-server/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/khala-sync/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-sync/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/khala-tools/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-tools/package.json",
        "name": "typecheck"
      },
      {
        "command": "tsgo -p tsconfig.json --noEmit",
        "manifest_path": "packages/khala-tools/package.json",
        "name": "typecheck:tsgo"
      },
      {
        "command": "effect-tsgo patch",
        "manifest_path": "packages/khala-tools/package.json",
        "name": "typecheck:tsgo:patch"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/mcp-contract/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/mcp-contract/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/nip90/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/nip90/package.json",
        "name": "typecheck"
      },
      {
        "command": "node --import tsx scripts/migrate.ts",
        "manifest_path": "packages/oa-infra/package.json",
        "name": "migrate"
      },
      {
        "command": "vp test --run src",
        "manifest_path": "packages/oa-infra/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/oa-infra/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/oxlint-plugin-openagents/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/oxlint-plugin-openagents/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/pipeline-signals/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/pipeline-signals/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/portable-session-contract/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/portable-session-contract/package.json",
        "name": "typecheck"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/postgres-runtime/package.json",
        "name": "typecheck"
      },
      {
        "command": "pnpm --dir packages/runtime run test",
        "manifest_path": "packages/probe/package.json",
        "name": "test"
      },
      {
        "command": "npm --prefix packages/runtime run typecheck",
        "manifest_path": "packages/probe/package.json",
        "name": "typecheck"
      },
      {
        "command": "node --import tsx src/cli.ts",
        "manifest_path": "packages/probe/packages/runtime/package.json",
        "name": "probe"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/probe/packages/runtime/package.json",
        "name": "test"
      },
      {
        "command": "node scripts/verify-test-typecheck.mjs",
        "manifest_path": "packages/probe/packages/runtime/package.json",
        "name": "typecheck"
      },
      {
        "command": "node scripts/verify-test-typecheck.mjs --update-baseline",
        "manifest_path": "packages/probe/packages/runtime/package.json",
        "name": "typecheck:baseline:update"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/product-spec/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/product-spec/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/provider-account-schema/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/provider-account-schema/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/public-activity-timeline/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/public-activity-timeline/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/pylon-core/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/pylon-core/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/qa-swarm-contract/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/qa-swarm-contract/package.json",
        "name": "typecheck"
      },
      {
        "command": "node --import tsx scripts/dogfood-smoke.ts",
        "manifest_path": "packages/reactor-contracts/package.json",
        "name": "smoke:dogfood"
      },
      {
        "command": "node --import tsx scripts/install-smoke.ts",
        "manifest_path": "packages/reactor-contracts/package.json",
        "name": "smoke:install"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/reactor-contracts/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/reactor-contracts/package.json",
        "name": "typecheck"
      },
      {
        "command": "node --test src/runtime-platform.node-suite.ts",
        "manifest_path": "packages/runtime-platform/package.json",
        "name": "test:node"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/runtime-platform/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run && pnpm run test:node",
        "manifest_path": "packages/sqlite-runtime/package.json",
        "name": "test"
      },
      {
        "command": "node --test src/node-database.node-suite.ts",
        "manifest_path": "packages/sqlite-runtime/package.json",
        "name": "test:node"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/sqlite-runtime/package.json",
        "name": "typecheck"
      },
      {
        "command": "node --import tsx src/replay-cli.ts",
        "manifest_path": "packages/tassadar-executor/package.json",
        "name": "replay"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/tassadar-executor/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/tassadar-executor/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/ui/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/ui/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
        "manifest_path": "packages/world-client/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
        "manifest_path": "packages/world-client/package.json",
        "name": "typecheck"
      },
      {
        "command": "vp test --run",
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
      "repository_candidates_unmapped",
      "repository_dirty"
    ],
    "head": "46ea6c28f04b0fc29df482227a2c8e87b16968c5",
    "inventory_digest": "sha256:d19ef78318d107e718179e64fa76742b1fdccf769a5142d2438956407dfc001f",
    "repository_label": "oa-l6-8901",
    "state": "dirty",
    "tracked_file_count": 9149,
    "tree": "6ed1d223f45c85922724c786081db1ba3d37ed14",
    "truncated": true
  }
}
```

## Obligations

Each criterion receives one incomplete proposed obligation. Missing proof-design fields project as needs_design and prevent admission or execution.

```assurancespec-obligations
[
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-01"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-01-01",
    "source_claim_digest": "sha256:5249b07e544314383070e43f5b8552b924fa4715fe5e32ca7c294ae802e59a1b",
    "source_claim_snapshot": "The composer renders exactly one `Full Auto` toggle\n(`shell-full-auto-toggle`), off by default, with `aria-pressed` reflecting\nstate. No other new screen or review surface ships with this spec.\nProof: `react-composer.test.tsx` \"Full Auto (#8852): renders as an\noff-by-default composer toggle and reports DesktopFullAutoToggled\".",
    "title": "Assure FA-AC-01"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-02"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-02-01",
    "source_claim_digest": "sha256:cb9866496df8854341bd1f37fd970b32ec998cc714b87d18f1b75b433531c4dd",
    "source_claim_snapshot": "A Codex-lane turn started with Full Auto on sends\n`approvalPolicy: \"never\"` on both `thread/start` and `turn/start`, and its\nprompt is prefixed with the Full Auto instruction; an ordinary turn keeps\n`approvalPolicy: \"on-request\"` and an unprefixed prompt.\nProof: `codex-local-runtime.test.ts` \"Full Auto (#8852) forces\napprovalPolicy never and prefixes the turn prompt...\" and \"an ordinary\n(non-Full-Auto) app-server turn keeps approvalPolicy on-request...\".",
    "title": "Assure FA-AC-02"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-03"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-03-01",
    "source_claim_digest": "sha256:13add3dccc6a5307434b19ee3920e3e6bc545427e15b5f7178a6f1a743af136d",
    "source_claim_snapshot": "A completed Full-Auto turn sends `fullAuto: true`\nexactly once from the renderer; the renderer never loops. Continuation is\ndecided in main by `reconcileFullAutoThreads`, called both right after that\nturn completes and once at app startup.\nProof: `shell.test.ts` \"a flagged turn sends fullAuto:true exactly once --\nmain, not the renderer, decides whether to continue\"; `main.ts`'s\n`dispatchCodexLocalTurn` calling `runFullAutoReconciliation()` after a\nsuccessful Full-Auto turn (code-reviewed; main.ts has no direct unit-test\nharness, see Receipts for the isolated-module proof used instead).",
    "title": "Assure FA-AC-03"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-04"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-04-01",
    "source_claim_digest": "sha256:c629cf55289ec3cb3f95bc3dab1527f5d37c1c83cc140663d7786463bed06f90",
    "source_claim_snapshot": "Toggling Full Auto off persists to main immediately\n(`CodexLocalFullAutoSetChannel`), independent of whether a turn is in\nflight, so a toggle-off durably stops the loop even if the app quits before\nthe next turn would have started.\nProof: `shell.test.ts` \"DesktopFullAutoToggled flips the flag and persists\nit to main immediately\"; `full-auto-restart.e2e.test.ts` \"toggling off\nbefore restart durably stops it -- Runtime B never dispatches\".",
    "title": "Assure FA-AC-04"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-05"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-05-01",
    "source_claim_digest": "sha256:da1b0266659d27ba0851e38614447a6334690794861843f6068c51b2fc57c224",
    "source_claim_snapshot": "When Full Auto is off, an ordinary turn sends `fullAuto`\nundefined (not `false`) and never resubmits automatically.\nProof: `shell.test.ts` \"toggled off, an ordinary Codex turn sends fullAuto\nundefined and never resubmits\".",
    "title": "Assure FA-AC-05"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-06"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-06-01",
    "source_claim_digest": "sha256:d170e89e8e52dee7023aa1809393d65ee48fcc60d556617a98328578342286ec",
    "source_claim_snapshot": "A run of 20 consecutive automatic continuations turns Full\nAuto off durably (registry, not renderer state) and appends an explanatory\nsystem note, rather than continuing unbounded -- and this holds even if a\nrestart happens partway through the count. The consecutive-continuation\ncounter resets only when Full Auto is toggled off for that thread; a manual\nsend while the toggle stays on does NOT reset it, and re-enabling an\nalready-enabled thread preserves the count. Since rev 4 the counter\nincrements only on a SUCCESSFUL dispatch: a failed dispatch consumes\nfailure/backoff budget (FA-AC-16), never a cap slot.\nProof: `full-auto-restart.e2e.test.ts` \"a genuinely stuck loop self-disables\nat the continuation cap across restarts, rather than continuing unbounded\"\nand \"failed dispatches never consume cap slots: fail once then succeed ->\ncontinuationCount is exactly 1\"; `full-auto-registry.test.ts`\n\"continuationCount resets ONLY on toggle-off: a manual send leaves it\nunchanged; off-then-on zeroes it\".",
    "title": "Assure FA-AC-06"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-07"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-07-01",
    "source_claim_digest": "sha256:2254ce441c65d583c1a4cbf7940ad440914370d88a5d733b13a633131878a801",
    "source_claim_snapshot": "A thread left enabled with no turn in flight when\nthe app quits resumes its next continuation on its own at the next launch,\nwith no user action beyond the original toggle.\nProof: `full-auto-restart.e2e.test.ts` \"a thread left enabled by Runtime A\nresumes on Runtime B with no manual re-toggle or re-send\".",
    "title": "Assure FA-AC-07"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-08"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-08-01",
    "source_claim_digest": "sha256:cb34dc2b34869318843b7a396111fdaaea0afed8ea1b1a8d5f51138c7bd84524",
    "source_claim_snapshot": "A thread whose turn was still in flight when the\napp quit is left alone by Full Auto reconciliation until existing\ninterrupted-turn recovery resolves it -- Full Auto never races or\nduplicates that recovery.\nProof: `full-auto-restart.e2e.test.ts` \"a thread with a turn still in\nflight at restart is left alone until that turn resolves\"; the real\nwiring sequences `runFullAutoReconciliation()` after `localTurnRecovery`\nresolves, and computes `nonterminalThreadRefs` from the same\n`localTurnJournal.nonterminal()` that recovery itself owns.",
    "title": "Assure FA-AC-08"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-09"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-09-01",
    "source_claim_digest": "sha256:e0928b54f48543be4384febe0697ca5995463691bab1060e1537e2bf3b7406d2",
    "source_claim_snapshot": "A brand new thread (no id yet when the user\ntoggles Full Auto on) persists its enabled state to main once it actually\ngets a real thread id, rather than silently dropping the toggle's intent.\nProof: `shell.test.ts` \"a brand new thread persists its enabled state to\nmain once it has a real id\".",
    "title": "Assure FA-AC-09"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-10"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-10-01",
    "source_claim_digest": "sha256:d1daf49b0dffa4a3db43527a9d08ceac7b8c450c367d16f34c3d54fa6b5b66f7",
    "source_claim_snapshot": "No Full Auto packet performs a direct commit, merge, or push;\nCodex proposes changes exactly as every other Desktop Codex turn already\ndoes. (Unchanged existing boundary; no new authority was added.)",
    "title": "Assure FA-AC-10"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-11"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-11-01",
    "source_claim_digest": "sha256:c4801546b6e401d8e1f127f2d3bb0c94521df237aab304b8f0f84f9b5c17e6ca",
    "source_claim_snapshot": "A corrupt or schema-invalid registry file never\nblocks Desktop main initialization. Opening it fails closed for the feature\nand open for the app: the bad file is quarantined beside the registry\n(best-effort rename to `registry.json.quarantined-<ISO timestamp>` with an\nowner-visible console diagnostic naming the quarantine path), the registry\nstarts empty (Full Auto disabled for all threads), and subsequent writes\npersist normally.\nProof: `full-auto-registry.test.ts` \"a corrupt registry file is quarantined\nand the registry opens empty instead of throwing\" and \"a schema-invalid (but\nvalid JSON) registry file is also quarantined rather than thrown\".",
    "title": "Assure FA-AC-11"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-12"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-12-01",
    "source_claim_digest": "sha256:27450f70ca5382f43098d7df09b20a74f7ecd2370302593d44cce1f89f8a9564",
    "source_claim_snapshot": "Registry record eviction never drops an\n`enabled: true` record. All enabled records are kept; only the disabled tail\nis bounded, filling remaining capacity (up to 128 total) with the\nmost-recently-updated disabled records. An owner-enabled thread therefore\nalways survives to the next restart, no matter how many other records were\ntouched more recently.\nProof: `full-auto-registry.test.ts` \"eviction never drops an enabled record:\nthe oldest enabled thread survives while old disabled records are evicted\".",
    "title": "Assure FA-AC-12"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-13"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-13-01",
    "source_claim_digest": "sha256:0c0fc5b541489fe29a468ba48824663fad47343b2d0e61639f918b8e0d80915a",
    "source_claim_snapshot": "Enabling Full Auto binds the currently resolved workspace onto\nthe durable record -- resolved by main from the exact same source of truth\ncodex-local turns execute against, never a renderer-supplied path. A\ncontinuation whose currently-resolved workspace differs from the recorded\nbinding does NOT dispatch: the record is disabled with\n`blockedReason: \"workspace_mismatch\"` and an owner-visible system note\nexplains that Full Auto was turned off because the granted workspace no\nlonger matches.\nProof: `full-auto-restart.e2e.test.ts` \"enable on workspace A, resolve\nworkspace B at reconcile -> no dispatch, record disabled with\nworkspace_mismatch, block reported\"; `main.ts` binds via\n`resolveDesktopLocalWorkspaceRoot()` in the `CodexLocalFullAutoSetChannel`\nhandler and passes the same resolver into reconciliation (code-reviewed;\nmain.ts has no direct unit-test harness).",
    "title": "Assure FA-AC-13"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-14"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-14-01",
    "source_claim_digest": "sha256:d2d82d33d573b2bc82414f00aaa1866434b3f2c15b624f36da6cdc1eb9bfcb36",
    "source_claim_snapshot": "An enabled record with NO recorded workspace (a pre-upgrade v1\nrow) fails CLOSED at dispatch: it is never silently adopted onto the current\nworkspace -- the record is disabled with\n`blockedReason: \"workspace_unbound\"` and an owner-visible note. The binding\nis (re)established only by a successful ENABLE, which always records the\nthen-current workspace.\nProof: `full-auto-restart.e2e.test.ts` \"an enabled record with NO workspace\nbinding (pre-upgrade v1 row) fails CLOSED: no dispatch, disabled with\nworkspace_unbound\".",
    "title": "Assure FA-AC-14"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-15"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-15-01",
    "source_claim_digest": "sha256:fa5c6e534d0a1fed8ff7e5ef7b6b9970b28b6442bfedbc401f5691fe09a115ff",
    "source_claim_snapshot": "Continuation dispatch is exactly-once. All reconciliation\ntriggers in main serialize through a promise-chain mutex, and before\ndispatching a thread the reconciler durably claims a per-thread lease\ncarrying the exact continuation turn ref (the lease identity and the\ndispatched turn identity are the same value). Two overlapping reconcile\npasses dispatch an enabled thread at most once. The lease releases on\ndispatch completion (success or failure). Only the STARTUP pass clears a\nstale lease -- one whose turn ref has no nonterminal local-turn journal row\n(a dispatch that crashed before its turn was accepted); a mid-session pass\ntreats a held lease as in-flight and skips. As defense in depth, main's\ndispatch adapter refuses to start a continuation when the local-turn\njournal already holds a nonterminal turn on that thread.\nProof: `full-auto-restart.e2e.test.ts` \"audit probe (a): two overlapping\nreconcile passes against one enabled thread dispatch it exactly ONCE\n(durable lease), and continuationCount increments by exactly 1\", \"the\nserial task queue serializes overlapping reconciliation triggers...\", \"a\nstale lease (crashed mid-dispatch: no journal row for its turn ref) is\ncleared ONLY by the startup pass...\", and \"a lease whose turn IS still\nnonterminal in the journal is NOT cleared at startup...\";\n`full-auto-registry.test.ts` \"claimPending holds the lease exactly once\nuntil cleared; a missing record can never be claimed\".",
    "title": "Assure FA-AC-15"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-16"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-16-01",
    "source_claim_digest": "sha256:12427f5bc832f0d4b1a77615435d03c066a911354893db08f228bec1aeccaa3e",
    "source_claim_snapshot": "A failed continuation dispatch -- thrown OR `{ ok: false }` --\nis a typed, owner-visible outcome, never a silently dormant enabled record.\nFailure persists `consecutiveFailures`, `lastFailureAt`, and a bounded\n`blockedReason` on the record, releases the lease, and appends an\nowner-visible system note. Retries respect bounded exponential backoff:\ndispatch is skipped while the record is within\n`min(2^consecutiveFailures * 30s, 30min)` of `lastFailureAt`. The 5th\nconsecutive failure disables the record durably (with the failure reason as\n`blockedReason`) and a final note says so. A successful dispatch clears all\nfailure state.\nProof: `full-auto-restart.e2e.test.ts` \"audit probe (b): an { ok: false }\ndispatch is a typed, visible failure...\", \"a thrown dispatch is the same\ntyped failure outcome as ok:false\", \"the bounded backoff window skips\ndispatch after a failure, then allows it once the window has passed\", and\n\"the 5th consecutive failure disables the record with a blockedReason and\nreports disabled: true\"; `full-auto-registry.test.ts` \"recordFailure\nincrements and stamps typed failure state (releasing the lease);\nrecordSuccess clears all of it\".",
    "title": "Assure FA-AC-16"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-17"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-17-01",
    "source_claim_digest": "sha256:fad71b282b1060216a6e6d231714dcc37bc290b15534152555660bd5450db41c",
    "source_claim_snapshot": "Automatic continuations preserve the initiating turn's\nexecution profile. When a renderer-initiated turn carries\n`fullAuto: true`, main binds its effective account target, model, and\nreasoning effort onto the durable record; every continuation (including a\npost-restart resume) replays that bound profile, revalidated against the\nlive contract enums (a field that no longer decodes falls back to lane\ndefaults instead of failing the loop). Fields that deliberately RESET on a\ncontinuation: images, explicit context attachments, and extension\nselection -- a continuation is a fresh instruction, not a replay of the\ninitiating turn's payload.\nProof: `full-auto-restart.e2e.test.ts` \"a continuation dispatch carries the\nprofile bound by the initiating flagged turn (account, model, effort) --\nincluding across a restart\" and \"decodeCodexLocalContinuationProfile\nrevalidates stored strings against the live contract...\".",
    "title": "Assure FA-AC-17"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-18"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-18-01",
    "source_claim_digest": "sha256:1a81b9e9b1821120f34aa3c4ddcbf11bbc2992e916e3718b2f4e9898a9d05ed8",
    "source_claim_snapshot": "The wave-2 registry schema upgrade is strictly additive: every\nnew record field (workspace binding, profile, lease, failure state) is\noptional, and an existing v1 registry file decodes without quarantine so no\nuser's enabled state is lost by upgrading.\nProof: `full-auto-registry.test.ts` \"an existing v1 registry file (no\nwave-2 fields) still decodes -- the schema upgrade never quarantines a\nuser's state\".",
    "title": "Assure FA-AC-18"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-19"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-19-01",
    "source_claim_digest": "sha256:9fe946aaddb3c1c7217a2d42784eaefaaaba48f6230ff55ebf32135fc4fd7d37",
    "source_claim_snapshot": "A background (main-initiated) continuation is rendered as a\ncoarse, typed, per-thread in-flight state, not silence until completion.\nMain owns an in-memory live-state map (idle | turn_running |\nturn_completed | turn_failed | cap_reached | blocked; blocked carries the\ntyped blockedReason as bounded detail) and broadcasts every transition to\nall windows over `CodexLocalFullAutoStateChannel`: turn_running with the\nlease turn ref at dispatch start, turn_completed on success, turn_failed\nwith the typed reason on an ordinary failure, cap_reached at the cap, and\nblocked on a workspace or failure-limit disable. Terminal states persist\nuntil the next transition. The extended get channel additively returns\n`{ state, turnRef }` beside `enabled`, and while the active thread's state\nis turn_running the composer renders a \"Full Auto running…\" status badge.\nToken-by-token streaming remains deliberately out of scope.\nProof: `shell.test.ts` \"FA-H4 (#8877): withFullAutoLiveState projects a\nlive-state event per thread and activeFullAutoTurnRunning reads only the\nACTIVE thread\"; `react-composer.test.tsx` \"FA-H4 (#8877): a running\nbackground Full Auto turn renders the status badge and the Stop\naffordance; idle renders neither\"; `main.ts` wires the transitions around\nthe existing `runFullAutoReconciliation` dispatch adapter and callbacks\n(code-reviewed; main.ts has no direct unit-test harness).",
    "title": "Assure FA-AC-19"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-20"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-20-01",
    "source_claim_digest": "sha256:e5bb0ecede90a05283555057a9d8d8b5197ce004bdbb0836e6294e1d87bf2887",
    "source_claim_snapshot": "A working stop targets the ACTUAL background turn. While the\nactive thread's live state is turn_running (renderer non-pending), the\ncomposer's Stop control dispatches the same interrupt intent, whose\nhandler calls the thread-scoped `CodexLocalFullAutoInterruptChannel` with\nonly `{ threadRef }`; main resolves the live running turn ref itself and\nsignals the exact same `codexLocal.interrupt` runtime path the existing\nturn-interrupt channel uses, answering `{ ok: boolean }`. While the\nrenderer's OWN turn is pending, Stop keeps signalling the active streaming\nturn unchanged. The interrupted background turn terminates through the\nexisting FA-H5 typed-failure path; the toggle remains the durable\nloop-level stop.\nProof: `shell.test.ts` \"FA-H4 (#8877): DesktopTurnInterrupted with a\nrunning BACKGROUND turn (not pending) calls fullAutoHost.interrupt with\nthe active threadRef\" and \"FA-H4 (#8877): while renderer-pending, Stop\nkeeps signalling the ACTIVE streaming turn (chat.interruptActive), not the\nbackground channel\"; `react-composer.test.tsx` (Stop affordance case\nabove); `main.ts` interrupt handler (code-reviewed).",
    "title": "Assure FA-AC-20"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-21"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-21-01",
    "source_claim_digest": "sha256:96242f50d322c4619643e0da9d9d5496f93d97bcdc52f85239c3bef7b0b8e743",
    "source_claim_snapshot": "A manual send while a background Full Auto turn owns the\nthread is excluded, never run silently concurrently. When the active\nthread's live state is turn_running, `runNoteSubmission` refuses to start\na manual turn: it sets the transient notice \"Full Auto is running a turn\non this thread. Stop it first or wait for it to finish.\" and keeps the\ncomposer draft. Once the live state is terminal, the same submit goes\nthrough normally.\nProof: `shell.test.ts` \"FA-H4 (#8877): a manual send while a background\nFull Auto turn runs is FENCED -- sendMessage is never called, a notice\nsays why, and the draft is kept\".",
    "title": "Assure FA-AC-21"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-22"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-22-01",
    "source_claim_digest": "sha256:0f0f2018b26d60b1402371861ad8755b89b17097b98bf7eff2e9d9387997b89a",
    "source_claim_snapshot": "The programmatic control surface is opt-in and off by\ndefault, loopback-only, and bearer-gated. Desktop main constructs the\ncontrol server ONLY when `OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1`; the\nlistener binds 127.0.0.1 exclusively (ephemeral or env-pinned port); every\nrequest -- the OpenAPI document included -- requires the per-process scoped\nbearer credential (scopes drawn from `@openagentsinc/environment-auth`'s\nnarrowing-only exchange, verified with a constant-time comparison) or is\nrefused 401. Connection info is written mode-0600 to\n`full-auto/control.json` under userData and removed on stop.\nProof: `full-auto-control-server.test.ts` \"off by default: main's guard\nrequires OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1 exactly\", \"credential mint\nuses the environment-auth narrowing-only exchange...\", \"auth: no bearer and\na wrong bearer are 401 on every route...\", and \"the connection file is\nwritten mode 0600...\"; `main.ts` wraps the entire server wiring in\n`isFullAutoControlEnabled(process.env)` (code-reviewed; main.ts has no\ndirect unit-test harness -- the guard function itself is the tested unit).",
    "title": "Assure FA-AC-22"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-23"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-23-01",
    "source_claim_digest": "sha256:8e0ec08187c8ca2db29385a7622cd9af60cc26aa5313af4173bcfd6568a1f62c",
    "source_claim_snapshot": "Programmatic enable NAMES the workspace the caller expects\nand enforces it: the request body requires `workspaceRef`, the server\nresolves the current workspace itself via the same\n`resolveDesktopLocalWorkspaceRoot` codex-local turns execute against, and\nany difference is a 409 `workspace_mismatch` refusal with the registry left\nuntouched -- never a silent redirect. Programmatic enable can never grant a\nnew, previously-ungranted workspace; on success it binds exactly the\nresolved workspace, the same path as the IPC set handler.\nProof: `full-auto-control-server.test.ts` \"enable with a mismatched\nworkspaceRef is a 409 typed refusal and the registry is untouched\" and\n\"enable with the matching workspaceRef enables + binds the record...\".",
    "title": "Assure FA-AC-23"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-24"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-24-01",
    "source_claim_digest": "sha256:30a72566ec674ecb4397f6f1088a73fad8e68d8e024065fccc28cc46a485442b",
    "source_claim_snapshot": "Every mutating control-API call (enable, disable,\ncontinue-now) appends a durable, distinctly-attributed system note to the\nthread through the existing `appendFullAutoSystemNote` (naming the\nprogrammatic path and caller `control-api`), plus a public-safe console\naudit line, so the owner can always tell a programmatic action from their\nown click.\nProof: `full-auto-control-server.test.ts` attribution assertions inside the\nenable, disable, and continue-now cases (note text contains \"programmatically\"\nand \"control-api\").",
    "title": "Assure FA-AC-24"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-25"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-25-01",
    "source_claim_digest": "sha256:7abfea21c68f5feefe7da620209c18ddc291b890cb51b133c2a8843d2f7340a8",
    "source_claim_snapshot": "continue-now is a new TRIGGER into the shared serialized\nreconciliation path, never a new dispatch mechanism: the handler invokes\nthe exact injected reconciliation trigger (main passes\n`runFullAutoReconciliation`, the same FA-H3 promise-chain mutex + durable\nlease every other trigger point uses) exactly once and returns\n`{ scheduled: true }` immediately; dispatch remains subject to lease,\nworkspace binding, backoff, and cap policy. An unknown threadRef is 404 and\nnever touches the trigger.\nProof: `full-auto-control-server.test.ts` \"continue-now invokes the\ninjected reconcile trigger exactly once and returns { scheduled: true }\"\n(spy on the injected trigger) and \"continue-now on an unknown threadRef is\na 404 and never touches the trigger\"; `main.ts` passes\n`() => runFullAutoReconciliation()` as that capability (code-reviewed).",
    "title": "Assure FA-AC-25"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-26"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-26-01",
    "source_claim_digest": "sha256:d841490075cc6fc71f4f7368ad6244f539f0f77f395e28cd13cf4a013f46e0af",
    "source_claim_snapshot": "The served surface and the published OpenAPI 3.1 document\ncannot drift: `GET /v1/openapi.json` serves the hand-authored document, and\na structural parity test asserts every route in the shared\n`FULL_AUTO_CONTROL_ROUTES` table appears in the document (path, method,\noperationId) AND every operation in the document is a served route.\nResponse bodies decode against the Effect Schemas in\n`full-auto-control-contract.ts`, whose bounds mirror the IPC contract.\nProjections stay public-safe: records expose only\nthreadRef/enabled/continuationCount/updatedAt/workspaceRef/blockedReason/\nlive state plus accountRef (never model/effort/raw profile material), and\nturns expose identity/phase/disposition/timestamps for at most the last 20\nFull Auto turns -- never transcript text.\nProof: `full-auto-control-server.test.ts` \"GET /v1/openapi.json serves the\ndocument, and the document <-> served routes agree in both directions\",\n\"list and status match the contract schemas... expose no profile material\nbeyond accountRef\", and \"turns returns a bounded, most-recent-first Full\nAuto projection with no transcript text\".",
    "title": "Assure FA-AC-26"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-27"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-27-01",
    "source_claim_digest": "sha256:a379ac228a58d621c4498bc9f579070a44fe338d59d598b45551ef863bac629c",
    "source_claim_snapshot": "The MCP server and CLI are thin pass-through clients of the\none control surface: both discover the server from `full-auto/control.json`\n(with `--user-data` / `OPENAGENTS_DESKTOP_USER_DATA` overrides), attach the\nbearer, call the HTTP API, and return the server's JSON verbatim -- no\nclient-side policy and no second schema vocabulary. Both fail with a clear\n\"server not enabled\" message when the connection file is missing. The MCP\nserver exposes `full_auto_list` / `full_auto_status` / `full_auto_enable` /\n`full_auto_disable` / `full_auto_continue_now` / `full_auto_turns` over the\nrepo's public MCP protocol revision (2025-06-18).\nProof: `scripts/full-auto-cli.ts` and `scripts/full-auto-mcp.ts`\n(pass-through by construction over the shared\n`scripts/full-auto-control-client.ts`); live end-to-end receipt in the\nrev 6 entry under Receipts (`pnpm run smoke:full-auto-control` exercises\nthe real CLI as a second OS process against the real running Electron\nmain).",
    "title": "Assure FA-AC-27"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-28"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-28-01",
    "source_claim_digest": "sha256:bfadbd7c3bc0f39bdda56240e27fd763503abb7e944c2e60d6ade253ef6efa6a",
    "source_claim_snapshot": "The control surface can BOOTSTRAP Full Auto with no existing\nthread: `POST /v1/full-auto/start` (OpenAPI `startFullAuto`, MCP\n`full_auto_start`, CLI `start --workspace <path> [--title <t>]`) mints a\nbrand-new local thread in main's own thread store (main names the ref --\nthe caller never supplies one), binds the resolved workspace, enables the\nrecord through the same `registry.set` path as the composer toggle,\nappends the distinctly-attributed `(caller: control-api)` system note, and\nschedules the shared serialized reconcile pass so the first continuation\ndispatches without a separate continue-now call -- the reconcile\ndispatcher then opens a brand-new provider conversation because the\nminted thread has no session continuity. start obeys the exact enable\nauthority rule: the caller MUST name the workspace it expects, and on any\ndifference from the currently resolved workspace the call refuses with\n409 `workspace_mismatch` with NO thread minted, NO record written, and NO\nnote appended -- never a redirect, never a new grant.\nProof: `src/full-auto-control-server.test.ts` (\"start with the matching\nworkspaceRef mints a thread...\", \"start with a mismatched workspaceRef is\na 409 typed refusal: NO thread minted...\", \"start discipline: bodyless\nstart is 400...\", plus the doc <-> route parity test covering\n`startFullAuto`).",
    "title": "Assure FA-AC-28"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-29"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-29-01",
    "source_claim_digest": "sha256:95a4f3578438a53c7032f9e110b29e4202cb40695533f6792bbb6b092a360945",
    "source_claim_snapshot": "The durable execution profile carries an optional ProviderLane\nref. A rev-7 registry row with no lane still decodes and continues on\n`codex-local`; a selected `fable-local` row survives a Runtime A → Runtime B\nreopen and reaches the shared dispatch seam with the same lane/account/model.\nProof: `full-auto-restart.e2e.test.ts` \"a Claude lane selection survives\nRuntime A -> Runtime B...\" plus the retained legacy-file registry tests;\n`pnpm run smoke:full-auto-restart` launches real Electron OS processes for\n`seed-claude` → `resume-claude` and receipts `dispatchedLane:fable-local`.",
    "title": "Assure FA-AC-29"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-30"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-30-01",
    "source_claim_digest": "sha256:a9b55a40165284d3dce1b35a96082a6a59d8fd6ad9f9ce15c77b73050a65fdcd",
    "source_claim_snapshot": "Reconciliation dispatches through the L1 ProviderLane SPI and\nfails closed for any lane that is unknown, L2-quarantined, does not advertise\nFull Auto, or lacks safe background-question settlement. Workspace binding,\nexactly-once lease, backoff, cap, and attribution behavior are unchanged.\nProof: `main.ts` lane selection + `projectProviderLaneCapabilities` gate;\nfocused Full Auto regression suites.",
    "title": "Assure FA-AC-30"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-31"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-31-01",
    "source_claim_digest": "sha256:4c079b7e3e08a8adad86a2918fff6937a69a26010d2306c4e3bcf404bf155219",
    "source_claim_snapshot": "Codex and Claude Full Auto turns use the single lane-keyed\ninstruction policy. A background Claude `AskUserQuestion` never parks: it is\ndenied immediately with guidance to make a reasonable judgment and proceed,\nwhile an interactive ordinary Claude turn retains the existing real question\nUI flow.\nProof: `fable-local-runtime.test.ts` \"background Full Auto denies\nAskUserQuestion immediately...\" and the retained interactive question tests.",
    "title": "Assure FA-AC-31"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-32"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-32-01",
    "source_claim_digest": "sha256:4f022a210eada46d961d2fb53d39763565d354d6465328d5cda3bb511253e1cd",
    "source_claim_snapshot": "`start` and `enable` accept an optional lane ref (default\n`codex-local`) through the shared control contract, served OpenAPI document,\nMCP tools, and CLI `--lane`; status/list expose the public-safe selected lane.\nAn ineligible lane returns typed 409 `lane_not_eligible` without mutating the\nregistry.\nProof: `full-auto-control-server.test.ts` \"enable accepts an admitted lane\nselector...\" plus document/route/schema parity.",
    "title": "Assure FA-AC-32"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "FA-AC-33"
    ],
    "disposition": "required",
    "id": "AO-FA-AC-33-01",
    "source_claim_digest": "sha256:d13b2fc55b4284f1e5b47327c615a43b87c98d3c71736701129b26fdd82c9b96",
    "source_claim_snapshot": "A real bounded Claude Code Full Auto run must be retained as a\nrelease receipt. ACP peer proof remains conditional on #8893/#8894 admission\nand must not be inferred from fixture coverage.\nProof: owner/dogfood receipt linked from #8901; until captured this criterion\nremains an explicit residual, not a release claim.",
    "title": "Assure FA-AC-33"
  }
]
```

## Gates

No execution or release gates are inferred. Gate design remains blocked pending review.

```assurancespec-gates
[]
```

## Evidence Policy

Links are pointers, not verdicts. Missing or unreviewed evidence remains INCONCLUSIVE.

```assurancespec-evidence-policy
{
  "links_are_verdicts": false,
  "missing_evidence_verdict": "INCONCLUSIVE",
  "policy_state": "needs_design",
  "required_for_ready_obligation": [
    "oracle_observation",
    "falsifier_observation",
    "environment_binding",
    "independent_review"
  ]
}
```

## Authority Boundaries

This proposal cannot admit, execute, verify, waive, release, or change a public promise.

```assurancespec-authority
{
  "admitted_roles": [],
  "policy_state": "needs_design",
  "proposal_may_change_public_promises": false,
  "proposal_may_execute": false,
  "proposal_may_release": false,
  "proposal_may_self_admit": false,
  "proposal_may_verify": false,
  "release_roles": [],
  "verifier_roles": []
}
```
