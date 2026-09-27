# WebSocket direct channels verification — September 27, 2026

The "WebSocket direct channels" item of
[issue #9719](https://github.com/OpenAgentsInc/openagents/issues/9719) carries
the [NIP-REACH](../../../nips/openagents/NIP-REACH.md#websocket-mapping) direct
channel over WebSocket.
[`crates/coder-reach`](../../../crates/coder-reach/README.md) adds the
`websocket` module, which presents a WebSocket connection as the byte stream
the handshake and channel already use, so TCP and WebSocket share one
handshake, one frame format, one sequence rule, and one set of bounds.
[`crates/coder-host`](../../../crates/coder-host/README.md) adds a WebSocket
listener to `coder host serve`, advertises it as a `websocket` hint, and
dials `websocket` hints from its client connector.

## Evidence class

All evidence is synthetic. Tests run on one macOS computer. Sockets bind
`127.0.0.1` port 0 and connect over loopback. The host test uses the
in-process relay fixture from `coder-control`. No TLS endpoint, forwarder
that terminates TLS, browser, second computer, LAN, or tailnet was involved.

## Checks that ran

Each command ran from the worktree with `CARGO_TARGET_DIR` set to the
worktree's own target directory and `CARGO_PROFILE_DEV_DEBUG=0`:

```sh
cargo test -p coder-reach
cargo test -p coder-host
cargo clippy -p coder-reach -p coder-host -p coder-computers -p coder-mobile -p coder --all-targets -- -D warnings
cargo fmt -p coder-reach -p coder-host --check
```

Results: `coder-reach` passed 22 unit tests, 11 TCP channel tests, 8
WebSocket channel tests, and 5 wire fixture tests. `coder-host` passed 10 unit
tests, the end-to-end scenario, 3 serve tests, and the WebSocket test.
Clippy reported no warnings for the two crates and their direct consumers,
and formatting matched. The `coder` crate's `host_serve` test did not run: the
disk filled during that build. The workspace gate did not run.

## Acceptance coverage

The WebSocket cases in `crates/coder-reach/tests/websocket.rs` mirror the TCP
cases in `tests/channel.rs`.

| Acceptance item | TCP test | WebSocket test |
| --- | --- | --- |
| Handshake success and sequenced, encrypted data | `handshake_succeeds_and_carries_sequenced_encrypted_data` | `handshake_succeeds_and_carries_sequenced_encrypted_data` |
| Split halves | `split_halves_keep_sequence_and_close_in_order` | `split_halves_keep_sequence_and_close_in_order` |
| Wrong host key | `wrong_host_key_is_refused` | `wrong_host_key_is_refused` |
| Replayed nonce | `replayed_nonce_is_refused` | `replayed_nonce_is_refused` |
| Revoked grant | `revoked_grant_is_refused_after_the_device_proves_its_key` | `revoked_grant_is_refused_after_the_device_proves_its_key` |
| Stale host generation | `stale_host_generation_is_refused` | `stale_host_generation_is_refused` |
| Oversized frame | `oversized_frame_is_refused_before_reading_its_body` | `oversized_frame_is_refused`: a prefix over the frame bound, and a message over the message bound refused from its WebSocket frame header |
| One frame per message | Not applicable | `a_message_must_carry_exactly_one_frame`, `websocket::tests::a_message_must_be_exactly_one_bounded_frame`, and the `websocket_messages` wire vectors in `tests/wire.rs` |
| Enroll, connect over WebSocket, run a terminal command, revoke closes the channel | Not applicable | `crates/coder-host/tests/websocket.rs` |

The `coder-host` test starts a host whose only direct hint is a `ws` URL, so
the connector reaches the host only by choosing the `websocket` hint. It
asserts that route, runs `printf` in a terminal over the channel and reads its
output, revokes the device in the access store, and sees the open channel
close with `revoked` and the supervisor block. A new WebSocket handshake then
receives the host's signed `revoked` verdict. The unit tests in `config`
check that the WebSocket listener follows the loopback rule and that an
advertised `ws` or `wss` URL is a `websocket` hint.

## Limits

- The WebSocket listener serves plain `ws`. No test covers `wss`: the client
  dials `wss` URLs through `tokio-tungstenite` with `rustls`, but no TLS
  endpoint ran.
- No browser or web client connected. The client that ran is the Rust
  connector in `coder-host`.
- Evidence is from one machine over loopback.
