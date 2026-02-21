# Client Telemetry v1 (Khala/Runtime)

Status: Active  
Schema authority: `proto/openagents/sync/v1/client_telemetry.proto`  
Issue: OA-RUST-093 (`#1928`)

## Purpose

Define a versioned, privacy-safe client telemetry contract for reconnect/auth/replay diagnostics across web, desktop, iOS, and Onyx surfaces.

## Envelope

`openagents.sync.v1.ClientTelemetryEvent` carries:

1. Versioning:
   - `schema_version` (required; currently `openagents.sync.client_telemetry.v1`)
2. Correlation:
   - `event_id`, `session_id`, `occurred_at_unix_ms`
3. Surface/build segmentation:
   - `surface`, `client_build_id`, `app_version`, `protocol_version`
4. Sync segmentation:
   - `topic`, `topic_class`
5. Privacy-safe actor segmentation:
   - `actor_scope_hash` (hashed; no raw user/device identifiers)
6. Event payload:
   - oneof `reconnect | auth_failure | replay | delivery_error`

## Privacy Rules

1. No raw user id, org id, email, or device UUID in telemetry payloads.
2. `actor_scope_hash` must be a one-way hash generated client-side or at ingestion edge.
3. Payloads are diagnostics-only and must not include message content bodies.
4. Telemetry retention should follow minimum needed for SLO diagnosis and incident response.

## Event Types

1. `ClientReconnect`
   - reconnect attempts/backoff/resume behavior.
2. `ClientAuthFailure`
   - auth deny status + reason taxonomy.
3. `ClientReplay`
   - replay catchup/stale cursor and catchup duration diagnostics.
4. `ClientDeliveryError`
   - channel-specific delivery error envelopes.

## Governance

1. Additive-only evolution in `v1`.
2. Breaking changes require a new versioned namespace.
3. Changes require:
   - `buf lint`
   - `buf breaking`
   - `./scripts/verify-proto-generate.sh`

## Fixture

- `docs/protocol/fixtures/client-telemetry-v1.json`
