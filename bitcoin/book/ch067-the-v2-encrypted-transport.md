# 67. The v2 encrypted transport

Chapter 65's v1 messages travel in the clear. Anyone on the path can read
them, change them, or recognize Bitcoin traffic by its first bytes.
BIP-324's v2 transport encrypts and authenticates every message, and makes
the whole byte stream look random to an eavesdropper. It is unauthenticated
encryption: it does not prove who the peer is, but it forces an attacker to
be active, and both operators can compare a session ID to detect one.

This chapter builds the v2 transport's cryptography and framing: the keys
derived from chapter 66's shared secret, the two ciphers that rekey every
224 uses, the packet with its encrypted length and ignore bit, the garbage
and its terminator, and the short message IDs.

## What this chapter teaches

- How one shared secret becomes four cipher keys, two garbage terminators
  and a session ID, and why the salt names the network.
- FSChaCha20, the length cipher, and FSChaCha20Poly1305, the packet AEAD:
  their nonces, and how each rekeys after 224 uses for forward secrecy.
- The packet: an encrypted 3-byte length, then an AEAD over a header byte
  and the contents.
- Why the receiver decrypts the length first, and the size Core refuses.
- Garbage, terminators and decoys: how the handshake hides its shape, and
  why the garbage is authenticated.
- Short message IDs, the long form, and the rules Core reads them by.

## Where Bitcoin uses the v2 transport

| Where | What it uses | Chapter |
| --- | --- | --- |
| The handshake | the keys, the garbage scan and the version packet, over a socket | 68 |
| Connections | v2 first when a peer advertises `NODE_P2P_V2`, v1 as the fallback | 70 |
| Denial-of-service limits | the 4,000,013-byte packet limit and the 4,095-byte garbage limit | 71 |
| Fuzzing | every reader here | 119 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [v2transport.coax](../../coax/lib/bitcoin/v2transport.coax) | `v2_cipher_init(c, secret, magic, initiating)` | the keys of one side |
| | `v2_stream_crypt` | FSChaCha20 |
| | `v2_aead_seal`, `v2_aead_open` | FSChaCha20Poly1305 |
| | `v2_packet_seal(c, n, aad, ignore, packet)` | a packet, in place |
| | `v2_packet_length_vartime`, `v2_packet_open` | reading one: the length, then the rest |
| | `v2_garbage_vartime(received, terminator)` | where the garbage ends |
| | `v2_looks_like_v1_vartime(first, magic)` | a v1 peer, detected from 16 bytes |
| | `v2_contents_write_vartime`, `v2_contents_read_vartime` | message types: short IDs and the long form |
| [ellswift.coax](../../coax/lib/bitcoin/ellswift.coax) | `ellswift_create`, `ellswift_xdh` | keys and the shared secret (chapter 66) |
| [hkdf.coax](../../coax/lib/crypto/hkdf.coax) | `hkdf_extract_checked`, `hkdf_expand_checked` | HKDF-SHA256 (chapter 9) |
| [chacha20.coax](../../coax/lib/crypto/chacha20.coax) | `chacha20_block_checked` | the ChaCha20 block (chapter 11) |
| [poly1305.coax](../../coax/lib/crypto/poly1305.coax) | `poly1305_init`, `poly1305_update`, `poly1305_final` | Poly1305 (chapter 12) |

Keys, keystream and plaintext are secret. ChaCha20 and Poly1305 run in
constant time, the tag comparison is `cb_ct_equal`, and the derivation
wipes its intermediate keys. Lengths, garbage and message types are public
once they are on the wire, and those procedures end in `_vartime`
(chapter 3).

The library's codes and sizes:

<!-- source: coax/lib/bitcoin/v2transport.coax limits -->
```jai
V2_OK :: 0;
V2_TRUNCATED :: 1;      // more bytes are needed: a reader waits
V2_NO_TERMINATOR :: 2;  // 4,095 bytes of garbage and still no terminator: Core disconnects
V2_TOO_LARGE :: 3;      // a length above what Core accepts: "packet too large"
V2_BAD_TAG :: 4;        // the packet does not authenticate: "packet decryption failure"
V2_BAD_TYPE :: 5;       // contents with no message type Core knows how to read
V2_CAPACITY :: 6;       // the caller's buffer is too small
V2_KEYS :: 7;           // the key derivation refused its input

V2_LENGTH :: 3;          // the encrypted length field
V2_HEADER :: 1;          // the header byte, inside the AEAD
V2_TAG :: 16;            // Poly1305
V2_EXPANSION :: 20;      // a packet is its contents plus 20 bytes
V2_IGNORE :: 0x80;       // header bit 7: a decoy, to be dropped
V2_REKEY :: 224;         // packets (or lengths) per key
V2_TERMINATOR :: 16;     // garbage terminator
V2_MAX_GARBAGE :: 4095;  // net.h MAX_GARBAGE_LEN
V2_MAX_PACKET :: 16777215;            // 2^24 - 1, the most a 3-byte length can say
V2_MAX_CONTENTS :: 1 + 12 + 4000000;  // Core's MAX_CONTENTS_LEN: type byte, long type, message
```

## Keys

Both sides hold the same 32-byte secret from chapter 66. HKDF-SHA256
(chapter 9) extracts a key from it with the salt
`bitcoin_v2_shared_secret` followed by the network's 4-byte message start,
then expands six 32-byte outputs, each named by its info string:
`initiator_L`, `initiator_P`, `responder_L`, `responder_P`,
`garbage_terminators` and `session_id`. The initiator sends with the
`initiator_` keys and receives with the `responder_` keys; the responder
does the opposite. The first 16 bytes of `garbage_terminators` end the
initiator's garbage and the last 16 the responder's.

The message start in the salt means a mainnet node and a testnet4 node
that connect by mistake derive different keys, and the first packet fails.

<!-- source: coax/lib/bitcoin/v2transport.coax keys -->
```jai
// The keys of one side of a connection, from the 32-byte shared secret of
// chapter 66. The salt names the network, so peers on different networks
// get different keys and fail at the first packet.
V2Stream :: struct { key: [32] u8; block: u32; chunk: u64; buffer: [64] u8; at: int; }
V2Aead :: struct { key: [32] u8; packet: u64; }
V2Cipher :: struct {
  // One-element arrays, so a procedure can take each as a view.
  send_l: [1] V2Stream; recv_l: [1] V2Stream;   // FSChaCha20 for the lengths
  send_p: [1] V2Aead; recv_p: [1] V2Aead;       // FSChaCha20Poly1305 for the packets
  send_terminator: [16] u8; recv_terminator: [16] u8;
  session_id: [32] u8;
}
v2_cipher_init :: (c: [] V2Cipher, secret: [] u8, magic: [4] u8, initiating: bool) -> int {
  if c.count != 1 || secret.count != 32 { return V2_KEYS; }
  salt: [28] u8; name := "bitcoin_v2_shared_secret";
  for i: 0..<24 { salt[i] = name[i]; } for i: 0..<4 { salt[24 + i] = magic[i]; }
  prk: [32] u8; okm: [6] [32] u8; info: [32] u8;
  if hkdf_extract_checked(salt[..], secret, prk[..], false) != SHA_OK { return V2_KEYS; }
  names := [6] string.["initiator_L", "initiator_P", "responder_L", "responder_P", "garbage_terminators", "session_id"];
  ok := true;
  for k: 0..<6 { ok = ok && hkdf_expand_checked(prk[..], v2_text(names[k], info[..]), okm[k][..], 32, false) == SHA_OK; }
  if ok {
    c[0] = V2Cipher.{};
    mine := ifx initiating then 0 else 2; theirs := 2 - mine;
    c[0].send_l[0] = v2_stream(okm[mine]); c[0].send_p[0] = V2Aead.{key = okm[mine + 1]};
    c[0].recv_l[0] = v2_stream(okm[theirs]); c[0].recv_p[0] = V2Aead.{key = okm[theirs + 1]};
    for i: 0..<16 {
      first := okm[4][i]; second := okm[4][16 + i];
      c[0].send_terminator[i] = ifx initiating then first else second;
      c[0].recv_terminator[i] = ifx initiating then second else first;
    }
    c[0].session_id = okm[5];
  }
  // Forward secrecy: nothing that could rebuild the keys stays behind.
  for i: 0..<32 { prk[i] = 0; for k: 0..<6 { okm[k][i] = 0; } }
  return ifx ok then V2_OK else V2_KEYS;
}
v2_stream :: (key: [32] u8) -> V2Stream { return V2Stream.{key = key, at = 64}; }
```

Core's `BIP324Cipher::Initialize` also wipes the secret and every derived
key once the ciphers hold them, so a later memory leak cannot decrypt
earlier traffic. Here the caller owns the secret, and
`v2_cipher_init` wipes its own copies.

## The length cipher: FSChaCha20

Each packet starts with its contents' length, 3 bytes little-endian,
encrypted on its own. The length cipher is one ChaCha20 keystream that
runs across packets: 3 bytes for each length, with nonce `0 || r` where
`r` is the number of rekeys so far, and the block counter starting at 0.
After 224 lengths, the next 32 keystream bytes become the new key, and the
block counter starts again.

Rekeying is forward secrecy inside a connection. Once a key is replaced,
nothing in memory can decrypt the packets it covered.

<!-- source: coax/lib/bitcoin/v2transport.coax fschacha20 -->
```jai
// FSChaCha20, the length cipher: one keystream across many chunks, with
// nonce 0 || rekeys and the block counter from 0. After 224 chunks, the
// next 32 keystream bytes become the key. Lengths are 3-byte chunks, and
// 224 * 3 + 32 = 704 = 11 * 64, so a rekey always starts a fresh block.
v2_stream_byte :: (s: [] V2Stream) -> u8 {
  if s[0].at == 64 {
    nonce: [12] u8; stored := cb_store_le64(nonce[..], 4, s[0].chunk / V2_REKEY);
    r := chacha20_block_checked(s[0].key[..], nonce[..], s[0].block, s[0].buffer[..], false);
    s[0].block += 1; s[0].at = 0;
  }
  b := s[0].buffer[s[0].at]; s[0].buffer[s[0].at] = 0; s[0].at += 1;
  return b;
}
v2_stream_crypt :: (s: [] V2Stream, chunk: [] u8) {
  for i: 0..<chunk.count { chunk[i] ^= v2_stream_byte(s); }
  if (s[0].chunk + 1) % V2_REKEY == 0 {
    for i: 0..<32 { s[0].key[i] = v2_stream_byte(s); }
    s[0].block = 0; s[0].at = 64;
  }
  s[0].chunk += 1;
}
```

224 lengths use 672 bytes and the new key 32 more: 704 bytes, exactly 11
blocks. The rekey always lands on a block boundary, so no keystream is
left over, and an implementation that drops leftover keystream at a rekey
agrees with BIP-324's `reference.py`, which keeps it. The library's check
"the length cipher rekeys after 224 lengths" pins the boundary.

## The packet cipher: FSChaCha20Poly1305

The packet body is ChaCha20-Poly1305 (chapter 12) with a nonce that counts
packets: `p mod 224` as 4 bytes, then `p / 224` as 8 bytes, both
little-endian. After the 224th packet with a key, the new key is the first
32 bytes of the keystream for the nonce `ffffffff || p / 224`, which is
what encrypting 32 zero bytes gives. That nonce is never used for a packet,
because `p mod 224` stays below 224.

<!-- source: coax/lib/bitcoin/v2transport.coax fsaead -->
```jai
// FSChaCha20Poly1305, the packet AEAD: ChaCha20-Poly1305 with the nonce
// (packet mod 224) as 4 bytes || (packet / 224) as 8 bytes, little-endian.
// After the 224th packet with a key, the new key is the first 32 bytes of
// the keystream for nonce ffffffff || the same 8 bytes.
v2_nonce :: (packet: u64) -> [12] u8 {
  nonce: [12] u8;
  a := cb_store_le32(nonce[..], 0, cast(u32) (packet % V2_REKEY));
  b := cb_store_le64(nonce[..], 4, packet / V2_REKEY);
  return nonce;
}
v2_aead_next :: (a: [] V2Aead) {
  if (a[0].packet + 1) % V2_REKEY == 0 {
    nonce := v2_nonce(a[0].packet); for i: 0..<4 { nonce[i] = 0xff; }
    block: [64] u8;
    r := chacha20_block_checked(a[0].key[..], nonce[..], 1, block[..], false);
    for i: 0..<32 { a[0].key[i] = block[i]; } for i: 0..<64 { block[i] = 0; }
  }
  a[0].packet += 1;
}
// XOR text with the ChaCha20 keystream from block counter 1, one block at a
// time, so the length is not limited to one call's 1 MiB.
v2_xor :: (key: [] u8, nonce: [] u8, text: [] u8) {
  block: [64] u8;
  for b: 0..<(text.count + 63) / 64 {
    r := chacha20_block_checked(key, nonce, cast(u32) (b + 1), block[..], false);
    for j: 0..<64 { at := b * 64 + j; if at < text.count { text[at] ^= block[j]; } }
  }
  for j: 0..<64 { block[j] = 0; }
}
// RFC 8439's tag: Poly1305 over aad, zeros to 16, ciphertext, zeros to 16,
// and both lengths, fed to Poly1305 in pieces of at most 1 MiB.
v2_tag :: (key: [] u8, nonce: [] u8, aad: [] u8, cipher: [] u8, tag: [] u8) {
  block: [64] u8; state: [1] Poly1305State; zeros: [16] u8; lengths: [16] u8;
  r := chacha20_block_checked(key, nonce, 0, block[..], false);
  #assert poly1305_init(state[..], block[0..<32], false) == POLY_OK;
  for j: 0..<64 { block[j] = 0; }
  for part: [2] int.[0, 1] {
    text := ifx part == 0 then aad else cipher;
    at := 0;
    while at < text.count {
      n := ifx text.count - at < CB_MAX_BYTES then text.count - at else CB_MAX_BYTES;
      #assert poly1305_update(state[..], text[at..<at + n], false) == POLY_OK;
      at += n;
    }
    #assert poly1305_update(state[..], zeros[0..<(16 - text.count % 16) % 16], false) == POLY_OK;
  }
  a := cb_store_le64(lengths[..], 0, cast(u64) aad.count);
  b := cb_store_le64(lengths[..], 8, cast(u64) cipher.count);
  #assert poly1305_update(state[..], lengths[..], false) == POLY_OK;
  #assert poly1305_final(state[..], tag, false) == POLY_OK;
}
// Seal text in place and write its tag after it.
v2_aead_seal :: (a: [] V2Aead, aad: [] u8, text: [] u8, tag: [] u8) {
  nonce := v2_nonce(a[0].packet);
  v2_xor(a[0].key[..], nonce[..], text);
  v2_tag(a[0].key[..], nonce[..], aad, text, tag);
  v2_aead_next(a);
}
// Check the tag, then decrypt in place. A bad tag leaves text as it was.
// Either way the packet counts, as in Core.
v2_aead_open :: (a: [] V2Aead, aad: [] u8, text: [] u8, tag: [] u8) -> bool {
  nonce := v2_nonce(a[0].packet); want: [16] u8;
  v2_tag(a[0].key[..], nonce[..], aad, text, want[..]);
  ok := cb_ct_equal(want[..], tag) == CB_MAX64;
  if ok { v2_xor(a[0].key[..], nonce[..], text); }
  v2_aead_next(a);
  return ok;
}
```

Chapter 12's `chacha20_poly1305_seal` handles up to 1 MiB in one call, and
a v2 packet can hold almost 16 MiB. So the packet cipher calls the
ChaCha20 block itself, one block at a time, and feeds Poly1305 in pieces of
at most 1 MiB. The tag is still RFC 8439's: the AAD, zeros to a multiple of
16, the ciphertext, zeros to a multiple of 16, and the two lengths.

A packet that fails its tag still counts. Core does the same, and the
connection closes anyway.

## Packets

A packet is the encrypted length, then the AEAD of a header byte and the
contents, then the tag:

```text
3 bytes    length of the contents, encrypted with FSChaCha20
1 byte     header: bit 7 is "ignore"         \
n bytes    contents                           > FSChaCha20Poly1305
16 bytes   tag                               /
```

A packet is its contents plus 20 bytes. The library seals and opens in
place: the caller writes the contents at `packet[4..]`, sealing turns the
buffer into the packet, and opening turns it back.

<!-- source: coax/lib/bitcoin/v2transport.coax packet -->
```jai
// A packet: the encrypted 3-byte length of the contents, then the AEAD of
// the header byte and the contents, with its tag. The caller has written
// the n bytes of contents at packet[4..<4+n]; seal turns
// packet[0..<n+20] into the packet and returns n + 20, or -1.
v2_packet_seal :: (c: [] V2Cipher, n: int, aad: [] u8, ignore: bool, packet: [] u8) -> int {
  if n < 0 || n > V2_MAX_PACKET || packet.count < n + V2_EXPANSION { return -1; }
  packet[0] = cast(u8) (n & 255); packet[1] = cast(u8) ((n >> 8) & 255); packet[2] = cast(u8) (n >> 16);
  v2_stream_crypt(c[0].send_l[..], packet[0..<3]);
  packet[3] = ifx ignore then V2_IGNORE else 0;
  v2_aead_seal(c[0].send_p[..], aad, packet[3..<4 + n], packet[4 + n..<20 + n]);
  return n + V2_EXPANSION;
}
// The receiver first decrypts the 3-byte length, to know how many more
// bytes to read: n + 17. Core refuses a length above V2_MAX_CONTENTS here.
V2Length :: struct { code: int; n: int; }
v2_packet_length_vartime :: (c: [] V2Cipher, first: [] u8) -> V2Length {
  if first.count < 3 { return V2Length.{code = V2_TRUNCATED}; }
  b: [3] u8; for i: 0..<3 { b[i] = first[i]; }
  v2_stream_crypt(c[0].recv_l[..], b[..]);
  n := cast(int) b[0] + (cast(int) b[1] << 8) + (cast(int) b[2] << 16);
  return V2Length.{code = ifx n > V2_MAX_CONTENTS then V2_TOO_LARGE else V2_OK, n = n};
}
// Then the rest, rest = packet[3..<n+20], is opened in place: on success
// the contents are rest[1..<1+n].
V2Opened :: struct { code: int; ignore: bool; }
v2_packet_open :: (c: [] V2Cipher, aad: [] u8, rest: [] u8) -> V2Opened {
  if rest.count < V2_HEADER + V2_TAG { return V2Opened.{code = V2_TRUNCATED}; }
  n := rest.count - V2_HEADER - V2_TAG;
  if !v2_aead_open(c[0].recv_p[..], aad, rest[0..<1 + n], rest[1 + n..<rest.count]) { return V2Opened.{code = V2_BAD_TAG}; }
  return V2Opened.{code = V2_OK, ignore = (rest[0] & V2_IGNORE) != 0};
}
```

The receiver reads 3 bytes, decrypts the length, and only then knows how
much more to read. A 3-byte length can say 16,777,215. Core refuses any
length above 4,000,013, which is one byte for a type, 12 for a long
command and the 4,000,000 of `MAX_PROTOCOL_MESSAGE_LENGTH`. It refuses
before reading the body, so a peer cannot make it buffer 16 MiB.

A header with bit 7 set marks a decoy. The receiver authenticates it and
then drops it. Decoys let a sender pad the traffic's shape; BIP-324 leaves
when to send them to the implementation.

## Garbage, terminators and v1 peers

Each side's first bytes are its 64-byte ElligatorSwift key and then 0 to
4,095 bytes of random garbage. The garbage makes the handshake's sizes
vary, so its length does not identify the protocol. Once a side has the
other's key, it sends its garbage terminator, 16 bytes from the key
derivation, and then packets.

The receiver does not know the garbage length. It checks for the
terminator after each byte, and gives up after 4,095 bytes of garbage and
the 16 of a terminator. The first packet after the terminator carries the
garbage as its AAD, so a man in the middle who changed the garbage fails
that packet's tag.

<!-- source: coax/lib/bitcoin/v2transport.coax garbage -->
```jai
// After its 64-byte key, each side sends 0 to 4,095 bytes of garbage, then
// its 16-byte terminator. The receiver looks for the terminator after
// every byte, as Core does. The garbage becomes the AAD of the first
// packet, so a man in the middle cannot change it.
V2Garbage :: struct { code: int; n: int; }
v2_garbage_vartime :: (received: [] u8, terminator: [16] u8) -> V2Garbage {
  t := terminator;
  for end: V2_TERMINATOR..<received.count + 1 {
    if end > V2_MAX_GARBAGE + V2_TERMINATOR { return V2Garbage.{code = V2_NO_TERMINATOR}; }
    if cb_ct_equal(received[end - V2_TERMINATOR..<end], t[..]) == CB_MAX64 {
      return V2Garbage.{code = V2_OK, n = end - V2_TERMINATOR};
    }
  }
  if received.count >= V2_MAX_GARBAGE + V2_TERMINATOR { return V2Garbage.{code = V2_NO_TERMINATOR}; }
  return V2Garbage.{code = V2_TRUNCATED};
}
// A responder that reads magic || "version" and five zeros where a key
// should be is talking to a v1 peer: those are the first 16 bytes of a v1
// version message (chapter 65).
v2_looks_like_v1_vartime :: (first: [] u8, magic: [4] u8) -> bool {
  if first.count < 16 { return false; }
  want := [12] u8.['v', 'e', 'r', 's', 'i', 'o', 'n', 0, 0, 0, 0, 0];
  for i: 0..<4 { if first[i] != magic[i] { return false; } }
  for i: 0..<12 { if first[4 + i] != want[i] { return false; } }
  return true;
}
```

A v1 peer opens with a `version` message: the message start, `version`
and five zero bytes. A v2 initiator's random key starts with those 16
bytes with probability 2^-128, so a responder that sees them treats the
peer as v1. Chapter 68 does that switch.

## Short message IDs

The contents of an application packet start with the message type. A
message with a short ID spends one byte on it, instead of v1's 12. ID 0
means the 12-byte padded command of v1 follows, for messages without an
ID, such as `version` and `sendaddrv2`.

<!-- source: coax/lib/bitcoin/v2transport.coax ids -->
```jai
// Short message IDs: contents start with one byte. 1 to 28 are BIP-324's
// table; Core also assigns 37 to BIP-434's "feature". 0 means the 12-byte
// command of v1 follows. The empty strings are unassigned.
V2_MESSAGE_IDS :: [38] string.["", "addr", "block", "blocktxn", "cmpctblock", "feefilter",
  "filteradd", "filterclear", "filterload", "getblocks", "getblocktxn", "getdata",
  "getheaders", "headers", "inv", "mempool", "merkleblock", "notfound", "ping", "pong",
  "sendcmpct", "tx", "getcfilters", "cfilter", "getcfheaders", "cfheaders", "getcfcheckpt",
  "cfcheckpt", "addrv2", "", "", "", "", "", "", "", "", "feature"];
v2_message_id :: (command: string) -> int {
  for i: 1..<V2_MESSAGE_IDS.count { if V2_MESSAGE_IDS[i].count > 0 && V2_MESSAGE_IDS[i] == command { return i; } }
  return 0;
}
// Contents for a message: the short ID when there is one, else 0 and the
// padded command. Returns the length, or -1.
v2_contents_write_vartime :: (command: string, payload: [] u8, out: [] u8) -> int {
  id := v2_message_id(command);
  head := ifx id > 0 then 1 else 1 + P2P_COMMAND;
  if command.count > P2P_COMMAND || out.count < head + payload.count { return -1; }
  for i: 0..<head { out[i] = 0; }
  if id > 0 { out[0] = cast(u8) id; } else { for i: 0..<command.count { out[1 + i] = command[i]; } }
  for i: 0..<payload.count { out[head + i] = payload[i]; }
  return head + payload.count;
}
// Read contents as Core's GetMessageType does: the command and where the
// payload starts. An unknown ID, a short long-form type, or a bad padding
// is V2_BAD_TYPE. The command is either a table entry or contents[1..<1+n].
V2Contents :: struct { code: int; id: int; command_n: int; payload: int; }
v2_contents_read_vartime :: (contents: [] u8) -> V2Contents {
  if contents.count == 0 { return V2Contents.{code = V2_BAD_TYPE}; }
  id := cast(int) contents[0];
  if id != 0 {
    if id >= V2_MESSAGE_IDS.count { return V2Contents.{code = V2_BAD_TYPE, id = id}; }
    return V2Contents.{code = V2_OK, id = id, command_n = V2_MESSAGE_IDS[id].count, payload = 1};
  }
  if contents.count < 1 + P2P_COMMAND { return V2Contents.{code = V2_BAD_TYPE}; }
  n := 0;
  while n < P2P_COMMAND && contents[1 + n] != 0 {
    if contents[1 + n] < 0x20 || contents[1 + n] > 0x7e { return V2Contents.{code = V2_BAD_TYPE}; }
    n += 1;
  }
  for i: n..<P2P_COMMAND { if contents[1 + i] != 0 { return V2Contents.{code = V2_BAD_TYPE}; } }
  return V2Contents.{code = V2_OK, command_n = n, payload = 1 + P2P_COMMAND};
}
```

Core reads the long form by v1's rules: printable bytes, then zeros only.
It accepts any short ID inside its table. An unassigned one gives an empty
message type, which no handler matches, so the message is ignored. An ID
past the table's end is refused.

## The example

The example is [bitcoin/examples/v2-transport.coax](../examples/v2-transport.coax).
It runs BIP-324's seven packet vectors from the private key to the
ciphertext, and has a receiver open each one. Then two peers with keys 1
and 2 do a handshake in memory, exchange two messages, and cross a rekey.

<!-- source: bitcoin/examples/v2-transport.coax example -->
```jai
#load "../../coax/lib/bitcoin/v2transport.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex, at most 256 of them.
show :: (b: [] u8) {
  text: [512] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
CODES :: [8] string.["ok", "truncated", "no terminator", "too large", "bad tag", "bad type", "capacity", "keys"];
MAINNET :: [4] u8.[0xf9, 0xbe, 0xb4, 0xd9];

// One side of a vector row, from its key: chapter 66's shared secret,
// then this chapter's keys.
row_cipher :: (row: V2Row, initiating: bool, c: [] V2Cipher) -> bool {
  key: [32] u8; ours: [64] u8; theirs: [64] u8; secret: [32] u8; work: [1] EcdhScratch;
  code := ellswift_xdh(v2_unhex(row.ours, ours[..]), v2_unhex(row.theirs, theirs[..]),
    v2_unhex(row.priv, key[..]), row.initiating, secret[..], work[..]);
  work[0] = EcdhScratch.{}; for i: 0..<32 { key[i] = 0; }
  ok := code == EC_OK && v2_cipher_init(c, secret[..], MAINNET, initiating) == V2_OK;
  for i: 0..<32 { secret[i] = 0; }
  return ok;
}
// Send a packet whose contents are already in place at p[4..].
send :: (c: [] V2Cipher, n: int, aad: [] u8, ignore: bool, p: [] u8) -> int { return v2_packet_seal(c, n, aad, ignore, p); }
// Receive one packet in place: its contents are p[4..<4+n].
V2Got :: struct { code: int; n: int; ignore: bool; }
receive :: (c: [] V2Cipher, aad: [] u8, p: [] u8) -> V2Got {
  l := v2_packet_length_vartime(c, p[0..<3]);
  if l.code != V2_OK { return V2Got.{code = l.code, n = l.n}; }
  if l.n + V2_EXPANSION > p.count { return V2Got.{code = V2_TRUNCATED, n = l.n}; }
  o := v2_packet_open(c, aad, p[3..<l.n + V2_EXPANSION]);
  return V2Got.{code = o.code, n = l.n, ignore = o.ignore};
}

same :: (a: [32] u8, b: [32] u8) -> bool { for i: 0..<32 { if a[i] != b[i] { return false; } } return true; }

main :: () {
  // ---- BIP-324's packet vectors, from the keys to the ciphertext and back
  big: [16777235] u8; empty: [1] u8; aad: [4096] u8; part: [512] u8;
  print("packet vectors\n");
  for k: 0..<V2_PACKETS.count {
    row := V2_PACKETS[k];
    c: [1] V2Cipher; d: [1] V2Cipher;
    ok := row_cipher(row, row.initiating, c[..]) && row_cipher(row, !row.initiating, d[..]);
    keys := ok && v2_hex_is(c[0].session_id[..], row.session_id)
      && v2_hex_is(c[0].send_terminator[..], row.send_terminator) && v2_hex_is(c[0].recv_terminator[..], row.recv_terminator);
    // The row's index: that many empty decoys first, each opened by the peer.
    for i: 0..<row.idx {
      n := send(c[..], 0, empty[0..<0], true, part[..]);
      got := receive(d[..], empty[0..<0], part[0..<n]);
      if got.code != V2_OK || !got.ignore { ok = false; }
    }
    unit := v2_unhex(row.contents, part[..]); n := unit.count * row.multiply;
    for i: 0..<row.multiply { for j: 0..<unit.count { big[4 + i * unit.count + j] = unit[j]; } }
    a := v2_unhex(row.aad, aad[..]);
    m := send(c[..], n, a, row.ignore, big[..]);
    match := ifx row.ciphertext.count > 0 then v2_hex_is(big[0..<m], row.ciphertext)
      else v2_hex_is(big[m - 128..<m], row.endswith);
    got := receive(d[..], a, big[0..<m]);
    // Opened, the contents are back in place; a decoy says so.
    if got.code == V2_OK {
      if got.n != n || got.ignore != row.ignore { got.code = V2_BAD_TYPE; }
      for i: 0..<row.multiply { for j: 0..<unit.count { if big[4 + i * unit.count + j] != unit[j] { got.code = V2_BAD_TYPE; } } }
    }
    print("  row %: packet % from the %, % byte% %: keys %, % %; opened: %\n", k, row.idx,
      ifx row.initiating then "initiator" else "responder", n, ifx n == 1 then "" else "s",
      ifx row.ignore then "(a decoy)" else "(data)", ok && keys,
      ifx row.ciphertext.count > 0 then "ciphertext" else "last 128 bytes", match, CODES[got.code]);
  }

  // ---- a handshake between keys 1 and 2, in memory
  print("a handshake\n");
  ka: [32] u8; ka[31] = 1; kb: [32] u8; kb[31] = 2; ea: [64] u8; eb: [64] u8; work: [1] EcdhScratch;
  ra: [32] u8; rb: [32] u8; for i: 0..<32 { ra[i] = cast(u8) i; rb[i] = cast(u8) (32 + i); }
  created := ellswift_create(ka[..], ra[..], ea[..], work[..]) == EC_OK && ellswift_create(kb[..], rb[..], eb[..], work[..]) == EC_OK;
  work[0] = EcdhScratch.{};
  // The initiator's first bytes: its key and 7 bytes of garbage.
  wire: [4096] u8; w := 0;
  for i: 0..<64 { wire[w] = ea[i]; w += 1; }
  garbage_a := "garbage"; for i: 0..<garbage_a.count { wire[w] = garbage_a[i]; w += 1; }
  print("  initiator sends % bytes: its key and % bytes of garbage\n", w, garbage_a.count);
  print("  the responder's v1 check on the first 16 bytes: %\n", v2_looks_like_v1_vartime(wire[0..<16], MAINNET));
  // The responder answers: its key, no garbage, its terminator, its version packet.
  sa: [32] u8; sb: [32] u8; ca: [1] V2Cipher; cb: [1] V2Cipher;
  xb := ellswift_xdh(eb[..], wire[0..<64], kb[..], false, sb[..], work[..]); work[0] = EcdhScratch.{};
  ib := v2_cipher_init(cb[..], sb[..], MAINNET, false);
  back: [4096] u8; r := 0;
  for i: 0..<64 { back[r] = eb[i]; r += 1; }
  for i: 0..<16 { back[r] = cb[0].send_terminator[i]; r += 1; }
  r += send(cb[..], 0, back[64..<64], false, back[r..<4096]);   // version: empty contents, AAD = no garbage
  print("  responder sends % bytes: key, terminator, version packet\n", r);
  // The initiator derives the same keys and finds the terminator.
  xa := ellswift_xdh(ea[..], back[0..<64], ka[..], true, sa[..], work[..]); work[0] = EcdhScratch.{};
  ia := v2_cipher_init(ca[..], sa[..], MAINNET, true);
  g := v2_garbage_vartime(back[64..<r], ca[0].recv_terminator);
  print("  initiator finds the terminator: %, after % bytes of garbage\n", CODES[g.code], g.n);
  at := 64 + g.n + V2_TERMINATOR;
  got := receive(ca[..], back[64..<64 + g.n], back[at..<r]);
  print("  and reads the version packet: %, % bytes of contents\n", CODES[got.code], got.n);
  print("  both sides have session id "); show(ca[0].session_id[..]); print(": %\n", same(ca[0].session_id, cb[0].session_id));
  // The initiator's terminator, a decoy carrying its garbage as AAD, then its version packet.
  for i: 0..<16 { wire[w] = ca[0].send_terminator[i]; w += 1; }
  start := w;
  for i: 0..<10 { wire[w + 4 + i] = 0; }
  w += send(ca[..], 10, wire[64..<64 + garbage_a.count], true, wire[w..<4096]);
  w += send(ca[..], 0, empty[0..<0], false, wire[w..<4096]);
  g = v2_garbage_vartime(wire[64..<w], cb[0].recv_terminator);
  print("  responder finds the terminator after % bytes of garbage\n", g.n);
  got = receive(cb[..], wire[64..<64 + g.n], wire[start..<w]);
  print("  a decoy: %, % bytes, ignore %\n", CODES[got.code], got.n, got.ignore);
  got = receive(cb[..], empty[0..<0], wire[start + 30..<w]);
  print("  the version packet: %, ignore %\n", CODES[got.code], got.ignore);

  // ---- messages after the handshake
  print("messages\n");
  nonce := [8] u8.[1, 2, 3, 4, 5, 6, 7, 8]; p: [128] u8;
  n := v2_contents_write_vartime("ping", nonce[..], p[4..<128]); id := p[4];
  m := send(ca[..], n, empty[0..<0], false, p[..]);
  v1: [64] u8; v1n := p2p_frame_write_vartime(MAINNET, "ping", nonce[..], v1[..]);
  print("  ping: short ID %, % bytes of contents, % bytes on the wire (v1: %)\n", id, n, m, v1n);
  got = receive(cb[..], empty[0..<0], p[0..<m]);
  t := v2_contents_read_vartime(p[4..<4 + got.n]);
  print("  received: %, message %, payload ", CODES[got.code], V2_MESSAGE_IDS[t.id]); show(p[4 + t.payload..<4 + got.n]); print("\n");
  n = v2_contents_write_vartime("sendaddrv2", empty[0..<0], p[4..<128]);
  m = send(ca[..], n, empty[0..<0], false, p[..]);
  print("  sendaddrv2 has no short ID: % bytes of contents, % on the wire\n", n, m);
  got = receive(cb[..], empty[0..<0], p[0..<m]);
  t = v2_contents_read_vartime(p[4..<4 + got.n]);
  print("  received: %, message %\n", CODES[got.code], cast(string) p[5..<5 + t.command_n]);
  p[4] = 200;
  print("  contents with ID 200: %\n", CODES[v2_contents_read_vartime(p[4..<5]).code]);

  // ---- rekeying
  print("rekeying\n");
  key := ca[0].send_p[0].key; lkey := ca[0].send_l[0].key; sent := ca[0].send_p[0].packet;
  while ca[0].send_p[0].packet % V2_REKEY != 0 {
    m = send(ca[..], 0, empty[0..<0], true, p[..]);
    got = receive(cb[..], empty[0..<0], p[0..<m]);
  }
  print("  the initiator had sent % packets; % decoys later it has sent %\n", sent, ca[0].send_p[0].packet - sent, ca[0].send_p[0].packet);
  print("  the packet key changed: %, the length key changed: %\n", !same(key, ca[0].send_p[0].key), !same(lkey, ca[0].send_l[0].key));
  p[4] = 18; for i: 0..<8 { p[5 + i] = nonce[i]; }
  m = send(ca[..], 9, empty[0..<0], false, p[..]);
  print("  packet 224, the first with the new keys: "); show(p[0..<m]); print("\n");
  q := p;
  got = receive(cb[..], empty[0..<0], p[0..<m]);
  print("  the responder opens it: %, message %\n", CODES[got.code], V2_MESSAGE_IDS[cast(int) p[4]]);
  // The same bytes again: the responder's ciphers have moved on.
  got = receive(cb[..], empty[0..<0], q[0..<m]);
  print("  the same packet replayed: %, its length reads as %\n", CODES[got.code], got.n);
}
```

The example keeps a 16 MiB packet on the stack, so it runs with a larger
stack than the default 8 MiB. From the repository root:

```shell
ulimit -S -s 24576
coax/bin/coax run bitcoin/examples/v2-transport.coax
```

The output:

```text
packet vectors
  row 0: packet 1 from the initiator, 1 byte (data): keys true, ciphertext true; opened: ok
  row 1: packet 999 from the responder, 17 bytes (data): keys true, ciphertext true; opened: ok
  row 2: packet 0 from the initiator, 63 bytes (data): keys true, ciphertext true; opened: ok
  row 3: packet 223 from the responder, 128 bytes (a decoy): keys true, last 128 bytes true; opened: ok
  row 4: packet 448 from the initiator, 193 bytes (data): keys true, last 128 bytes true; opened: ok
  row 5: packet 673 from the responder, 4000001 bytes (data): keys true, last 128 bytes true; opened: ok
  row 6: packet 1024 from the initiator, 16777215 bytes (a decoy): keys true, last 128 bytes true; opened: too large
a handshake
  initiator sends 71 bytes: its key and 7 bytes of garbage
  the responder's v1 check on the first 16 bytes: false
  responder sends 100 bytes: key, terminator, version packet
  initiator finds the terminator: ok, after 0 bytes of garbage
  and reads the version packet: ok, 0 bytes of contents
  both sides have session id 1d4d114f7ded609f67be356e5bc8f18f4aa864871fd7f66ec72713ad86bf453c: true
  responder finds the terminator after 7 bytes of garbage
  a decoy: ok, 10 bytes, ignore true
  the version packet: ok, ignore false
messages
  ping: short ID 18, 9 bytes of contents, 29 bytes on the wire (v1: 32)
  received: ok, message ping, payload 0102030405060708
  sendaddrv2 has no short ID: 13 bytes of contents, 33 on the wire
  received: ok, message sendaddrv2
  contents with ID 200: bad type
rekeying
  the initiator had sent 4 packets; 220 decoys later it has sent 224
  the packet key changed: true, the length key changed: true
  packet 224, the first with the new keys: e4140e18dee69123c7d4f0eeb8d57fadd4561fef0a0dd89de0bdf7a279
  the responder opens it: ok, message ping
  the same packet replayed: truncated, its length reads as 2780197
```

What the output shows:

- Every packet vector gives BIP-324's keys, terminators and session ID
  from the private key, and its ciphertext, or its last 128 bytes for the
  long ones. Row 1 is packet 999, so it comes after 999 decoys and four
  rekeys of each cipher.
- The receiver opens every vector but the last: its 16,777,215 bytes are
  more than Core accepts, so the length is refused before the body is read.
- In the handshake the responder sends no garbage, and the initiator sends
  7 bytes, which then authenticate as the AAD of its decoy.
- A `ping` takes 29 bytes on the wire against v1's 32. Around its 8-byte
  nonce, v2 spends 21 bytes (length 3, header 1, type 1, tag 16) where v1
  spends 24 (message start 4, command 12, length 4, checksum 4).
- After 224 packets both of the initiator's sending keys change, and the
  next packet still opens.
- A replayed packet does not open: the receiver's length cipher has moved
  on, so the length decrypts to 2,780,197, more than the 29 bytes there.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/v2-transport.out](../fixtures/v2-transport.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/v2-transport.coax](../examples/checks/v2-transport.coax),
which loads the example:

- BIP-324's rows 0 and 2 from the private key, every byte of the
  ciphertext
- every one of the 168 single-bit changes to a 21-byte packet is refused
- a changed garbage byte fails the first packet's tag
- 230 packets, decoys and data in turn, open across the rekey at 224
- every short ID writes as one byte and reads back

<!-- source: bitcoin/examples/checks/v2-transport.coax check -->
```jai
// A sender and a receiver in step, from two fixed keys: no key exchange,
// so each check starts cheaply.
pair :: (c: [] V2Cipher, d: [] V2Cipher) {
  l: [32] u8; p: [32] u8; for i: 0..<32 { l[i] = cast(u8) i; p[i] = cast(u8) (100 + i); }
  c[0].send_l[0] = v2_stream(l); c[0].send_p[0] = V2Aead.{key = p};
  d[0].recv_l[0] = v2_stream(l); d[0].recv_p[0] = V2Aead.{key = p};
}
// BIP-324's rows 0 and 2 from their keys: every byte of the ciphertext.
#forall "BIP-324 packet rows 0 and 2 from the keys" k: 0..<2 #steps 400000000 {
  row := V2_PACKETS[k * 2]; c: [1] V2Cipher; p: [5000] u8; aad: [4096] u8; part: [64] u8; empty: [1] u8;
  #assert row_cipher(row, row.initiating, c[..]);
  for i: 0..<row.idx { n := send(c[..], 0, empty[0..<0], true, part[..]); }
  unit := v2_unhex(row.contents, part[..]); for j: 0..<unit.count { p[4 + j] = unit[j]; }
  m := send(c[..], unit.count, v2_unhex(row.aad, aad[..]), row.ignore, p[..]);
  #assert v2_hex_is(p[0..<m], row.ciphertext);
}
// Every one of the 168 single-bit changes to a 21-byte packet is refused:
// a changed length reads as another size, a changed body fails the tag.
#forall "one changed bit of a packet" bit: 0..<168 #steps 20000000 {
  c: [1] V2Cipher; d: [1] V2Cipher; pair(c[..], d[..]); p: [64] u8; empty: [1] u8;
  p[4] = 18; m := send(c[..], 1, empty[0..<0], false, p[..]);
  p[bit / 8] ^= cast(u8) (1 << (bit % 8));
  #assert receive(d[..], empty[0..<0], p[0..<m]).code != V2_OK;
}
// The first packet's AAD is the garbage: a changed garbage byte fails it.
#forall "the garbage is authenticated" at: 0..<7 #steps 20000000 {
  c: [1] V2Cipher; d: [1] V2Cipher; pair(c[..], d[..]); p: [64] u8;
  garbage := [7] u8.['g', 'a', 'r', 'b', 'a', 'g', 'e']; seen := garbage; seen[at] ^= 0x20;
  m := send(c[..], 10, garbage[..], true, p[..]);
  #assert receive(d[..], seen[..], p[0..<m]).code == V2_BAD_TAG;
}
// 230 packets cross the rekey after 224 on both ciphers and all open.
#test "packets open across the rekey" #steps 300000000 {
  c: [1] V2Cipher; d: [1] V2Cipher; pair(c[..], d[..]); p: [64] u8; empty: [1] u8;
  first := c[0].send_p[0].key;
  for i: 0..<230 {
    a := cast(u8) (i % 256); b := cast(u8) ((i * 7) % 256); p[4] = a; p[5] = b;
    m := send(c[..], 2, empty[0..<0], i % 2 == 0, p[..]);
    got := receive(d[..], empty[0..<0], p[0..<m]);
    #assert got.code == V2_OK && got.n == 2 && got.ignore == (i % 2 == 0) && p[4] == a && p[5] == b;
  }
  #assert !same(first, c[0].send_p[0].key) && same(c[0].send_p[0].key, d[0].recv_p[0].key);
}
// Every message with a short ID writes as one byte and reads back.
#forall "each short ID round-trips" id: 1..<38 {
  name := V2_MESSAGE_IDS[id]; out: [32] u8; payload := [1] u8.[0x5a];
  if name.count > 0 {
    n := v2_contents_write_vartime(name, payload[..], out[..]);
    r := v2_contents_read_vartime(out[0..<n]);
    #assert n == 2 && out[0] == cast(u8) id && r.code == V2_OK && r.id == id && out[r.payload] == 0x5a;
  }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/v2-transport.coax
```

`coax test` runs 257 checks, including the loaded files' own checks, in
about 30 seconds on 2026-11-03.

### The library's own checks

`v2transport.coax` is an `#authority` file. Its checks cover the keys of a
packet vector from both sides and the derivation's refusals, a vector's
packet sealed and opened again, a changed byte and a wrong AAD (and that
the failed packet still counts), the seal's size refusals, the length
limit at 4,000,013 and 4,000,014, the length cipher's rekey on its block
boundary, the AEAD's nonce and rekey, the garbage search (found, waiting,
at 4,095 bytes and past it), the v1 check on and off its network, and the
message types: short IDs, the long form, an unknown ID, a short long
form, bad padding and a byte out of range.

```shell
coax/bin/coax test coax/lib/bitcoin/v2transport.coax
```

`coax test` runs 249 checks, including those of the files it loads, in
about 30 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### BIP-324's reference and Bitcoin Core's sources

[v2transport.py](../../coax/tests/bitcoin/v2transport.py) computes every
line the example prints with BIP-324's pinned `reference.py`:
`initialize_v2_transport`, `FSChaCha20`, `FSChaCha20Poly1305` and
`v2_enc_packet`, with its decryption steps for the receiver. It reads the
short-ID table from the pinned `net.cpp` and `protocol.h`, the limits from
`net.h`, `net.cpp` and `serialize.h`, and transliterates the garbage
search and `GetMessageType` from `net.cpp`. The transcript must equal the
fixture.

```shell
python3 -I coax/tests/bitcoin/v2transport.py --coax coax/bin/coax
```

On 2026-11-03 all 30 lines agree, and the example's output in native
debug and release builds equals them. Wasm was not run. `reference.py`'s
ChaCha20 is plain Python, so the two long rows take a minute or two.

[make_v2transport_vectors.py](../../coax/tests/bitcoin/make_v2transport_vectors.py)
reads the pinned `packet_encoding_test_vectors.csv` into
[v2transport_vectors.coax](../../coax/tests/bitcoin/v2transport_vectors.coax).

## What this chapter leaves out

- Sockets, timeouts and the receive state machine: chapter 68.
- Choosing v2 or v1 for a connection, and retrying in v1: chapters 68
  and 70.
- When to send decoys and how much garbage, which BIP-324 leaves to the
  implementation.
- The `version` packet's contents, empty today and reserved for future
  transport versions.

## Summary

- HKDF turns the shared secret and the network's message start into four
  cipher keys, two garbage terminators and a session ID.
- FSChaCha20 encrypts lengths from one running keystream; FSChaCha20Poly1305
  encrypts packets with a counting nonce. Both rekey after 224 uses.
- A packet is a 3-byte encrypted length, then the AEAD of a header byte and
  the contents: 20 bytes more than the contents.
- The receiver decrypts the length first and refuses more than 4,000,013
  bytes before reading the rest.
- Garbage hides the handshake's shape, a terminator ends it, and the first
  packet authenticates it.
- A short message ID is one byte; ID 0 carries v1's 12-byte command.

## Dependencies

- [9. HKDF](ch009-hkdf.md)
- [12. Poly1305 and ChaCha20-Poly1305](ch012-poly1305-and-chacha20-poly1305.md)
- [65. P2P messages and checksums](ch065-p2p-messages-and-checksums.md)
- [66. ElligatorSwift](ch066-elligatorswift.md)

## Sources

- [BIP-324](https://github.com/bitcoin/bips/blob/master/bip-0324.mediawiki),
  "Transport layer specification" and "Application layer specification",
  with `packet_encoding_test_vectors.csv` and `reference.py`, at bips
  revision `927b6de9`, pinned in
  [coax/tests/vectors/upstream/bip324/](../../coax/tests/vectors/upstream/bip324/bip-0324.mediawiki).
- Bitcoin Core at revision `ddea9196`: `bip324.cpp` (`BIP324Cipher`),
  pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/bip324.cpp),
  and `net.cpp` (`V2Transport`, `V2_MESSAGE_IDS`), `net.h`, `protocol.h`
  and `serialize.h`, pinned there by earlier chapters.
- [RFC 8439](https://www.rfc-editor.org/rfc/rfc8439), ChaCha20 and
  Poly1305 (chapters 11 and 12).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
