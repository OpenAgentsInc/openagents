# Deployment Docs

This directory holds release/deployment documentation for OpenAgents Desktop.

## Files

- `MACOS_RELEASE_PLAN.md`: current macOS release flow and script usage.
- `MACOS_SIGNING_NOTARIZATION.md`: Apple signing + notarization setup and verification.
- `DEPLOYMENT_CONSIDERATIONS.md`: cross-platform reference notes modeled from Zed (background context).
- `NEXUS_HOTFIX_LANE.md`: frozen April 15 hotfix lane contract for Nexus.
- `SYMPHONY_GCP_RUNBOOK.md`: gcloud-first Symphony deployment runbook (connected to existing `oa-bitcoind`) with scripted rollout, hardening, ops bootstrap, and restore drill flow.
- `NEXUS_GCP_RUNBOOK.md`: image fallback path plus the staged binary-first Nexus deploy runbook.
- `NEXUS_WARM_BUILDER.md`: dedicated warm-builder bootstrap, cache layout, and binary build path for Nexus hotfixes.
