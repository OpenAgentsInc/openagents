# Aethyrforge host protocol

Public Rust wire types, lifecycle validation, browser host, and engine-side SDK.
This crate contains no renderer, editor, engine assets, private dependency, or
engine license grant. Aethyrforge remains proprietary. OpenAgents contributors
can build and test this crate with no Aethyrforge checkout or credentials.

- [Wire schema](schema/host-v1.json)
- [Protocol and ownership](../../docs/verse/aethyrforge-host-protocol.md)
- [Synthetic host](examples/host.html) and [engine](examples/mock.html)

Run `cargo test --locked -p aethyrforge-host`. Build the browser SDK with
`scripts/build-aethyrforge-host.sh ABSOLUTE_OUTPUT_DIRECTORY`. It requires the
pinned Rust toolchain, the `wasm32-unknown-unknown` target, and the wasm-bindgen CLI
version matching Cargo.lock. Build output stays outside Git. The generated JS is
Wasm loader glue; browser transport and lifecycle logic are Rust.

For browser conformance, serve the output twice with
`python3 crates/aethyrforge-host/examples/serve.py OUTPUT --port PORT`. The fixture
defaults are host port 55306 and engine port 55307. Use available local ports if
those are occupied: serve the mock with `--host-origin HOST_ORIGIN`, set
`AETHYRFORGE_TEST_HOST`, and set `AETHYRFORGE_TEST_MOCK` to the mock URL. With Node and Playwright installed, run:

```sh
openagents browser run -- node scripts/check-aethyrforge-host.mjs
```

If Playwright is outside normal module lookup, set `NODE_PATH` to its installation.
`AETHYRFORGE_TEST_ENGINE` selects an explicitly approved private artifact instead
of the public mock. Private-artifact checking is optional and never runs during
Cargo tests or the public build. See the protocol document for its current limits.

The fixture HTTP server has permissive CORS for this public SDK. It is a local
test tool, not a production engine deployment or a place to serve private assets.
