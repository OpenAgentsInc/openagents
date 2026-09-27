# Host CAP/CJ binding verification — September 27, 2026

[Issue #9719](https://github.com/OpenAgentsInc/openagents/issues/9719) lists
the CAP/CJ binding of [NIP-HOST](../../../nips/openagents/NIP-HOST.md) as
open. This record covers that item. The resident host in
[`crates/coder-host`](../../../crates/coder-host/README.md) now advertises a
NIP-CAP operation set and answers NIP-HOST requests that arrive as NIP-CJ
execution requests, through the same admission as the direct artifact
binding. [`coder_access::cj`](../../../crates/coder-access/src/cj.rs) holds
the definition, the binding checks, and the client path.

## What was built

- Two JSON Schema 2020-12 documents,
  [`host-call.v1.json`](../../../nips/openagents/schemas/host-call.v1.json)
  and [`host-answer.v1.json`](../../../nips/openagents/schemas/host-answer.v1.json):
  the CJ input and output, each carrying one exact signed `3188` artifact.
- The `host-access` NIP-CAP `adapter` definition: transport `nostr-cj`,
  interface `openagents.host-request.v1`, all 11 NIP-HOST operations, the
  host key as worker, the served relays, and SchemaRefs that pin the two
  schemas by digest and size. `coder host serve` signs it as a `kind:30180`
  with `d` = `host-access` and publishes it on every relay it serves. A
  reference definition for a placeholder host key is checked in at
  [`crates/coder-access/fixtures/host-access-capability.json`](../../../crates/coder-access/fixtures/host-access-capability.json).
  It is not under `capabilities/`, because that directory holds executor
  manifests that the `coder` delegation registry loads and probes.
- The host side: `cj::intake` opens a `kind:25920` request with the shared
  `nostr::execution` parser, requires every execute field to be the value
  the definition and the embedded request fix, validates the input against
  the pinned schema, and requires the CJ signer to be the request's signer.
  `coder-host`'s `serve/cj.rs` passes the embedded request to the same
  `host_request` function the relay artifact and direct-channel bindings
  call, and seals the reply in a `completed` `kind:26920` result.
- The client side: `cj::fetch_capability` reads the manifest only from the
  pinned host key and accepts only the exact definition the binding builds.
  `Client::call_cj`, `send_cj`, and `exchange_cj` send an operation, bind the
  result to the request with `nostr::execution::bind_worker_event`, check
  the output against the pinned answer schema, and verify the reply with the
  same `verify_reply` as the direct artifact binding.

## Evidence class

All evidence is synthetic, from one macOS 26.4 arm64 computer with Rust
1.97.1. The relay is the shared synthetic NIP-42 relay fixture in
`crates/coder-control/src/tests/relay.rs`, which now also stores
`kind:30180` heads and lets any authenticated reader fetch them. It is not
the production relay. No second computer, device, or production relay took
part.

## Acceptance checks

`crates/coder-host/tests/cj.rs` starts a real host with `coder_host::start`
and a recording task owner, enrolls devices by invitation, and reads the
capability with `fetch_capability`, as a device does.

| Case | What the test asserts |
| --- | --- |
| Granted operation | A `standard` device's `task.create` returns `dispatched task.create` through the relay artifact binding and through CJ; the task owner records both. |
| Missing right | An `observe`-only device's `task.create` refuses as `missing_right` naming `operate` through both bindings. |
| Revoked grant | After the device is revoked and re-enrolled, a request that names its old grant refuses as `revoked` through both bindings. |
| Stale epoch | A request that names the new grant at epoch 0 refuses as `stale` through both bindings. Neither refusal reaches the task owner. |
| Duplicate request ID | One prepared request sent through the relay artifact binding and then twice through CJ returns the same signed reply event each time, and the task owner records one task. The same request resealed into other event bytes refuses as `conflict` through both bindings. |
| CJ `task.create` round trip | `send_cj` returns a `dispatched` receipt whose reference is the task the owner recorded, with the device's title. The task is `queued`: the completed CJ result is a handling receipt, not a finished task. A retry returns the same receipt and records nothing new. An `access_read` device's `device.list` over CJ shows the host saw the device. |
| Binding refusals | A second key that wraps the device's signed request in its own CJ job is refused as `not_admitted`, and a target whose digest is not the host's definition is refused as `identity_mismatch`. Neither reaches NIP-HOST admission; the untouched request then dispatches once. |

`crates/coder-access/tests/capability.rs` checks that the reference
definition equals the one a host builds, passes `nostr::cap::parse_definition`
as an `adapter` over `nostr-cj` with `request_attempt` idempotency and all 11
operations, pins the checked-in schemas by digest and size, and that both
schemas load in `nostr::contracts::prepare_closure`. It also checks that a
signed manifest passes `nostr::cap::check_discovery_tags`, reads back only
under its own host key, and that a host-signed manifest with another
definition is refused.

## Commands

```sh
export CARGO_TARGET_DIR="$PWD/target" CARGO_PROFILE_DEV_DEBUG=0
cargo test -p coder-host --test cj
cargo test -p coder-access -p coder-host -p coder-control
cargo test -p coder-access --no-default-features --test capability
cargo clippy -p coder-access -p coder-host --all-targets -- -D warnings
cargo clippy -p coder-access -p coder-host --no-default-features --lib -- -D warnings
cargo fmt -p coder-access -p coder-host --check
```

All passed. The `coder` crate's `tests/host_serve.rs` was not run; it
shares the acceptance scenario that `crates/coder-host/tests/end_to_end.rs`
ran and passed with the CJ loop active.

## Limits

- The operation answers within the request, so the host sends no `accepted`
  or progress feedback and ignores status, replay, and cancel controls.
- Execution kinds are ephemeral. A request sent while the host reconnects
  its subscription, about every 110 seconds, is not replayed; the device
  times out after 12 seconds and retries the same request.
- The binding carries each NIP-HOST request as-is, so every NIP-HOST bound
  still applies. The CJ parser also caps the input at 128 KiB, which holds
  every current operation: the largest, a 16 KiB prompt, seals to about
  22 KiB.
- The host publishes its manifest once at start. A relay that is down then
  receives it after three publication attempts or not at all; the next
  start publishes again.
- NIP-TERM requests have no CJ binding; `terminal.open` over CJ returns the
  terminal ID, and the device attaches over a direct channel or relay
  artifacts.
