# 28. Sockets, TLS and relay URLs

A Nostr client reaches a relay through a URL such as `wss://relay.damus.io`.
Before any byte goes out, something must decide whether the connection is
plain TCP or TLS, which host to resolve and to check the certificate
against, which port to dial and which request target to put in the opening
request. This chapter shows where that line is drawn. Coax makes every one
of those decisions and every decision about the frames that come back. The C
host opens the socket and, for `wss://`, does the TLS handshake, the
certificate checks and the record encryption, and nothing else. AGENTS.md
allows that one exception: the C host may link one TLS library.

## What this chapter teaches

- How `uri.coax` and `relay_uri.coax` split a relay URL into four
  decisions: TLS or plain, host, port and request target. They keep the
  caller's bytes and change none of them.
- What they refuse: other schemes, uppercase schemes, user info, fragments,
  IP literals, bad DNS labels, bad ports, bad path bytes and URLs over the
  length limit.
- How the NIP-42 relay URL rule in `nip42.coax` differs. It is the only code
  in the library that normalizes a relay URL, and it does so to compare
  relay identities, not to connect.
- How `relay_conn.coax` turns `relay_uri`'s answer into foreign calls, and
  how `relay_conn_rules.coax` (`#authority`) decides what each server frame
  and each transport read result means.
- What the TLS effect in `tls-native.c` checks, the error codes it returns,
  and why there is no plaintext fallback for `wss://`.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [uri.coax](../../coax/lib/net/uri.coax) (`#authority`) | `uri_parse(text)` | bounded absolute `https://`, `http://` and `wss://` URIs with DNS hosts |
| | `uri_dns(host)` | DNS name syntax: labels of 1 to 63 letters, digits and hyphens, at most 253 bytes |
| [relay_uri.coax](../../coax/lib/net/relay_uri.coax) (`#authority`) | `relay_uri(text)` | `ws://` or `wss://`: `tls`, host range, port, request target range |
| [relay_conn_rules.coax](../../coax/lib/net/relay_conn_rules.coax) (`#authority`) | `rc_scan(rd, bytes, msg)` | what the next server frame asks for |
| | `rc_read(n)`, `rc_closed_stream(n, code)` | what a transport read result means |
| | `rc_target(path, out)`, `rc_close_reply(code)`, `rc_wait(...)` | request target, close echo, time left |
| [relay_conn.coax](../../coax/lib/net/relay_conn.coax) | `relay_conn_open`, `_send`, `_poll`, `_ping`, `_close`, `_drop` | the connection: moves bytes through the foreign effects |
| [socket-native.c](../../coax/platform/socket-native.c) | `cxsock_*` | TCP: resolve, connect, read, write, close |
| [tls-native.c](../../coax/platform/tls-native.c) | `cxtls_*` | TLS: OpenSSL 3 on Linux, Network.framework on Apple |
| [nip42.coax](../../coax/lib/nostr/nip42.coax) (`#authority`) | `nip42_url(url, out)` | the canonical form of a relay URL, used by chapter 38 |

## A relay URL is four decisions

`relay_uri` returns a `RelayUri`:

```jai
RelayUri :: struct { ok: bool; tls: bool; host_from: int; host_to: int; port: int; target_from: int; target_to: int; }
```

- `tls` is true for `wss://` and false for `ws://`. The host driver picks the
  TLS effect or the plain socket effect from this field alone.
- `host_from..<host_to` is the host. The same bytes go to name resolution,
  to TLS SNI and to the certificate name check.
- `port` is the explicit port, or 443 for `wss://` and 80 for `ws://`.
- `target_from..<target_to` is the path and query. `rc_target` puts a `/` in
  front when the range is empty or starts with `?`.

The ranges point into the caller's string. `relay_uri` copies nothing and
changes nothing: it doesn't lowercase the host, drop a default port or
remove a trailing slash.

`uri_parse` knows `https://`, `http://` and `wss://`. `relay_uri` needs
`ws://` too, so it copies a `ws://` URL into a buffer as `wss://` (one byte
longer), parses that, and moves every range back by one byte. When no
`:port` stands between the host and the path, it sets the port to 80. This
is why a `ws://` URL can be at most 2047 bytes: its `wss://` spelling must
fit `uri_parse`'s 2048-byte limit.

### Worked example: a URL by hand

Take `wss://Relay.Example:8443/nostr?x=1`, 34 bytes.

| Bytes | Text | Step |
| --- | --- | --- |
| 0–5 | `wss://` | The prefix matches exactly, byte for byte, so the scheme is `wss`, `tls` is true, the host starts at 6 and the port starts at 443. |
| 6–18 | `Relay.Example` | The host runs until the first `/`, `?` or `:`. That is the `:` at byte 19, so `host_to` is 19. |
| | | `uri_dns` checks it: 13 bytes; label `Relay` (5 bytes) ends at the `.` at byte 11, with no hyphen at either end; the last label `Example` (7 bytes) starts with a letter. The host passes. |
| 19 | `:` | A port follows. Its first digit is `8`, not `0`. |
| 20–23 | `8443` | 8, then 8 × 10 + 4 = 84, then 844, then 8443. The value is from 1 to 65535. The `/` at byte 24 ends it. |
| 24–29 | `/nostr` | The path starts at 24. Every byte is a letter or `/`. |
| 30 | `?` | The first `?` ends the path and starts the query at 31. |
| 31–33 | `x=1` | Allowed query bytes. |

So `relay_uri` returns `tls` true, host 6..<19 (`Relay.Example`), port 8443
and target 24..<34 (`/nostr?x=1`). The target starts with `/`, so
`rc_target` copies it unchanged. `relay_conn_open` then pushes the 13 host
bytes into `cxtls_host_push`, calls `cxtls_connect(8443, 5000)` and sends:

```text
GET /nostr?x=1 HTTP/1.1
Host: Relay.Example:8443
```

The same URL spelled `ws://` gives the same port and every range one byte
earlier: host 5..<18 and target 23..<33. Only `tls` changes.

### What is refused

`relay_uri` returns `ok` false, with every other field zero, for:

| Input | Why |
| --- | --- |
| `http://relay.example`, `https://relay.example` | not `ws://` or `wss://` |
| `WSS://relay.example` | the scheme must be lowercase |
| `wss://user@relay.example` | `@` is not a DNS byte: no user info |
| `wss://relay.example/#frag` | `#` is not a path byte: no fragment |
| `wss://relay.example:0`, `:65536`, `:0443`, `:` | the port must be 1 to 65535, without leading zeros, and present after a `:` |
| `wss://127.0.0.1:7777` | the last label must start with a letter, so no IPv4 address, in any spelling |
| `wss://[::1]:7777` | `[` is not a DNS byte: no IPv6 literal |
| `wss://relay.example.` | no trailing dot |
| `wss://relay_1.example`, `wss://-relay.example` | labels are letters, digits and inner hyphens |
| `wss://bücher.example` | the host is ASCII only |
| `wss://relay.example/a b`, `/%00` | no space, control byte or non-ASCII byte in the path, raw or percent-escaped |
| a `ws://` URL of 2048 bytes | over the length limit |

Three things are not refused:

- **Plain `ws://` to any host.** `ws://relay.example` is accepted with `tls`
  false and port 80. Nothing in the code limits `ws://` to `localhost`. A
  caller that wants only TLS checks `tls`.
- **Internationalized names in their ASCII form.** The code does no IDNA
  conversion. `bücher.example` is refused, but its A-label
  `xn--bcher-kva.example` (Python's `"bücher".encode("idna")` gives
  `xn--bcher-kva`) is ordinary letters, digits and hyphens and is accepted.
- **A port that doesn't match the scheme.** `ws://relay.example:443` is
  plain TCP to port 443. The scheme alone decides TLS.

A local test relay is reached as `ws://localhost:PORT`, not
`ws://127.0.0.1:PORT`, because IP literals are refused. The relay fixture in
`coax/tests/net/` binds 127.0.0.1 and checks that the `Host` header is
`localhost:PORT`.

## Normalizing a relay URL: NIP-42's rule

`relay_uri` doesn't normalize, so `wss://relay.example`,
`wss://relay.example/` and `wss://relay.example:443` are three different
strings that connect to the same place. When a client must decide whether
two URLs name the same relay, it needs a canonical form. NIP-42 needs one:
an AUTH event's `relay` tag must match the relay the client is talking to.
`nip42_url` in `nip42.coax` (chapter 38) is that rule, and the only
normalizer in the library:

1. The scheme is `ws://` or `wss://` in any case. It is written lowercase.
2. The host is letters, digits, `.` and `-`, written lowercase. Brackets and
   user info are refused.
3. A port of 1 to 5 digits with a value from 1 to 65535 is kept, unless it is
   the default (443 for `wss`, 80 for `ws`), which is dropped.
4. The path is printable ASCII without `?` or `#`, kept as written, except
   that trailing slashes are removed. A query or fragment is refused.

### Worked example: a canonical form by hand

Take `WSS://Relay.Example:443/nostr//`.

1. `WSS://` matches `wss://` without case: write `wss://`.
2. `Relay.Example` up to the `:`: write `relay.example`.
3. `443` is the default port for `wss`: write nothing.
4. `/nostr//` has no `?` or `#`. Remove the two trailing slashes: write
   `/nostr`.

The canonical form is `wss://relay.example/nostr`, 25 bytes.
`relay_uri` refuses the same input, because its scheme is uppercase.

The two rules disagree in both directions. The example prints both for
every URL:

| Input | `relay_uri` | `nip42_url` |
| --- | --- | --- |
| `WSS://Relay.Example:443/nostr//` | refused (uppercase scheme) | `wss://relay.example/nostr` |
| `wss://Relay.Example:8443/nostr?x=1` | accepted | refused (query) |
| `wss://127.0.0.1:7777` | refused (IP literal) | `wss://127.0.0.1:7777` |
| `wss://relay.example.`, `wss://-relay.example` | refused (DNS labels) | kept as written |
| `wss://relay.example:0443` | refused (leading zero) | `wss://relay.example` |
| `wss://relay.example/%00` | refused (escaped control byte) | kept as written |

So the canonical form is for comparing names, never for connecting. Connect
with the URL that `relay_uri` accepts; compare with `nip42_url`.

## The connection

`relay_conn.coax` holds one connection in a `RelayConn`: the transport
handle `h`, the `tls` flag, the WebSocket reader, an input buffer, the last
text message and the counts of pings and pongs. It decides nothing itself.
`relay_conn_open` does this:

1. `relay_uri(uri)`. On refusal, return false before touching the network.
2. Push each host byte into `cxtls_host_push` or `cxsock_host_push`, chosen
   by `tls`.
3. `cxtls_connect(port, 5000)` or `cxsock_connect(port, 5000)`. A negative
   handle means failure: return false. There is no second try over the
   other effect.
4. Draw a 16-byte nonce from the entropy contract, write the opening request
   with `ws_request` (chapter 26), send it, and read until `ws_response`
   admits the 101 answer. Connect, TLS and the whole handshake share one
   5-second deadline.

`relay_conn_poll` then loops: `rc_scan` looks at the buffered bytes and
names one event; `relay_conn` carries it out; when `rc_scan` needs more
bytes, `rc_fill` reads from the transport and `rc_read` says what the read
result means.

| `rc_scan` result | What `relay_conn_poll` does |
| --- | --- |
| `RC_S_TEXT` | returns `RC_TEXT`; the message is in `text[0..<text_n]` |
| `RC_S_PING` | sends a masked pong with the same payload, continues |
| `RC_S_PONG` | stores the payload, counts it, continues |
| `RC_S_CLOSE` | echoes the peer's code (1000 for an empty close, because 1005 may not be sent), closes, returns `RC_CLOSED` |
| `RC_S_REFUSE` | sends the close code `rc_scan` gives, closes, returns `RC_FAIL` |
| `RC_S_MORE` | reads more bytes |

`rc_scan` adds one rule to chapter 26's frame rules: NIP-01 speaks text
only, so a complete binary message is refused with 1003. A frame or message
over 128 KiB (`RC_MSG_MAX`) gets 1009.

### Read results

The plain and TLS effects share their read codes, and `rc_read` maps them:

| Code | Meaning | `rc_read` |
| ---: | --- | --- |
| > 0 | bytes arrived | `RC_GOT` |
| 0, -2 | nothing yet, timeout | `RC_NONE` |
| -1 | clean end of stream (on TLS, after close_notify) | `RC_CLOSED` |
| -3 | error, including a TCP close without close_notify on Linux TLS | `RC_FAIL` |
| -6, -7 | full, unsupported | `RC_FAIL` |
| -8 | TLS handshake failed | `RC_FAIL` |
| -9 | certificate chain not trusted | `RC_FAIL` |
| -10 | certificate is for another name | `RC_FAIL` |
| -11 | TLS older than 1.2 | `RC_FAIL` |
| -12 | Apple: stream ended, close_notify not confirmed | `RC_FAIL` |

Network.framework reports a TLS close_notify and a bare TCP FIN the same
way, so the Apple branch returns -12 for both. A normal poll treats -12 as
a failure. `rc_closed_stream(-12, code)` accepts it as the end of the stream
only when a complete WebSocket close frame arrived first, inside the
encrypted stream, so the close itself is authenticated.

## TLS: what the host does

`tls-native.c` does the TLS handshake, the certificate checks and the record
encryption. Its policy is fixed in the source, with no switch to turn a
check off:

- **Linux, OpenSSL 3:** TLS 1.2 at least (1.3 is negotiated first), SNI set
  to the host, `SSL_VERIFY_PEER` against the system trust store,
  `SSL_set1_host` with partial wildcards refused, and the verify result and
  peer certificate checked again after the handshake. No compression and no
  renegotiation. The handshake, reads and writes are nonblocking and
  bounded by the deadline.
- **Apple, Network.framework and Security:** TLS 1.2 and 1.3,
  `SecPolicyCreateSSL` for the host name and `SecTrustEvaluateWithError`
  for the chain, expiry and name.
- **Tests only:** a build with the `CXTLS_TEST_TRUST` macro trusts exactly
  the CA file named by `$CXTLS_TEST_CA`. Without the macro that code isn't
  compiled, and the variable is ignored. Only the test scripts define it.

There is no plaintext fallback for `wss://`. `relay_conn_open` chooses the
effect from `tls` once. If `cxtls_connect` fails, the open fails and the
connection stays closed. The caller (chapter 29's reconnect logic) tries
the same URL again later, again over TLS.

The host never parses the URL. It receives the host bytes one at a time and
refuses a zero byte, a byte over 255 or a host over 253 bytes. Coax has
already decided that the bytes form a DNS name. OpenSSL's name check
ignores ASCII case, so `Relay.Example` matches a certificate for
`relay.example`.

## The example

The example is [nostr/examples/relay-urls.coax](../examples/relay-urls.coax).
It needs no network. It runs `relay_uri` and `nip42_url` on accepted and
refused URLs, prints the opening request for the worked URL, and replays a
scripted relay through the same `rc_scan` and `rc_read` decisions that
`relay_conn_poll` uses, with a fake transport that hands over seven bytes per
read and a timeout (-2) at the end of the script.

The script is 51 bytes of server frames: a text message
`["NOTICE","hi"]`, the first fragment of `["EVENT","s",{}]`, a ping `p1`
between the fragments, the last fragment, a pong `k` and a close with code
1000 and reason `bye`. The example also replays it with every read size
from 1 to 51 bytes and checks that each gives the same events and text, and
replays it cut after 20 bytes. Then it shows six frames that the rules
refuse, the meaning of every read code, and `rc_wait`.

<!-- source: nostr/examples/relay-urls.coax example -->
```jai
#load "../../coax/lib/net/relay_uri.coax";
#load "../../coax/lib/net/relay_conn_rules.coax";
#load "../../coax/lib/nostr/nip42.coax";
#import "Basic";

// The name of a nip42_url result.
url_error_name :: (e: int) -> string {
  names := [7] string.["OK", "OUTPUT", "LIMIT", "SCHEME", "HOST", "PORT", "PATH"];
  return names[e];
}
// What relay_uri decides for url, the request target relay_conn_open would
// send, and the NIP-42 canonical form of the same text.
show_url :: (url: string) {
  canon: [N42_MAX_URL] u8;
  c := nip42_url(url, canon[..]);
  key := ifx c.error == N42U_OK then cast(string) canon[0..<c.count] else url_error_name(c.error);
  r := relay_uri(url);
  if !r.ok { print("%  -> refused | nip42 %\n", url, key); return; }
  target: [2049] u8;
  n := rc_target(url[r.target_from..<r.target_to], target[..]);
  print("%  -> % host % port % target % | nip42 %\n", url, ifx r.tls then "TLS" else "plain",
        url[r.host_from..<r.host_to], r.port, cast(string) target[0..<n], key);
}

// One unmasked server frame of at most 125 bytes, appended at out[at..].
// Returns the new end.
put_frame :: (out: [] u8, at: int, op: int, fin: bool, payload: string) -> int {
  out[at] = cast(u8) (ifx fin then op + 128 else op);
  out[at + 1] = cast(u8) payload.count;
  for b, i: payload { out[at + 2 + i] = b; }
  return at + 2 + payload.count;
}
// A close frame's payload: the code big-endian, then a reason.
close_payload :: (code: int, reason: string, out: [] u8) -> string {
  out[0] = cast(u8) (code >> 8); out[1] = cast(u8) (code & 255);
  for b, i: reason { out[2 + i] = b; }
  return cast(string) out[0..<2 + reason.count];
}

// The buffers relay_conn.coax keeps, without the transport.
Sim :: struct {
  inb: [256] u8; used: int;
  rd: [1] WsReader; text: [64] u8;
}
drop_front :: (s: [] Sim, k: int) {
  for i: k..<s[0].used { s[0].inb[i - k] = s[0].inb[i]; }
  s[0].used -= k;
}
// What a replay saw: the poll result that ended it, one letter per event
// (T text, P ping, O pong, C close, R refusal), the text bytes delivered,
// the close code relay_conn sends and the number of reads.
Replay :: struct { result: int; log: [16] u8; events: int; text_bytes: int; text_sum: int; close: int; reads: int; }
note :: (t: [] Replay, letter: u8) { if t[0].events < 16 { t[0].log[t[0].events] = letter; } t[0].events += 1; }
// relay_conn_poll's loop over a scripted transport: each read hands over
// the next `chunk` bytes of `wire`, and the end of the wire reads as a
// timeout (-2). rc_scan decides each step, and `loud` prints what
// relay_conn would do.
replay :: (wire: string, chunk: int, loud: bool) -> Replay {
  s: [1] Sim; t: [1] Replay; sent := 0;
  while true {
    v := rc_scan(s[0].rd[..], cast(string) s[0].inb[0..<s[0].used], s[0].text[..]);
    if v.kind == RC_S_TEXT {
      msg := cast(string) s[0].text[0..<s[0].rd[0].used];
      if loud { print("  scan: TEXT %\n", msg); }
      note(t[..], 'T'); t[0].text_bytes += msg.count;
      for b, i: msg { t[0].text_sum += (i + 1) * cast(int) b; }
      drop_front(s[..], v.consumed); continue;
    }
    if v.kind == RC_S_PING {
      if loud { print("  scan: PING \"%\" -> send a masked pong with the same payload\n", cast(string) s[0].inb[v.from..<v.to]); }
      note(t[..], 'P'); drop_front(s[..], v.consumed); continue;
    }
    if v.kind == RC_S_PONG {
      if loud { print("  scan: PONG \"%\" -> pongs += 1\n", cast(string) s[0].inb[v.from..<v.to]); }
      note(t[..], 'O'); drop_front(s[..], v.consumed); continue;
    }
    if v.kind == RC_S_CLOSE {
      t[0].close = rc_close_reply(v.code); t[0].result = RC_CLOSED; note(t[..], 'C');
      if loud { print("  scan: CLOSE % -> echo close %, shut, RC_CLOSED\n", v.code, t[0].close); }
      return t[0];
    }
    if v.kind == RC_S_REFUSE {
      t[0].close = v.code; t[0].result = RC_FAIL; note(t[..], 'R');
      if loud { print("  scan: REFUSE -> send close %, shut, RC_FAIL\n", v.code); }
      return t[0];
    }
    if loud && v.consumed > 0 { print("  scan: MORE, % fragment bytes taken into the message\n", v.consumed); }
    drop_front(s[..], v.consumed);
    n := ifx sent < wire.count then (ifx chunk < wire.count - sent then chunk else wire.count - sent) else -2;
    for i: 0..<n { s[0].inb[s[0].used + i] = wire[sent + i]; }
    if n > 0 { s[0].used += n; sent += n; }
    t[0].reads += 1;
    k := rc_read(n);
    if loud { print("read %: %\n", n, read_name(k)); }
    if k == RC_NONE { t[0].result = RC_NONE; return t[0]; }
  }
  return t[0];
}
// The name of an rc_read answer.
read_name :: (k: int) -> string {
  names := [5] string.["RC_NONE", "RC_TEXT", "RC_CLOSED", "RC_FAIL", "RC_GOT"];
  return names[k];
}
// Scan one frame alone with a 64-byte message limit and print the decision.
refuse :: (label: string, bytes: string) {
  rd: [1] WsReader; msg: [64] u8;
  v := rc_scan(rd[..], bytes, msg[..]);
  kinds := [6] string.["MORE", "TEXT", "PING", "PONG", "CLOSE", "REFUSE"];
  print("%: % close %\n", label, kinds[v.kind], v.code);
}

main :: () {
  // What a relay URL decides: plain or TLS, the host, the port, the target.
  print("accepted:\n");
  show_url("wss://relay.damus.io");
  show_url("wss://relay.example/");
  show_url("wss://relay.example:443");
  show_url("wss://Relay.Example:8443/nostr?x=1");
  show_url("WSS://Relay.Example:443/nostr//");
  show_url("ws://localhost:7777");
  show_url("ws://relay.example");
  show_url("ws://relay.example:443");
  show_url("wss://relay.example?x");
  show_url("wss://xn--bcher-kva.example");

  // Refusals.
  print("refused:\n");
  show_url("http://relay.example");
  show_url("https://relay.example");
  show_url("wss://user@relay.example");
  show_url("wss://relay.example/#frag");
  show_url("wss://relay.example:0");
  show_url("wss://relay.example:65536");
  show_url("wss://relay.example:0443");
  show_url("wss://relay.example:");
  show_url("wss://127.0.0.1:7777");
  show_url("wss://[::1]:7777");
  show_url("wss://relay.example.");
  show_url("wss://relay_1.example");
  show_url("wss://-relay.example");
  show_url("wss://bücher.example");
  show_url("wss://relay.example/a b");
  show_url("wss://relay.example/%00");
  long: [2049] u8;
  for i: 0..<2049 { long[i] = 'a'; }
  prefix := "ws://a.test/"; for b, i: prefix { long[i] = b; }
  print("ws:// of 2047 bytes: %\n", relay_uri(cast(string) long[0..<2047]).ok);
  print("ws:// of 2048 bytes: %\n", relay_uri(cast(string) long[0..<2048]).ok);

  // The opening request relay_conn_open writes for one URL.
  url := "wss://Relay.Example:8443/nostr?x=1";
  r := relay_uri(url);
  target: [64] u8; req: [512] u8;
  tn := rc_target(url[r.target_from..<r.target_to], target[..]);
  n := ws_request(url[r.host_from..<r.host_to], r.port, cast(string) target[0..<tn], "dGhlIHNhbXBsZSBub25jZQ==", req[..]);
  line := 0; lines := 0;
  for c, i: cast(string) req[0..<n] {
    if c == '\r' && lines < 2 { print("request: %\n", cast(string) req[line..<i]); line = i + 2; lines += 1; }
  }

  // A scripted relay, read seven bytes at a time.
  wire: [256] u8; cp: [16] u8;
  w := put_frame(wire[..], 0, WS_OP_TEXT, true, "[\"NOTICE\",\"hi\"]");
  w = put_frame(wire[..], w, WS_OP_TEXT, false, "[\"EVENT\",\"s\",");
  w = put_frame(wire[..], w, WS_OP_PING, true, "p1");
  w = put_frame(wire[..], w, WS_OP_CONT, true, "{}]");
  w = put_frame(wire[..], w, WS_OP_PONG, true, "k");
  w = put_frame(wire[..], w, WS_OP_CLOSE, true, close_payload(1000, "bye", cp[..]));
  print("script: % bytes\n", w);
  end := replay(cast(string) wire[0..<w], 7, true);
  print("poll result: %, events %\n", read_name(end.result), cast(string) end.log[0..<end.events]);
  // The same script read 1 to 51 bytes at a time gives the same events.
  same := 0;
  for c: 1..<w + 1 {
    t := replay(cast(string) wire[0..<w], c, false);
    if t.result == end.result && t.events == end.events && cast(string) t.log[0..<t.events] == cast(string) end.log[0..<end.events] &&
       t.text_bytes == end.text_bytes && t.text_sum == end.text_sum && t.close == end.close { same += 1; }
  }
  print("read sizes 1 to %: % of % give the same events and text\n", w, same, w);
  print("script cut after 20 bytes:\n");
  end = replay(cast(string) wire[0..<20], 7, true);
  print("poll result: %, events %\n", read_name(end.result), cast(string) end.log[0..<end.events]);
  w = put_frame(wire[..], 0, WS_OP_CLOSE, true, "");
  print("empty close:\n");
  end = replay(cast(string) wire[0..<w], 7, true);

  // Frames the rules refuse, each with the close code relay_conn sends.
  big: [70] u8;
  w = put_frame(wire[..], 0, WS_OP_BINARY, true, "x");
  refuse("binary message", cast(string) wire[0..<w]);
  w = put_frame(wire[..], 0, WS_OP_TEXT, true, "x"); wire[1] = wire[1] | 128;
  refuse("masked server frame", cast(string) wire[0..<w]);
  w = put_frame(wire[..], 0, WS_OP_TEXT, true, "x"); wire[2] = 0xff;
  refuse("text ff", cast(string) wire[0..<w]);
  w = put_frame(big[..], 0, WS_OP_TEXT, true, cast(string) wire[0..<65]);
  refuse("65 bytes against 64", cast(string) big[0..<w]);
  w = put_frame(wire[..], 0, WS_OP_CONT, true, "x");
  refuse("continuation first", cast(string) wire[0..<w]);
  w = put_frame(wire[..], 0, WS_OP_CLOSE, true, close_payload(1005, "", cp[..]));
  refuse("close code 1005 sent", cast(string) wire[0..<w]);

  // What a transport read result means: socket-native.c and tls-native.c
  // share the codes.
  codes := [10] int.[5, 0, -2, -1, -3, -8, -9, -10, -11, -12];
  names := [10] string.["5 bytes", "0", "-2 timeout", "-1 end of stream", "-3 error", "-8 handshake",
                        "-9 untrusted", "-10 wrong name", "-11 old version", "-12 unverified end"];
  for i: 0..<10 { print("read %: %\n", names[i], read_name(rc_read(codes[i]))); }
  print("-12 accepted as end with no close frame: %\n", rc_closed_stream(-12, 0));
  print("-12 accepted as end after close 1000: %\n", rc_closed_stream(-12, 1000));
  print("wait: % % % %\n", rc_wait(-1, 5000, 0), rc_wait(5000, 5000, 1200), rc_wait(5000, 5000, 4999), rc_wait(5000, 5000, 6000));
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/relay-urls.coax
```

The output matches the values worked out by hand: `Relay.Example`, port
8443 and target `/nostr?x=1` for the first worked URL, and
`wss://relay.example/nostr` for the second.

```text
accepted:
wss://relay.damus.io  -> TLS host relay.damus.io port 443 target / | nip42 wss://relay.damus.io
wss://relay.example/  -> TLS host relay.example port 443 target / | nip42 wss://relay.example
wss://relay.example:443  -> TLS host relay.example port 443 target / | nip42 wss://relay.example
wss://Relay.Example:8443/nostr?x=1  -> TLS host Relay.Example port 8443 target /nostr?x=1 | nip42 PATH
WSS://Relay.Example:443/nostr//  -> refused | nip42 wss://relay.example/nostr
ws://localhost:7777  -> plain host localhost port 7777 target / | nip42 ws://localhost:7777
ws://relay.example  -> plain host relay.example port 80 target / | nip42 ws://relay.example
ws://relay.example:443  -> plain host relay.example port 443 target / | nip42 ws://relay.example:443
wss://relay.example?x  -> TLS host relay.example port 443 target /?x | nip42 HOST
wss://xn--bcher-kva.example  -> TLS host xn--bcher-kva.example port 443 target / | nip42 wss://xn--bcher-kva.example
refused:
http://relay.example  -> refused | nip42 SCHEME
https://relay.example  -> refused | nip42 SCHEME
wss://user@relay.example  -> refused | nip42 HOST
wss://relay.example/#frag  -> refused | nip42 PATH
wss://relay.example:0  -> refused | nip42 PORT
wss://relay.example:65536  -> refused | nip42 PORT
wss://relay.example:0443  -> refused | nip42 wss://relay.example
wss://relay.example:  -> refused | nip42 PORT
wss://127.0.0.1:7777  -> refused | nip42 wss://127.0.0.1:7777
wss://[::1]:7777  -> refused | nip42 HOST
wss://relay.example.  -> refused | nip42 wss://relay.example.
wss://relay_1.example  -> refused | nip42 HOST
wss://-relay.example  -> refused | nip42 wss://-relay.example
wss://bücher.example  -> refused | nip42 HOST
wss://relay.example/a b  -> refused | nip42 PATH
wss://relay.example/%00  -> refused | nip42 wss://relay.example/%00
ws:// of 2047 bytes: true
ws:// of 2048 bytes: false
request: GET /nostr?x=1 HTTP/1.1
request: Host: Relay.Example:8443
script: 51 bytes
read 7: RC_GOT
read 7: RC_GOT
read 7: RC_GOT
  scan: TEXT ["NOTICE","hi"]
read 7: RC_GOT
read 7: RC_GOT
  scan: MORE, 15 fragment bytes taken into the message
read 7: RC_GOT
  scan: PING "p1" -> send a masked pong with the same payload
  scan: TEXT ["EVENT","s",{}]
read 7: RC_GOT
  scan: PONG "k" -> pongs += 1
read 2: RC_GOT
  scan: CLOSE 1000 -> echo close 1000, shut, RC_CLOSED
poll result: RC_CLOSED, events TPTOC
read sizes 1 to 51: 51 of 51 give the same events and text
script cut after 20 bytes:
read 7: RC_GOT
read 7: RC_GOT
read 6: RC_GOT
  scan: TEXT ["NOTICE","hi"]
read -2: RC_NONE
poll result: RC_NONE, events T
empty close:
read 2: RC_GOT
  scan: CLOSE 1005 -> echo close 1000, shut, RC_CLOSED
binary message: REFUSE close 1003
masked server frame: REFUSE close 1002
text ff: REFUSE close 1007
65 bytes against 64: REFUSE close 1009
continuation first: REFUSE close 1002
close code 1005 sent: REFUSE close 1002
read 5 bytes: RC_GOT
read 0: RC_NONE
read -2 timeout: RC_NONE
read -1 end of stream: RC_CLOSED
read -3 error: RC_FAIL
read -8 handshake: RC_FAIL
read -9 untrusted: RC_FAIL
read -10 wrong name: RC_FAIL
read -11 old version: RC_FAIL
read -12 unverified end: RC_FAIL
-12 accepted as end with no close frame: false
-12 accepted as end after close 1000: true
wait: -1 3800 1 0
```

What the output shows:

- Each accepted line gives TLS or plain, the host as written, the port and
  the request target. `wss://relay.example/` and `wss://relay.example:443`
  both connect to `relay.example` port 443 with target `/`, and both have
  the canonical form `wss://relay.example`.
- Each refused line says `refused`. The `nip42` column shows that the
  identity rule accepts some of them.
- In the replay, the first three reads bring 21 bytes, which hold the 17
  bytes of the first text frame. The first fragment of the second
  message (a 15-byte frame) is taken into the message reader (`MORE`), the ping between the
  fragments is answered, and the last fragment completes
  `["EVENT","s",{}]`. The close ends the poll with `RC_CLOSED` and an echo
  of 1000. The event letters are `TPTOC`: text, ping, text, pong, close.
- The cut script delivers the first message, then the read times out:
  `RC_NONE`, with no event invented from the partial frame.
- An empty close frame has no code (1005), and the echo is 1000.
- The refusals: a binary message gets 1003, a masked server frame 1002, the
  text byte `ff` 1007, a 65-byte frame against a 64-byte limit 1009, a
  continuation with nothing open 1002, and a close frame carrying the code
  1005, which a peer may not send, 1002.
- Read codes -3 to -12 all fail the connection. -12 counts as an end of
  stream only after a close frame.
- `rc_wait(-1, ...)` waits for ever (-1). With 5000 ms to the deadline, at
  1200 ms there are 3800 left, at 4999 there is 1, and past the deadline
  there are 0.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/relay-urls.out](../fixtures/relay-urls.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/relay-urls.coax](../examples/checks/relay-urls.coax),
which loads the example, so running the example doesn't run them:

- 12 relay URLs against Python's `urllib.parse.urlsplit`, which shares no
  code with `uri.coax` or `relay_uri.coax`: the host (lowercased with
  `nip42_host_byte` first, because `urlsplit` lowercases it), the port and
  the request target;
- the worked byte positions of both spellings, `wss://` and `ws://`, and the
  worked canonical form;
- every refusal in the example returns `ok` false with empty fields;
- for every read size from 1 to 51 bytes, the script gives `TPTOC`, 31 text
  bytes and an echo of 1000, and the script cut after any byte gives
  `RC_NONE` and at most four events.

<!-- source: nostr/examples/checks/relay-urls.coax check -->
```jai
// URL, host and request target, and the port, from Python's
// urllib.parse.urlsplit, which shares no code with uri.coax or relay_uri.coax:
//   s = urlsplit(u); s.hostname; (s.path or "/") + ("?" + s.query if "?" in u else "")
//   s.port or (443 if s.scheme == "wss" else 80)
SPLIT :: [36] string.[
  "wss://relay.damus.io", "relay.damus.io", "/",
  "wss://relay.example/", "relay.example", "/",
  "wss://relay.example:443", "relay.example", "/",
  "wss://Relay.Example:8443/nostr?x=1", "relay.example", "/nostr?x=1",
  "ws://localhost:7777", "localhost", "/",
  "ws://relay.example", "relay.example", "/",
  "ws://relay.example:443", "relay.example", "/",
  "wss://relay.example?x", "relay.example", "/?x",
  "wss://xn--bcher-kva.example", "xn--bcher-kva.example", "/",
  "ws://relay.example:7777/a%20b?x=1", "relay.example", "/a%20b?x=1",
  "wss://nos.lol/", "nos.lol", "/",
  "wss://relay.primal.net/v1?a=b&c=d", "relay.primal.net", "/v1?a=b&c=d"];
PORTS :: [12] int.[443, 443, 443, 8443, 7777, 80, 443, 443, 443, 7777, 443, 443];
// urlsplit lowercases the host, so relay_uri's host is lowercased with
// nip42_host_byte before the comparison.
#forall "host, port and target agree with urlsplit" i: 0..<12 {
  url := SPLIT[3 * i]; want := SPLIT[3 * i + 1]; r := relay_uri(url);
  #assert r.ok && r.tls == (url[2] == 's') && r.port == PORTS[i];
  host := url[r.host_from..<r.host_to];
  #assert host.count == want.count;
  for b, k: host { #assert nip42_host_byte(b) == cast(int) want[k]; }
  t: [64] u8; n := rc_target(url[r.target_from..<r.target_to], t[..]);
  #assert n > 0 && cast(string) t[0..<n] == SPLIT[3 * i + 2];
}
// The worked trace in the chapter, by hand.
#test "the worked URL's byte positions" {
  url := "wss://Relay.Example:8443/nostr?x=1"; r := relay_uri(url);
  #assert url.count == 34 && r.ok && r.tls && r.host_from == 6 && r.host_to == 19 && r.port == 8443;
  #assert r.target_from == 24 && r.target_to == 34;
  // The ws:// spelling: every range one byte earlier, the same port.
  w := "ws://Relay.Example:8443/nostr?x=1"; q := relay_uri(w);
  #assert q.ok && !q.tls && q.host_from == 5 && q.host_to == 18 && q.port == 8443 && q.target_from == 23 && q.target_to == 33;
  // Canonical NIP-42 form of the second worked URL.
  out: [N42_MAX_URL] u8; c := nip42_url("WSS://Relay.Example:443/nostr//", out[..]);
  #assert c.error == N42U_OK && cast(string) out[0..<c.count] == "wss://relay.example/nostr";
  #assert !relay_uri("WSS://Relay.Example:443/nostr//").ok;
}
// Every refusal in the example, with the reason uri.coax gives.
#test "the example's refusals" {
  bad := [16] string.["http://relay.example", "https://relay.example", "wss://user@relay.example",
    "wss://relay.example/#frag", "wss://relay.example:0", "wss://relay.example:65536", "wss://relay.example:0443",
    "wss://relay.example:", "wss://127.0.0.1:7777", "wss://[::1]:7777", "wss://relay.example.",
    "wss://relay_1.example", "wss://-relay.example", "wss://bücher.example", "wss://relay.example/a b",
    "wss://relay.example/%00"];
  for u: bad { r := relay_uri(u); #assert !r.ok && r.port == 0 && r.host_to == 0; }
}
// Splitting the wire into reads of any size never changes what the rules
// decide, and a cut wire never invents an event.
#forall "every read size gives the same events" chunk: 1..<52 {
  wire: [256] u8; cp: [16] u8;
  w := put_frame(wire[..], 0, WS_OP_TEXT, true, "[\"NOTICE\",\"hi\"]");
  w = put_frame(wire[..], w, WS_OP_TEXT, false, "[\"EVENT\",\"s\",");
  w = put_frame(wire[..], w, WS_OP_PING, true, "p1");
  w = put_frame(wire[..], w, WS_OP_CONT, true, "{}]");
  w = put_frame(wire[..], w, WS_OP_PONG, true, "k");
  w = put_frame(wire[..], w, WS_OP_CLOSE, true, close_payload(1000, "bye", cp[..]));
  #assert w == 51;
  t := replay(cast(string) wire[0..<w], chunk, false);
  #assert t.result == RC_CLOSED && cast(string) t.log[0..<t.events] == "TPTOC" && t.close == 1000;
  #assert t.text_bytes == 15 + 16;
  // Cut after any byte: at most the events whose frames are complete.
  for cut: 0..<w {
    c := replay(cast(string) wire[0..<cut], chunk, false);
    #assert c.result == RC_NONE && c.events <= 4;
  }
}
// Read results from both transports, and the -12 rule.
#test "transport codes" {
  #assert rc_read(1) == RC_GOT && rc_read(-1) == RC_CLOSED && rc_read(-2) == RC_NONE;
  for code: 3..<13 { #assert rc_read(-code) == RC_FAIL; }
  #assert !rc_closed_stream(-12, 0) && rc_closed_stream(-12, 1000) && rc_closed_stream(-1, 0);
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/relay-urls.coax
```

On 2026-11-01 `coax test` ran 94 checks, including the loaded files' own
checks (89 from `uri.coax`, `relay_uri.coax`, `relay_conn_rules.coax`,
`ws.coax`, `sha1.coax`, `nip42.coax` and what they load), and all passed.
The checks file defines no procedures, so `coax mutate --root-only` on it
finds no mutation site. The library's mutants are below. These commands
were run with `coax0 0.46.0`.

### The library's own checks

`uri.coax` checks the three schemes, every authority refusal, percent
escapes, the 2048-byte and DNS label limits, and every byte's character
class. `relay_uri.coax` checks the TLS choice, default and explicit ports,
ranges, refusals and the 2047- and 2048-byte limits. `relay_conn_rules.coax`
checks text, fragments, pings, pongs, closes, every refusal and its close
code, read results, waits, request targets and the -12 rule.

The independent URI oracle `coax/tests/net/uri.py` compares `uri_parse` with
`urlsplit` on 463 cases (256 random valid URIs and every refused form) in
checked and release builds, under a sanitizer and in Wasm; the evidence is
`coax/tests/net/uri-evidence-2026-10-28.json`.

Mutation results from the records, not rerun today (the files are unchanged
since):

- `relay_uri.coax` with `uri.coax`: 264 of 264 mutants killed, 2026-10-29,
  [2026-10-29-nostr-relay-tls.md](../../docs/verification/2026-10-29-nostr-relay-tls.md).
- `relay_conn_rules.coax` with `ws.coax` and `sha1.coax`: 1140 of 1140
  killed, 2026-10-30,
  [2026-10-30-relay-conn.md](../../docs/verification/2026-10-30-relay-conn.md).
  The Apple TLS change later that day added `rc_closed_stream` and its
  check; [2026-10-30-apple-tls.md](../../docs/verification/2026-10-30-apple-tls.md)
  records 29 of 29 checks passing and says no full mutation run was made
  for it.

### Against local TLS endpoints

`coax/tests/net/tls.sh` generates a one-run CA and leaf certificates with
`openssl`, starts Python `ssl` endpoints on localhost, builds a driver from
`tls_ws.coax` with `tls-native.c` under AddressSanitizer and
UndefinedBehaviorSanitizer, and checks TLS 1.3 and 1.2 echoes, a leaf for
`other.test` reached as `localhost` (refused with -10, `CXTLS_NAME`), a leaf
from an untrusted CA (-9), a server capped at TLS 1.1 (-11), `ws://` at a
TLS port, an `https://` URL refused by `relay_uri`, the production build
refusing the test CA, and a relay script behind TLS.

On 2026-11-01 `coax/tests/net/tls.sh` ran on Linux 6.18.49 with GCC 15.2.0
and OpenSSL 3.6.3 under ASan and UBSan, and printed
`tls.sh: all local checks pass`. Its lines included
`PASS connect wss://localhost:44631/: got -10, want -10` (the wrong-name
certificate), `got -9` for the untrusted CA and for the production build,
`got -11` for TLS 1.1, TLS 1.3 (772) and 1.2 (771) echoes, 3 events before
EOSE from the relay behind TLS, and, in the relay session it also runs,
`the Coax client (relay_conn) over wss://: challenge, 3 stored kind 1
events, EOSE, close 1000` and `no sanitizer report`. It took about two
minutes.

The relay e2e in
[2026-10-30-relay-conn.md](../../docs/verification/2026-10-30-relay-conn.md)
runs `relay_conn.coax` itself over `ws://` and `wss://` against a local
relay: 24 of 24 checks each, and an untrusted `wss://` refused before any
WebSocket handshake. The Apple record runs the same checks on macOS. Neither
was rerun for this chapter.

## Summary

- `relay_uri` turns a relay URL into four decisions: TLS or plain, host,
  port and request target. It keeps the caller's bytes as written.
- It refuses other and uppercase schemes, user info, fragments, IP
  literals, bad DNS labels, bad ports, bad path bytes and long URLs. It
  accepts plain `ws://` to any host and IDN names only in A-label form.
- `nip42_url` is the library's only normalizer: lowercase scheme and host,
  default port dropped, trailing slashes removed, queries refused. It is for
  comparing relay names, not for connecting.
- `relay_conn_rules.coax` decides every frame and every read result;
  `relay_conn.coax` carries the decisions out, and any read size gives the
  same events.
- The C host does TCP and TLS only, with peer verification, host name
  checks and TLS 1.2 at least, and no plaintext fallback for `wss://`.

## Dependencies

- [26. WebSocket frames](ch026-websocket-frames.md)
- [Coax chapter 20, Living beside C, C++ and
  Swift](../../coax/book/ch20-interop.md)

## Sources

- [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986), URI Generic Syntax:
  authority, port, path, query and fragment.
- [RFC 1123](https://www.rfc-editor.org/rfc/rfc1123) section 2.1 and
  [RFC 1035](https://www.rfc-editor.org/rfc/rfc1035) section 2.3.1: host name
  labels.
- [RFC 6455](https://www.rfc-editor.org/rfc/rfc6455) section 3: `ws` and
  `wss` URIs and their default ports.
- [RFC 6125](https://www.rfc-editor.org/rfc/rfc6125): checking a server's
  certificate against a DNS name.
- [RFC 5891](https://www.rfc-editor.org/rfc/rfc5891): internationalized
  names and A-labels.
- [NIP-01](../nips/official/01.md) and [NIP-42](../nips/official/42.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
