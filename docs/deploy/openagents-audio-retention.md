# OpenAgents retained-audio production runbook

This is an owner-gated production path. It never makes retained media public.
The checked-in live resource descriptor is
`apps/openagents-audio/deploy/production.json`; it contains identifiers and
policy only, never credentials.

1. Create a dedicated regional GCS bucket with uniform bucket-level access,
   enforced public-access prevention, media object versioning **disabled**,
   CMEK, and lifecycle deletion no later than the maximum policy TTL. Cloud SQL
   backups contain manifests/receipts but no media. Grant the dedicated retention
   service account only object create/get/delete/list and KMS encrypt/decrypt.
2. Apply `packages/khala-sync-server/migrations/0064_audio_retention.sql` to the
   private Cloud SQL database through its Auth Connector. SQL stores manifests
   and receipts only, never media bytes or object credentials.
3. Configure the service with the private bucket name, workload identity, KMS
   key epoch, segment/session byte ceilings, segment-count ceiling, and maximum
   TTL. Do not use a service-account key file in the deployed service.
4. Run `bun run --cwd apps/openagents-audio test` and `typecheck`, then the
   gated smoke with a synthetic retained-session receipt. It must upload at
   all raw/derived classes as encrypted bounded objects, reconcile every sequence or explicit
   gap, export and byte-compare the fixture, delete it, and confirm no objects
   remain while the retained-session and access receipts remain.
5. Inspect Cloud Audit Logs for the service identity's create/read/delete
   operations. Run a source/log/support-bundle scan for raw fixture bytes,
   bearer tokens, object credentials, prompts, and public URLs.
6. Record only public-safe refs, counts, digests, and timestamps in the issue.
   Never paste object paths containing owner refs, media, credentials, or raw
   SQL rows into GitHub.

Rollback is fail-closed: stop new retained-session admissions first, allow no
new object writes, reconcile existing manifests, export only on an authorized
owner request, then delete/expire under the applicable policy. Legal hold
prevents media deletion until separately released; it does not authorize new
capture.
