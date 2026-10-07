# Brainstorm client

This Rust crate discovers a configured Brainstorm HTTPS origin and reads public
profile search and GrapeRank scores. It reimplements the public Open Ranking
contract without importing Brainstorm's server implementation.

Create `Client` with `Config`, then call `set_enabled(true)` when the caller
admits the integration. Construction, enablement, and disablement perform no
lookup. Call `discover`, `search`, or `rank` with a `Cancellation`. Disabling
cancels active and queued operations, including operations from an earlier
enablement generation. Configuration stays immutable; disable the old client
before replacing it with a client for different settings.

The defaults bound queries to 512 Unicode characters and 1 KiB, search to
10 results, rank to 20 keys, concurrency to two operations, and the entire
operation, including queue time and discovery, to 15 seconds. Response bodies
are limited to 256 KiB before accumulation; normalized output is limited to
64 KiB. You can reduce these limits. Production origins require HTTPS without
credentials or a path prefix. The client refuses all redirects and performs no
authentication fallback or automatic retry. `Retry-After` blocks another read
of that endpoint until its interval expires.

Each operation discovers advertised algorithms and the current house key.
Search enriches its results with one rank batch and preserves search relevance
order. Scores remain raw continuous values; zero has unknown coverage.
Missing or failed influence remains unavailable. Each response retains its
origin, endpoint, HTTP status, requested algorithm, times, and exact request
and response body digests. The separately discovered house identity is an HTTPS
observation, not a signed score or an atomic binding to the effective observer.

`Observation::model_context` retains provenance and explicitly omits subjects
to fit an 8 KiB allowance. The caller must include that allowance in its full
route budget. `TaskCache` is optional storage owned by the current task; the
client keeps no global query history. Service TTLs and HTTP cache controls bound
freshness, with a five-minute local maximum. Missing score TTL means no caching.
Discovery without an HTTP TTL uses the local freshness policy. A refetch is a
new observation.

All acceptance tests use a local fake HTTP server through a test-only
constructor. Run `cargo test -p brainstorm-client` through the supported build
lease and `cargo fmt -p brainstorm-client`. Live deployment qualification
remains in [the owner checks](../../NEEDS_OWNER.md).
