# Compatibility and deprecation

How the public API changes without breaking callers, and the notice a
breaking change owes them.

## What is the public API

Everything `docs/agents/api-catalog.json` lists as `implemented`:
routes, their request and response envelopes (each versioned by its `v`
field), the refusal-code set, response headers (`x-request-id`,
`x-attempt`, `x-outcome`, `x-receipt`, `x-api-version`, `retry-after`),
and the CORS behavior. The catalog's `unimplemented` list is explicitly
not covered — a caller that depends on an advertised-but-absent surface
has no compatibility claim.

## Compatible changes — no notice required

These may ship in any release:

- New routes, new optional request fields, new response fields.
- New refusal codes (callers must handle unknown codes as typed
  refusals, not crash — the catalog's refusal shape is the contract).
- New envelope schemas under a new `v` value.
- Tighter validation only where the old behavior was undocumented.

## Breaking changes — announced migration windows

Removing or renaming a route, changing a `v` schema's required fields,
changing a refusal code's meaning, or changing auth requirements is a
breaking change. A breaking change to a supported public API announces
a migration window of **at least six months** before the old behavior
stops being served. The announcement lives in the catalog — a
`deprecated` status on the route with the successor named — and the
`sunset` date the old behavior ends.

A route may serve the old and new shapes side by side during the
window; the `v` field is the mechanism that lets both exist.

## Security exceptions

A change that closes a security hole may ship without the window —
credential handling, auth bypasses, and disclosure fixes are the
category. The exception is documented in the release notes with the
reason the window could not hold; "convenience" is never a security
exception.

## Safe aliases

No route aliases exist today, and callers should not assume any —
`/v1/systemone` is the only name for the decision call, and a `404` is
the answer a misspelled or legacy path earns. If an alias ever ships,
it is documented in the catalog as a first-class route entry, not a
silent redirect.
