# Site Source Export Contract

Date: 2026-06-05

Issue: `#204` / `OPENAGENTS-SITES-VIBE-013`

## What Changed

OpenAgents product surface now records reviewed Site source export receipts in
`site_source_exports`.

The first operator endpoint is:

```text
POST /api/operator/sites/:siteId/versions/:versionId/source-exports
```

It requires:

- an OpenAgents admin browser session;
- an `Idempotency-Key` header;
- a saved Site version with at least one exportable source ref:
  `source_archive_r2_key`, `artifact_manifest_r2_key`,
  `worker_module_r2_key`, or an explicit `sourceArtifactRef`;
- a passed secret scan;
- a bounded destination contract for `download`, `github_branch`, or
  `github_pull_request`.

The API records:

- actor and approver user refs;
- destination provider, owner, repository, branch, PR URL, or download URL;
- source archive, artifact manifest, worker module, or source artifact refs;
- token ref, token hash, and token expiry;
- secret scan status/ref;
- redacted receipt metadata.

## Token Boundary

This slice does not return a raw download or clone token. It stores only
`token_hash` in D1 and returns a public-safe `tokenRef` plus `tokenExpiresAt` in
the receipt projection.

Future source download or clone endpoints must require a presented token,
verify it against `token_hash`, reject expired/revoked rows, and avoid logging
or projecting raw token values.

## GitHub Boundary

This slice records reviewed GitHub export intent and refs. It does not yet call
GitHub to create branches or pull requests. The recorded receipt is the
authority handoff for the later credentialed GitHub writeback worker.

For non-Site PR-style requests, the same receipt shape should be reused once a
non-Site fulfillment artifact has a source artifact ref and a GitHub
destination. The current endpoint remains scoped to Site versions.

## Verification

- `bun run --cwd workers/api test src/site-source-exports.test.ts src/operator-sites-routes.test.ts`
- `bun run check:architecture`
