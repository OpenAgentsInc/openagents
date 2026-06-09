# Sites Workers for Platforms Deployment Attempts

Issue #200 adds the first deployment-attempt ledger and WFP health gate for
reviewed Sites deployments.

## What Changed

- Migration `0085_site_deployment_attempts.sql` adds
  `site_deployment_attempts`.
- `deployVersion` records an attempt for every deployment activation path.
- Workers for Platforms deployments now require a passed health receipt before
  the Site is marked live.
- Operator deploy requests can pass bounded upload, health, rollback,
  observability, and tag refs.

## Attempt Ledger

Each deployment attempt stores:

- Site, version, and optional deployment ID.
- Runtime kind, runtime script name, dispatch namespace, and external
  deployment ID.
- Attempt status: `activated`, `health_missing`, or `health_failed`.
- Upload receipt ref, health status/url/ref, rollback ref, observability ref.
- Bounded metadata with health check time/summary and tags.

The ledger stores refs and summaries only. It must not store raw Cloudflare API
responses, secret values, environment values, or full logs.

## WFP Health Gate

For `runtimeKind = "workers_for_platforms"`:

- The saved version must already contain a worker module artifact.
- `runtimeScriptName` and `dispatchNamespace` remain required.
- A health receipt with `status = "passed"` is required before activation.
- Missing or failed health writes a deployment attempt and then rejects the
  deployment without inserting an active `site_deployments` row or updating the
  Site's active version/deployment fields.

Static R2 deployments are recorded with `health_status = "not_recorded"` and
`healthStatus = "not_required"` in the activation event.

## Operator Deploy Body

The operator deploy endpoint accepts these additional bounded fields:

```json
{
  "runtimeKind": "workers_for_platforms",
  "runtimeScriptName": "site-worker-otec",
  "dispatchNamespace": "openagents-sites-production",
  "externalDeploymentId": "cf-deployment-ref",
  "uploadReceiptRef": "cf-upload-ref",
  "healthCheck": {
    "status": "passed",
    "url": "https://sites.openagents.com/slug/__health",
    "healthRef": "health-ref",
    "checkedAt": "2026-06-05T00:00:00.000Z",
    "summary": "Health check passed."
  },
  "rollbackRef": "rollback-ref",
  "observabilityRef": "observability-ref",
  "tags": ["site:site_project_id", "version:site_version_id"]
}
```

## Remaining Upload Gap

Cloudflare's Workers for Platforms upload path requires a Cloudflare account
API credential or an SDK/client capable of writing to:

`/accounts/:account_id/workers/dispatch/namespaces/:namespace/scripts/:script`

The current OpenAgents product surface Worker has the `SITES_DISPATCH` binding for runtime dispatch,
but it does not yet expose a safe first-party upload client or Cloudflare API
token binding. Therefore this slice records the reviewed upload receipt and
health result rather than pretending to upload from the Worker.

The next provisioner/deployment issue should add a credentialed WFP upload
client or an out-of-band deployer receipt writer, then feed its upload receipt
and health check result into this deployment-attempt contract.
