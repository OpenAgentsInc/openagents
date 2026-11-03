# 111. Onion routing and blinded paths

A Lightning payment crosses several channels, and no node on the way
should learn more than it needs: the channel it receives on, the channel
it forwards on, and the amount and timeout for that channel. BOLT 4 does
this with Sphinx onions. The payer wraps one payload for each hop in
layers of encryption, and each hop removes one layer with its own key.

This chapter builds BOLT 4's onion: the shared secrets and blinding
factors, the keys made from them, the ChaCha20 stream, the filler, the
HMAC chain and the 1,300 bytes of hop payloads. It builds error onions,
which carry a failure back to the payer, and blinded paths, which let a
recipient hide its own node behind keys the payer cannot link to it.

## What this chapter teaches

- The shared secret with each hop, and the blinding factor that gives
  each hop a different ephemeral key.
- The `rho`, `mu`, `um`, `pad` and `ammag` keys: an HMAC of the shared
  secret with the key's name as the HMAC key.
- The pseudo-random stream: ChaCha20 with a zero nonce.
- The packet: a version byte, an ephemeral key, 1,300 bytes of hop
  payloads and an HMAC. Each hop payload is a TLV stream with its length
  in front.
- The filler, which lets the payer compute the last hop's HMAC over the
  bytes the earlier hops' streams leave behind.
- Peeling one layer: check the HMAC, decrypt, read the payload, shift.
- Error onions: the erring node's HMAC under `um`, each hop's `ammag`
  layer, and how the payer finds the hop that failed.
- Blinded paths: blinded node IDs, `encrypted_recipient_data`, path keys,
  and joining two paths with `next_path_key_override`.

## Where Lightning uses onions

| Where | What it uses | Chapter |
| --- | --- | --- |
| `update_add_htlc` | the 1,366-byte onion, with the payment hash as associated data | 109, 113 |
| `update_fail_htlc` | the error onion each hop adds a layer to | 109, 113 |
| Paying an invoice or offer with a blinded path | blinded node IDs, `encrypted_recipient_data`, the first path key | 113, 115 |
| `onion_message` | an onion along a blinded path, with no payment | 115 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [onion.coax](../../coax/lib/lightning/onion.coax) | `onion_key(kind, secret, out)` | `rho`, `mu`, `um`, `pad`, `ammag` and `blinded_node_id` |
| | `onion_xor(key, data)` | the ChaCha20 stream, XORed in place |
| | `onion_route(session, pubkeys, route)` | each hop's ephemeral key and shared secret |
| | `onion_add_hop_vartime(hops, pubkey, payload)` | a hop and its TLV payload |
| | `onion_wrap(session, pubkeys, payloads, ad, out)` | builds the onion |
| | `onion_peel(secret, onion, ad, payload, next)` | one hop's layer |
| | `onion_error_create`, `onion_error_forward`, `onion_error_open` | error onions |
| | `onion_blind_hop(e, node_id, data, box)` | one hop of a blinded path |
| | `onion_blinded_secret`, `onion_unblind` | a hop's side of a blinded path |
| [bolt1.coax](../../coax/lib/lightning/bolt1.coax) | `bigsize_read_vartime`, `tlv_next_vartime` | BigSize and TLV records (chapter 104) |
| [chacha20.coax](../../coax/lib/crypto/chacha20.coax) | `chacha20_xor_checked` | the stream (chapter 11) |
| [hmac.coax](../../coax/lib/crypto/hmac.coax) | `hmac_sha256_checked` | keys and HMACs (chapter 8) |
| [secp_mult_const.coax](../../coax/lib/crypto/secp_mult_const.coax) | `smc_multiply` | constant-time scalar multiplication (chapter 16) |

Session keys, ephemeral keys, node secret keys, shared secrets and the
keys made from them are secret. Scalar multiplication uses chapter 16's
constant-time ladder, scalar products are constant time, HMACs are
compared in constant time, and each procedure clears the secrets it leaves
on the stack. Public keys and lengths are public, and a hop's decrypted
payload belongs to that hop. The procedures that branch only on those
end in `_vartime` (chapter 3).

The library's codes and sizes:

<!-- source: coax/lib/lightning/onion.coax codes -->
```jai
ONION_OK :: 0;
ONION_BAD_POINT :: 1;     // a public key that is not a compressed point
ONION_BAD_KEY :: 2;       // a secret key of zero or at least n
ONION_BAD_VERSION :: 3;   // a packet version other than 0: invalid_onion_version
ONION_BAD_HMAC :: 4;      // a packet whose HMAC does not check: invalid_onion_hmac
ONION_BAD_PAYLOAD :: 5;   // a hop payload that is empty or does not fit
ONION_TOO_LONG :: 6;      // no hops, more than 27, or payloads above 1,300 bytes
ONION_BAD_LENGTH :: 7;    // a packet or error of the wrong size
ONION_CAPACITY :: 8;      // the caller's buffer is too small
ONION_BAD_TAG :: 9;       // encrypted_recipient_data that does not decrypt
ONION_UNKNOWN :: 10;      // an error no hop of the route made

ONION_PAYLOADS :: 1300;   // hop_payloads
ONION_PACKET :: 1366;     // version 1, public key 33, hop_payloads 1,300, HMAC 32
ONION_HMAC :: 32;
ONION_MAX_HOPS :: 27;     // 1,300 bytes hold at most 27 of the smallest payloads
ONION_ERROR_PAD :: 256;   // failure_len + pad_len, at least
ONION_MAX_ERROR :: 32768;
ONION_TAG :: 16;
```

## Keys and the stream

Every key comes from a shared secret. The key type's name, `rho`, `mu`,
`um`, `pad`, `ammag` or `blinded_node_id`, is the HMAC key, with no
terminating zero, and the 32-byte secret is the message. The stream is
ChaCha20 under one of these keys with a zero nonce, starting at block 0.
A fixed nonce is safe because no key is used twice.

The shared secret is SHA-256 of the compressed point k·P, as in BOLT 8.

<!-- source: coax/lib/lightning/onion.coax primitives -->
```jai
// Keys from a shared secret: HMAC-SHA256 with the key type's name as the
// HMAC key, "rho", "mu", "um", "pad", "ammag" or "blinded_node_id", and
// the secret as the message.
onion_key :: (kind: string, secret: [] u8, out: [] u8) {
  name: [16] u8; for i: 0..<kind.count { name[i] = kind[i]; }
  #assert hmac_sha256_checked(name[0..<kind.count], secret, out, false) == SHA_OK;
}
// The pseudo-random stream: ChaCha20 under the key with a zero nonce,
// from block 0. XOR it onto data, in place; onto zeros it is the stream.
onion_xor :: (key: [] u8, data: [] u8) {
  nonce: [12] u8; chunk: [512] u8; out: [512] u8; at := 0;
  while at < data.count {
    n := ifx data.count - at < 512 then data.count - at else 512;
    onion_copy(chunk[..], data[at..<at + n]);
    r := chacha20_xor_checked(key, nonce[..], cast(u32) (at / 64), chunk[0..<n], out[0..<n], false);
    #assert r.error == CC_OK;
    onion_copy(data[at..<at + n], out[0..<n]);
    at += n;
  }
  onion_wipe(chunk[..]); onion_wipe(out[..]);
}
// HMAC-SHA256 of a || b.
onion_mac :: (key: [] u8, a: [] u8, b: [] u8, out: [] u8) {
  state: [224] u8;
  #assert hmac_sha256_init_checked(state[..], key, false) == SHA_OK;
  #assert hmac_sha256_update_checked(state[..], a, false) == SHA_OK;
  #assert hmac_sha256_update_checked(state[..], b, false) == SHA_OK;
  #assert hmac_sha256_final_checked(state[..], out, false) == SHA_OK;
  onion_wipe(state[..]);
}
// The shared secret: SHA-256 of the compressed point k * P, as in BOLT 8.
onion_ecdh :: (k: ScScalar, point: [] u8, out: [] u8) -> int {
  if point.count != 33 { return ONION_BAD_POINT; }
  p := se_decode(point);
  if !p.ok { return ONION_BAD_POINT; }
  shared := onion_encode(smc_multiply(k, p.value));
  #assert sha256_hash_checked(shared.bytes[..], out, false) == SHA_OK;
  onion_wipe(shared.bytes[..]);
  return shared.code;
}
// A 32-byte secret as a scalar, refusing zero and n or more.
OnionSecret :: struct { code: int; value: ScScalar; }
onion_secret :: (secret: [] u8) -> OnionSecret {
  if secret.count != 32 { return OnionSecret.{code = ONION_BAD_KEY}; }
  k := sc_load(secret);
  if !k.ok || sc_is_zero(k.value) { k = ScLoad.{}; return OnionSecret.{code = ONION_BAD_KEY}; }
  r := OnionSecret.{value = k.value}; k = ScLoad.{};
  return r;
}
// A point times the 32-byte hash h, in constant time: the blinding steps.
onion_tweak :: (h: [] u8, point: [] u8) -> OnionPoint {
  t := onion_secret(h); p := se_decode(point);
  if t.code != ONION_OK { return OnionPoint.{code = t.code}; }
  if !p.ok { return OnionPoint.{code = ONION_BAD_POINT}; }
  r := onion_encode(smc_multiply(t.value, p.value));
  t = OnionSecret.{};
  return r;
}
// A secret's public key.
onion_public :: (secret: [] u8) -> OnionPoint {
  k := onion_secret(secret);
  if k.code != ONION_OK { return OnionPoint.{code = k.code}; }
  r := onion_encode(smc_multiply(k.value, sg_generator())); k = OnionSecret.{};
  return r;
}
```

## Shared secrets along the route

The payer picks a session key and walks the route. Hop i sees the
ephemeral public key e_i·G. Its shared secret is ECDH of e_i and its node
key. The next ephemeral secret is e_i times the blinding factor
SHA256(e_i·G || ss_i). Each hop sees a different key, and only the payer
knows how they relate. A hop computes the same secret from its node
secret and the key it sees, and the next key as that key times the same
factor.

<!-- source: coax/lib/lightning/onion.coax route -->
```jai
// The payer's view of a route. The first hop sees the session key's
// public key. Each next ephemeral key is the last times the blinding
// factor SHA256(epk || ss), so every hop sees a different key, and each
// hop's shared secret is ECDH of its ephemeral key and the hop's node key.
OnionRoute :: struct { hops: int; epk: [891] u8; ss: [864] u8; }
onion_route :: (session: [] u8, pubkeys: [] u8, route: [] OnionRoute) -> int {
  n := pubkeys.count / 33;
  if pubkeys.count % 33 != 0 || n == 0 || n > ONION_MAX_HOPS { return ONION_TOO_LONG; }
  e := onion_secret(session);
  if e.code != ONION_OK { return e.code; }
  route[0] = OnionRoute.{hops = n};
  code := ONION_OK;
  for i: 0..<n {
    epk := onion_encode(smc_multiply(e.value, sg_generator()));
    onion_copy(route[0].epk[33 * i..<33 * i + 33], epk.bytes[..]);
    c := onion_ecdh(e.value, pubkeys[33 * i..<33 * i + 33], route[0].ss[32 * i..<32 * i + 32]);
    if c != ONION_OK { code = c; break; }
    b: [32] u8; onion_sha(epk.bytes[..], route[0].ss[32 * i..<32 * i + 32], b[..]);
    f := onion_secret(b[..]);
    if f.code != ONION_OK { code = f.code; break; }
    e.value = sc_mul(e.value, f.value);
    onion_wipe(b[..]); f = OnionSecret.{};
  }
  e = OnionSecret.{};
  return code;
}
```

## Hop payloads

Each hop's payload is a TLV stream with a BigSize length in front. A
forwarding hop's stream usually holds `amt_to_forward` (2),
`outgoing_cltv_value` (4) and `short_channel_id` (6). The payer adds the
hops in route order.

<!-- source: coax/lib/lightning/onion.coax hops -->
```jai
// A route as the payer builds it: each hop's node key, and its payload, a
// TLV stream, with the stream's BigSize length in front.
OnionHops :: struct { pubkeys: [891] u8; keys_n: int; payloads: [1300] u8; payloads_n: int; }
onion_add_hop_vartime :: (h: [] OnionHops, pubkey: [] u8, payload: [] u8) -> int {
  if pubkey.count != 33 || h[0].keys_n + 33 > 891 { return ONION_TOO_LONG; }
  at := bigsize_write_vartime(h[0].payloads[..], h[0].payloads_n, cast(u64) payload.count);
  if at < 0 || payload.count > ONION_PAYLOADS - at { return ONION_TOO_LONG; }
  onion_copy(h[0].pubkeys[h[0].keys_n..<h[0].keys_n + 33], pubkey); h[0].keys_n += 33;
  onion_copy(h[0].payloads[at..<at + payload.count], payload); h[0].payloads_n = at + payload.count;
  return ONION_OK;
}
```

## The filler

Each hop shifts the 1,300 bytes left by its payload and HMAC, and fills
the gap at the end with its stream. By the last hop, the end of the
payloads holds bytes made by every earlier hop's stream. The payer needs
those bytes to compute the last hop's HMAC, so it computes them first:
the filler.

<!-- source: coax/lib/lightning/onion.coax filler -->
```jai
// The filler. Each hop shifts the payloads left by its own payload and
// HMAC and appends that many stream bytes, so the last hop's payloads end
// in bytes every earlier hop's stream made. The payer computes them, so
// the last hop's HMAC covers them too.
onion_filler :: (route: [] OnionRoute, size: [] int, filler: [] u8) -> int {
  s: [2600] u8; rho: [32] u8; length := 0;
  for i: 0..<route[0].hops - 1 {
    for j: length..<length + size[i] { filler[j] = 0; }
    length += size[i];
    onion_key("rho", route[0].ss[32 * i..<32 * i + 32], rho[..]);
    total := ONION_PAYLOADS + size[i];
    onion_wipe(s[0..<total]); onion_xor(rho[..], s[0..<total]);
    for j: 0..<length { filler[j] ^= s[total - length + j]; }
  }
  onion_wipe(s[..]); onion_wipe(rho[..]);
  return length;
}
```

## Wrapping the onion

Wrapping starts at the last hop. The payloads start as the `pad` key's
stream, so the unused end looks random. For each hop, from last to first:
put its payload and the next hop's HMAC in front, drop the same number of
bytes from the end, XOR all 1,300 bytes with the hop's `rho` stream, and
compute this hop's HMAC under `mu` over the payloads and the associated
data. For a payment, the associated data is the payment hash, so the
onion only works with its HTLC. The last hop gets the filler and an
all-zero next HMAC.

<!-- source: coax/lib/lightning/onion.coax wrap -->
```jai
// Build an onion. pubkeys holds the hops' node keys, 33 bytes each;
// payloads holds each hop's payload with its BigSize length in front, one
// after another. The associated data, the payment hash for a payment, is
// covered by every HMAC. Wrapping starts at the last hop: each layer puts
// a payload and the HMAC for the next hop in front, encrypts all 1,300
// bytes under rho, and computes this hop's HMAC under mu.
onion_wrap :: (session: [] u8, pubkeys: [] u8, payloads: [] u8, ad: [] u8, out: [] u8) -> int {
  if out.count < ONION_PACKET { return ONION_CAPACITY; }
  route: [1] OnionRoute;
  code := onion_route(session, pubkeys, route[..]);
  if code != ONION_OK { return code; }
  n := route[0].hops;
  start: [27] int; size: [27] int; at := 0; total := 0;
  for i: 0..<n {
    l := bigsize_read_vartime(payloads, at);
    if l.code != LN_OK || l.value == 0 || l.value > cast(u64) ONION_PAYLOADS { return ONION_BAD_PAYLOAD; }
    length := l.next - at + cast(int) l.value;
    if length > payloads.count - at { return ONION_BAD_PAYLOAD; }
    start[i] = at; size[i] = length + ONION_HMAC; total += size[i]; at += length;
  }
  if at != payloads.count { return ONION_BAD_PAYLOAD; }
  if total > ONION_PAYLOADS { return ONION_TOO_LONG; }
  filler: [1300] u8;
  filled := onion_filler(route[..], size[..], filler[..]);
  // The starting bytes come from the pad key, so nothing about the route
  // shows in the unused end of the payloads.
  hp: [1300] u8; key: [32] u8;
  onion_key("pad", session, key[..]); onion_xor(key[..], hp[..]);
  mac: [32] u8; layer: [1300] u8;
  for k: 0..<n {
    i := n - 1 - k; ss := route[0].ss[32 * i..<32 * i + 32];
    length := size[i] - ONION_HMAC;
    onion_copy(layer[0..<length], payloads[start[i]..<start[i] + length]);
    onion_copy(layer[length..<size[i]], mac[..]);
    onion_copy(layer[size[i]..<ONION_PAYLOADS], hp[0..<ONION_PAYLOADS - size[i]]);
    onion_key("rho", ss, key[..]); onion_xor(key[..], layer[..]);
    hp = layer;
    if i == n - 1 { onion_copy(hp[ONION_PAYLOADS - filled..<ONION_PAYLOADS], filler[0..<filled]); }
    onion_key("mu", ss, key[..]); onion_mac(key[..], hp[..], ad, mac[..]);
  }
  out[0] = 0;
  onion_copy(out[1..<34], route[0].epk[0..<33]);
  onion_copy(out[34..<34 + ONION_PAYLOADS], hp[..]);
  onion_copy(out[34 + ONION_PAYLOADS..<ONION_PACKET], mac[..]);
  onion_wipe(key[..]); onion_wipe(layer[..]); onion_wipe(filler[..]); route[0] = OnionRoute.{};
  return ONION_OK;
}
```

## Peeling one layer

A hop checks the HMAC first: a changed byte anywhere in the payloads or
the associated data fails, and the hop sends back
`invalid_onion_hmac`. It then appends 1,300 zero bytes, XORs all 2,600
with its `rho` stream, reads its payload's length and payload, and takes
the next HMAC. The next 1,300 bytes are the next hop's payloads. The zero
bytes it appended have become stream bytes, so the packet stays 1,366
bytes and the next hop cannot tell how far along the route it is.

<!-- source: coax/lib/lightning/onion.coax peel -->
```jai
// One hop's layer. The hop checks the HMAC, decrypts the payloads with
// 1,300 zero bytes appended, reads its own payload and the next HMAC, and
// shifts the rest left to make the next hop's packet. An all-zero next
// HMAC means this hop is the last. ss is kept to wrap a failure later.
OnionPeel :: struct { code: int; last: bool; payload_n: int; ss: [32] u8; }
onion_peel :: (secret: [] u8, onion: [] u8, ad: [] u8, payload: [] u8, next: [] u8) -> OnionPeel {
  if onion.count != ONION_PACKET { return OnionPeel.{code = ONION_BAD_LENGTH}; }
  if onion[0] != 0 { return OnionPeel.{code = ONION_BAD_VERSION}; }
  if payload.count < ONION_PAYLOADS || next.count < ONION_PACKET { return OnionPeel.{code = ONION_CAPACITY}; }
  k := onion_secret(secret);
  if k.code != ONION_OK { return OnionPeel.{code = k.code}; }
  r: OnionPeel;
  epk := onion[1..<34]; hp := onion[34..<34 + ONION_PAYLOADS];
  r.code = onion_ecdh(k.value, epk, r.ss[..]);
  k = OnionSecret.{};
  if r.code != ONION_OK { return r; }
  key: [32] u8; mac: [32] u8;
  onion_key("mu", r.ss[..], key[..]); onion_mac(key[..], hp, ad, mac[..]);
  if cb_ct_equal(mac[..], onion[34 + ONION_PAYLOADS..<ONION_PACKET]) != CB_MAX64 {
    onion_wipe(key[..]); return OnionPeel.{code = ONION_BAD_HMAC};
  }
  d: [2600] u8; onion_copy(d[0..<ONION_PAYLOADS], hp);
  onion_key("rho", r.ss[..], key[..]); onion_xor(key[..], d[..]);
  onion_wipe(key[..]);
  t := onion_take_vartime(d[..], payload, epk, r.ss[..], next);
  onion_wipe(d[..]);
  r.code = t.code; r.last = t.last; r.payload_n = t.payload_n;
  return r;
}
// The decrypted payloads, now this hop's own: its payload, then the next
// packet with the next ephemeral key, epk times SHA256(epk || ss).
OnionTake :: struct { code: int; last: bool; payload_n: int; }
onion_take_vartime :: (d: [] u8, payload: [] u8, epk: [] u8, ss: [] u8, next: [] u8) -> OnionTake {
  l := bigsize_read_vartime(d, 0);
  if l.code != LN_OK || l.value == 0 || l.value > cast(u64) ONION_PAYLOADS { return OnionTake.{code = ONION_BAD_PAYLOAD}; }
  n := cast(int) l.value; at := l.next + n;
  if at + ONION_HMAC > ONION_PAYLOADS { return OnionTake.{code = ONION_BAD_PAYLOAD}; }
  onion_copy(payload[0..<n], d[l.next..<at]);
  zero := true; for i: 0..<ONION_HMAC { if d[at + i] != 0 { zero = false; } }
  if zero { return OnionTake.{last = true, payload_n = n}; }
  b: [32] u8; onion_sha(epk, ss, b[..]);
  e := onion_tweak(b[..], epk);
  onion_wipe(b[..]);
  if e.code != ONION_OK { return OnionTake.{code = e.code}; }
  next[0] = 0;
  onion_copy(next[1..<34], e.bytes[..]);
  onion_copy(next[34..<34 + ONION_PAYLOADS], d[at + ONION_HMAC..<at + ONION_HMAC + ONION_PAYLOADS]);
  onion_copy(next[34 + ONION_PAYLOADS..<ONION_PACKET], d[at..<at + ONION_HMAC]);
  return OnionTake.{payload_n = n};
}
```

## Error onions

When a hop fails an HTLC, it sends a return packet back the way the
payment came. The erring hop writes the failure message, pads it so that
`failure_len` plus `pad_len` is at least 256, puts an HMAC under its `um`
key in front, and XORs the whole packet with its `ammag` stream. Each hop
on the way back XORs it with its own `ammag` stream. Only the payer knows
every shared secret. It removes the layers in route order and, after
each, checks that hop's `um` HMAC. The first that checks is the hop that
failed.

<!-- source: coax/lib/lightning/onion.coax errors -->
```jai
// The erring node's return packet: an HMAC under um, then failure_len,
// the failure message, pad_len and zero padding, so that failure_len plus
// pad_len is pad_to, at least 256. Every node on the way back, the erring
// node first, XORs the whole packet with its ammag stream. Returns the
// packet's length, or -1.
onion_error_create :: (ss: [] u8, failure: [] u8, pad_to: int, out: [] u8) -> int {
  pad := pad_to - failure.count;
  if pad < 0 { pad = 0; }
  n := ONION_HMAC + 2 + failure.count + 2 + pad;
  if pad_to < ONION_ERROR_PAD || n > out.count || n > ONION_MAX_ERROR { return -1; }
  ln_put16(out, ONION_HMAC, failure.count);
  onion_copy(out[ONION_HMAC + 2..<ONION_HMAC + 2 + failure.count], failure);
  ln_put16(out, ONION_HMAC + 2 + failure.count, pad);
  onion_wipe(out[n - pad..<n]);
  key: [32] u8; empty: [1] u8;
  onion_key("um", ss, key[..]); onion_mac(key[..], out[ONION_HMAC..<n], empty[0..<0], out[0..<ONION_HMAC]);
  onion_wipe(key[..]);
  onion_error_forward(ss, out[0..<n]);
  return n;
}
// Each node passing the error back adds its layer.
onion_error_forward :: (ss: [] u8, packet: [] u8) {
  key: [32] u8;
  onion_key("ammag", ss, key[..]); onion_xor(key[..], packet);
  onion_wipe(key[..]);
}
// The payer removes the layers in route order. After hop i's layer, the
// packet carries hop i's um HMAC if hop i made it. The failure message is
// at packet[failure..<failure + failure_n].
OnionFailure :: struct { code: int; hop: int; failure: int; failure_n: int; }
onion_error_open :: (route: [] OnionRoute, packet: [] u8) -> OnionFailure {
  if packet.count < ONION_HMAC + 4 { return OnionFailure.{code = ONION_BAD_LENGTH}; }
  key: [32] u8; mac: [32] u8; empty: [1] u8;
  for i: 0..<route[0].hops {
    ss := route[0].ss[32 * i..<32 * i + 32];
    onion_error_forward(ss, packet);
    onion_key("um", ss, key[..]); onion_mac(key[..], packet[ONION_HMAC..<packet.count], empty[0..<0], mac[..]);
    if cb_ct_equal(mac[..], packet[0..<ONION_HMAC]) == CB_MAX64 {
      onion_wipe(key[..]);
      n := ln_u16(packet, ONION_HMAC);
      if n > packet.count - ONION_HMAC - 2 { return OnionFailure.{code = ONION_BAD_LENGTH, hop = i}; }
      return OnionFailure.{hop = i, failure = ONION_HMAC + 2, failure_n = n};
    }
  }
  onion_wipe(key[..]);
  return OnionFailure.{code = ONION_UNKNOWN, hop = -1};
}
```

## Blinded paths

A recipient that does not want to reveal its node makes a blinded path to
itself, starting at an introduction node. It picks a path secret e. For
each hop, the path key E = e·G is what that hop will be given, ss is ECDH
of e and the hop's node key, and the hop's blinded node ID is its node key
times HMAC("blinded_node_id", ss). The data for the hop, such as the next
channel, is sealed with ChaCha20-Poly1305 under `rho` with a zero nonce:
the hop's `encrypted_recipient_data`. Then e becomes e·SHA256(E || ss).

The payer sees only the introduction node's real ID, the first path key,
and the blinded node IDs with their encrypted data. It wraps the onion to
the introduction node's real key and to the blinded IDs after it.

<!-- source: coax/lib/lightning/onion.coax blind -->
```jai
// Making a blinded path. The recipient picks a path secret e for the
// first hop; for each hop, E = e * G is the path key that hop is given,
// ss is ECDH of e and the hop's node key, the hop's blinded node ID is its
// node key times HMAC("blinded_node_id", ss), and its data is sealed with
// ChaCha20-Poly1305 under rho with a zero nonce. Then e becomes
// e * SHA256(E || ss). Setting e anew joins another path.
OnionBlindHop :: struct { code: int; path_key: [33] u8; blinded: [33] u8; }
onion_blind_hop :: (e: [] u8, node_id: [] u8, data: [] u8, box: [] u8) -> OnionBlindHop {
  if box.count != data.count + ONION_TAG { return OnionBlindHop.{code = ONION_CAPACITY}; }
  k := onion_secret(e);
  if k.code != ONION_OK { return OnionBlindHop.{code = k.code}; }
  r: OnionBlindHop; ss: [32] u8; key: [32] u8; nonce: [12] u8;
  path := onion_encode(smc_multiply(k.value, sg_generator()));
  r.path_key = path.bytes;
  r.code = onion_ecdh(k.value, node_id, ss[..]);
  if r.code == ONION_OK {
    onion_key("blinded_node_id", ss[..], key[..]);
    b := onion_tweak(key[..], node_id); r.blinded = b.bytes; r.code = b.code;
    onion_key("rho", ss[..], key[..]);
    #assert chacha20_poly1305_seal(key[..], nonce[..], nonce[0..<0], data, box, false) == CCP_OK;
    h: [32] u8; onion_sha(path.bytes[..], ss[..], h[..]);
    f := onion_secret(h[..]);
    stored := sc_store(sc_mul(k.value, f.value), e);
    onion_wipe(h[..]); f = OnionSecret.{};
  }
  k = OnionSecret.{}; onion_wipe(ss[..]); onion_wipe(key[..]);
  return r;
}
```

## Unblinding

A hop on a blinded path receives the path key, in the onion's payload at
the introduction node and next to the HTLC after it. Its blinded secret is
its node secret times HMAC("blinded_node_id", ss), which is the secret of
its blinded node ID, so it can peel the onion. It opens its
`encrypted_recipient_data` under `rho` and passes on the next path key.
When its data holds `next_path_key_override`, the path joins another one,
and the override is the next path key instead.

<!-- source: coax/lib/lightning/onion.coax unblind -->
```jai
// A hop on a blinded path is given the path key E. Its blinded secret is
// its node secret times HMAC("blinded_node_id", ss), with ss the ECDH of
// its node secret and E: that is the key the onion was wrapped to.
onion_blinded_secret :: (node_secret: [] u8, path_key: [] u8, out: [] u8) -> int {
  k := onion_secret(node_secret);
  if k.code != ONION_OK { return k.code; }
  ss: [32] u8; h: [32] u8;
  code := onion_ecdh(k.value, path_key, ss[..]);
  if code == ONION_OK {
    onion_key("blinded_node_id", ss[..], h[..]);
    f := onion_secret(h[..]);
    stored := sc_store(sc_mul(k.value, f.value), out);
    f = OnionSecret.{};
  }
  k = OnionSecret.{}; onion_wipe(ss[..]); onion_wipe(h[..]);
  return code;
}
// It opens its encrypted_recipient_data under rho, and passes on the next
// path key: E * SHA256(E || ss), unless the data holds
// next_path_key_override (type 8), where the path joins another one.
OnionUnblind :: struct { code: int; data_n: int; next_path_key: [33] u8; }
onion_unblind :: (node_secret: [] u8, path_key: [] u8, box: [] u8, data: [] u8) -> OnionUnblind {
  if box.count < ONION_TAG || data.count < box.count - ONION_TAG { return OnionUnblind.{code = ONION_CAPACITY}; }
  k := onion_secret(node_secret);
  if k.code != ONION_OK { return OnionUnblind.{code = k.code}; }
  r: OnionUnblind; ss: [32] u8; key: [32] u8; nonce: [12] u8;
  r.code = onion_ecdh(k.value, path_key, ss[..]);
  k = OnionSecret.{};
  if r.code != ONION_OK { return r; }
  r.data_n = box.count - ONION_TAG;
  onion_key("rho", ss[..], key[..]);
  opened := chacha20_poly1305_open(key[..], nonce[..], nonce[0..<0], box, data[0..<r.data_n], false) == CCP_OK;
  h: [32] u8; onion_sha(path_key, ss[..], h[..]);
  next := onion_tweak(h[..], path_key);
  onion_wipe(ss[..]); onion_wipe(key[..]); onion_wipe(h[..]);
  if !opened { return OnionUnblind.{code = ONION_BAD_TAG}; }
  r.next_path_key = next.bytes; r.code = next.code;
  over := onion_record_vartime(data[0..<r.data_n], 8);
  if over.code != LN_OK { r.code = ONION_BAD_PAYLOAD; }
  if over.found {
    if over.length != 33 || !se_decode(data[over.start..<over.start + 33]).ok { r.code = ONION_BAD_PAYLOAD; } else {
      onion_copy(r.next_path_key[..], data[over.start..<over.start + 33]);
    }
  }
  return r;
}
// One record of a TLV stream, by type: where its value is, if present.
OnionRecord :: struct { code: int; found: bool; start: int; length: int; }
onion_record_vartime :: (b: [] u8, type: u64) -> OnionRecord {
  at := 0; previous: u64 = 0; first := true; r: OnionRecord;
  while at < b.count {
    t := tlv_next_vartime(b, at, previous, first);
    if t.code != LN_OK { return OnionRecord.{code = t.code}; }
    if t.type == type { r.found = true; r.start = t.start; r.length = t.length; }
    previous = t.type; first = false; at = t.next;
  }
  return r;
}
```

## The example

The example,
[bitcoin/examples/lightning-onion.coax](../examples/lightning-onion.coax),
runs BOLT 4's onion and error vectors, sends a payment from Alice to Dave
through Bob and Carol, lets Carol fail it, and then pays Dave through a
blinded path that starts at Carol.

<!-- source: bitcoin/examples/lightning-onion.coax example -->
```jai
#load "../../coax/lib/lightning/onion.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex.
show :: (b: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
// A 32-byte secret whose bytes are k, k + 1, k + 2, ...
secret :: (k: int) -> [32] u8 { s: [32] u8; for i: 0..<32 { s[i] = cast(u8) ((k + i) & 255); } return s; }
node_id :: (k: int) -> [33] u8 { s := secret(k); p := onion_public(s[..]); return p.bytes; }
// A short channel ID, block x transaction x output, as 8 bytes.
scid :: (block: u64, tx: u64, output: u64) -> [8] u8 {
  v := (block << 40) | (tx << 16) | output; b: [8] u8;
  for i: 0..<8 { b[i] = cast(u8) ((v >> cast(u64) (56 - 8 * i)) & 255); }
  return b;
}
show_scid :: (b: [] u8) {
  v: u64 = 0; for i: 0..<8 { v = (v << 8) | cast(u64) b[i]; }
  print("%x%x%", v >> 40, (v >> 16) & 0xffffff, v & 0xffff);
}
// A TLV stream being written.
Tlv :: struct { b: [256] u8; n: int; }
put :: (t: [] Tlv, type: u64, value: [] u8) { t[0].n = tlv_write_vartime(t[0].b[..], t[0].n, type, value); }
put_tu :: (t: [] Tlv, type: u64, v: u64) { b: [8] u8; n := tu_write_vartime(b[..], 0, v); put(t, type, b[0..<n]); }
// A record's value as a number.
number :: (payload: [] u8, type: u64) -> u64 {
  r := onion_record_vartime(payload, type);
  return tu_read_vartime(payload[r.start..<r.start + r.length], 8).value;
}
// A forwarding hop's payload: amt_to_forward (2), outgoing_cltv_value (4)
// and short_channel_id (6). The last hop has no short_channel_id.
forward :: (amount: u64, cltv: u64, channel: [] u8) -> Tlv {
  t: [1] Tlv; put_tu(t[..], 2, amount); put_tu(t[..], 4, cltv);
  if channel.count > 0 { put(t[..], 6, channel); }
  return t[0];
}
// What a hop reads in its payload.
report :: (name: string, p: OnionPeel, payload: [] u8) {
  print("  %: % msat, cltv %", name, number(payload, 2), number(payload, 4));
  r := onion_record_vartime(payload, 6);
  if r.found { print(", forward over "); show_scid(payload[r.start..<r.start + 8]); }
  if p.last { print(", last hop"); }
  print("\n");
}
ONION_NAMES :: [11] string.["OK", "BAD_POINT", "BAD_KEY", "BAD_VERSION", "BAD_HMAC", "BAD_PAYLOAD", "TOO_LONG",
                            "BAD_LENGTH", "CAPACITY", "BAD_TAG", "UNKNOWN"];

main :: () {
  // ---- BOLT 4's packet and error vectors
  print("BOLT 4 vectors\n");
  h: [1] OnionHops; buf: [1300] u8; onion: [1366] u8; next: [1366] u8; payload: [1300] u8; s: [32] u8; ad: [32] u8;
  for row: ONION_TEST.hops {
    key: [33] u8; p := ln_unhex(row.payload, buf[..]); l := bigsize_read_vartime(p, 0);
    added := onion_add_hop_vartime(h[..], ln_unhex(row.pubkey, key[..]), p[l.next..<p.count]);
  }
  code := onion_wrap(ln_unhex(ONION_TEST.session, s[..]), h[0].pubkeys[0..<h[0].keys_n], h[0].payloads[0..<h[0].payloads_n],
                     ln_unhex(ONION_TEST.ad, ad[..]), onion[..]);
  print("  onion-test.json: the onion matches: %\n", ln_hex_is(onion[..], ONION_TEST.onion));
  read := 0; last := 0;
  for row, i: ONION_TEST.hops {
    p := onion_peel(ln_unhex(row.secret, s[..]), onion[..], ad[..], payload[..], next[..]);
    w := ln_unhex(row.payload, buf[..]); l := bigsize_read_vartime(w, 0);
    if p.code == ONION_OK && p.payload_n == w.count - l.next {
      same := true; for j: 0..<p.payload_n { if payload[j] != w[l.next + j] { same = false; } }
      if same { read += 1; }
    }
    if p.last { last = i; }
    onion = next;
  }
  print("  hops that read their own payload: % of 5, the last is hop %\n", read, last);
  route: [1] OnionRoute; keys: [165] u8; f: [2] u8; packet: [292] u8;
  for row, i: ONION_ERROR.hops { k := ln_unhex(row.pubkey, keys[33 * i..<33 * i + 33]); }
  code = onion_route(ln_unhex(ONION_ERROR.session, s[..]), keys[..], route[..]);
  n := onion_error_create(route[0].ss[128..<160], ln_unhex(ONION_ERROR.failure, f[..]), 256, packet[..]);
  for k: 0..<4 { onion_error_forward(route[0].ss[32 * (3 - k)..<32 * (3 - k) + 32], packet[0..<n]); }
  print("  onion-error-test.json: the error packet matches: %\n", ln_hex_is(packet[0..<n], ONION_ERROR.errorpacket));
  got := onion_error_open(route[..], packet[0..<n]);
  print("  the origin finds hop %, failure ", got.hop); show(packet[got.failure..<got.failure + got.failure_n]); print("\n");

  // ---- a payment from Alice to Dave through Bob and Carol
  print("Alice pays Dave through Bob and Carol\n");
  bob := secret(21); carol := secret(41); dave := secret(61); session := secret(81); hash := secret(101);
  bob_id := node_id(21); carol_id := node_id(41); dave_id := node_id(61);
  bob_carol := scid(800000, 1, 0); carol_dave := scid(800001, 2, 1);
  h[0] = OnionHops.{};
  t := forward(1002000, 800180, bob_carol[..]); code = onion_add_hop_vartime(h[..], bob_id[..], t.b[0..<t.n]);
  t = forward(1000000, 800140, carol_dave[..]); code = onion_add_hop_vartime(h[..], carol_id[..], t.b[0..<t.n]);
  none: [1] u8;
  t = forward(1000000, 800100, none[0..<0]); code = onion_add_hop_vartime(h[..], dave_id[..], t.b[0..<t.n]);
  code = onion_wrap(session[..], h[0].pubkeys[0..<h[0].keys_n], h[0].payloads[0..<h[0].payloads_n], hash[..], onion[..]);
  print("  onion: % bytes; payloads and HMACs use % of 1300\n", onion.count, h[0].payloads_n + 3 * 32);
  first := onion;
  names := [3] string.["Bob", "Carol", "Dave"];
  for i: 0..<3 {
    print("  % sees ephemeral key ", names[i]); show(onion[1..<34]); print("\n");
    k := ifx i == 0 then bob else ifx i == 1 then carol else dave;
    p := onion_peel(k[..], onion[..], hash[..], payload[..], next[..]);
    report(names[i], p, payload[0..<p.payload_n]);
    onion = next;
  }
  changed := first; changed[500] ^= 1;
  print("  Bob given a changed byte: %\n", ONION_NAMES[onion_peel(bob[..], changed[..], hash[..], payload[..], next[..]).code]);
  other := secret(102);
  print("  Bob given another payment hash: %\n", ONION_NAMES[onion_peel(bob[..], first[..], other[..], payload[..], next[..]).code]);

  // ---- Carol fails it
  print("Carol fails the payment\n");
  p := onion_peel(bob[..], first[..], hash[..], payload[..], next[..]);
  bob_ss := p.ss;
  p = onion_peel(carol[..], next[..], hash[..], payload[..], onion[..]);
  failure := [4] u8.[0x10, 0x07, 0, 0];   // temporary_channel_failure, no channel_update
  n = onion_error_create(p.ss[..], failure[..], 256, packet[..]);
  onion_error_forward(bob_ss[..], packet[0..<n]);
  print("  the packet Alice receives: % bytes\n", n);
  code = onion_route(session[..], h[0].pubkeys[0..<h[0].keys_n], route[..]);
  back := packet;
  got = onion_error_open(route[..], packet[0..<n]);
  print("  Alice finds hop % (%), failure ", got.hop, names[got.hop]); show(packet[got.failure..<got.failure + got.failure_n]); print("\n");
  back[100] ^= 1;
  print("  a packet changed on the way: %\n", ONION_NAMES[onion_error_open(route[..], back[0..<n]).code]);

  // ---- Dave hides behind a blinded path that starts at Carol
  print("Dave's blinded path from Carol\n");
  e := secret(121); path_id := secret(141);
  carol_data: [1] Tlv; put(carol_data[..], 2, carol_dave[..]);
  dave_data: [1] Tlv; put(dave_data[..], 6, path_id[..]);
  carol_box: [64] u8; dave_box: [64] u8;
  cb := onion_blind_hop(e[..], carol_id[..], carol_data[0].b[0..<carol_data[0].n], carol_box[0..<carol_data[0].n + 16]);
  db := onion_blind_hop(e[..], dave_id[..], dave_data[0].b[0..<dave_data[0].n], dave_box[0..<dave_data[0].n + 16]);
  print("  Carol's blinded node ID is her node ID: %\n", cb_ct_equal(cb.blinded[..], carol_id[..]) == CB_MAX64);
  print("  Dave's blinded node ID: "); show(db.blinded[..]); print("\n");
  print("  Dave's blinded node ID is his node ID: %\n", cb_ct_equal(db.blinded[..], dave_id[..]) == CB_MAX64);
  // Alice pays to the path: Carol, the introduction node, by her node ID
  // with the first path key; Dave by his blinded node ID.
  h[0] = OnionHops.{};
  t = forward(1002000, 800180, bob_carol[..]); code = onion_add_hop_vartime(h[..], bob_id[..], t.b[0..<t.n]);
  tt: [1] Tlv; put(tt[..], 10, carol_box[0..<carol_data[0].n + 16]); put(tt[..], 12, cb.path_key[..]);
  code = onion_add_hop_vartime(h[..], carol_id[..], tt[0].b[0..<tt[0].n]);
  tt[0] = Tlv.{}; put_tu(tt[..], 2, 1000000); put_tu(tt[..], 4, 800100);
  put(tt[..], 10, dave_box[0..<dave_data[0].n + 16]); put_tu(tt[..], 18, 1000000);
  code = onion_add_hop_vartime(h[..], db.blinded[..], tt[0].b[0..<tt[0].n]);
  session2 := secret(161);
  code = onion_wrap(session2[..], h[0].pubkeys[0..<h[0].keys_n], h[0].payloads[0..<h[0].payloads_n], hash[..], onion[..]);
  p = onion_peel(bob[..], onion[..], hash[..], payload[..], next[..]);
  report("Bob", p, payload[0..<p.payload_n]);
  p = onion_peel(carol[..], next[..], hash[..], payload[..], onion[..]);
  key := onion_record_vartime(payload[0..<p.payload_n], 12);
  box := onion_record_vartime(payload[0..<p.payload_n], 10);
  data: [64] u8;
  u := onion_unblind(carol[..], payload[key.start..<key.start + 33], payload[box.start..<box.start + box.length], data[..]);
  print("  Carol: introduction node, her data says forward over "); show_scid(data[2..<10]); print("\n");
  print("  Carol passes on the path key Dave's hop was made with: %\n", cb_ct_equal(u.next_path_key[..], db.path_key[..]) == CB_MAX64);
  blinded: [32] u8;
  code = onion_blinded_secret(dave[..], u.next_path_key[..], blinded[..]);
  p = onion_peel(blinded[..], onion[..], hash[..], payload[..], next[..]);
  report("Dave", p, payload[0..<p.payload_n]);
  box = onion_record_vartime(payload[0..<p.payload_n], 10);
  v := onion_unblind(dave[..], u.next_path_key[..], payload[box.start..<box.start + box.length], data[..]);
  print("  Dave's data holds his path_id: %\n", v.code == ONION_OK && cb_ct_equal(data[2..<34], path_id[..]) == CB_MAX64);
  w := onion_unblind(bob[..], u.next_path_key[..], payload[box.start..<box.start + box.length], data[..]);
  print("  Bob opening Dave's data: %\n", ONION_NAMES[w.code]);
  print("  Dave's own key opening the onion: %\n", ONION_NAMES[onion_peel(dave[..], onion[..], hash[..], payload[..], next[..]).code]);
}
```

From the repository root:

```shell
coax/bin/coax run bitcoin/examples/lightning-onion.coax
```

The output:

```text
BOLT 4 vectors
  onion-test.json: the onion matches: true
  hops that read their own payload: 5 of 5, the last is hop 4
  onion-error-test.json: the error packet matches: true
  the origin finds hop 4, failure 2002
Alice pays Dave through Bob and Carol
  onion: 1366 bytes; payloads and HMACs use 149 of 1300
  Bob sees ephemeral key 03ade3c1324a45296291c9f53547457b43fb254d1b11e3cad30b6669a9584ba846
  Bob: 1002000 msat, cltv 800180, forward over 800000x1x0
  Carol sees ephemeral key 03c6a288bbb068819a0b82a010922d71ccc1e40ee354a59238586f68fc3da6403b
  Carol: 1000000 msat, cltv 800140, forward over 800001x2x1
  Dave sees ephemeral key 021834e7718c9a52f5782ea287143fea67d1aea5b162aa038ce149747a0afd37cc
  Dave: 1000000 msat, cltv 800100, last hop
  Bob given a changed byte: BAD_HMAC
  Bob given another payment hash: BAD_HMAC
Carol fails the payment
  the packet Alice receives: 292 bytes
  Alice finds hop 1 (Carol), failure 10070000
  a packet changed on the way: UNKNOWN
Dave's blinded path from Carol
  Carol's blinded node ID is her node ID: false
  Dave's blinded node ID: 023312129f6b26f277053cd00b51d6d3df9e40695e11e8f7c5af6261c3dddbc020
  Dave's blinded node ID is his node ID: false
  Bob: 1002000 msat, cltv 800180, forward over 800000x1x0
  Carol: introduction node, her data says forward over 800001x2x1
  Carol passes on the path key Dave's hop was made with: true
  Dave: 1000000 msat, cltv 800100, last hop
  Dave's data holds his path_id: true
  Bob opening Dave's data: BAD_TAG
  Dave's own key opening the onion: BAD_HMAC
```

What the output shows:

- The onion of BOLT 4's `onion-test.json` comes out byte for byte, and
  each of its five hops reads its own payload. The error of
  `onion-error-test.json` comes out byte for byte, and the payer finds
  that hop 4 made it.
- Alice's onion is 1,366 bytes, of which her three payloads and HMACs use
  149. Bob, Carol and Dave each see a different ephemeral key and read only
  their own amount, timeout and next channel. Dave sees that he is the
  last hop.
- One changed byte, or a different payment hash, fails Bob's HMAC check.
- Carol's failure, `temporary_channel_failure`, reaches Alice through
  Bob's layer, and Alice finds that hop 1, Carol, made it. A changed byte
  makes the packet no hop's.
- Neither Carol's nor Dave's blinded node ID is their node ID. Carol
  peels the onion with her own key, opens her data to find the next
  channel, and passes on the path key Dave's entry was made with. Dave
  peels with his blinded secret and finds his `path_id`.
- Bob cannot open Dave's data, and Dave's own node key does not open an
  onion wrapped to his blinded node ID.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/lightning-onion.out](../fixtures/lightning-onion.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/lightning-onion.coax](../examples/checks/lightning-onion.coax),
which loads the example:

- for routes of one to four hops, each hop reads its own payload and only
  the last sees an all-zero next HMAC
- whichever hop of three fails, the payer attributes the failure to it
- on a three-hop blinded path, each blinded secret times G is the hop's
  blinded node ID, and each hop passes on the next hop's path key

<!-- source: bitcoin/examples/checks/lightning-onion.coax check -->
```jai
// For routes of one to four hops, each hop peels exactly its own payload,
// and only the last hop sees an all-zero next HMAC.
#forall "every hop of a route reads its own payload" hops: 1..<5 #steps 8000000000 {
  h: [1] OnionHops; session := secret(7 * hops); ad := secret(3);
  for i: 0..<hops {
    id := node_id(10 * i + 1);
    t := forward(cast(u64) (1000 * (i + 1)), cast(u64) (500 + i), ad[0..<8]);
    #assert onion_add_hop_vartime(h[..], id[..], t.b[0..<t.n]) == ONION_OK;
  }
  onion: [1366] u8; next: [1366] u8; payload: [1300] u8;
  #assert onion_wrap(session[..], h[0].pubkeys[0..<h[0].keys_n], h[0].payloads[0..<h[0].payloads_n], ad[..], onion[..]) == ONION_OK;
  for i: 0..<hops {
    k := secret(10 * i + 1);
    p := onion_peel(k[..], onion[..], ad[..], payload[..], next[..]);
    #assert p.code == ONION_OK && p.last == (i == hops - 1);
    #assert number(payload[0..<p.payload_n], 2) == cast(u64) (1000 * (i + 1));
    onion = next;
  }
}
// Whichever hop of a three-hop route fails, the payer attributes the
// failure to that hop and reads its message.
#forall "the payer finds the hop that failed" erring: 0..<3 #steps 4000000000 {
  route: [1] OnionRoute; keys: [99] u8; session := secret(5);
  for i: 0..<3 { id := node_id(10 * i + 1); onion_copy(keys[33 * i..<33 * i + 33], id[..]); }
  #assert onion_route(session[..], keys[..], route[..]) == ONION_OK;
  failure := [2] u8.[0x40, cast(u8) erring]; packet: [292] u8;
  #assert onion_error_create(route[0].ss[32 * erring..<32 * erring + 32], failure[..], 256, packet[..]) == 292;
  for k: 0..<erring { i := erring - 1 - k; onion_error_forward(route[0].ss[32 * i..<32 * i + 32], packet[..]); }
  got := onion_error_open(route[..], packet[..]);
  #assert got.code == ONION_OK && got.hop == erring && got.failure_n == 2 && packet[got.failure + 1] == cast(u8) erring;
}
// On a three-hop blinded path, each hop's blinded secret times G is its
// blinded node ID, and each hop passes on the path key the next hop's
// entry was made with.
#test "each hop's blinded secret matches its blinded node ID" #steps 4000000000 {
  e := secret(9); data := [3] u8.[1, 1, 0]; boxes: [57] u8; made: [3] OnionBlindHop;
  for i: 0..<3 { id := node_id(10 * i + 1); made[i] = onion_blind_hop(e[..], id[..], data[..], boxes[19 * i..<19 * i + 19]); }
  out: [3] u8; blinded: [32] u8;
  for i: 0..<3 {
    k := secret(10 * i + 1);
    #assert onion_blinded_secret(k[..], made[i].path_key[..], blinded[..]) == ONION_OK;
    pub := onion_public(blinded[..]);
    #assert cb_ct_equal(pub.bytes[..], made[i].blinded[..]) == CB_MAX64;
    u := onion_unblind(k[..], made[i].path_key[..], boxes[19 * i..<19 * i + 19], out[..]);
    #assert u.code == ONION_OK && u.data_n == 3;
    if i < 2 { #assert cb_ct_equal(u.next_path_key[..], made[i + 1].path_key[..]) == CB_MAX64; }
  }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/lightning-onion.coax
```

`coax test` runs 184 checks, including the loaded files' own checks, in about four and a half minutes on 2026-11-03.

### The library's own checks

`onion.coax` is an `#authority` file. Its checks run every vector of
BOLT 4's five `bolt04/*.json` files and its "Returning Errors" trace:

- `onion-test.json`: the onion byte for byte, and each hop's payload
- `onion-error-test.json`: each hop's shared secret and `ammag` key, the
  `um` key, the error packet, and the payer finding hop 4
- the "Returning Errors" trace: a 320-byte failure padded to 1,024 bytes,
  and the packet each node sends back
- `route-blinding-test.json`: each hop's path key, blinded node ID,
  encrypted data, blinded secret and next path key, with the override
- `blinded-payment-onion-test.json`: the onion, and the onion each hop
  receives, with the path keys
- `blinded-onion-message-onion-test.json`: the blinded path, the onion
  message packet, and the `onion_message` each hop receives and sends on

They also check the hop builder and refusals: a changed byte or
associated data, a bad version or key, a short packet or buffer, payloads
above 1,300 bytes, an empty or overrunning payload, an error from another
route, and changed `encrypted_recipient_data`.

```shell
coax/bin/coax test coax/lib/lightning/onion.coax
```

`coax test` runs 178 checks, including those of the files it loads, in about three minutes on 2026-11-03. Mutation testing: not run (owner speed order).

### BOLT 4 and an independent Sphinx

[onion.py](../../coax/tests/lightning/onion.py) runs every BOLT 4 vector
on [sphinx.py](../../coax/tests/lightning/sphinx.py), a separate Sphinx,
error onion and route blinding written from BOLT 4 on
[ln_crypto.py](../../coax/tests/lightning/ln_crypto.py)'s secp256k1 and
ChaCha20, sharing no code with Coax. It then computes every line the
example prints and compares the transcript with the fixture.

```shell
python3 -I coax/tests/lightning/onion.py --coax coax/bin/coax
```

On 2026-11-03 every vector passes, all 30 lines agree, and the example's
output in native debug and release builds equals them. Wasm was not run.

[make_onion_vectors.py](../../coax/tests/lightning/make_onion_vectors.py)
reads the pinned BOLT 4 files into
[onion_vectors.coax](../../coax/tests/lightning/onion_vectors.coax).

## What this chapter leaves out

- Choosing the route, the amounts and the timeouts: chapter 113.
- Carrying the onion in `update_add_htlc` and the error in
  `update_fail_htlc`: chapters 109 and 113.
- What each hop does with its payload: checking the amount and timeout
  against its channel, and the failure codes it returns.
- Attributable failures (`attribution_data`, with hold times and
  truncated HMACs) and `fulfillment_payload`. BOLT 4's "Returning
  success" values and the attribution data in its error trace are not
  checked here.
- The origin's constant-time error decryption, which keeps going for 27
  rounds with fixed keys after it finds the erring hop.
- Onion messages larger than 1,300 bytes of payloads, and the
  `onion_message` and `reply_path` handling around them: chapter 115.

## Summary

- Each hop's shared secret is ECDH of an ephemeral key and its node key,
  and the blinding factor SHA256(epk || ss) gives the next hop a
  different ephemeral key.
- `rho`, `mu`, `um`, `pad` and `ammag` are HMACs of the shared secret
  with the key's name as the HMAC key. The stream is ChaCha20 with a zero
  nonce.
- The payer wraps from the last hop to the first, puts the filler at the
  end, and chains an HMAC over each layer and the payment hash. A hop
  checks the HMAC, decrypts with 1,300 extra bytes, reads its payload and
  shifts the rest.
- An error onion is an HMAC under `um` and a padded failure, with an
  `ammag` layer from every hop. The payer removes the layers in order
  until one hop's HMAC checks.
- A blinded path gives each hop a blinded node ID and encrypted data. A
  hop given the path key computes its blinded secret, peels the onion,
  opens its data, and passes on the next path key or the override.

## Dependencies

- [8. HMAC-SHA256](ch008-hmac-sha256.md)
- [11. ChaCha20](ch011-chacha20.md)
- [16. Scalar multiplication](ch016-scalar-multiplication.md)

## Sources

- [BOLT 4](https://github.com/lightning/bolts/blob/master/04-onion-routing.md),
  "Key Generation", "Pseudo Random Byte Stream", "Packet Structure",
  "Route Blinding", "Shared Secret", "Blinding Ephemeral Onion Keys",
  "Packet Construction", "Onion Decryption", "Filler Generation",
  "Returning Errors" and "Test Vector", at lightning/bolts revision
  `19146282`, pinned in
  [coax/tests/vectors/upstream/bolts/](../../coax/tests/vectors/upstream/bolts/04-onion-routing.md)
  with its five vector files in
  [`bolt04/`](../../coax/tests/vectors/upstream/bolts/bolt04/onion-test.json).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
