# 26. WebSocket frames

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Nostr runs over WebSocket. This chapter builds the client side of RFC 6455 in
Coax: the HTTP upgrade, frame parsing and writing, masking and the closing
handshake.

Part VII, Transport. Result: RFC 6455 as a client: the opening handshake with
`Sec-WebSocket-Accept` from SHA-1, masked client frames, fragmentation, ping,
pong and close with status codes.

## What this chapter teaches

- The handshake: a random 16-byte key, `Sec-WebSocket-Accept =
  base64(SHA-1(key || 258EAFA5-E914-47DA-95CA-C5AB0DC85B11))`.
- SHA-1 here is only for the handshake value; it is not a security primitive.
- Frame header: FIN, opcode, mask bit, 7/16/64-bit length; clients mask every
  frame.
- Close codes 1000, 1002, 1003, 1009; frames over 128 KiB refuse with 1009.
- `verse/voice-host/ws.c` has a separate C implementation for the voice host;
  this chapter's code replaces it.

## What exists today

- `coax/lib/net/ws.coax` frames and checks the handshake;
  `coax/lib/crypto/sha1.coax` computes the accept value.

## Dependencies

- [4. SHA-256](ch004-sha-256.md)
- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)

## Done means

- RFC 6455 section 1.3's example key gives `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`.
- A fuzzed frame stream never reads past its buffer under ASan.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
