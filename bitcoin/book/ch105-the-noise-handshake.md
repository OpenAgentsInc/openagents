# 105. The Noise handshake

Lightning nodes never send a message in the clear. BOLT 8 opens every
connection with a three-act handshake, the Noise protocol's XK pattern on
secp256k1, ChaCha20-Poly1305 and SHA-256. The handshake proves the
initiator knows the responder's static key, hides the initiator's own
key from everyone else, and gives the two nodes a pair of session keys.
After it, each message travels as an encrypted length and an encrypted
body, and each key rotates every 1,000 uses.

This chapter builds the handshake and the transport. They carry chapter
104's messages, and every later Lightning chapter talks to its peer
through them.

## What this chapter teaches

- Noise's handshake state: the handshake hash `h`, which covers every
  byte sent and received, and the chaining key `ck`, which absorbs every
  shared secret.
- Lightning's ECDH: SHA-256 of the compressed shared point.
- HKDF with empty info and 64 bytes out, which turns `ck` and a shared
  secret into the next `ck` and an encryption key.
- The three acts byte by byte: 50, 50 and 66 bytes, each with a version
  byte and a tag.
- The transport: a 2-byte length sealed into 18 bytes, then the body and
  its 16-byte tag, with a 96-bit nonce that counts each use of a key.
- Key rotation after 1,000 uses, which is every 500 messages.
- Which values are secret, and how the code keeps them out of branches and
  off the stack.

## Where Lightning uses the transport

| Where | What it carries | Chapter |
| --- | --- | --- |
| Every peer connection | `init`, `error`, `ping` and `pong` | 104 |
| Channels | `open_channel`, `commitment_signed` and the others | 109 |
| Gossip | channel and node announcements | 112 |
| Onion messages | BOLT 12 offers and invoice requests | 115 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [noise.coax](../../coax/lib/lightning/noise.coax) | `noise_public`, `noise_ecdh`, `noise_hkdf` | BOLT 8's functions |
| | `noise_handshake_init(hs, initiator, s_priv, responder)` | the starting `h` and `ck` |
| | `noise_act_one_write`, `noise_act_one_read` | act one |
| | `noise_act_two_write`, `noise_act_two_read` | act two |
| | `noise_act_three_write`, `noise_act_three_read` | act three, and the session keys |
| | `noise_encrypt`, `noise_decrypt_length`, `noise_decrypt_body` | messages |
| | `noise_used` | the nonce, and rotation |
| [hkdf.coax](../../coax/lib/crypto/hkdf.coax) | `hkdf_extract_checked`, `hkdf_expand_checked` | HKDF (chapter 9) |
| [chacha20_poly1305.coax](../../coax/lib/crypto/chacha20_poly1305.coax) | `chacha20_poly1305_seal`, `chacha20_poly1305_open` | the AEAD (chapter 12) |
| [secp_mult_const.coax](../../coax/lib/crypto/secp_mult_const.coax) | `smc_multiply` | constant-time scalar multiplication (chapter 16) |
| [secp_encode.coax](../../coax/lib/crypto/secp_encode.coax) | `se_decode`, `se_encode` | compressed points (chapter 17) |

The code is cryptography and framing only. It opens no socket and keeps
no timer: the caller reads exactly 50, 50 and 66 bytes for the acts, then
18 bytes and the length they give for each message.

The library's codes and sizes:

<!-- source: coax/lib/lightning/noise.coax codes -->
```jai
NOISE_OK :: 0;
NOISE_TRUNCATED :: 1;       // an act or a message shorter than its fixed size: "READ_FAILED"
NOISE_BAD_VERSION :: 2;     // a handshake version other than 0
NOISE_BAD_PUBKEY :: 3;      // a public key that is not a compressed point
NOISE_BAD_TAG :: 4;         // a MAC that does not check: abort with no reply
NOISE_BAD_CIPHERTEXT :: 5;  // act three's encrypted static key fails its MAC
NOISE_BAD_KEY :: 6;         // a secret key of zero or at least n
NOISE_STATE :: 7;           // an act out of order
NOISE_TOO_LARGE :: 8;       // a message above 65,535 bytes
NOISE_CAPACITY :: 9;        // the caller's buffer is too small

NOISE_ACT_ONE :: 50;        // version 1, ephemeral key 33, tag 16
NOISE_ACT_TWO :: 50;
NOISE_ACT_THREE :: 66;      // version 1, encrypted static key 33 + 16, tag 16
NOISE_HEADER :: 18;         // encrypted 2-byte length and its tag
NOISE_TAG :: 16;
NOISE_MAX_MESSAGE :: 65535;
NOISE_ROTATE :: 1000;       // uses of a key before it rotates
```

## What is secret

The static and ephemeral secret keys, every ECDH output, the chaining
key, the temporary keys and the session keys are secret. Chapter 3's
rules apply to all of them:

- Scalar multiplication is chapter 16's constant-time ladder, and SHA-256,
  HKDF and ChaCha20-Poly1305 run in constant time.
- The AEAD compares tags in constant time, so a forged tag fails in the
  same time whichever byte is wrong.
- No branch depends on a secret. The code branches on the act number, a
  version byte, a length and whether a tag checked, all of which an
  observer of the connection already knows.
- Each procedure overwrites the secrets it leaves on the stack, and the
  handshake state forgets its secrets when the session keys are made.

## BOLT 8's functions

Noise needs a Diffie-Hellman function, a hash and an AEAD. Lightning's
ECDH multiplies a public point by a secret key and hashes the compressed
result, parity byte included. Both sides compute the same point, so both
get the same 32 bytes. HKDF (chapter 9) runs with the chaining key as
salt, the new secret as input key material, empty info and 64 bytes out.
The first 32 bytes are the new chaining key, the last 32 an encryption
key.

The AEAD is chapter 12's ChaCha20-Poly1305. Its 96-bit nonce is 4 zero
bytes and then a 64-bit counter in little-endian. During the handshake the
associated data is `h`, so each tag also proves the two sides agree on
everything said so far.

<!-- source: coax/lib/lightning/noise.coax primitives -->
```jai
// BOLT 8's functions. ECDH(k, P) is SHA-256 of the compressed point k * P.
// Both sides of a connection compute the same point, and hashing its
// compressed form, parity byte included, gives the same 32 bytes.
NoisePublic :: struct { ok: bool; bytes: [33] u8; }
noise_public :: (secret: [] u8) -> NoisePublic {
  r: NoisePublic; k := sc_load(secret);
  if !k.ok || sc_is_zero(k.value) { return r; }
  p := smc_multiply(k.value, sg_generator());
  r.ok = p.ok && se_encode(p.value, SE_COMPRESSED, r.bytes[..]) == SE_COMPRESSED;
  k = ScLoad.{}; p = SgResult.{};
  return r;
}
noise_ecdh :: (secret: [] u8, peer: [] u8, out: [] u8) -> int {
  if peer.count != 33 { return NOISE_BAD_PUBKEY; }
  point := se_decode(peer);
  if !point.ok { return NOISE_BAD_PUBKEY; }
  k := sc_load(secret);
  if !k.ok || sc_is_zero(k.value) { return NOISE_BAD_KEY; }
  shared := smc_multiply(k.value, point.value);
  compressed: [33] u8;
  n := se_encode(shared.value, SE_COMPRESSED, compressed[..]);
  #assert sha256_hash_checked(compressed[..], out, false) == SHA_OK;
  k = ScLoad.{}; shared = SgResult.{}; noise_wipe(compressed[..]);
  return ifx n == SE_COMPRESSED then NOISE_OK else NOISE_BAD_PUBKEY;
}
// HKDF(salt, ikm) with empty info and 64 bytes out: the first 32 replace
// one key, the last 32 another.
noise_hkdf :: (salt: [] u8, ikm: [] u8, first: [] u8, second: [] u8) {
  prk: [32] u8; okm: [64] u8; empty: [1] u8;
  #assert hkdf_extract_checked(salt, ikm, prk[..], false) == SHA_OK;
  #assert hkdf_expand_checked(prk[..], empty[0..<0], okm[..], 64, false) == SHA_OK;
  for i: 0..<32 { first[i] = okm[i]; second[i] = okm[32 + i]; }
  noise_wipe(prk[..]); noise_wipe(okm[..]);
}
// The nonce: 32 zero bits, then n as a little-endian 64-bit number.
noise_nonce :: (n: u64) -> [12] u8 { nonce: [12] u8; stored := cb_store_le64(nonce[..], 4, n); return nonce; }
noise_seal :: (key: [] u8, n: u64, ad: [] u8, plain: [] u8, out: [] u8) {
  nonce := noise_nonce(n);
  #assert chacha20_poly1305_seal(key, nonce[..], ad, plain, out[0..<plain.count + NOISE_TAG], false) == CCP_OK;
}
noise_open :: (key: [] u8, n: u64, ad: [] u8, box: [] u8, out: [] u8) -> bool {
  nonce := noise_nonce(n);
  return chacha20_poly1305_open(key, nonce[..], ad, box, out[0..<box.count - NOISE_TAG], false) == CCP_OK;
}
```

## The handshake state

Each side keeps `h`, `ck`, a temporary key, its own static and ephemeral
keys, and the peer's keys as it learns them. Both sides start the same
way:

1. `h` = SHA-256 of the protocol name,
   `Noise_XK_secp256k1_ChaChaPoly_SHA256`.
2. `ck` = `h`.
3. `h` = SHA-256(`h` || `lightning`), the prologue.
4. `h` = SHA-256(`h` || the responder's static public key).

The initiator must already know the responder's static key, from gossip
or from a node URI. That is the "K" in XK.

<!-- source: coax/lib/lightning/noise.coax state -->
```jai
// One side's handshake. h hashes everything sent and received; ck chains
// every ECDH output. Both start from the protocol name and the prologue,
// then mix in the responder's static public key.
NoiseHandshake :: struct {
  initiator: bool; act: int;
  h: [32] u8; ck: [32] u8; temp_k: [32] u8;
  s_priv: [32] u8; s_pub: [33] u8;     // ours
  e_priv: [32] u8; e_pub: [33] u8;     // ours, this session only
  rs: [33] u8; re: [33] u8;            // theirs
}
NOISE_PROTOCOL :: "Noise_XK_secp256k1_ChaChaPoly_SHA256";
NOISE_PROLOGUE :: "lightning";
// The initiator knows the responder's static key; the responder passes
// its own. Returns NOISE_OK or NOISE_BAD_KEY or NOISE_BAD_PUBKEY.
noise_handshake_init :: (hs: [] NoiseHandshake, initiator: bool, s_priv: [] u8, responder: [] u8) -> int {
  if hs.count != 1 || s_priv.count != 32 || responder.count != 33 || !se_decode(responder).ok { return NOISE_BAD_PUBKEY; }
  mine := noise_public(s_priv);
  if !mine.ok { return NOISE_BAD_KEY; }
  hs[0] = NoiseHandshake.{initiator = initiator, act = 1, s_pub = mine.bytes};
  for i: 0..<32 { hs[0].s_priv[i] = s_priv[i]; }
  for i: 0..<33 { hs[0].rs[i] = ifx initiator then responder[i] else 0; }
  name: [36] u8; for i: 0..<36 { name[i] = NOISE_PROTOCOL[i]; }
  prologue: [9] u8; for i: 0..<9 { prologue[i] = NOISE_PROLOGUE[i]; }
  #assert sha256_hash_checked(name[..], hs[0].h[..], false) == SHA_OK;
  hs[0].ck = hs[0].h;
  h := hs[0].h; noise_sha(h[..], prologue[..], hs[0].h[..]);
  h = hs[0].h; noise_sha(h[..], responder, hs[0].h[..]);
  return NOISE_OK;
}
```

## Act one

The initiator makes an ephemeral key `e` and sends its public key. It then
computes `es`, the ECDH of its ephemeral secret and the responder's static
key, and mixes it into `ck`. The act ends with a tag over `h` and an empty
plaintext. Only the holder of the responder's static secret can compute
`es`, so only the intended responder can check the tag.

Act one is 50 bytes: a version byte (0), the 33-byte ephemeral public key
and a 16-byte tag. The reader checks the length, then the version, then
the key, then the tag, in that order, and stops at the first failure.

<!-- source: coax/lib/lightning/noise.coax act1 -->
```jai
// Act one, initiator to responder: -> e, es. The initiator proves it knows
// the responder's static key: only that key's holder can compute es and
// check the tag.
noise_act_one_write :: (hs: [] NoiseHandshake, e_priv: [] u8, out: [] u8) -> int {
  if hs[0].act != 1 || !hs[0].initiator { return NOISE_STATE; }
  if out.count < NOISE_ACT_ONE { return NOISE_CAPACITY; }
  e := noise_public(e_priv);
  if !e.ok { return NOISE_BAD_KEY; }
  for i: 0..<32 { hs[0].e_priv[i] = e_priv[i]; } hs[0].e_pub = e.bytes;
  h := hs[0].h; noise_sha(h[..], e.bytes[..], hs[0].h[..]);
  es: [32] u8; code := noise_ecdh(e_priv, hs[0].rs[..], es[..]);
  ck := hs[0].ck; noise_hkdf(ck[..], es[..], hs[0].ck[..], hs[0].temp_k[..]); noise_wipe(es[..]); noise_wipe(ck[..]);
  empty: [1] u8; c: [16] u8;
  noise_seal(hs[0].temp_k[..], 0, hs[0].h[..], empty[0..<0], c[..]);
  h = hs[0].h; noise_sha(h[..], c[..], hs[0].h[..]);
  out[0] = 0; for i: 0..<33 { out[1 + i] = e.bytes[i]; } for i: 0..<16 { out[34 + i] = c[i]; }
  hs[0].act = 2;
  return code;
}
noise_act_one_read :: (hs: [] NoiseHandshake, m: [] u8) -> int {
  if hs[0].act != 1 || hs[0].initiator { return NOISE_STATE; }
  if m.count != NOISE_ACT_ONE { return NOISE_TRUNCATED; }
  if m[0] != 0 { return NOISE_BAD_VERSION; }
  re := m[1..<34];
  if !se_decode(re).ok { return NOISE_BAD_PUBKEY; }
  for i: 0..<33 { hs[0].re[i] = re[i]; }
  h := hs[0].h; noise_sha(h[..], re, hs[0].h[..]);
  es: [32] u8; code := noise_ecdh(hs[0].s_priv[..], re, es[..]);
  ck := hs[0].ck; noise_hkdf(ck[..], es[..], hs[0].ck[..], hs[0].temp_k[..]); noise_wipe(es[..]); noise_wipe(ck[..]);
  empty: [1] u8;
  if !noise_open(hs[0].temp_k[..], 0, hs[0].h[..], m[34..<50], empty[0..<0]) { return NOISE_BAD_TAG; }
  h = hs[0].h; noise_sha(h[..], m[34..<50], hs[0].h[..]);
  hs[0].act = 2;
  return code;
}
```

## Act two

The responder answers with its own ephemeral key and `ee`, the ECDH of the
two ephemeral keys. From here every key depends on secrets that exist for
this connection only. Someone who later steals both static keys still
cannot decrypt a recorded session: this is forward secrecy.

Act two has the same 50-byte layout as act one.

<!-- source: coax/lib/lightning/noise.coax act2 -->
```jai
// Act two, responder to initiator: <- e, ee. Both ephemeral keys meet, so
// everything after this depends on fresh secrets: forward secrecy.
noise_act_two_write :: (hs: [] NoiseHandshake, e_priv: [] u8, out: [] u8) -> int {
  if hs[0].act != 2 || hs[0].initiator { return NOISE_STATE; }
  if out.count < NOISE_ACT_TWO { return NOISE_CAPACITY; }
  e := noise_public(e_priv);
  if !e.ok { return NOISE_BAD_KEY; }
  for i: 0..<32 { hs[0].e_priv[i] = e_priv[i]; } hs[0].e_pub = e.bytes;
  h := hs[0].h; noise_sha(h[..], e.bytes[..], hs[0].h[..]);
  ee: [32] u8; code := noise_ecdh(e_priv, hs[0].re[..], ee[..]);
  ck := hs[0].ck; noise_hkdf(ck[..], ee[..], hs[0].ck[..], hs[0].temp_k[..]); noise_wipe(ee[..]); noise_wipe(ck[..]);
  empty: [1] u8; c: [16] u8;
  noise_seal(hs[0].temp_k[..], 0, hs[0].h[..], empty[0..<0], c[..]);
  h = hs[0].h; noise_sha(h[..], c[..], hs[0].h[..]);
  out[0] = 0; for i: 0..<33 { out[1 + i] = e.bytes[i]; } for i: 0..<16 { out[34 + i] = c[i]; }
  hs[0].act = 3;
  return code;
}
noise_act_two_read :: (hs: [] NoiseHandshake, m: [] u8) -> int {
  if hs[0].act != 2 || !hs[0].initiator { return NOISE_STATE; }
  if m.count != NOISE_ACT_TWO { return NOISE_TRUNCATED; }
  if m[0] != 0 { return NOISE_BAD_VERSION; }
  re := m[1..<34];
  if !se_decode(re).ok { return NOISE_BAD_PUBKEY; }
  for i: 0..<33 { hs[0].re[i] = re[i]; }
  h := hs[0].h; noise_sha(h[..], re, hs[0].h[..]);
  ee: [32] u8; code := noise_ecdh(hs[0].e_priv[..], re, ee[..]);
  ck := hs[0].ck; noise_hkdf(ck[..], ee[..], hs[0].ck[..], hs[0].temp_k[..]); noise_wipe(ee[..]); noise_wipe(ck[..]);
  empty: [1] u8;
  if !noise_open(hs[0].temp_k[..], 0, hs[0].h[..], m[34..<50], empty[0..<0]) { return NOISE_BAD_TAG; }
  h = hs[0].h; noise_sha(h[..], m[34..<50], hs[0].h[..]);
  hs[0].act = 3;
  return code;
}
```

## The session keys

After act three, both sides run HKDF on the final `ck` with an empty
input. The first 32 bytes are the key from initiator to responder, the
last 32 the key the other way. Each side's sending nonce and receiving
nonce start at 0, and each direction keeps its own copy of `ck` for
rotation.

<!-- source: coax/lib/lightning/noise.coax transport -->
```jai
// The session: a key and nonce each way, and a chaining key each way for
// rotation. Both start from the handshake's final ck.
NoiseTransport :: struct { sk: [32] u8; rk: [32] u8; sn: u64; rn: u64; sck: [32] u8; rck: [32] u8; }
noise_split :: (hs: [] NoiseHandshake, t: [] NoiseTransport) {
  empty: [1] u8; first: [32] u8; second: [32] u8;
  noise_hkdf(hs[0].ck[..], empty[0..<0], first[..], second[..]);
  t[0] = NoiseTransport.{sck = hs[0].ck, rck = hs[0].ck};
  // The initiator sends with the first key; the responder receives with it.
  t[0].sk = ifx hs[0].initiator then first else second;
  t[0].rk = ifx hs[0].initiator then second else first;
  noise_wipe(first[..]); noise_wipe(second[..]);
  // The handshake's secrets are no longer needed.
  hs[0] = NoiseHandshake.{act = 4, initiator = hs[0].initiator, rs = hs[0].rs};
}
```

## Act three

The initiator now reveals its static key, encrypted under the act-two
temporary key with nonce 1, so only the responder learns it. It then
computes `se`, the ECDH of its static secret and the responder's
ephemeral key, and sends a tag under the key that `se` gives. The tag
proves the initiator holds the secret behind the key it just revealed.

Act three is 66 bytes: a version byte, the 49-byte encrypted static key
(33 bytes and a tag), and a 16-byte tag. The responder's reader has two
tags to check, and reports which one failed: `BAD_CIPHERTEXT` for the
encrypted key, `BAD_TAG` for the last.

<!-- source: coax/lib/lightning/noise.coax act3 -->
```jai
// Act three, initiator to responder: -> s, se. The initiator sends its
// static key encrypted under temp_k2 with nonce 1, then proves it holds
// the matching secret with a tag under temp_k3.
noise_act_three_write :: (hs: [] NoiseHandshake, out: [] u8, t: [] NoiseTransport) -> int {
  if hs[0].act != 3 || !hs[0].initiator { return NOISE_STATE; }
  if out.count < NOISE_ACT_THREE { return NOISE_CAPACITY; }
  c: [49] u8;
  noise_seal(hs[0].temp_k[..], 1, hs[0].h[..], hs[0].s_pub[..], c[..]);
  h := hs[0].h; noise_sha(h[..], c[..], hs[0].h[..]);
  se: [32] u8; code := noise_ecdh(hs[0].s_priv[..], hs[0].re[..], se[..]);
  ck := hs[0].ck; noise_hkdf(ck[..], se[..], hs[0].ck[..], hs[0].temp_k[..]); noise_wipe(se[..]); noise_wipe(ck[..]);
  empty: [1] u8; tag: [16] u8;
  noise_seal(hs[0].temp_k[..], 0, hs[0].h[..], empty[0..<0], tag[..]);
  out[0] = 0; for i: 0..<49 { out[1 + i] = c[i]; } for i: 0..<16 { out[50 + i] = tag[i]; }
  noise_split(hs, t);
  return code;
}
noise_act_three_read :: (hs: [] NoiseHandshake, m: [] u8, t: [] NoiseTransport) -> int {
  if hs[0].act != 3 || hs[0].initiator { return NOISE_STATE; }
  if m.count != NOISE_ACT_THREE { return NOISE_TRUNCATED; }
  if m[0] != 0 { return NOISE_BAD_VERSION; }
  rs: [33] u8;
  if !noise_open(hs[0].temp_k[..], 1, hs[0].h[..], m[1..<50], rs[..]) { return NOISE_BAD_CIPHERTEXT; }
  if !se_decode(rs[..]).ok { return NOISE_BAD_PUBKEY; }
  hs[0].rs = rs;
  h := hs[0].h; noise_sha(h[..], m[1..<50], hs[0].h[..]);
  se: [32] u8; code := noise_ecdh(hs[0].e_priv[..], rs[..], se[..]);
  ck := hs[0].ck; noise_hkdf(ck[..], se[..], hs[0].ck[..], hs[0].temp_k[..]); noise_wipe(se[..]); noise_wipe(ck[..]);
  empty: [1] u8;
  if !noise_open(hs[0].temp_k[..], 0, hs[0].h[..], m[50..<66], empty[0..<0]) { return NOISE_BAD_TAG; }
  noise_split(hs, t);
  return code;
}
```

## Messages and rotation

A message of up to 65,535 bytes is sent in two parts. The first is its
length, 2 bytes big-endian, sealed into 18 bytes. The second is the
message, sealed into its own length plus 16. Each seal uses the sending
key with the current nonce and then increments it, so a message uses two
nonces. A "hello" of 5 bytes takes 18 + 5 + 16 = 39 bytes.

The receiver reads 18 bytes, opens the length, then reads that many bytes
plus 16 and opens the body. A tag that fails on either part means the
stream is corrupt or forged, and the receiver closes the connection.

<!-- source: coax/lib/lightning/noise.coax messages -->
```jai
// Encrypt one message: its 2-byte big-endian length sealed with sn, then
// the message sealed with sn + 1. out gets 18 + m.count + 16 bytes;
// returns that length, or -1.
noise_encrypt :: (t: [] NoiseTransport, m: [] u8, out: [] u8) -> int {
  if m.count > NOISE_MAX_MESSAGE || out.count < NOISE_HEADER + m.count + NOISE_TAG { return -1; }
  length := [2] u8.[cast(u8) (m.count >> 8), cast(u8) (m.count & 255)];
  n: [1] u64;
  noise_seal(t[0].sk[..], t[0].sn, length[0..<0], length[..], out[0..<NOISE_HEADER]);
  n[0] = t[0].sn; noise_used(t[0].sk[..], t[0].sck[..], n[..]); t[0].sn = n[0];
  empty: [1] u8;
  noise_seal(t[0].sk[..], t[0].sn, empty[0..<0], m, out[NOISE_HEADER..<NOISE_HEADER + m.count + NOISE_TAG]);
  n[0] = t[0].sn; noise_used(t[0].sk[..], t[0].sck[..], n[..]); t[0].sn = n[0];
  return NOISE_HEADER + m.count + NOISE_TAG;
}
// Read the first 18 bytes: the length of the message that follows, which
// the caller then reads with its 16-byte tag. A bad tag here means the
// stream is corrupt; the connection ends.
NoiseLength :: struct { code: int; n: int; }
noise_decrypt_length :: (t: [] NoiseTransport, header: [] u8) -> NoiseLength {
  if header.count != NOISE_HEADER { return NoiseLength.{code = NOISE_TRUNCATED}; }
  length: [2] u8; empty: [1] u8; n: [1] u64;
  ok := noise_open(t[0].rk[..], t[0].rn, empty[0..<0], header, length[..]);
  n[0] = t[0].rn; noise_used(t[0].rk[..], t[0].rck[..], n[..]); t[0].rn = n[0];
  if !ok { return NoiseLength.{code = NOISE_BAD_TAG}; }
  return NoiseLength.{n = (cast(int) length[0] << 8) | cast(int) length[1]};
}
// Then the body and its tag, decrypted into out.
noise_decrypt_body :: (t: [] NoiseTransport, body: [] u8, out: [] u8) -> int {
  if body.count < NOISE_TAG { return NOISE_TRUNCATED; }
  if out.count < body.count - NOISE_TAG { return NOISE_CAPACITY; }
  empty: [1] u8; n: [1] u64;
  ok := noise_open(t[0].rk[..], t[0].rn, empty[0..<0], body, out);
  n[0] = t[0].rn; noise_used(t[0].rk[..], t[0].rck[..], n[..]); t[0].rn = n[0];
  return ifx ok then NOISE_OK else NOISE_BAD_TAG;
}
```

A key rotates after 1,000 uses. The new chaining key and key are
HKDF(`ck`, `k`), and the nonce starts again at 0. Two uses per message
make that every 500 messages. A key that has encrypted a lot of traffic is
retired, and a key stolen later does not decrypt earlier traffic.

<!-- source: coax/lib/lightning/noise.coax rotate -->
```jai
// A key rotates after 1,000 uses: ck', k' = HKDF(ck, k), and its nonce
// starts again at 0. Each message uses a key twice, length and body, so
// keys rotate every 500 messages.
noise_used :: (key: [] u8, ck: [] u8, n: [] u64) {
  n[0] += 1;
  if n[0] == NOISE_ROTATE {
    old_k: [32] u8; old_ck: [32] u8;
    for i: 0..<32 { old_k[i] = key[i]; old_ck[i] = ck[i]; }
    noise_hkdf(old_ck[..], old_k[..], ck, key);
    noise_wipe(old_k[..]); noise_wipe(old_ck[..]);
    n[0] = 0;
  }
}
```

## The example

The example,
[bitcoin/examples/lightning-noise.coax](../examples/lightning-noise.coax),
runs every handshake vector in BOLT 8 from the side it names, shows the
initiator's side of the successful handshake step by step, and runs the
message vectors across two rotations. Then two Coax nodes complete a
handshake and send chapter 104's `init` over the transport.

<!-- source: bitcoin/examples/lightning-noise.coax example -->
```jai
#load "../../coax/lib/lightning/noise.coax";
#load "../../coax/lib/lightning/bolt1.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex, at most 256 of them.
show :: (b: [] u8) {
  text: [512] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
// A 32-byte secret key whose last byte is k.
small :: (k: u8) -> [32] u8 { key: [32] u8; key[31] = k; return key; }

main :: () {
  // ---- BOLT 8's handshake vectors, each from the side it names
  print("handshake vectors\n");
  for row: NOISE_HANDSHAKES {
    t: [1] NoiseTransport;
    code := noise_run_vector(row, t[..]);
    keys := false;
    if code == NOISE_OK {
      both: [64] u8; first := ifx row.initiator then t[0].sk else t[0].rk; second := ifx row.initiator then t[0].rk else t[0].sk;
      for i: 0..<32 { both[i] = first[i]; both[32 + i] = second[i]; }
      keys = nz_hex_is(both[..], row.keys);
    }
    print("  %: %", row.name, NOISE_CODE_NAMES[code]);
    if code == NOISE_OK { print(", session keys match: %", keys); }
    else { print(", BOLT 8 says %", row.error); }
    print("\n");
  }

  // ---- the initiator's side of the successful vector, step by step
  print("the initiator, step by step\n");
  row := NOISE_HANDSHAKES[0];
  hs: [1] NoiseHandshake; key: [32] u8; e: [32] u8; peer: [33] u8; m: [66] u8; out: [66] u8; t: [1] NoiseTransport;
  nz_fill(row.ls_priv, key[..]); nz_fill(row.e_priv, e[..]); nz_fill(row.rs_pub, peer[..]);
  code := noise_handshake_init(hs[..], true, key[..], peer[..]);
  print("  h before act one: "); show(hs[0].h[..]); print("\n");
  code = noise_act_one_write(hs[..], e[..], out[..]);
  print("  act one, 50 bytes: "); show(out[0..<50]); print("\n");
  print("  h after act one: "); show(hs[0].h[..]); print("\n");
  code = noise_act_two_read(hs[..], nz_unhex(row.act2, m[..]));
  print("  act two read: %, ck: ", NOISE_CODE_NAMES[code]); show(hs[0].ck[..]); print("\n");
  code = noise_act_three_write(hs[..], out[..], t[..]);
  print("  act three, 66 bytes: "); show(out[0..<66]); print("\n");
  print("  sk: "); show(t[0].sk[..]); print("\n");
  print("  rk: "); show(t[0].rk[..]); print("\n");

  // ---- BOLT 8's message vectors: 1,002 "hello"s, two rotations
  print("message vectors\n");
  tm: [1] NoiseTransport; k: [32] u8;
  nz_fill(NOISE_MESSAGES.sk, k[..]); tm[0].sk = k; nz_fill(NOISE_MESSAGES.ck, k[..]); tm[0].sck = k;
  hello := [5] u8.['h', 'e', 'l', 'l', 'o']; box: [128] u8;
  for i: 0..<1002 {
    n := noise_encrypt(tm[..], hello[..], box[..]);
    for o: NOISE_MESSAGES.outputs {
      if o.index == i { print("  message %, % bytes: %, nonce now %\n", i, n, nz_hex_is(box[0..<n], o.hex), tm[0].sn); }
    }
  }

  // ---- two Coax nodes: a handshake, then BOLT 1 messages
  print("two nodes\n");
  ka := small(7); kb := small(9); ea := small(11); eb := small(13);
  a: [1] NoiseHandshake; b: [1] NoiseHandshake; ta: [1] NoiseTransport; tb: [1] NoiseTransport;
  apub := noise_public(ka[..]); bpub := noise_public(kb[..]);
  print("  responder's static key: "); show(bpub.bytes[..]); print("\n");
  one: [50] u8; two: [50] u8; three: [66] u8;
  code = noise_handshake_init(a[..], true, ka[..], bpub.bytes[..]);
  code = noise_handshake_init(b[..], false, kb[..], bpub.bytes[..]);
  code = noise_act_one_write(a[..], ea[..], one[..]);
  r1 := noise_act_one_read(b[..], one[..]);
  code = noise_act_two_write(b[..], eb[..], two[..]);
  r2 := noise_act_two_read(a[..], two[..]);
  code = noise_act_three_write(a[..], three[..], ta[..]);
  r3 := noise_act_three_read(b[..], three[..], tb[..]);
  print("  acts read: %, %, %\n", NOISE_CODE_NAMES[r1], NOISE_CODE_NAMES[r2], NOISE_CODE_NAMES[r3]);
  same := true; for i: 0..<33 { if b[0].rs[i] != apub.bytes[i] { same = false; } }
  print("  the responder learned the initiator's static key: %\n", same);
  print("  initiator sk = responder rk: %\n", cb_ct_equal(ta[0].sk[..], tb[0].rk[..]) == CB_MAX64);
  // A third node that does not hold the responder's key cannot finish act one.
  c: [1] NoiseHandshake; kc := small(5); cpub := noise_public(kc[..]);
  code = noise_handshake_init(c[..], false, kc[..], cpub.bytes[..]);
  print("  act one read by a node with another key: %\n", NOISE_CODE_NAMES[noise_act_one_read(c[..], one[..])]);
  // init (chapter 104) over the transport.
  f: [3] u8; chains: [1] u8; msg: [64] u8; got: [64] u8; wire: [128] u8;
  put := ln_feature_put(f[..], 8); put = ln_feature_put(f[..], 14);
  n := ln_init_write_vartime(f[1..<3], chains[0..<0], msg[..]);
  w := noise_encrypt(ta[..], msg[0..<n], wire[..]);
  print("  init: % bytes, % on the wire: ", n, w); show(wire[0..<w]); print("\n");
  l := noise_decrypt_length(tb[..], wire[0..<NOISE_HEADER]);
  body := noise_decrypt_body(tb[..], wire[NOISE_HEADER..<w], got[..]);
  r := ln_init_read_vartime(got[0..<l.n]);
  print("  the responder reads length %, body %, init %, features %\n", l.n, NOISE_CODE_NAMES[body], CODES[r.code], r.features_n);
  // The next message with one body byte changed, read by a copy of the
  // responder's state: the length opens, the body does not.
  again: [128] u8;
  w2 := noise_encrypt(ta[..], msg[0..<n], again[..]); again[NOISE_HEADER + 3] ^= 1;
  probe: [1] NoiseTransport; probe[0] = tb[0];
  l = noise_decrypt_length(probe[..], again[0..<NOISE_HEADER]);
  print("  a changed body byte: length %, body %\n", NOISE_CODE_NAMES[l.code], NOISE_CODE_NAMES[noise_decrypt_body(probe[..], again[NOISE_HEADER..<w2], got[..])]);
  // The first message again: the responder's nonce has moved on.
  print("  the first message replayed: %\n", NOISE_CODE_NAMES[noise_decrypt_length(tb[..], wire[0..<NOISE_HEADER]).code]);
}
CODES :: [9] string.["ok", "truncated", "not minimal", "order", "unknown even", "length", "value", "capacity", "too large"];
```

From the repository root:

```shell
coax/bin/coax run bitcoin/examples/lightning-noise.coax
```

The output:

```text
handshake vectors
  transport-initiator successful handshake: OK, session keys match: true
  transport-initiator act2 short read test: READ_FAILED, BOLT 8 says READ_FAILED
  transport-initiator act2 bad version test: BAD_VERSION, BOLT 8 says BAD_VERSION
  transport-initiator act2 bad key serialization test: BAD_PUBKEY, BOLT 8 says BAD_PUBKEY
  transport-initiator act2 bad MAC test: BAD_TAG, BOLT 8 says BAD_TAG
  transport-responder successful handshake: OK, session keys match: true
  transport-responder act1 short read test: READ_FAILED, BOLT 8 says READ_FAILED
  transport-responder act1 bad version test: BAD_VERSION, BOLT 8 says BAD_VERSION
  transport-responder act1 bad key serialization test: BAD_PUBKEY, BOLT 8 says BAD_PUBKEY
  transport-responder act1 bad MAC test: BAD_TAG, BOLT 8 says BAD_TAG
  transport-responder act3 bad version test: BAD_VERSION, BOLT 8 says BAD_VERSION
  transport-responder act3 short read test: READ_FAILED, BOLT 8 says READ_FAILED
  transport-responder act3 bad MAC for ciphertext test: BAD_CIPHERTEXT, BOLT 8 says BAD_CIPHERTEXT
  transport-responder act3 bad rs test: BAD_PUBKEY, BOLT 8 says BAD_PUBKEY
  transport-responder act3 bad MAC test: BAD_TAG, BOLT 8 says BAD_TAG
the initiator, step by step
  h before act one: 8401b3fdcaaa710b5405400536a3d5fd7792fe8e7fe29cd8b687216fe323ecbd
  act one, 50 bytes: 00036360e856310ce5d294e8be33fc807077dc56ac80d95d9cd4ddbd21325eff73f70df6086551151f58b8afe6c195782c6a
  h after act one: 9d1ffbb639e7e20021d9259491dc7b160aab270fb1339ef135053f6f2cebe9ce
  act two read: OK, ck: e89d31033a1b6bf68c07d22e08ea4d7884646c4b60a9528598ccb4ee2c8f56ba
  act three, 66 bytes: 00b9e3a702e93e3a9948c2ed6e5fd7590a6e1c3a0344cfc9d5b57357049aa22355361aa02e55a8fc28fef5bd6d71ad0c38228dc68b1c466263b47fdf31e560e139ba
  sk: 969ab31b4d288cedf6218839b27a3e2140827047f2c0f01bf5c04435d43511a9
  rk: bb9020b8965f4df047e07f955f3c4b88418984aadc5cdb35096b9ea8fa5c3442
message vectors
  message 0, 39 bytes: true, nonce now 2
  message 1, 39 bytes: true, nonce now 4
  message 500, 39 bytes: true, nonce now 2
  message 501, 39 bytes: true, nonce now 4
  message 1000, 39 bytes: true, nonce now 2
  message 1001, 39 bytes: true, nonce now 4
two nodes
  responder's static key: 03acd484e2f0c7f65309ad178a9f559abde09796974c57e714c35f110dfc27ccbe
  acts read: OK, OK, OK
  the responder learned the initiator's static key: true
  initiator sk = responder rk: true
  act one read by a node with another key: BAD_TAG
  init: 8 bytes, 42 on the wire: 9e58a5dc39563f339e23bb1c8160883350718616e3116b05b592d98f8f056bb8c2374dd73f3a8e4a8a28
  the responder reads length 8, body OK, init ok, features 2
  a changed body byte: length OK, body BAD_TAG
  the first message replayed: BAD_TAG
```

What the output shows:

- All 15 handshake vectors give BOLT 8's result: the 2 successful
  handshakes give its session keys, and the 13 failures give the error it
  names.
- The initiator's `h`, act one, `ck` after act two, act three and session
  keys are BOLT 8's, byte for byte.
- The message vectors' "hello"s at 0, 1, 500, 501, 1,000 and 1,001 are
  BOLT 8's 39 bytes each. The nonce is 2 and 4 after each pair because the
  key rotated at messages 500 and 1,000.
- Two nodes with static keys 7 and 9 complete all three acts. The
  responder learns the initiator's static key, and their keys pair up.
- A node with static key 5 cannot read act one meant for key 9: its `es`
  differs, so the tag fails.
- Our 8-byte `init` takes 42 bytes on the wire, and the responder reads it
  back with its two feature bits.
- A message with one body byte changed opens its length but not its body.
  The first message sent again fails, because the receiver's nonce has
  moved on.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/lightning-noise.out](../fixtures/lightning-noise.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/lightning-noise.coax](../examples/checks/lightning-noise.coax),
which loads the example:

- each of the 312 single-bit changes to a 39-byte message is refused, in
  the length or in the body
- 510 messages of different lengths, starting 10 nonces before a
  rotation, cross two rotations and all open, and both sides end with the
  same key and chaining key
- a changed byte in each part of act one, the version, the key's prefix,
  its x-coordinate and the tag, fails the responder's read

<!-- source: bitcoin/examples/checks/lightning-noise.coax check -->
```jai
// Two transports in step from BOLT 8's message keys: no handshake, so
// each check starts cheaply.
pair :: (a: [] NoiseTransport, b: [] NoiseTransport) {
  k: [32] u8; ck: [32] u8; nz_fill(NOISE_MESSAGES.sk, k[..]); nz_fill(NOISE_MESSAGES.ck, ck[..]);
  a[0] = NoiseTransport.{sk = k, sck = ck}; b[0] = NoiseTransport.{rk = k, rck = ck};
}
// Every one of the 312 single-bit changes to a 39-byte "hello" is refused,
// in the length or in the body.
#forall "one changed bit of a message" bit: 0..<312 #steps 40000000 {
  a: [1] NoiseTransport; b: [1] NoiseTransport; pair(a[..], b[..]);
  hello := [5] u8.['h', 'e', 'l', 'l', 'o']; box: [64] u8; got: [8] u8;
  n := noise_encrypt(a[..], hello[..], box[..]);
  box[bit / 8] ^= cast(u8) (1 << (bit % 8));
  l := noise_decrypt_length(b[..], box[0..<NOISE_HEADER]);
  #assert l.code != NOISE_OK || noise_decrypt_body(b[..], box[NOISE_HEADER..<n], got[..]) != NOISE_OK;
}
// 510 messages of different lengths, starting 10 uses before a rotation,
// cross two rotations and all open, and both sides rotate in step.
#test "messages open across two rotations" #steps 2000000000 {
  a: [1] NoiseTransport; b: [1] NoiseTransport; pair(a[..], b[..]);
  a[0].sn = 990; b[0].rn = 990;
  m: [40] u8; box: [80] u8; got: [40] u8;
  for i: 0..<510 {
    size := i % 40; for j: 0..<size { m[j] = cast(u8) ((i + j) & 255); }
    n := noise_encrypt(a[..], m[0..<size], box[..]);
    l := noise_decrypt_length(b[..], box[0..<NOISE_HEADER]);
    #assert l.code == NOISE_OK && l.n == size;
    #assert noise_decrypt_body(b[..], box[NOISE_HEADER..<n], got[..]) == NOISE_OK;
    for j: 0..<size { #assert got[j] == m[j]; }
  }
  // 5 messages to the first rotation, 500 to the second, 5 more.
  #assert a[0].sn == 10 && b[0].rn == 10;
  for i: 0..<32 { #assert a[0].sk[i] == b[0].rk[i] && a[0].sck[i] == b[0].rck[i]; }
}
// A changed byte in each part of act one fails the responder's read: the
// version, the key's prefix, its x-coordinate, and the tag.
ACT_ONE_BYTES :: [8] int.[0, 1, 2, 17, 33, 34, 41, 49];
#forall "one changed byte of act one" k: 0..<8 #steps 2000000000 {
  at := ACT_ONE_BYTES[k];
  row := NOISE_HANDSHAKES[5]; hs: [1] NoiseHandshake; key: [32] u8; m: [50] u8;
  nz_fill(row.ls_priv, key[..]); mine := noise_public(key[..]);
  #assert noise_handshake_init(hs[..], false, key[..], mine.bytes[..]) == NOISE_OK;
  nz_fill(row.act1, m[..]); m[at] ^= 0x10;
  #assert noise_act_one_read(hs[..], m[..]) != NOISE_OK;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/lightning-noise.coax
```

`coax test` runs 187 checks, including the loaded files' own
checks, in about 110 seconds on 2026-11-03.

### The library's own checks

`noise.coax` is an `#authority` file. Its checks cover every BOLT 8
handshake vector from each side, the message vectors with both
rotations, the starting `h` and `ck`, refusals for bad secret and public
keys, acts out of order and short buffers, and two sides that complete a
handshake, exchange messages and refuse a changed byte.

```shell
coax/bin/coax test coax/lib/lightning/noise.coax
```

`coax test` runs 171 checks, including those of the files it loads, in
about 75 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### BOLT 8 and an independent handshake

[noise.py](../../coax/tests/lightning/noise.py) computes every line the
example prints with its own Python handshake and transport, written from
BOLT 8's "Handshake Exchange" and "Lightning Message Specification". Its
secp256k1, ChaCha20-Poly1305 and HKDF are in
[ln_crypto.py](../../coax/tests/lightning/ln_crypto.py), and share no
code with Coax. Every handshake vector must give BOLT 8's outputs, keys
or error, and the message vector its six outputs, before the transcript
is compared with the fixture.

```shell
python3 -I coax/tests/lightning/noise.py --coax coax/bin/coax
```

On 2026-11-03 all 41 lines agree, and the example's output in native debug
and release builds equals them. Wasm was not run.

[make_noise_vectors.py](../../coax/tests/lightning/make_noise_vectors.py)
reads the pinned BOLT 8 into
[noise_vectors.coax](../../coax/tests/lightning/noise_vectors.coax).

## What this chapter leaves out

- Sockets, timeouts and reading exactly the bytes each step needs: the
  node code that calls this library, in chapter 109.
- Making ephemeral keys. The library takes them from the caller, which
  draws them from chapter 18's randomness; the vectors fix them.
- Finding the responder's static key, from a node announcement (chapter
  112) or a node URI.
- BIP-324, Bitcoin's own peer-to-peer encryption, which is chapter 67.

## Summary

- BOLT 8 is Noise XK: the initiator knows the responder's static key and
  reveals its own only in act three, encrypted.
- `h` covers every byte of the handshake, and `ck` absorbs every shared
  secret through HKDF.
- ECDH is SHA-256 of the compressed shared point.
- The acts are 50, 50 and 66 bytes, and a reader checks length, version,
  key and tag in that order.
- A message is an 18-byte sealed length, then the sealed body, and uses two
  nonces.
- Each key rotates after 1,000 uses, every 500 messages.

## Dependencies

- [9. HKDF](ch009-hkdf.md)
- [12. Poly1305 and ChaCha20-Poly1305](ch012-poly1305-and-chacha20-poly1305.md)
- [16. Scalar multiplication](ch016-scalar-multiplication.md)

## Sources

- [BOLT 8](https://github.com/lightning/bolts/blob/master/08-transport.md),
  "Authenticated Key Agreement Handshake", "Handshake Exchange",
  "Lightning Message Specification", "Lightning Message Key Rotation" and
  "Appendix A: Transport Test Vectors", at lightning/bolts revision
  `19146282`, pinned in
  [coax/tests/vectors/upstream/bolts/](../../coax/tests/vectors/upstream/bolts/08-transport.md).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
