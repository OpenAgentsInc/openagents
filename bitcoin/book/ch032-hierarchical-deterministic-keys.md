# 32. Hierarchical deterministic keys

A wallet that made a new random key for every address would need a new
backup after every payment. BIP-32 makes all of a wallet's keys from one
64-byte seed instead: a tree of keys in which each node can make 2^32
children, and the seed alone rebuilds every one. This chapter derives the
master node from a seed with HMAC-SHA512 (chapters 7 and 8), derives
hardened and normal children from a private node and normal children from
a public one, computes fingerprints with HASH160 (chapter 6), and reads and
writes the 78-byte serialization as `xprv`, `xpub`, `tprv` and `tpub`
text in Base58Check (chapter 26). It shows by hand why hardened derivation
exists: a public node and one normal child's private key give away the
parent's private key. Private derivation follows chapter 3's rules. The
code is checked against BIP-32's test vectors 1 to 5, the `xprv` column of
chapter 31's pinned BIP-39 vectors and a separate Python implementation,
natively and in Wasm.

## What this chapter teaches

- The master node: `I = HMAC-SHA512(key = "Bitcoin seed", data = seed)`,
  the key `I_L` and the chain code `I_R`.
- CKDpriv and CKDpub: a child's key is the parent's key plus `I_L`, as a
  scalar mod `n` or as a point, and its chain code is `I_R`.
- Hardened children (index `2^31` and up), which hash the parent's private
  key, and normal children, which hash its public key, so a public node can
  derive them too.
- The leak: a public node and one normal child's private key give the
  parent's private key, and with it every key below. Worked by hand.
- Fingerprints, the first 4 bytes of HASH160 of a public key.
- The 78 bytes: version, depth, parent fingerprint, index, chain code and
  key; the four versions; and the Base58Check text, always 111
  characters.
- The invalid cases: `I_L` of `n` or more and a zero key, BIP-32's rule to
  move to the next index, and every invalid key in test vector 5.
- What runs in constant time, and what is public.

## Where Bitcoin uses hierarchical keys

| Step | Input | Output | Chapter |
| --- | --- | --- | --- |
| Restore | the words and the passphrase | the 64-byte seed | 31 |
| Make the master | the seed | the master node `m` | 32 |
| Derive | a node and a list of indexes | a node below it | 32 |
| Choose accounts | a path string such as `m/84'/0'/0'/0/0` | the indexes | 33 |
| Share a watch-only wallet | an account's private node | its `xpub` | 32 |
| Make addresses | a public key | an address | 29 |

A watch-only wallet, such as a merchant's server, holds an account's
`xpub`. It derives a fresh receiving address for every payment and never
holds a private key. The signing device holds the `xprv`, or only the seed.

The Nostr book's chapter 46 uses the same code: NIP-06 takes a Nostr key
from a BIP-39 phrase at `m/44'/1237'/0'/0/0`.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [bip32.coax](../../coax/lib/crypto/bip32.coax) | `bip32_master_ct(seed, testnet, work, node)` | the master node of a 16- to 64-byte seed |
| | `bip32_ckd_ct(parent, index, work, child)` | one child: CKDpriv for a private node, CKDpub for a public one |
| | `bip32_descend_ct(parent, indexes, work, node)` | the node at a list of indexes |
| | `bip32_neuter_ct(node, work, out)`, `bip32_fingerprint_ct(node, work, out)` | the public node, and the 4-byte fingerprint |
| | `bip32_encode_ct(node, work, out)`, `bip32_decode_ct(text, work, node)` | the node as 111 characters of Base58Check, and back with every check |
| | `bip32_public`, `bip32_master`, `bip32_child`, `bip32_path` | the older `bool` interface that NIP-06, the website log-in, the CLI and the Nostr code use; they call the procedures above |
| [sha512.coax](../../coax/lib/crypto/sha512.coax) | `hmac_sha512(key, msg, out)` | `I` (chapters 7 and 8) |
| [secp_mult_const.coax](../../coax/lib/crypto/secp_mult_const.coax), [secp_mult.coax](../../coax/lib/crypto/secp_mult.coax) | `smc_multiply`, `ecmult_vartime` | `k·G` in constant time, and `I_L·G + K` on public data (chapter 16) |
| [secp_encode.coax](../../coax/lib/crypto/secp_encode.coax) | `se_encode`, `se_decode` | the 33-byte compressed public key (chapter 17) |
| [hash160.coax](../../coax/lib/bitcoin/hash160.coax) | `hash160_checked` | the fingerprint's hash (chapter 6) |
| [base58.coax](../../coax/lib/bitcoin/base58.coax) | `b58_digit_ct`, `b58_char_ct`, `bitcoin_checksum_checked` | the alphabet in constant time and the checksum (chapter 26) |

Each `_ct` procedure takes a caller-owned work buffer of
`BIP32_WORK_BYTES` (326) bytes. The secret intermediates live there:
`I`, the message HMAC hashes, the child's key, the bytes being turned into
Base58 and the node being derived. The procedure fills it with zeros before
it returns, on success and on every refusal after it starts using it. Each
returns the number of bytes or characters written, or one of ten negative
refusals, and writes its output only on success:

<!-- source: coax/lib/crypto/bip32.coax codes -->
```jai
BIP32_HARDENED :: 2147483648;     // 2^31: this index and the ones above are hardened
BIP32_NODE_BYTES :: 78;           // version, depth, parent fingerprint, index, chain code, key
BIP32_TEXT :: 111;                // the Base58Check length of every node with these versions
BIP32_XPRV :: 0x0488ade4;         // mainnet private, "xprv"
BIP32_XPUB :: 0x0488b21e;         // mainnet public, "xpub"
BIP32_TPRV :: 0x04358394;         // testnet, signet and regtest private, "tprv"
BIP32_TPUB :: 0x043587cf;         // testnet, signet and regtest public, "tpub"

// Refusals.
BIP32_BAD_SIZE :: -1;             // a seed that isn't 16 to 64 bytes, or a node that isn't 78
BIP32_BAD_INDEX :: -2;            // an index below 0 or above 2^32 - 1
BIP32_INVALID_KEY :: -3;          // I_L >= n, a key of 0 or >= n, a point off the curve or at infinity
BIP32_NEEDS_PRIVATE :: -4;        // a hardened child of a public node
BIP32_CAPACITY :: -5;             // an output or work buffer that is too small
BIP32_BAD_TEXT :: -6;             // not 111 characters, or one outside the Base58 alphabet
BIP32_BAD_CHECKSUM :: -7;         // the last 4 bytes aren't the SHA-256d checksum
BIP32_BAD_VERSION :: -8;          // a version other than the four above
BIP32_BAD_PREFIX :: -9;           // a key byte the version doesn't allow
BIP32_BAD_DEPTH :: -10;           // depth 0 with a parent or an index, or a child past depth 255
```

## The node: 78 bytes

BIP-32 calls a node an *extended key*: a key and a 32-byte chain code. A
private node holds the private key `k`, and a public node the public key
`K = k·G`. Serialized, a node also says where it sits in the tree. This
code keeps every node as those 78 bytes, in a buffer the caller owns:

<!-- source: coax/lib/crypto/bip32.coax layout -->
```jai
// The node's fields, at these offsets.
HDK_VERSION :: 0;                 // 4 bytes, big-endian
HDK_DEPTH :: 4;                   // 1 byte: 0 for the master
HDK_PARENT :: 5;                  // 4 bytes: the parent's fingerprint, 0 for the master
HDK_INDEX :: 9;                   // 4 bytes, big-endian: this node's index, 0 for the master
HDK_CHAIN :: 13;                  // 32 bytes
HDK_KEY :: 45;                    // 33 bytes: 0x00 then k, or the compressed public key
// The work buffer: HMAC-SHA512's output I, its message, the parent's public
// key, the child's key, Base58's payload and checksum, and the node being
// derived.
HDK_I :: 0;
HDK_DATA :: 64;
HDK_PUB :: 101;
HDK_CHILD :: 134;
HDK_TEXT :: 166;
HDK_NODE :: 248;
BIP32_WORK_BYTES :: 326;
```

| Bytes | Field | The master | A child |
| --- | --- | --- | --- |
| 0–3 | version | `0488ade4` (xprv) or `04358394` (tprv) | the parent's |
| 4 | depth | 0 | the parent's plus 1 |
| 5–8 | parent fingerprint | `00000000` | the parent's fingerprint |
| 9–12 | index | 0 | its index, big-endian |
| 13–44 | chain code | `I_R` of the seed | `I_R` of its derivation |
| 45–77 | key | `00` and `k` | `00` and `k`, or `K` compressed |

A private key is 32 bytes, so it gets a `00` byte in front, and a public
key is the 33-byte compressed form of chapter 17, which starts `02` or
`03`. The first key byte tells the two kinds apart, and the version must
agree with it. The version is public, so these checks may branch:

<!-- source: coax/lib/crypto/bip32.coax kind -->
```jai
// 1 for a private version, 0 for a public one, or BIP32_BAD_VERSION.
// Versions are public.
hdk_kind :: (version: int) -> int {
  if version == BIP32_XPRV || version == BIP32_TPRV { return 1; }
  if version == BIP32_XPUB || version == BIP32_TPUB { return 0; }
  return BIP32_BAD_VERSION;
}
// The kind of a node, after checking that its key byte fits its version:
// 0x00 before a private key, 0x02 or 0x03 for a public one.
hdk_check :: (node: [] u8) -> int {
  kind := hdk_kind(hdk_get32(node, HDK_VERSION));
  if kind < 0 { return kind; }
  p := node[HDK_KEY];
  if kind == 1 && p != 0 { return BIP32_BAD_PREFIX; }
  if kind == 0 && p != 2 && p != 3 { return BIP32_BAD_PREFIX; }
  return kind;
}
```

## The master node

```text
I = HMAC-SHA512(key = "Bitcoin seed", data = seed)
k = I_L (the first 32 bytes), as a number below n
c = I_R (the last 32 bytes)
```

The seed is 16 to 64 bytes. BIP-39's seed (chapter 31) is 64. The key of
the HMAC is the same fixed string for every wallet; the secret is the
seed. If `I_L` is 0 or `n` or more, the seed has no master node; that
happens with a probability below 1 in 2^127, and no seed is known that
does it.

Two scalar steps do the work for the master and for every private child.
`hdk_point_ct` multiplies with chapter 16's constant-time code, and
`hdk_tweak_ct` adds `I_L` to a key mod `n` and refuses the two cases that
BIP-32 skips:

<!-- source: coax/lib/crypto/bip32.coax scalar -->
```jai
// The compressed public key k*G of a 32-byte private key into out[0..<33],
// with chapter 16's constant-time multiplication. A key of 0 or n or more
// loads as 0, whose product is the point at infinity, which has no
// encoding: the answer is false and out is unwritten. That answer is
// public.
hdk_point_ct :: (key: [] u8, out: [] u8) -> bool {
  k := sc_load(key);
  return se_encode(smc_multiply(k.value, sg_generator()).value, SE_COMPRESSED, out) == SE_COMPRESSED;
}
// The child key I_L + k mod n into out[0..<32]. Returns 32, or
// BIP32_INVALID_KEY when I_L is n or more or the sum is 0; BIP-32 then
// says to use the next index. The two checks become one flag, and the
// procedure branches once, on it.
hdk_tweak_ct :: (il: [] u8, key: [] u8, out: [] u8) -> int {
  t := sc_load(il); k := sc_load(key);
  sum := sc_add(t.value, k.value);
  bad := (ifx t.ok then cast(u64) 0 else 1) | (ifx sc_is_zero(sum) then cast(u64) 1 else 0);  // mutant: equivalent (any nonzero flag refuses)
  if bad != 0 { return BIP32_INVALID_KEY; }
  #assert sc_store(sum, out);
  return 32;
}
```

The master is the tweak with a parent key of 0: `0 + I_L` is refused
exactly when `I_L` is 0 or `n` or more.

<!-- source: coax/lib/crypto/bip32.coax master -->
```jai
// The master node of a seed into work's node slot: I = HMAC-SHA512 with the
// key "Bitcoin seed" over the seed, k = I_L and the chain code I_R. I_L of
// 0 or n or more is refused, which is the tweak's rule with a parent key
// of 0.
hdk_master :: (seed: [] u8, testnet: bool, work: [] u8) -> int {
  node := work[HDK_NODE..<HDK_NODE + BIP32_NODE_BYTES]; i := work[HDK_I..<HDK_DATA];  // mutant: equivalent (HMAC fills the first 64 bytes of any longer slice)
  label := "Bitcoin seed"; key: [12] u8; zero: [32] u8;
  for j: 0..<12 { key[j] = label[j]; }
  #assert hmac_sha512(key[..], seed, i);
  result := hdk_tweak_ct(i[0..<32], zero[..], node[HDK_KEY + 1..<BIP32_NODE_BYTES]);
  if result < 0 { return result; }  // mutant: equivalent (a result is 32 or negative)
  hdk_put32(node, HDK_VERSION, ifx testnet then BIP32_TPRV else BIP32_XPRV);
  for j: HDK_DEPTH..<HDK_CHAIN { node[j] = 0; }   // mutant: equivalent (depth, parent and index; byte 13 is the chain code's first, written next)
  for j: 0..<32 { node[HDK_CHAIN + j] = i[32 + j]; }
  node[HDK_KEY] = 0;
  return BIP32_NODE_BYTES;
}
```

### Worked example: test vector 1

Test vector 1's seed is the 16 bytes `000102…0f`. HMAC-SHA512 with the key
`Bitcoin seed` gives:

```text
I_L = e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35
I_R = 873dff81c02f525623fd1fe5167eac3a55a049de3d314bb42ee227ffed37d508
```

`I_L` is below `n`, so it is the master key, and the 78 bytes are the
version `0488ade4`, depth 0, fingerprint `00000000`, index 0, `I_R` and
`00 ‖ I_L`. In Base58Check they are the published master:

```text
xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi
```

## Child keys

A child at index `i`, from 0 to `2^32 - 1`, hashes the parent's chain code
with the parent's key and `ser32(i)`, the index as 4 bytes, big-endian:

```text
hardened, i ≥ 2^31:  I = HMAC-SHA512(key = c_par, data = 00 ‖ k_par ‖ ser32(i))
normal,   i < 2^31:  I = HMAC-SHA512(key = c_par, data = K_par ‖ ser32(i))
private child:       k_i = I_L + k_par mod n
public child:        K_i = I_L·G + K_par
chain code:          c_i = I_R
```

Because `(I_L + k_par)·G = I_L·G + K_par`, a normal child's public key is
the same whether it comes from the private parent or from the public one.
That is what lets a watch-only wallet make addresses. Paths write a
hardened index with an apostrophe or `H`: `0'` and `0H` are both index
`2^31`, and `44'` is `2^31 + 44`.

Every child records its parent's fingerprint, its depth and its index:

<!-- source: coax/lib/crypto/bip32.coax finish -->
```jai
// The parts every child shares: its parent's fingerprint (the first 4
// bytes of HASH160 of the parent's public key, chapter 6), depth + 1, its
// index and its chain code I_R.
hdk_finish :: (node: [] u8, index: int, parent: [] u8, chain: [] u8, work: [] u8) {
  hash := work[HDK_DATA..<HDK_DATA + 20];  // mutant: equivalent (HASH160 fills the first 20 bytes of any longer slice)
  #assert hash160_checked(parent, hash, false) == RMD_OK;
  node[HDK_DEPTH] = cast(u8) (cast(int) node[HDK_DEPTH] + 1);
  for j: 0..<4 { node[HDK_PARENT + j] = hash[j]; }  // mutant: equivalent (a fifth byte lands in the index, written next)
  hdk_put32(node, HDK_INDEX, index);
  for j: 0..<32 { node[HDK_CHAIN + j] = chain[j]; }
}
```

### CKDpriv

The data a hardened child hashes, `00 ‖ k_par`, is exactly the key field of
the parent's 78 bytes. A normal child hashes `K_par`, which the code
computes anyway, because the fingerprint needs it:

<!-- source: coax/lib/crypto/bip32.coax ckdpriv -->
```jai
// CKDpriv: the child at index of the private node in work's node slot,
// in place. A hardened index (2^31 and up) hashes 0x00 || k, which is the
// node's key field as stored; a normal index hashes the public key k*G.
// The parent's public key is computed either way, for the fingerprint.
hdk_ckd_priv_ct :: (index: int, work: [] u8) -> int {
  node := work[HDK_NODE..<HDK_NODE + BIP32_NODE_BYTES]; i := work[HDK_I..<HDK_DATA];  // mutant: equivalent (HMAC fills the first 64 bytes of any longer slice)
  data := work[HDK_DATA..<HDK_PUB]; pub := work[HDK_PUB..<HDK_CHILD];
  if !hdk_point_ct(node[HDK_KEY + 1..<BIP32_NODE_BYTES], pub) { return BIP32_INVALID_KEY; }
  if index >= BIP32_HARDENED { for j: 0..<33 { data[j] = node[HDK_KEY + j]; } }
  else { for j: 0..<33 { data[j] = pub[j]; } }
  hdk_put32(data, 33, index);
  #assert hmac_sha512(node[HDK_CHAIN..<HDK_KEY], data, i);  // mutant: equivalent (for a private node, a 33-byte key ending in its 00 byte is HMAC's own zero padding)
  result := hdk_tweak_ct(i[0..<32], node[HDK_KEY + 1..<BIP32_NODE_BYTES], work[HDK_CHILD..<HDK_TEXT]);  // mutant: equivalent (the sum fills the first 32 bytes of any longer slice)
  if result < 0 { return result; }  // mutant: equivalent (a result is 32 or negative)
  hdk_finish(node, index, pub, i[32..<64], work);
  for j: 0..<32 { node[HDK_KEY + 1 + j] = work[HDK_CHILD + j]; }
  return BIP32_NODE_BYTES;
}
```

The branch on the index is public: the index is in the child's own 78
bytes. The parent's key decides no branch and no address. `hdk_point_ct`
returns false only for a key of 0 or `n` or more, which the result reports
anyway.

### CKDpub

A public node can derive only normal children: a hardened one needs
`k_par`. Everything CKDpub reads, the parent's public key, its chain code
and the index, is in the `xpub`, so it uses chapter 16's faster
variable-time multiplication:

<!-- source: coax/lib/crypto/bip32.coax ckdpub -->
```jai
// I_L*G + K for public I_L and K, in variable time (chapter 16's public
// multiplication), into out[0..<33]. Returns 33, or BIP32_INVALID_KEY for
// I_L of n or more, a K that isn't on the curve, or the point at
// infinity.
hdk_add_point :: (il: [] u8, key: [] u8, out: [] u8) -> int {
  t := sc_load(il); k := se_decode(key);
  if !t.ok || !k.ok { return BIP32_INVALID_KEY; }
  p := ecmult_vartime(t.value, k.value, sc_small(1));
  if se_encode(p.value, SE_COMPRESSED, out) != SE_COMPRESSED { return BIP32_INVALID_KEY; }
  return 33;
}
// CKDpub: the child at a normal index of the public node in work's node
// slot, in place. Everything it reads is public. A hardened child needs
// the parent's private key, so it refuses.
hdk_ckd_pub :: (index: int, work: [] u8) -> int {
  if index >= BIP32_HARDENED { return BIP32_NEEDS_PRIVATE; }
  node := work[HDK_NODE..<HDK_NODE + BIP32_NODE_BYTES]; i := work[HDK_I..<HDK_DATA];  // mutant: equivalent (HMAC fills the first 64 bytes of any longer slice)
  data := work[HDK_DATA..<HDK_PUB]; pub := work[HDK_PUB..<HDK_CHILD];
  for j: 0..<33 { pub[j] = node[HDK_KEY + j]; data[j] = pub[j]; }
  hdk_put32(data, 33, index);
  #assert hmac_sha512(node[HDK_CHAIN..<HDK_KEY], data, i);
  result := hdk_add_point(i[0..<32], pub, node[HDK_KEY..<BIP32_NODE_BYTES]);
  if result < 0 { return result; }  // mutant: equivalent (a result is 33 or negative)
  hdk_finish(node, index, pub, i[32..<64], work);
  return BIP32_NODE_BYTES;
}
```

### One call for both

`bip32_ckd_ct` copies the parent into the work buffer and picks CKDpriv or
CKDpub by the parent's kind. `bip32_descend_ct` takes a list of indexes
and derives one level at a time. Path strings such as `m/84'/0'/0'` are
chapter 33's; here a path is the list of numbers.

<!-- source: coax/lib/crypto/bip32.coax derive -->
```jai
// One child of the node in work's node slot, in place: CKDpriv for a
// private node, CKDpub for a public one. The kind is public.
hdk_ckd :: (index: int, work: [] u8) -> int {
  node := work[HDK_NODE..<HDK_NODE + BIP32_NODE_BYTES];
  kind := hdk_check(node);
  if kind < 0 { return kind; }
  if node[HDK_DEPTH] == 255 { return BIP32_BAD_DEPTH; }
  if kind == 1 { return hdk_ckd_priv_ct(index, work); }
  return hdk_ckd_pub(index, work);
}
// The master node of a 16- to 64-byte seed into node, xprv or, for the
// test networks, tprv. Returns 78 or a refusal, and writes node only on
// success. work comes back as zeros.
bip32_master_ct :: (seed: [] u8, testnet: bool, work: [] u8, node: [] u8) -> int #export_c {
  if seed.count < 16 || seed.count > 64 { return BIP32_BAD_SIZE; }
  if node.count < BIP32_NODE_BYTES || work.count < BIP32_WORK_BYTES { return BIP32_CAPACITY; }
  result := hdk_master(seed, testnet, work);
  if result > 0 { for j: 0..<BIP32_NODE_BYTES { node[j] = work[HDK_NODE + j]; } }  // mutant: equivalent (a result is 78 or negative)
  hdk_wipe(work);
  return result;
}
// The child at index (0 to 2^32 - 1) of parent into child, which may be
// parent itself. Returns 78 or a refusal, and writes child only on
// success. work comes back as zeros, except after BIP32_BAD_SIZE,
// BIP32_CAPACITY and BIP32_BAD_INDEX, which refuse before using it.
bip32_ckd_ct :: (parent: [] u8, index: int, work: [] u8, child: [] u8) -> int #export_c {
  if parent.count != BIP32_NODE_BYTES { return BIP32_BAD_SIZE; }
  if child.count < BIP32_NODE_BYTES || work.count < BIP32_WORK_BYTES { return BIP32_CAPACITY; }
  if index < 0 || index > 4294967295 { return BIP32_BAD_INDEX; }
  for j: 0..<BIP32_NODE_BYTES { work[HDK_NODE + j] = parent[j]; }
  result := hdk_ckd(index, work);
  if result > 0 { for j: 0..<BIP32_NODE_BYTES { child[j] = work[HDK_NODE + j]; } }  // mutant: equivalent (a result is 78 or negative)
  hdk_wipe(work);
  return result;
}
// The node at a list of indexes below parent, into node. An empty list
// gives parent back after checking its version and key byte.
bip32_descend_ct :: (parent: [] u8, indexes: [] int, work: [] u8, node: [] u8) -> int #export_c {
  if parent.count != BIP32_NODE_BYTES { return BIP32_BAD_SIZE; }
  if node.count < BIP32_NODE_BYTES || work.count < BIP32_WORK_BYTES { return BIP32_CAPACITY; }
  for index: indexes { if index < 0 || index > 4294967295 { return BIP32_BAD_INDEX; } }
  for j: 0..<BIP32_NODE_BYTES { work[HDK_NODE + j] = parent[j]; }
  result := hdk_check(work[HDK_NODE..<HDK_NODE + BIP32_NODE_BYTES]);
  if result >= 0 { result = BIP32_NODE_BYTES; }
  for index: indexes { if result > 0 { result = hdk_ckd(index, work); } }  // mutant: equivalent (a result is 78 or negative)
  if result > 0 { for j: 0..<BIP32_NODE_BYTES { node[j] = work[HDK_NODE + j]; } }  // mutant: equivalent (a result is 78 or negative)
  hdk_wipe(work);
  return result;
}
```

A child can't go below depth 255, because the depth is one byte; the node
at depth 255 refuses with `BIP32_BAD_DEPTH`. The refusals that come before
the work buffer is used (`BIP32_BAD_SIZE`, `BIP32_CAPACITY` and
`BIP32_BAD_INDEX`) leave it as it was; every other result returns it as
zeros.

## Why hardened derivation exists

For a normal child, `k_i = I_L + k_par mod n`, and `I_L` comes from the
parent's public key, chain code and the index, all of which are in the
parent's `xpub`. So anyone who holds the `xpub` and learns one normal
child's private key can subtract:

```text
k_par = k_i - I_L mod n
```

With `k_par` and the chain code they hold the parent's `xprv`, and so
every child below it, hardened ones included.

### Worked example: N(m/0H) and m/0H/1's key

In test vector 1, the public node `N(m/0H)` holds:

```text
K   = 035a784662a4a20a65bf6aab9ae98a6c068a81c52e4b032c0fb5400c706cfccc56
c   = 47fdacbd0f1097043b78c63c20c34ef4ed9a111d980047ad16282c7ae6236141
```

1. `I = HMAC-SHA512(c, K ‖ 00000001)`. Its first half is
   `I_L = 4eb9d78157bae7a24115001621c4d91e3a3110e11e143c5259eaa4e55c5ec4bf`.
2. The private key of `m/0H/1` is
   `k_1 = 3c6cb8d0f6a264c91ea8b5030fadaa8e538b020f0a387421a12de9319dc93368`.
3. `k_1` is smaller than `I_L`, so `k_1 - I_L` is negative, and adding
   `n` brings it back into range:
   `edb2e14f9ee77d26dd93b4ecede8d16ed408ce149b6cd80b0715a2d911a0afea`.
4. That is the private key of `m/0H`, the published one. Put back into the
   `xpub`'s 78 bytes with the version `0488ade4` and the key byte `00`, it
   encodes to `m/0H`'s published `xprv`.

The example does these four steps with `hmac_sha512` and `sc_sub`, and then
derives `m/0H/5H` from the rebuilt node.

The same subtraction for a hardened child gives nothing: `I_L` of `m/0H`
hashed `00 ‖ k_m`, which no `xpub` holds, so `N(m)` and `m/0H`'s key give a
wrong answer. A leaked hardened child exposes only itself and the nodes
below it.

That is why BIP-44 and the paths of chapter 33 harden the levels above an
account (`m/84'/0'/0'`) and use normal derivation only for the addresses
inside it (`…/0/5`). The account's `xpub` can go to a watch-only wallet, and
a leak of one address key, together with that `xpub`, costs the account but
not the other accounts or the seed.

## Fingerprints

A node's fingerprint is the first 4 bytes of HASH160 of its compressed
public key. Every child carries its parent's fingerprint in bytes 5–8, so
software can tell which node a key came from without the parent's key.
Fingerprints are short and can collide; BIP-32 says they identify, they
don't prove. Test vector 1's master public key
`0339a36013301597daef41fbe593a02cc513d0b55527ec2df1050e2e8ff49c85c2`
hashes to `3442193e1bb70916e914552172cd4e2dbc9df811`, and `m/0H` starts
its parent fingerprint field with `3442193e`.

`N()` is BIP-32's name for the public node of a private one. It changes
the version (`xprv` to `xpub`, `tprv` to `tpub`) and replaces `00 ‖ k` with
`k·G`, and leaves the other fields alone. The fingerprint of a private node
is the fingerprint of its public node:

<!-- source: coax/lib/crypto/bip32.coax neuter -->
```jai
// N(): the public node of a private one, into out: the version becomes
// xpub or tpub and the key k*G. A public node comes back as it is.
hdk_neuter_ct :: (work: [] u8) -> int {
  node := work[HDK_NODE..<HDK_NODE + BIP32_NODE_BYTES]; pub := work[HDK_PUB..<HDK_CHILD];  // mutant: equivalent (k*G fills the first 33 bytes of any longer slice)
  kind := hdk_check(node);
  if kind < 0 { return kind; }
  if kind == 1 {
    if !hdk_point_ct(node[HDK_KEY + 1..<BIP32_NODE_BYTES], pub) { return BIP32_INVALID_KEY; }
    private := hdk_get32(node, HDK_VERSION);
    hdk_put32(node, HDK_VERSION, ifx private == BIP32_XPRV then BIP32_XPUB else BIP32_TPUB);
    for j: 0..<33 { node[HDK_KEY + j] = pub[j]; }
  }
  return BIP32_NODE_BYTES;
}
bip32_neuter_ct :: (node: [] u8, work: [] u8, out: [] u8) -> int #export_c {
  if node.count != BIP32_NODE_BYTES { return BIP32_BAD_SIZE; }
  if out.count < BIP32_NODE_BYTES || work.count < BIP32_WORK_BYTES { return BIP32_CAPACITY; }
  for j: 0..<BIP32_NODE_BYTES { work[HDK_NODE + j] = node[j]; }
  result := hdk_neuter_ct(work);
  if result > 0 { for j: 0..<BIP32_NODE_BYTES { out[j] = work[HDK_NODE + j]; } }  // mutant: equivalent (a result is 78 or negative)
  hdk_wipe(work);
  return result;
}
// A node's fingerprint, the first 4 bytes of HASH160 of its public key,
// into out[0..<4]. Its children carry it as their parent fingerprint.
bip32_fingerprint_ct :: (node: [] u8, work: [] u8, out: [] u8) -> int #export_c {
  if node.count != BIP32_NODE_BYTES { return BIP32_BAD_SIZE; }
  if out.count < 4 || work.count < BIP32_WORK_BYTES { return BIP32_CAPACITY; }
  for j: 0..<BIP32_NODE_BYTES { work[HDK_NODE + j] = node[j]; }
  result := hdk_neuter_ct(work);
  if result > 0 {  // mutant: equivalent (a result is 78 or negative)
    hash := work[HDK_DATA..<HDK_DATA + 20];  // mutant: equivalent (HASH160 fills the first 20 bytes of any longer slice)
    #assert hash160_checked(work[HDK_NODE + HDK_KEY..<HDK_NODE + BIP32_NODE_BYTES], hash, false) == RMD_OK;
    for j: 0..<4 { out[j] = hash[j]; }
    result = 4;
  }
  hdk_wipe(work);
  return result;
}
```

## Serialization

The text of a node is the Base58Check (chapter 26) of its 78 bytes: the
bytes, then the first 4 bytes of their SHA-256d as a checksum, as one
base-58 number. The version's first bytes fix the start of the text:

| Version | Network | Key | Text starts |
| --- | --- | --- | --- |
| `0488ade4` | mainnet | private | `xprv` |
| `0488b21e` | mainnet | public | `xpub` |
| `04358394` | testnet, signet and regtest | private | `tprv` |
| `043587cf` | testnet, signet and regtest | public | `tpub` |

All four versions start with the byte `04`. An 82-byte value that starts
with one of them is at least `58^110` and below `58^111`, so the text is
always 111 characters with no leading `1`. That fixed length lets the
encoder run 111 divisions by 58 over all 82 bytes, the way chapter 26's WIF
encoder does, with every alphabet lookup reading the whole table. An `xprv`
is a secret, so the encoder never branches on a byte or a digit:

<!-- source: coax/lib/crypto/bip32.coax encode -->
```jai
// The 78 bytes and their 4-byte SHA-256d checksum (chapter 5) in Base58,
// in constant time, as chapter 26's WIF encoder does it. The four versions
// all start 0x04, and every 82-byte value that starts with one of them is
// at least 58^110 and below 58^111, so the text is always 111 characters.
// Returns 111 or a refusal.
hdk_encode_ct :: (work: [] u8, out: [] u8) -> int {
  node := work[HDK_NODE..<HDK_NODE + BIP32_NODE_BYTES]; b := work[HDK_TEXT..<HDK_NODE];  // mutant: equivalent (only the first 82 bytes are used)
  kind := hdk_check(node);
  if kind < 0 { return kind; }
  for j: 0..<BIP32_NODE_BYTES { b[j] = node[j]; }
  #assert bitcoin_checksum_checked(b[0..<78], b[78..<82], false) == SHA_OK;
  // 111 divisions by 58, each over all 82 bytes; q = cur / 58 by multiply
  // and shift. The remainders are the digits, least significant first.
  for j: 0..<BIP32_TEXT {
    rem: u64 = 0;
    for k: 0..<82 {
      cur := add_wrap(mul_wrap(rem, 256), cast(u64) b[k]);
      q := mul_wrap(cur, 565) >> 15;
      rem = sub_wrap(cur, mul_wrap(q, 58));
      b[k] = cast(u8) q;
    }
    out[BIP32_TEXT - 1 - j] = cast(u8) rem;
  }
  for j: 0..<BIP32_TEXT { out[j] = b58_char_ct(cast(u64) out[j]); }
  return BIP32_TEXT;
}
bip32_encode_ct :: (node: [] u8, work: [] u8, out: [] u8) -> int #export_c {
  if node.count != BIP32_NODE_BYTES { return BIP32_BAD_SIZE; }
  if out.count < BIP32_TEXT || work.count < BIP32_WORK_BYTES { return BIP32_CAPACITY; }
  for j: 0..<BIP32_NODE_BYTES { work[HDK_NODE + j] = node[j]; }
  result := hdk_encode_ct(work, out);
  hdk_wipe(work);
  return result;
}
```

The decoder reads exactly 111 characters, so a text of any other length
refuses before any digit is read. 111 digits are below `58^111`, which is
less than `2^651`, so the value always fits in the 82 bytes. Then it checks
the fields in a fixed order:

<!-- source: coax/lib/crypto/bip32.coax decode -->
```jai
// A 111-character text into work's node slot, checking what test vector 5
// lists: the alphabet and the checksum, a known version, depth 0 only with
// no parent and index 0, a key byte that fits the version, a private key
// from 1 to n - 1 and a public key on the curve. The digits and the
// checksum are read in constant time and become flags; the version, depth,
// fingerprint, index and key byte are public, and each is checked in turn.
hdk_decode_ct :: (text: string, work: [] u8) -> int {
  if text.count != BIP32_TEXT { return BIP32_BAD_TEXT; }
  b := work[HDK_TEXT..<HDK_NODE]; sum := work[HDK_DATA..<HDK_DATA + 4];  // mutant: equivalent (only the first 82 and 4 bytes are used)
  for k: 0..<82 { b[k] = 0; }  // mutant: equivalent (the top byte times 58^111 has 2^759 as a factor and leaves the 82 bytes)
  // 111 digits are below 58^111 < 2^651, so the value always fits in 82
  // bytes and the last carry is always 0.
  bad: u64 = 0;
  for c: 0..<BIP32_TEXT {
    d := b58_digit_ct(text[c]);
    bad |= d >> 8;
    carry := d & 255;                               // b = b * 58 + digit
    for k: 0..<82 {
      j := 81 - k;
      carry = add_wrap(carry, mul_wrap(cast(u64) b[j], 58));
      b[j] = cast(u8) (carry & 255);
      carry = carry >> 8;
    }
  }
  #assert bitcoin_checksum_checked(b[0..<78], sum, false) == SHA_OK;
  diff: u64 = 0;
  for k: 0..<4 { diff |= cast(u64) (sum[k] ^ b[78 + k]); }
  if bad != 0 { return BIP32_BAD_TEXT; }
  if diff != 0 { return BIP32_BAD_CHECKSUM; }
  node := work[HDK_NODE..<HDK_NODE + BIP32_NODE_BYTES];
  for j: 0..<BIP32_NODE_BYTES { node[j] = b[j]; }
  kind := hdk_kind(hdk_get32(node, HDK_VERSION));
  if kind < 0 { return kind; }
  if node[HDK_DEPTH] == 0 && (hdk_get32(node, HDK_PARENT) != 0 || hdk_get32(node, HDK_INDEX) != 0) { return BIP32_BAD_DEPTH; }
  kind = hdk_check(node);
  if kind < 0 { return kind; }
  if kind == 1 {
    k := sc_load(node[HDK_KEY + 1..<BIP32_NODE_BYTES]);
    if !k.ok || sc_is_zero(k.value) { return BIP32_INVALID_KEY; }
  } else if !se_decode(node[HDK_KEY..<BIP32_NODE_BYTES]).ok { return BIP32_INVALID_KEY; }
  return BIP32_NODE_BYTES;
}
// An xprv, xpub, tprv or tpub into node. Returns 78 or a refusal, and
// writes node only on success. work comes back as zeros.
bip32_decode_ct :: (text: string, work: [] u8, node: [] u8) -> int #export_c {
  if node.count < BIP32_NODE_BYTES || work.count < BIP32_WORK_BYTES { return BIP32_CAPACITY; }
  result := hdk_decode_ct(text, work);
  if result > 0 { for j: 0..<BIP32_NODE_BYTES { node[j] = work[HDK_NODE + j]; } }  // mutant: equivalent (a result is 78 or negative)
  hdk_wipe(work);
  return result;
}
```

Test vector 5 lists 16 texts that a reader must refuse. Each gets one
code:

| BIP-32's reason | Count | Refusal |
| --- | ---: | --- |
| a public version with `00`, a private version with `02` | 2 | `BIP32_BAD_PREFIX` |
| key byte `04` or `01` | 4 | `BIP32_BAD_PREFIX` |
| depth 0 with a parent fingerprint | 2 | `BIP32_BAD_DEPTH` |
| depth 0 with an index | 2 | `BIP32_BAD_DEPTH` |
| unknown version | 2 | `BIP32_BAD_VERSION` |
| private key 0, or `n` | 2 | `BIP32_INVALID_KEY` |
| the public key `02…07`, whose `x` is on no point | 1 | `BIP32_INVALID_KEY` |
| a wrong checksum | 1 | `BIP32_BAD_CHECKSUM` |

The two unknown versions decode to 82 bytes with a good checksum, so the
version check is what refuses them. A key byte `04` would start an
uncompressed public key, which BIP-32 never serializes.

## Invalid keys and the next index

Two derivations have no child:

- `I_L` is `n` or more, so it is not a valid scalar.
- The child key is 0 (`I_L + k_par = n`), or, in CKDpub, the child point is
  the point at infinity (`I_L·G = -K_par`).

BIP-32 says that a wallet that meets either case should move to the next
index. Each happens with a probability below 1 in 2^127, and no published
seed or index reaches one. This code reports both as
`BIP32_INVALID_KEY` and never skips on its own: an index list that names
an invalid child refuses, and the caller, who knows what the index means,
picks the next one. A wallet scanning for addresses moves to index `i + 1`;
a path with a fixed meaning, such as NIP-06's, has no next index and
fails. The example runs the two checks directly, with `I_L = n`, with
`I_L = n - 1` and the key 1, and with `I_L = n - 1` and the public key
`G`.

`BIP32_INVALID_KEY` also covers a node that holds a private key of 0 or `n`
or more, or a public key that isn't on the curve. A decoded node never
does; a node built by hand can.

## Secrets and constant time

Chapter 3's rules apply: a secret never decides a branch, never picks a
memory address and never reaches a variable-time routine. The private key,
the chain code of a private node, `I` and the text of an `xprv` are
secrets. Anyone with an `xprv` can spend from every key below it.

- `k·G` uses chapter 16's constant-time multiplication, for every key. A
  key of 0 or `n` or more is multiplied too; it loads as 0 and its product
  has no encoding.
- The scalar checks of the tweak become one flag, and the procedure
  branches once, on the result.
- HMAC-SHA512 has no branch on its data.
- The Base58 encoder and decoder use chapter 26's constant-time digit and
  character lookups and fixed loops.
- Every secret intermediate is in the work buffer, which comes back as
  zeros. The caller can register it for erasure (chapter 3).

These facts are public on purpose:

- The version, depth, parent fingerprint and index, and so whether a node
  is private, which network it is for and whether an index is hardened.
- The result code. A refusal says what was wrong.
- Everything in a public node. CKDpub, the public key's encoding and the
  fingerprint's HASH160 read only public data, and CKDpub uses the faster
  variable-time multiplication.

The chain code of an `xpub` is public in this sense, but it is still
private information: with it, anyone can derive every normal child and link
all of a wallet's addresses. Share an `xpub` only with software that should
see the whole account.

As in chapter 3, these are properties of the Coax source, not a proof
about a particular CPU or compiler.

## The audit

`bip32.coax` was written for Nostr chapter 46 and the website's words
account, with private derivation only. This chapter audited it and rebuilt
it around the 78-byte node. The old procedures keep their names and their
answers for their callers.

- There was no CKDpub, no `N()`, no fingerprint and no serialization. They
  are added, with every test vector 5 check.
- `bip32_child` accepted a parent key of 0 for a hardened child, and
  derived from it. A key of 0 or `n` or more now refuses.
- `bip32_public` returned early, before the multiplication, for a key of 0
  or `n` or more. It now multiplies every key, and the encoding refuses
  the product of an invalid one.
- `I`, the HMAC message and the parent's public key were locals that the
  procedures zeroed by hand; they now live in the work buffer, which comes
  back as zeros on every path.
- Every refusal was `false`. The new procedures return ten distinct codes;
  the old ones still return `false`.
- `bip32_path` accepted any number of levels. A path that would go below
  depth 255 now refuses.
- `bip32_hex`, which the Nostr checks use for their seeds, is marked as
  variable-time and for published test data only.

The older interface is now a thin layer over the new procedures:

<!-- source: coax/lib/crypto/bip32.coax older -->
```jai
// The compressed public key (33 bytes) of a private key.
bip32_public :: (key: [] u8, out: [] u8) -> bool #export_c {
  if key.count != 32 || out.count < 33 { return false; }  // mutant: equivalent (a short key or out fails in hdk_point_ct anyway)
  return hdk_point_ct(key, out);
}
// The master key and chain code of a seed (16 to 64 bytes).
bip32_master :: (seed: [] u8, key: [] u8, chain: [] u8) -> bool #export_c {
  if key.count != 32 || chain.count != 32 { return false; }
  work: [BIP32_WORK_BYTES] u8; node: [BIP32_NODE_BYTES] u8;
  ok := bip32_master_ct(seed, false, work[..], node[..]) > 0;  // mutant: equivalent (the version isn't copied out, and a result is 78 or negative)
  hdk_split(ok, node[..], key, chain);
  return ok;
}
// The child at index (0 .. 2^32-1; BIP32_HARDENED and up is hardened) of
// key and chain, into key_out and chain_out.
bip32_child :: (key: [] u8, chain: [] u8, index: int, key_out: [] u8, chain_out: [] u8) -> bool #export_c {
  if key.count != 32 || chain.count != 32 || key_out.count != 32 || chain_out.count != 32 { return false; }
  work: [BIP32_WORK_BYTES] u8; node: [BIP32_NODE_BYTES] u8;
  hdk_put32(node[..], HDK_VERSION, BIP32_XPRV);
  for j: 0..<32 { node[HDK_CHAIN + j] = chain[j]; node[HDK_KEY + 1 + j] = key[j]; }
  ok := bip32_ckd_ct(node[..], index, work[..], node[..]) > 0;  // mutant: equivalent (a result is 78 or negative)
  hdk_split(ok, node[..], key_out, chain_out);
  return ok;
}
// A path of indexes from a seed's master, into key and chain, which are
// zeroed on a refusal.
bip32_path :: (seed: [] u8, path: [] int, key: [] u8, chain: [] u8) -> bool #export_c {
  if key.count != 32 || chain.count != 32 { return false; }
  work: [BIP32_WORK_BYTES] u8; node: [BIP32_NODE_BYTES] u8;
  ok := bip32_master_ct(seed, false, work[..], node[..]) > 0 && bip32_descend_ct(node[..], path, work[..], node[..]) > 0;  // mutant: equivalent (the version isn't copied out, and a result is 78 or negative)
  hdk_split(ok, node[..], key, chain);
  if !ok { for j: 0..<32 { key[j] = 0; chain[j] = 0; } }
  return ok;
}
// Copies a node's key and chain code out when ok, and zeroes the node.
hdk_split :: (ok: bool, node: [] u8, key: [] u8, chain: [] u8) {
  if ok { for j: 0..<32 { key[j] = node[HDK_KEY + 1 + j]; chain[j] = node[HDK_CHAIN + j]; } }
  for j: 0..<BIP32_NODE_BYTES { node[j] = 0; }
}
```

NIP-06's path never meets a zero key or depth 255, so its keys don't
change. Nostr chapter 46's listings show its example and checks, which call
the old procedures; they are unchanged, and its fixture still matches.

## The example

The example is [bitcoin/examples/hd-keys.coax](../examples/hd-keys.coax).
It derives test vector 1's master by hand and prints its 78 bytes, computes
the master's public node and fingerprint, derives a hardened and a normal
child and shows that CKDpub gives the same public child, recovers a parent
key from its `xpub` and a child key, runs every chain of test vectors 1 to
4 both ways, tries every key of test vector 5 and every master in chapter
31's vectors, runs the two skipped cases, makes the testnet forms, and
prints the refusals.

<!-- source: bitcoin/examples/hd-keys.coax example -->
```jai
#load "../../coax/lib/crypto/bip32.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/bip32_vectors.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [200] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode hex into out and return the filled part. Every literal here and
// in the vector table is valid lowercase hex, so an error stops the program.
decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
equal_bytes :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}
all_zero :: (work: [] u8) -> bool {
  for b: work { if b != 0 { return false; } }
  return true;
}
// The name of a refusal.
REASONS :: [10] string.["bad size", "bad index", "invalid key", "needs private", "capacity",
  "bad text", "bad checksum", "bad version", "bad prefix", "bad depth"];
reason :: (status: int) -> string { return ifx status < 0 && status >= -10 then REASONS[-status - 1] else "accepted"; }
// A node as its xprv, xpub, tprv or tpub, into out.
text_of :: (node: [] u8, out: [] u8) -> string {
  work: [BIP32_WORK_BYTES] u8;
  #assert bip32_encode_ct(node, work[..], out) == BIP32_TEXT;
  return cast(string) out[0..<BIP32_TEXT];
}
// A node from its text, which must decode.
node_of :: (text: string, out: [] u8) -> [] u8 {
  work: [BIP32_WORK_BYTES] u8;
  #assert bip32_decode_ct(text, work[..], out) == BIP32_NODE_BYTES;
  return out[0..<BIP32_NODE_BYTES];
}
// The six fields of a node.
show_fields :: (node: [] u8) {
  show("  version", node[HDK_VERSION..<HDK_DEPTH]);
  print("  depth: %\n", cast(int) node[HDK_DEPTH]);
  show("  parent fingerprint", node[HDK_PARENT..<HDK_INDEX]);
  print("  index: %\n", hdk_get32(node, HDK_INDEX));
  show("  chain code", node[HDK_CHAIN..<HDK_KEY]);
  show("  key", node[HDK_KEY..<BIP32_NODE_BYTES]);
}
// k_child - I_L mod n: the parent's private key, from the parent's public
// key and chain code and one child's private key at index. Returns false
// when a scalar is out of range.
parent_key :: (xpub: [] u8, child_key: [] u8, index: int, out: [] u8) -> bool {
  data: [37] u8; i: [64] u8;
  for j: 0..<33 { data[j] = xpub[HDK_KEY + j]; }
  hdk_put32(data[..], 33, index);
  #assert hmac_sha512(xpub[HDK_CHAIN..<HDK_KEY], data[..], i[..]);
  il := sc_load(i[0..<32]); k := sc_load(child_key);
  return il.ok && k.ok && sc_store(sc_sub(k.value, il.value), out);
}

main :: () {
  work: [BIP32_WORK_BYTES] u8; sb: [64] u8; i: [64] u8; h: [20] u8; fp: [4] u8;
  m: [BIP32_NODE_BYTES] u8; a: [BIP32_NODE_BYTES] u8; b: [BIP32_NODE_BYTES] u8; p: [BIP32_NODE_BYTES] u8;
  q: [BIP32_NODE_BYTES] u8; text: [BIP32_TEXT] u8; k: [32] u8;

  // The master by hand: HMAC-SHA512 with the key "Bitcoin seed".
  print("the master by hand\n");
  seed := decode(BIP32_SEEDS[0], sb[..]);
  show("seed", seed);
  label := "Bitcoin seed"; lb: [12] u8;
  for j: 0..<12 { lb[j] = label[j]; }
  #assert hmac_sha512(lb[..], seed, i[..]);
  show("I_L, the master key", i[0..<32]);
  show("I_R, the master chain code", i[32..<64]);
  #assert bip32_master_ct(seed, false, work[..], m[..]) == BIP32_NODE_BYTES;
  print("the 78 bytes\n");
  show_fields(m[..]);
  print("m: %\n", text_of(m[..], text[..]));

  // N(m), and the fingerprint: HASH160 of the public key, first 4 bytes.
  print("public node and fingerprint\n");
  #assert bip32_neuter_ct(m[..], work[..], p[..]) == BIP32_NODE_BYTES;
  show("  key", p[HDK_KEY..<BIP32_NODE_BYTES]);
  print("N(m): %\n", text_of(p[..], text[..]));
  #assert hash160_checked(p[HDK_KEY..<BIP32_NODE_BYTES], h[..], false) == RMD_OK;
  show("HASH160 of m's key", h[..]);
  #assert bip32_fingerprint_ct(m[..], work[..], fp[..]) == 4;
  show("m's fingerprint", fp[..]);

  // A hardened child and a normal one.
  print("children\n");
  #assert bip32_ckd_ct(m[..], BIP32_HARDENED, work[..], a[..]) == BIP32_NODE_BYTES;
  print("m/0H: %\n", text_of(a[..], text[..]));
  show_fields(a[..]);
  #assert bip32_ckd_ct(a[..], 1, work[..], b[..]) == BIP32_NODE_BYTES;
  print("m/0H/1: %\n", text_of(b[..], text[..]));
  #assert bip32_neuter_ct(b[..], work[..], q[..]) == BIP32_NODE_BYTES;
  print("N(m/0H/1): %\n", text_of(q[..], text[..]));
  // CKDpub from N(m/0H) gives N(m/0H/1) without a private key.
  #assert bip32_neuter_ct(a[..], work[..], p[..]) == BIP32_NODE_BYTES;
  print("N(m/0H): %\n", text_of(p[..], text[..]));
  #assert bip32_ckd_ct(p[..], 1, work[..], p[..]) == BIP32_NODE_BYTES;
  print("CKDpub(N(m/0H), 1) is N(m/0H/1): %\n", equal_bytes(p[..], q[..]));
  #assert bip32_neuter_ct(a[..], work[..], p[..]) == BIP32_NODE_BYTES;
  print("CKDpub(N(m/0H), 0H): %\n", reason(bip32_ckd_ct(p[..], BIP32_HARDENED, work[..], q[..])));
  print("work buffer zero: %\n", all_zero(work[..]));

  // The leak: N(m/0H) and m/0H/1's private key give m/0H's private key.
  print("an xpub and one normal child's private key\n");
  #assert parent_key(p[..], b[HDK_KEY + 1..<BIP32_NODE_BYTES], 1, k[..]);
  show("k of m/0H/1", b[HDK_KEY + 1..<BIP32_NODE_BYTES]);
  show("k of m/0H/1 - I_L", k[..]);
  print("is m/0H's private key: %\n", equal_bytes(k[..], a[HDK_KEY + 1..<BIP32_NODE_BYTES]));
  // Put the key into the xpub's node: the result is m/0H's xprv, and with
  // it every child, hardened ones included.
  hdk_put32(p[..], HDK_VERSION, BIP32_XPRV);
  p[HDK_KEY] = 0;
  for j: 0..<32 { p[HDK_KEY + 1 + j] = k[j]; }
  print("rebuilt: %\n", text_of(p[..], text[..]));
  #assert bip32_ckd_ct(p[..], BIP32_HARDENED + 5, work[..], q[..]) == BIP32_NODE_BYTES;
  #assert bip32_ckd_ct(a[..], BIP32_HARDENED + 5, work[..], p[..]) == BIP32_NODE_BYTES;
  print("its child 5H is m/0H/5H: %\n", equal_bytes(p[..], q[..]));
  // The same sum for a hardened child gives nothing: I_L of m/0H hashed
  // m's private key, which the xpub doesn't hold.
  #assert bip32_neuter_ct(m[..], work[..], p[..]) == BIP32_NODE_BYTES;
  #assert parent_key(p[..], a[HDK_KEY + 1..<BIP32_NODE_BYTES], BIP32_HARDENED, k[..]);
  print("N(m) and m/0H's key give m's private key: %\n", equal_bytes(k[..], m[HDK_KEY + 1..<BIP32_NODE_BYTES]));

  // Test vectors 1 to 4: every chain from its seed, its xpub with N, both
  // texts back to the same node, and CKDpub where the last index is normal.
  print("BIP-32 test vectors 1 to 4\n");
  prv := 0; pub := 0; back := 0; ckdpub := 0; normal := 0;
  for c: BIP32_CHAINS {
    s := decode(BIP32_SEEDS[c.vector - 1], sb[..]);
    #assert bip32_master_ct(s, false, work[..], m[..]) == BIP32_NODE_BYTES;
    path := c.indexes;
    #assert bip32_descend_ct(m[..], path[0..<c.depth], work[..], a[..]) == BIP32_NODE_BYTES;
    if text_of(a[..], text[..]) == c.xprv { prv += 1; }
    #assert bip32_neuter_ct(a[..], work[..], q[..]) == BIP32_NODE_BYTES;
    if text_of(q[..], text[..]) == c.xpub { pub += 1; }
    if equal_bytes(node_of(c.xprv, b[..]), a[..]) && equal_bytes(node_of(c.xpub, p[..]), q[..]) { back += 2; }
    if c.depth > 0 && c.indexes[c.depth - 1] < BIP32_HARDENED {
      normal += 1;
      #assert bip32_descend_ct(m[..], path[0..<c.depth - 1], work[..], p[..]) == BIP32_NODE_BYTES;
      #assert bip32_neuter_ct(p[..], work[..], p[..]) == BIP32_NODE_BYTES;
      #assert bip32_ckd_ct(p[..], c.indexes[c.depth - 1], work[..], p[..]) == BIP32_NODE_BYTES;
      if text_of(p[..], text[..]) == c.xpub { ckdpub += 1; }
    }
  }
  print("% of % xprv, % of % xpub, % of % texts decoded to the same node\n", prv, BIP32_CHAINS.count, pub, BIP32_CHAINS.count, back, 2 * BIP32_CHAINS.count);
  print("CKDpub from the parent's xpub: % of % normal children\n", ckdpub, normal);
  last := BIP32_CHAINS[11];
  print("vector %, %, depth %: %\n", last.vector, last.path, last.depth, last.xprv);

  // Test vector 5: invalid extended keys.
  print("BIP-32 test vector 5\n");
  for x: BIP32_INVALID { print("  %: %\n", x.reason, reason(bip32_decode_ct(x.text, work[..], a[..]))); }

  // The xprv column of trezor/python-mnemonic's vectors.json: each seed's master.
  masters := 0;
  for x: BIP32_TREZOR {
    #assert bip32_master_ct(decode(x.seed, sb[..]), false, work[..], m[..]) == BIP32_NODE_BYTES;
    if text_of(m[..], text[..]) == x.xprv { masters += 1; }
  }
  print("vectors.json: % of % masters\n", masters, BIP32_TREZOR.count);

  // Skipping an index. No published seed or index reaches either case; these
  // run the two checks directly.
  print("the cases BIP-32 skips\n");
  n: [32] u8; n1: [32] u8; g: [33] u8; out: [33] u8;
  #assert decode("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141", n[..]).count == 32;
  #assert decode("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140", n1[..]).count == 32;
  #assert decode("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", g[..]).count == 33;
  one: [32] u8; one[31] = 1;
  print("I_L = n: %\n", reason(hdk_tweak_ct(n[..], one[..], out[..])));
  print("I_L = n - 1, k = 1, so k_i = 0: %\n", reason(hdk_tweak_ct(n1[..], one[..], out[..])));
  print("I_L = n - 1, K = G, so K_i is infinity: %\n", reason(hdk_add_point(n1[..], g[..], out[..])));

  // The test networks.
  print("testnet\n");
  #assert bip32_master_ct(decode(BIP32_SEEDS[0], sb[..]), true, work[..], m[..]) == BIP32_NODE_BYTES;
  print("m: %\n", text_of(m[..], text[..]));
  #assert bip32_neuter_ct(m[..], work[..], p[..]) == BIP32_NODE_BYTES;
  print("N(m): %\n", text_of(p[..], text[..]));

  // Refusals.
  print("refusals\n");
  print("a 15-byte seed: %\n", reason(bip32_master_ct(sb[0..<15], false, work[..], m[..])));
  #assert bip32_master_ct(decode(BIP32_SEEDS[0], sb[..]), false, work[..], m[..]) == BIP32_NODE_BYTES;
  print("index 2^32: %\n", reason(bip32_ckd_ct(m[..], 4294967296, work[..], a[..])));
  print("index -1: %\n", reason(bip32_ckd_ct(m[..], -1, work[..], a[..])));
  print("a node into 77 bytes: %\n", reason(bip32_ckd_ct(m[..], 0, work[..], a[0..<77])));
  long := BIP32_CHAINS[0].xprv; cut: [BIP32_TEXT] u8;
  for j: 0..<BIP32_TEXT { cut[j] = long[j]; }
  print("110 characters: %\n", reason(bip32_decode_ct(cast(string) cut[0..<110], work[..], a[..])));
  cut[7] = 'l';
  print("an l, outside the alphabet: %\n", reason(bip32_decode_ct(cast(string) cut[..], work[..], a[..])));
  cut[7] = long[7]; cut[110] = 'j';
  print("the last character changed: %\n", reason(bip32_decode_ct(cast(string) cut[..], work[..], a[..])));
  for j: 0..<BIP32_NODE_BYTES { a[j] = m[j]; }
  a[HDK_DEPTH] = 255;
  print("a child past depth 255: %\n", reason(bip32_ckd_ct(a[..], 0, work[..], b[..])));
  print("work buffer zero: %\n", all_zero(work[..]));
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/hd-keys.coax
```

The output:

```text
the master by hand
seed: 000102030405060708090a0b0c0d0e0f
I_L, the master key: e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35
I_R, the master chain code: 873dff81c02f525623fd1fe5167eac3a55a049de3d314bb42ee227ffed37d508
the 78 bytes
  version: 0488ade4
  depth: 0
  parent fingerprint: 00000000
  index: 0
  chain code: 873dff81c02f525623fd1fe5167eac3a55a049de3d314bb42ee227ffed37d508
  key: 00e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35
m: xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi
public node and fingerprint
  key: 0339a36013301597daef41fbe593a02cc513d0b55527ec2df1050e2e8ff49c85c2
N(m): xpub661MyMwAqRbcFtXgS5sYJABqqG9YLmC4Q1Rdap9gSE8NqtwybGhePY2gZ29ESFjqJoCu1Rupje8YtGqsefD265TMg7usUDFdp6W1EGMcet8
HASH160 of m's key: 3442193e1bb70916e914552172cd4e2dbc9df811
m's fingerprint: 3442193e
children
m/0H: xprv9uHRZZhk6KAJC1avXpDAp4MDc3sQKNxDiPvvkX8Br5ngLNv1TxvUxt4cV1rGL5hj6KCesnDYUhd7oWgT11eZG7XnxHrnYeSvkzY7d2bhkJ7
  version: 0488ade4
  depth: 1
  parent fingerprint: 3442193e
  index: 2147483648
  chain code: 47fdacbd0f1097043b78c63c20c34ef4ed9a111d980047ad16282c7ae6236141
  key: 00edb2e14f9ee77d26dd93b4ecede8d16ed408ce149b6cd80b0715a2d911a0afea
m/0H/1: xprv9wTYmMFdV23N2TdNG573QoEsfRrWKQgWeibmLntzniatZvR9BmLnvSxqu53Kw1UmYPxLgboyZQaXwTCg8MSY3H2EU4pWcQDnRnrVA1xe8fs
N(m/0H/1): xpub6ASuArnXKPbfEwhqN6e3mwBcDTgzisQN1wXN9BJcM47sSikHjJf3UFHKkNAWbWMiGj7Wf5uMash7SyYq527Hqck2AxYysAA7xmALppuCkwQ
N(m/0H): xpub68Gmy5EdvgibQVfPdqkBBCHxA5htiqg55crXYuXoQRKfDBFA1WEjWgP6LHhwBZeNK1VTsfTFUHCdrfp1bgwQ9xv5ski8PX9rL2dZXvgGDnw
CKDpub(N(m/0H), 1) is N(m/0H/1): true
CKDpub(N(m/0H), 0H): needs private
work buffer zero: true
an xpub and one normal child's private key
k of m/0H/1: 3c6cb8d0f6a264c91ea8b5030fadaa8e538b020f0a387421a12de9319dc93368
k of m/0H/1 - I_L: edb2e14f9ee77d26dd93b4ecede8d16ed408ce149b6cd80b0715a2d911a0afea
is m/0H's private key: true
rebuilt: xprv9uHRZZhk6KAJC1avXpDAp4MDc3sQKNxDiPvvkX8Br5ngLNv1TxvUxt4cV1rGL5hj6KCesnDYUhd7oWgT11eZG7XnxHrnYeSvkzY7d2bhkJ7
its child 5H is m/0H/5H: true
N(m) and m/0H's key give m's private key: false
BIP-32 test vectors 1 to 4
17 of 17 xprv, 17 of 17 xpub, 34 of 34 texts decoded to the same node
CKDpub from the parent's xpub: 6 of 6 normal children
vector 2, m/0/2147483647H/1/2147483646H/2, depth 5: xprvA2nrNbFZABcdryreWet9Ea4LvTJcGsqrMzxHx98MMrotbir7yrKCEXw7nadnHM8Dq38EGfSh6dqA9QWTyefMLEcBYJUuekgW4BYPJcr9E7j
BIP-32 test vector 5
  pubkey version / prvkey mismatch: bad prefix
  prvkey version / pubkey mismatch: bad prefix
  invalid pubkey prefix 04: bad prefix
  invalid prvkey prefix 04: bad prefix
  invalid pubkey prefix 01: bad prefix
  invalid prvkey prefix 01: bad prefix
  zero depth with non-zero parent fingerprint: bad depth
  zero depth with non-zero parent fingerprint: bad depth
  zero depth with non-zero index: bad depth
  zero depth with non-zero index: bad depth
  unknown extended key version: bad version
  unknown extended key version: bad version
  private key 0 not in 1..n-1: invalid key
  private key n not in 1..n-1: invalid key
  invalid pubkey 020000000000000000000000000000000000000000000000000000000000000007: invalid key
  invalid checksum: bad checksum
vectors.json: 24 of 24 masters
the cases BIP-32 skips
I_L = n: invalid key
I_L = n - 1, k = 1, so k_i = 0: invalid key
I_L = n - 1, K = G, so K_i is infinity: invalid key
testnet
m: tprv8ZgxMBicQKsPeDgjzdC36fs6bMjGApWDNLR9erAXMs5skhMv36j9MV5ecvfavji5khqjWaWSFhN3YcCUUdiKH6isR4Pwy3U5y5egddBr16m
N(m): tpubD6NzVbkrYhZ4XgiXtGrdW5XDAPFCL9h7we1vwNCpn8tGbBcgfVYjXyhWo4E1xkh56hjod1RhGjxbaTLV3X4FyWuejifB9jusQ46QzG87VKp
refusals
a 15-byte seed: bad size
index 2^32: bad index
index -1: bad index
a node into 77 bytes: capacity
110 characters: bad text
an l, outside the alphabet: bad text
the last character changed: bad checksum
a child past depth 255: bad depth
work buffer zero: true
```

What the output shows:

- `I_L` and `I_R` of the seed, and the master's 78 bytes field by field,
  which encode to the published `xprv`.
- `N(m)`'s key, its HASH160 and the fingerprint `3442193e`, which is the
  parent fingerprint in `m/0H`'s fields.
- `m/0H`, with the index `2147483648` (`2^31`), and `m/0H/1`, both as
  published. CKDpub from `N(m/0H)` gives `N(m/0H/1)`, and a hardened child
  of `N(m/0H)` refuses.
- The leak: `k_1 - I_L` is `m/0H`'s key, the rebuilt node is `m/0H`'s
  `xprv`, and its child `5H` is `m/0H/5H`. For the hardened child, the same
  subtraction gives a wrong key.
- All 17 chains of test vectors 1 to 4 as `xprv` and `xpub`, all 34 texts
  decoded back to the same nodes, and CKDpub for the 6 chains whose last
  index is normal.
- Each test vector 5 key with BIP-32's reason and this code's refusal.
- All 24 masters of the `xprv` column of trezor/python-mnemonic's
  `vectors.json`.
- The two skipped cases, the testnet forms, and the refusals, with the work
  buffer zero at the end.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/hd-keys.out](../fixtures/hd-keys.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/hd-keys.coax](../examples/checks/hd-keys.coax),
which loads the example:

- every chain of test vectors 1 to 4 from its seed, as its `xprv`, and its
  public node as its `xpub`, with the work buffer zero;
- every test vector 5 key, with the code that matches BIP-32's reason and
  nothing written;
- the 24 masters of `vectors.json`;
- the leak, with the right index and a wrong one, and for a hardened child;
- CKDpub from test vector 2's master `xpub` to `m/0`, and a hardened child
  below it refused.

<!-- source: bitcoin/examples/checks/hd-keys.coax check -->
```jai
// Test vectors 1 to 4: every chain from its seed, as its xprv, and N of it
// as its xpub, with the work buffer zero after each call.
#forall "test vectors 1 to 4, every chain" i: 0..<17 #steps 2000000000 {
  c := BIP32_CHAINS[i]; sb: [64] u8; work: [BIP32_WORK_BYTES] u8; m: [BIP32_NODE_BYTES] u8; node: [BIP32_NODE_BYTES] u8;
  text: [BIP32_TEXT] u8; path := c.indexes;
  #assert bip32_master_ct(decode(BIP32_SEEDS[c.vector - 1], sb[..]), false, work[..], m[..]) == BIP32_NODE_BYTES;
  #assert bip32_descend_ct(m[..], path[0..<c.depth], work[..], node[..]) == BIP32_NODE_BYTES && all_zero(work[..]);
  #assert text_of(node[..], text[..]) == c.xprv && cast(int) node[HDK_DEPTH] == c.depth;
  #assert bip32_neuter_ct(node[..], work[..], node[..]) == BIP32_NODE_BYTES && text_of(node[..], text[..]) == c.xpub;
}
// Test vector 5: each invalid key refuses, with the code that matches the
// reason BIP-32 gives.
#forall "test vector 5, every invalid key" i: 0..<16 #steps 100000000 {
  x := BIP32_INVALID[i]; work: [BIP32_WORK_BYTES] u8; node: [BIP32_NODE_BYTES] u8;
  want := BIP32_BAD_PREFIX;
  if x.reason == "zero depth with non-zero parent fingerprint" || x.reason == "zero depth with non-zero index" { want = BIP32_BAD_DEPTH; }
  if x.reason == "unknown extended key version" { want = BIP32_BAD_VERSION; }
  if x.reason == "private key 0 not in 1..n-1" || x.reason == "private key n not in 1..n-1" { want = BIP32_INVALID_KEY; }
  if x.reason == "invalid pubkey 020000000000000000000000000000000000000000000000000000000000000007" { want = BIP32_INVALID_KEY; }
  if x.reason == "invalid checksum" { want = BIP32_BAD_CHECKSUM; }
  #assert bip32_decode_ct(x.text, work[..], node[..]) == want && all_zero(work[..]) && all_zero(node[..]);
}
// The xprv column of trezor/python-mnemonic's vectors.json.
#forall "each BIP-39 seed's master" i: 0..<24 #steps 100000000 {
  x := BIP32_TREZOR[i]; sb: [64] u8; work: [BIP32_WORK_BYTES] u8; m: [BIP32_NODE_BYTES] u8; text: [BIP32_TEXT] u8;
  #assert bip32_master_ct(decode(x.seed, sb[..]), false, work[..], m[..]) == BIP32_NODE_BYTES && text_of(m[..], text[..]) == x.xprv;
}
// The leak: vector 1's N(m/0H) and the private key of m/0H/1 give m/0H's
// private key. For the hardened child m/0H, N(m) and its key don't give m's.
#test "an xpub and a normal child's private key give the parent's" #steps 400000000 {
  p: [BIP32_NODE_BYTES] u8; b: [BIP32_NODE_BYTES] u8; a: [BIP32_NODE_BYTES] u8; m: [BIP32_NODE_BYTES] u8; k: [32] u8;
  #assert node_of(BIP32_CHAINS[1].xpub, p[..]).count > 0 && node_of(BIP32_CHAINS[2].xprv, b[..]).count > 0;
  #assert node_of(BIP32_CHAINS[1].xprv, a[..]).count > 0;
  #assert parent_key(p[..], b[HDK_KEY + 1..<BIP32_NODE_BYTES], 1, k[..]) && equal_bytes(k[..], a[HDK_KEY + 1..<BIP32_NODE_BYTES]);
  #assert parent_key(p[..], b[HDK_KEY + 1..<BIP32_NODE_BYTES], 2, k[..]) && !equal_bytes(k[..], a[HDK_KEY + 1..<BIP32_NODE_BYTES]);
  #assert node_of(BIP32_CHAINS[0].xpub, p[..]).count > 0 && node_of(BIP32_CHAINS[0].xprv, m[..]).count > 0;
  #assert parent_key(p[..], a[HDK_KEY + 1..<BIP32_NODE_BYTES], BIP32_HARDENED, k[..]) && !equal_bytes(k[..], m[HDK_KEY + 1..<BIP32_NODE_BYTES]);
}
// CKDpub from vector 2's master xpub gives m/0's xpub, and the hardened
// child below it refuses.
#test "CKDpub from an xpub" #steps 400000000 {
  p: [BIP32_NODE_BYTES] u8; work: [BIP32_WORK_BYTES] u8; text: [BIP32_TEXT] u8;
  #assert BIP32_CHAINS[6].vector == 2 && BIP32_CHAINS[7].depth == 1;
  #assert bip32_ckd_ct(node_of(BIP32_CHAINS[6].xpub, p[..]), 0, work[..], p[..]) == BIP32_NODE_BYTES && text_of(p[..], text[..]) == BIP32_CHAINS[7].xpub;
  #assert bip32_ckd_ct(p[..], 2147483647 + BIP32_HARDENED, work[..], p[..]) == BIP32_NEEDS_PRIVATE && all_zero(work[..]);
}
#test "the helpers" {
  a := [3] u8.[1, 2, 3]; b := a; z: [3] u8;
  #assert equal_bytes(a[..], b[..]) && !equal_bytes(a[..], b[0..<2]);
  b[2] = 9; #assert !equal_bytes(a[..], b[..]);
  #assert all_zero(z[..]) && !all_zero(a[..]);
  #assert reason(-1) == "bad size" && reason(-10) == "bad depth" && reason(0) == "accepted" && reason(78) == "accepted" && reason(-11) == "accepted";
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/hd-keys.coax
coax/bin/coax mutate bitcoin/examples/checks/hd-keys.coax --root-only
```

`coax test` runs 207 checks, including the loaded files' own checks, in about 5 minutes on 2026-11-02 on a loaded machine. The checks file has no procedures of its own, so root-only mutation finds no site in it; the example's own procedures, such as `parent_key`, were not mutated for this chapter.

### The library's own checks

`bip32.coax` is an `#authority` file. Its own checks cover:

- the 4-byte fields, each version and its neighbours, and every key byte;
- `k·G` for 1 and `n - 1`, and the keys 0 and `n`, which refuse and write
  nothing;
- the tweak, including `I_L = n`, a sum of 0, a sum that wraps past `n`,
  and a master `I_L` of 0;
- `I_L·G + K`, including `I_L = n`, a sum at infinity and a point off the
  curve;
- test vector 1's master, `m/0H` and `m/0H/1`, CKDpub, `N()` and the
  fingerprint from an `xprv` and an `xpub`, and the testnet forms;
- every test vector 5 key, and the decoder's other refusals: 110 and 112
  characters, characters outside the alphabet, a wrong checksum in its
  first or last byte, and depth 0 with only the first parent fingerprint
  byte set;
- indexes 0 and `2^32 - 1`, and outputs of exactly the size needed;
- every refusal of every procedure, with the output unwritten and the work
  buffer as the refusal leaves it, including depth 255 and an index list
  that fails part way down;
- the older interface's answers.

```shell
coax/bin/coax test coax/lib/crypto/bip32.coax
coax/bin/coax mutate coax/lib/crypto/bip32.coax --root-only
```

`coax test` runs 198 checks, including those of the files it loads, in about 90 seconds on 2026-11-02 on a loaded machine. The last complete root-only mutation run, on 2026-11-02, made 703 mutants: it killed 654, and 48 were marked equivalent in the source, each with its reason, such as a result that is never 0 or a slice longer than the bytes written into it. One survived, on the line that clears the master's depth, parent and index, where the mark followed another comment and wasn't read; the mark now comes first, and the run wasn't repeated after that change.

### Python, natively and in Wasm

[bip32.py](../../coax/tests/bitcoin/bip32.py) compiles `bip32.coax` to C in
checked and release builds and runs the seven `_ct` procedures against a
separate Python implementation that uses `hmac`, `hashlib` (SHA-256,
SHA-512 and RIPEMD-160) and Python's integers for the curve. It shares no
code with Coax. Its cases are:

- test vectors 1 to 5 and the 24 masters of `vectors.json`;
- 160 seeded random seeds of 16 to 64 bytes on both networks, with random
  hardened and normal children, the boundary indexes and CKDpub from each
  public node, and random index lists;
- each node with one field broken: the version, the key byte, depth 255,
  depth 0 with a parent, keys of 0, `n` and above, a changed chain code or
  bit, and wrong sizes;
- each node's text with a character changed or swapped, a character
  outside the alphabet, 110 and 112 characters, and payloads with a good
  checksum and one bad field;
- outputs and work buffers one byte short, and indexes out of range.

Every result, every output and the work buffer must match. Then the
harness builds the same C for wasm32 and runs every case in Node.

```shell
python3 coax/tests/bitcoin/bip32.py
```

On 2026-11-02, on a loaded machine, all 6,197 cases agree with Python in the checked and release native builds and in both Wasm builds, in about 19 minutes.

## What this chapter leaves out

- Path strings such as `m/84'/0'/0'/0/0`, BIP-44, BIP-49, BIP-84 and
  BIP-86, and the version bytes of SLIP-132 (`ypub`, `zpub`): chapter 33.
- Addresses from the keys: chapter 29.
- Output descriptors, which write a key's origin as a fingerprint and a
  path: chapter 30.
- Skipping an invalid index automatically. The caller decides.
- Ed25519 and other curves (SLIP-10). Bitcoin uses secp256k1 only.

## Summary

- The master is `HMAC-SHA512("Bitcoin seed", seed)`: the key `I_L` and the
  chain code `I_R`.
- A child adds `I_L` to its parent's key, as a scalar or as a point, and
  takes `I_R` as its chain code.
- Hardened children hash the parent's private key; normal children hash its
  public key, so an `xpub` can derive them.
- An `xpub` and one normal child's private key give the parent's private
  key. Hardened levels stop that leak.
- A fingerprint is the first 4 bytes of HASH160 of a public key.
- A node is 78 bytes, and as `xprv`, `xpub`, `tprv` or `tpub` it is always
  111 characters of Base58Check. Every test vector 5 key refuses with its
  own code.
- `I_L` of `n` or more and a zero or infinite child refuse; the caller
  moves to the next index.
- Private derivation, the private keys and the `xprv` text are handled in
  constant time, in a work buffer that comes back as zeros.

## Dependencies

- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)
- [6. RIPEMD-160 and HASH160](ch006-ripemd-160-and-hash160.md)
- [7. SHA-512](ch007-sha-512.md)
- [8. HMAC-SHA256](ch008-hmac-sha256.md)
- [16. Scalar multiplication](ch016-scalar-multiplication.md)
- [17. Point encodings](ch017-point-encodings-compressed-uncompressed-and-x-only.md)
- [26. Base58Check](ch026-base58check.md)
- [31. Seed phrases](ch031-seed-phrases.md)

## Sources

- [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki),
  with test vectors 1 to 5, pinned in
  [coax/tests/vectors/upstream/bip32/](../../coax/tests/vectors/upstream/bip32/bip-0032.mediawiki)
  and listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- trezor/python-mnemonic's `vectors.json` (its `xprv` column), pinned for
  chapter 31 in
  [coax/tests/vectors/upstream/bip39/](../../coax/tests/vectors/upstream/bip39/vectors.json).
- [BIP-44](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki),
  for why accounts are hardened.
