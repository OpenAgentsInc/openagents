---
status: "accepted"
date: 2026-07-14
decision-makers: OpenAgents maintainers
consulted: Root AGENTS.md, INVARIANTS.md, docs/DEPLOYMENT.md, apps/openagents.com/INVARIANTS.md
informed: OpenAgents contributors and agents
---

# Use Google Cloud as the sole production infrastructure

## Context and problem statement

OpenAgents production runs on Google Cloud. Stale repository language and
retired provider-account resources made Cloudflare and the former SHC pilot
look like current or fallback authorities. That ambiguity could misroute data,
credentials, traffic, deployments, or spend.

## Decision

Google Cloud is the sole production infrastructure authority:

- Cloud Run and GCE own compute.
- Cloud SQL owns relational state.
- Cloud Storage owns blobs and retained archives.
- Secret Manager owns runtime credentials.
- Cloud Scheduler owns scheduled entrypoints. And
- Google Cloud load balancing owns application ingress.

Cloudflare remains the authoritative DNS provider for `openagents.com`.
Registrar nameservers stay delegated to Cloudflare, and Cloudflare DNS records
remain DNS-only while pointing directly to Google Cloud. This deliberate DNS
control-plane boundary is not application hosting and is not a pending Cloud
DNS migration. Enabling the Cloudflare HTTP proxy, CDN, or WAF would require a
new infrastructure decision.

Cloudflare Workers, Durable Objects, D1, Hyperdrive, Queues, R2, Analytics
Engine, Browser Rendering, Pages, and Wrangler are retired. They may not be
runtime options, deployment targets, storage sources of truth, operator paths,
migration requirements, or fallbacks.

SHC was a limited pilot. It was never the primary production infrastructure
and may not be selected, priced, provisioned, or used as fallback. Terminal
pilot records may retain explicit `retired_pilot` provenance.

## Consequences

- Supported deploys go through the Cloud Run deployment script.
- Managed placement admits only `cloud-gcp`. Managed execution uses
  `gcloud_vm`.
- Live synchronization uses Cloud SQL plus the Cloud Run LiveHub service.
- Authoritative DNS remains in Cloudflare with DNS-only records targeting
  Google Cloud. No nameserver migration is planned.
- Retired-provider exports are private evidence, not warm standby systems.
- Historical documents remain evidence only when their status is explicit.
- The Google Cloud authority guard runs before deploy and in repository checks.

## Verification

The enforcement model is executable:

- `scripts/google-cloud-authority-guard.mjs` rejects retired packages,
  configurations, deploy operations, credentials, and service lanes.
- migration `0071_google_cloud_only_admission.sql` constrains production
  admission and preserves terminal pilot provenance without making it
  dispatchable. And
- the corrective after-action audit records the live-account and database
  retirement evidence.

See
[`docs/sol/2026-07-14-google-cloud-authority-cleanup-after-action.md`](../sol/2026-07-14-google-cloud-authority-cleanup-after-action.md).
