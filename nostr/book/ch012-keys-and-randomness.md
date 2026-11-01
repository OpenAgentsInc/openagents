# 12. Keys and randomness

A Nostr identity is a random scalar. This chapter defines where randomness
comes from on each host, how a key is drawn from it, and how the rest of the
stack asks a signer for a signature without ever holding the key.

## What this chapter teaches

- The entropy contract: the host fills a buffer or fails, and there is no
  fallback to a weak source.
- Key generation: draw 32 bytes, refuse 0 and values at least `n`, and draw
  again.
- How to decide `k < n` by comparing limbs, and why the code subtracts
  instead.
- The x-only public key: the x coordinate of `k·G`.
- A signer request that fixes the NIP-01 ID the reply must have, so a reply
  is accepted only for that identity and body.

## Where the code is

| File | What it holds |
| --- | --- |
| [entropy.coax](../../coax/lib/crypto/entropy.coax) | The request and completion policy every host runs: `entropy_admit`, `entropy_completed`, `entropy_finish` (`#authority`) |
| [entropy-native.c](../../coax/platform/entropy-native.c) | The native host effect: `SecRandomCopyBytes` on Apple, `getrandom` on Linux |
| [entropy.js](../../coax/web/entropy.js) and [entropy-wasm.c](../../coax/platform/entropy-wasm.c) | The browser host effect: WebCrypto `getRandomValues` in chunks of 65,536 bytes |
| [secp_scalar.coax](../../coax/lib/crypto/secp_scalar.coax) | `sc_load`, which refuses 32 bytes that are `n` or more, and `sc_is_zero` (chapter 8) |
| [keys.coax](../../coax/lib/nostr/keys.coax) | Signer requests: one lifecycle for the local slot, a NIP-07 browser signer and, later, a phone over NIP-46 (`#authority`) |
| [nostr/examples/keys.coax](../examples/keys.coax) | This chapter's test host, `draw_key` and `derive_public_key` |

The library has no `draw_key`: each app draws through its host and checks
the result with `sc_load`, as the example does. The example's version shows
the loop with a test host so that its output is the same on every run.

## Where randomness comes from

A secret key, a signing nonce's auxiliary bytes and a NIP-44 nonce all come
from the operating system's random source. Coax never generates random bytes
itself. It asks the host and checks the answer:

1. The caller names a count (0 to 1 MiB), an output buffer and a scratch
   buffer. `entropy_admit` refuses a bad count (`ENTROPY_COUNT`), a buffer
   too small (`ENTROPY_CAPACITY`), a span that is not valid memory
   (`ENTROPY_SPAN`) and buffers that overlap (`ENTROPY_ALIAS`) before the
   host is asked for anything.
2. The host fills scratch from its source:
   - macOS and iOS: `SecRandomCopyBytes`.
   - Linux: `getrandom` with flags 0, repeated after `EINTR` and partial
     reads.
   - The browser: `crypto.getRandomValues`, at most 65,536 bytes per call,
     so `entropy_full_chunks` and `entropy_tail` split a request.
3. `entropy_completed` judges the host's report. No source is
   `ENTROPY_UNAVAILABLE`, a failed call is `ENTROPY_FAILED`, and a count other
   than the one requested is `ENTROPY_SHORT`.
4. `entropy_finish` copies scratch to the output only on success, then
   zeroes the scratch prefix. On any failure the output is unchanged: the
   caller never sees a partial draw.

There is no fallback. A host that can't supply randomness makes the request
fail; it never substitutes a clock, a counter or a seeded generator. The
production API takes no seed and no injected provider. A test provider
exists only in a C build that defines `COAX_ENTROPY_TEST_PROVIDER`.

The scratch reset is an ordinary memory write. It is not a guarantee that
the bytes are gone from every copy the compiler or the OS made; chapter 3
covers what Coax claims about secrets.

## Drawing a key

A secp256k1 secret key is an integer `k` with `0 < k < n`, where

```jai
n = fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141
```

Thirty-two random bytes give an integer from 0 to `2^256 - 1`. Most are
keys. The rest are 0 and the `2^256 - n` values from `n` up. Reducing such a
value mod `n` would make small keys twice as likely as others, and 0 has no
public key. So the rule is rejection: refuse the draw and draw again.

The procedure:

1. Draw 32 bytes from the host. If the host fails, stop and report its
   error.
2. Load them big-endian with `sc_load`. Its `ok` is false when the value is
   `n` or more.
3. If `ok` is true and the value is not zero, the bytes are the key.
4. Otherwise draw again, at most `KEY_MAX_DRAWS` (8) times in all.

A draw is refused with probability

```jai
(2^256 - n + 1) / 2^256 = 432420386565659656852420866394968145601 / 2^256
                        ≈ 3.7 × 10^-39 ≈ 2^-127.65
```

so a working source almost never needs a second draw, and eight refusals in
a row mean the source is broken: it returns zeros, or a constant. The bound
turns that into the refusal `KEY_EXHAUSTED` instead of a loop that never
ends. On every refusal the key buffer is zeroed.

The loop branches on whether a draw was refused. That reveals only that a
discarded value was out of range, not anything about the key that is kept.

### Worked example: is a value less than n?

Write `n` as eight 32-bit limbs, most significant first:

| Limb | 7 | 6 | 5 | 4 | 3 | 2 | 1 | 0 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `n` | `ffffffff` | `ffffffff` | `ffffffff` | `fffffffe` | `baaedce6` | `af48a03b` | `bfd25e8c` | `d0364141` |
| above | `ffffffff` | `ffffffff` | `ffffffff` | `fffffffe` | `baaedce6` | `af48a03c` | `00000000` | `00000000` |
| below | `ffffffff` | `ffffffff` | `ffffffff` | `fffffffe` | `baaedce6` | `af48a03a` | `ffffffff` | `ffffffff` |

By hand, compare from the top limb down and stop at the first limb that
differs:

1. Limbs 7 to 3 of all three values are equal.
2. Limb 2: `af48a03c > af48a03b`, so "above" is at least `n` and is refused,
   even though its lower limbs are 0. `af48a03a < af48a03b`, so "below" is
   less than `n`, even though its lower limbs are all `ffffffff`. It is not
   zero, so it is a key.

That comparison stops early, so its time depends on where the first
difference is. `sc_overflow` (chapter 8) computes the same answer without
stopping: it subtracts `n` limb by limb from limb 0 up, carrying a borrow,
and looks only at the final borrow. Each step computes
`2^32 + a[i] - n[i] - borrow`; the new borrow is 1 when that is below
`2^32`.

For "below":

| Limb | `a[i]` | `n[i]` | Difference | Borrow out |
| --- | --- | --- | --- | --- |
| 0 | `ffffffff` | `d0364141` | `2fc9bebe` | 0 |
| 1 | `ffffffff` | `bfd25e8c` | `402da173` | 0 |
| 2 | `af48a03a` | `af48a03b` | `ffffffff` | 1 |
| 3 to 7 | equal | equal | `ffffffff` | 1 |

The borrow out of limb 7 is 1, so `a - n` is negative: `a < n`.

For "above":

| Limb | `a[i]` | `n[i]` | Difference | Borrow out |
| --- | --- | --- | --- | --- |
| 0 | `00000000` | `d0364141` | `2fc9bebf` | 1 |
| 1 | `00000000` | `bfd25e8c` | `402da173` | 1 |
| 2 | `af48a03c` | `af48a03b` | `00000000` | 0 |
| 3 to 7 | equal | equal | `00000000` | 0 |

The borrow is 0, so `a ≥ n` and `sc_load` returns `ok` false. Limb 2
decides both values, as it did by hand, but every limb is visited.

## The public key

The public key is `P = k·G`, computed with the fixed-base comb of chapter 10.
Nostr and BIP-340 publish only P's x coordinate, as 32 big-endian bytes: the
x-only public key. Of the two points with that x, BIP-340 picks the one with
even y, and a signer whose `P` has odd y signs with `n - k` instead
(chapter 13). For the key `k = 3`, BIP-340 vector 0, the x-only public key is

```jai
f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
```

## Asking a signer

The rest of the stack never needs the key. A page that wants an event signed
records a request in `keys.coax` and accepts only a reply that is exactly the
requested event, signed. The same request works for three signers:

- `SIGNER_LOCAL`: a key held in memory by `SignerLocal`, for tests and
  explicitly temporary use. It is never stored.
- `SIGNER_NIP07`: a browser extension through `window.nostr.signEvent`. The
  host's `coaxial_nip07_sign` passes it `signer_template`'s JSON.
- `SIGNER_PHONE`: a phone over NIP-46 (chapter 48).

A request names its target, its scope, the identity that must sign and the
whole body: kind, `created_at`, tags and content. From those `signer_ask`
computes the NIP-01 event ID that the reply must have:

```jai
id = sha256([0, identity, created_at, kind, tags, content])
```

The scope limits the kind. `SCOPE_RELAY_AUTH` allows only kind 22242
(NIP-42), `SCOPE_HTTP_AUTH` only kind 27235 (NIP-98), and `SCOPE_EVENT` any
other kind, so an ordinary request can't be used to sign an auth event.

A request moves through these states:

| State | Meaning |
| --- | --- |
| `SLOT_ASKED` | Recorded. Nothing has been sent to a signer. |
| `SLOT_APPROVED` | The person approved it on the page, so it may be sent. |
| `SLOT_SIGNED` | A verified reply was accepted. Final. |
| `SLOT_DENIED` | The person or the signer said no. Final. |
| `SLOT_CANCELLED` | The page withdrew it. Final. |
| `SLOT_EXPIRED` | Its deadline, at most 120 seconds after the request, passed. Final. |

`signer_reply` checks a reply in this order, and returns the first failure:

1. The request: `SR_UNKNOWN` (no such ID), `SR_DUPLICATE` (already signed),
   `SR_LATE` (expired, or the deadline has passed now), `SR_CLOSED` (denied
   or cancelled), `SR_UNAPPROVED` (not approved yet).
2. The event: `SR_MALFORMED` if it is not a NIP-01 event, and `SR_TAMPERED`
   if its `id` is not the hash of its own fields.
3. The identity: `SR_IDENTITY` if its `pubkey` is not the requested one.
4. The body: `SR_BODY` if its `id` is not the expected one. With the
   identity already checked, this means another kind, time, tags or content.
5. The signature: `SR_SIGNATURE` if the BIP-340 signature doesn't verify.

Steps 2 to 4 cost a hash and some comparisons, so a wrong reply is refused
before any curve arithmetic. A refused reply leaves the request
`SLOT_APPROVED`, so the right reply can still arrive before the deadline.
Only an accepted one makes it `SLOT_SIGNED`.

The local slot goes through the same check: `signer_local_step` signs with
`nostr_event_sign_step` and then passes its own output to `signer_reply`.
A local key that is not the requested identity is refused like any other
signer's.

## The example

The example is [nostr/examples/keys.coax](../examples/keys.coax). It
classifies five 32-byte values, draws a key from a test host whose first
three draws are refused, derives its x-only public key, and shows three
hosts that fail. It then asks for a signature for that identity, sends
six wrong replies and the right one, and signs the same request in the
local slot.

The drawn key is BIP-340 test vector 1's secret, a published test key. The
example prints whether the draw equals it, not the key itself. Real keys
come from the host and are never printed.

<!-- source: nostr/examples/keys.coax example -->
```jai
#load "../../coax/lib/crypto/entropy.coax";
#load "../../coax/lib/crypto/secp_mult_base_native.coax";
#load "../../coax/lib/nostr/keys.coax";
#import "Basic";

// A test host. It hands out fixed 32-byte draws in order, through the same
// completion policy (entropy_finish) that the Mac, Linux, iOS and browser
// hosts use. A real host fills scratch from the OS; this one copies the next
// fixed draw, or zeros once the list runs out. written is what the host
// reports, so a test can make it short.
HOST_DRAWS :: 8;
TestHost :: struct {
  pool: [256] u8;   // HOST_DRAWS draws of 32 bytes
  count: int;       // how many draws the list holds
  next: int;        // the next draw to hand out
  available: bool = true;
  written: int = 32;
}
// Adds one draw, given as 64 hex digits. A full list or bad hex adds nothing.
host_add :: (h: [] TestHost, text: string) {
  b: [32] u8;
  if h[0].count >= HOST_DRAWS || hex_decode_checked(text, b[..], false).error != 0 { return; }
  for i: 0..<32 { h[0].pool[32 * h[0].count + i] = b[i]; }
  h[0].count += 1;
}
// One 32-byte request. Returns an ENTROPY_ code; on any error out is untouched.
host_fill :: (h: [] TestHost, out: [] u8) -> u32 {
  scratch: [32] u8;
  if h[0].next < h[0].count {
    for i: 0..<32 { scratch[i] = h[0].pool[32 * h[0].next + i]; }
  }
  h[0].next += 1;
  r := entropy_finish(32, out, scratch[..], true, false, h[0].available, true, h[0].written);
  return r.error;
}

// Key generation: draw 32 bytes, keep them only if 0 < k < n, else draw
// again. A bounded number of draws turns a broken host (one that returns
// zeros forever) into a refusal instead of a hang.
KEY_OK :: 0; KEY_ENTROPY :: 1; KEY_EXHAUSTED :: 2;
KEY_MAX_DRAWS :: 8;
KeyDraw :: struct { status: int = KEY_OK; draws: int; entropy: u32; }
key_valid :: (bytes: [] u8) -> bool {
  l := sc_load(bytes);   // ok is false when the value is n or more
  return l.ok && !sc_is_zero(l.value);
}
draw_key :: (h: [] TestHost, key: [] u8) -> KeyDraw {
  r: KeyDraw;
  for d: 0..<KEY_MAX_DRAWS {
    r.draws = d + 1;
    r.entropy = host_fill(h, key);
    if r.entropy != ENTROPY_OK { for i: 0..<key.count { key[i] = 0; } r.status = KEY_ENTROPY; return r; }
    if key_valid(key) { return r; }
  }
  for i: 0..<key.count { key[i] = 0; }
  r.status = KEY_EXHAUSTED; return r;
}
// The x-only public key: the x coordinate of k·G, as 32 bytes.
derive_public_key :: (key: [] u8, out: [] u8) -> bool {
  l := sc_load(key);
  if !l.ok || sc_is_zero(l.value) { return false; }
  // For 0 < k < n, k·G is a finite point, so its affine x always exists.
  a := sg_to_affine(smb_multiply_native(l.value));
  return sf_store(a.value.x, out);
}

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [256] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Say why one 32-byte value is or is not a secret key.
classify :: (label: string, text: string) {
  b: [32] u8;
  #assert hex_decode_checked(text, b[..], false).error == 0;
  l := sc_load(b[..]);
  why := ifx !l.ok then "n or more, refused" else ifx sc_is_zero(l.value) then "zero, refused" else "valid";
  print("%: %\n", label, why);
}
SR_NAMES :: [16] string.["OK", "INPUT", "SCOPE", "FULL", "UNKNOWN", "UNAPPROVED", "DUPLICATE", "LATE",
                         "CLOSED", "MALFORMED", "TAMPERED", "IDENTITY", "BODY", "SIGNATURE", "TARGET", "STATE"];
show_reply :: (label: string, status: int) { print("%: %\n", label, SR_NAMES[status]); }
KEY_NAMES :: [3] string.["KEY_OK", "KEY_ENTROPY", "KEY_EXHAUSTED"];
ENTROPY_NAMES :: [8] string.["OK", "COUNT", "CAPACITY", "SPAN", "ALIAS", "UNAVAILABLE", "FAILED", "SHORT"];
show_draw :: (label: string, d: KeyDraw) {
  print("%: %, draws made %", label, KEY_NAMES[d.status], d.draws);
  if d.status == KEY_ENTROPY { print(", ENTROPY_%", ENTROPY_NAMES[d.entropy]); }
  print("\n");
}

ZERO      :: "0000000000000000000000000000000000000000000000000000000000000000";
N_HEX     :: "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141";
N_MINUS_1 :: "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140";
ABOVE     :: "fffffffffffffffffffffffffffffffebaaedce6af48a03c0000000000000000";
BELOW     :: "fffffffffffffffffffffffffffffffebaaedce6af48a03affffffffffffffff";
// BIP-340 test vector 1's secret key: a published test key, never a real one.
VECTOR_1  :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";

main :: () {
  // Which 32-byte values are secret keys.
  classify("0", ZERO);
  classify("n", N_HEX);
  classify("n - 1", N_MINUS_1);
  classify("above (limb 2 is af48a03c)", ABOVE);
  classify("below (limb 2 is af48a03a)", BELOW);

  // Draw a key from a host whose first three draws are refused.
  h: [1] TestHost;
  host_add(h[..], ZERO); host_add(h[..], N_HEX); host_add(h[..], ABOVE); host_add(h[..], VECTOR_1);
  key: [32] u8;
  d := draw_key(h[..], key[..]);
  show_draw("draw", d);
  vector: [32] u8;
  #assert hex_decode_checked(VECTOR_1, vector[..], false).error == 0;
  same := true; for i: 0..<32 { if key[i] != vector[i] { same = false; } }
  print("the key is BIP-340 vector 1's secret: %\n", same);
  pub: [32] u8;
  print("public key derived: %\n", derive_public_key(key[..], pub[..]));
  show("x-only public key", pub[..]);

  // Hosts that fail.
  bad: [32] u8;
  zeros: [1] TestHost;
  d = draw_key(zeros[..], bad[..]);
  show_draw("host that returns only zeros", d);
  wiped := true; for b: bad { if b != 0 { wiped = false; } }
  print("key buffer left zero: %\n", wiped);
  short: [1] TestHost; short[0].written = 31; host_add(short[..], VECTOR_1);
  d = draw_key(short[..], bad[..]);
  show_draw("host that writes 31 of 32 bytes", d);
  gone: [1] TestHost; gone[0].available = false;
  d = draw_key(gone[..], bad[..]);
  show_draw("host with no source", d);

  // A signer request for the drawn identity and the vector's body.
  t: [1] SignerTable; scratch: [1024] u8;
  tags := ES_TAGS; content := ES_CONTENT;
  a := signer_ask(t[..], SIGNER_NIP07, SCOPE_EVENT, pub[..], ES_KIND, ES_CREATED,
                  cast(string) tags[..], cast(string) content[..], ES_CREATED, 60, scratch[..]);
  print("ask: % id %\n", SR_NAMES[a.status], a.id);
  show("expected event id", t[0].slots[0].expect[..]);
  good := ES_EVENT;
  show_reply("reply before approval", signer_reply(t[..], a.id, cast(string) good[..], ES_CREATED, scratch[..]));
  show_reply("approve", signer_approve(t[..], a.id, ES_CREATED));

  // Refused replies. None of them closes the request.
  show_reply("reply {}", signer_reply(t[..], a.id, "{}", ES_CREATED, scratch[..]));
  tampered := ES_EVENT; c := ne_parse(cast(string) tampered[..]).content_from;
  tampered[c] = ifx tampered[c] == 'l' then 'L' else 'l';
  show_reply("reply with one content byte changed", signer_reply(t[..], a.id, cast(string) tampered[..], ES_CREATED, scratch[..]));
  forged := ES_EVENT; last := forged.count - 3;   // the signature's last hex digit
  forged[last] = ifx forged[last] == '0' then '1' else '0';
  show_reply("reply with one signature digit changed", signer_reply(t[..], a.id, cast(string) forged[..], ES_CREATED, scratch[..]));
  other := signer_ask(t[..], SIGNER_NIP07, SCOPE_EVENT, pub[..], ES_KIND, ES_CREATED,
                      cast(string) tags[..], "other", ES_CREATED, 60, scratch[..]).id;
  #assert signer_approve(t[..], other, ES_CREATED) == SR_OK;
  show_reply("reply to a request for other content", signer_reply(t[..], other, cast(string) good[..], ES_CREATED, scratch[..]));
  g: [32] u8;   // x of G, the public key of the secret 1
  #assert hex_decode_checked("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", g[..], false).error == 0;
  stranger := signer_ask(t[..], SIGNER_NIP07, SCOPE_EVENT, g[..], ES_KIND, ES_CREATED,
                         cast(string) tags[..], cast(string) content[..], ES_CREATED, 60, scratch[..]).id;
  #assert signer_approve(t[..], stranger, ES_CREATED) == SR_OK;
  show_reply("reply to a request for another key", signer_reply(t[..], stranger, cast(string) good[..], ES_CREATED, scratch[..]));
  print("request still approved: %\n", signer_state(t[..], a.id) == SLOT_APPROVED);

  // The right reply, then the same reply again.
  show_reply("reply with the vector event", signer_reply(t[..], a.id, cast(string) good[..], ES_CREATED, scratch[..]));
  show_reply("the same reply again", signer_reply(t[..], a.id, cast(string) good[..], ES_CREATED, scratch[..]));

  // The local slot signs with the drawn key; its output goes through the
  // same reply check.
  local: [1] SignerLocal; aux := ES_AUX; out: [1024] u8;
  #assert signer_local_load(local[..], key[..], aux[..]) == SR_OK;
  l := signer_ask(t[..], SIGNER_LOCAL, SCOPE_EVENT, pub[..], ES_KIND, ES_CREATED,
                  cast(string) tags[..], cast(string) content[..], ES_CREATED, 60, scratch[..]).id;
  #assert signer_approve(t[..], l, ES_CREATED) == SR_OK;
  n := 0; steps := 0;
  while n == 0 && steps < 100 {
    n = signer_local_step(t[..], local[..], l, cast(string) tags[..], cast(string) content[..], ES_CREATED, scratch[..], out[..]);
    steps += 1;
  }
  signer_local_forget(local[..]);
  print("local signer: % bytes, request signed %\n", n, signer_state(t[..], l) == SLOT_SIGNED);
  equal := n == good.count; for i: 0..<good.count { if equal && out[i] != good[i] { equal = false; } }
  print("local event equals the vector event: %\n", equal);
}
```

It prints:

```jai
0: zero, refused
n: n or more, refused
n - 1: valid
above (limb 2 is af48a03c): n or more, refused
below (limb 2 is af48a03a): valid
draw: KEY_OK, draws made 4
the key is BIP-340 vector 1's secret: true
public key derived: true
x-only public key: dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
host that returns only zeros: KEY_EXHAUSTED, draws made 8
key buffer left zero: true
host that writes 31 of 32 bytes: KEY_ENTROPY, draws made 1, ENTROPY_SHORT
host with no source: KEY_ENTROPY, draws made 1, ENTROPY_UNAVAILABLE
ask: OK id 1
expected event id: c2a51f7bfd1bddf835bcd3e1a0c9157eba6aa1234c322d93384e1f9446772c87
reply before approval: UNAPPROVED
approve: OK
reply {}: MALFORMED
reply with one content byte changed: TAMPERED
reply with one signature digit changed: SIGNATURE
reply to a request for other content: BODY
reply to a request for another key: IDENTITY
request still approved: true
reply with the vector event: OK
the same reply again: DUPLICATE
local signer: 589 bytes, request signed true
local event equals the vector event: true
```

The draw takes 4 draws: 0, `n` and "above" are refused, and the fourth is
kept. Its public key `dff1d7…a659` is vector 1's published key, and the
event ID `c2a51f…2c87` is the one the pure-Python signer computed for the
test event.

The refusals:

- A host that returns only zeros is refused after 8 draws with
  `KEY_EXHAUSTED`, and the key buffer is zero.
- A host that reports 31 of 32 bytes is refused on its first draw with
  `ENTROPY_SHORT`, and a host with no source with `ENTROPY_UNAVAILABLE`.
- A reply before approval is `UNAPPROVED`.
- `{}` is `MALFORMED`.
- Changing one byte of the content makes the event's `id` wrong for its
  fields: `TAMPERED`.
- Changing the signature's last hex digit leaves the `id` and `pubkey`
  right, so only the signature check catches it: `SIGNATURE`.
- The same event sent to a request for other content is `BODY`, and to a
  request for the key `x(G)` is `IDENTITY`.
- After all of that, the request is still approved. The right event is
  `OK`, and the same event again is `DUPLICATE`.

The local slot, given the drawn key and vector 1's auxiliary bytes, writes
the same 589 bytes as the pure-Python event, signature included, because
BIP-340 signing is deterministic for a fixed key, message and auxiliary
input.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/keys.out](../fixtures/keys.out).

## How the code is checked

### Checks in the example

The checks are in [nostr/examples/checks/keys.coax](../examples/checks/keys.coax),
which loads the example, so running the example doesn't run them. They
compare the key draw with independent values:

- the BIP-340 vectors 0 to 3: each secret, drawn from a test host, is kept on
  the first draw and gives the published public key;
- hand-derived boundaries: `n` minus 1 in any limb is a key, `n` plus 1 in
  limbs 0 to 3 is not, and 0, 1, `n - 1`, `n`, `n + 1` and `2^256 - 1` fall
  on the right side;
- 0 to 7 refused draws are each retried in order and the next valid one is
  kept, and 8 refused draws are `KEY_EXHAUSTED` with the key zeroed;
- a host that reports any count from 0 to 31 bytes is `ENTROPY_SHORT` on the
  first draw, and a host with no source is `ENTROPY_UNAVAILABLE`; both leave
  the key zero;
- the test host adds at most 8 draws, ignores bad hex, and hands its draws
  out once, in order, then zeros;
- the local slot holding the secret 3, asked to sign for vector 1's identity,
  produces a valid signature for the wrong key, which `signer_reply` refuses
  as `SR_IDENTITY`, wiping the output and leaving the request approved.

<!-- source: nostr/examples/checks/keys.coax check -->
```jai
// The BIP-340 test vectors 0 to 3 as (secret key, public key x). Each secret,
// drawn from a test host, is accepted on the first draw and gives the
// published x.
BIP340 :: [8] string.[
    "0000000000000000000000000000000000000000000000000000000000000003", "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
    "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef", "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
    "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9", "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8",
    "0b432b2677937381aef05bb02a66ecd012773062cf3fa2549e44f58ed2401710", "25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517"];
#forall "a drawn BIP-340 secret gives the published public key" i: 0..<4 #steps 1000000000 {
  h: [1] TestHost; host_add(h[..], BIP340[2 * i]);
  key: [32] u8; pub: [32] u8; want: [32] u8;
  d := draw_key(h[..], key[..]);
  #assert d.status == KEY_OK && d.draws == 1;
  #assert derive_public_key(key[..], pub[..]);
  #assert hex_decode_checked(BIP340[2 * i + 1], want[..], false).error == 0;
  for j: 0..<32 { #assert pub[j] == want[j]; }
}
// Hand-derived boundaries: n's limbs, big-endian, are ffffffff ffffffff
// ffffffff fffffffe baaedce6 af48a03b bfd25e8c d0364141. n minus one in
// limb i is a valid key; n plus one in limb i (limbs 0 to 3, which are not
// ffffffff) is not. 0, n, n + 1 and 2^256 - 1 are refused; 1 and n - 1 are
// accepted.
N_LIMBS :: [8] u32.[0xd0364141, 0xbfd25e8c, 0xaf48a03b, 0xbaaedce6, 0xfffffffe, 0xffffffff, 0xffffffff, 0xffffffff];
bytes_of :: (limbs: [8] u32, out: [] u8) {
  for i: 0..<8 { for j: 0..<4 { out[31 - 4 * i - j] = cast,trunc(u8)(limbs[i] >> (8 * j)); } }
}
#forall "one below n in any limb is a key; one above n is not" i: 0..<8 {
  limbs := N_LIMBS; b: [32] u8;
  limbs[i] -= 1; bytes_of(limbs, b[..]);
  #assert key_valid(b[..]);
  if i < 4 { limbs[i] += 2; bytes_of(limbs, b[..]); #assert !key_valid(b[..]); }
}
#test "the edges of the key range" {
  b: [32] u8;
  #assert !key_valid(b[..]);
  b[31] = 1; #assert key_valid(b[..]);
  bytes_of(N_LIMBS, b[..]); #assert !key_valid(b[..]);
  b[31] = 0x42; #assert !key_valid(b[..]);
  b[31] = 0x40; #assert key_valid(b[..]);
  for i: 0..<32 { b[i] = 0xff; } #assert !key_valid(b[..]);
  #assert !key_valid(b[0..<31]);
}
// Each refused draw costs one draw; the first valid one is kept.
#forall "refused draws are retried in order" refused: 0..<9 {
  h: [1] TestHost;
  bad := [3] string.[ZERO, N_HEX, "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"];
  for r: 0..<refused { host_add(h[..], bad[r % 3]); }
  if refused < KEY_MAX_DRAWS { host_add(h[..], N_MINUS_1); }
  key: [32] u8;
  d := draw_key(h[..], key[..]);
  if refused < KEY_MAX_DRAWS {
    want: [32] u8; #assert hex_decode_checked(N_MINUS_1, want[..], false).error == 0;
    #assert d.status == KEY_OK && d.draws == refused + 1;
    for j: 0..<32 { #assert key[j] == want[j]; }
  } else {
    #assert d.status == KEY_EXHAUSTED && d.draws == KEY_MAX_DRAWS;
    for b: key { #assert b == 0; }
  }
}
// A failing host refuses on the draw it fails, and the key stays zero.
#forall "a host failure stops the draw" written: 0..<32 {
  h: [1] TestHost; h[0].written = written; host_add(h[..], BIP340[2]);
  key: [32] u8; for j: 0..<32 { key[j] = 0x5a; }
  d := draw_key(h[..], key[..]);
  #assert d.status == KEY_ENTROPY && d.draws == 1 && d.entropy == ENTROPY_SHORT;
  for b: key { #assert b == 0; }
}
#test "a host with no source refuses" {
  h: [1] TestHost; h[0].available = false; host_add(h[..], BIP340[2]);
  key: [32] u8;
  d := draw_key(h[..], key[..]);
  #assert d.status == KEY_ENTROPY && d.entropy == ENTROPY_UNAVAILABLE;
  pub: [32] u8; #assert !derive_public_key(key[..], pub[..]);
}
// The test host itself: at most 8 draws, bad hex is not a draw, and each
// draw is handed out once, in order.
#test "the test host hands out its list in order, then zeros" {
  h: [1] TestHost;
  host_add(h[..], "not hex");
  #assert h[0].count == 0;
  host_add(h[..], "abc");
  #assert h[0].count == 0;
  for i: 0..<HOST_DRAWS + 1 { host_add(h[..], BIP340[2 * (i % 4)]); }
  #assert h[0].count == HOST_DRAWS;
  out: [32] u8; want: [32] u8;
  for i: 0..<HOST_DRAWS {
    #assert host_fill(h[..], out[..]) == ENTROPY_OK;
    #assert hex_decode_checked(BIP340[2 * (i % 4)], want[..], false).error == 0;
    for j: 0..<32 { #assert out[j] == want[j]; }
  }
  #assert host_fill(h[..], out[..]) == ENTROPY_OK;
  for b: out { #assert b == 0; }
}
// The local slot with the wrong key: vector 0's secret (3) signs a request
// for vector 1's identity. The signature is valid, but for another pubkey,
// so the reply check refuses it and the request stays approved.
#test "a local key that is not the requested identity is refused" #steps 1000000000 {
  t: [1] SignerTable; scratch: [1024] u8; out: [1024] u8; local: [1] SignerLocal;
  tags := ES_TAGS; content := ES_CONTENT; pub := ES_PUB; aux := ES_AUX;
  three: [32] u8; three[31] = 3;
  #assert signer_local_load(local[..], three[..], aux[..]) == SR_OK;
  id := signer_ask(t[..], SIGNER_LOCAL, SCOPE_EVENT, pub[..], ES_KIND, ES_CREATED,
                   cast(string) tags[..], cast(string) content[..], ES_CREATED, 60, scratch[..]).id;
  #assert signer_approve(t[..], id, ES_CREATED) == SR_OK;
  n := 0; steps := 0;
  while n == 0 && steps < 100 {
    n = signer_local_step(t[..], local[..], id, cast(string) tags[..], cast(string) content[..], ES_CREATED, scratch[..], out[..]);
    steps += 1;
  }
  #assert n == -SR_IDENTITY && signer_state(t[..], id) == SLOT_APPROVED;
  for b: out { #assert b == 0; }
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/keys.coax --run-steps 250000000
```

`coax test` runs 353 checks, including the loaded files' own checks, in
about 45 seconds. Root-only mutation of the example and these checks
together (the example's procedures are the entry source) killed 79 of 79
mutants on 2026-11-01. Its only other finding is that the four printing
procedures (`show`, `classify`, `show_reply`, `show_draw`) are reached by
no check; the fixture comparison covers them. A first run left six
survivors: four guards in the test host, a redundant `KEY_OK` assignment
and a redundant `a.ok` test. They became the test host check above, a
default status and a comment.

### The library's own checks

- [entropy.coax](../../coax/lib/crypto/entropy.coax) checks every admission
  boundary and every completion failure, including that a failed draw leaves
  the output untouched and the scratch prefix zeroed.
- [keys.coax](../../coax/lib/nostr/keys.coax) checks the scopes, the
  lifecycle, table reuse, and the malformed, tampered, other-identity and
  other-body replies.
  [signer_checks.coax](../../coax/tests/nostr/signer_checks.coax) checks
  them against the pure-Python signer's event, with real signatures.

### On each host

[coax-entropy-2026-10-28.md](../../docs/verification/coax-entropy-2026-10-28.md)
records the entropy contract on each host: all 57 mutants of `entropy.coax`
killed; macOS `SecRandomCopyBytes` and a test provider under ASan and UBSan
for requests of 0 to 1,048,576 bytes; the same executables run in the iOS
26.5 simulator; and 21 cases in Chrome 156 with real WebCrypto.
[coax-browser-random-2026-10-28.md](../../docs/verification/coax-browser-random-2026-10-28.md)
records the browser host's `coaxial_random` import.

Those records left the Linux `getrandom` branch unexecuted. On 2026-11-01
it ran on x86_64 Linux 6.18 with GCC 15.2 under ASan and UBSan:

- `coax/tests/entropy_native.c`, linked with the real `entropy-native.c`,
  passed its requests of 0 to 1,048,576 bytes and its boundary cases, and
  the test-provider build passed its failure cases. The production object's
  only random-source symbol is `getrandom`.
- A small harness drew 10,000 keys of 32 bytes through
  `coax_entropy_native` and checked each with the Coax-generated `sc_load`
  and `sc_is_zero`: all 10,000 were valid scalars, and the scratch buffer
  was zero after every draw. It prints no key bytes.

`coax/tests/entropy.py` runs the first of these on any host; its iOS and
Chrome cases need macOS and Chrome. They were not rerun today. Safari,
Firefox and a physical phone remain later checks.

## Summary

- The host fills a buffer from the OS or fails. `entropy_finish` releases
  bytes only on full success, and nothing falls back to a weaker source.
- A key is 32 drawn bytes with `0 < k < n`. Anything else is refused and
  drawn again, at most 8 times.
- `sc_load` decides `k < n` from the final borrow of `k - n`, visiting every
  limb.
- The x-only public key is the x coordinate of `k·G`.
- A signer request fixes the event ID its reply must have. A reply with the
  wrong ID, pubkey, body or signature is refused, and the request stays open
  for the right one.

## Dependencies

- [8. Scalars mod n](ch008-scalars-mod-n.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)

## Sources

- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
  and its pinned
  [test-vectors.csv](../../coax/tests/vectors/upstream/bip340/test-vectors.csv),
  for the keys and public keys.
- [NIP-01](../nips/official/01.md), for the event ID, and
  [NIP-07](../nips/official/07.md), for the browser signer.
- [Linux getrandom](https://man7.org/linux/man-pages/man2/getrandom.2.html),
  [Apple SecRandomCopyBytes](https://developer.apple.com/documentation/security/secrandomcopybytes(_:_:_:))
  and [Web Cryptography getRandomValues](https://www.w3.org/TR/webcrypto/#Crypto-method-getRandomValues).
- Primary sources are pinned in
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
