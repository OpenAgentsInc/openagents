# 26. WebSocket frames

Nostr clients and relays talk over WebSocket (RFC 6455). This chapter builds
the client side in Coax: the HTTP upgrade request, the check of the server's
`Sec-WebSocket-Accept` value, masked client frames, server frame reading,
fragmented messages, ping, pong and close with status codes. The code does no
I/O. The host moves the bytes and supplies the random nonce and mask keys;
chapter 28 covers sockets and TLS, and chapter 29 joins them into a relay
connection.

## What this chapter teaches

- The opening handshake: a random 16-byte nonce becomes `Sec-WebSocket-Key`,
  and the server proves it read the request with
  `Sec-WebSocket-Accept = base64(SHA-1(key || 258EAFA5-E914-47DA-95CA-C5AB0DC85B11))`.
- Why SHA-1 is acceptable here and nowhere else in the book.
- The frame header: FIN, three reserved bits, opcode, mask bit and a 7-, 16-
  or 64-bit length. Clients mask every frame; servers mask none.
- Fragmented messages, control frames between fragments, and the rules for
  control frames.
- Close codes, and which refusal sends 1002, 1007 or 1009.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [sha1.coax](../../coax/lib/crypto/sha1.coax) | `sha1_hash(input, out)` | SHA-1 of at most 4096 bytes, for the handshake only |
| [ws.coax](../../coax/lib/net/ws.coax) | `ws_key(nonce, out)` | `Sec-WebSocket-Key` from 16 nonce bytes |
| | `ws_accept_of(key, out)` | the accept value a server must return |
| | `ws_request(host, port, path, key, out)` | the opening request |
| | `ws_response(bytes, key)` | admits or refuses the server's 101 response |
| | `ws_frame_write(op, fin, payload, mask, out)` | one masked client frame |
| | `ws_frame_read(bytes, limit)` | one server frame's header and status |
| | `ws_take(r, f, payload, msg)` | feeds a frame to the message reader |
| | `ws_close_code`, `ws_close_write`, `ws_error_code` | close payloads and the code to send for a refusal |

`ws.coax` and `sha1.coax` are `#authority`: they decide what the client
admits. Each procedure writes into a buffer the caller owns and returns a
length, a status or -1. [verse/voice-host/ws.c](../../verse/voice-host/ws.c)
is a separate C implementation that the voice host and the room server still
use; the Nostr connection uses `ws.coax`.

## The opening handshake

The client sends an HTTP/1.1 `GET` with these headers:

```text
GET / HTTP/1.1
Host: relay.example
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Sec-WebSocket-Version: 13
```

`ws_request` writes these bytes with CRLF line ends. It refuses an empty
host or one over 253 bytes, a port outside 1 to 65535, a path that does not
start with `/` or is over 2048 bytes, and any space, control byte or
non-ASCII byte in the host, path or key. So a caller can't inject a header
with `"\r\n"`. It writes `Host: name:port` only when the port is not 80. It
asks for no extension and no subprotocol.

The server answers `101 Switching Protocols`. `ws_response` admits it only
when:

1. the status line starts `HTTP/1.1 101`;
2. every header line holds visible ASCII, space or tab, and has a colon
   after a nonempty name;
3. `Upgrade` is `websocket` and `Connection` has the token `upgrade`, in
   any case;
4. there is exactly one `Sec-WebSocket-Accept`, and it equals the value
   computed from the key;
5. there is no `Sec-WebSocket-Extensions` or `Sec-WebSocket-Protocol`
   header, because the client asked for none;
6. the head, through its blank line, is at most 8192 bytes.

It returns `WS_HELLO_MORE` until the blank line arrives and, on success,
`consumed`: the bytes after it already belong to frames.

### SHA-1, only here

SHA-1 is broken for collision resistance. The handshake does not need it to
be strong: the key is public, and the accept value only shows that the
server is a WebSocket server that read this request, not an HTTP cache
replaying an old answer. `sha1.coax` says so in its header, caps its input
at 4096 bytes and has one caller. Event ids, signatures and encryption use
SHA-256 (Bitcoin chapter 4) and never SHA-1.

### Worked example: the RFC 6455 section 1.3 key

RFC 6455 section 1.3 uses the 16-byte nonce `the sample nonce`.

1. Base64 reads three bytes at a time as four 6-bit numbers. The first three
   bytes are `t h e` = `74 68 65` = `011101 000110 100001 100101` = 29, 6,
   33, 37, which are `d G h l` in the alphabet `A-Z a-z 0-9 + /`.
2. 16 bytes are five groups of three and one byte left over. The last group
   has one byte, so it gives two characters and `==`: the key has 24
   characters, `dGhlIHNhbXBsZSBub25jZQ==`.
3. Append the GUID `258EAFA5-E914-47DA-95CA-C5AB0DC85B11`: 24 + 36 = 60
   bytes, which fit in SHA-1's two 64-byte blocks once padded.
4. SHA-1 of those 60 bytes is `b37a4f2cc0624f1690f64606cf385945b2bec4ea`.
5. Base64 again. `b3 7a 4f` = `101100 110111 101001 001111` = 44, 55, 41,
   15 = `s 3 p P`. 20 bytes are six groups and two bytes left over, which
   give three characters and `=`: 28 characters,
   `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`.

That is the value in RFC 6455 section 1.3.

## Frames

Every message travels in frames. The first two bytes are:

```text
byte 0:  FIN | RSV1 | RSV2 | RSV3 | opcode (4 bits)
byte 1:  MASK | length (7 bits)
```

then 2 or 8 more length bytes when the 7-bit length is 126 or 127, then a
4-byte mask key when MASK is set, then the payload.

| Opcode | Frame |
| ---: | --- |
| 0 | continuation |
| 1 | text (UTF-8) |
| 2 | binary |
| 8 | close |
| 9 | ping |
| 10 | pong |

Opcodes 3 to 7 and 11 to 15 are reserved. The reserved bits must be 0,
because the client negotiated no extension that would give them a meaning.

### Masking

A client masks every frame it sends: payload byte `i` is XORed with mask key
byte `i mod 4`. The key is fresh and random for each frame, so a script in
a browser can't choose the bytes that reach a proxy on the wire (RFC 6455
section 10.3). A server never masks. `ws_frame_write` takes the key from the
caller, who draws it from the entropy contract; the example uses RFC 6455's
fixed key so the output can be checked.

### Worked example: a masked "Hello" by hand

RFC 6455 section 5.7 gives the client frame
`81 85 37 fa 21 3d 7f 9f 4d 51 58`.

1. `0x81` = `1000 0001`: FIN is 1, the reserved bits are 0, the opcode is
   1. This is a whole text message in one frame.
2. `0x85` = `1000 0101`: MASK is 1 and the length is 5.
3. The mask key is `37 fa 21 3d`.
4. Unmask the five payload bytes:

| i | Byte | Key byte `i mod 4` | XOR | Character |
| ---: | --- | --- | --- | --- |
| 0 | `7f` = `0111 1111` | `37` = `0011 0111` | `0100 1000` = `48` | `H` |
| 1 | `9f` = `1001 1111` | `fa` = `1111 1010` | `0110 0101` = `65` | `e` |
| 2 | `4d` = `0100 1101` | `21` = `0010 0001` | `0110 1100` = `6c` | `l` |
| 3 | `51` = `0101 0001` | `3d` = `0011 1101` | `0110 1100` = `6c` | `l` |
| 4 | `58` = `0101 1000` | `37` = `0011 0111` | `0110 1111` = `6f` | `o` |

`ws_frame_write(WS_OP_TEXT, true, "Hello", key, out)` must give these 11
bytes. The same "Hello" from a server is `81 05 48 65 6c 6c 6f`, unmasked.
If a server sends the masked form, `ws_frame_read` refuses it with
`WS_FRAME_MASKED`.

### Lengths

| Payload bytes | Byte 1 (without MASK) | Extra length bytes |
| --- | --- | --- |
| 0 to 125 | the length | none |
| 126 to 65535 | 126 | 2, big-endian |
| 65536 and up | 127 | 8, big-endian, top bit 0 |

`ws_frame_write` always uses the shortest form, and `ws_frame_read` refuses
a longer one (`WS_FRAME_LENGTH`): 125 written in the 16-bit form, 65535 in
the 64-bit form, or a 64-bit length with its top bit set. It also refuses a
frame longer than the caller's `limit` (`WS_FRAME_TOO_BIG`) as soon as the
length bytes arrive, before the payload is buffered. The relay connection
passes 128 KiB (`RC_MSG_MAX` in chapter 29) and answers a bigger frame with
close code 1009.

### Fragments and control frames

A message can be split: the first frame has opcode 1 or 2 and FIN 0, the
middle frames have opcode 0 and FIN 0, and the last has opcode 0 and FIN 1.
Control frames (close, ping, pong) may arrive between fragments, but they
can't be fragmented themselves and carry at most 125 bytes.

`ws_take` keeps one `WsReader`: whether a message is open, its opcode and
how many bytes it holds. It returns an event:

- `WS_EV_PART` for a non-final fragment, `WS_EV_MESSAGE` when the final one
  lands;
- `WS_EV_PING`, `WS_EV_PONG` and `WS_EV_CLOSE` for control frames, which
  leave an open message untouched;
- a refusal event otherwise (below).

A text message is checked as strict UTF-8 (RFC 3629: no overlong forms, no
surrogates, nothing above U+10FFFF) once it is complete, because a fragment
may end in the middle of a character.

### Closing

A close frame's payload is empty or a 2-byte big-endian code followed by a
UTF-8 reason. `ws_close_code` returns the code, 1005 (no status) for an
empty payload, or -1 for one byte, a code a peer may not send, or a reason
that is not UTF-8. Peers may send 1000 to 1003, 1007 to 1011 and 3000 to
4999. `ws_close_write(1000, key, out)` writes a masked close frame; with
RFC 6455's key it is `88 82 37 fa 21 3d 34 12`, where `03 e8` (1000) XOR
`37 fa` is `34 12`.

## What the client refuses

| Input | Result | Close code sent |
| --- | --- | ---: |
| a reserved bit set | `WS_FRAME_RSV` | 1002 |
| opcode 3 to 7 or 11 to 15 | `WS_FRAME_OPCODE` | 1002 |
| a masked server frame | `WS_FRAME_MASKED` | 1002 |
| a fragmented control frame, or one over 125 bytes | `WS_FRAME_CONTROL` | 1002 |
| a length not in its shortest form, or with its top bit set | `WS_FRAME_LENGTH` | 1002 |
| a frame over the caller's limit | `WS_FRAME_TOO_BIG` | 1009 |
| a continuation with no open message | `WS_EV_CONTINUATION` | 1002 |
| a new data frame while a message is open | `WS_EV_INTERLEAVED` | 1002 |
| a message bigger than the caller's buffer | `WS_EV_TOO_BIG` | 1009 |
| a text message that is not UTF-8 | `WS_EV_UTF8` | 1007 |
| a malformed close payload | `WS_EV_BAD_CLOSE` | 1002 |

`ws_error_code` maps the message events to their close codes. The frame
statuses are mapped by chapter 29's `rc_scan`: 1009 for `WS_FRAME_TOO_BIG`
and 1002 for the rest. On the writing side, `ws_frame_write` returns -1 for
an unknown opcode, a fragmented or over-125-byte control frame, a mask key
that is not four bytes, or a buffer that is too small.

## The example

The example is [nostr/examples/websocket-frames.coax](../examples/websocket-frames.coax).
It computes the RFC 6455 section 1.3 key and accept value, writes the
opening request and admits the RFC's response, writes and unmasks the
masked "Hello", reads server frames in each length form, assembles a
fragmented message around a ping, writes and reads close frames, shows the
refusals, and runs 20,000 fuzzed frame streams.

The fuzz loop starts from one valid 18-byte stream (a fragment, a ping, a
continuation, the final fragment and a close), changes one to four bytes,
cuts or extends it to 0 to 48 bytes, and reads it frame by frame the way a
connection does. It counts any admitted frame whose end lies past the bytes
it was given. The generator is a fixed linear congruential generator, so
every run reads the same streams.

<!-- source: nostr/examples/websocket-frames.coax example -->
```jai
#load "../../coax/lib/net/ws.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

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
// The name of a ws_frame_read status.
frame_name :: (status: int) -> string {
  names := [8] string.["OK", "MORE", "RSV", "OPCODE", "MASKED", "CONTROL", "LENGTH", "TOO_BIG"];
  return names[status];
}
// The name of a ws_take event.
event_name :: (event: int) -> string {
  names := [10] string.["PART", "MESSAGE", "PING", "PONG", "CLOSE", "CONTINUATION", "INTERLEAVED", "TOO_BIG", "UTF8", "BAD_CLOSE"];
  return names[event];
}
// Read the start of one server frame and print what its header says.
read_frame :: (label: string, hex: string, limit: int) {
  b: [64] u8;
  f := ws_frame_read(bytes_of(hex, b[..]), limit);
  print("%: % fin % op % header % length %\n", label, frame_name(f.status), f.fin, f.op, f.header, f.length);
}
// Read one complete server frame of at most 125 bytes and feed it to the
// message reader.
take :: (r: [] WsReader, hex: string, msg: [] u8) -> int {
  b: [64] u8;
  bytes := bytes_of(hex, b[..]);
  f := ws_frame_read(bytes, 125);
  #assert f.status == WS_FRAME_OK;
  return ws_take(r, f, bytes[f.header..<f.total], msg);
}
// Print an HTTP head with each CRLF shown as one line break.
show_head :: (head: string) {
  line := 0;
  for c, i: head {
    if c == '\r' { print("%\n", head[line..<i]); line = i + 2; }
  }
}
// The next value of a fixed linear congruential generator.
next :: (state: u64) -> u64 { return add_wrap(mul_wrap(state, 6364136223846793005), 1442695040888963407); }

// Change a valid frame stream at random and read it to the end, the way a
// connection does. The generator is fixed, so every run reads the same
// streams. The debug build checks every slice bound, and the C can be built
// with AddressSanitizer.
FuzzTally :: struct { frames: int; refused: int; waiting: int; past: int; }
fuzz :: (streams: int) -> FuzzTally {
  base: [32] u8;
  valid := bytes_of("010348656c89017000016c80016f880203e8", base[..]);
  state: u64 = 0x2545f4914f6cdd1d;
  buf: [48] u8; msg: [16] u8;
  t: FuzzTally;
  for s: 0..<streams {
    for i: 0..<valid.count { buf[i] = valid[i]; }
    // Change one to four bytes, then cut or extend the stream.
    state = next(state);
    for k: 0..<cast(int) (state >> 62) + 1 {
      state = next(state);
      buf[cast(int) ((state >> 40) % cast(u64) valid.count)] = cast,trunc(u8) (state >> 56);
    }
    state = next(state);
    n := cast(int) ((state >> 33) % cast(u64) (buf.count + 1));
    for i: valid.count..<n { state = next(state); buf[i] = cast,trunc(u8) (state >> 56); }
    r: [1] WsReader;
    at := 0;
    while true {
      f := ws_frame_read(cast(string) buf[at..<n], msg.count);
      if f.status == WS_FRAME_MORE { t.waiting += 1; break; }
      if f.status != WS_FRAME_OK { t.refused += 1; break; }
      if f.total > n - at { t.past += 1; break; }
      ev := ws_take(r[..], f, cast(string) buf[at + f.header..<at + f.total], msg[..]);
      t.frames += 1;
      at += f.total;
      if ev >= WS_EV_CONTINUATION { t.refused += 1; break; }
    }
  }
  return t;
}

main :: () {
  // RFC 6455 section 1.3: the key from the 16-byte nonce "the sample nonce".
  text := "the sample nonce"; nonce: [16] u8; key: [24] u8; accept: [28] u8;
  for i: 0..<16 { nonce[i] = text[i]; }
  print("key ok: %\n", ws_key(nonce[..], key[..]));
  print("Sec-WebSocket-Key: %\n", cast(string) key[..]);
  joined: [60] u8;
  for i: 0..<24 { joined[i] = key[i]; }
  for i: 0..<36 { joined[24 + i] = WS_GUID[i]; }
  digest: [20] u8;
  print("SHA-1 ok: %\n", sha1_hash(cast(string) joined[..], digest[..]));
  show("SHA-1(key || GUID)", digest[..]);
  print("accept ok: %\n", ws_accept_of(cast(string) key[..], accept[..]));
  print("Sec-WebSocket-Accept: %\n", cast(string) accept[..]);

  // The opening request and the server's answer.
  request: [256] u8;
  n := ws_request("relay.example", 80, "/", cast(string) key[..], request[..]);
  print("request: % bytes\n", n);
  show_head(cast(string) request[0..<n]);
  answer := "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
  hello := ws_response(answer, cast(string) key[..]);
  print("response: status % consumed % of %\n", hello.status, hello.consumed, answer.count);

  // RFC 6455 section 5.7: the client's masked "Hello".
  mask := [4] u8.[0x37, 0xfa, 0x21, 0x3d]; out: [70000] u8;
  n = ws_frame_write(WS_OP_TEXT, true, "Hello", mask[..], out[..]);
  show("masked Hello", out[0..<n]);
  plain: [5] u8;
  for i: 0..<5 { plain[i] = out[6 + i] ^ out[2 + i % 4]; }
  print("unmasked by hand: %\n", cast(string) plain[..]);

  // Server frames are not masked.
  read_frame("server Hello", "810548656c6c6f", 100);
  read_frame("masked server frame", "818537fa213d7f9f4d5158", 100);

  // Fragments with a ping between them.
  r: [1] WsReader; msg: [16] u8; small: [4] u8;
  print("01 03 Hel: %\n", event_name(take(r[..], "010348656c", msg[..])));
  print("89 05 Hello: %\n", event_name(take(r[..], "890548656c6c6f", msg[..])));
  print("80 02 lo: %\n", event_name(take(r[..], "80026c6f", msg[..])));
  print("message: op % \"%\"\n", r[0].op, cast(string) msg[0..<r[0].used]);

  // The three length forms.
  zero := [4] u8.[0, 0, 0, 0]; payload: [65536] u8;
  n = ws_frame_write(WS_OP_BINARY, true, cast(string) payload[0..<125], zero[..], out[..]);
  show("client header, 125 bytes", out[0..<6]);
  n = ws_frame_write(WS_OP_BINARY, true, cast(string) payload[0..<256], zero[..], out[..]);
  show("client header, 256 bytes", out[0..<8]);
  n = ws_frame_write(WS_OP_BINARY, true, cast(string) payload[..], zero[..], out[..]);
  show("client header, 65536 bytes", out[0..<14]);
  read_frame("server 256 bytes, header only", "827e0100", 1 << 20);
  read_frame("server 65536 bytes, header only", "827f0000000000010000", 1 << 20);

  // Closing.
  n = ws_close_write(WS_CLOSE_NORMAL, mask[..], out[..]);
  show("client close 1000", out[0..<n]);
  print("server close 03 e8: %, code %\n", event_name(take(r[..], "880203e8", msg[..])), ws_close_code(bytes_of("03e8", small[..])));

  // Refusals: frames.
  read_frame("RSV1 set", "c100", 100);
  read_frame("opcode 3", "8300", 100);
  read_frame("fragmented ping", "0900", 100);
  read_frame("ping of 126 bytes", "897e007e", 1000);
  read_frame("126 in the 16-bit form", "827e007d", 1000);
  read_frame("131073 bytes under a 128 KiB limit", "827f0000000000020001", 131072);
  print("ping of 126 bytes, written: %\n", ws_frame_write(WS_OP_PING, true, cast(string) payload[0..<126], mask[..], out[..]));
  print("three-byte mask, written: %\n", ws_frame_write(WS_OP_TEXT, true, "Hello", mask[0..<3], out[..]));

  // Refusals: messages, with the close code to send.
  ev := take(r[..], "8101ff", msg[..]);
  print("text ff: %, close %\n", event_name(ev), ws_error_code(ev));
  r[0] = WsReader.{};
  ev = take(r[..], "800178", msg[..]);
  print("continuation first: %, close %\n", event_name(ev), ws_error_code(ev));
  ev = take(r[..], "880203e7", msg[..]);
  print("close code 999: %, close %\n", event_name(ev), ws_error_code(ev));
  ev = take(r[..], "810548656c6c6f", small[..]);
  print("Hello into 4 bytes: %, close %\n", event_name(ev), ws_error_code(ev));

  // Refusals: the handshake.
  wrong := "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: AAAAAAAAAAAAAAAAAAAAAAAAAAA=\r\n\r\n";
  print("wrong accept: status %\n", ws_response(wrong, cast(string) key[..]).status);
  print("200 OK: status %\n", ws_response("HTTP/1.1 200 OK\r\n\r\n", cast(string) key[..]).status);
  print("request with CRLF in the host: %\n", ws_request("a\r\nX: y", 80, "/", cast(string) key[..], request[..]));

  t := fuzz(20000);
  print("fuzz: 20000 streams, % frames read, % refused, % waiting for more, % read past the buffer\n",
        t.frames, t.refused, t.waiting, t.past);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/websocket-frames.coax
```

The output matches the values worked out by hand: the key
`dGhlIHNhbXBsZSBub25jZQ==`, the digest `b37a4f2c...b2bec4ea`, the accept
value `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=` and the 11 bytes of the masked
"Hello":

```text
key ok: true
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
SHA-1 ok: true
SHA-1(key || GUID): b37a4f2cc0624f1690f64606cf385945b2bec4ea
accept ok: true
Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
request: 152 bytes
GET / HTTP/1.1
Host: relay.example
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Sec-WebSocket-Version: 13

response: status 0 consumed 129 of 129
masked Hello: 818537fa213d7f9f4d5158
unmasked by hand: Hello
server Hello: OK fin true op 1 header 2 length 5
masked server frame: MASKED fin true op 1 header 0 length 0
01 03 Hel: PART
89 05 Hello: PING
80 02 lo: MESSAGE
message: op 1 "Hello"
client header, 125 bytes: 82fd00000000
client header, 256 bytes: 82fe010000000000
client header, 65536 bytes: 82ff000000000001000000000000
server 256 bytes, header only: MORE fin true op 2 header 4 length 256
server 65536 bytes, header only: MORE fin true op 2 header 10 length 65536
client close 1000: 888237fa213d3412
server close 03 e8: CLOSE, code 1000
RSV1 set: RSV fin true op 1 header 0 length 0
opcode 3: OPCODE fin true op 3 header 0 length 0
fragmented ping: CONTROL fin false op 9 header 0 length 0
ping of 126 bytes: CONTROL fin true op 9 header 0 length 0
126 in the 16-bit form: LENGTH fin true op 2 header 0 length 0
131073 bytes under a 128 KiB limit: TOO_BIG fin true op 2 header 10 length 131073
ping of 126 bytes, written: -1
three-byte mask, written: -1
text ff: UTF8, close 1007
continuation first: CONTINUATION, close 1002
close code 999: BAD_CLOSE, close 1002
Hello into 4 bytes: TOO_BIG, close 1009
wrong accept: status 7
200 OK: status 3
request with CRLF in the host: -1
fuzz: 20000 streams, 35570 frames read, 15877 refused, 4123 waiting for more, 0 read past the buffer
```

The response is admitted with status 0 (`WS_HELLO_OK`) and consumes all 129
bytes. The 125-byte header is `82 fd`: `0xfd` is MASK plus 125. The 256-byte
header is `82 fe 01 00` and the 65536-byte header uses the 8-byte form. For
the server's 256- and 65536-byte frames only the header is given, so the
reader says `MORE` but already knows the header size and the length.

The refusals:

- `c1 00` sets RSV1, `83 00` uses opcode 3, `09 00` is a ping without FIN,
  and `89 7e 00 7e` is a 126-byte ping. Each is refused before any payload
  is read.
- `82 7e 00 7d` writes 125 in the 16-bit form: `LENGTH`.
- `82 7f 00 00 00 00 00 02 00 01` announces 131,073 bytes against a
  131,072-byte limit: `TOO_BIG`, known from the 10 header bytes alone.
- `ws_frame_write` refuses a 126-byte ping and a three-byte mask key with
  -1.
- The text frame `81 01 ff` is not UTF-8: close 1007. A continuation with no
  open message and a close with code 999 get 1002. "Hello" into a 4-byte
  message buffer gets 1009.
- A response whose accept value is wrong is refused with 7
  (`WS_HELLO_ACCEPT`), a `200 OK` with 3 (`WS_HELLO_STATUS`), and a host
  holding CRLF with -1.

In the fuzz run, 4,123 streams ended waiting for more bytes, 15,877 were
refused, and the 35,570 admitted frames all lay inside their buffers.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/websocket-frames.out](../fixtures/websocket-frames.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/websocket-frames.coax](../examples/checks/websocket-frames.coax),
which loads the example, so running the example doesn't run them:

- every example frame in RFC 6455 section 5.7: the masked and unmasked
  "Hello", the fragmented "Hel" + "lo", the unmasked ping and masked pong,
  and the 256-byte and 64 KiB binary headers;
- keys and accept values for three nonces, computed with Python's `base64`
  and `hashlib`, which share no code with `ws.coax` or `sha1.coax`;
- a frame written for every opcode and the lengths 0, 1, 125, 126, 127, 255,
  65535 and 65536, with its mask key removed, reads back with the same
  opcode, FIN bit and length;
- 2,000 random 24-byte strings, read at every prefix length: an admitted
  frame always has a 2-, 4- or 10-byte header and ends inside the input;
- the example's fuzz loop on 500 streams admits frames, refuses some, waits
  on some and reads nothing past its buffer.

<!-- source: nostr/examples/checks/websocket-frames.coax check -->
```jai
// True when bytes equal the hex text.
is_hex :: (bytes: [] u8, hex: string) -> bool {
  b: [64] u8;
  want := bytes_of(hex, b[..]);
  return cast(string) bytes == want;
}
// Every example frame in RFC 6455 section 5.7, written or read by the code.
#test "RFC 6455 section 5.7 example frames" {
  mask := [4] u8.[0x37, 0xfa, 0x21, 0x3d]; out: [70000] u8; payload: [65536] u8;
  // A single-frame masked text message, and the masked pong.
  n := ws_frame_write(WS_OP_TEXT, true, "Hello", mask[..], out[..]);
  #assert is_hex(out[0..<n], "818537fa213d7f9f4d5158");
  n = ws_frame_write(WS_OP_PONG, true, "Hello", mask[..], out[..]);
  #assert is_hex(out[0..<n], "8a8537fa213d7f9f4d5158");
  // A single-frame unmasked text message and the unmasked ping.
  b: [64] u8;
  f := ws_frame_read(bytes_of("810548656c6c6f", b[..]), 100);
  #assert f.status == WS_FRAME_OK && f.fin && f.op == WS_OP_TEXT && f.header == 2 && f.length == 5;
  f = ws_frame_read(bytes_of("890548656c6c6f", b[..]), 100);
  #assert f.status == WS_FRAME_OK && f.fin && f.op == WS_OP_PING && f.length == 5;
  // A fragmented unmasked text message.
  r: [1] WsReader; msg: [16] u8;
  #assert take(r[..], "010348656c", msg[..]) == WS_EV_PART;
  #assert take(r[..], "80026c6f", msg[..]) == WS_EV_MESSAGE && cast(string) msg[0..<r[0].used] == "Hello";
  // 256 bytes and 64 KiB of binary data: the 16-bit and 64-bit lengths.
  f = ws_frame_read(bytes_of("827e0100", b[..]), 1 << 20);
  #assert f.status == WS_FRAME_MORE && f.header == 4 && f.length == 256;
  f = ws_frame_read(bytes_of("827f0000000000010000", b[..]), 1 << 20);
  #assert f.status == WS_FRAME_MORE && f.header == 10 && f.length == 65536;
  n = ws_frame_write(WS_OP_BINARY, true, cast(string) payload[0..<256], mask[..], out[..]);
  #assert n == 4 + 4 + 256 && is_hex(out[0..<8], "82fe010037fa213d");
  n = ws_frame_write(WS_OP_BINARY, true, cast(string) payload[..], mask[..], out[..]);
  #assert n == 10 + 4 + 65536 && is_hex(out[0..<14], "82ff000000000001000037fa213d");
}
// Keys and accept values from Python's base64 and hashlib, which share no
// code with ws.coax or sha1.coax:
// base64.b64encode(hashlib.sha1(key + GUID).digest()) for key = b64(nonce).
NONCES :: [9] string.[
  "000102030405060708090a0b0c0d0e0f", "AAECAwQFBgcICQoLDA0ODw==", "Bz3qJYTGdOe8gUSpLosEdiLKDrk=",
  "ffffffffffffffffffffffffffffffff", "/////////////////////w==", "XXpj4jYzLM2yUE0C7TIgMwTQh2g=",
  "30313233343536373839616263646566", "MDEyMzQ1Njc4OWFiY2RlZg==", "BACScCJPNqyz+UBoqMH89VmURoA="];
#forall "keys and accept values match Python's hashlib" i: 0..<3 {
  b: [64] u8; key: [24] u8; accept: [28] u8;
  #assert bytes_of(NONCES[3 * i], b[..]).count == 16;
  #assert ws_key(b[0..<16], key[..]) && cast(string) key[..] == NONCES[3 * i + 1];
  #assert ws_accept_of(cast(string) key[..], accept[..]) && cast(string) accept[..] == NONCES[3 * i + 2];
}
// A client frame with its mask key removed and its mask bit cleared is the
// same frame a server would send. With a zero key the payload is unchanged,
// so the reader must give back the writer's opcode, FIN bit and length.
LENGTHS :: [8] int.[0, 1, 125, 126, 127, 255, 65535, 65536];
#forall "a written frame reads back with its length" i: 0..<8, op: 0..<11 {
  if ws_op_ok(op) && (op < 8 || LENGTHS[i] <= 125) {
    zero := [4] u8.[0, 0, 0, 0]; payload: [70000] u8; out: [70000] u8; server: [70000] u8;
    fin := op >= 8 || i % 2 == 0;
    n := ws_frame_write(op, fin, cast(string) payload[0..<LENGTHS[i]], zero[..], out[..]);
    #assert n > 0;
    header := n - LENGTHS[i];
    for k: 0..<header - 4 { server[k] = out[k]; }
    server[1] = out[1] & 127;
    for k: header..<n { server[k - 4] = out[k]; }
    f := ws_frame_read(cast(string) server[0..<n - 4], 1 << 20);
    #assert f.status == WS_FRAME_OK && f.op == op && f.fin == fin && f.length == LENGTHS[i] && f.total == n - 4;
  }
}
// Random byte strings: whatever ws_frame_read says, the frame it admits lies
// inside the bytes it was given, and its header has one of the three sizes.
#forall "the reader stays inside random input" seed: 0..<2000 {
  state: u64 = cast(u64) seed + 1;
  b: [24] u8;
  for k: 0..<24 { state = next(state); b[k] = cast,trunc(u8) (state >> 56); }
  // Clear the reserved and mask bits in half the cases, so more frames pass.
  if seed % 2 == 0 { b[0] = b[0] & 0x8f; b[1] = b[1] & 0x7f; }
  for count: 0..<25 {
    f := ws_frame_read(cast(string) b[0..<count], 16);
    if f.status == WS_FRAME_OK {
      #assert f.header == 2 || f.header == 4 || f.header == 10;
      #assert f.total == f.header + f.length && f.total <= count && f.length <= 16;
    }
  }
}
// The example's fuzz loop on a shorter run.
#test "no fuzzed frame ends past its buffer" {
  t := fuzz(500);
  #assert t.past == 0 && t.frames > 0 && t.refused > 0 && t.waiting > 0;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/websocket-frames.coax
```

On 2026-11-01 `coax test` ran 31 checks, including the loaded files' own
checks (23 from `ws.coax` and `sha1.coax`), and all passed. The checks file
defines one helper, so `coax mutate --root-only` on it finds no mutation
site. The library's mutants are below.

### The library's own checks

`sha1.coax` checks RFC 3174 section 7.3 vectors 1, 2 and 4, the empty
string, inputs of 55, 56, 63, 64, 65 and 119 bytes against Python's
`hashlib`, and the RFC 6455 digest. `ws.coax` checks the request byte for
byte, header injection, every response refusal and the head limit, every
length form and its boundaries, every frame refusal, fragment assembly,
strict UTF-8 and close payloads.

On 2026-11-01, `coax mutate coax/lib/net/ws.coax` killed 1024 of 1024
mutants (`ws.coax` with `sha1.coax` loaded), the same count as the record
[2026-10-29-nostr-relay-client.md](../../docs/verification/2026-10-29-nostr-relay-client.md),
which also records 199 of 199 for `sha1.coax` alone. `ws.coax` changed once
since, on 2026-10-31, when two locals were renamed.

### AddressSanitizer

The debug build checks every slice bound, so a read past a buffer stops the
program. The same example was also built as C under AddressSanitizer and
UndefinedBehaviorSanitizer on 2026-11-01 (x86_64, GCC 15.2,
`-O1 -fsanitize=address,undefined -fno-sanitize-recover=all`):

```shell
coax/bin/coax c nostr/examples/websocket-frames.coax > /tmp/ws.c
gcc -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer \
  -fno-sanitize-recover=all /tmp/ws.c -o /tmp/ws-asan -lm
/tmp/ws-asan
```

It exited 0 with no sanitizer report, and its output was byte for byte the
fixture, including the 20,000 fuzzed streams. These commands were run with
`coax0 0.46.0`.

### Against a real server

The record
[2026-10-30-relay-conn.md](../../docs/verification/2026-10-30-relay-conn.md)
runs the client against a local relay written with Python's standard
library, under ASan and UBSan, over `ws://` and `wss://`: 3 handshakes and
10 masked frames with distinct mask keys, 24 of 24 checks. It was not rerun
for this chapter. Chapter 27 is the server side in Coax.

## Summary

- `Sec-WebSocket-Accept` is base64 of SHA-1 of the key and a fixed GUID; the
  RFC's nonce gives `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`.
- Client frames are masked with a fresh key; the RFC's "Hello" is
  `81 85 37 fa 21 3d 7f 9f 4d 51 58`.
- `ws_frame_read` refuses reserved bits and opcodes, masked server frames,
  bad control frames, non-minimal lengths and frames over the limit, from
  the header alone.
- `ws_take` assembles fragments around control frames and checks text as
  strict UTF-8, and each refusal maps to close code 1002, 1007 or 1009.
- 20,000 fuzzed streams read nothing past their buffers in the debug build
  and under ASan.

## Dependencies

- [4. SHA-256](../../bitcoin/book/ch004-sha-256.md)
- [1. Bytes, hex and base64](../../bitcoin/book/ch001-bytes-hex-and-base64.md)

## Sources

- [RFC 6455](https://www.rfc-editor.org/rfc/rfc6455), The WebSocket
  Protocol: section 1.3 (the handshake example), 5.2 (the frame header),
  5.5 (control frames), 5.7 (example frames), 7.4 (close codes) and 10.3
  (why clients mask).
- [RFC 3174](https://www.rfc-editor.org/rfc/rfc3174), US Secure Hash
  Algorithm 1 (SHA1), section 7.3 test vectors.
- [RFC 3629](https://www.rfc-editor.org/rfc/rfc3629), UTF-8.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
