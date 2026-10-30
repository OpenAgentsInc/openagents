# Relay connection for native clients (2026-10-30)

`coax/lib/net/relay_conn.coax` is a reusable client connection to a Nostr
relay over `ws://` or `wss://`. It replaces the test-only connect loop in
`coax/tests/net/relay_e2e.coax` (`rx_open`, `rx_send`, `rx_send_close`,
`rx_fill`, `rx_recv`, `rx_draw`, `rx_shut`). The Psionic Pylon's
`py_open`/`py_send`/`py_recv`/`py_pump` can switch to it. That switch belongs
to the Pylon's owner and was not made here.

## What was built

| File | What |
| --- | --- |
| `coax/lib/net/relay_conn.coax` | The I/O. `RelayConn` holds the handle, the `tls` flag, the ws reader `rd`, the input buffer `inb`/`used`, the last text message `text`/`text_n`, the last pong `pong`/`pong_n`, the `pings` and `pongs` counts, the peer's `close_code` and `eof`. It calls `cxsock_*`, `cxtls_*` and `cxrand_*` and carries out what the rules return. It has no checks of its own (every procedure performs effects), so it isn't `#authority`. The e2e covers it |
| `coax/lib/net/relay_conn_rules.coax` (`#authority`) | The decisions. `rc_scan` says what the next server frame asks for: a text message, a ping to answer (with its payload range), a pong, a close (with the peer's code), more bytes, or a refusal with the close code to send. Binary messages get 1003, a frame or message over the cap gets 1009, and other violations get ws.coax's codes. It also has `rc_close_reply` (echo the peer's code, or 1000 for 1005), `rc_read` (what a transport read result means), `rc_wait` (time left until a deadline), `rc_room` (buffer bound) and `rc_target` (request target, with `/` before an empty path or a bare query). 5 checks of its own, 28 with ws.coax's and sha1's |

API:

```
relay_conn_open(c: [] RelayConn, uri: string) -> bool
relay_conn_send(c: [] RelayConn, text: string) -> bool
relay_conn_poll(c: [] RelayConn, timeout_ms: int) -> int   // RC_TEXT, RC_NONE, RC_CLOSED, RC_FAIL
relay_conn_close(c: [] RelayConn, code: int) -> bool
relay_conn_ping(c: [] RelayConn, payload: string) -> bool  // addition: the e2e pings the relay
relay_conn_drop(c: [] RelayConn)                           // addition: close at once, no handshake
rc_draw(n: int, out: [] u8) -> bool                        // OS bytes through the entropy bridge
```

- **open:** `relay_uri` parses the URL (IP literals are refused, as uri.coax
  decides). It runs TCP connect, then `cxtls_start` for wss://, which checks
  the chain and the host name with no option to skip either. Then it sends
  the RFC 6455 request with a 16-byte nonce from `cxrand_fill`, and
  `ws_response` admits the 101 and Sec-WebSocket-Accept. Connect, TLS and the
  whole handshake share one 5 s deadline, so a server that trickles its head
  can't hold the connection open. Any failure returns false with the
  connection closed.
- **send:** one masked text frame with a fresh mask key, up to 128 KiB. A
  write failure closes the connection.
- **poll:** reassembles fragments, answers pings with pongs, and records
  pongs. A peer close is echoed and returns `RC_CLOSED` with `close_code`. A
  clean end of stream returns `RC_CLOSED` with `eof`. A refusal is answered
  with its close code and returns `RC_FAIL`. The whole wait is bounded by
  `timeout_ms`: 0 means only what is ready, and a negative value waits
  without limit.
- **close:** sends close with `code`, then waits up to 2 s for the peer's
  close frame and end of stream. Text that arrives in that time is
  discarded. Then it closes the transport.
- **Left to the caller:** reconnecting, backoff, generations and
  resubscription stay in `relay.coax` (`rl_start`, `rl_opened`,
  `rl_dropped`). The e2e driver now holds the generation in its stats.
- **Limits:** 128 KiB for one message and one frame. The input buffer holds
  one partial frame plus one transport read (`RC_MSG_MAX + 14 + 65536`).
  `RelayConn` is about 450 KB, so keep it in static or caller storage.

## The e2e and its fixture

`relay_e2e.coax` now loads `relay_conn.coax`, and its script and its 24
checks are the same. Some of the code under them changed:

- The URL is `ws://localhost:$RX_PORT`, or `wss://` when `$RX_TLS=1`. It has
  no path, so `rc_target` sends `/`.
- The ping and pong checks read `RelayConn.pings`, `pongs` and `pong`.
- The last three checks read `close_code` and `eof` after
  `relay_conn_close`.
- A relay-initiated close, which the script never sends, now also drops the
  connection.

`relay_fixture.py` used to check that the request's `Host` header starts
with `127.0.0.1:`. It now checks that it is exactly `localhost:<port>`,
because relay_uri refuses IP literals. The fixture still binds 127.0.0.1.

`relay_e2e.py` now links `tls-native.c`, with OpenSSL from Nix through
`tls-env.sh`, and builds with `-DCXTLS_TEST_TRUST`. The default C compiler is
now `gcc`. It runs the driver three times:

1. **ws://:** the full script.
2. **wss://:** the full script, against the same fixture with its listening
   socket wrapped by Python's `ssl`. openssl generates a one-day CA and a
   localhost leaf for each run, the driver trusts only that CA through
   `CXTLS_TEST_CA`, and none of it is committed. The fixture ends
   connections with `shutdown(SHUT_RDWR)`. Over TLS that is a TCP close
   without close_notify, which `cxtls_read` correctly reports as an error
   rather than an end of stream. So the harness's `ClosingSSLSocket` sends
   close_notify first (`unwrap`), as a TLS relay does.
3. **wss:// with the system trust store:** `CXTLS_TEST_CA` is unset. Every
   connection must be refused before the relay sees a WebSocket handshake.

`--ws` runs only the first. No public relay is contacted. `tls.sh --public`
remains the opt-in public check.

## Results

All runs were on Linux under load (about 60 to 70 on 28 cores), with
gcc 15.2.0 and OpenSSL 3.6.3. The compiler was the `coax/bin` from the
oa-nip01 worktree (symlinked, not rebuilt).

| Check | Result |
| --- | --- |
| `coax test coax/lib/net/relay_conn_rules.coax` | 28/28 passed (5 its own, 23 ws.coax's and sha1's) |
| `coax mutate relay_conn_rules.coax --jobs 2 --shard I/4`, 4 shards of 6 to 8 s, merged | 1140 mutants, 1140 killed, 0 survived, no exemptions. This includes ws.coax's and sha1.coax's mutants, which the unit loads |
| `python3 coax/tests/net/relay_e2e.py` (ASan+UBSan) | ws://: `relay e2e: ok`, 24/24 checks, 3 handshakes, 10 masked frames, all mask keys distinct, 1.8 s. wss://: `relay e2e: ok`, the same counts, 2.3 s. Untrusted wss://: refused, 0 handshakes. `relay_e2e: ok` |
| `coax/tests/net/tls.sh` (unchanged; run because `relay_fixture.py` changed) | `tls.sh: all local checks pass` (11 PASS lines and `RELAY ok`) |

The first mutation run left 3 survivors, and none was marked equivalent:

- `rc_target`'s `out.count < 1` survived as `< 2` and as `<= 1`. A new
  check uses one-byte and two-byte output buffers.
- `rc_wait`'s `deadline > now` survived as `>=`. At equality both forms
  return 0, so the line was rewritten as `left >= 1`, where the boundary
  changes the result.

The rerun on the final file (sha256 `5980671a…6c819a`) killed all 1140.

## Not done

- **The Pylon switch-over** (`psionic/lib/pylon/run.coax`) belongs to the
  Pylon's owner. Note that `relay_conn.coax` declares the `cxsock_*`,
  `cxtls_*` and `cxrand_*` foreign imports itself, so a program that loads
  it must drop its own copies of those declarations and link
  `tls-native.c` with OpenSSL.
- **Apple:** `tls-native.c` returns `CXTLS_UNSUPPORTED` there, so wss://
  opens fail on Apple and ws:// works.
- **Close timing:** when the peer starts the close, poll echoes it and
  closes TCP at once. It doesn't wait for the server to close first, which
  RFC 6455 7.1.1 prefers. This has no effect on Nostr semantics.
- **Inherited:** ws.coax still sends `Host: name:443` for wss on the default
  port, as described in the 2026-10-29 TLS record.
