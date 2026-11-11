# The room's WebTransport server

Issue: #13175, part of #13164. The design review is
[#13164's comment](https://github.com/OpenAgentsInc/openagents/issues/13164#issuecomment-6796318419).

The room server can accept WebTransport sessions from Chrome: a session,
QUIC datagrams inside it, and bidirectional and unidirectional streams. The
transport and crypto are vendored C libraries. The WebTransport session layer
on top of them is ours.

## What is where

| Piece | Source | Lines |
| --- | --- | --- |
| QUIC (RFC 9000, 9001, 9002), datagrams (RFC 9221), congestion control, loss recovery | ngtcp2 1.26.0, `coax/web/room/third_party/ngtcp2/` | 33,974 |
| HTTP/3 and QPACK, extended CONNECT, the H3_DATAGRAM setting | nghttp3 1.19.0, `third_party/nghttp3/` | 19,564 |
| TLS 1.3, ECDSA P-256 signing, X25519 and P-256 key exchange, AES-GCM, ChaCha20-Poly1305 | picotls with its minicrypto binding (cifra, micro-ecc), `third_party/picotls/` | 15,051 |
| The WebTransport session layer and the UDP socket | `coax/web/room/wt.c`, `wt.h` (ours) | 700 + 67 |
| The echo server, the certificate and the Chrome check | `wt-echo.c`, `wt-cert.sh`, `wt-check.sh`, `wt-check.coax`, `wt-check.js` (ours) | 273 |

Exact versions, download URLs, hashes and licenses (MIT, CC0, BSD 2-clause)
are in `third_party/VERSIONS`. `third_party/build.sh` builds the three into
one archive with the host's C compiler and nothing installed, so the Mac, the
build hosts and the room's image build it the same way.

The C is a stated, firm exception to "tools in Coax", for transport and crypto
maturity. Everything above the transport interface (`aethyr/net`) stays Coax.

### Why picotls with minicrypto

ngtcp2 needs a TLS stack with a QUIC interface. Its picotls backend uses
picotls's OpenSSL binding, but the room's image is Debian bookworm (OpenSSL
3.0, no QUIC API) and the rule is no system installs. picotls's minicrypto
binding is plain C with no dependency. It has the one certificate type Chrome
needs for `serverCertificateHashes` (ECDSA P-256). It is slower than OpenSSL's
AES, which doesn't matter at a room's few kilobytes a second per player. The
backend file is upstream's picotls backend with the binding swapped
(`third_party/PATCHES.md`).

## What nghttp3 already does, and what we wrote

nghttp3 already does:

- the control and QPACK streams, SETTINGS, and request and response headers;
- `SETTINGS_ENABLE_CONNECT_PROTOCOL` and `SETTINGS_H3_DATAGRAM`, and it
  accepts a request with `:protocol` (RFC 9220);
- a response whose body is produced on demand, so a 200 to CONNECT can leave
  the stream open.

It has no WebTransport, so `wt.c` does:

- **The settings Chrome looks for.** nghttp3 can't add a setting, so a
  four-line patch sends `SETTINGS_ENABLE_WEBTRANSPORT` (draft 02) and
  `SETTINGS_WEBTRANSPORT_MAX_SESSIONS` (draft 07) when extended CONNECT and
  datagrams are on.
- **The session.** A request with `:method CONNECT` and `:protocol
  webtransport` asks the host (`events.session`) and gets 200 with
  `sec-webtransport-http3-draft: draft02`, or 404. The session id is the
  CONNECT stream's id.
- **The capsule protocol.** The CONNECT stream's body is parsed as capsules. A
  `CLOSE_WEBTRANSPORT_SESSION` ends the session; others are passed over.
  `wt_close` sends one, then ends the stream.
- **Datagrams.** A QUIC DATAGRAM's payload begins with the session's quarter
  stream id (RFC 9297). Incoming ones are routed by it; outgoing ones get it
  prefixed. Each waits in a bounded queue for the congestion window, and the
  host can drop its own waiting ones (`wt_drop_waiting`) when newer state
  replaces them.
- **Streams.** A peer stream that begins `0x41` (bidirectional) or `0x54`
  (unidirectional) and a session id is the session's, and nghttp3 never sees
  it. Every other stream goes to nghttp3. Streams we open get the same
  header. Bytes of ours are kept until QUIC acknowledges them.
- **Flow control.** Every byte the session layer hands the host is credited
  back to the stream and the connection at once.
- **The socket.** One non-blocking UDP socket, read and flushed from the host's
  own poll loop (`wt_fd`, `wt_read`, `wt_flush`, `wt_timeout_ms`). Nothing
  starts a thread or waits.

The first run against Chrome found one bug outside our code: picotls's
OpenSSL binding needs an init call on an AES-ECB context and minicrypto's has
none. The swapped backend called a null pointer on the first Initial packet.
The fix initialises only a context that has an init (`PATCHES.md`).

## The certificate

`coax/web/room/wt-cert.sh CERT KEY [DAYS] [NAME]` makes a new ECDSA P-256 key
(PKCS#8, as picotls reads it) and a self-signed certificate valid for 10 days,
with the `openssl` command every host has, and prints its SHA-256. A page
passes that hash as `serverCertificateHashes`. Chrome accepts a hash only for
a P-256 certificate valid for under 14 days. Production uses a CA certificate
on the VM instead (#13174), and no hash is needed.

## The check

```sh
coax/web/room/wt-check.sh            # local, port 9441
coax/web/room/wt-check.sh --remote https://HOST:PORT/wt HASH
```

It builds `wt-echo` (any session accepted, each datagram sent back on its
session, each bidirectional stream echoed), makes a certificate and starts the
server on UDP and TCP. Then a headless Chrome opens `http://127.0.0.1:PORT/`
(a secure origin, served by `wt-echo` over TCP) and runs `wt-check.js`:

1. `new WebTransport(url, {serverCertificateHashes})`, and waits for it to be
   ready;
2. sends 1,000 numbered 100-byte datagrams, paced about one a millisecond, and
   one of 1,190 bytes, and counts the echoes;
3. writes 100 kB on a bidirectional stream, closes its side, and reads it all
   back, checking every byte;
4. closes the session and waits for it to be closed.

It holds when at least 99% of the datagrams echo, the stream comes back intact
and the session closes. The check runs in the room's checks
(`coax/remote/check.sh room`). A build host with no headless Chromium still
builds the stack and starts the server, and says the browser half was skipped.

### Results

Local, 2026-11-11, Apple Silicon Mac, Chrome 157 headless, no loss injected:

```
{"ok":true,"ready_ms":12,"max_datagram":1201,"datagrams_sent":1000,"datagrams_echoed":1000,
 "big_echoed":1190,"datagram_rtt_p50":2.2,"datagram_rtt_p95":3.2,"stream_ms":129,
 "stream_bytes":100000,"stream_same":true,"closed":true}
```

- The session opened in 12 ms. All 1,000 datagrams and the 1,190-byte one
  echoed, with a round trip of 2.2 ms at the median and 3.2 ms at the 95th
  percentile.
- 100 kB round-tripped on a stream in 129 ms, intact. The session closed
  cleanly, and the server saw the close.
- Chrome's `maxDatagramSize` was 1,201 bytes, so a room datagram of up to
  1,200 bytes (`WT_DATAGRAM_MAX`) fits in one packet.
- The same build compiles and serves on a Linux build host (Debian, gcc).

## The room on WebTransport (#13176)

The room server takes WebTransport sessions next to its WebSockets
(`room-wt.c`, included by `room-host.c`). It starts when it has a
certificate: `--wt CERT KEY`, or `OA_ROOM_WT_CERT` and `OA_ROOM_WT_KEY`. The
UDP port has the TCP port's number unless `OA_ROOM_WT_PORT` says otherwise.

- A session's CONNECT path is the WebSocket's request path (`/api/room/...`),
  so a session joins, seats and plays through the same `rm_join`, and is one
  of the room's connections.
- The page sends inputs as datagrams and everything else on one stream, each
  frame its length (2 bytes) and its bytes. The room sends each frame on the
  channel `room-wt.coax` names: a datagram, or that domain's own stream.
- At each tick's start the room drops the last tick's datagrams that still
  wait for the congestion window, and counts them. Each second the page hears
  `nq STALE WAITING LOST`.
- The page host (`coax/web/room-wt.js`) opens `https://HOST/PATH` for a
  `wt+ws://HOST/PATH` URL, and falls back to the WebSocket when WebTransport
  is missing, refused or not ready in 2.5 seconds. The two are never open at
  once, since each would take the seat.

Local check, 2026-11-11, Chrome 157 headless, the room with `--wt`, the page
with `?transport=wt&wthash=HASH`:

- The session opened and the player seated, walked and cast. In a few
  seconds the page heard 418 datagrams and sent 203.
- `everglade2-latency` with two players on WebTransport held: every walk,
  turn, attack and loose answered, 12 casts sent and 12 heard, and B saw A's
  arrow 19 ms (median) after A loosed it. On WebSocket the same run gave 23 ms.
- `?net` showed `LINK WT LOSS 0.0%` and `KEY 2 STALE 0 WAIT 2`.

### On the room VM

Each room unit on the VM owns one UDP port. The staging unit,
`oa-room@staging`, has UDP 443 for `room-staging.openagents.com`. Production
shard K, `oa-room@sK`, has UDP 443K (4431 to 4436) for
`room.openagents.com`. Its endpoint, the one the directory hands pages, is
`wss://room.openagents.com:443K/api/room/sK/`: Caddy serves its WebSocket on
that TCP port, and the shard serves WebTransport on that UDP port.

`vm-deploy.sh wt` copies Caddy's certificate for each name (the key as
PKCS#8), gives each unit its files and port, and installs a daily timer that
does it again after Caddy renews (`vm/wt-cert.sh`). Then it drains and
restarts each running shard onto them. A renewal restarts the staging unit
at once. A production shard takes it at its next restart, unless the
certificate it runs expires within a week.

The page opens WebTransport by default (`E2N_WT_DEFAULT`, #13187) when the
directory names a shard's own endpoint. It falls back to that endpoint's
WebSocket when WebTransport fails. When neither opens (a network that blocks
the port), it uses the load balancer's path from then on. `?transport=ws`
keeps the WebSocket.

## Loss: the matrix and the pipeline check (#13177)

Three tools measure the room under loss:

- `coax/web/room/net-proxy.coax` stands between the pages and a local room.
  It carries TCP (WebSocket) and UDP (WebTransport) on one port. Each read
  or datagram is held half the round trip, give or take half the jitter. On
  TCP a lost read comes TCP's resend later (the larger of 200 ms and 2 RTT).
  A lost datagram is gone. On both paths jitter is a queue's: nothing
  overtakes what was sent before it, so QUIC doesn't read reordering as
  loss.
- `coax/web/room/net-matrix.coax` runs `everglade2-latency` (two headless
  Chromes) through the proxy for each loss (0, 2, 5 and 10%), round trip
  (50, 100 and 150 ms, jitter ±30 ms) and transport. It writes one JSON
  line per cell and prints the table. `--quick` runs only 2% at 120 ms;
  `--rtt MS` runs one round trip; `--check BASELINE` fails when a cell's
  remote-arrow p95 is over 1.5 times the baseline's plus 50 ms, or fewer
  than half the arrows were seen.
- `coax/web/room/wt-room-check.coax` plays one seat over WebTransport
  against a real room. Run it against the VM's staging unit:

  ```
  OA_ROOM_TICKET_KEY=... coax/web/tool.sh coax/web/room/wt-room-check.coax \
    http://127.0.0.1:PAGE/ https://room-staging.openagents.com/api/room/relay/everglade/pair-e2-wtnet-STAMP -
  ```

The pipeline's build hosts have no Chrome, so the pipeline's check is a
model in the room's own checks (`room-wt.coax`, "webtransport under loss").
It feeds the room's real frames through a seeded model of both transports
at 2% and 10% loss, 120 ms and ±30 ms. It fails when a WebTransport page's
state is 2 RTT old or more at the 95th percentile, or older than a
WebSocket page's. It runs with `coax/web/room/room.coax` in every
`check.sh` run that touches the room.

Results: `docs/audits/2026-11-11-room-loss-matrix.md`.
