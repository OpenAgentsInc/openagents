# coder-reach

`coder-reach` implements [NIP-REACH](../../nips/openagents/NIP-REACH.md): how a
client finds the hosts its owner runs, judges which are online and
compatible, orders the routes it may try, and opens a direct channel that
authenticates both Nostr keys before any data flows. None of it grants access.

## Modules

| Module | What it does |
| --- | --- |
| `directory` | The owner host directory: owner-signed, owner-encrypted, revisioned. A host-signed copy refuses. |
| `presence` | Host presence with bounded telemetry, receipt-time freshness, generation rollback refusal, and the compatibility rule. |
| `hints` | Reachability hints, address validation by class, and selection that never offers loopback to another machine. |
| `channel` | The frame format and the direct-channel handshake over any ordered byte stream. |
| `websocket` | The WebSocket mapping: `WebSocket` presents a WebSocket connection as that byte stream, one frame per binary message, with the message bound checked from each WebSocket frame header. `accept` and `client` run the upgrade with those limits. |
| `placement` | The pure placement rule over directory weights and fresh presence. |
| `artifact` | Sealing and opening bodies as private `3188` artifacts through `nostr::private_artifact`. |

## Grant checks

The channel asks one question of the host's grant store through the
`channel::GrantCheck` trait: is this grant ID, at this epoch, the device's
current, unrevoked grant? The crate does not depend on the grant store. The
host service supplies the implementation and rechecks grants per operation.

## Limits

- The handshake and frame tests run over TCP (`tests/channel.rs`) and over
  WebSocket (`tests/websocket.rs`). The crate opens no TLS connection: a
  `wss` hint needs a TLS stream from the caller. The client in
  [`coder-host`](../coder-host/README.md) opens one.
- The crate does not publish to relays, carry relay control, enroll devices,
  retry connections, or draw screens. The resident host in
  [`coder-host`](../coder-host/README.md) does the publishing, serving, and
  retrying, and `channel::Channel::into_split` lets it read and write one
  channel from separate tasks.
- Presence freshness uses the caller's clock. The caller records receipt time.

## Checks

```sh
CARGO_TARGET_DIR=target-reach cargo test -p coder-reach
CARGO_TARGET_DIR=target-reach cargo clippy -p coder-reach --all-targets -- -D warnings
cargo fmt -p coder-reach --check
```
