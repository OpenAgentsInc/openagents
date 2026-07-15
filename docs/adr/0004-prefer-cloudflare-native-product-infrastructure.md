---
status: "superseded"
date: 2026-06-28
superseded-by: ADR-0014
decision-makers: OpenAgents maintainers
---

# Superseded: Prefer Cloudflare-native product infrastructure

This decision is retired. It described a short-lived implementation direction
that no longer represents production and must not be used as deployment,
storage, runtime, migration, or fallback authority.

Google Cloud was and remains the production infrastructure authority. The
stale Cloudflare applications and account resources described by the original
decision have been removed. SHC was a limited pilot, never the primary
infrastructure, and is also retired.

Current authority is [ADR-0014](0014-use-google-cloud-as-the-sole-production-infrastructure.md).
Historical details remain available in Git history and the corrective
after-action audit at
[`docs/sol/2026-07-14-google-cloud-authority-cleanup-after-action.md`](../sol/2026-07-14-google-cloud-authority-cleanup-after-action.md).
