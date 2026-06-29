# Sites Provisioning Plan Contract

Issue #201 adds the first reviewed provisioning contract for generated Sites
that need hosted storage or runtime configuration.

## What Changed

- Migration `0086_site_provisioning_plans.sql` adds
  `site_provisioning_plans`.
- `sites-provisioning.ts` validates D1, R2, KV, plain env, and secret env needs
  before any plan or receipt is stored.
- `POST /api/operator/sites/:siteId/provisioning-plans` lets an operator record
  a review-required plan or an approved receipt with an `Idempotency-Key`.

## Manifest Shape

A generated app can declare:

- `d1`: binding names, migration refs, retention policy, and per-Site/shared
  scope.
- `r2`: binding names, prefixes, retention policy, and per-Site/shared scope.
- `kv`: binding names, namespace refs, retention policy, and per-Site/shared
  scope.
- `env`: key names, plain public/runtime values, or secret refs.

Secrets must be refs only. The manifest rejects secret env entries with
`plainValue`, missing `secretRef`, or provider-secret-shaped material.

## Statuses

- `review_required`: recorded when an operator or agent submits a provisioning
  plan without an approving reviewer.
- `approved`: recorded when the authenticated operator records the plan with
  `approve: true`.

The plan table stores `requested_by_user_id`, `reviewed_by_user_id`,
`reviewed_at`, the manifest JSON, and receipt JSON.

## Operator Request

```json
{
  "approve": true,
  "resourceManifest": {
    "d1": [{ "bindingName": "SITE_DB", "retentionPolicy": "standard" }],
    "r2": [{ "bindingName": "SITE_UPLOADS", "prefix": "sites/site_id" }],
    "kv": [{ "bindingName": "SITE_CACHE" }],
    "env": [
      { "key": "PUBLIC_SITE_NAME", "kind": "plain", "plainValue": "OTEC" },
      {
        "key": "PAYMENT_WEBHOOK_SECRET",
        "kind": "secret",
        "secretRef": "cf-secret:sites/otec/payment-webhook"
      }
    ]
  },
  "receipt": {
    "d1": [{ "bindingName": "SITE_DB", "migrationRef": "migration:001" }],
    "r2": [{ "bindingName": "SITE_UPLOADS", "prefix": "sites/site_id" }]
  }
}
```

## Boundaries

This slice records a reviewed plan/receipt. It does not yet create Cloudflare
D1 databases, KV namespaces, R2 buckets, or hosted secret values from inside
OpenAgents product surface. Those actual provisioning actions should be a credentialed follow-up
client that consumes this approved plan and writes a redacted receipt back to
the same contract.

No secret values belong in `.openagents/site.json`, docs, issue comments,
customer projections, generated source, or D1 plain text.
