# Sites Builder Session API

Issue #193 exposes the first customer/operator-safe API surface for the
Autopilot Sites builder ledger.

## Endpoints

- `POST /api/sites/builder-sessions`
  - Creates or reconnects to a builder session.
  - Requires a browser session and `Idempotency-Key`.
  - Stores the current user as the owner and defaults the customer user to the
    same account.
- `GET /api/sites/builder-sessions/:sessionId`
  - Reads a customer-safe builder session projection.
  - Returns `404` for unauthorized users so private session existence is not
    leaked.
  - Admin users also receive the safe operator projection.
- `POST /api/sites/builder-sessions/:sessionId/messages`
  - Appends a customer-visible feedback/message record.
  - Requires ownership/customer access and `Idempotency-Key`.
- `POST /api/operator/sites/builder-sessions/:sessionId/events`
  - Appends operator builder events for phase/file/preview/build/deploy
    progress.
  - Requires an admin browser session and `Idempotency-Key`.

## Safety Boundary

The route layer maps typed `SiteBuilderSessionValidationError` and
`SiteBuilderSessionStorageError` values to bounded JSON responses. It does not
classify English error strings, expose raw runner/provider payloads, or return
private source/archive material.

The new route code stays Effect-native and does not add a new
`Effect.runPromise` bridge or route-level `Effect.promise` adapter.
