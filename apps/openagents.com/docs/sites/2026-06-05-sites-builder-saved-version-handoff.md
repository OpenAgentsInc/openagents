# Sites Builder Saved Version Handoff

Issue #199 implements the first durable handoff from a successful builder
session candidate into the existing Sites version lifecycle.

## What Changed

- Migration `0084_site_builder_saved_versions.sql` adds
  `site_builder_saved_versions` as the idempotent bridge between a builder
  session and the saved `site_versions` row.
- `sites-builder-saved-versions.ts` validates the builder session, rejects
  mismatched Site IDs, calls the existing Sites `saveVersion` lifecycle, stores
  the builder-to-version mapping, and emits a customer-visible builder event.
- `POST /api/operator/sites/builder-sessions/:sessionId/versions` lets an
  admin/operator save a review candidate from builder output.

## Request Shape

The endpoint requires `Idempotency-Key` and an authenticated admin session.
The body must include:

```json
{
  "siteId": "site_project_id",
  "staticAssetsManifest": {
    "assets": {
      "/index.html": {
        "r2Key": "sites/site_project_id/builder/index.html",
        "contentType": "text/html; charset=utf-8"
      }
    }
  }
}
```

Optional fields include `previewId`, `artifactRef`, `buildReceiptRef`,
`sourceHash`, `sourceCommitSha`, `sourceArchiveText`, `buildLogText`,
`workerModuleText`, `workerModuleR2Key`, D1/R2 binding names, and
`siteMetadata`.

## Metadata Contract

The saved `site_versions.metadata_json` receives the caller's `siteMetadata`
plus a `builder` block:

```json
{
  "builder": {
    "sessionId": "site_builder_session_id",
    "orderId": "software_order_id_or_null",
    "previewId": "preview_id_or_null",
    "artifactRef": "artifact_ref_or_null",
    "buildReceiptRef": "receipt_ref_or_null",
    "sourceHash": "source_hash_or_null",
    "notes": "bounded_notes_or_null",
    "idempotencyKey": "request_idempotency_key"
  }
}
```

This preserves the path from builder session to reviewable Site version without
changing deployment authority.

## Save Versus Deploy

Saving a builder candidate:

- creates a `site_versions` row with `build_status = 'saved'`;
- writes source, build log, worker, and manifest artifacts through the existing
  R2 artifact store;
- records `site_version.saved` in the Site event ledger;
- records a `save_requested` builder-session event visible to the customer;
- does not create or activate a `site_deployments` row.

Deployment remains a separate reviewed action. A saved builder candidate can be
listed and reviewed anywhere existing saved Site versions are surfaced, but it
does not replace the live revision until the deployment path explicitly
activates it.

## Follow-Up

The next issue should wire WFP upload, binding injection, health checks, and
deployment recording to consume these saved versions instead of bypassing the
version lifecycle.
