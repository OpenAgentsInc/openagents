# 94. PSBT

A transaction often needs more than one party. One wallet picks the coins
and the outputs, another knows the scripts, a hardware device holds a
key, and a second signer holds another. Each of them needs the
transaction, plus what it can't know itself: the outputs being spent,
the scripts behind their hashes, and which key sits at which BIP-32
path. A partially signed Bitcoin transaction (PSBT) carries all of it in
one format that every party reads and adds to.

This chapter writes BIP-174's PSBT version 0, BIP-370's version 2 and
BIP-371's Taproot fields, read as Bitcoin Core reads them, and the six
roles that pass a PSBT from party to party. All 155 distinct PSBTs that
BIP-174, BIP-370, BIP-371 and Core's `rpc_psbt.json` publish read or
refuse as published. Each refusal has Core's message. BIP-174's worked
example runs from its master key to the final network transaction, byte
for byte. The chapter ends with our own spend of a P2WPKH coin, a P2WSH
2-of-2 coin and a P2TR coin, which chapter 36's interpreter and chapter
41's key-path check accept.

## What this chapter teaches

- What a PSBT is: a magic, a global map, one map per input and one per
  output, each a list of key-value pairs.
- How versions 0 and 2 differ: one unsigned transaction, or the
  transaction's parts as fields.
- The checks a reader makes: duplicate keys, key lengths, stated sizes,
  public keys, signatures, and the fields each version allows or
  requires.
- The six roles: Creator, Updater, Signer, Combiner, Input Finalizer and
  Transaction Extractor.
- What a signer checks before it signs, and why.
- How Core signs: low-R ECDSA, ground with libsecp256k1's extra data.

## The six roles

| Role | What it does | Here |
| --- | --- | --- |
| Creator | Writes a PSBT for an unsigned transaction | `psbt_create_vartime` |
| Updater | Adds UTXOs, scripts, key paths and hash types | `psbt_set_vartime` |
| Signer | Checks the input, then adds a partial or key-path signature | `psbt_sign_vartime` |
| Combiner | Merges PSBTs of one transaction | `psbt_combine_vartime` |
| Input Finalizer | Builds each input's final scriptSig and witness | `psbt_finalize_vartime` |
| Transaction Extractor | Writes the network transaction | `psbt_extract_vartime` |

BIP-370 adds a Constructor, which adds inputs and outputs to a version 2
PSBT while its modifiable flags allow it. This chapter reads and writes
those flags but builds no Constructor.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [psbt.coax](../../coax/lib/bitcoin/psbt.coax) | `psbt_read_vartime`, `psbt_base64_vartime` | Core's `DecodeRawPSBT` and `DecodeBase64PSBT` |
| | `psbt_key_size_vartime`, `psbt_key_name_vartime` | Core's expected key size of each type, and its name |
| | `psbt_maps_vartime`, `psbt_find_vartime`, `psbt_set_vartime` | The pairs, a lookup, the Updater |
| | `psbt_write_vartime` | Core's `Serialize` order |
| | `psbt_create_vartime`, `psbt_locktime_vartime`, `psbt_unsigned_vartime` | The Creator, BIP-370's lock time and unsigned transaction |
| | `psbt_sign_vartime` | The Signer |
| | `psbt_combine_vartime` | The Combiner |
| | `psbt_finalize_input_vartime`, `psbt_finalize_vartime` | The Input Finalizer |
| | `psbt_extract_vartime` | The Transaction Extractor |
| [ecdsa_sign.coax](../../coax/lib/crypto/ecdsa_sign.coax) | `ecdsa_sign_low_r` | Core's low-R ECDSA (chapter 20) |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `script_sighash_legacy_vartime`, `script_sighash_v0_vartime` | Chapters 37's and 38's signature hashes |
| [sighash.coax](../../coax/lib/bitcoin/sighash.coax) | `bt_sighash_taproot` | Chapter 43's BIP-341 signature hash |
| [taproot.coax](../../coax/lib/bitcoin/taproot.coax) | `bt_taproot_sign`, `taproot_key_path_verify_vartime` | Chapter 41's key-path signer and check |
| [bip32.coax](../../coax/lib/crypto/bip32.coax) | `bip32_public` | Chapter 32's public key of a secret key |

## The format

A PSBT starts with the five bytes `psbt` and `0xff`. Then come maps: the
global map, one map per input and one per output, in the transaction's
order. Each map is a list of pairs and ends with a zero byte, the
separator. A pair is a key and a value, each with its length in front
of it as a compact size. A key's first compact size is its type, and the
rest of the key is data, for example the public key a partial signature
belongs to.

A version 0 PSBT holds the unsigned transaction in the global map, type
`0x00`. Its scriptSigs and witnesses are empty, so the transaction's
txid doesn't change as the PSBT is signed. A version 2 PSBT has no
unsigned transaction. Its global map gives the transaction's version and
the numbers of inputs and outputs, each input map gives its previous
txid, output index and sequence, and each output map its amount and
script. That lets a Constructor add an input or an output later.

The reader keeps the pairs where they are in the caller's bytes:

<!-- source: coax/lib/bitcoin/psbt.coax layout -->
```jai
// The three kinds of map.
PSBT_GLOBAL :: 0; PSBT_INPUT :: 1; PSBT_OUTPUT :: 2;
PSBT_IO :: 512;          // inputs or outputs of one transaction the reader holds
PSBT_ITEMS :: 2048;      // witness items of one transaction
// One key-value pair: its map (and which input or output), its type, the
// key (type included) at key, key_n bytes, and the value at value, value_n
// bytes, without the lengths in front of them.
PsbtField :: struct { map: int; index: int; type: int; key: int; key_n: int; value: int; value_n: int; }
// A read PSBT, or the refusal code found at byte at; for PSBT_KEY_SIZE,
// the map and type whose key was the wrong size. version is 0 or 2, and
// fields[0..<nfields] are its pairs in the order they came. For version
// 0, tx is where the unsigned transaction's value starts and tx_n its
// length.
Psbt :: struct { code: int; at: int; map: int; type: int; version: int; nin: int; nout: int; nfields: int; tx: int; tx_n: int; }
// The reader's workspace: the global transaction's inputs, and room to
// read each non-witness UTXO.
PsbtRead :: struct {
  gins: [PSBT_IO] TxIn; gouts: [PSBT_IO] TxOut; gt: Tx;
  ins: [PSBT_IO] TxIn; outs: [PSBT_IO] TxOut; items: [PSBT_ITEMS] TxItem;
}
```

## Reading one map

Core reads each map the same way, and so does `pr_map`. A key that the
map already has refuses as a duplicate. A key's type decides how long
the key must be and how the value is read: `pr_value` reads each type's
value, checks its public keys and signatures, and checks that it read
exactly the stated size. The separator ends the map, and in version 2 an
input map must have its previous txid and index, and an output map its
amount and script:

<!-- source: coax/lib/bitcoin/psbt.coax map -->
```jai
// One map, from at to its separator: each key once, each value checked,
// each pair added to fields. found[t] says a pair of type t was read.
// Returns where the next map starts, or -1.
pr_map :: (b: [] u8, at: int, map: int, index: int, version: int, fields: [] PsbtField, w: [] PsbtRead, r: [] Psbt, found: [] bool) -> int {
  for t: 0..<256 { found[t] = false; }
  first := r[0].nfields;
  pos := at;
  while pos < b.count {
    kat := pos;
    k := pr_vector(b, pos, r);
    if k.next < 0 { return -1; }
    if k.n == 0 {
      // The separator; version 2 maps have required fields.
      if map == PSBT_INPUT && version >= 2 {
        if !found[0x0e] { return pr_fail(r, PSBT_V2_PREV_TXID, k.next); }
        if !found[0x0f] { return pr_fail(r, PSBT_V2_PREV_INDEX, k.next); }
      }
      if map == PSBT_OUTPUT && version >= 2 {
        if !found[0x03] { return pr_fail(r, PSBT_V2_AMOUNT, k.next); }
        if !found[0x04] { return pr_fail(r, PSBT_V2_SCRIPT, k.next); }
      }
      return k.next;
    }
    for j: first..<r[0].nfields {
      f := fields[j];
      same := f.key_n == k.n;
      for x: 0..<k.n { if same && b[f.key+x] != b[k.at+x] { same = false; } }
      if same { return pr_fail(r, PSBT_DUP_GLOBAL + map, kat); }
    }
    tc := tx_compact_read_vartime(b[k.at..<k.next], 0);
    if tc.code != TX_OK { return pr_fail(r, pr_tx_code(tc.code), k.at); }
    t := tc.value;
    next := pr_value(b, k.next, map, version, t, k, w, r, kat);
    if next < 0 { return -1; }
    if r[0].nfields == fields.count { return pr_fail(r, PSBT_CAPACITY, kat); }
    v := tx_compact_read_vartime(b, k.next);
    fields[r[0].nfields] = PsbtField.{map=map, index=index, type=t, key=k.at, key_n=k.n, value=v.next, value_n=v.value};
    r[0].nfields += 1;
    if t < 256 { found[t] = true; }
    pos = next;
  }
  return pr_fail(r, PSBT_SEP_GLOBAL + map, pos);
}
```

`psbt_read_vartime` reads the global map, then checks the version. A
version 0 PSBT needs its unsigned transaction and refuses BIP-370's
global fields; version 1 doesn't exist; version 2 needs its counts and
version and refuses an unsigned transaction. Each input's non-witness
UTXO must hash to the txid it spends and have the output it names. The
number of maps must be the transaction's, and nothing may follow the last
one.

A key of the wrong length for its type refuses with Core's message,
which names the size and the type, for example `Size of key was not 1
for the type Input Witness UTXO`. `psbt_key_size_vartime` gives the size
and `psbt_key_name_vartime` the name.

## The pairs, and the Updater

The roles work on `PsbtMaps`, the pairs copied into one arena. The
Updater is `psbt_set_vartime`: it adds a pair, or replaces the value of a
key the map already has:

<!-- source: coax/lib/bitcoin/psbt.coax pairs -->
```jai
PSBT_PAIRS :: 512;       // pairs in one PSBT
PSBT_ARENA :: 65536;     // bytes of their keys and values
PSBT_TX :: 16384;        // the longest transaction a role builds
// One pair: its map and index, its key and value in the arena.
PsbtPair :: struct { map: int; index: int; key: int; key_n: int; value: int; value_n: int; }
// A PSBT as its pairs: the version, the counts, pairs[0..<n] and the
// arena's first used bytes. The roles read and change this.
PsbtMaps :: struct { version: int; nin: int; nout: int; n: int; used: int; pairs: [PSBT_PAIRS] PsbtPair; arena: [PSBT_ARENA] u8; }
pm_key :: (m: [] PsbtMaps, j: int) -> [] u8 { p := m[0].pairs[j]; return m[0].arena[p.key..<p.key+p.key_n]; }
pm_value :: (m: [] PsbtMaps, j: int) -> [] u8 { p := m[0].pairs[j]; return m[0].arena[p.value..<p.value+p.value_n]; }
pm_type :: (m: [] PsbtMaps, j: int) -> int { return tx_compact_read_vartime(pm_key(m, j), 0).value; }
pm_copy :: (m: [] PsbtMaps, s: [] u8) -> int {
  at := m[0].used;
  for i: 0..<s.count { m[0].arena[at+i] = s[i]; }
  m[0].used += s.count;
  return at;
}
// The pair with this key in map (index), or -1.
psbt_find_vartime :: (m: [] PsbtMaps, map: int, index: int, key: [] u8) -> int {
  for j: 0..<m[0].n {
    p := m[0].pairs[j];
    if p.map == map && p.index == index && p.key_n == key.count {
      same := true;
      for x: 0..<key.count { if m[0].arena[p.key+x] != key[x] { same = false; } }
      if same { return j; }
    }
  }
  return -1;
}
// The pair of the one-byte key t, or -1.
psbt_find1_vartime :: (m: [] PsbtMaps, map: int, index: int, t: int) -> int {
  k := [1] u8.[cast(u8) t];
  return psbt_find_vartime(m, map, index, k[..]);
}
// The Updater: key's value in map (index) becomes value, added when the
// key is new. False when the arena or the pairs are full.
psbt_set_vartime :: (m: [] PsbtMaps, map: int, index: int, key: [] u8, value: [] u8) -> bool #export_c {
  if m[0].used + key.count + value.count > PSBT_ARENA { return false; }
  j := psbt_find_vartime(m, map, index, key);
  if j < 0 {
    if m[0].n == PSBT_PAIRS { return false; }
    j = m[0].n; m[0].n += 1;
    m[0].pairs[j] = PsbtPair.{map=map, index=index, key=pm_copy(m, key), key_n=key.count};
  }
  m[0].pairs[j].value = pm_copy(m, value); m[0].pairs[j].value_n = value.count;
  return true;
}
psbt_set1_vartime :: (m: [] PsbtMaps, map: int, index: int, t: int, value: [] u8) -> bool {
  k := [1] u8.[cast(u8) t];
  return psbt_set_vartime(m, map, index, k[..], value);
}
// A read PSBT's pairs, in the order they came.
psbt_maps_vartime :: (b: [] u8, p: Psbt, fields: [] PsbtField, m: [] PsbtMaps) -> bool #export_c {
  if p.code != PSBT_OK { return false; }
  m[0].version = p.version; m[0].nin = p.nin; m[0].nout = p.nout; m[0].n = 0; m[0].used = 0;
  for j: 0..<p.nfields {
    f := fields[j];
    if !psbt_set_vartime(m, f.map, f.index, b[f.key..<f.key+f.key_n], b[f.value..<f.value+f.value_n]) { return false; }
  }
  return true;
}
```

`psbt_write_vartime` writes the pairs in the order Core's `Serialize`
writes them: by type in a fixed order for each map, then by key. Core
keys partial signatures by the HASH160 of their public key, so they sort
by that hash. Every published PSBT that reads is already in this order,
except one of Core's, whose pairs came in another order.

## The Creator and BIP-370's lock time

The Creator writes a version 0 PSBT for an unsigned transaction whose
scriptSigs are empty. For version 2, `psbt_unsigned_vartime` builds the
unsigned transaction from the fields. Its lock time follows BIP-370:
with no input requiring one, the fallback lock time; else the largest
required height if every such input accepts a height, else the largest
time; and no lock time at all when an input wants a height and another
a time:

<!-- source: coax/lib/bitcoin/psbt.coax creator -->
```jai
// The Creator: a version 0 PSBT for an unsigned transaction, which must
// read in the legacy form with every scriptSig empty.
psbt_create_vartime :: (unsigned: [] u8, m: [] PsbtMaps) -> bool #export_c {
  w: [1] PsbtRead;
  t := tx_read_vartime(unsigned, false, w[0].ins[..], w[0].outs[..], w[0].items[0..<0]);
  if t.code != TX_OK { return false; }
  for k: 0..<t.nin { if w[0].ins[k].script_n != 0 { return false; } }
  m[0].version = 0; m[0].nin = t.nin; m[0].nout = t.nout; m[0].n = 0; m[0].used = 0;
  return psbt_set1_vartime(m, PSBT_GLOBAL, 0, 0x00, unsigned);
}
// BIP-370's Determining Lock Time: with no input requiring one, the
// fallback (or 0); else the largest height when every input that
// requires a lock time accepts a height, else the largest time when every
// one accepts a time; -1 when neither.
psbt_locktime_vartime :: (m: [] PsbtMaps) -> int #export_c {
  height := 0; time := 0; any_height := true; any_time := true; seen := false;
  for i: 0..<m[0].nin {
    h := psbt_find1_vartime(m, PSBT_INPUT, i, 0x12); t := psbt_find1_vartime(m, PSBT_INPUT, i, 0x11);
    if h < 0 && t < 0 { continue; }
    seen = true;
    if h >= 0 { v := tx_le_vartime(pm_value(m, h), 0, 4); if v > height { height = v; } } else { any_height = false; }
    if t >= 0 { v := tx_le_vartime(pm_value(m, t), 0, 4); if v > time { time = v; } } else { any_time = false; }
  }
  if !seen {
    f := psbt_find1_vartime(m, PSBT_GLOBAL, 0, 0x03);
    return ifx f >= 0 then tx_le_vartime(pm_value(m, f), 0, 4) else 0;
  }
  if any_height { return height; }
  if any_time { return time; }
  return -1;
}
// The unsigned transaction into out: version 0's global one, or version
// 2's built from its fields (empty scriptSigs, sequences 0xffffffff
// unless given, BIP-370's lock time). Its length, or -1 when no lock time
// satisfies every input or out is too short.
psbt_unsigned_vartime :: (m: [] PsbtMaps, out: [] u8) -> int #export_c {
  if m[0].version < 2 {
    j := psbt_find1_vartime(m, PSBT_GLOBAL, 0, 0x00);
    v := pm_value(m, j);
    if v.count > out.count { return -1; }
    return tx_put(out, 0, v);
  }
  lt := psbt_locktime_vartime(m);
  if lt < 0 { return -1; }
  at := tx_put(out, 0, pm_value(m, psbt_find1_vartime(m, PSBT_GLOBAL, 0, 0x02)));
  at = tx_compact_write_vartime(out, at, m[0].nin);
  for i: 0..<m[0].nin {
    if at + 41 > out.count { return -1; }
    at = tx_put(out, at, pm_value(m, psbt_find1_vartime(m, PSBT_INPUT, i, 0x0e)));
    at = tx_put(out, at, pm_value(m, psbt_find1_vartime(m, PSBT_INPUT, i, 0x0f)));
    out[at] = 0; at += 1;
    s := psbt_find1_vartime(m, PSBT_INPUT, i, 0x10);
    if s >= 0 { at = tx_put(out, at, pm_value(m, s)); }
    else { for k: 0..<4 { out[at+k] = 0xff; } at += 4; }
  }
  at = tx_compact_write_vartime(out, at, m[0].nout);
  for i: 0..<m[0].nout {
    s := pm_value(m, psbt_find1_vartime(m, PSBT_OUTPUT, i, 0x04));
    if at + 17 + s.count > out.count { return -1; }
    at = tx_put(out, at, pm_value(m, psbt_find1_vartime(m, PSBT_OUTPUT, i, 0x03)));
    at = tx_compact_write_vartime(out, at, s.count);
    at = tx_put(out, at, s);
  }
  if at + 4 > out.count { return -1; }
  for k: 0..<4 { out[at+k] = cast(u8) ((lt >> (8 * k)) & 255); }
  return at + 4;
}
```

## The Signer

BIP-174 lists what a simple signer checks before it signs. The input
must have a UTXO. A witness UTXO alone isn't enough for a non-witness
input, because its amount isn't signed and a malicious updater could lie
about it. A redeemScript must hash to the P2SH script, a witnessScript to
the P2WSH program, and the hash type must be one the signer accepts. The
key must be in the script. Then it signs the input with chapter 37's
legacy or chapter 38's segwit v0 signature hash and adds a partial
signature: type `0x02`, keyed by the public key.

For a P2TR input the signer computes BIP-341's signature hash over every
input's spent output, tweaks the key with the input's Merkle root (none
for a key with no scripts), and adds a BIP-340 signature as type `0x13`.
It checks the signature under the output key, so a key that isn't the
input's is refused:

<!-- source: coax/lib/bitcoin/psbt.coax signer -->
```jai
// The Signer's results: ok, or the check that refused.
PSBT_SIGN_OK :: 0;
PSBT_SIGN_NO_UTXO :: 1;          // no UTXO for this input (or, for Taproot, for another)
PSBT_SIGN_WITNESS_UTXO :: 2;     // a witness UTXO for a non-witness input
PSBT_SIGN_REDEEM :: 3;           // the redeemScript doesn't match the scriptPubKey
PSBT_SIGN_WITNESS_SCRIPT :: 4;   // the witnessScript doesn't match, or isn't wanted
PSBT_SIGN_SIGHASH :: 5;          // a hash type the signer doesn't sign
PSBT_SIGN_NOT_MINE :: 6;         // the key isn't in the script
PSBT_SIGN_FINAL :: 7;            // the input is already finalized
PSBT_SIGN_LOCKTIME :: 8;         // no lock time satisfies every input
PSBT_SIGN_CAPACITY :: 9;         // a transaction or script larger than the workspace
PSBT_SIGN_REASONS :: [10] string.["ok", "no UTXO", "a witness UTXO for a non-witness input", "redeemScript does not match the scriptPubKey",
  "witnessScript does not match the scriptPubKey or redeemScript", "unacceptable sighash type", "key not in the script",
  "already finalized", "no lock time satisfies every input", "capacity"];
// The signer's workspace: the unsigned transaction and its parts, the
// spent outputs, and the two signers' workspaces.
PsbtSign :: struct {
  tx: [PSBT_TX] u8; ins: [PSBT_IO] TxIn; outs: [PSBT_IO] TxOut; items: [1] TxItem; t: Tx;
  pins: [PSBT_IO] TxIn; pouts: [PSBT_IO] TxOut; pitems: [PSBT_ITEMS] TxItem;
  amount: int; spk: [SCRIPT_MAX_SIZE] u8; spk_n: int; witness_utxo: bool;
  ecdsa: [1] EcdsaWork; tap: [1] BtTapWork;
}
// The output input n spends, into w.amount and w.spk: from its witness
// UTXO, else from its non-witness UTXO. False when it has neither.
ps_spent :: (m: [] PsbtMaps, n: int, w: [] PsbtSign) -> bool {
  u := psbt_find1_vartime(m, PSBT_INPUT, n, 0x01);
  s: [] u8;
  if u >= 0 {
    v := pm_value(m, u);
    w[0].amount = tx_le_vartime(v, 0, 8);
    c := tx_compact_read_vartime(v, 8);
    s = v[c.next..<c.next+c.value];
    w[0].witness_utxo = true;
  } else {
    p := psbt_find1_vartime(m, PSBT_INPUT, n, 0x00);
    if p < 0 { return false; }
    prev := pm_value(m, p);
    t := tx_read_vartime(prev, true, w[0].pins[..], w[0].pouts[..], w[0].pitems[..]);
    o := w[0].pouts[tx_le_vartime(w[0].tx[..], w[0].ins[n].outpoint + 32, 4)];
    w[0].amount = tx_le_vartime(prev, o.value, 8);
    s = prev[o.script..<o.script+o.script_n];
    w[0].witness_utxo = false;
  }
  if s.count > SCRIPT_MAX_SIZE { return false; }
  for k: 0..<s.count { w[0].spk[k] = s[k]; }
  w[0].spk_n = s.count;
  return true;
}
// ALL, NONE or SINGLE, with or without ANYONECANPAY.
ps_hashtype :: (h: int) -> bool { return (h >= 1 && h <= 3) || (h >= 0x81 && h <= 0x83); }
ps_p2wpkh :: (s: [] u8) -> bool { return s.count == 22 && s[0] == 0 && s[1] == 20; }
ps_p2wsh :: (s: [] u8) -> bool { return s.count == 34 && s[0] == 0 && s[1] == 32; }
ps_p2tr :: (s: [] u8) -> bool { return s.count == 34 && s[0] == 0x51 && s[1] == 32; }
ps_p2pkh :: (s: [] u8) -> bool { return s.count == 25 && s[0] == 0x76 && s[1] == 0xa9 && s[2] == 20 && s[23] == 0x88 && s[24] == 0xac; }
// The P2PKH script of a 20-byte hash into out.
ps_p2pkh_of :: (h: [] u8, out: [] u8) -> [] u8 {
  out[0] = 0x76; out[1] = 0xa9; out[2] = 20;
  for k: 0..<20 { out[3+k] = h[k]; }
  out[23] = 0x88; out[24] = 0xac;
  return out[0..<25];
}
ps_equal :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for k: 0..<a.count { if a[k] != b[k] { return false; } }
  return true;
}
// Whether needle occurs in s.
ps_contains :: (s: [] u8, needle: [] u8) -> bool {
  for k: 0..<s.count - needle.count + 1 { if ps_equal(s[k..<k+needle.count], needle) { return true; } }
  return false;
}
// BIP-174's simple signer for input n with one key, the checks it lists
// first: an ECDSA partial signature (types 02) for P2PKH, P2WPKH, P2SH
// and P2WSH scripts, a BIP-340 key-path signature (type 13) for P2TR. A
// partial signature the input already has for this key is kept, as Core
// keeps it. grind selects Core's low-R signatures (ecdsa_sign_low_r);
// BIP-174's worked example signs without it. aux is BIP-340's auxiliary
// randomness.
psbt_sign_vartime :: (m: [] PsbtMaps, n: int, key: [] u8, aux: [] u8, grind: bool, w: [] PsbtSign) -> int #export_c {
  if psbt_final_vartime(m, n) { return PSBT_SIGN_FINAL; }
  size := psbt_unsigned_vartime(m, w[0].tx[..]);
  if size < 0 { return PSBT_SIGN_LOCKTIME; }
  b := w[0].tx[0..<size];
  w[0].t = tx_read_vartime(b, false, w[0].ins[..], w[0].outs[..], w[0].items[0..<0]);
  if !ps_spent(m, n, w) { return PSBT_SIGN_NO_UTXO; }
  spk := w[0].spk[0..<w[0].spk_n];
  ht := psbt_find1_vartime(m, PSBT_INPUT, n, 0x03);
  if ps_p2tr(spk) { return ps_sign_taproot(m, n, b, key, aux, ht, w); }
  hashtype := ifx ht >= 0 then tx_le_vartime(pm_value(m, ht), 0, 4) else 1;
  if !ps_hashtype(hashtype) { return PSBT_SIGN_SIGHASH; }
  // The script the key signs: the redeemScript under P2SH, P2WPKH's
  // P2PKH form, the witnessScript under P2WSH.
  script := spk; witness := false; code: [25] u8;
  redeem := psbt_find1_vartime(m, PSBT_INPUT, n, 0x04);
  if redeem >= 0 {
    r := pm_value(m, redeem); h: [20] u8;
    #assert hash160_checked(r, h[..], false) == RMD_OK;
    if !script_p2sh_vartime(spk) || !ps_equal(h[..], spk[2..<22]) { return PSBT_SIGN_REDEEM; }
    script = r;
  }
  ws := psbt_find1_vartime(m, PSBT_INPUT, n, 0x05);
  if ps_p2wpkh(script) { script = ps_p2pkh_of(script[2..<22], code[..]); witness = true; }
  else if ps_p2wsh(script) {
    if ws < 0 { return PSBT_SIGN_WITNESS_SCRIPT; }
    h: [32] u8; #assert sha256_hash_checked(pm_value(m, ws), h[..], false) == SHA_OK;
    if !ps_equal(h[..], script[2..<34]) { return PSBT_SIGN_WITNESS_SCRIPT; }
    script = pm_value(m, ws); witness = true;
  } else if ws >= 0 { return PSBT_SIGN_WITNESS_SCRIPT; }
  if !witness && w[0].witness_utxo { return PSBT_SIGN_WITNESS_UTXO; }
  pub: [34] u8; pub[0] = 33;
  if !bip32_public(key, pub[1..<34]) { return PSBT_SIGN_NOT_MINE; }
  mine := false;
  if ps_p2pkh(script) { h: [20] u8; #assert hash160_checked(pub[1..<34], h[..], false) == RMD_OK; mine = ps_equal(h[..], script[3..<23]); }
  else { mine = ps_contains(script, pub[..]); }
  if !mine { return PSBT_SIGN_NOT_MINE; }
  pub[0] = 0x02;                                           // the key of a partial signature: type 02, then the public key
  if psbt_find_vartime(m, PSBT_INPUT, n, pub[..]) >= 0 { return PSBT_SIGN_OK; }
  sp := ScriptSpend.{b=b, t=w[0].t, ins=w[0].ins[..], outs=w[0].outs[..], items=w[0].items[0..<0], index=n, amount=w[0].amount, spk=spk};
  digest: [32] u8;
  if witness { script_sighash_v0_vartime(sp, script, hashtype, digest[..]); }
  else { script_sighash_legacy_vartime(sp, script, hashtype, digest[..]); }
  compact: [64] u8;
  signed := ifx grind then ecdsa_sign_low_r(key, digest[..], w[0].ecdsa[..], compact[..], false)
    else ecdsa_sign(key, digest[..], w[0].ecdsa[..], compact[..], false);
  if !signed { return PSBT_SIGN_NOT_MINE; }
  der: [73] u8;
  k := ecdsa_der(compact[..], der[..]);
  der[k] = cast(u8) hashtype;
  return ifx psbt_set_vartime(m, PSBT_INPUT, n, pub[..], der[0..<k+1]) then PSBT_SIGN_OK else PSBT_SIGN_CAPACITY;
}
// BIP-371's key-path signer: the hash type (SIGHASH_DEFAULT when none is
// given), every input's spent output for BIP-341's signature hash, then
// a signature by the key tweaked with the input's Merkle root (none for a
// key with no scripts). The key is the input's when the signature
// verifies under the scriptPubKey's output key.
ps_sign_taproot :: (m: [] PsbtMaps, n: int, b: [] u8, key: [] u8, aux: [] u8, ht: int, w: [] PsbtSign) -> int {
  hashtype := ifx ht >= 0 then tx_le_vartime(pm_value(m, ht), 0, 4) else 0;
  if hashtype != 0 && !ps_hashtype(hashtype) { return PSBT_SIGN_SIGHASH; }
  tp := bt_tx_read(b);
  if !tp.ok || m[0].nin > BT_IO { return PSBT_SIGN_CAPACITY; }
  amounts: [BT_IO] int; spks: [BT_IO] BtScript; output: [32] u8;
  for i: 0..<m[0].nin {
    if !ps_spent(m, i, w) { return PSBT_SIGN_NO_UTXO; }
    if w[0].spk_n > BT_SPK { return PSBT_SIGN_CAPACITY; }
    amounts[i] = w[0].amount; spks[i].n = w[0].spk_n;
    for k: 0..<w[0].spk_n { spks[i].b[k] = w[0].spk[k]; }
  }
  for k: 0..<32 { output[k] = spks[n].b[2+k]; }
  sighash: [32] u8;
  if !bt_sighash_taproot(b, tp, n, amounts[0..<m[0].nin], spks[0..<m[0].nin], hashtype, sighash[..]) { return PSBT_SIGN_SIGHASH; }
  merkle := psbt_find1_vartime(m, PSBT_INPUT, n, 0x18);
  root := ifx merkle >= 0 then pm_value(m, merkle) else key[0..<0];
  sig: [65] u8;
  if !bt_taproot_sign(key, root, sighash[..], aux, w[0].tap[..], sig[0..<64], false) { return PSBT_SIGN_NOT_MINE; }
  if !taproot_key_path_verify_vartime(output[..], sig[0..<64], sighash[..]) { return PSBT_SIGN_NOT_MINE; }
  sig[64] = cast(u8) hashtype;
  return ifx psbt_set1_vartime(m, PSBT_INPUT, n, 0x13, sig[0..<ifx hashtype == 0 then 64 else 65]) then PSBT_SIGN_OK else PSBT_SIGN_CAPACITY;
}
```

Core grinds its ECDSA signatures to a low R, which saves a byte about
half the time. It signs, and while the top bit of R is set, signs again
with a counter as libsecp256k1's 32 bytes of extra data, which chapter
20's RFC 6979 seeding now takes. tiny-secp256k1's `extraEntropy` rows
check that extra data, and Core's signer cases come out byte for byte.
BIP-174's worked example signs without grinding.

## The Combiner

The Combiner takes two PSBTs of one unsigned transaction and keeps every
pair of either. When both have a key, it keeps the first one's value, as
Core does:

<!-- source: coax/lib/bitcoin/psbt.coax combiner -->
```jai
// The Combiner: a's pairs, then each pair of b's whose key a lacks, into
// out. False when the two aren't PSBTs of one unsigned transaction.
psbt_combine_vartime :: (a: [] PsbtMaps, b: [] PsbtMaps, out: [] PsbtMaps) -> bool #export_c {
  ta: [PSBT_TX] u8; tb: [PSBT_TX] u8;
  na := psbt_unsigned_vartime(a, ta[..]); nb := psbt_unsigned_vartime(b, tb[..]);
  if na < 0 || !ps_equal(ta[0..<na], tb[0..<nb]) { return false; }
  out[0].version = a[0].version; out[0].nin = a[0].nin; out[0].nout = a[0].nout; out[0].n = 0; out[0].used = 0;
  for j: 0..<a[0].n { p := a[0].pairs[j]; if !psbt_set_vartime(out, p.map, p.index, pm_key(a, j), pm_value(a, j)) { return false; } }
  for j: 0..<b[0].n {
    p := b[0].pairs[j];
    if psbt_find_vartime(out, p.map, p.index, pm_key(b, j)) < 0 && !psbt_set_vartime(out, p.map, p.index, pm_key(b, j), pm_value(b, j)) { return false; }
  }
  return true;
}
```

## The Input Finalizer and the Transaction Extractor

The Input Finalizer turns each input's partial signatures into its final
scriptSig (type `0x07`) and witness (type `0x08`). Once an input is
final, the writer leaves out the pairs that only signing needs. P2PKH
and P2WPKH take a signature and its key. An m-of-n `CHECKMULTISIG` takes
the empty dummy and m signatures in the script's key order. Under P2SH
the redeemScript follows, and under P2WSH the witnessScript. A P2TR key
path takes its one signature:

<!-- source: coax/lib/bitcoin/psbt.coax finalizer -->
```jai
// A push of data, in the shortest form for its length.
pf_push :: (data: [] u8, out: [] u8, at: int) -> int {
  pos := at;
  if data.count == 0 { out[pos] = 0; return pos + 1; }
  if data.count < 0x4c { out[pos] = cast(u8) data.count; pos += 1; }
  else if data.count <= 0xff { out[pos] = 0x4c; out[pos+1] = cast(u8) data.count; pos += 2; }
  else { out[pos] = 0x4d; out[pos+1] = cast(u8) (data.count & 255); out[pos+2] = cast(u8) (data.count >> 8); pos += 3; }
  return tx_put(out, pos, data);
}
// The partial signature of public key k (33 or 65 bytes), or -1.
pf_sig :: (m: [] PsbtMaps, n: int, k: [] u8) -> int {
  key: [66] u8; key[0] = 0x02;
  for x: 0..<k.count { key[1+x] = k[x]; }
  return psbt_find_vartime(m, PSBT_INPUT, n, key[0..<1+k.count]);
}
// The stack that satisfies script from input n's partial signatures, as
// the indexes of the pairs it pushes (a -2 is the public key, a -3 the
// empty dummy): P2PKH takes a signature and its key, k-of-n
// CHECKMULTISIG the dummy and k signatures in the script's key order.
// Returns the stack's size, or -1.
pf_satisfy :: (m: [] PsbtMaps, n: int, script: [] u8, stack: [] int, key: [] int) -> int {
  if ps_p2pkh(script) {
    for j: 0..<m[0].n {
      p := m[0].pairs[j];
      if p.map != PSBT_INPUT || p.index != n || pm_type(m, j) != 0x02 { continue; }
      h: [20] u8; k := pm_key(m, j);
      #assert hash160_checked(k[1..<k.count], h[..], false) == RMD_OK;
      if ps_equal(h[..], script[3..<23]) { stack[0] = j; stack[1] = -2; key[0] = j; return 2; }
    }
    return -1;
  }
  if script.count < 3 || script[script.count-1] != 0xae || script[0] < 0x51 || script[0] > 0x60 { return -1; }
  need := cast(int) script[0] - 0x50;
  at := 1; keys := 0; got := 0;
  stack[0] = -3;
  while at < script.count - 2 {
    size := cast(int) script[at];
    if (size != 33 && size != 65) || at + 1 + size > script.count - 2 { return -1; }
    j := pf_sig(m, n, script[at+1..<at+1+size]);
    if j >= 0 && got < need { got += 1; stack[got] = j; }
    keys += 1; at += 1 + size;
  }
  if at != script.count - 2 || cast(int) script[at] != 0x50 + keys || need > keys { return -1; }
  return ifx got == need then need + 1 else -1;
}
// The stack's items: a pair's value, the public key in a P2PKH spend
// (its partial signature's key without the type), or nothing.
pf_item :: (m: [] PsbtMaps, item: int, key: int) -> [] u8 {
  if item == -2 { k := pm_key(m, key); return k[1..<k.count]; }
  if item == -3 { return m[0].arena[0..<0]; }
  return pm_value(m, item);
}
// The Input Finalizer for input n: its final scriptSig (type 07) and
// witness (type 08) from its partial signatures and scripts, for P2PKH,
// P2WPKH, multisig, P2SH, P2WSH and P2SH-P2WSH, and from its key-path
// signature for P2TR. True when the input is final.
psbt_finalize_input_vartime :: (m: [] PsbtMaps, n: int, w: [] PsbtSign) -> bool #export_c {
  if psbt_final_vartime(m, n) { return true; }
  size := psbt_unsigned_vartime(m, w[0].tx[..]);
  if size < 0 { return false; }
  w[0].t = tx_read_vartime(w[0].tx[0..<size], false, w[0].ins[..], w[0].outs[..], w[0].items[0..<0]);
  if !ps_spent(m, n, w) { return false; }
  spk := w[0].spk[0..<w[0].spk_n];
  wit: [PSBT_TX] u8;
  if ps_p2tr(spk) {
    s := psbt_find1_vartime(m, PSBT_INPUT, n, 0x13);
    if s < 0 { return false; }
    at := tx_compact_write_vartime(wit[..], 0, 1);
    at = tx_compact_write_vartime(wit[..], at, pm_value(m, s).count);
    at = tx_put(wit[..], at, pm_value(m, s));
    return psbt_set1_vartime(m, PSBT_INPUT, n, 0x08, wit[0..<at]);
  }
  script := spk; p2sh := script_p2sh_vartime(spk); code: [25] u8;
  redeem := psbt_find1_vartime(m, PSBT_INPUT, n, 0x04);
  if p2sh {
    if redeem < 0 { return false; }
    script = pm_value(m, redeem);
  }
  stack: [32] int; key: [1] int; sig: [PSBT_TX] u8; sn := 0; wn := 0; count := 0; ws := -1;
  if ps_p2wpkh(script) {
    count = pf_satisfy(m, n, ps_p2pkh_of(script[2..<22], code[..]), stack[..], key[..]);
  } else if ps_p2wsh(script) {
    ws = psbt_find1_vartime(m, PSBT_INPUT, n, 0x05);
    if ws < 0 { return false; }
    count = pf_satisfy(m, n, pm_value(m, ws), stack[..], key[..]);
  } else {
    count = pf_satisfy(m, n, script, stack[..], key[..]);
    if count < 0 { return false; }
    for k: 0..<count { sn = pf_push(pf_item(m, stack[k], key[0]), sig[..], sn); }
    count = -2;                                               // no witness
  }
  if count == -1 { return false; }
  if count >= 0 {
    wn = tx_compact_write_vartime(wit[..], 0, count + ifx ws >= 0 then 1 else 0);
    for k: 0..<count {
      item := pf_item(m, stack[k], key[0]);
      wn = tx_compact_write_vartime(wit[..], wn, item.count); wn = tx_put(wit[..], wn, item);
    }
    if ws >= 0 { wn = tx_compact_write_vartime(wit[..], wn, pm_value(m, ws).count); wn = tx_put(wit[..], wn, pm_value(m, ws)); }
  }
  if p2sh { sn = pf_push(script, sig[..], sn); }
  if sn > 0 && !psbt_set1_vartime(m, PSBT_INPUT, n, 0x07, sig[0..<sn]) { return false; }
  if wn > 0 && !psbt_set1_vartime(m, PSBT_INPUT, n, 0x08, wit[0..<wn]) { return false; }
  return true;
}
// Every input; true when all are final.
psbt_finalize_vartime :: (m: [] PsbtMaps, w: [] PsbtSign) -> bool #export_c {
  all := true;
  for n: 0..<m[0].nin { if !psbt_finalize_input_vartime(m, n, w) { all = false; } }
  return all;
}
```

The Transaction Extractor puts each final scriptSig and witness into the
unsigned transaction:

<!-- source: coax/lib/bitcoin/psbt.coax extractor -->
```jai
// The Transaction Extractor: the unsigned transaction with each input's
// final scriptSig and witness, in BIP-144's form when any input has a
// witness. Its length in out, or -1 when an input isn't final or out is
// too short.
psbt_extract_vartime :: (m: [] PsbtMaps, out: [] u8, w: [] PsbtSign) -> int #export_c {
  size := psbt_unsigned_vartime(m, w[0].tx[..]);
  if size < 0 { return -1; }
  b := w[0].tx[0..<size];
  t := tx_read_vartime(b, false, w[0].ins[..], w[0].outs[..], w[0].items[0..<0]);
  segwit := false;
  for i: 0..<t.nin {
    if !psbt_final_vartime(m, i) { return -1; }
    if psbt_find1_vartime(m, PSBT_INPUT, i, 0x08) >= 0 { segwit = true; }
  }
  need := size + 2;
  for j: 0..<m[0].n { need += m[0].pairs[j].value_n + 9; }
  if need > out.count { return -1; }
  at := tx_put(out, 0, b[0..<4]);
  if segwit { out[at] = 0; out[at+1] = 1; at += 2; }
  at = tx_compact_write_vartime(out, at, t.nin);
  for i: 0..<t.nin {
    at = tx_put(out, at, b[w[0].ins[i].outpoint..<w[0].ins[i].outpoint+36]);
    s := psbt_find1_vartime(m, PSBT_INPUT, i, 0x07);
    script := ifx s >= 0 then pm_value(m, s) else b[0..<0];
    at = tx_compact_write_vartime(out, at, script.count); at = tx_put(out, at, script);
    at = tx_put(out, at, b[w[0].ins[i].sequence..<w[0].ins[i].sequence+4]);
  }
  outputs := ifx t.nin > 0 then w[0].ins[t.nin-1].sequence + 4 else 5;   // the output count, then the outputs
  at = tx_put(out, at, b[outputs..<t.locktime]);
  if segwit {
    for i: 0..<t.nin {
      s := psbt_find1_vartime(m, PSBT_INPUT, i, 0x08);
      if s >= 0 { at = tx_put(out, at, pm_value(m, s)); } else { out[at] = 0; at += 1; }
    }
  }
  return tx_put(out, at, b[t.locktime..<t.total]);
}
```

## The example

The example is [bitcoin/examples/psbt.coax](../examples/psbt.coax). It
reads every published PSBT, runs BIP-174's worked example
from its master key, signs BIP-174's signer-check cases, replays Core's
role cases, and then makes our own spend:

<!-- source: bitcoin/examples/psbt.coax example -->
```jai
#load "../../coax/lib/bitcoin/psbt.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

put_hex :: (b: [] u8) {
  text: [256] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
yes :: (b: bool) -> string { return ifx b then "yes" else "no"; }
plural :: (n: int, one: string, many: string) -> string { return ifx n == 1 then one else many; }
// Core's message for a refusal.
put_reason :: (p: Psbt) {
  if p.code == PSBT_KEY_SIZE { print("Size of key was not % for the type %", psbt_key_size_vartime(p.map, p.type), psbt_key_name_vartime(p.map, p.type)); }
  else { print("%", PSBT_REASONS[p.code]); }
}
// A Base64 PSBT read into m; its Psbt.
Reader :: struct { raw: [8192] u8; fields: [128] PsbtField; w: [1] PsbtRead; }
load :: (text: string, rd: [] Reader, m: [] PsbtMaps) -> Psbt {
  code: [1] int;
  b := psbt_base64_vartime(text, rd[0].raw[..], code[..]);
  if code[0] != PSBT_OK { return Psbt.{code=code[0]}; }
  p := psbt_read_vartime(b, rd[0].fields[..], rd[0].w[..]);
  if p.code == PSBT_OK { #assert psbt_maps_vartime(b, p, rd[0].fields[..], m); }
  return p;
}
// Whether m, written, is the PSBT whose hex or Base64 is want.
same_hex :: (m: [] PsbtMaps, want: string) -> bool {
  out: [8192] u8; w: [8192] u8;
  n := psbt_write_vartime(m, out[..]);
  return n > 0 && ps_equal(out[0..<n], psbt_hex(want, w[..]));
}
same_base64 :: (m: [] PsbtMaps, want: string) -> bool {
  out: [8192] u8; w: [8192] u8; code: [1] int;
  n := psbt_write_vartime(m, out[..]);
  return n > 0 && ps_equal(out[0..<n], psbt_base64_vartime(want, w[..], code[..]));
}
// The key at m/a'/b'/c' below the BIP-174 master key, into key.
derive :: (path: [3] int, key: [] u8) {
  work: [BIP32_WORK_BYTES] u8; master: [BIP32_NODE_BYTES] u8; node: [BIP32_NODE_BYTES] u8;
  #assert bip32_decode_ct(PSBT_BIP174_TPRV, work[..], master[..]) == BIP32_NODE_BYTES;
  steps := [3] int.[path[0] + BIP32_HARDENED, path[1] + BIP32_HARDENED, path[2] + BIP32_HARDENED];
  #assert bip32_descend_ct(master[..], steps[..], work[..], node[..]) == BIP32_NODE_BYTES;
  for k: 0..<32 { key[k] = node[HDK_KEY + 1 + k]; }
}

main :: () {
  rd: [1] Reader; m: [1] PsbtMaps; w: [1] PsbtSign;

  // Every published PSBT: Core's verdict, and what reads.
  print("Every published PSBT, read as Bitcoin Core reads it:\n");
  kinds := [4] string.["invalid", "valid", "valid, fails the signer's checks", "lock-time case"];
  refused := 0; differ := 0;
  for i: 0..<PSBT_CASES.count {
    c := PSBT_CASES[i];
    p := load(c.text, rd[..], m[..]);
    print("  % % (%): ", i + 1, c.source, kinds[c.kind]);
    if p.code != PSBT_OK { put_reason(p); refused += 1; }
    else {
      out: [8192] u8;
      n := psbt_write_vartime(m[..], out[..]);
      print("v%, % %, % %, % %, written back %", p.version, p.nin, plural(p.nin, "input", "inputs"), p.nout,
        plural(p.nout, "output", "outputs"), p.nfields, plural(p.nfields, "pair", "pairs"), ifx ps_equal(out[0..<n], rd[0].raw[0..<n]) then "the same" else "in Core's order");
      if c.kind == PSBT_CASE_LOCKTIME {
        lt := psbt_locktime_vartime(m[..]);
        if lt < 0 { print("; no lock time satisfies every input"); } else { print("; lock time %", lt); }
      }
    }
    print("\n");
    if (p.code == PSBT_OK) != (c.kind != PSBT_CASE_INVALID) || p.code != c.code { differ += 1; }
  }
  print("  % PSBTs: % refused, % read; % differ from the published verdicts\n", PSBT_CASES.count, refused, PSBT_CASES.count - refused, differ);

  // BIP-174's worked example, from its master key.
  print("BIP-174's worked example, from its master key %:\n", PSBT_BIP174_TPRV);
  work: [BIP32_WORK_BYTES] u8; master: [BIP32_NODE_BYTES] u8; fp: [4] u8;
  #assert bip32_decode_ct(PSBT_BIP174_TPRV, work[..], master[..]) == BIP32_NODE_BYTES;
  #assert bip32_fingerprint_ct(master[..], work[..], fp[..]) == 4;
  all := true;
  for i: 0..<PSBT_BIP174_PATHS.count {
    key: [32] u8; pub: [33] u8; want: [33] u8;
    derive(PSBT_BIP174_PATHS[i].path, key[..]);
    #assert bip32_public(key[..], pub[..]);
    if !ps_equal(pub[..], psbt_hex(PSBT_BIP174_PATHS[i].key, want[..])) { all = false; }
  }
  print("  the master key's fingerprint: "); put_hex(fp[..]);
  print("; its six public keys at their paths are the published ones: %\n", yes(all));
  all = true;
  for i: 0..<PSBT_BIP174_SIGNERS.count {
    key: [32] u8; want: [32] u8;
    derive(PSBT_BIP174_SIGNERS[i].path, key[..]);
    if !ps_equal(key[..], psbt_hex(PSBT_BIP174_SIGNERS[i].key, want[..])) { all = false; }
  }
  print("  the signers' keys at their paths are their WIFs': %\n", yes(all));
  tx: [256] u8;
  #assert psbt_create_vartime(psbt_bip174_unsigned(tx[..]), m[..]);
  print("  Creator: %\n", yes(same_hex(m[..], PSBT_BIP174_RESULTS[0])));
  update(m[..], fp[..]);
  print("  Updater: %\n", yes(same_hex(m[..], PSBT_BIP174_RESULTS[1])));
  all1 := [4] u8.[1, 0, 0, 0];
  for n: 0..<m[0].nin { #assert psbt_set1_vartime(m[..], PSBT_INPUT, n, 0x03, all1[..]); }
  print("  SIGHASH_ALL on every input: %\n", yes(same_hex(m[..], PSBT_BIP174_RESULTS[2])));
  signed: [2] PsbtMaps;
  for s: 0..<2 {
    signed[s] = m[0];
    for k: 0..<2 {
      key: [32] u8; aux: [32] u8;
      derive(PSBT_BIP174_SIGNERS[2*s+k].path, key[..]);
      for n: 0..<m[0].nin {
        r := psbt_sign_vartime(signed[s..<s+1], n, key[..], aux[..], false, w[..]);
        print("  Signer %, key m/0'/0'/%', input %: %\n", s + 1, PSBT_BIP174_SIGNERS[2*s+k].path[2], n, PSBT_SIGN_REASONS[r]);
      }
    }
    print("  Signer %: %\n", s + 1, yes(same_hex(signed[s..<s+1], PSBT_BIP174_RESULTS[3+s])));
  }
  both: [1] PsbtMaps; other: [1] PsbtMaps;
  #assert psbt_combine_vartime(signed[0..<1], signed[1..<2], both[..]) && psbt_combine_vartime(signed[1..<2], signed[0..<1], other[..]);
  print("  Combiner, in either order: %\n", yes(same_hex(both[..], PSBT_BIP174_RESULTS[5]) && same_hex(other[..], PSBT_BIP174_RESULTS[5])));
  print("  Input Finalizer: final %, %\n", yes(psbt_finalize_vartime(both[..], w[..])), yes(same_hex(both[..], PSBT_BIP174_RESULTS[6])));
  net: [8192] u8; want: [8192] u8;
  n := psbt_extract_vartime(both[..], net[..], w[..]);
  print("  Transaction Extractor: %, % bytes\n", yes(ps_equal(net[0..<n], psbt_hex(PSBT_BIP174_RESULTS[7], want[..]))), n);
  combine_hex(PSBT_BIP174_RESULTS[8], PSBT_BIP174_RESULTS[9], PSBT_BIP174_RESULTS[10], "  Combiner, keys only one side has");
  combine_hex(PSBT_BIP174_RESULTS[11], PSBT_BIP174_RESULTS[12], PSBT_BIP174_RESULTS[13], "  Combiner, a key both sides have");

  // BIP-174's signer checks, each input signed with the first signer's key.
  print("BIP-174's signer checks, each input signed with m/0'/0'/0':\n");
  key0: [32] u8; aux: [32] u8;
  derive(PSBT_BIP174_SIGNERS[0].path, key0[..]);
  for i: 0..<PSBT_CASES.count {
    if PSBT_CASES[i].kind != PSBT_CASE_SIGNER { continue; }
    p := load(PSBT_CASES[i].text, rd[..], m[..]);
    print("  case %:", i + 1);
    for k: 0..<p.nin { print(" input %, %;", k, PSBT_SIGN_REASONS[psbt_sign_vartime(m[..], k, key0[..], aux[..], true, w[..])]); }
    print("\n");
  }

  // Core's cases for the other roles.
  print("Bitcoin Core's role cases (rpc_psbt.json):\n");
  for i: 0..<PSBT_CORE_SIGNERS.count {
    c := PSBT_CORE_SIGNERS[i];
    p := load(c.psbt, rd[..], m[..]);
    keys: [64] u8;
    #assert psbt_hex(c.keys, keys[..]).count == c.keys.count / 2;
    print("  walletprocesspsbt %:", i + 1);
    for k: 0..<c.keys.count / 64 { for x: 0..<p.nin { print(" %", psbt_sign_vartime(m[..], x, keys[32*k..<32*k+32], aux[..], true, w[..])); } }
    print("; the published result: %\n", yes(same_base64(m[..], c.result)));
  }
  for i: 0..<PSBT_CORE_COMBINERS.count {
    c := PSBT_CORE_COMBINERS[i];
    a: [1] PsbtMaps; b: [1] PsbtMaps;
    #assert load(c.a, rd[..], a[..]).code == PSBT_OK && load(c.b, rd[..], b[..]).code == PSBT_OK;
    #assert psbt_combine_vartime(a[..], b[..], both[..]) && psbt_combine_vartime(b[..], a[..], other[..]);
    print("  combinepsbt %, in either order: %\n", i + 1, yes(same_base64(both[..], c.result) && same_base64(other[..], c.result)));
  }
  #assert load(PSBT_CORE_FINALIZE, rd[..], m[..]).code == PSBT_OK;
  print("  finalizepsbt: final %, %\n", yes(psbt_finalize_vartime(m[..], w[..])), yes(same_base64(m[..], PSBT_CORE_FINALIZED)));
  #assert load(PSBT_CORE_EXTRACT, rd[..], m[..]).code == PSBT_OK;
  n = psbt_extract_vartime(m[..], net[..], w[..]);
  print("  finalizepsbt with extract: %\n", yes(ps_equal(net[0..<n], psbt_hex(PSBT_CORE_EXTRACTED, want[..]))));

  // Our own spend: P2WPKH, P2WSH 2-of-2 and a P2TR key path.
  own(w[..]);
}
```

```shell
coax/bin/coax run bitcoin/examples/psbt.coax
```

```text
Every published PSBT, read as Bitcoin Core reads it:
  1 BIP-174 (invalid): Invalid PSBT magic bytes
  2 BIP-174 (invalid): Outputs provided does not match the number of outputs in transaction.
  3 BIP-174 (invalid): Unsigned tx does not have empty scriptSigs and scriptWitnesses.
...
  7 BIP-174 (invalid): Size of key was not 1 for the type Input Witness UTXO
  8 BIP-174 (invalid): Size of key was not the expected size for the type partial signature pubkey
...
  29 BIP-174 (valid): v0, 0 inputs, 0 outputs, 1 pair, written back the same
...
  58 BIP-370 and Core (invalid): Required height based locktime is invalid (0)
...
  74 BIP-370 (lock-time case): v2, 2 inputs, 1 output, 12 pairs, written back the same; lock time 10000
...
  155 PSBTs: 90 refused, 65 read; 0 differ from the published verdicts
```

Of the 155, 34 come from BIP-174, 47 from BIP-370, 17 from BIP-371 and
57 from Core alone; 4 of the BIPs' cases are also Core's. Nine BIP-370
cases give a lock time, among them the one where no lock time satisfies
every input.

BIP-174's worked example starts from a testnet master key. The example
derives each key at its path with chapter 32's BIP-32 and checks it
against the published public keys and WIF keys, then runs each role and
compares each result with the published one:

```text
BIP-174's worked example, from its master key tprv8ZgxMBicQKsPd9TeAdPADNnSyH9SSUUbTVeFszDE23Ki6TBB5nCefAdHkK8Fm3qMQR6sHwA56zqRmKmxnHk37JkiFzvncDqoKmPWubu7hDF:
  the master key's fingerprint: d90c6a4f; its six public keys at their paths are the published ones: yes
  the signers' keys at their paths are their WIFs': yes
  Creator: yes
  Updater: yes
  SIGHASH_ALL on every input: yes
  Signer 1, key m/0'/0'/0', input 0: ok
  Signer 1, key m/0'/0'/0', input 1: key not in the script
...
  Signer 2: yes
  Combiner, in either order: yes
  Input Finalizer: final yes, yes
  Transaction Extractor: yes, 628 bytes
  Combiner, keys only one side has: yes
  Combiner, a key both sides have: yes
```

BIP-174's four signer-check PSBTs read, but a signer must refuse to sign
one of their inputs. Our signer refuses each for the reason BIP-174
gives:

```text
BIP-174's signer checks, each input signed with m/0'/0'/0':
  case 31: input 0, a witness UTXO for a non-witness input; input 1, key not in the script;
  case 32: input 0, redeemScript does not match the scriptPubKey; input 1, key not in the script;
  case 33: input 0, ok; input 1, redeemScript does not match the scriptPubKey;
  case 34: input 0, ok; input 1, witnessScript does not match the scriptPubKey or redeemScript;
Bitcoin Core's role cases (rpc_psbt.json):
  walletprocesspsbt 1: 0 6 6 0; the published result: yes
...
  finalizepsbt: final yes, yes
  finalizepsbt with extract: yes
```

### Our own spend

The last part spends three coins: a P2WPKH coin of key A, a P2WSH 2-of-2
coin of keys A and B, and a P2TR coin of key C with no scripts. Their
previous transactions are made up, so the spend can't be broadcast, but
each input is checked the way a node checks it: chapter 36's interpreter
for the segwit v0 inputs and chapter 41's key-path check for the P2TR
one.

<!-- source: bitcoin/examples/psbt.coax own -->
```jai
// Three coins, one each of P2WPKH (key A), P2WSH 2-of-2 (keys A and B)
// and P2TR with no scripts (key C), spent to one P2WPKH output: created,
// updated with the coins and the witness script, signed, finalized,
// extracted, and each input verified the way a node verifies it.
OWN_KEYS :: [3] string.["1111111111111111111111111111111111111111111111111111111111111111",
  "2222222222222222222222222222222222222222222222222222222222222222", "3333333333333333333333333333333333333333333333333333333333333333"];
OWN_AMOUNTS :: [3] int.[50000, 60000, 70000];
own :: (w: [] PsbtSign) {
  print("Our own spend of P2WPKH, P2WSH 2-of-2 and P2TR coins:\n");
  keys: [3][32] u8; pubs: [3][33] u8; xonly: [32] u8; spks: [3][34] u8; spk_n: [3] int; ws: [71] u8;
  for k: 0..<3 { #assert psbt_hex(OWN_KEYS[k], keys[k][..]).count == 32 && bip32_public(keys[k][..], pubs[k][..]); }
  spk_n[0] = p2wpkh_script_vartime(pubs[0][..], spks[0][..]);
  ws[0] = 0x52; ws[1] = 33; ws[35] = 33; ws[69] = 0x52; ws[70] = 0xae;    // 2 <A> <B> 2 CHECKMULTISIG
  for k: 0..<33 { ws[2+k] = pubs[0][k]; ws[36+k] = pubs[1][k]; }
  spk_n[1] = p2wsh_script_vartime(ws[..], spks[1][..]);
  for k: 0..<32 { xonly[k] = pubs[2][1+k]; }
  spk_n[2] = p2tr_script_vartime(xonly[..], spks[2][..]);
  // The unsigned transaction: three inputs (txids 01..01, 02..02 and
  // 03..03, output 0), one output paying 179,000 to key B's P2WPKH.
  tx: [256] u8; out: [22] u8;
  tx[0] = 2; at := tx_compact_write_vartime(tx[..], 4, 3);
  for i: 0..<3 {
    for k: 0..<32 { tx[at+k] = cast(u8) (i + 1); }
    for k: 0..<4 { tx[at+32+k] = 0; tx[at+37+k] = 0xff; }
    tx[at+36] = 0; at += 41;
  }
  at = tx_compact_write_vartime(tx[..], at, 1);
  total := 179000;
  for k: 0..<8 { tx[at+k] = cast(u8) ((total >> (8 * k)) & 255); }
  n := p2wpkh_script_vartime(pubs[1][..], out[..]);
  at = tx_compact_write_vartime(tx[..], at + 8, n); at = tx_put(tx[..], at, out[0..<n]);
  for k: 0..<4 { tx[at+k] = 0; }
  unsigned := tx[0..<at+4];
  m: [1] PsbtMaps; buf: [4096] u8;
  #assert psbt_create_vartime(unsigned, m[..]);
  for i: 0..<3 {
    u: [64] u8;
    for k: 0..<8 { u[k] = cast(u8) ((OWN_AMOUNTS[i] >> (8 * k)) & 255); }
    u[8] = cast(u8) spk_n[i]; for k: 0..<spk_n[i] { u[9+k] = spks[i][k]; }
    #assert psbt_set1_vartime(m[..], PSBT_INPUT, i, 0x01, u[0..<9+spk_n[i]]);
  }
  #assert psbt_set1_vartime(m[..], PSBT_INPUT, 1, 0x05, ws[..]);
  print("  Creator and Updater: % bytes\n", psbt_write_vartime(m[..], buf[..]));
  aux: [32] u8;
  inputs := [4] int.[0, 1, 1, 2]; signer := [4] int.[0, 0, 1, 2];   // each input, and the key that signs it
  names := [3] string.["A", "B", "C"];
  for s: 0..<4 {
    i := inputs[s]; k := signer[s];
    print("  Signer, input % with key %: %\n", i, names[k], PSBT_SIGN_REASONS[psbt_sign_vartime(m[..], i, keys[k][..], aux[..], true, w)]);
  }
  print("  key A on the P2TR input: %\n", PSBT_SIGN_REASONS[psbt_sign_vartime(m[..], 2, keys[0][..], aux[..], true, w)]);
  print("  signed: % bytes\n", psbt_write_vartime(m[..], buf[..]));
  print("  Input Finalizer: final %, % bytes\n", yes(psbt_finalize_vartime(m[..], w)), psbt_write_vartime(m[..], buf[..]));
  net: [8192] u8; ins: [4] TxIn; outs: [4] TxOut; items: [16] TxItem;
  size := psbt_extract_vartime(m[..], net[..], w);
  b := net[0..<size];
  t := tx_read_vartime(b, true, ins[..], outs[..], items[..]);
  id: [32] u8; rev: [32] u8;
  #assert tx_txid_vartime(b, t, id[..]);
  for k: 0..<32 { rev[k] = id[31-k]; }
  print("  Transaction Extractor: % bytes, weight %, txid ", size, tx_weight_vartime(t)); put_hex(rev[..]); print("\n");
  // Each input as a node checks it: chapter 39's interpreter for the
  // segwit v0 inputs, chapter 43's key-path check for the P2TR one.
  sw: [1] ScriptWork;
  flags := SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_WITNESS | SCRIPT_VERIFY_DERSIG | SCRIPT_VERIFY_LOW_S | SCRIPT_VERIFY_STRICTENC
    | SCRIPT_VERIFY_NULLDUMMY | SCRIPT_VERIFY_CLEANSTACK | SCRIPT_VERIFY_NULLFAIL | SCRIPT_VERIFY_WITNESS_PUBKEYTYPE;
  for i: 0..<2 {
    sp := ScriptSpend.{b=b, t=t, ins=ins[..], outs=outs[..], items=items[..], index=i, amount=OWN_AMOUNTS[i], spk=spks[i][0..<spk_n[i]]};
    e := script_verify_vartime(sp, flags, sw[..]);
    print("  input %: %\n", i, SCRIPT_ERR_NAMES[e]);
  }
  tp := bt_tx_read(unsigned);
  amounts: [3] int; spent: [3] BtScript; sighash: [32] u8;
  for i: 0..<3 { amounts[i] = OWN_AMOUNTS[i]; spent[i].n = spk_n[i]; for k: 0..<spk_n[i] { spent[i].b[k] = spks[i][k]; } }
  #assert bt_sighash_taproot(unsigned, tp, 2, amounts[..], spent[..], BT_DEFAULT, sighash[..]);
  sig := b[items[ins[2].first].at..<items[ins[2].first].at+items[ins[2].first].n];
  print("  input 2: a % byte key-path signature, valid %\n", sig.count, yes(taproot_key_path_verify_vartime(spks[2][2..<34], sig, sighash[..])));
}
```

```text
Our own spend of P2WPKH, P2WSH 2-of-2 and P2TR coins:
  Creator and Updater: 377 bytes
  Signer, input 0 with key A: ok
  Signer, input 1 with key A: ok
  Signer, input 1 with key B: ok
  Signer, input 2 with key C: ok
  key A on the P2TR input: key not in the script
  signed: 765 bytes
  Input Finalizer: final yes, 703 bytes
  Transaction Extractor: 557 bytes, weight 1049, txid ee66dd2123fc1821ea8eed61051e5f0bab8b0d2e52f5ed6421fc21de90d83d1d
  input 0: OK
  input 1: OK
  input 2: a 64 byte key-path signature, valid yes
```

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/psbt.coax](../examples/checks/psbt.coax),
which loads the example:

- every published PSBT that reads, written in Core's order, reads again
  and writes the same bytes;
- BIP-174's updated PSBT cut short at any byte refuses, and whole it
  reads.

<!-- source: bitcoin/examples/checks/psbt.coax check -->
```jai
// Every published PSBT that reads, written in Core's order, reads again
// and writes the same bytes: writing settles after one pass.
#forall "a written PSBT reads back and writes the same" i: 0..<PSBT_CASES.count #steps 2000000000 {
  rd: [1] Reader; m: [1] PsbtMaps; again: [1] PsbtMaps;
  if load(PSBT_CASES[i].text, rd[..], m[..]).code == PSBT_OK {
    out: [8192] u8; twice: [8192] u8; f: [128] PsbtField; rw: [1] PsbtRead;
    n := psbt_write_vartime(m[..], out[..]);
    b := out[0..<n];
    p := psbt_read_vartime(b, f[..], rw[..]);
    #assert p.code == PSBT_OK && psbt_maps_vartime(b, p, f[..], again[..]);
    #assert psbt_write_vartime(again[..], twice[..]) == n && ps_equal(twice[0..<n], b);
  }
}
// BIP-174's updated PSBT cut short at any byte refuses; whole, it reads.
#forall "a PSBT cut short refuses" k: 0..<1000 #steps 4000000000 {
  raw: [2048] u8; f: [64] PsbtField; rw: [1] PsbtRead;
  b := psbt_hex(PSBT_BIP174_RESULTS[1], raw[..]);
  if k < b.count { #assert psbt_read_vartime(b[0..<k], f[..], rw[..]).code != PSBT_OK; }
  else if k == b.count { #assert psbt_read_vartime(b, f[..], rw[..]).code == PSBT_OK; }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/psbt.coax
```

`coax test` runs 478 checks, including the loaded files' own checks, in about 230 CPU seconds on 2026-11-03.

### The library's own checks

`psbt.coax` is an `#authority` file. Its checks run the Creator against
BIP-174's first PSBT, the codes against their messages, the smallest
PSBTs (magic, separators, counts, versions), compressed, uncompressed
and hybrid public keys, Taproot trees, every published case with its
code (and, for a key of the wrong size, its map and type), BIP-370's
lock times, Core's signer, combiner, finalizer and extractor cases,
tiny-secp256k1's `extraEntropy` rows, the signer's refusals, and a P2TR
key-path signature that verifies under the output key.

```shell
coax/bin/coax test coax/lib/bitcoin/psbt.coax
```

`coax test` runs 474 checks, including those of the files it loads, in about 185 CPU seconds on 2026-11-03. Mutation testing: not run (owner speed order).

### Python

[psbt_reference.py](../../coax/tests/bitcoin/psbt_reference.py)
transliterates Core's reader and each role into Python.
[make_psbt_vectors.py](../../coax/tests/bitcoin/make_psbt_vectors.py)
copies every published PSBT from the pinned BIP texts and
`rpc_psbt.json`, decides each one in Python and checks that verdict
against the published one, replays Core's role cases, recomputes
tiny-secp256k1's `extraEntropy` signatures, and writes
`psbt_vectors.coax`. [psbt.py](../../coax/tests/bitcoin/psbt.py) checks
that `psbt.coax`'s messages and codes are Python's, derives BIP-174's
keys with its own BIP-32, verifies our own spend with
`script_reference.py`'s interpreter and a BIP-340 check, and writes the
transcript the example must print.

```shell
python3 -I coax/tests/bitcoin/make_psbt_vectors.py
python3 -I coax/tests/bitcoin/psbt.py --coax coax/bin/coax
```

On 2026-11-03 all 155 cases agree with their published verdicts, all 207 lines of the transcript agree with Python, and the example's output in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- BIP-370's Constructor, which adds inputs and outputs while the
  modifiable flags allow it.
- Script-path Taproot signing (type `0x14`) and finalizing: chapters 42
  and 44 build script paths, and chapter 100 signs `multi_a` leaves.
- BIP-373's MuSig2 fields are read and checked but not used: chapter
  101.
- Coin selection and change: chapter 95. A wallet that uses PSBTs:
  chapter 99.

## Summary

- A PSBT is a magic, then a global map and one map per input and per
  output, each a list of typed key-value pairs.
- Version 0 carries the unsigned transaction; version 2 carries its parts
  as fields, and BIP-370 decides its lock time.
- Core refuses duplicate keys, keys of the wrong length, values of the
  wrong size, bad public keys and signatures, and fields the version
  doesn't allow.
- The Creator, Updater, Signer, Combiner, Input Finalizer and
  Transaction Extractor each add one thing.
- A signer checks the UTXO, the scripts and the hash type before it
  signs, and Core grinds ECDSA signatures to a low R.

## Dependencies

- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)
- [38. Segwit v0 signature hashes](ch038-segwit-v0-signature-hashes.md)
- [43. Taproot signature hashes](ch043-taproot-signature-hashes.md)

## Sources

- [BIP-174](https://github.com/bitcoin/bips/blob/master/bip-0174.mediawiki)
  (PSBT version 0),
  [BIP-370](https://github.com/bitcoin/bips/blob/master/bip-0370.mediawiki)
  (version 2) and
  [BIP-371](https://github.com/bitcoin/bips/blob/master/bip-0371.mediawiki)
  (Taproot fields), pinned in
  [coax/tests/vectors/upstream/](../../coax/tests/vectors/upstream/bip174/bip-0174.mediawiki).
- Bitcoin Core: `psbt.h`, `psbt.cpp`, `pubkey.h`,
  `test/functional/data/rpc_psbt.json` and
  `wallet/test/psbt_wallet_tests.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/psbt.h).
- tiny-secp256k1's `ecdsa.json` (`extraEntropy` rows), pinned in
  [coax/tests/vectors/upstream/tiny-secp256k1/](../../coax/tests/vectors/upstream/tiny-secp256k1/ecdsa.json).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
