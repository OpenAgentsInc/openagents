# OpenAgents retained-audio production runbook

This is an owner-gated production path. It never makes retained media public.
The checked-in live resource descriptor is
`apps/openagents-audio/deploy/production.json`. It contains identifiers and
policy only, never credentials.

1. Create a dedicated regional GCS bucket. Use uniform bucket-level access and
   enforced public-access prevention. **Disable** media object versioning. Use
   CMEK and lifecycle deletion no later than the maximum policy TTL. Cloud SQL
   backups contain manifests/receipts but no media. Grant the dedicated retention
   service account only object create/get/delete/list and KMS encrypt/decrypt.
2. Apply `packages/khala-sync-server/migrations/0064_audio_retention.sql` to the
   private Cloud SQL database through its Auth Connector. SQL stores manifests
   and receipts only, never media bytes or object credentials.
3. Configure the service with the private bucket name, workload identity, KMS
   key epoch, segment/session byte ceilings, segment-count ceiling, and maximum
   TTL. Do not use a service-account key file in the deployed service.
4. Run `bun run --cwd apps/openagents-audio test` and `typecheck`, then the
   gated smoke with a synthetic retained-session receipt. Upload all raw and
   derived classes as encrypted bounded objects. Reconcile each sequence or
   explicit gap.

   Export the fixture and compare its bytes. Delete the fixture.
   Confirm that no objects remain. Keep the retained-session and access receipts.
5. Inspect Cloud Audit Logs for the service identity's create/read/delete
   operations. Run a source/log/support-bundle scan for raw fixture bytes,
   bearer tokens, object credentials, prompts, and public URLs.
6. Record only public-safe refs, counts, digests, and timestamps in the issue.
   Never paste object paths containing owner refs, media, credentials, or raw
   SQL rows into GitHub.

Rollback is fail-closed. First, stop new retained-session admissions. Do not
permit new object writes. Reconcile existing manifests. Export only after an
authorized owner request.

Then, delete or expire the data under the applicable policy. Legal hold
prevents media deletion until separately released. It does not authorize new
capture.
