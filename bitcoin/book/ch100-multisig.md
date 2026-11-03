# 100. Multisig

A multisig output needs signatures from m of n keys. A company keeps
its savings under a 2-of-3: one key in the office, one with the
finance lead and one in a bank vault. Losing one key loses nothing,
and stealing one key steals nothing.

Bitcoin has two ways to write m of n. The original
`OP_CHECKMULTISIG` script lists the keys and checks the signatures in
one opcode. It can sit in the output itself (bare multisig), behind a
hash under P2SH, under P2WSH, or under P2WSH nested inside P2SH.
Tapscript replaced `OP_CHECKMULTISIG` with `OP_CHECKSIGADD`, and
BIP-387's `multi_a` builds an m-of-n leaf from it.

This chapter writes both scripts as Bitcoin Core writes them, with
BIP-67's sorted keys, and reads them back as Core's solver reads them.
One 2-of-3 is locked all four classic ways and spent in one transaction
through chapter 94's PSBT roles, which chapter 36's interpreter accepts.
The same three keys then form a `multi_a` leaf, which two of them sign
on the script path.

## What this chapter teaches

- The `OP_CHECKMULTISIG` script: the threshold, the keys, the key count
  and the opcode.
- The four places it can go: bare, P2SH, P2WSH and P2SH-P2WSH, and what
  each one puts in the scriptSig and the witness.
- BIP-67's sorted keys, so that the order the parties list their keys in
  doesn't change the address.
- How Core's solver recognizes a multisig script, and which near misses
  it refuses.
- How two signers spend one coin through a PSBT.
- BIP-387's `multi_a` leaf, its script-path signature hash, and how its
  witness is ordered.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [multisig.coax](../../coax/lib/bitcoin/multisig.coax) | `multisig_script_vartime` | Core's `GetScriptForMultisig`, in the given order or BIP-67's |
| | `multisig_output_vartime` | The scriptPubKey of each of the four kinds |
| | `multisig_match_vartime`, `multisig_a_match_vartime` | Core's `MatchMultisig` and `MatchMultiA` |
| | `multisig_a_script_vartime` | BIP-387's `multi_a` and `sortedmulti_a` leaf |
| | `multisig_a_sighash_vartime`, `multisig_a_sign` | BIP-342's script-path signature hash, and a BIP-340 signature of it |
| | `multisig_a_check_vartime` | A `multi_a` witness, decided as BIP-342's interpreter runs it |
| [psbt.coax](../../coax/lib/bitcoin/psbt.coax) | `psbt_sign_vartime`, `psbt_finalize_vartime`, `psbt_extract_vartime` | Chapter 94's Signer, Input Finalizer and Transaction Extractor |
| [descriptor.coax](../../coax/lib/bitcoin/descriptor.coax) | `ds_multi` | Chapter 30's `multi()` and `sortedmulti()` descriptors |
| [taproot.coax](../../coax/lib/bitcoin/taproot.coax) | `taproot_leaf_hash_vartime`, `taproot_control_block_vartime` | Chapter 42's leaf hash and control block |
| [sighash.coax](../../coax/lib/bitcoin/sighash.coax) | `bt_sighash_taproot_ext` | Chapter 43's BIP-341 signature hash with BIP-342's extension |

## The codes

A builder returns a script length, or one of these codes. The limits
are Core's: at most 20 keys in an `OP_CHECKMULTISIG` and at most 999 in
a `multi_a` leaf. Policy relays bare multisig with at most 3 keys.

<!-- source: coax/lib/bitcoin/multisig.coax codes -->
```jai
// Results: a length, or one of these.
MS_THRESHOLD :: -1;     // m is not from 1 to n
MS_KEY_COUNT :: -2;     // no keys, or more than the kind allows
MS_KEY :: -3;           // a key that doesn't start with 02 or 03
MS_TOO_LONG :: -4;      // a P2SH redeem script over 520 bytes
MS_CAPACITY :: -5;      // the output is too small
MS_MAX_KEYS :: 20;      // MAX_PUBKEYS_PER_MULTISIG
MS_A_MAX_KEYS :: 999;   // MAX_PUBKEYS_PER_MULTI_A
MS_BARE_MAX :: 3;       // policy: a bare multisig output has at most 3 keys
// Where the script goes.
MS_BARE :: 0; MS_P2SH :: 1; MS_P2WSH :: 2; MS_P2SH_P2WSH :: 3;
MS_KINDS :: [4] string.["bare", "P2SH", "P2WSH", "P2SH-P2WSH"];
```

## The script and where it goes

The script is `m <key 1> ... <key n> n OP_CHECKMULTISIG`. A count from
1 to 16 is one opcode, `OP_1` to `OP_16`. Larger counts, such as 17 of
20, are pushed as a minimal script number.

The parties must agree on the key order, because a different order is
a different script and a different address. BIP-67 settles it: sort the
compressed keys as bytes. Output descriptors write that as
`sortedmulti()`, which chapter 30's `ds_multi` already parses.

The script then goes in one of four places:

| Kind | scriptPubKey | scriptSig | Witness |
| --- | --- | --- | --- |
| Bare | The script | `OP_0 <sig> <sig>` | None |
| P2SH | `OP_HASH160 <HASH160(script)> OP_EQUAL` | `OP_0 <sig> <sig> <script>` | None |
| P2WSH | `OP_0 <SHA-256(script)>` | Empty | Empty item, signatures, script |
| P2SH-P2WSH | `OP_HASH160 <HASH160(P2WSH program)> OP_EQUAL` | `<P2WSH program>` | Empty item, signatures, script |

The leading `OP_0`, or empty witness item, feeds a bug:
`OP_CHECKMULTISIG` pops one item more than it uses. BIP-147's
`NULLDUMMY` rule requires that item to be empty.

P2SH limits the redeem script to 520 bytes, the largest push, so a P2SH
multisig holds at most 15 keys. P2WSH has no such limit up to 20 keys.

<!-- source: coax/lib/bitcoin/multisig.coax build -->
```jai
// CScript() << n for a count: OP_1 to OP_16, else the minimal CScriptNum
// pushed. Returns the next position.
ms_number :: (n: int, out: [] u8, at: int) -> int {
  if n >= 1 && n <= 16 { out[at] = cast(u8) (0x50 + n); return at + 1; }
  num: [9] u8;
  k := script_num_write_vartime(n, num[..]);
  out[at] = cast(u8) k;
  for i: 0..<k { out[at+1+i] = num[i]; }
  return at + 1 + k;
}
// The order of n keys of size bytes each, sorted as bytes (BIP-67).
ms_sort :: (keys: [] u8, size: int, n: int, order: [] int) {
  for i: 0..<n { order[i] = i; }
  for i: 1..<n {
    j := i;
    while j > 0 && ms_before(keys, size, order[j], order[j-1]) { t := order[j]; order[j] = order[j-1]; order[j-1] = t; j -= 1; }
  }
}
ms_before :: (keys: [] u8, size: int, a: int, b: int) -> bool {
  for k: 0..<size { x := keys[a*size+k]; y := keys[b*size+k]; if x != y { return x < y; } }
  return false;
}
// m <key 1> ... <key n> n OP_CHECKMULTISIG, for n compressed keys (33
// bytes each, one after another), in their order or, with sorted, in
// BIP-67's. Like Core's GetScriptForMultisig it doesn't check that a key
// is on the curve; BIP-67 sorts only compressed keys. Returns the
// script's length or a code.
multisig_script_vartime :: (m: int, keys: [] u8, n: int, sorted: bool, out: [] u8) -> int #export_c {
  if n < 1 || n > MS_MAX_KEYS || keys.count != 33 * n { return MS_KEY_COUNT; }
  if m < 1 || m > n { return MS_THRESHOLD; }
  for i: 0..<n {
    k := keys[33*i..<33*i+33];
    if k[0] != 2 && k[0] != 3 { return MS_KEY; }
  }
  if out.count < 3 + 2 + 34 * n { return MS_CAPACITY; }
  order: [MS_MAX_KEYS] int;
  if sorted { ms_sort(keys, 33, n, order[..]); } else { for i: 0..<n { order[i] = i; } }
  at := ms_number(m, out, 0);
  for i: 0..<n {
    out[at] = 33;
    for k: 0..<33 { out[at+1+k] = keys[33*order[i]+k]; }
    at += 34;
  }
  at = ms_number(n, out, at);
  out[at] = 0xae;                                         // OP_CHECKMULTISIG
  return at + 1;
}
// The scriptPubKey that locks to script in one of the four kinds: the
// script itself, its HASH160 under P2SH, its SHA-256 under P2WSH, or the
// P2WSH program's HASH160 under P2SH. For P2SH-P2WSH the redeem script,
// the P2WSH program, goes into redeem (34 bytes). Returns its length.
multisig_output_vartime :: (kind: int, script: [] u8, out: [] u8, redeem: [] u8) -> int #export_c {
  if out.count < 34 { return MS_CAPACITY; }
  if kind == MS_BARE { if out.count < script.count { return MS_CAPACITY; } return tx_put(out, 0, script); }
  if kind == MS_P2SH { if script.count > AD_REDEEM_MAX { return MS_TOO_LONG; } return p2sh_script_vartime(script, out); }
  if kind == MS_P2WSH { return p2wsh_script_vartime(script, out); }
  if redeem.count < 34 { return MS_CAPACITY; }
  #assert p2wsh_script_vartime(script, redeem) == 34;
  return p2sh_script_vartime(redeem[0..<34], out);
}
```

## Reading a script back

A wallet that finds a script must decide whether it is multisig before
it can sign it. Core's solver does that in `MatchMultisig`. The
threshold and the count must each be `OP_1` to `OP_16` or a minimal push
of a minimal number, from 1 to 20. Each key must have a valid size: 33
bytes after `02` or `03`, or 65 after `04`, `06` or `07`. The count must
equal the number of keys, and `OP_CHECKMULTISIG` must be the last byte.

<!-- source: coax/lib/bitcoin/multisig.coax match -->
```jai
// Core's CPubKey::ValidSize: 33 bytes after 02 or 03, 65 after 04, 06 or 07.
ms_valid_size :: (s: [] u8, at: int, n: int) -> bool {
  if n == 0 { return false; }
  h := s[at];
  return (n == 33 && (h == 2 || h == 3)) || (n == 65 && (h == 4 || h == 6 || h == 7));
}
// Core's GetScriptNumber: OP_1 to OP_16, or a minimal push of a minimal
// CScriptNum, from lo to hi; -1 otherwise.
ms_script_number :: (s: [] u8, op: ScriptOp, lo: int, hi: int) -> int {
  v := -1;
  if op.op >= OP_1 && op.op <= OP_16 { v = op.op - 0x50; }
  else if op.ok && op.op <= OP_PUSHDATA4 {
    data := s[op.at..<op.at+op.n];
    if !script_minimal_push_vartime(data, op.op) { return -1; }
    x := script_num_vartime(data, true, 4);
    if !x.ok { return -1; }
    v = x.value;
  }
  if v < lo || v > hi { return -1; }
  return v;
}
// A matched script: m of n keys, the key i at s[key[i]] (key_n[i] bytes).
MultisigMatch :: struct { ok: bool; m: int; n: int; }
// Core's MatchMultisig: the last byte is OP_CHECKMULTISIG, then m, keys
// of a valid size, n equal to the keys counted, and OP_CHECKMULTISIG.
multisig_match_vartime :: (s: [] u8, key: [] int, key_n: [] int) -> MultisigMatch #export_c {
  if s.count < 1 || s[s.count-1] != 0xae { return MultisigMatch.{}; }
  op := script_op_vartime(s, 0);
  if !op.ok { return MultisigMatch.{}; }
  m := ms_script_number(s, op, 1, MS_MAX_KEYS);
  if m < 0 { return MultisigMatch.{}; }
  n := 0;
  op = script_op_vartime(s, op.next);
  while op.ok && op.op <= OP_PUSHDATA4 && ms_valid_size(s, op.at, op.n) {
    if n < key.count { key[n] = op.at; key_n[n] = op.n; }
    n += 1;
    op = script_op_vartime(s, op.next);
  }
  count := ms_script_number(s, op, m, MS_MAX_KEYS);
  if count < 0 || count != n || n > key.count { return MultisigMatch.{}; }
  if op.next + 1 != s.count { return MultisigMatch.{}; }
  return MultisigMatch.{ok=true, m=m, n=n};
}
```

The solver checks only the shape. An uncompressed key is a valid size,
so a script with one is still multisig. A key that isn't on the curve
is the interpreter's problem, when a signature is checked against it.

## multi_a: m of n in tapscript

Tapscript disables `OP_CHECKMULTISIG`. It checked every signature
against several keys, so its cost wasn't known until it ran. Instead,
`OP_CHECKSIGADD` checks one signature against one key and adds 1 to a
counter when it is valid. BIP-387's `multi_a(m, key 1, ..., key n)` is
`<key 1> OP_CHECKSIG <key 2> OP_CHECKSIGADD ... <key n> OP_CHECKSIGADD
m OP_NUMEQUAL`, with 32-byte x-only keys. `sortedmulti_a` sorts them.

The leaf goes in a script tree, as chapter 42 builds it. To leave only
the script path, the internal key is BIP-341's H, a point whose secret
key nobody knows.

The witness holds one item per key. The first key's `OP_CHECKSIG` pops
the top item, so the item for key i is item n-1-i from the bottom. A
key that doesn't sign gets an empty item, which counts nothing. Any
other item must be a valid signature, or the whole spend fails: BIP-342
doesn't skip a bad signature.

The signature hash is BIP-341's, extended by BIP-342 with the leaf
hash, key version 0 and the position of the last `OP_CODESEPARATOR`
(none). Chapter 43 computes it. `multisig_a_check_vartime` decides a
witness as BIP-342's interpreter runs the leaf. Chapter 44's tapscript
interpreter runs it in full.

<!-- source: coax/lib/bitcoin/multisig.coax multia -->
```jai
// BIP-387's multi_a: <key 1> OP_CHECKSIG <key 2> OP_CHECKSIGADD ...
// <key n> OP_CHECKSIGADD m OP_NUMEQUAL, for n x-only keys (32 bytes
// each), in their order or sorted. Returns the leaf script's length.
multisig_a_script_vartime :: (m: int, keys: [] u8, n: int, sorted: bool, order: [] int, out: [] u8) -> int #export_c {
  if n < 1 || n > MS_A_MAX_KEYS || keys.count != 32 * n || order.count < n { return MS_KEY_COUNT; }
  if m < 1 || m > n { return MS_THRESHOLD; }
  if out.count < 34 * n + 5 { return MS_CAPACITY; }
  if sorted { ms_sort(keys, 32, n, order); } else { for i: 0..<n { order[i] = i; } }
  at := 0;
  for i: 0..<n {
    out[at] = 32;
    for k: 0..<32 { out[at+1+k] = keys[32*order[i]+k]; }
    out[at+33] = ifx i == 0 then 0xac else 0xba;         // OP_CHECKSIG, then OP_CHECKSIGADD
    at += 34;
  }
  at = ms_number(m, out, at);
  out[at] = 0x9c;                                         // OP_NUMEQUAL
  return at + 1;
}
// Core's MatchMultiA: 32-byte keys, the first followed by OP_CHECKSIG
// and the rest by OP_CHECKSIGADD, then the threshold and OP_NUMEQUAL. Key
// i is at s[34 * i + 1].
multisig_a_match_vartime :: (s: [] u8) -> MultisigMatch #export_c {
  if s.count == 0 || s[0] != 32 || s[s.count-1] != 0x9c { return MultisigMatch.{}; }
  at := 0; n := 0;
  while s.count - at >= 34 {
    if s[at] != 32 { return MultisigMatch.{}; }
    n += 1; at += 33;
    if cast(int) s[at] != (ifx n == 1 then 0xac else 0xba) { return MultisigMatch.{}; }
    at += 1;
  }
  if n == 0 || n > MS_A_MAX_KEYS { return MultisigMatch.{}; }
  op := script_op_vartime(s, at);
  if !op.ok || op.next >= s.count || s[op.next] != 0x9c || op.next + 1 != s.count { return MultisigMatch.{}; }
  m := ms_script_number(s, op, 1, n);
  if m < 0 { return MultisigMatch.{}; }
  return MultisigMatch.{ok=true, m=m, n=n};
}
// BIP-342's script-path signature hash of input index for a leaf of
// version 0xc0: BIP-341's message with the leaf hash, key version 0 and
// no OP_CODESEPARATOR.
multisig_a_sighash_vartime :: (b: [] u8, index: int, amounts: [] int, spent: [] BtScript, hashtype: int, leaf: [] u8, out: [] u8) -> bool #export_c {
  tp := bt_tx_read(b);
  if !tp.ok { return false; }
  h: [32] u8;
  if taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, leaf, h[..]) != TR_OK { return false; }
  return bt_sighash_taproot_ext(b, tp, index, amounts, spent, hashtype, b[0..<0], h[..], BT_NO_CODESEP, out);
}
// A BIP-340 signature of sighash by key (not tweaked: a script-path key
// signs as itself), into out (64 bytes).
multisig_a_sign :: (key: [] u8, sighash: [] u8, aux: [] u8, w: [] SignWork, out: [] u8) -> bool {
  status := SIGN_REFUSED;
  for k: 0..<6 { status = schnorr_sign_step(key, aux, sighash, w, out, false); }
  return status == SIGN_DONE;
}
// The leaf run on its witness stack (items[0] at the bottom): the key
// whose OP_CHECKSIG runs first takes the top item. An empty item counts
// nothing; any other must be a valid signature (64 bytes, or 65 with a
// hash type of 0x01 to 0x03 or 0x81 to 0x83 that hashtypes[] says the
// hash was made with); the count must be m. False for a stack of the
// wrong size.
multisig_a_check_vartime :: (s: [] u8, items: [] [] u8, sighashes: [] [32] u8, hashtypes: [] int) -> bool #export_c {
  x := multisig_a_match_vartime(s);
  if !x.ok || items.count != x.n { return false; }
  count := 0;
  for i: 0..<x.n {
    sig := items[x.n-1-i];
    if sig.count == 0 { continue; }
    t := ifx sig.count == 65 then cast(int) sig[64] else 0;
    if (sig.count != 64 && sig.count != 65) || (sig.count == 65 && (t == 0 || !ps_hashtype(t))) { return false; }
    h := -1;
    for k: 0..<hashtypes.count { if hashtypes[k] == t { h = k; } }
    if h < 0 || !schnorr_verify_vartime(sighashes[h][..], s[34*i+1..<34*i+33], sig[0..<64]) { return false; }
    count += 1;
  }
  return count == x.m;
}
```

## The example

The example is
[bitcoin/examples/multisig.coax](../examples/multisig.coax). It builds
BIP-67's vectors and asks Core's solver about twenty scripts. Its keys
A, B and C have secret keys `0x11...11`, `0x22...22` and `0x33...33`.

<!-- source: bitcoin/examples/multisig.coax example -->
```jai
#load "../../coax/lib/bitcoin/multisig.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

put_hex :: (b: [] u8) {
  text: [512] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
yes :: (b: bool) -> string { return ifx b then "yes" else "no"; }
put_address :: (spk: [] u8) {
  a: [128] u8;
  n := address_encode_vartime(SW_MAINNET, spk, a[..]);
  if n < 0 { print("no address"); } else { print("%", cast(string) a[0..<n]); }
}
// The keys A, B and C: secret keys 0x11..11, 0x22..22 and 0x33..33.
Keys :: struct { secret: [3][32] u8; pub: [99] u8; xonly: [96] u8; }
make_keys :: () -> Keys {
  k: Keys;
  for i: 0..<3 {
    for x: 0..<32 { k.secret[i][x] = cast(u8) (0x11 * (i + 1)); }
    #assert bip32_public(k.secret[i][..], k.pub[33*i..<33*i+33]);
    for x: 0..<32 { k.xonly[32*i+x] = k.pub[33*i+1+x]; }
  }
  return k;
}
put_le :: (out: [] u8, at: int, v: int, n: int) -> int {
  for k: 0..<n { out[at+k] = cast(u8) ((v >> (8 * k)) & 255); }
  return at + n;
}
// A transaction with one output per script, all of amount each, spending
// the outpoint whose txid is 32 bytes of fill; its length.
fund :: (fill: u8, spks: [] [128] u8, spk_n: [] int, count: int, amount: int, out: [] u8) -> int {
  at := put_le(out, 0, 2, 4);
  at = tx_compact_write_vartime(out, at, 1);
  for k: 0..<32 { out[at+k] = fill; }
  at = put_le(out, at + 32, 0, 4); out[at] = 0; at = put_le(out, at + 1, 0xffffffff, 4);
  at = tx_compact_write_vartime(out, at, count);
  for i: 0..<count {
    at = put_le(out, at, amount, 8);
    at = tx_compact_write_vartime(out, at, spk_n[i]); at = tx_put(out, at, spks[i][0..<spk_n[i]]);
  }
  return put_le(out, at, 0, 4);
}

main :: () {
  // BIP-67: sort the keys, and every wallet gets the same address.
  print("BIP-67's vectors, 2 of n with the keys sorted:\n");
  for i: 0..<MS_BIP67.count {
    v := MS_BIP67[i];
    keys: [33 * 4] u8; script: [256] u8; want: [256] u8; spk: [34] u8; redeem: [34] u8;
    k := ms_hex(v.keys, keys[..]);
    n := multisig_script_vartime(2, k, k.count / 33, true, script[..]);
    #assert multisig_output_vartime(MS_P2SH, script[0..<n], spk[..], redeem[..]) == 23;
    print("  vector %: 2 of %, script %, ", i + 1, k.count / 33, yes(ps_equal(script[0..<n], ms_hex(v.script, want[..]))));
    put_address(spk[0..<23]); print("\n");
  }

  // Which scripts Core's solver calls multisig.
  print("Core's MatchMultisig and MatchMultiA:\n");
  for i: 0..<MS_MATCH.count {
    c := MS_MATCH[i];
    s: [2048] u8; key: [MS_MAX_KEYS] int; key_n: [MS_MAX_KEYS] int;
    b := ms_hex(c.script, s[..]);
    x := ifx c.tapscript then multisig_a_match_vartime(b) else multisig_match_vartime(b, key[..], key_n[..]);
    if x.ok { print("  %: % of %\n", c.name, x.m, x.n); } else { print("  %: not multisig\n", c.name); }
  }

  keys: [1] Keys; keys[0] = make_keys();
  four(keys[..]);
  tapscript(keys[..]);
}
```

```shell
coax/bin/coax run bitcoin/examples/multisig.coax
```

```text
BIP-67's vectors, 2 of n with the keys sorted:
  vector 1: 2 of 2, script yes, 39bgKC7RFbpoCRbtD5KEdkYKtNyhpsNa3Z
  vector 2: 2 of 3, script yes, 3CKHTjBKxCARLzwABMu9yD85kvtm7WnMfH
  vector 3: 2 of 4, script yes, 32V85igBri9zcfBRVupVvwK18NFtS37FuD
  vector 4: 2 of 3, script yes, 3Q4sF6tv9wsdqu2NtARzNCpQgwifm2rAba
Core's MatchMultisig and MatchMultiA:
  a 2-of-2: 2 of 2
  a 1-of-2: 1 of 2
  n not the key count: not multisig
  m above n: not multisig
  m of 0: not multisig
  an uncompressed key: 1 of 1
  a key of 32 bytes: not multisig
  m pushed, not OP_1: not multisig
  17 of 20, pushed: 17 of 20
  21 keys: not multisig
  20 pushed as two bytes: not multisig
  a byte after the count: not multisig
  OP_CHECKMULTISIGVERIFY: not multisig
  a 2-of-3 multi_a: 2 of 3
  a 1-of-1 multi_a: 1 of 1
  multi_a, 4 of 3: not multisig
  multi_a, OP_CHECKSIG twice: not multisig
  multi_a, OP_NUMEQUALVERIFY: not multisig
  multi_a, 17 of 20: 17 of 20
  multi_a, threshold pushed: not multisig
```

Each BIP-67 script and address matches the one the BIP publishes. Core
refuses `m pushed, not OP_1`, because a push of 1 isn't minimal: `OP_1`
is. It refuses `20 pushed as two bytes` for the same reason.

### One 2-of-3, four ways

The next part locks one `sortedmulti` 2-of-3 of A, B and C four ways,
funds each with 100,000 satoshis, and spends all four in one
transaction. The Updater gives each input its UTXO and its scripts. A
signs every input, then C does. The Input Finalizer builds each
scriptSig and witness, and chapter 36's interpreter checks each input.

<!-- source: bitcoin/examples/multisig.coax four -->
```jai
// One 2-of-3 of keys A, B and C, sorted, locked four ways, then spent in
// one transaction through a PSBT: A and C sign each input.
four :: (keys: [] Keys) {
  script: [128] u8; spks: [4][128] u8; spk_n: [4] int; redeem: [34] u8;
  n := multisig_script_vartime(2, keys[0].pub[..], 3, true, script[..]);
  ws := script[0..<n];
  print("One 2-of-3 (sortedmulti), % bytes, locked four ways:\n", n);
  for kind: 0..<4 {
    spk_n[kind] = multisig_output_vartime(kind, ws, spks[kind][..], redeem[..]);
    print("  %: ", MS_KINDS[kind]); put_hex(spks[kind][0..<spk_n[kind]]); print(", "); put_address(spks[kind][0..<spk_n[kind]]); print("\n");
  }
  // The coins: one transaction with the four outputs, 100,000 each.
  prev: [512] u8; id: [32] u8; pins: [1] TxIn; pouts: [4] TxOut; pitems: [1] TxItem;
  pn := fund(1, spks[..], spk_n[..], 4, 100000, prev[..]);
  pt := tx_read_vartime(prev[0..<pn], false, pins[..], pouts[..], pitems[0..<0]);
  #assert tx_txid_vartime(prev[0..<pn], pt, id[..]);
  // The spend: its four outputs to one P2WPKH output of key A.
  tx: [512] u8; out: [22] u8;
  at := put_le(tx[..], 0, 2, 4);
  at = tx_compact_write_vartime(tx[..], at, 4);
  for i: 0..<4 { at = tx_put(tx[..], at, id[..]); at = put_le(tx[..], at, i, 4); tx[at] = 0; at = put_le(tx[..], at + 1, 0xffffffff, 4); }
  at = tx_compact_write_vartime(tx[..], at, 1);
  at = put_le(tx[..], at, 399000, 8);
  k := p2wpkh_script_vartime(keys[0].pub[0..<33], out[..]);
  at = tx_compact_write_vartime(tx[..], at, k); at = tx_put(tx[..], at, out[0..<k]);
  at = put_le(tx[..], at, 0, 4);
  // Updater: the previous transaction for bare and P2SH, the output for
  // the segwit kinds; the redeemScript under P2SH, the witnessScript
  // under P2WSH.
  m: [1] PsbtMaps; w: [1] PsbtSign; aux: [32] u8;
  #assert psbt_create_vartime(tx[0..<at], m[..]);
  for i: 0..<4 {
    if i < 2 { #assert psbt_set1_vartime(m[..], PSBT_INPUT, i, 0x00, prev[0..<pn]); }
    else { #assert psbt_set1_vartime(m[..], PSBT_INPUT, i, 0x01, prev[pouts[i].value..<pouts[i].script+pouts[i].script_n]); }
  }
  #assert psbt_set1_vartime(m[..], PSBT_INPUT, 1, 0x04, ws);
  #assert psbt_set1_vartime(m[..], PSBT_INPUT, 2, 0x05, ws);
  #assert psbt_set1_vartime(m[..], PSBT_INPUT, 3, 0x04, redeem[..]) && psbt_set1_vartime(m[..], PSBT_INPUT, 3, 0x05, ws);
  print("Spending all four through one PSBT:\n");
  for i: 0..<4 { print("  input % (%), key A: %\n", i, MS_KINDS[i], PSBT_SIGN_REASONS[psbt_sign_vartime(m[..], i, keys[0].secret[0][..], aux[..], true, w[..])]); }
  one: [1] PsbtMaps; one[0] = m[0];
  print("  with A's signatures only, input 0 is final: %\n", yes(psbt_finalize_input_vartime(one[..], 0, w[..])));
  for i: 0..<4 { print("  input % (%), key C: %\n", i, MS_KINDS[i], PSBT_SIGN_REASONS[psbt_sign_vartime(m[..], i, keys[0].secret[2][..], aux[..], true, w[..])]); }
  print("  Input Finalizer: final %\n", yes(psbt_finalize_vartime(m[..], w[..])));
  net: [4096] u8; ins: [4] TxIn; outs: [1] TxOut; items: [32] TxItem; txid: [32] u8; rev: [32] u8;
  size := psbt_extract_vartime(m[..], net[..], w[..]);
  b := net[0..<size];
  t := tx_read_vartime(b, true, ins[..], outs[..], items[..]);
  #assert tx_txid_vartime(b, t, txid[..]);
  for x: 0..<32 { rev[x] = txid[31-x]; }
  print("  Transaction Extractor: % bytes, weight %, txid ", size, tx_weight_vartime(t)); put_hex(rev[..]); print("\n");
  sw: [1] ScriptWork;
  flags := SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_WITNESS | SCRIPT_VERIFY_DERSIG | SCRIPT_VERIFY_LOW_S | SCRIPT_VERIFY_STRICTENC
    | SCRIPT_VERIFY_NULLDUMMY | SCRIPT_VERIFY_CLEANSTACK | SCRIPT_VERIFY_NULLFAIL | SCRIPT_VERIFY_WITNESS_PUBKEYTYPE;
  for i: 0..<4 {
    sp := ScriptSpend.{b=b, t=t, ins=ins[..], outs=outs[..], items=items[..], index=i, amount=100000, spk=spks[i][0..<spk_n[i]]};
    in := ins[i];
    print("  input % (%): scriptSig % bytes, % witness items, %\n", i, MS_KINDS[i], in.script_n, in.count, SCRIPT_ERR_NAMES[script_verify_vartime(sp, flags, sw[..])]);
  }
}
```

```text
One 2-of-3 (sortedmulti), 105 bytes, locked four ways:
  bare: 5221023c72addb4fdf09af94f0c94d7fe92a386a7e70cf8a1d85916386bb2535c7b1b12102466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f2721034f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa53ae, no address
  P2SH: a914ecc0f983d9089a2cb149c6ddc4b938acfa5e8e7c87, 3PGrZvwEZgYFqJENFqBMiZo6eXfSCN6ueY
  P2WSH: 0020f21b1eeed4f1ba38d9cbc48c2ef34509654a372316aea91f5feaeefd1f74b666, bc1q7gd3amk57xar3kwtcjxzau69p9j55derz6h2j86lath068m5kenqzkhtp3
  P2SH-P2WSH: a914ea0c230ea79e3ba3dd08becefb2e253f0f9c382387, 3P2YaiokgXjT86Euv9Vhvzn1t53HfKQQfP
Spending all four through one PSBT:
  input 0 (bare), key A: ok
  input 1 (P2SH), key A: ok
  input 2 (P2WSH), key A: ok
  input 3 (P2SH-P2WSH), key A: ok
  with A's signatures only, input 0 is final: no
  input 0 (bare), key C: ok
  input 1 (P2SH), key C: ok
  input 2 (P2WSH), key C: ok
  input 3 (P2SH-P2WSH), key C: ok
  Input Finalizer: final yes
  Transaction Extractor: 1145 bytes, weight 3056, txid 6acb9f2c85ac6a9de0987bcb6fd3fbda6574179b0afda06664133a7501eca696
  input 0 (bare): scriptSig 145 bytes, 0 witness items, OK
  input 1 (P2SH): scriptSig 252 bytes, 0 witness items, OK
  input 2 (P2WSH): scriptSig 0 bytes, 4 witness items, OK
  input 3 (P2SH-P2WSH): scriptSig 35 bytes, 4 witness items, OK
```

A bare multisig output has no address format, so a wallet can't ask to
be paid to one. The sizes show what each kind costs. The bare and P2SH
inputs carry the signatures in the scriptSig, at 4 weight units a byte.
The P2WSH input carries them in the witness, at 1 unit a byte. The
P2SH-P2WSH input adds only the 35-byte push of the P2WSH program.

### The same keys as a multi_a leaf

The last part builds a `sortedmulti_a` 2-of-3 of the same keys' x-only
halves under BIP-341's H, and spends its one coin on the script path.

<!-- source: bitcoin/examples/multisig.coax tapscript -->
```jai
// The same 2-of-3 as a Taproot leaf, multi_a sorted, under BIP-341's
// unspendable internal key H: no key can spend the key path, so only the
// leaf can. A and C sign the leaf's signature hash.
NAMES :: [3] string.["A", "B", "C"];
TAP_H :: "50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0";
tapscript :: (keys: [] Keys) {
  print("The same 2-of-3 as a multi_a leaf under BIP-341's key H:\n");
  leaf: [128] u8; order: [3] int; internal: [32] u8; tree: [1] TrTree; control: [33] u8; q: [32] u8;
  n := multisig_a_script_vartime(2, keys[0].xonly[..], 3, true, order[..], leaf[..]);
  s := leaf[0..<n];
  hashes: [1][32] u8; depths := [1] int.[0];
  #assert taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, s, hashes[0][..]) == TR_OK && taproot_tree_vartime(depths[..], hashes[..], tree[..]) == TR_OK;
  #assert ms_hex(TAP_H, internal[..]).count == 32 && taproot_control_block_vartime(tree[..], 0, internal[..], TR_LEAF_TAPSCRIPT, control[..]) == 33;
  #assert taproot_output_key_vartime(internal[..], tree[0].root[..], q[..]) >= 0;
  spks: [4][128] u8; spk_n: [4] int;
  spks[0][0] = 0x51; spks[0][1] = 32; for x: 0..<32 { spks[0][2+x] = q[x]; } spk_n[0] = 34;
  print("  leaf, % bytes: ", n); put_hex(s); print("\n");
  print("  output: "); put_address(spks[0][0..<34]); print("\n");
  // The spend of its one coin of 100,000 to key A's P2WPKH.
  prev: [128] u8; id: [32] u8; pins: [1] TxIn; pouts: [1] TxOut; pitems: [1] TxItem;
  pn := fund(2, spks[..], spk_n[..], 1, 100000, prev[..]);
  pt := tx_read_vartime(prev[0..<pn], false, pins[..], pouts[..], pitems[0..<0]);
  #assert tx_txid_vartime(prev[0..<pn], pt, id[..]);
  tx: [512] u8; out: [22] u8;
  at := put_le(tx[..], 0, 2, 4);
  at = tx_compact_write_vartime(tx[..], at, 1);
  at = tx_put(tx[..], at, id[..]); at = put_le(tx[..], at, 0, 4); tx[at] = 0; at = put_le(tx[..], at + 1, 0xffffffff, 4);
  at = tx_compact_write_vartime(tx[..], at, 1);
  at = put_le(tx[..], at, 99000, 8);
  k := p2wpkh_script_vartime(keys[0].pub[0..<33], out[..]);
  at = tx_compact_write_vartime(tx[..], at, k); at = tx_put(tx[..], at, out[0..<k]);
  at = put_le(tx[..], at, 0, 4);
  unsigned := tx[0..<at];
  amounts := [1] int.[100000]; spent: [1] BtScript; spent[0].n = 34;
  for x: 0..<34 { spent[0].b[x] = spks[0][x]; }
  sighash: [1][32] u8; hashtypes := [1] int.[0];
  #assert multisig_a_sighash_vartime(unsigned, 0, amounts[..], spent[..], 0, s, sighash[0][..]);
  // Each signer's place: the key at leaf position i takes item n-1-i.
  sigs: [3][64] u8; aux: [32] u8; w: [1] SignWork; blank: [1] u8;
  items: [3] [] u8;
  for i: 0..<3 {
    who := order[i];
    items[2-i] = blank[0..<0];
    if who != 1 {
      #assert multisig_a_sign(keys[0].secret[who][..], sighash[0][..], aux[..], w[..], sigs[i][..]);
      items[2-i] = sigs[i][..];
    }
    print("  key % at leaf position %: %\n", NAMES[who], i, ifx who == 1 then "no signature" else "signs");
  }
  print("  the control block commits to the output: %\n", yes(taproot_commitment_vartime(control[..], s, q[..]) == TR_OK));
  print("  A's and C's signatures satisfy the leaf: %\n", yes(multisig_a_check_vartime(s, items[..], sighash[..], hashtypes[..])));
  // The network transaction: the witness is the three items, the leaf
  // and the control block.
  net: [1024] u8;
  nt := tx_put(net[..], 0, unsigned[0..<4]); net[nt] = 0; net[nt+1] = 1;
  nt = tx_put(net[..], nt + 2, unsigned[4..<at-4]);
  nt = tx_compact_write_vartime(net[..], nt, 5);
  for i: 0..<3 { nt = tx_compact_write_vartime(net[..], nt, items[i].count); nt = tx_put(net[..], nt, items[i]); }
  nt = tx_compact_write_vartime(net[..], nt, n); nt = tx_put(net[..], nt, s);
  nt = tx_compact_write_vartime(net[..], nt, 33); nt = tx_put(net[..], nt, control[..]);
  nt = tx_put(net[..], nt, unsigned[at-4..<at]);
  ins: [1] TxIn; outs: [1] TxOut; titems: [8] TxItem; txid: [32] u8; rev: [32] u8;
  t := tx_read_vartime(net[0..<nt], true, ins[..], outs[..], titems[..]);
  #assert t.code == TX_OK && tx_txid_vartime(net[0..<nt], t, txid[..]);
  for x: 0..<32 { rev[x] = txid[31-x]; }
  print("  the spend: % bytes, weight %, txid ", nt, tx_weight_vartime(t)); put_hex(rev[..]); print("\n");
  // What a node refuses: one signature, or C's signature in A's place.
  one := items;
  for i: 0..<3 { if order[i] == 0 { one[2-i] = blank[0..<0]; } }
  print("  one signature satisfies it: %\n", yes(multisig_a_check_vartime(s, one[..], sighash[..], hashtypes[..])));
  swapped := items; tmp := swapped[0]; swapped[0] = swapped[2]; swapped[2] = tmp;
  print("  the signatures swapped satisfy it: %\n", yes(multisig_a_check_vartime(s, swapped[..], sighash[..], hashtypes[..])));
}
```

```text
The same 2-of-3 as a multi_a leaf under BIP-341's key H:
  leaf, 104 bytes: 203c72addb4fdf09af94f0c94d7fe92a386a7e70cf8a1d85916386bb2535c7b1b1ac20466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f27ba204f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aaba529c
  output: bc1paeqku8nf0necsracs6qa32ktenjfwwxkxmnnj49ve59zga77f38szefnue
  key C at leaf position 0: signs
  key B at leaf position 1: no signature
  key A at leaf position 2: signs
  the control block commits to the output: yes
  A's and C's signatures satisfy the leaf: yes
  the spend: 355 bytes, weight 601, txid 00c2d4660b1bde39a49dccbf4013e872cfa8a954a9a9e438eb69e0532368cb25
  one signature satisfies it: no
  the signatures swapped satisfy it: no
```

Sorting puts C's key first and A's last. The witness is C's signature on
top, then an empty item for B, then A's signature at the bottom,
followed by the leaf and the control block. Swapped, each signature
meets the wrong key and the spend fails. The witness still carries one
item for every key and the whole leaf. Chapter 101's MuSig2 shrinks an
n-of-n to one key-path signature.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/multisig.coax](../examples/checks/multisig.coax),
which loads the example:

- `sortedmulti` and `sortedmulti_a` give the same script for every order
  of the same three keys and every threshold;
- every script the builder writes for m of n up to 20, bare and
  `multi_a`, matches with its m and n.

<!-- source: bitcoin/examples/checks/multisig.coax check -->
```jai
// The six orders of three keys, three entries each.
ORDERS :: [18] int.[0,1,2, 0,2,1, 1,0,2, 1,2,0, 2,0,1, 2,1,0];
// sortedmulti doesn't depend on the order the keys come in: each of the
// six orders of BIP-67's three keys gives the same script, for every
// threshold.
#forall "sortedmulti is the same script in every key order" p: 0..<18 #steps 20000000 {
  raw: [128] u8; pub := ms_hex(MS_BIP67[1].keys, raw[..]); xonly: [96] u8;
  for i: 0..<3 { for x: 0..<32 { xonly[32*i+x] = pub[33*i+1+x]; } }
  m := p / 6 + 1; o := 3 * (p % 6);
  shuffled: [99] u8; xs: [96] u8;
  for i: 0..<3 {
    for x: 0..<33 { shuffled[33*i+x] = pub[33*ORDERS[o+i]+x]; }
    for x: 0..<32 { xs[32*i+x] = xonly[32*ORDERS[o+i]+x]; }
  }
  a: [128] u8; b: [128] u8; order: [3] int; order2: [3] int;
  n := multisig_script_vartime(m, pub, 3, true, a[..]);
  #assert multisig_script_vartime(m, shuffled[..], 3, true, b[..]) == n && ps_equal(a[0..<n], b[0..<n]);
  n = multisig_a_script_vartime(m, xonly[..], 3, true, order[..], a[..]);
  #assert multisig_a_script_vartime(m, xs[..], 3, true, order2[..], b[..]) == n && ps_equal(a[0..<n], b[0..<n]);
}
// What the builder writes, Core's solver reads back: m of n for every
// m and n up to 20, as bare multisig and as multi_a.
#forall "every built script matches with its m and n" c: 0..<400 #steps 20000000 {
  m := c / 20 + 1; n := c % 20 + 1;
  if m <= n {
    raw: [128] u8; pub := ms_hex(MS_BIP67[1].keys, raw[..]); xonly: [96] u8;
    for i: 0..<3 { for x: 0..<32 { xonly[32*i+x] = pub[33*i+1+x]; } }
    many: [660] u8; xs: [640] u8;
    for i: 0..<n {
      for x: 0..<33 { many[33*i+x] = pub[33*(i%3)+x]; }
      for x: 0..<32 { xs[32*i+x] = xonly[32*(i%3)+x]; }
    }
    s: [2048] u8; key: [MS_MAX_KEYS] int; key_n: [MS_MAX_KEYS] int; order: [20] int;
    k := multisig_script_vartime(m, many[0..<33*n], n, false, s[..]);
    x := multisig_match_vartime(s[0..<k], key[..], key_n[..]);
    #assert x.ok && x.m == m && x.n == n;
    k = multisig_a_script_vartime(m, xs[0..<32*n], n, false, order[..], s[..]);
    y := multisig_a_match_vartime(s[0..<k]);
    #assert y.ok && y.m == m && y.n == n;
  }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/multisig.coax
```

`coax test` runs 485 checks, including the loaded files' own checks, on 2026-11-03.

### The library's own checks

`multisig.coax` is an `#authority` file. Its checks run the counts, all
four BIP-67 vectors (sorted keys, script and P2SH address), the
builder's refusals (threshold, key count, key prefix, the 520-byte P2SH
limit, capacity), Core's `MatchMultisig` and `MatchMultiA` on all twenty
cases, all ten scripts of BIP-387's valid descriptors, a `multi_a`
witness with two, one, a bad and a wrong number of signatures, and a
script-path signature hash that changes with the leaf.

```shell
coax/bin/coax test coax/lib/bitcoin/multisig.coax
```

`coax test` runs 481 checks, including those of the files it loads, on 2026-11-03. Mutation testing: not run (owner speed order).

### Python

[make_multisig_vectors.py](../../coax/tests/bitcoin/make_multisig_vectors.py)
copies BIP-67's vectors and BIP-387's descriptors from the pinned texts.
It recomputes each BIP-67 script and address, and resolves each BIP-387
key from its WIF, hex or BIP-32 form, builds the leaf and checks the
published scriptPubKey. It transliterates Core's `MatchMultisig` and
`MatchMultiA`, checks the lines it follows against the pinned
`solver.cpp`, decides the twenty cases, and writes
`multisig_vectors.coax`.
[multisig.py](../../coax/tests/bitcoin/multisig.py) writes the
transcript the example must print from Python: chapter 94's
`psbt_reference.py` signs and finalizes the four inputs,
`script_reference.py`'s interpreter checks them, and Python's BIP-340
and BIP-342 signature hash sign and check the `multi_a` spend.

```shell
python3 -I coax/tests/bitcoin/make_multisig_vectors.py
python3 -I coax/tests/bitcoin/multisig.py --coax coax/bin/coax
```

On 2026-11-03 all four BIP-67 vectors and all ten BIP-387 scripts recompute, all 58 lines of the transcript agree with Python, and the example's output in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Miniscript, which writes any policy of keys, hashes and time locks as
  a script, and its descriptors (BIP-379).
- Signing `multi_a` leaves through PSBT fields (BIP-371's type `0x14`):
  the leaf here is signed directly.
- MuSig2's aggregate key, which hides an n-of-n behind one key:
  chapter 101. FROST threshold signatures: chapter 102.

## Summary

- `m <keys> n OP_CHECKMULTISIG` needs m signatures from n keys, in key
  order, after one extra empty item.
- It goes bare, under P2SH, under P2WSH or under P2SH-P2WSH; witness
  kinds carry the signatures at a quarter of the weight.
- BIP-67 sorts the keys so every party gets the same script and address.
- Core's solver accepts only minimal counts, valid key sizes and a key
  count that matches.
- Tapscript's `multi_a` checks one signature per key with
  `OP_CHECKSIGADD`. The witness has one item per key, top first, empty
  for a key that doesn't sign.

## Dependencies

- [30. Output descriptors and their checksum](ch030-output-descriptors-and-their-checksum.md)
- [36. The script interpreter](ch036-the-script-interpreter.md)
- [42. Script trees: tapleaves and tapbranches](ch042-script-trees-tapleaves-and-tapbranches.md)
- [43. Taproot signature hashes](ch043-taproot-signature-hashes.md)
- [94. PSBT](ch094-psbt.md)

## Sources

- [BIP-67](https://github.com/bitcoin/bips/blob/master/bip-0067.mediawiki)
  (sorted multisig keys) and
  [BIP-387](https://github.com/bitcoin/bips/blob/master/bip-0387.mediawiki)
  (`multi_a` descriptors), pinned in
  [coax/tests/vectors/upstream/](../../coax/tests/vectors/upstream/bip67/bip-0067.mediawiki).
- Bitcoin Core's `script/solver.cpp` (`MatchMultisig`, `MatchMultiA`,
  `GetScriptForMultisig`), pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/solver.cpp).
- [BIP-11](https://github.com/bitcoin/bips/blob/master/bip-0011.mediawiki)
  (m-of-n standard transactions),
  [BIP-147](https://github.com/bitcoin/bips/blob/master/bip-0147.mediawiki)
  (`NULLDUMMY`) and
  [BIP-342](https://github.com/bitcoin/bips/blob/master/bip-0342.mediawiki)
  (`OP_CHECKSIGADD`).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
