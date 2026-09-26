# Client packages

The supported clients, what each covers, and how they version. The
supported client for the Decision API is Rust — `crates/jev` for code,
`crates/oak` for the command line, `oak-mcp` for agents. Every other
language reaches the same contract over HTTP directly; the runnable
examples in [`../examples/`](../examples/) are documentation for that,
not SDKs.

[Documentation index](../README.md)

## Integration matrix

| Surface | `jev` (Rust SDK) | `oak` (CLI) | `oak-mcp` (MCP) | HTTP |
| --- | --- | --- | --- | --- |
| Native decisions — `POST /v1/systemone` | `system_one`, `BlockingClient::system_one` | `oak ask`, `--input lines|ndjson` batch | — | [`ask.py`, `ask.go`, `ask.js`, curl](../examples/) |
| Classification — `POST /v1/classify` | `classify().run`, `classify().run_raw` | `oak classify --envelope` | `classify` tool | envelope over HTTP |
| Review and fallback | `ClassifyItem.review_status`, `ClassifyUnit.review`, `.fallback` | the report verbatim | the report verbatim | report fields |
| Durable jobs — `POST /v1/jobs`, `GET /v1/jobs/{id}`, cancel, results, delete | `jobs().submit`, `.status`, `.cancel`, `.results`, `.remove` | — | — | over HTTP |
| Account and usage — `GET /v1/account`, `/v1/balance`, `/v1/usage`, `/v1/session` | `account().details`, `.balance`, `.usage`, `.session` | — | — | over HTTP |
| Identity — `GET /v1/models` | `models().list` | `oak models` | `list_models` tool | `GET /v1/models` |
| Retries | `RetryPolicy` — bounded, honors `Retry-After`, a predicate bounds which statuses retry | `--retries` — bounded, honors `Retry-After`, keeps `Idempotency-Key` and bumps `x-attempt` | the transport's own | the caller's |
| Typed errors | `Error::Api` + `ApiErrorKind` | exit codes — `1` failure, `3` refused, `4` unavailable, `5` mixed — plus `code: message` on stderr | JSON-RPC error | `{"error": {"code", "message"}}` |

An empty cell is a named gap, not an oversight: `oak` speaks no jobs
routes and the MCP server exposes inference only. `jev` is the client
that carries the full surface.

## Contract fixtures

`../fixtures/` holds one recorded exchange per contract case — partial
classification failure, null reviewer confidence, idempotency conflict,
key revocation — in a `{request, response, expect}` shape any client
replays. `crates/jev/tests/contract.rs` and `crates/oak/tests/fixtures.rs`
replay them today; a new SDK's test suite replays the same files before
it is called supported.

## Package and version support

| Package | Form | Versioning |
| --- | --- | --- |
| `jev` | a crate in this workspace | the workspace version; the public surface is pinned by `src/lib.rs`'s snapshot — adding or removing a name fails the surface test |
| `oak`, `oak-mcp`, `oak-mcp-http` | checksummed binaries | the workspace version, reported by `oak version`; each release artifact carries a manifest naming its commit and toolchain |
| HTTP contract | `docs/agents/api-catalog.json` | the catalog's `version` field is what `x-api-version` emits on every response; envelope schemas version in-band (`openagents.classify.v1`) |

### Authentication

Credentials never belong in a flag that exposes them in the process list.
`jev` accepts an explicit typed key in `Client::new` or reads
`TYPESAFE_API_KEY` through `Client::from_env`; it does not load a config file
on its own. `oak` also supports a `0600` config file and reads `OPENAGENTS_API_KEY`,
`OPENAGENTS_BASE_URL`, `OPENAGENTS_MODEL`, `OPENAGENTS_WORKSPACE`, or the
same keys in `--config`'s JSON file.

### Timeouts and cancellation

`jev` applies a per-attempt timeout — 10 seconds by default, overridden
by `Config::timeout` for the client or `CallOptions::timeout` for one
call — and honors `tokio` cancellation at the await point. `oak` runs a
bounded per-attempt `--timeout` (60 seconds by default) and exits on
SIGINT. A cancellation abandons the HTTP exchange; the service's
`(request, attempt)` idempotency means a safe retry is the same pair
again, not a new key.

### Serialization

Every body is JSON. Schemas the contract versions carry a `v` field;
fields a client does not know are ignored on decode, never an error.
The SDK types expose the reconciliation surface — outcomes, counts,
typed errors — and carry the door's own documents (`policy`, `served`,
unit `raw`, `selected`) as sent, so a new field never breaks an old
client.

### Retries

Retry policy is shared across every supported client:

- Bounded: `jev`'s `RetryPolicy` and `oak`'s `--retries` both count
  attempts and stop.
- `Retry-After` and `retry-after-ms` are honored.
- The idempotency pair is caller-named: `oak`'s `--request-id` or
  `jev`'s `CallOptions::idempotency_key` sets `Idempotency-Key`, and
  every attempt then carries `x-attempt` (one-based), so a retry can
  never double a settled call. With no key, the service mints a request
  id per attempt.
- A daily-quota refusal (`quota_exhausted` without a `Retry-After`) is
  terminal, never retried indefinitely.
- Typed errors the caller cannot fix — `unauthenticated`, envelope
  faults — are not retried.

## Install and update

`./scripts/package-oak.sh` builds the three binaries and writes
`dist/clients/`:

- `oak-<version>-<target>.tar.gz` — the binaries.
- `oak-<version>-<target>.manifest.json` — the commit, toolchain, and
  target the archive was built from.
- `SHA256SUMS` — checksums over both artifacts.

To install from a release artifact:

```bash
cd dist/clients
shasum -a 256 -c SHA256SUMS
tar -xzf oak-<version>-<target>.tar.gz -C ~/.local/bin
oak version
```

An update is the same steps with the newer archive — verify the
checksums, untar over the same directory, run `oak version` to confirm.
Signed provenance is a release-process layer over `SHA256SUMS` where
the operator's process supports it; the packaging script does not sign.
Publishing these artifacts and submitting packages to registries are
explicit release operations, not part of a build.

## Release readiness

The supported surface today is Rust and HTTP. The Python and Go SDK
distributions the product specifies are **not shipped**: the
architecture decision for them is recorded in
[`../service/sdk-language-decision.md`](../service/sdk-language-decision.md),
and until thin native packages exist and replay the shared fixtures in
their own test suites, release notes and the matrix above continue to
name HTTP as the Python, Go, and JavaScript path. Documentation
examples are not SDK delivery.
