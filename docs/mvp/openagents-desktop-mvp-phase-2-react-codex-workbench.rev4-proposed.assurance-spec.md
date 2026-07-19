---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.openagents.desktop.react-codex-workbench.phase2"
assurance_revision: 4
title: "OpenAgents Desktop React Codex Workbench Phase 2 AssuranceSpec"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "OpenAgents"
---

## Assurance Objective

This proposed AssuranceSpec creates exact criterion-to-obligation coverage for
the React workbench, including the post-RC16 launch/workspace, compact shell,
image input, packaged Codex custody, event-cadence, and dual-control changes.
The detailed oracle and falsifier requirements live in the companion revision-4
assurance gates and expected-working-surface matrix. This document does not
claim that execution, evidence, admission, release, or publication is complete.

## Subject

The proposal is bound to the exact ProductSpec bytes, revision, path, and stable criterion identifiers below.

```assurancespec-subject
{
  "product_spec": {
    "criterion_refs": [
      "AC-1",
      "AC-2",
      "AC-3",
      "AC-4",
      "AC-5",
      "AC-6",
      "AC-7",
      "AC-8",
      "AC-9",
      "AC-10",
      "AC-11",
      "AC-12",
      "AC-13",
      "AC-14",
      "AC-15",
      "AC-16",
      "AC-17",
      "AC-18",
      "AC-19",
      "AC-20"
    ],
    "document_digest": "sha256:c9f0863e94d2ac6bb60b67a9de61cad70d27d734a6977b0d3556d48fea05c3e5",
    "path": "docs/mvp/openagents-desktop-mvp-phase-2-react-codex-workbench.product-spec.md",
    "profile": "openagents_executable_v0.1_exact_document",
    "spec_format_version": "0.1",
    "spec_revision": 4
  }
}
```

## Risk Model

The revision models control congruence, renderer event storms, workspace and
attachment authority, and packaged runtime/privacy as distinct risks. Older
Phase 2 criteria retain their generated proposal status pending a separate
full-proof-design review.

```assurancespec-risks
{
  "risks": [
    {
      "id": "RISK-UI-CONTROL-CONGRUENCE",
      "statement": "A visible control can drift from its typed intent, become inaccessible to keyboard or pointer input, or require DOM/test-only mutation that external Computer Use cannot reproduce."
    },
    {
      "id": "RISK-RENDERER-EVENT-STORM",
      "statement": "Provider, PTY, filesystem, or React projection work can publish one state transition per source event, grow queues or memory without bound, and make typing or navigation visibly lag."
    },
    {
      "id": "RISK-WORKSPACE-AND-ATTACHMENT-AUTHORITY",
      "statement": "Workspace switching or image acquisition can expose arbitrary paths or bytes, split WorkContext, lose retry state, or mutate context after cancellation."
    },
    {
      "id": "RISK-RUNTIME-CUSTODY-AND-PRIVACY",
      "statement": "Desktop can accidentally discover or mutate an ambient Codex install, expose sensitive identity by default, or report configuration/update success before exact verification."
    }
  ],
  "source_digest": "sha256:8344962d1bb6a01b325c6b9c3e2ea8b58f3974497627a7a5906c0830fae16a65",
  "source_snapshot": "- A quick React surface can become a second application architecture. The\n  ownership ledger, import scanner, snapshot contract, and duplicate-intent\n  tests are release gates, not cleanup work.\n- A compatibility island can survive indefinitely and leave two browser\n  lifecycles. Each converted surface needs an explicit backend disposition,\n  and the same workbench may never mount through both.\n- Component-local state can drift from Effect truth during streaming or\n  restart. Durable or cross-surface state is forbidden in React.\n- Tailwind utilities can silently fork tokens or accessibility states. Every\n  color, spacing, type, radius, and state value must resolve through the\n  canonical theme mapping or an explicitly reviewed renderer mechanic.\n- A third-party overlay primitive can alter focus, portal, or dismissal\n  behavior across versions. Pin the version, wrap it behind typed nodes, and\n  keep interaction fixtures at the wrapper boundary.\n- A polished transcript can conceal missing or reordered events. Stable key,\n  gap, prepend, restart, and terminal-disposition oracles remain mandatory.\n- Narrowing the primary journey can accidentally delete Phase 1 specialist\n  capabilities. Phase 2 removes them from its completion path, not from stored\n  state or historical proof; any removal requires separate intent and gates.\n- The reference application may encourage uncritical dependency adoption.\n  Every dependency remains excluded until a scoped failure and boundary review\n  justify it.\n- Performance work can trade away correctness by coalescing distinct typed\n  events. Causal item identity and exact terminal state are never optimization\n  variables."
}
```

## Assurance Scope

Every executable ProductSpec criterion is in assurance scope. No criterion is silently excluded or marked not applicable.

## Environments

Repository facts are proposal context only. The post-RC16 obligations target a
proposed macOS ARM64 Node 24 Desktop profile. Admission must pin exact OS,
hardware, artifact, adapter, permission, and isolation details before execution.

```assurancespec-environments
{
  "profiles": [
    {
      "id": "ENV-OA-DESKTOP-PHASE2-NODE24-MACOS-ARM64",
      "status": "proposed"
    }
  ],
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
      "apps/openagents-desktop/src/codex-child-runtime.test.ts",
      "apps/openagents-desktop/src/codex-config-health.test.ts",
      "apps/openagents-desktop/src/codex-connection-signatures.test.ts",
      "apps/openagents-desktop/src/codex-handoff-host.test.ts",
      "apps/openagents-desktop/src/codex-handoff-integration.test.ts",
      "apps/openagents-desktop/src/codex-handoff.test.ts",
      "apps/openagents-desktop/src/codex-history-host.test.ts",
      "apps/openagents-desktop/src/codex-history-utility.test.ts",
      "apps/openagents-desktop/src/codex-local-runtime.test.ts",
      "apps/openagents-desktop/src/codex-preflight.test.ts",
      "apps/openagents-desktop/src/codex-release-notes.test.ts",
      "apps/openagents-desktop/src/desktop-host-lifecycle.test.ts",
      "apps/openagents-desktop/src/desktop-launch-workspace.test.ts",
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
      "apps/openagents-desktop/src/mvp-assurance-bridge.test.ts",
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "apps/openagents-desktop/src/mvp-proof.test.ts",
      "apps/openagents-desktop/src/product-spec-app-server-tools.test.ts",
      "apps/openagents-desktop/src/product-spec-workroom.test.ts",
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
      "apps/openagents.com/apps/docs/infra-contract.test.mjs",
      "apps/openagents.com/apps/docs/server.test.mjs",
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
      "apps/openagents.com/workers/api/src/artanis-approval-gates.test.ts",
      "apps/openagents.com/workers/api/src/artanis-autonomy-ladder.test.ts",
      "apps/openagents.com/workers/api/src/artanis-continual-learning-templates.test.ts",
      "apps/openagents.com/workers/api/src/artanis-diagnosis-grounding-gate.test.ts",
      "apps/openagents.com/workers/api/src/artanis-distillation-dataset-receipt.test.ts",
      "apps/openagents.com/workers/api/src/artanis-domain-repository.contract.test.ts",
      "apps/openagents.com/workers/api/src/artanis-domain-store.test.ts",
      "apps/openagents.com/workers/api/src/artanis-fleet-overseer-tick.test.ts",
      "apps/openagents.com/workers/api/src/artanis-forum-delivery.test.ts",
      "apps/openagents.com/workers/api/src/artanis-forum-listener.test.ts",
      "apps/openagents.com/workers/api/src/artanis-forum-publication.test.ts",
      "apps/openagents.com/workers/api/src/artanis-forum-responder-khala.test.ts",
      "apps/openagents.com/workers/api/src/artanis-forum-reward-smoke.test.ts",
      "apps/openagents.com/workers/api/src/artanis-forum-reward-visibility.test.ts",
      "apps/openagents.com/workers/api/src/artanis-forum-verification.test.ts",
      "apps/openagents.com/workers/api/src/artanis-gepa-production-smoke.test.ts",
      "apps/openagents.com/workers/api/src/artanis-gepa-scheduled-runner-proof.test.ts",
      "apps/openagents.com/workers/api/src/artanis-health.test.ts",
      "apps/openagents.com/workers/api/src/artanis-labor-green-readiness.test.ts",
      "apps/openagents.com/workers/api/src/artanis-labor-receipt-feed-verify.test.ts",
      "apps/openagents.com/workers/api/src/artanis-labor-receipt-routes.test.ts",
      "apps/openagents.com/workers/api/src/artanis-labor-receipt-store-d1.test.ts",
      "apps/openagents.com/workers/api/src/artanis-labor-receipt-store.test.ts",
      "apps/openagents.com/workers/api/src/artanis-labor-request-receipt-golden.test.ts",
      "apps/openagents.com/workers/api/src/artanis-labor-request-receipt.test.ts",
      "apps/openagents.com/workers/api/src/artanis-labor-requester.test.ts",
      "apps/openagents.com/workers/api/src/artanis-labor-tick-driver.test.ts",
      "apps/openagents.com/workers/api/src/artanis-launch-smoke.test.ts",
      "apps/openagents.com/workers/api/src/artanis-loop.test.ts",
      "apps/openagents.com/workers/api/src/artanis-mind.test.ts",
      "apps/openagents.com/workers/api/src/artanis-model-lab-context.test.ts",
      "apps/openagents.com/workers/api/src/artanis-network-stats-d1.test.ts",
      "apps/openagents.com/workers/api/src/artanis-operator-chat-routes.test.ts",
      "apps/openagents.com/workers/api/src/artanis-operator-console-routes.test.ts",
      "apps/openagents.com/workers/api/src/artanis-operator-dashboard-routes.test.ts",
      "apps/openagents.com/workers/api/src/artanis-operator-dispatch-execution.test.ts",
      "apps/openagents.com/workers/api/src/artanis-operator-forum-update.test.ts",
      "apps/openagents.com/workers/api/src/artanis-operator-grounding-gate.test.ts"
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
        "command": "blume build --strict --analyze --budget-js 3800 --budget-css 120",
        "manifest_path": "apps/openagents.com/apps/docs/package.json",
        "name": "build"
      },
      {
        "command": "blume dev --host 127.0.0.1 --port 4322",
        "manifest_path": "apps/openagents.com/apps/docs/package.json",
        "name": "dev"
      },
      {
        "command": "node --test server.test.mjs infra-contract.test.mjs",
        "manifest_path": "apps/openagents.com/apps/docs/package.json",
        "name": "test"
      },
      {
        "command": "blume check --strict",
        "manifest_path": "apps/openagents.com/apps/docs/package.json",
        "name": "typecheck"
      },
      {
        "command": "blume validate --strict",
        "manifest_path": "apps/openagents.com/apps/docs/package.json",
        "name": "validate"
      },
      {
        "command": "pnpm run typecheck && pnpm run validate && pnpm run test && pnpm run build",
        "manifest_path": "apps/openagents.com/apps/docs/package.json",
        "name": "verify"
      },
      {
        "command": "pnpm run build && node --import tsx src/routes/-funnel-budget.ts",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "budget"
      },
      {
        "command": "vp build --logLevel warn && vp pack cloudrun/server.mjs --out-dir dist/cloudrun --format esm --platform node --target node24 --minify",
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
        "command": "vp dev",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "dev"
      },
      {
        "command": "vp preview",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "preview"
      },
      {
        "command": "vp test --root ../../../.. --run --project @openagentsinc/openagents-com-start",
        "manifest_path": "apps/openagents.com/apps/start/package.json",
        "name": "test"
      },
      {
        "command": "tsc -p tsconfig.json --noEmit",
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
        "command": "pnpm --dir apps/docs run build",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "build:docs"
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
        "command": "pnpm --dir apps/docs run dev",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "dev:docs"
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
        "command": "pnpm --dir apps/docs run test",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "test:docs"
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
        "command": "pnpm --dir apps/docs run typecheck",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "typecheck:docs"
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
        "command": "pnpm --dir apps/docs run verify",
        "manifest_path": "apps/openagents.com/package.json",
        "name": "verify:docs"
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
        "command": "pnpm run check:google-cloud-authority && pnpm run check:sol-docs && pnpm run test:sol-docs && pnpm --dir apps/openagents.com run check:deploy",
        "manifest_path": "package.json",
        "name": "check:deploy"
      },
      {
        "command": "vp lint --quiet && node scripts/vp1-retired-money-surface-guard.mjs . && node scripts/zero-supported-bun-guard.mjs . && node scripts/google-cloud-authority-guard.mjs",
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
    "head": "a474c1593b1c69ae37b66cb2fedf048310c58b58",
    "inventory_digest": "sha256:4755edbedd3c0d32bf3c0d1e3e8e15d716a8befc7f8d534e43c7b2f138d9e438",
    "repository_label": "productspec-assurancespec-ui-performance",
    "state": "dirty",
    "tracked_file_count": 8728,
    "tree": "10baca8ee42d370c2fddb306659b537d6942e4d6",
    "truncated": true
  }
}
```

## Obligations

Every criterion has one obligation. AC-15 through AC-20 carry complete proposed
proof design for the post-RC16 UI work. AC-1 through AC-14 remain explicit
`needs_design` obligations and therefore prevent full Phase 2 admission or
execution until their proof design is reconciled.

```assurancespec-obligations
[
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-1"
    ],
    "disposition": "required",
    "id": "AO-AC-1-01",
    "source_claim_digest": "sha256:0224169f0370835b67902d9b22bc7459bd940f469dcd6080d539298ec1cf6a2c",
    "source_claim_snapshot": "The shipped Desktop workbench mounts through one React 19 root and renders the scoped shell, session rail, conversation header, timeline, composer, command palette, decision surface, review drawer, and status notices as ordinary React elements produced by declared renderer-private Effect Native lowerings.",
    "title": "Assure AC-1"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-2"
    ],
    "disposition": "required",
    "id": "AO-AC-2-01",
    "source_claim_digest": "sha256:e27b7e1d0ca90dc6e405a6f6ad883d2a2cf2d6c89488fc590e721b88d2344b95",
    "source_claim_snapshot": "The React surface consumes one synchronous Effect-owned snapshot subscription and dispatches only existing registered typed intent keys; Strict Mode, rerender, reload, remount, and recovery tests observe no duplicate host subscription, command, or terminal outcome.",
    "title": "Assure AC-2"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-3"
    ],
    "disposition": "required",
    "id": "AO-AC-3-01",
    "source_claim_digest": "sha256:c15372f2edabf55ecb8ba3f3ee35c5d7b4cbca69392108ee3555313053d7e773",
    "source_claim_snapshot": "A supported user can launch locally, use the ordinary logged-in Codex session, grant one repository, and start an ordinary new chat without an OpenAgents account, ProductSpec artifact, accepted plan, Fleet setup, account selector, or provider selector.",
    "title": "Assure AC-3"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-4"
    ],
    "disposition": "required",
    "id": "AO-AC-4-01",
    "source_claim_digest": "sha256:0871bce25793e24c7858acb87b3e8a1fcbe709c703faac14a0fa60c7fc559531",
    "source_claim_snapshot": "The React session rail paints bounded top-level metadata before transcript hydration and supports new, search, select, resume, archive, delete, and paging while preserving stable ordering, titles, status, attention, selection, and restart restoration.",
    "title": "Assure AC-4"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-5"
    ],
    "disposition": "required",
    "id": "AO-AC-5-01",
    "source_claim_digest": "sha256:34136b648bbab41f8a3385fa41d14933dbd725dd5f977b52bef692c0a0f38c21",
    "source_claim_snapshot": "The React timeline renders stable keyed authored text plus compact plan, tool, change, blocker, error, interruption, gap, and terminal treatments in causal order; transport, metadata, context, and token-accounting events do not appear as conversation messages, settled consecutive work folds behind a bounded disclosure, active work and streaming state remain visible without duplicating or reordering items, and prepending older variable-height content preserves the first visible item without a stale intermediate frame.",
    "title": "Assure AC-5"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-6"
    ],
    "disposition": "required",
    "id": "AO-AC-6-01",
    "source_claim_digest": "sha256:642ccbbdec82f179ba52b5994a0d35ad7661c4b194929eaae5d1255bcc87ca85",
    "source_claim_snapshot": "The React composer captures the first keystroke, preserves IME composition, grows within its bound, and exposes the correct send, stop, steer, or queue action for the current state; pointer, keyboard, command-palette, and native-menu entry points resolve to the same registered command identity.",
    "title": "Assure AC-6"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-7"
    ],
    "disposition": "required",
    "id": "AO-AC-7-01",
    "source_claim_digest": "sha256:617137640c4334e0167cba759742d88ad1247001ce465d3dcabbde48dd919136",
    "source_claim_snapshot": "Question, approval, and plan-review requests open an accessible focused decision surface with complete choices, explicit pending, stale, failed, and resolved states, predictable Escape behavior, and focus restoration; prose or optimistic UI never records the decision.",
    "title": "Assure AC-7"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-8"
    ],
    "disposition": "required",
    "id": "AO-AC-8-01",
    "source_claim_digest": "sha256:d86c421ce60e05e5f4d40289b31002bf8e50e17a57be0e95e5d80771c42295aa",
    "source_claim_snapshot": "Repository context and read-only status or exact diff review remain bounded to the current WorkContext and correlated timeline refs; stale, revoked, secret-shaped, binary, oversized, or unavailable output fails visibly and no Git mutation or absolute path enters React props or state.",
    "title": "Assure AC-8"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-9"
    ],
    "disposition": "required",
    "id": "AO-AC-9-01",
    "source_claim_digest": "sha256:9a16aea2c950fdc824caed18b898f8a216c5ce0747df3027c6528fa5ed2d1d7a",
    "source_claim_snapshot": "The scoped workbench is fully operable by keyboard at 760 by 520 CSS pixels, restores focus across overlays, provides bounded screen-reader announcements, meets WCAG 2.2 AA contrast for text and controls, and honors reduced-motion preference.",
    "title": "Assure AC-9"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-10"
    ],
    "disposition": "required",
    "id": "AO-AC-10-01",
    "source_claim_digest": "sha256:11a98bf7488b019d5589aaff11537c7a9e5e415f44380a704c2f4538e5be5528",
    "source_claim_snapshot": "Empty, loading, hydrating, streaming, blocked, offline, incompatible, signed-out, quota, rate-limit, policy-denied, revoked, interrupted, and failed states are visually distinct, have actionable copy where recovery exists, and never masquerade as success or completion.",
    "title": "Assure AC-10"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-11"
    ],
    "disposition": "required",
    "id": "AO-AC-11-01",
    "source_claim_digest": "sha256:52af8840c7b62e504c57d729cbe35ebb3a67bafa9332aaea6d2f3344f5691c80",
    "source_claim_snapshot": "React, Tailwind, Base UI, DOM APIs, hooks, and JSX remain in declared renderer or Desktop-host modules; portable Effect Native modules remain React-free and schema fields contain no arbitrary component, callback, ReactNode, className, credential, raw provider payload, absolute path, Node handle, generic IPC, process handle, or filesystem handle.",
    "title": "Assure AC-11"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-12"
    ],
    "disposition": "required",
    "id": "AO-AC-12-01",
    "source_claim_digest": "sha256:6e0bc7506543d5b764f9226ede1d7ec46a27949e418cbf94968697a91b4d3705",
    "source_claim_snapshot": "One installed-app journey creates a chat, submits a real Codex turn, streams typed content, resolves one blocker or review request, opens one exact read-only change, stops or steers a running turn, switches away and back, reloads the renderer, restarts the app, and resumes the same durable session without duplicate execution or silent rerun.",
    "title": "Assure AC-12"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-13"
    ],
    "disposition": "required",
    "id": "AO-AC-13-01",
    "source_claim_digest": "sha256:b67eee290e43b80f8e7bb208fce1ef13c46537ff7e1380c8dc704fcbe8064faa",
    "source_claim_snapshot": "Non-converted surfaces use the existing compatibility backend without rendering the same workbench state through two backends, and the Phase 2 release does not require catalog-wide React-DOM coverage, web hydration, mobile changes, or compatibility-backend removal.",
    "title": "Assure AC-13"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-14"
    ],
    "disposition": "required",
    "id": "AO-AC-14-01",
    "source_claim_digest": "sha256:603a765f9f0feb31b5107d05f0caae99e01b48cc5a0b382ac053bb010eb82e2a",
    "source_claim_snapshot": "The React workbench meets the existing signed artifact and security gates, has a median warm launch-to-interactive time no greater than 1500 ms and p95 no greater than 2500 ms on the admitted macOS ARM64 profile, and switches a metadata-ready session to its first stable transcript paint within 400 ms at p95 for the acceptance corpus.",
    "title": "Assure AC-14"
  },
  {
    "activation_gate": "GATE-PHASE2-POST-RC16",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/desktop-launch-workspace.test.ts",
      "apps/openagents-desktop/src/renderer/composer-focus.test.ts",
      "apps/openagents-desktop/src/renderer/shell.test.ts"
    ],
    "criterion_refs": [
      "AC-15"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workbench",
      "workspace_authority",
      "visible_interaction"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-PHASE2-NODE24-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "contract_plus_headed_electron",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "headed_computer_use_receipt",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "cancel_or_launch_context_divergence",
      "ref": "apps/openagents-desktop/src/desktop-launch-workspace.test.ts"
    },
    "id": "AO-AC-15-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/shell.test.ts",
      "statement": "Launch work area, cwd adoption, empty-chat disclosure, cancel/select semantics, and composer focus pass their typed tests and are operable in built headed Electron."
    },
    "source_claim_digest": "sha256:1f823cd6a46e48a57d8e20795c1f1d960d06c71f9d6122d266d6cdd2b0fc31f1",
    "source_claim_snapshot": "Ordinary launch fills the active display work area without entering fullscreen, adopts the launcher directory as the canonical WorkContext, centers that directory and one accessible Change action in an empty new chat, preserves the current context on cancel or failure, updates all workspace consumers after confirmation, and focuses the new-session composer before pointer use.",
    "technique": "typed_contract_with_headed_computer_use",
    "title": "Assure AC-15"
  },
  {
    "activation_gate": "GATE-PHASE2-POST-RC16",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/navigation-history.test.ts",
      "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
      "apps/openagents-desktop/src/renderer/design-conformance.test.ts"
    ],
    "criterion_refs": [
      "AC-16"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workbench",
      "navigation",
      "visible_interaction"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-PHASE2-NODE24-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "contract_plus_headed_electron",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "headed_computer_use_receipt",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "unregistered_or_duplicate_navigation",
      "ref": "apps/openagents-desktop/src/renderer/react-composer.test.tsx"
    },
    "id": "AO-AC-16-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/navigation-history.test.ts",
      "statement": "The exact four-entry rail, bounded successful navigation history, stable geometry, and registry-only palette pass typed identity and headed keyboard/pointer checks."
    },
    "source_claim_digest": "sha256:612cf2f21ce972c1983c3b0d5b062588609f215973eb3005bed076eaa5bf6615",
    "source_claim_snapshot": "The compact shell exposes exactly New session, Chat, Project home, and Settings as primary destinations; keeps titles, status, and timestamps aligned without hover layout shift; records one bounded successful-navigation Back/Forward history; and opens a registry-only command palette whose pointer and keyboard selection dispatch the exact same typed command while unavailable or copied reference-only actions remain absent.",
    "technique": "surface_allowlist_with_identity_sensitivity",
    "title": "Assure AC-16"
  },
  {
    "activation_gate": "GATE-PHASE2-POST-RC16",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/composer-images.test.ts",
      "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
      "apps/openagents-desktop/src/codex-local-runtime.test.ts"
    ],
    "criterion_refs": [
      "AC-17"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workbench",
      "attachment_authority",
      "runtime_lowering"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-PHASE2-NODE24-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "bounded_input_contract_plus_smoke",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "built_smoke_receipt",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "oversize_wrong_type_or_renderer_path",
      "ref": "apps/openagents-desktop/src/renderer/composer-images.test.ts"
    },
    "id": "AO-AC-17-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
      "statement": "Picker/paste/drop admission, count and byte bounds, preview removal, image-only idle send, failure restoration, and exact Codex lowering pass without renderer path authority."
    },
    "source_claim_digest": "sha256:e0a9df4484a59a71f4d60e0e9755ec6939ae74cc86e197d4a11261511604625c",
    "source_claim_snapshot": "The composer admits PNG, JPEG, WebP, and GIF attachments through picker, paste, and drop with at most 8 images and 10 MB per image, renders removable accessible previews, permits image-only send only while idle, keeps Steer and Queue text-only, restores attachments after failed send, translates admitted content to Codex localImage input, and exposes no arbitrary renderer path or base64 authority.",
    "technique": "bounded_input_contract_with_privacy_falsifier",
    "title": "Assure AC-17"
  },
  {
    "activation_gate": "GATE-PHASE2-POST-RC16",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/provider-runtime-host.test.ts",
      "apps/openagents-desktop/src/codex-config-health.test.ts",
      "apps/openagents-desktop/src/renderer/react-composer.test.tsx"
    ],
    "criterion_refs": [
      "AC-18"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workbench",
      "runtime_custody",
      "privacy"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-PHASE2-NODE24-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "minimal_path_contract_plus_built_artifact",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "artifact_runtime_receipt",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "ambient_runtime_or_optimistic_repair",
      "ref": "apps/openagents-desktop/src/provider-runtime-host.test.ts"
    },
    "id": "AO-AC-18-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/codex-config-health.test.ts",
      "statement": "Minimal-PATH packaged runtime custody, Codex-only settings, private identity projection, exact diagnosis, narrow repair, and honest update failure pass candidate and negative-path checks."
    },
    "source_claim_digest": "sha256:9eb27269e8724dbd6e617187c1d29291a5b417b47605cc59b9445a7c1354a738",
    "source_claim_snapshot": "The exact packaged pinned Codex runtime is the sole turn and maintenance authority under a minimal PATH; Desktop neither discovers nor mutates ambient PATH or NVM installations, Settings and maintenance remain Codex-only, sensitive identity is same-length fake and blurred until explicit reveal, configuration diagnostics name exact defects, repair changes only narrowly verified keys, and launch advisory or update failure never steals focus or claims optimistic success.",
    "technique": "custody_contract_with_minimal_path_falsifier",
    "title": "Assure AC-18"
  },
  {
    "activation_gate": "GATE-PHASE2-POST-RC16",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/local-harness.test.ts",
      "apps/openagents-desktop/src/renderer/terminal-workspace.test.ts",
      "apps/openagents-desktop/tests/workspace-service.test.ts"
    ],
    "criterion_refs": [
      "AC-19"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workbench",
      "renderer_performance",
      "event_correctness"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-PHASE2-NODE24-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "deterministic_stress_plus_headed_sampling",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "performance_receipt",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "one_to_one_publication_or_unbounded_tail",
      "ref": "apps/openagents-desktop/tests/workspace-service.test.ts"
    },
    "id": "AO-AC-19-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/renderer/local-harness.test.ts",
      "statement": "Ten-thousand-event provider, PTY, and workspace corpora preserve order and terminal semantics while bounding publications, queues, refs, teardown work, RSS/CPU growth, and p95 input latency."
    },
    "source_claim_digest": "sha256:759cb1bc3103bfa91dc57055784a11a91d38069a1b8a52f18824d24f8c2ff9af",
    "source_claim_snapshot": "Provider text and PTY output publish at renderer cadence with semantic-boundary, completion, and teardown flush laws; workspace watching ignores dot/generated trees, deduplicates for 75 ms, carries at most 256 exact refs before one explicit overflow, and refreshes only visible loaded directories or matching open tabs; 10,000-event corpora preserve exact order and bounded tails without 10,000 state publications, unbounded queue growth, late post-dispose work, or loss of input responsiveness.",
    "technique": "deterministic_stress_with_resource_budgets",
    "title": "Assure AC-19"
  },
  {
    "activation_gate": "GATE-PHASE2-POST-RC16",
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/renderer/mvp-assurance-congruence.test.ts",
      "apps/openagents-desktop/src/renderer/react-composer.test.tsx",
      "apps/openagents-desktop/scripts/smoke.ts"
    ],
    "criterion_refs": [
      "AC-20"
    ],
    "disposition": "required",
    "domains": [
      "desktop_workbench",
      "programmatic_control",
      "visible_computer_use"
    ],
    "environment_refs": [
      "ENV-OA-DESKTOP-PHASE2-NODE24-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "identity_matrix_plus_headed_computer_use",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "headed_computer_use_receipt",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "missing_route_duplicate_dispatch_or_test_only_control",
      "ref": "apps/openagents-desktop/src/renderer/mvp-assurance-congruence.test.ts"
    },
    "id": "AO-AC-20-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/scripts/smoke.ts",
      "statement": "Every expected-working action maps to one canonical typed identity and one accessible visible route, and built headed Electron drives the route with real focus, keyboard, pointer, and accessibility semantics."
    },
    "source_claim_digest": "sha256:e33e37cc7c3da0fa8fc2a795f5ce17792f7077eddd1604700c01b78e9a07f2c6",
    "source_claim_snapshot": "Every expected-working scoped action has one canonical programmatic intent or command route and one visible accessible keyboard or pointer route that dispatch the same identity exactly once; a built headed Electron journey proves the visible route with real Chromium focus, accessibility, keyboard, and pointer semantics suitable for external Computer Use without adding product-operated computer-use authority.",
    "technique": "control_congruence_with_headed_computer_use",
    "title": "Assure AC-20"
  }
]
```

## Gates

The post-RC16 gate is intentionally candidate-local. It requires exact subject
and environment bindings, confirmed candidate observations, refuted
falsifiers, stable resource measurements, headed visible control proof,
independent review, and no exception. It does not admit or release itself.

```assurancespec-gates
[
  {
    "expression": "subject=current && environment=bound && candidate=CONFIRMED && falsifier=REFUTED && stability=stable && freshness=current && headed_computer_use=green && independent_review=accepted && exception=none",
    "id": "GATE-PHASE2-POST-RC16"
  }
]
```

## Evidence Policy

Links are pointers, not verdicts. Missing or unreviewed evidence remains INCONCLUSIVE.

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
    "performance_or_headed_receipt_as_applicable"
  ]
}
```

## Authority Boundaries

This proposal cannot admit, execute, verify, waive, release, or change a public promise.

```assurancespec-authority
{
  "admitted_roles": [],
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
