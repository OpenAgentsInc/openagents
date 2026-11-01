# 27. A WebSocket server

A relay is a WebSocket server. This chapter builds the server half of RFC
6455 in Coax: it admits a client's opening request, answers it with
`101 Switching Protocols`, reads the client's masked frames, refuses an
unmasked one, writes unmasked frames and closes. Like chapter 26's client,
the code does no I/O. It reads bytes from a buffer and writes bytes into
one, so this chapter drives a whole session with byte buffers and opens no
socket. Chapter 28 covers sockets and TLS, and the Coax relay in
`coax/relay/` moves the bytes.

## What this chapter teaches

- Validating the upgrade request: the request line, header syntax,
  `Upgrade`, `Connection`, version 13 and the key, under an 8192-byte head
  limit.
- Telling a NIP-11 request (a plain `GET` that asks for
  `application/nostr+json`) from an upgrade.
- Refusing unmasked client frames with close code 1002.
- Reassembling fragmented messages under a byte limit, with the client's
  `ws_take` from chapter 26.
- The closing handshake from the server side.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [ws_server.coax](../../coax/lib/net/ws_server.coax) | `wss_request(bytes)` | admits or refuses a client's request head; gives the key's offsets |
| | `wss_accept_write(key, out)` | the 101 response with `Sec-WebSocket-Accept` |
| | `wss_http_write(status, type, body, out)` | a complete HTTP response that closes the connection |
| | `wss_frame_read(bytes, limit)` | one client frame's header and status |
| | `wss_unmask(bytes, f, out)` | the admitted frame's payload, unmasked |
| | `wss_frame_write(op, payload, out)` | one unmasked, final server frame |
| | `wss_close_write(code, out)` | a server close frame |
| [ws.coax](../../coax/lib/net/ws.coax) | `ws_accept_of`, `ws_take`, `ws_close_code`, `ws_error_code` | shared with the client: the accept value, message assembly, close payloads and the close code for a refusal |
| [relay_route.coax](../../coax/lib/nostr/relay_route.coax) | `rr_http`, `rr_frame_read`, `rr_frame_event` | the relay's decision for each status and event |

`ws_server.coax` is `#authority`: it decides what the relay admits. It
loads `ws.coax`, so the server and the client share one SHA-1, one accept
computation, one UTF-8 check and one list of close codes. The relay program
[coax/relay/relay.coax](../../coax/relay/relay.coax) calls these procedures
from `ry_http` and `ry_frames`; the example below copies that loop without
the sockets.

## The opening request

A client asks to upgrade with an HTTP/1.1 `GET`. RFC 6455 section 1.2 gives
this request:

```text
GET /chat HTTP/1.1
Host: server.example.com
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Origin: http://example.com
Sec-WebSocket-Version: 13
```

`wss_request` reads the bytes received so far. It returns `WSS_MORE` until
it finds the blank line (`CR LF CR LF`) that ends the head. Then it checks,
in this order:

1. **The head limit.** The head, through its blank line, is at most
   `WS_HELLO_MAX` = 8192 bytes. With 8192 bytes and no blank line yet, the
   answer is `WSS_TOO_BIG`: the server stops buffering instead of waiting
   for more.
2. **The request line** is `GET /target HTTP/1.1`. The target starts with
   `/` and holds only visible ASCII: no space, control byte, DEL or byte
   above 127. Anything else is `WSS_METHOD`, including `POST` and
   `HTTP/1.0`.
3. **Every header line** holds only tab and visible ASCII or space, and has
   a colon after a nonempty name. A line without a colon, a line starting
   with a colon or a bare LF is `WSS_HEADER`.
4. **`Upgrade`** is `websocket`, in any case. Without it the request is
   not an upgrade: `WSS_UPGRADE`, or `WSS_NIP11` when an `Accept` header
   names `application/nostr+json` among its media ranges.
5. **`Connection`** has the token `upgrade` in its comma-separated list,
   in any case (`keep-alive, Upgrade` passes): else `WSS_CONNECTION`.
6. **`Sec-WebSocket-Version`** is exactly `13`: else `WSS_VERSION`.
7. **`Sec-WebSocket-Key`** appears exactly once and is base64 of 16 bytes:
   22 symbols from `A-Z a-z 0-9 + /` and then `==`. A missing key, a second
   key, 23 or 28 characters, or a `-` or `!` is `WSS_KEY`.

Header names fold case, and spaces and tabs around a value are trimmed. The
server offers no extension and no subprotocol: a request that asks for
`permessage-deflate` is admitted, and the 101 does not select it, so the
client must not use it. The `Host` and `Origin` headers are not checked; a
public relay accepts any origin.

On `WSS_OK` and `WSS_NIP11`, `consumed` is the head's length: bytes after it
are frames that arrived in the same read. On `WSS_OK`,
`key_from..<key_to` are the key's offsets in the buffer, so no copy is made.

### Worked example: admit the RFC request by hand

Count the bytes of the RFC request, each line with its `CR LF`:

| Line | Bytes | Running total |
| --- | ---: | ---: |
| `GET /chat HTTP/1.1` | 18 + 2 | 20 |
| `Host: server.example.com` | 24 + 2 | 46 |
| `Upgrade: websocket` | 18 + 2 | 66 |
| `Connection: Upgrade` | 19 + 2 | 87 |
| `Sec-WebSocket-Key: ` | 19 | 106 |
| `dGhlIHNhbXBsZSBub25jZQ==` | 24 + 2 | 132 |
| `Origin: http://example.com` | 26 + 2 | 160 |
| `Sec-WebSocket-Version: 13` | 25 + 2 | 187 |
| the blank line | 2 | 189 |

1. The blank line ends at byte 189, under 8192: the head is 189 bytes and
   `consumed` is 189.
2. The request line is 18 bytes, starts with `GET /` and ends with
   ` HTTP/1.1`. The target between them is `chat`: four visible bytes.
3. Each header has a colon after a nonempty name. `Host` and `Origin` are
   read and ignored.
4. `Upgrade` is `websocket`, `Connection` holds `Upgrade` and the version
   is `13`.
5. The key starts at byte 106 and is 24 bytes, so `key_from` is 106 and
   `key_to` is 130. Its first 22 characters are base64 symbols and the last
   two are `==`.

The answer is `WSS_OK`. The accept value is chapter 26's:
`base64(SHA-1("dGhlIHNhbXBsZSBub25jZQ==" || "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"))`
= `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`. The response is

| Line | Bytes | Running total |
| --- | ---: | ---: |
| `HTTP/1.1 101 Switching Protocols` | 32 + 2 | 34 |
| `Upgrade: websocket` | 18 + 2 | 54 |
| `Connection: Upgrade` | 19 + 2 | 75 |
| `Sec-WebSocket-Accept: ` | 22 | 97 |
| `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=` | 28 + 2 | 127 |
| the blank line | 2 | 129 |

so `wss_accept_write` returns 129, and the accept value sits at bytes
97..<125.

### What the relay answers

The relay maps each status with `rr_http`:

| Status | Relay answer |
| --- | --- |
| `WSS_MORE` | nothing yet; wait for more bytes |
| `WSS_OK` | `101 Switching Protocols`, then frames |
| `WSS_NIP11` | `200 OK` with the NIP-11 document as `application/nostr+json`, then close |
| every other status | `400 Bad Request` with a short text body, then close |

`wss_http_write` writes the 200 and 400 responses with an exact
`Content-Length`, `Connection: close` and the CORS headers NIP-11 asks for
(`Access-Control-Allow-Origin: *` and the others). Each malformed upgrade
gets 400. The relay does not tell the client which check failed.

The relay also drops a connection whose head does not arrive in time
(`rr_deadline` in `relay_route.coax`), so a client can't hold a slot open by
sending a head one byte at a time.

## Frames from the client

Chapter 26 described the frame header. The server reads it with
`wss_frame_read`, which differs from the client's `ws_frame_read` in one
rule: **every client frame must be masked** (RFC 6455 section 5.1). The
mask bit is byte 1's top bit. When it is 0 the answer is `WS_FRAME_MASKED`
and the relay closes with 1002. Because the mask key always follows the
length, the header is 6, 8 or 14 bytes, not 2, 4 or 10. On `WS_FRAME_OK` the
key is at `header - 4..<header` and the masked payload at
`header..<total`. `wss_unmask` XORs payload byte `i` with key byte
`i mod 4` into a buffer the caller owns.

The other refusals are the client's: a reserved bit (`WS_FRAME_RSV`), a
reserved opcode (`WS_FRAME_OPCODE`), a fragmented control frame or one over
125 bytes (`WS_FRAME_CONTROL`), a length not in its shortest form
(`WS_FRAME_LENGTH`), and a frame over the caller's `limit`
(`WS_FRAME_TOO_BIG`). Each is known from the header alone, before the
payload is buffered. A 64-bit length with its top bit set is
`WS_FRAME_LENGTH`, and one whose top two bytes are not 0 is
`WS_FRAME_TOO_BIG` at once, so a length is never computed past 2^48.

The relay passes `RS_MAX_MESSAGE` = 131072 bytes as the limit and holds
the assembled message in a 131072-byte buffer. `ws_take`, shared with the
client, assembles fragments around control frames and checks text as strict
UTF-8 when the last fragment lands.

### What the relay does with each frame

`rr_frame_read` maps the frame status: `WS_FRAME_MORE` waits,
`WS_FRAME_TOO_BIG` closes with 1009 and every other refusal closes with
1002. For an admitted frame, `rr_frame_event` maps `ws_take`'s event:

| Event | Relay action |
| --- | --- |
| `WS_EV_PART`, `WS_EV_PONG` | nothing; read the next frame |
| `WS_EV_MESSAGE`, text | handle the Nostr message (chapter 34 on) |
| `WS_EV_MESSAGE`, binary | a NOTICE: the relay reads text only |
| `WS_EV_PING` | a pong with the same payload |
| `WS_EV_CLOSE` | a close frame with 1000, then the connection ends |
| a refusal event | a close frame with `ws_error_code`'s code: 1007 for bad UTF-8, 1009 for a message too big, 1002 for the rest |

### Frames to the client

`wss_frame_write(op, payload, out)` writes one final, unmasked frame with
the shortest length form: 2 header bytes up to 125 payload bytes, 4 up to
65535 and 10 above. It returns -1 for a reserved opcode, a control frame
over 125 bytes or a buffer that is too small. The server never fragments;
a long message is one frame. `wss_close_write(code, out)` writes `88 02`
and the code big-endian, and returns -1 for a code a peer may not send,
such as 1005 (no status), which exists only to report an empty close.

### Closing

Either side may start the closing handshake. When the client sends a close
frame, the server answers with its own close frame and ends the
connection; RFC 6455 section 7.1.1 says the server should close the TCP
connection first. When the server refuses a frame, it sends its close
frame with the refusal's code and ends the connection without waiting for
the client's answer, because the client has already broken the protocol.

## The example

The example is [nostr/examples/websocket-server.coax](../examples/websocket-server.coax).
Its `feed` procedure is the relay's loop without the sockets: it appends the
bytes that arrived to a `Conn`, admits the request head, and then reads
frames until it needs more bytes, writing what the server sends into a
buffer. It uses the relay's own `rr_http`, `rr_frame_read` and
`rr_frame_event`, and a frame limit of 16 bytes so that `TOO_BIG` is short
to show.

The example:

1. admits the RFC 6455 request and writes the 101;
2. runs a session against chapter 26's client: the request arrives in two
   reads, the client admits the 101, sends "Hel", a ping and "lo" in one
   read, reads the pong and the echoed "Hello", and closes;
3. sends ten request heads that the server answers with 400 or 200;
4. sends six refused frames on open connections, and tries two refused
   writes;
5. admits 20,000 changed copies of the RFC request and counts any `WSS_OK`
   whose key is not a 24-byte base64 key inside the head.

<!-- source: nostr/examples/websocket-server.coax example -->
```jai
#load "../../coax/lib/net/ws_server.coax";
#load "../../coax/lib/nostr/relay_route.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// The largest frame payload this example's server admits. The Coax relay
// uses 131072 (RS_MAX_MESSAGE); a small limit makes the refusal easy to show.
LIMIT :: 16;

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [160] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  #assert r.error == 0;
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode hex into out; the bytes as a string.
bytes_of :: (text: string, out: [] u8) -> string {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return cast(string) out[0..<r.written];
}
// The name of a wss_request status.
request_name :: (status: int) -> string {
  names := [10] string.["OK", "MORE", "TOO_BIG", "METHOD", "HEADER", "UPGRADE", "CONNECTION", "VERSION", "KEY", "NIP11"];
  return names[status];
}
// Print an HTTP head with each CRLF shown as one line break.
show_head :: (head: string) {
  line := 0;
  for c, i: head {
    if c == '\r' { print("%\n", head[line..<i]); line = i + 2; }
  }
}
// The first line of an HTTP response.
first_line :: (bytes: string) -> string { return bytes[0..<wss_cr(bytes)]; }

// One connection's server state: the bytes not yet used, whether the
// upgrade happened, the message being assembled, and whether the server has
// finished with the connection.
Conn :: struct {
  ws: bool; done: bool;
  say: bool;   // print each decision (the checks run quietly)
  inb: [8400] u8; used: int;
  rd: [1] WsReader;
  msg: [LIMIT] u8;
}
// Drop the first n bytes of the input.
consume :: (c: [] Conn, n: int) {
  for i: n..<c[0].used { c[0].inb[i - n] = c[0].inb[i]; }
  c[0].used -= n;
}
// Append one written frame or response to out at at; the new length.
emit :: (out: [] u8, at: int, bytes: [] u8, n: int) -> int {
  #assert n > 0 && at + n <= out.count;
  for i: 0..<n { out[at + i] = bytes[i]; }
  return at + n;
}
// The server: take the bytes that arrived, decide, and write the bytes to
// send into out. Returns how many bytes were written. The decisions are the
// Coax relay's (rr_http, rr_frame_read, rr_frame_event in relay_route.coax).
feed :: (c: [] Conn, bytes: string, out: [] u8) -> int {
  #assert !c[0].done && c[0].used + bytes.count <= c[0].inb.count;
  for i: 0..<bytes.count { c[0].inb[c[0].used + i] = bytes[i]; }
  c[0].used += bytes.count;
  w: [256] u8;
  at := 0;
  if !c[0].ws {
    h := wss_request(cast(string) c[0].inb[0..<c[0].used]);
    a := rr_http(h.status);
    if c[0].say { print("server: request %\n", request_name(h.status)); }
    if a == RR_H_WAIT { return 0; }
    if a == RR_H_UPGRADE {
      at = emit(out, at, w[..], wss_accept_write(cast(string) c[0].inb[h.key_from..<h.key_to], w[..]));
      consume(c, h.consumed);
      c[0].ws = true;
    } else {
      // NIP-11 or a refusal: one HTTP response, then the connection ends.
      n := ifx a == RR_H_NIP11 then wss_http_write("200 OK", "application/nostr+json", "{}", w[..])
        else wss_http_write("400 Bad Request", "text/plain", "", w[..]);
      c[0].done = true;
      return emit(out, at, w[..], n);
    }
  }
  while c[0].used > 0 && !c[0].done {
    f := wss_frame_read(cast(string) c[0].inb[0..<c[0].used], LIMIT);
    fr := rr_frame_read(f.status);
    if fr.action == RR_F_WAIT { break; }
    if fr.action == RR_F_CLOSE {
      if c[0].say { print("server: frame refused, close %\n", fr.code); }
      at = emit(out, at, w[..], wss_close_write(fr.code, w[..]));
      c[0].done = true; break;
    }
    payload: [LIMIT] u8;
    n := wss_unmask(cast(string) c[0].inb[0..<c[0].used], f, payload[..]);
    ev := ws_take(c[0].rd[..], f, cast(string) payload[0..<n], c[0].msg[..]);
    consume(c, f.total);
    d := rr_frame_event(ev, c[0].rd[0].op);
    if d.action == RR_F_TEXT || d.action == RR_F_BINARY {
      if c[0].say { print("server: message op % \"%\", echoed\n", c[0].rd[0].op, cast(string) c[0].msg[0..<c[0].rd[0].used]); }
      at = emit(out, at, w[..], wss_frame_write(c[0].rd[0].op, cast(string) c[0].msg[0..<c[0].rd[0].used], w[..]));
    } else if d.action == RR_F_PONG {
      if c[0].say { print("server: ping \"%\", pong sent\n", cast(string) payload[0..<n]); }
      at = emit(out, at, w[..], wss_frame_write(WS_OP_PONG, cast(string) payload[0..<n], w[..]));
    } else if d.action == RR_F_CLOSE {
      if c[0].say { print("server: close received, close % sent\n", d.code); }
      at = emit(out, at, w[..], wss_close_write(d.code, w[..]));
      c[0].done = true;
    } else if d.action == RR_F_ERROR {
      if c[0].say { print("server: message refused, close %\n", ws_error_code(ev)); }
      at = emit(out, at, w[..], wss_close_write(ws_error_code(ev), w[..]));
      c[0].done = true;
    } else {
      if c[0].say { print("server: fragment of % bytes held\n", n); }
    }
  }
  return at;
}

// The RFC 6455 section 1.2 opening request.
RFC_REQUEST :: "GET /chat HTTP/1.1\r\nHost: server.example.com\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nOrigin: http://example.com\r\nSec-WebSocket-Version: 13\r\n\r\n";

// A fresh connection that has already been upgraded.
open_conn :: (c: [] Conn) {
  c[0] = Conn.{say = true};
  out: [256] u8;
  n := feed(c, RFC_REQUEST, out[..]);
  #assert c[0].ws && n == 129;
}
// Send one request head to a fresh connection; print the server's answer.
refuse_request :: (label: string, request: string) {
  c: [1] Conn; out: [512] u8;
  c[0].say = true;
  print("%:\n", label);
  n := feed(c[..], request, out[..]);
  print("  answer: %\n", ifx n > 0 then first_line(cast(string) out[0..<n]) else "(nothing yet)");
}
// Send frame bytes (hex) to an upgraded connection; print what comes back.
refuse_frame :: (label: string, hex: string) {
  c: [1] Conn; out: [256] u8; b: [64] u8;
  print("%:\n", label);
  open_conn(c[..]);
  n := feed(c[..], bytes_of(hex, b[..]), out[..]);
  show("  sent back", out[0..<n]);
}

// The next value of a fixed linear congruential generator.
next :: (state: u64) -> u64 { return add_wrap(mul_wrap(state, 6364136223846793005), 1442695040888963407); }

// Change the RFC request at random and admit it. Whatever the status, an
// admitted key must be a 24-byte base64 key inside the head. The generator
// is fixed, so every run reads the same requests.
FuzzTally :: struct { ok: int; nip11: int; refused: int; waiting: int; bad_key: int; }
fuzz :: (requests: int) -> FuzzTally {
  base := RFC_REQUEST;
  state: u64 = 0x2545f4914f6cdd1d;
  buf: [256] u8;
  t: FuzzTally;
  for s: 0..<requests {
    for i: 0..<base.count { buf[i] = base[i]; }
    // Change zero to three bytes, then cut or extend the request.
    state = next(state);
    for k: 0..<cast(int) (state >> 62) {
      state = next(state);
      at := cast(int) ((state >> 40) % cast(u64) base.count);
      state = next(state);
      // Half the changes pick a byte the head is made of, so more requests pass.
      if (state >> 63) == 0 { buf[at] = cast,trunc(u8) (state >> 56); }
      else { set := " :\r\n=/aA1"; buf[at] = set[cast(int) ((state >> 40) % 9)]; }
    }
    state = next(state);
    n := base.count - 8 + cast(int) ((state >> 33) % 17);
    for i: base.count..<n { state = next(state); buf[i] = cast,trunc(u8) (state >> 56); }
    h := wss_request(cast(string) buf[0..<n]);
    if h.status == WSS_OK {
      t.ok += 1;
      if h.key_from < 0 || h.key_to > h.consumed || !wss_key_ok(cast(string) buf[h.key_from..<h.key_to]) { t.bad_key += 1; }
    } else if h.status == WSS_NIP11 { t.nip11 += 1; }
    else if h.status == WSS_MORE { t.waiting += 1; }
    else { t.refused += 1; }
  }
  return t;
}

main :: () {
  // Worked example: admit the RFC request and answer it.
  h := wss_request(RFC_REQUEST);
  print("RFC request: % bytes, status %, consumed %, key at %..<%\n", RFC_REQUEST.count, request_name(h.status), h.consumed, h.key_from, h.key_to);
  print("key: %\n", RFC_REQUEST[h.key_from..<h.key_to]);
  answer: [256] u8;
  n := wss_accept_write(RFC_REQUEST[h.key_from..<h.key_to], answer[..]);
  print("101 response: % bytes\n", n);
  show_head(cast(string) answer[0..<n]);

  // A session: the Coax client of chapter 26 against this server.
  print("-- session --\n");
  text := "the sample nonce"; nonce: [16] u8; key: [24] u8;
  for i: 0..<16 { nonce[i] = text[i]; }
  #assert ws_key(nonce[..], key[..]);
  request: [256] u8; out: [512] u8; got: [512] u8;
  rn := ws_request("relay.example", 80, "/", cast(string) key[..], request[..]);
  c: [1] Conn;
  c[0].say = true;
  // The request arrives in two reads: the server waits for the blank line.
  n = feed(c[..], cast(string) request[0..<100], out[..]);
  print("after 100 of % bytes: % bytes sent\n", rn, n);
  n = feed(c[..], cast(string) request[100..<rn], out[..]);
  print("after % of % bytes: % bytes sent\n", rn, rn, n);
  hello := ws_response(cast(string) out[0..<n], cast(string) key[..]);
  print("client admits the response: status % consumed %\n", hello.status, hello.consumed);

  // The client sends "Hel", a ping and "lo" in one read, each masked.
  mask := [4] u8.[0x37, 0xfa, 0x21, 0x3d];
  m := ws_frame_write(WS_OP_TEXT, false, "Hel", mask[..], got[..]);
  m += ws_frame_write(WS_OP_PING, true, "p", mask[..], got[m..<got.count]);
  m += ws_frame_write(WS_OP_CONT, true, "lo", mask[..], got[m..<got.count]);
  show("client sends", got[0..<m]);
  n = feed(c[..], cast(string) got[0..<m], out[..]);
  show("server sends", out[0..<n]);
  // The client reads the server's frames.
  r: [1] WsReader; msg: [16] u8;
  at := 0;
  while at < n {
    f := ws_frame_read(cast(string) out[at..<n], 125);
    #assert f.status == WS_FRAME_OK;
    ev := ws_take(r[..], f, cast(string) out[at + f.header..<at + f.total], msg[..]);
    print("client reads op % \"%\"\n", f.op, cast(string) out[at + f.header..<at + f.total]);
    at += f.total;
  }

  // The closing handshake, started by the client.
  m = ws_close_write(WS_CLOSE_NORMAL, mask[..], got[..]);
  show("client sends close", got[0..<m]);
  n = feed(c[..], cast(string) got[0..<m], out[..]);
  show("server sends", out[0..<n]);
  f := ws_frame_read(cast(string) out[0..<n], 125);
  print("client reads close code %\n", ws_close_code(cast(string) out[f.header..<f.total]));

  // Refusals: the opening request. The relay answers each with 400.
  print("-- request refusals --\n");
  refuse_request("POST", "POST / HTTP/1.1\r\n\r\n");
  refuse_request("HTTP/1.0", "GET / HTTP/1.0\r\n\r\n");
  refuse_request("a header line with no colon", "GET / HTTP/1.1\r\nnocolon\r\n\r\n");
  refuse_request("no Upgrade header", "GET / HTTP/1.1\r\nHost: h\r\n\r\n");
  refuse_request("Connection: close", "GET / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: close\r\n\r\n");
  refuse_request("version 8", "GET / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 8\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n");
  refuse_request("a 23-byte key", "GET / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ=\r\n\r\n");
  refuse_request("two keys", "GET / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n");
  big: [8193] u8;
  for i: 0..<big.count { big[i] = RFC_REQUEST[i % 20]; }
  refuse_request("8193 bytes and no blank line", cast(string) big[..]);
  refuse_request("a plain GET for application/nostr+json", "GET / HTTP/1.1\r\nAccept: application/nostr+json\r\n\r\n");

  // Refusals: frames on an open connection.
  print("-- frame refusals --\n");
  refuse_frame("unmasked text Hello", "810548656c6c6f");
  refuse_frame("RSV1 set", "c18037fa213d");
  refuse_frame("17 bytes over a 16-byte limit", "819137fa213d");
  refuse_frame("text ff", "818137fa213dc8");
  refuse_frame("continuation first", "808137fa213d4f");
  refuse_frame("close code 999", "888237fa213d341d");
  p126: [126] u8;
  print("ping of 126 bytes, written: %\n", wss_frame_write(WS_OP_PING, cast(string) p126[..], out[..]));
  print("close 1005, written: %\n", wss_close_write(WS_CLOSE_NO_STATUS, out[..]));

  t := fuzz(20000);
  print("fuzz: 20000 requests, % admitted, % NIP-11, % refused, % waiting for more, % bad keys admitted\n",
        t.ok, t.nip11, t.refused, t.waiting, t.bad_key);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/websocket-server.coax
```

The output:

```text
RFC request: 189 bytes, status OK, consumed 189, key at 106..<130
key: dGhlIHNhbXBsZSBub25jZQ==
101 response: 129 bytes
HTTP/1.1 101 Switching Protocols
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=

-- session --
server: request MORE
after 100 of 152 bytes: 0 bytes sent
server: request OK
after 152 of 152 bytes: 129 bytes sent
client admits the response: status 0 consumed 129
client sends: 018337fa213d7f9f4d898137fa213d47808237fa213d5b95
server: fragment of 3 bytes held
server: ping "p", pong sent
server: message op 1 "Hello", echoed
server sends: 8a0170810548656c6c6f
client reads op 10 "p"
client reads op 1 "Hello"
client sends close: 888237fa213d3412
server: close received, close 1000 sent
server sends: 880203e8
client reads close code 1000
-- request refusals --
POST:
server: request METHOD
  answer: HTTP/1.1 400 Bad Request
HTTP/1.0:
server: request METHOD
  answer: HTTP/1.1 400 Bad Request
a header line with no colon:
server: request HEADER
  answer: HTTP/1.1 400 Bad Request
no Upgrade header:
server: request UPGRADE
  answer: HTTP/1.1 400 Bad Request
Connection: close:
server: request CONNECTION
  answer: HTTP/1.1 400 Bad Request
version 8:
server: request VERSION
  answer: HTTP/1.1 400 Bad Request
a 23-byte key:
server: request KEY
  answer: HTTP/1.1 400 Bad Request
two keys:
server: request KEY
  answer: HTTP/1.1 400 Bad Request
8193 bytes and no blank line:
server: request TOO_BIG
  answer: HTTP/1.1 400 Bad Request
a plain GET for application/nostr+json:
server: request NIP11
  answer: HTTP/1.1 200 OK
-- frame refusals --
unmasked text Hello:
server: request OK
server: frame refused, close 1002
  sent back: 880203ea
RSV1 set:
server: request OK
server: frame refused, close 1002
  sent back: 880203ea
17 bytes over a 16-byte limit:
server: request OK
server: frame refused, close 1009
  sent back: 880203f1
text ff:
server: request OK
server: message refused, close 1007
  sent back: 880203ef
continuation first:
server: request OK
server: message refused, close 1002
  sent back: 880203ea
close code 999:
server: request OK
server: message refused, close 1002
  sent back: 880203ea
ping of 126 bytes, written: -1
close 1005, written: -1
fuzz: 20000 requests, 3845 admitted, 0 NIP-11, 6478 refused, 9677 waiting for more, 0 bad keys admitted
```

The first lines match the worked example: 189 bytes, `consumed` 189, the
key at 106..<130 and a 129-byte response with the RFC's accept value.

In the session, the client's request is 152 bytes. After 100 bytes the
server sends nothing. After all 152 it sends the 129-byte 101, and the
client admits it with status 0 (`WS_HELLO_OK`). The client's 24 bytes are
three masked frames with the RFC key `37 fa 21 3d`:

| Bytes | Frame |
| --- | --- |
| `01 83 37 fa 21 3d 7f 9f 4d` | text, FIN 0, "Hel": `48 65 6c` XOR `37 fa 21` |
| `89 81 37 fa 21 3d 47` | ping, "p": `70` XOR `37` |
| `80 82 37 fa 21 3d 5b 95` | continuation, FIN 1, "lo": `6c 6f` XOR `37 fa` |

The server holds the 3-byte fragment, answers the ping between the
fragments with the pong `8a 01 70`, and echoes the assembled message as
`81 05 48 65 6c 6c 6f`, unmasked. The client's close `88 82 37 fa 21 3d 34
12` carries 1000 (`03 e8` XOR `37 fa`), and the server answers `88 02 03
e8`.

The refusals:

- `POST` and `HTTP/1.0` are `METHOD`, a header line with no colon is
  `HEADER`, a request without `Upgrade` is `UPGRADE`, `Connection: close`
  is `CONNECTION`, version 8 is `VERSION`, and a 23-byte key or two keys
  are `KEY`. 8193 bytes without a blank line are `TOO_BIG`. Every one gets
  `HTTP/1.1 400 Bad Request`.
- A plain `GET` with `Accept: application/nostr+json` is `NIP11` and gets
  `200 OK`.
- The unmasked frame `81 05 Hello` gets the close frame `88 02 03 ea`
  (1002), and so does a frame with RSV1 set.
- `81 91` announces a masked 17-byte text frame against the 16-byte limit:
  `88 02 03 f1` (1009), from the two header bytes alone.
- The text frame whose one byte unmasks to `ff` (`c8` XOR `37`) is not
  UTF-8: `88 02 03 ef` (1007). A continuation with no open message and a
  close with code 999 (`03 e7`, masked to `34 1d`) get 1002.
- `wss_frame_write` refuses a 126-byte ping and `wss_close_write` refuses
  1005, each with -1.

In the fuzz run, the changes keep 3,845 requests admissible, 6,478 are
refused, 9,677 lose their blank line and wait for more, and no admitted
request has a bad key.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/websocket-server.out](../fixtures/websocket-server.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/websocket-server.coax](../examples/checks/websocket-server.coax),
which loads the example, so running the example doesn't run them:

- the RFC 6455 sample request gets the RFC's accept value, byte for byte;
- for three keys, the 101's accept value equals the value computed with
  Python's `base64` and `hashlib`, which share no code with `ws.coax` or
  `sha1.coax`, and chapter 26's client admits the response;
- ten request heads get the status worked out by hand, and `feed` answers
  each malformed one with `400 Bad Request` and the good one with 101;
- a client frame with a random mask key and a length of 0, 1, 125, 126,
  127, 255, 65535 or 65536 unmasks on the server to the client's payload,
  and the server's unmasked reply reads back on the client;
- 2,000 random 24-byte strings, read at every prefix length: an admitted
  client frame always has a 6-, 8- or 14-byte header and ends inside the
  input;
- close frames for 1000, 1002, 1007 and 1009 are the bytes worked out by
  hand;
- the example's fuzz loop on 500 requests admits some, refuses some, waits
  on some and admits no bad key.

<!-- source: nostr/examples/checks/websocket-server.coax check -->
```jai
// RFC 6455 section 1.3: the sample request's key gives this accept value.
#test "the RFC 6455 sample request gets the RFC's accept value" {
  c: [1] Conn; out: [256] u8;
  n := feed(c[..], RFC_REQUEST, out[..]);
  #assert c[0].ws && c[0].used == 0;
  #assert cast(string) out[0..<n] == "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
}
// Keys and accept values from Python's base64 and hashlib, which share no
// code with ws.coax or sha1.coax:
// base64.b64encode(hashlib.sha1(key + GUID).digest()).
KEYS :: [6] string.[
  "AAECAwQFBgcICQoLDA0ODw==", "Bz3qJYTGdOe8gUSpLosEdiLKDrk=",
  "/////////////////////w==", "XXpj4jYzLM2yUE0C7TIgMwTQh2g=",
  "MDEyMzQ1Njc4OWFiY2RlZg==", "BACScCJPNqyz+UBoqMH89VmURoA="];
#forall "the server's accept values match Python's hashlib" i: 0..<3 {
  req: [256] u8; out: [256] u8;
  n := ws_request("relay.example", 80, "/", KEYS[2 * i], req[..]);
  h := wss_request(cast(string) req[0..<n]);
  #assert h.status == WSS_OK && h.consumed == n;
  m := wss_accept_write(cast(string) req[h.key_from..<h.key_to], out[..]);
  // The accept line starts 97 bytes in and is followed by CRLF CRLF.
  #assert m == 97 + 28 + 4 && cast(string) out[97..<125] == KEYS[2 * i + 1];
  // And the chapter 26 client admits it.
  #assert ws_response(cast(string) out[0..<m], KEYS[2 * i]).status == WS_HELLO_OK;
}
// Every malformed request head gets the status worked out by hand, and the
// relay's answer is 400 for every one of them.
BAD :: [10] string.[
  "GET / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
  "PUT / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
  "GET chat HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
  "GET / HTTP/1.1\r\nUpgrade websocket\r\n\r\n",
  "GET / HTTP/1.1\r\nUpgrade: h2c\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
  "GET / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: keep-alive\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
  "GET / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13, 8\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
  "GET / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==AAAA\r\n\r\n",
  "GET / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ-=\r\n\r\n",
  "GET / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\n\r\n"];
WANT :: [10] int.[WSS_OK, WSS_METHOD, WSS_METHOD, WSS_HEADER, WSS_UPGRADE, WSS_CONNECTION, WSS_VERSION, WSS_KEY, WSS_KEY, WSS_KEY];
#forall "each malformed request refuses with its status and a 400" i: 0..<10 {
  #assert wss_request(BAD[i]).status == WANT[i];
  c: [1] Conn; out: [512] u8;
  n := feed(c[..], BAD[i], out[..]);
  if i == 0 { #assert c[0].ws && first_line(cast(string) out[0..<n]) == "HTTP/1.1 101 Switching Protocols"; }
  else { #assert c[0].done && !c[0].ws && first_line(cast(string) out[0..<n]) == "HTTP/1.1 400 Bad Request"; }
}
// A frame the chapter 26 client writes, with any mask key and any length
// form, unmasks on the server to the same payload; the server's unmasked
// reply reads back on the client.
LENGTHS :: [8] int.[0, 1, 125, 126, 127, 255, 65535, 65536];
#forall "client frames unmask on the server and server frames read on the client" i: 0..<8, seed: 0..<2 #steps 60000000 {
  payload: [65536] u8; wire: [70000] u8; plain: [65536] u8;
  state: u64 = cast(u64) (seed * 8 + i) + 1;
  mask: [4] u8;
  for k: 0..<4 { state = next(state); mask[k] = cast,trunc(u8) (state >> 56); }
  for k: 0..<LENGTHS[i] { state = next(state); payload[k] = cast,trunc(u8) (state >> 56); }
  n := ws_frame_write(WS_OP_BINARY, true, cast(string) payload[0..<LENGTHS[i]], mask[..], wire[..]);
  f := wss_frame_read(cast(string) wire[0..<n], 1 << 20);
  #assert f.status == WS_FRAME_OK && f.total == n && f.length == LENGTHS[i];
  #assert wss_unmask(cast(string) wire[0..<n], f, plain[..]) == LENGTHS[i];
  #assert cast(string) plain[0..<LENGTHS[i]] == cast(string) payload[0..<LENGTHS[i]];
  m := wss_frame_write(WS_OP_BINARY, cast(string) plain[0..<LENGTHS[i]], wire[..]);
  g := ws_frame_read(cast(string) wire[0..<m], 1 << 20);
  #assert g.status == WS_FRAME_OK && g.total == m && g.length == LENGTHS[i];
  #assert cast(string) wire[g.header..<g.total] == cast(string) payload[0..<LENGTHS[i]];
}
// Random byte strings: a client frame wss_frame_read admits lies inside the
// bytes it was given and has a 6-, 8- or 14-byte header.
#forall "the server's frame reader stays inside random input" seed: 0..<2000 {
  state: u64 = cast(u64) seed + 1;
  b: [24] u8;
  for k: 0..<24 { state = next(state); b[k] = cast,trunc(u8) (state >> 56); }
  // Clear the reserved bits and set the mask bit in half the cases.
  if seed % 2 == 0 { b[0] = b[0] & 0x8f; b[1] = b[1] | 0x80; }
  for count: 0..<25 {
    f := wss_frame_read(cast(string) b[0..<count], 16);
    if f.status == WS_FRAME_OK {
      #assert f.header == 6 || f.header == 8 || f.header == 14;
      #assert f.total == f.header + f.length && f.total <= count && f.length <= 16;
    }
  }
}
// Close frames worked out by hand: 0x88, length 2, the code big-endian.
#test "close frames for 1000, 1002, 1007 and 1009" {
  out: [8] u8;
  codes := [4] int.[1000, 1002, 1007, 1009];
  low := [4] u8.[0xe8, 0xea, 0xef, 0xf1];
  for k: 0..<4 {
    #assert wss_close_write(codes[k], out[..]) == 4;
    #assert out[0] == 0x88 && out[1] == 2 && out[2] == 0x03 && out[3] == low[k];
  }
}
// The example's fuzz loop on a shorter run.
#test "no fuzzed request admits a bad key" {
  t := fuzz(500);
  #assert t.bad_key == 0 && t.ok > 0 && t.refused > 0 && t.waiting > 0;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/websocket-server.coax
```

On 2026-11-01 `coax test` ran 55 checks, including the loaded files' own
checks (`ws_server.coax`, `ws.coax`, `sha1.coax`, `relay_route.coax` and
`hex.coax`), and all passed.

### The library's own checks

`ws_server.coax` checks the RFC request and its response byte for byte,
case folding and `Connection` lists, every request refusal, the target's
allowed bytes, the key alphabet, NIP-11 detection, the exact head limit
(8192 bytes admitted, 8193 refused, 8191 without a blank line waiting),
HTTP responses with exact lengths, the RFC 6455 masked "Hello", every
length form and its boundaries, every client frame refusal, and server
frames in each length form.

On 2026-11-01, `coax mutate coax/lib/net/ws_server.coax --root-only` killed
390 of 390 mutants in `ws_server.coax`'s own procedures. The full run with
`ws.coax` and `sha1.coax` mutated as well, 1414 of 1414 killed in 3 shards,
is in the record
[2026-10-29-coax-relay-r1.md](../../docs/verification/2026-10-29-coax-relay-r1.md)
(`b01a65cfe3`). It was not rerun: since then `ws_server.coax` changed once,
on 2026-10-31, when a local in one check was renamed, and chapter 26 reran
`ws.coax` today (1024 of 1024).

### AddressSanitizer

The session in the example uses chapter 26's client against this server.
The example was built as C under AddressSanitizer and
UndefinedBehaviorSanitizer on 2026-11-01 (x86_64, GCC 15.2):

```shell
coax/bin/coax c nostr/examples/websocket-server.coax > /tmp/wss.c
gcc -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer \
  -fno-sanitize-recover=all /tmp/wss.c -o /tmp/wss-asan -lm
/tmp/wss-asan
```

It exited 0 with no sanitizer report, and its output was byte for byte the
fixture, including the 20,000 fuzzed requests. These commands were run with
`coax0 0.46.0`.

### Against real sockets

The relay records run this code over TCP. The record
[2026-10-29-coax-relay-r1.md](../../docs/verification/2026-10-29-coax-relay-r1.md)
lists the server's checks and the relay program. They were not rerun for
this chapter.

## Summary

- `wss_request` admits `GET /target HTTP/1.1` with `Upgrade: websocket`, a
  `Connection` list holding `upgrade`, version 13 and exactly one 16-byte
  base64 key, in a head of at most 8192 bytes. The RFC request is 189 bytes
  with its key at 106..<130.
- The relay answers each malformed request with 400, a NIP-11 request with
  200, and an upgrade with a 129-byte 101.
- Client frames must be masked; an unmasked one gets close 1002. Frames
  over the limit get 1009, bad UTF-8 1007, and other protocol errors 1002.
- The server writes unmasked frames in the shortest length form, answers
  pings with pongs, and answers a close with a close.
- A whole session runs on byte buffers, with no socket, in the debug build
  and under ASan and UBSan.

## Dependencies

- [26. WebSocket frames](ch026-websocket-frames.md)

## Sources

- [RFC 6455](https://www.rfc-editor.org/rfc/rfc6455), The WebSocket
  Protocol: sections 1.2 and 1.3 (the handshake example), 4.2 (the
  server's opening handshake), 5.1 (client frames are masked), 5.2 (the
  frame header), 7.1.1 (closing the connection) and 7.4 (close codes).
- [NIP-11](../nips/official/11.md), Relay Information Document.
- [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110), HTTP Semantics:
  header fields and the `Connection` and `Upgrade` headers.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
