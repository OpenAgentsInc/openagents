# Sites Builder Session Ledger

Issue #192 adds the first durable VibeSDK-style builder foundation for
Autopilot Sites.

The ledger is intentionally repository-only in this slice. It does not expose a
new customer API yet. The next issues add create/read APIs, reconnectable event
streams, and file inspection surfaces.

## Durable Records

The `0082_site_builder_sessions.sql` migration adds:

- `site_builder_sessions`
- `site_builder_messages`
- `site_builder_phase_runs`
- `site_builder_events`
- `site_builder_file_snapshots`
- `site_builder_previews`
- `site_builder_artifacts`

Every row is linked to a builder session and uses idempotency keys. Session
records can reference an order, Site, workroom, source Site version, source
revision, active preview, and active artifact.

## Repository Contract

`workers/api/src/sites-builder-sessions.ts` provides typed Effect programs for:

- creating a builder session;
- appending customer-safe messages;
- appending phase/file/preview/build/deploy events;
- recording generated file snapshot metadata;
- recording preview refs;
- recording artifact refs;
- reading customer and operator projections.

The repository rejects private runner/provider payloads, secret-shaped strings,
wallet/payment material, bypass/captcha material, unsafe paths, and non
OpenAgents preview URLs.

## Projection Boundary

Customer projections expose session status, prompt summary, active preview ref,
and customer-visible messages only.

Operator projections expose counts and safe refs, but still avoid raw runner
payloads, provider logs, source archives, secrets, and unbounded diagnostics.

Full generated-file read/export behavior remains in `OPENAGENTS-SITES-VIBE-004`.
