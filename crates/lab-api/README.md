# Labs API

`Api` mounts the shared `/v1/labs` routes with an injected `Authorizer`,
space-scoped storage backend, and kind registry. The web adapter uses fresh
account credentials and current project roles. Kind adapters keep checks and
changes in their existing Rust domain crates. Portable HTTP values live in
`lab-kit::api`, so SDK clients need no server or database dependency.

Run `cargo test -p lab-api`. The synthetic Echo tests cover every route,
conflicts and refusals, cross-account/project reads and writes, job retry
identity, source uploads and file ranges, cross-kind references, and access
revocation during an event stream.

Generate the contract with `cargo run -p lab-api --example openapi`.
See [the caller contract](../../docs/sdk/labs.md) for limits and retry behavior.
