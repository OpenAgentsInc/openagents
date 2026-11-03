# 107. Shachain

A Lightning channel replaces its commitment transaction every time a
payment moves. To make an old commitment unsafe to broadcast, each side
hands the other a secret for it when it is replaced. A long-lived channel
revokes millions of commitments, so both sides need a compact way to
make and keep these secrets.

BOLT 3's answer is shachain. One 32-byte seed gives 2^48 per-commitment
secrets, by flipping bits and hashing. The peer that receives them keeps
every one it has seen in 49 slots, and checks each new secret against
the ones before it. This chapter builds the generator and the store.

## What this chapter teaches

- How one seed gives 2^48 secrets: for each set bit of the index, from
  the top, flip that bit of the value and hash it.
- Why secrets are used from index 2^48 - 1 down: a secret then derives
  every secret received before it that shares its prefix.
- The receiver's 49 slots, one per count of trailing zero bits, and why
  49 slots hold any number of secrets.
- The insert check: a new secret must derive the secrets in the slots
  below its own, or the peer did not use one seed.
- Looking up any secret received so far.

## Where Lightning uses shachain

| Where | What it uses | Chapter |
| --- | --- | --- |
| Per-commitment points | each secret times G is that commitment's point | 106 |
| Revocation | `revoke_and_ack` carries the secret of the replaced commitment | 109 |
| Penalties | a revoked commitment on chain is spent with its secret's key | 109 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [shachain.coax](../../coax/lib/lightning/shachain.coax) | `shachain_generate(seed, index, out)` | the sender's secret for an index |
| | `shachain_derive(base, bits, index, out)` | the low `bits` bits of an index from a prefix's secret |
| | `shachain_insert(store, index, secret)` | the receiver's checked insert |
| | `shachain_lookup(store, index, out)` | any secret received so far |
| | `shachain_slot`, `shachain_used` | where a secret goes, and which slots hold one |
| [sha256.coax](../../coax/lib/crypto/sha256.coax) | `sha256_hash_checked` | SHA-256 (chapter 4) |

The seed is secret, and so is every secret until its commitment is
revoked. The receiver's slots hold secrets the peer has revealed only to
it. SHA-256 runs in constant time, the insert compares secrets in
constant time, and each procedure overwrites the copies it leaves on the
stack. An index is a commitment number, which both peers know, so the
code branches on its bits.

The library's codes and sizes:

<!-- source: coax/lib/lightning/shachain.coax codes -->
```jai
SHACHAIN_OK :: 0;
SHACHAIN_WRONG_SECRET :: 1;  // the secret does not derive one already stored: another seed
SHACHAIN_ORDER :: 2;         // not the next index: secrets arrive one by one, counting down
SHACHAIN_RANGE :: 3;         // an index of 2^48 or more
SHACHAIN_UNKNOWN :: 4;       // an index whose secret has not arrived yet

SHACHAIN_BITS :: 48;
SHACHAIN_MAX_INDEX :: cast(u64) 281474976710655;  // 2^48 - 1, the first secret used
SHACHAIN_SLOTS :: 49;                             // one per count of trailing zero bits, 0 to 48
```

## Making the secrets

The secret for index I starts as the seed. For each bit B of I from 47
down to 0, if B is set, flip bit B of the value and replace the value with
its SHA-256. Bit B is bit B mod 8 of byte B div 8, so flipping bit 0 of
`e3b0...` gives `e2b0...`, and bit 10 gives `e3b4...`.

The same loop, started from the secret of a prefix and run over only the
low bits, is `derive`. If index P is I with its low b bits cleared, P's
secret has already flipped and hashed every bit above b exactly as I's
would, so `derive(secret(P), b, I)` is I's secret. Going the other way
needs a preimage of SHA-256, so a secret reveals nothing about indices
with a shorter prefix.

<!-- source: coax/lib/lightning/shachain.coax generate -->
```jai
// derive(base, bits, I): for each bit B of I below `bits`, from the top,
// flip bit B of P and hash. Bit B is bit B mod 8 of byte B div 8.
shachain_derive :: (base: [] u8, bits: int, index: u64, out: [] u8) {
  p: [32] u8; next: [32] u8;
  for i: 0..<32 { p[i] = base[i]; }
  for k: 0..<bits {
    b := bits - 1 - k;
    if ((index >> cast(u64) b) & 1) == 1 {
      p[b / 8] ^= cast(u8) (1 << (b % 8));
      #assert sha256_hash_checked(p[..], next[..], false) == SHA_OK;
      p = next;
    }
  }
  for i: 0..<32 { out[i] = p[i]; }
  shachain_wipe(p[..]); shachain_wipe(next[..]);
}
// generate_from_seed(seed, I): every one of the 48 bits.
shachain_generate :: (seed: [] u8, index: u64, out: [] u8) -> int {
  if index > SHACHAIN_MAX_INDEX { return SHACHAIN_RANGE; }
  shachain_derive(seed, SHACHAIN_BITS, index, out);
  return SHACHAIN_OK;
}
```

The first commitment uses index 2^48 - 1, all ones, and each commitment
after it uses the index one lower. Commitment n uses 2^48 - 1 - n. Counting
down means each new index has fewer one bits at the bottom, so its secret
can derive the indices received just before it.

## The receiver's store

The receiver gets secrets in descending order and wants to keep all of
them. It needs only one secret per prefix. An index that ends in exactly
b zero bits goes in slot b. That secret derives the 2^b - 1 indices above
it with the same prefix, all of which arrived before it, so it replaces
the secrets in the slots below. Index 0 ends in 48 zero bits and goes in slot
48, which makes 49 slots.

<!-- source: coax/lib/lightning/shachain.coax store -->
```jai
// The receiver's store. Slot b holds the last secret received whose
// index ends in exactly b zero bits; from it, every index that shares its
// top 48 - b bits can be derived.
ShachainSlot :: struct { index: u64; secret: [32] u8; }
Shachain :: struct { slots: [49] ShachainSlot; next: u64; received: u64; }
shachain_new :: () -> Shachain { return Shachain.{next = SHACHAIN_MAX_INDEX}; }

// The slot an index goes in: its count of trailing zero bits, 48 for 0.
shachain_slot :: (index: u64) -> int {
  for b: 0..<SHACHAIN_BITS { if ((index >> cast(u64) b) & 1) == 1 { return b; } }
  return SHACHAIN_BITS;
}
// Whether slot b holds a secret. An empty slot's index is 0, which
// belongs in slot 48, so only slot 48 needs the count.
shachain_used :: (store: [] Shachain, b: int) -> bool {
  if b == SHACHAIN_BITS { return store[0].received == SHACHAIN_MAX_INDEX + 1; }
  return shachain_slot(store[0].slots[b].index) == b;
}
```

A slot holds one index and one 32-byte secret, so the whole store is 49
times 40 bytes, however many secrets have arrived.

## The insert check

The receiver cannot check a secret on its own, but it can check that the
secrets agree. A new secret for an index in slot b must derive each
secret in slots 0 to b - 1, because those indices share its prefix. If
one does not, the peer's secrets did not come from one seed, and the
receiver refuses the secret and fails the channel.

The store also checks the order. Secrets arrive one at a time, counting
down from 2^48 - 1, so any other index is refused.

<!-- source: coax/lib/lightning/shachain.coax insert -->
```jai
// insert_secret: the new secret must derive every secret in the slots
// below its own. If one does not, the peer's secrets do not come from one
// seed, and the peer is cheating or broken.
shachain_insert :: (store: [] Shachain, index: u64, secret: [] u8) -> int {
  if index > SHACHAIN_MAX_INDEX { return SHACHAIN_RANGE; }
  if store[0].received == SHACHAIN_MAX_INDEX + 1 || index != store[0].next { return SHACHAIN_ORDER; }
  slot := shachain_slot(index);
  derived: [32] u8; same := CB_MAX64;
  for b: 0..<slot {
    shachain_derive(secret, slot, store[0].slots[b].index, derived[..]);
    same &= cb_ct_equal(derived[..], store[0].slots[b].secret[..]);
  }
  shachain_wipe(derived[..]);
  if same != CB_MAX64 { return SHACHAIN_WRONG_SECRET; }
  store[0].slots[slot].index = index;
  for i: 0..<32 { store[0].slots[slot].secret[i] = secret[i]; }
  store[0].received += 1;
  if index > 0 { store[0].next = index - 1; }
  return SHACHAIN_OK;
}
```

A wrong secret in slot 0 has nothing below it to check. It is caught only
when the next secret, in a higher slot, fails to derive it. BOLT 3's
storage vectors test exactly this: each corrupts one secret and the ones
derived from it, and the error comes at the first insert that can see it.

## Looking up a secret

To find the secret of an index that has arrived, the receiver looks for
the slot whose index is a prefix of it: the slot b where the two indices
agree once their low b bits are dropped. Then it derives the low b bits
from that slot's secret.

<!-- source: coax/lib/lightning/shachain.coax lookup -->
```jai
// derive_old_secret: the slot whose index is the top bits of I, then the
// remaining low bits from there. Secrets from the next index up to the
// first received have arrived; lower ones have not.
shachain_lookup :: (store: [] Shachain, index: u64, out: [] u8) -> int {
  if index > SHACHAIN_MAX_INDEX { return SHACHAIN_RANGE; }
  if store[0].received == 0 || index <= SHACHAIN_MAX_INDEX - store[0].received { return SHACHAIN_UNKNOWN; }
  for b: 0..<SHACHAIN_SLOTS {
    // The slot's index, with its b low bits clear, is I's prefix.
    if (index >> cast(u64) b) == (store[0].slots[b].index >> cast(u64) b) && shachain_used(store, b) {
      shachain_derive(store[0].slots[b].secret[..], b, index, out);
      return SHACHAIN_OK;
    }
  }
  return SHACHAIN_UNKNOWN;
}
// The commitment number n uses index 2^48 - 1 - n.
shachain_index :: (commitment: u64) -> u64 { return SHACHAIN_MAX_INDEX - commitment; }
```

## The example

The example,
[bitcoin/examples/lightning-shachain.coax](../examples/lightning-shachain.coax),
runs every generation and storage vector in BOLT 3, shows one flip and
one hash, then keeps the secrets of a 10,000-commitment channel and
catches a peer that sends a wrong one.

<!-- source: bitcoin/examples/lightning-shachain.coax example -->
```jai
#load "../../coax/lib/lightning/shachain.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex.
show :: (b: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
SHACHAIN_CODE_NAMES :: [5] string.["OK", "WRONG_SECRET", "ORDER", "RANGE", "UNKNOWN"];
COMMITMENTS :: 10000;

main :: () {
  // ---- BOLT 3's generation vectors
  print("generation vectors\n");
  seed: [32] u8; out: [32] u8;
  for row: SHACHAIN_GENERATION {
    shc_fill(row.seed, seed[..]);
    code := shachain_generate(seed[..], row.index, out[..]);
    print("  %: index %, matches: %\n", row.name, row.index, shc_hex_is(out[..], row.output));
  }

  // ---- index 1 from the seed 0101...: one flip, one hash
  print("index 1, step by step\n");
  shc_fill(SHACHAIN_GENERATION[4].seed, seed[..]);
  seed[0] ^= 1;
  print("  flip bit 0: "); show(seed[..]); print("\n");
  hashed: [32] u8; done := sha256_hash_checked(seed[..], hashed[..], false);
  print("  SHA-256:    "); show(hashed[..]); print("\n");

  // ---- BOLT 3's storage vectors
  print("storage vectors\n");
  for t: 0..<SHACHAIN_STORAGE_NAMES.count {
    got := shachain_run_storage(t); want := shachain_expected(t);
    if got < 0 { print("  %: every insert OK, BOLT 3 agrees: %\n", SHACHAIN_STORAGE_NAMES[t], want < 0); }
    else { print("  %: insert % refused, BOLT 3 says insert %\n", SHACHAIN_STORAGE_NAMES[t], got + 1, want + 1); }
  }

  // ---- a channel's life: one side reveals a secret per commitment
  print("a channel of % commitments\n", COMMITMENTS);
  for i: 0..<32 { seed[i] = cast(u8) ((7 * i + 1) & 255); }
  store: [1] Shachain; store[0] = shachain_new(); s: [32] u8;
  refused := 0;
  for n: 0..<COMMITMENTS {
    code := shachain_generate(seed[..], shachain_index(cast(u64) n), s[..]);
    if shachain_insert(store[..], shachain_index(cast(u64) n), s[..]) != SHACHAIN_OK { refused += 1; }
  }
  used := 0;
  for b: 0..<SHACHAIN_SLOTS { if shachain_used(store[..], b) { used += 1; } }
  print("  received %, refused %, slots in use %, next index %\n", store[0].received, refused, used, store[0].next);
  for n: [4] u64.[0, 1234, 5000, 9999] {
    code := shachain_lookup(store[..], shachain_index(n), out[..]);
    code = shachain_generate(seed[..], shachain_index(n), s[..]);
    print("  commitment %: lookup %, equals the sender's: %\n", n, SHACHAIN_CODE_NAMES[code], cb_ct_equal(out[..], s[..]) == CB_MAX64);
  }
  print("  commitment %: %\n", COMMITMENTS, SHACHAIN_CODE_NAMES[shachain_lookup(store[..], shachain_index(COMMITMENTS), out[..])]);

  // ---- a peer that cheats, on a copy of the store
  print("a peer that sends a wrong secret\n");
  copy: [1] Shachain; copy[0] = store[0];
  wrong: [32] u8; other: [32] u8; for i: 0..<32 { other[i] = 0xab; }
  code := shachain_generate(other[..], shachain_index(COMMITMENTS), wrong[..]);
  first := shachain_index(COMMITMENTS);
  print("  commitment %, index % in slot %: %\n", COMMITMENTS, first, shachain_slot(first),
        SHACHAIN_CODE_NAMES[shachain_insert(copy[..], first, wrong[..])]);
  code = shachain_generate(seed[..], shachain_index(COMMITMENTS + 1), s[..]);
  second := shachain_index(COMMITMENTS + 1);
  print("  commitment %, index % in slot %: %\n", COMMITMENTS + 1, second, shachain_slot(second),
        SHACHAIN_CODE_NAMES[shachain_insert(copy[..], second, s[..])]);
  copy[0] = store[0];
  code = shachain_generate(seed[..], shachain_index(COMMITMENTS + 1), s[..]);
  print("  commitment % skipped: %\n", COMMITMENTS, SHACHAIN_CODE_NAMES[shachain_insert(copy[..], shachain_index(COMMITMENTS + 1), s[..])]);
  print("  storage: % bytes of slots for % secrets of 32 bytes\n", SHACHAIN_SLOTS * 40, store[0].received);
}
```

From the repository root:

```shell
coax/bin/coax run bitcoin/examples/lightning-shachain.coax
```

The output:

```text
generation vectors
  generate_from_seed 0 final node: index 281474976710655, matches: true
  generate_from_seed FF final node: index 281474976710655, matches: true
  generate_from_seed FF alternate bits 1: index 11728124029610, matches: true
  generate_from_seed FF alternate bits 2: index 93824992236885, matches: true
  generate_from_seed 01 last nontrivial node: index 1, matches: true
index 1, step by step
  flip bit 0: 0001010101010101010101010101010101010101010101010101010101010101
  SHA-256:    915c75942a26bb3a433a8ce2cb0427c29ec6c1775cfc78328b57f6ba7bfeaa9c
storage vectors
  insert_secret correct sequence: every insert OK, BOLT 3 agrees: true
  insert_secret #1 incorrect: insert 2 refused, BOLT 3 says insert 2
  insert_secret #2 incorrect (#1 derived from incorrect): insert 4 refused, BOLT 3 says insert 4
  insert_secret #3 incorrect: insert 4 refused, BOLT 3 says insert 4
  insert_secret #4 incorrect (1,2,3 derived from incorrect): insert 8 refused, BOLT 3 says insert 8
  insert_secret #5 incorrect: insert 6 refused, BOLT 3 says insert 6
  insert_secret #6 incorrect (5 derived from incorrect): insert 8 refused, BOLT 3 says insert 8
  insert_secret #7 incorrect: insert 8 refused, BOLT 3 says insert 8
  insert_secret #8 incorrect: insert 8 refused, BOLT 3 says insert 8
a channel of 10000 commitments
  received 10000, refused 0, slots in use 14, next index 281474976700655
  commitment 0: lookup OK, equals the sender's: true
  commitment 1234: lookup OK, equals the sender's: true
  commitment 5000: lookup OK, equals the sender's: true
  commitment 9999: lookup OK, equals the sender's: true
  commitment 10000: UNKNOWN
a peer that sends a wrong secret
  commitment 10000, index 281474976700655 in slot 0: OK
  commitment 10001, index 281474976700654 in slot 1: WRONG_SECRET
  commitment 10000 skipped: ORDER
  storage: 1960 bytes of slots for 10000 secrets of 32 bytes
```

What the output shows:

- All 5 generation vectors give BOLT 3's secret.
- Index 1 is one flip of bit 0 of the seed and one SHA-256, and gives
  BOLT 3's "last nontrivial node".
- The correct storage sequence accepts all 8 secrets, and each of the 8
  incorrect sequences is refused at the insert BOLT 3 marks as an error.
- A store of 10,000 secrets uses 14 slots: index 2^48 - 1 - n ends in as
  many zero bits as n ends in one bits, at most 13 below 10,000. Any of
  them looks up to the sender's secret, and the next commitment's is
  unknown.
- A wrong secret for commitment 10,000 lands in slot 0 and is accepted.
  The next secret goes in slot 1, cannot derive it, and is refused.
- A skipped commitment is refused for its order.
- The store is 1,960 bytes for any number of secrets.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/lightning-shachain.out](../fixtures/lightning-shachain.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/lightning-shachain.coax](../examples/checks/lightning-shachain.coax),
which loads the example:

- for 64 indices, the secret of a prefix derives the index's secret
- after 15 good secrets, the 16th with one bit changed in any of its 32
  bytes is refused
- all 300 secrets of a 300-commitment store look up to the sender's, in 9
  slots

<!-- source: bitcoin/examples/checks/lightning-shachain.coax check -->
```jai
CHECK_SEED :: "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
// A secret derives every index that shares its prefix: the secret of I
// with its low b bits cleared, taken b bits further, is the secret of I.
#forall "a prefix's secret derives the index" k: 0..<64 #steps 400000000 {
  seed: [32] u8; shc_fill(CHECK_SEED, seed[..]);
  index := SHACHAIN_MAX_INDEX - cast(u64) (k * 4099); b := k % 13;
  prefix := (index >> cast(u64) b) << cast(u64) b;
  base: [32] u8; want: [32] u8; got: [32] u8;
  code := shachain_generate(seed[..], prefix, base[..]);
  code = shachain_generate(seed[..], index, want[..]);
  shachain_derive(base[..], b, index, got[..]);
  #assert cb_ct_equal(got[..], want[..]) == CB_MAX64;
}
// After 15 good secrets, the 16th (index 2^48 - 16, slot 4) with one bit
// changed, in any of its 32 bytes, fails the check against slots 0 to 3.
#forall "one changed bit of a secret that has children" bit: 0..<32 #steps 1200000000 {
  seed: [32] u8; shc_fill(CHECK_SEED, seed[..]); s: [32] u8;
  store: [1] Shachain; store[0] = shachain_new();
  for n: 0..<15 {
    code := shachain_generate(seed[..], shachain_index(cast(u64) n), s[..]);
    #assert shachain_insert(store[..], shachain_index(cast(u64) n), s[..]) == SHACHAIN_OK;
  }
  code := shachain_generate(seed[..], shachain_index(15), s[..]);
  at := bit * 8 + bit % 8; s[at / 8] ^= cast(u8) (1 << (at % 8));
  #assert shachain_slot(shachain_index(15)) == 4;
  #assert shachain_insert(store[..], shachain_index(15), s[..]) == SHACHAIN_WRONG_SECRET;
}
// Every one of 300 received secrets looks up to the sender's secret, and
// the store never uses more than 9 slots for them.
#test "300 commitments: every lookup is the sender's secret" #steps 2000000000 {
  seed: [32] u8; shc_fill(CHECK_SEED, seed[..]); s: [32] u8; got: [32] u8;
  store: [1] Shachain; store[0] = shachain_new();
  for n: 0..<300 {
    code := shachain_generate(seed[..], shachain_index(cast(u64) n), s[..]);
    #assert shachain_insert(store[..], shachain_index(cast(u64) n), s[..]) == SHACHAIN_OK;
  }
  for n: 0..<300 {
    code := shachain_generate(seed[..], shachain_index(cast(u64) n), s[..]);
    #assert shachain_lookup(store[..], shachain_index(cast(u64) n), got[..]) == SHACHAIN_OK;
    #assert cb_ct_equal(got[..], s[..]) == CB_MAX64;
  }
  used := 0; for b: 0..<SHACHAIN_SLOTS { if shachain_used(store[..], b) { used += 1; } }
  #assert used == 9;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/lightning-shachain.coax
```

`coax test` runs 51 checks, including the loaded files' own checks, in
about 70 seconds on 2026-11-03.

### The library's own checks

`shachain.coax` is an `#authority` file. Its checks cover every BOLT 3
generation vector and BOLT 3's two flip examples, every storage vector
refused at BOLT 3's insert, the slot of an index, a 64-secret store
whose every lookup is the sender's secret, and refusals for order,
range, an empty store and an index not yet received.

```shell
coax/bin/coax test coax/lib/lightning/shachain.coax
```

`coax test` runs 45 checks, including those of the files it loads, in
about 7 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### BOLT 3 and an independent store

[shachain.py](../../coax/tests/lightning/shachain.py) computes every line
the example prints with its own Python generator and store, written from
BOLT 3's "Per-commitment Secret Requirements" and "Efficient
Per-commitment Secret Storage" on Python's `hashlib`, sharing no code
with Coax. Every generation vector must give BOLT 3's secret and every
storage vector must refuse at BOLT 3's insert before the transcript is
compared with the fixture.

```shell
python3 -I coax/tests/lightning/shachain.py --coax coax/bin/coax
```

On 2026-11-03 all 31 lines agree, and the example's output in native debug
and release builds equals them. Wasm was not run.

[make_shachain_vectors.py](../../coax/tests/lightning/make_shachain_vectors.py)
reads the pinned BOLT 3 into
[shachain_vectors.coax](../../coax/tests/lightning/shachain_vectors.coax).

## What this chapter leaves out

- Turning a secret into its per-commitment point and the keys of a
  commitment: chapter 106.
- When secrets are sent, in `revoke_and_ack`, and what a node does with a
  refused one: chapter 109.
- Choosing the seed, which comes from chapter 18's randomness or a
  wallet's key derivation.
- Keeping the store on disk across restarts.

## Summary

- One seed gives 2^48 secrets: flip each set bit of the index, from the
  top, and hash.
- Secrets are used from index 2^48 - 1 down, so each new secret can derive
  the ones before it that share its prefix.
- The receiver keeps one secret per count of trailing zero bits: 49 slots
  for any number of secrets.
- Each insert must derive the secrets in the lower slots, which proves the
  peer used one seed.
- A wrong secret is caught at the first insert whose slot is above it.

## Dependencies

- [4. SHA-256](ch004-sha-256.md)

## Sources

- [BOLT 3](https://github.com/lightning/bolts/blob/master/03-transactions.md),
  "Per-commitment Secret Requirements", "Efficient Per-commitment Secret
  Storage" and "Appendix D: Per-commitment Secret Generation Test
  Vectors", at lightning/bolts revision `19146282`, pinned in
  [coax/tests/vectors/upstream/bolts/](../../coax/tests/vectors/upstream/bolts/03-transactions.md).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
