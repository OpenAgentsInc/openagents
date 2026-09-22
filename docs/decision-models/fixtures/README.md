# Contract fixtures

One recorded exchange per contract case, shared by every supported
client. A client replays the fixture's `response` — against a stub or a
fixture-aware harness — and asserts what `expect` names, so the file and
its meaning cannot drift apart silently. These are contract fixtures,
not captures of a deployment: identities and digests are placeholders.

[Documentation index](../README.md)

## The shape

```json
{
 "name": "the case",
 "description": "what a conforming client does with it",
 "request": {"method": "…", "path": "…", "headers": {}, "body": {}},
 "response": {"status": 200, "headers": {}, "body": {}},
 "expect": {"kind": "classify-report|api-error", …}
}
```

`expect` carries the client-visible truth of the case — outcome names,
counts, error codes, and whether the failure is retryable — not the
private details of any one SDK's error type.

## The cases

| Fixture | Contract |
| --- | --- |
| [classify-partial-failure](classify-partial-failure.json) | A `mixed` classify report is a success: per-item outcomes are named with their causes, in input order, never dropped and never an exception. |
| [classify-review-null-confidence](classify-review-null-confidence.json) | A review record whose reviewer raw answer carries `confidence: null` decodes whole — the record is the door's, not the client's to reject. |
| [jobs-idempotency-conflict](jobs-idempotency-conflict.json) | Changed content under a settled `Idempotency-Key` is a 409 `idempotency_conflict` — a typed error, never a second run, never retryable. |
| [key-revocation](key-revocation.json) | A revoked key is a 401 `unauthenticated` on every route — a typed refusal, not a transport failure, not retryable. |

## How each client replays them

- **Rust (`jev`)** — `crates/jev/tests/contract.rs` serves each
  fixture's recorded response on its recorded route and asserts the
  `expect` block against the decoded report or typed error.
- **`oak`** — `crates/oak/tests/fixtures.rs` serves the same responses
  and runs the real binary at them: `models` for the revocation row,
  `classify --envelope` for the report rows. The jobs fixture is `jev`'s
  — `oak` speaks no `/v1/jobs` — and the test names that rather than
  skipping it.
- **HTTP examples** — the Python, Go, JavaScript, and curl examples in
  `../examples/` send the same request shape and read the same response
  fields; a fixture's `request` is what they send and its `response` is
  what a stub answers.
