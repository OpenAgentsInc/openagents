# SDK language boundary decision

The architecture decision the [specification](../api/decision-api.md#cli-and-language-clients)
requires before supported Python and Go SDK distributions ship: whether
non-Rust clients are a Rust-owned core reached through bindings, or
native packages under a language-policy exception.

Status: decided. Distributions are not yet built — this record is what
the build is gated on.

[Documentation index](../README.md)

## Decision

Supported Python and Go SDKs are **thin native packages** — each
written in its own language's idiom over the documented HTTP contract —
under an explicit product-language policy exception recorded here. They
are product code, not infrastructure, and the exception names them as
such rather than letting them grow as unmarked second implementations.

A Rust-owned core/binding is the recorded fallback, not the choice.

## Context

The contract a client implements is small and fully specified:
`POST /v1/systemone` and `POST /v1/classify` plus the read routes,
typed errors in `{"error": {"code", "message"}}`, the `(request,
attempt)` idempotency pair, bounded retries honoring `Retry-After`, and
envelope schemas versioned in-band. None of it is stateful across calls;
there is no streaming, no session, no connection machinery the wire
does not show.

What a binding cannot buy is correctness of that contract — and what
three implementations can lose is its drift. The deciding factor is
therefore whether parity is testable, and it is: the shared contract
fixtures in `../fixtures/` record each case's request, response, and
client-visible expectation in a language-neutral shape.

## Why thin native packages

- The surface is one POST plus typed reads. A Rust core would add FFI
  tooling, shared-object or wheel packaging, and a second ABI to verify,
  to reimplement semantics that fit on one documented page.
- The risky semantics — bounded retry with `Retry-After`, the
  idempotency pair, typed-error mapping, partial-failure decode — are
  exactly what the conformance fixtures encode. Each SDK's own test
  suite replays the same fixtures against its own stub before
  publication, the way `crates/jev/tests/contract.rs` and
  `crates/oak/tests/fixtures.rs` do today.
- Native packages install and debug natively: `pip`, `go get`, their
  own vendoring and their own security advisories — no Rust toolchain
  in a caller's build.

## What the exception requires

Each thin SDK is a product implementation and is held to the same bar
as `jev`:

- It implements the shared retry rules — bounded, `Retry-After`
  honored, the `Idempotency-Key`/`x-attempt` pair on retries, a
  daily-quota refusal terminal.
- Its test suite replays every fixture whose route it speaks; coverage
  gaps are named in its documentation, not skipped silently.
- It carries no second product language's code and exposes no
  surface the contract does not define.
- Its version follows the catalog's `version` field, and its release
  notes say which catalog version it covers.

## Revisit triggers

The decision reverses to a Rust-owned core/binding when any of these
becomes true:

- The contract gains behavior fixtures cannot verify — streaming,
  connection-held sessions, stateful resumption.
- A supported SDK's fixture replay diverges and stays divergent across
  a release.
- A third product language is requested; at that point one shared core
  costs less than a fourth implementation.

Until then, examples remain documentation and do not count as SDK
delivery — see the release-readiness section of
[`../guides/clients.md`](../guides/clients.md).
