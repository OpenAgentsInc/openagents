# Deployment Docs

This directory holds release/deployment documentation for OpenAgents Desktop.

## Files

- `MACOS_RELEASE_PLAN.md`: current macOS release flow and script usage.
- `MACOS_SIGNING_NOTARIZATION.md`: Apple signing + notarization setup and verification.
- `DEPLOYMENT_CONSIDERATIONS.md`: cross-platform reference notes modeled from Zed (background context).
- `NEXUS_HOTFIX_LANE.md`: frozen April 15 hotfix lane contract for Nexus.
- `SYMPHONY_GCP_RUNBOOK.md`: gcloud-first Symphony deployment runbook (connected to existing `oa-bitcoind`) with scripted rollout, hardening, ops bootstrap, and restore drill flow.
- `NEXUS_GCP_RUNBOOK.md`: primary binary-first Nexus hotfix runbook plus the image fallback lane.
- `NEXUS_LDK_GCP_RUNBOOK.md`: private GCP topology for Nexus v0.2 LDK Server, `bitcoind`, read-only smoke, backups, and restore drills.
- `NEXUS_HEALTH_RUNNER_GCP_RUNBOOK.md`: Cloud Run Job/Service, Cloud Scheduler, and Secret Manager lane for the hosted `nexus-health-agent` service-account runner.
- `PYLON_NEXUS_EARNING_RELEASE_RUNBOOK.md`: public Pylon earning-loop and Nexus hosted-homework release/proof checklist, including the build and operator mistakes from Issue #4413.
- `NEXUS_WARM_BUILDER.md`: dedicated warm-builder bootstrap, cache layout, and binary build path for Nexus hotfixes.
