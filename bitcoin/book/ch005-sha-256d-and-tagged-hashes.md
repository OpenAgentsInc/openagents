# 5. SHA-256d and tagged hashes

Bitcoin rarely uses SHA-256 once. It builds two constructions on top of it:

- **SHA-256d** is SHA-256 applied twice. Txids, block hashes, the nodes of a
  block's Merkle tree and the checksum on every P2P message are SHA-256d.
- **Tagged hashes** put a tag in front of the message:
  `SHA256(SHA256(tag) || SHA256(tag) || x)`. BIP-340 signatures and every
  Taproot hash in BIP-341 use them.

This chapter builds both on [chapter 4](ch004-sha-256.md)'s SHA-256. It
hashes the genesis block header by hand, shows why the published hash
starts with zeros, and checks txids, the genesis block hash and the five
tagged hashes against published bytes.

## What this chapter teaches

- How SHA-256d is two SHA-256 calls, and why txids and block hashes are
  shown with their bytes reversed.
- Where a block header keeps its Merkle root, and why a block with one
  transaction has the coinbase txid as its root.
- How the P2P checksum is the first 4 bytes of SHA-256d.
- How a tagged hash separates domains, and why its 64-byte prefix lets you
  start from a saved state.
- What each BIP-341 tag hashes: a leaf, a branch, a tweak and a signature
  message.

## Where the code is

- [coax/lib/bitcoin/hash.coax](../../coax/lib/bitcoin/hash.coax) has
  `sha256d_checked`, `bitcoin_reverse32`, the checksum procedures and the
  tag IDs with `bitcoin_tagged_checked` and `bitcoin_tagged_init`.
- [coax/lib/crypto/sha256.coax](../../coax/lib/crypto/sha256.coax) has the
  tagged hash itself, `sha256_tagged_checked`, and since this chapter
  `sha256_tagged_init`, which both the tagged hash and `hash.coax` use.

`hash.coax` adds no SHA-256 code. Every hash in it calls the streaming
interface of chapter 4. The BIP-143 and BIP-341 signature hashes in
[coax/lib/bitcoin/sighash.coax](../../coax/lib/bitcoin/sighash.coax) had
their own double hash and tagged hash; they now call `sha256d_checked` and
`bitcoin_tagged_checked`. The BIP-340 verifier in
[coax/lib/crypto/schnorr.coax](../../coax/lib/crypto/schnorr.coax) keeps
its own path: it computes `SHA256("BIP0340/challenge")` at compile time and
streams R, P and the message after it. The check at the end of this chapter
compares the two paths.

## SHA-256d

SHA-256d hashes the input, then hashes the 32-byte result:

```text
SHA256d(x) = SHA256(SHA256(x))
```

The second hash always reads exactly 32 bytes, so it is always one block.

<!-- source: coax/lib/bitcoin/hash.coax sha256d -->
```jai
// SHA-256d: SHA-256 of the SHA-256 of the input, into the first 32 bytes
// of out. The first digest stays in a local buffer, so input and out are
// read and written once each.
sha256d_checked :: (input: [] u8, out: [] u8, overlap: bool) -> u32 {
  status := sha256_request(input.count, out.count, overlap); if status != SHA_OK { return status; }
  first: [32] u8;
  status = sha256_hash_checked(input, first[..], false); if status != SHA_OK { return status; }
  return sha256_hash_checked(first[..], out, false);
}
// Txids and block hashes are shown as the digest's bytes in reverse order.
// digest and out may be the same view.
bitcoin_reverse32 :: (digest: [] u8, out: [] u8) -> u32 {
  if digest.count != 32 { return SHA_BAD_INPUT; }
  if out.count != 32 { return SHA_BAD_OUTPUT; }
  copy: [32] u8; for i: 0..<32 { copy[i] = digest[i]; }
  for i: 0..<32 { out[i] = copy[31 - i]; }
  return SHA_OK;
}
```

### Worked example: the genesis block hash

A block header is 80 bytes:

| Bytes | Field |
| --- | --- |
| 0–3 | Version, little-endian |
| 4–35 | The previous block's hash |
| 36–67 | The Merkle root of the block's transactions |
| 68–71 | Time, little-endian |
| 72–75 | The compact target, `nBits` |
| 76–79 | The nonce |

The genesis block's header has version `01000000`, a previous hash of 32 zero
bytes, and a Merkle root field that starts `3ba3edfd`.

1. SHA-256 of the 80 bytes is `af42031e…71c5d66d`.
2. SHA-256 of those 32 bytes is
   `6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000`.
3. The last five bytes of that digest are zero. Bitcoin reads the digest as
   a 256-bit little-endian number when it compares it with the target, so
   those zero bytes are its most significant bytes.
4. Bitcoin shows the number most significant byte first, so it shows the
   bytes in reverse:
   `000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f`.

That last string is the genesis block hash that Bitcoin Core's
`chainparams.cpp` asserts. Txids follow the same rule: the bytes inside a
block are the digest, and the hex a block explorer shows is the digest
reversed.

### Worked example: the Merkle root field

The genesis block has one transaction, its coinbase. A Merkle tree with one
leaf has that leaf as its root, so the header's Merkle root field holds the
coinbase txid's digest bytes. Reverse the field's bytes:

```text
field (bytes 36-67):  3ba3edfd 7a7b12b2 ... 4b1e5e4a
reversed:             4a5e1e4b aab89f3a ... fdeda33b
```

`4a5e1e4b…fdeda33b` is the published coinbase txid, and the example below
computes it from the coinbase transaction's 204 bytes. [Chapter 46](PLAN.md)
builds Merkle trees with more than one leaf.

## The P2P checksum

Every message header in the original (v1) P2P transport carries the first 4
bytes of SHA-256d of the payload. A `verack` message has an empty payload,
so its checksum is the first 4 bytes of SHA-256d of nothing: `5df6e0e2`.

<!-- source: coax/lib/bitcoin/hash.coax checksum -->
```jai
// The P2P message checksum: the first 4 bytes of SHA-256d of the payload,
// into out[0..<4].
bitcoin_checksum_checked :: (payload: [] u8, out: [] u8, overlap: bool) -> u32 {
  if !sha256_input_ok(payload.count) { return SHA_BAD_INPUT; }
  if out.count < 4 { return SHA_BAD_OUTPUT; }
  if overlap { return SHA_ALIAS; }
  digest: [32] u8;
  status := sha256d_checked(payload, digest[..], false); if status != SHA_OK { return status; }
  for i: 0..<4 { out[i] = digest[i]; }
  return SHA_OK;
}
// True only when checksum is exactly 4 bytes and equals the payload's.
bitcoin_checksum_matches :: (payload: [] u8, checksum: [] u8) -> bool {
  if checksum.count != 4 { return false; }
  want: [4] u8;
  if bitcoin_checksum_checked(payload, want[..], false) != SHA_OK { return false; }
  same := true;
  for i: 0..<4 { if want[i] != checksum[i] { same = false; } }
  return same;
}
```

`bitcoin_checksum_matches` returns false for a checksum that isn't exactly 4
bytes, so a truncated header never matches. [Chapter 65](PLAN.md) parses
message headers.

## Tagged hashes

A tagged hash puts the tag's hash in front of the message twice:

```text
tagged(tag, x) = SHA256(SHA256(tag) || SHA256(tag) || x)
```

The tag separates domains. A hash computed for one purpose, such as a leaf of
a script tree, can't be passed off as a hash for another, such as a
signature message, because the two start from different prefixes. A plain
SHA-256 of `x` doesn't match either.

The prefix is 64 bytes, exactly one SHA-256 block. After it, the state holds
eight hash words and nothing buffered. That state, the tag's midstate, is
the same for every message under the tag, so a caller can compute it once
and stream each message after it. `sha256_tagged_init` returns that state:

<!-- source: coax/lib/crypto/sha256.coax tagged-init -->
```jai
// A fresh state that has absorbed SHA256(tag) || SHA256(tag): one full block,
// so the state holds the tag's midstate and nothing buffered.
sha256_tagged_init :: (state: [] u8, tag: string) -> u32 {
  if !sha256_tag_ok(tag.count) {return SHA_BAD_TAG;}
  if state.count != SHA_STATE_BYTES {return SHA_BAD_STATE;}
  digest:=S_hash(tag);if digest.error != 0 {return digest.error;}
  tag_hash:[32]u8;status:=sha256_emit(digest.words[..],tag_hash[..]);if status != SHA_OK {return status;}
  status=sha256_init(state);if status != SHA_OK {return status;}
  status=sha256_update_checked(state,tag_hash[..],false);if status != SHA_OK {return status;}
  return sha256_update_checked(state,tag_hash[..],false);
}
```

Bitcoin uses seven tags. `hash.coax` names each with an ID:

| ID | Tag | What it hashes | Where |
| --- | --- | --- | --- |
| 0 | `BIP0340/aux` | The signer's auxiliary randomness | Chapter 24 |
| 1 | `BIP0340/nonce` | The masked key, the public key and the message | Chapter 24 |
| 2 | `BIP0340/challenge` | R, the public key and the message | Chapter 23 |
| 3 | `TapLeaf` | Leaf version, script length and script | Chapter 42 |
| 4 | `TapBranch` | Two child hashes, the smaller first | Chapter 42 |
| 5 | `TapTweak` | The internal key, then the Merkle root if there is one | Chapter 41 |
| 6 | `TapSighash` | The signature message | Chapter 43 |

<!-- source: coax/lib/bitcoin/hash.coax tagged -->
```jai
// The tag string for a tag ID, or "" for an unknown ID.
bitcoin_tag_name :: (tag: int) -> string {
  if tag == BH_AUX { return "BIP0340/aux"; }
  if tag == BH_NONCE { return "BIP0340/nonce"; }
  if tag == BH_CHALLENGE { return "BIP0340/challenge"; }
  if tag == BH_TAPLEAF { return "TapLeaf"; }
  if tag == BH_TAPBRANCH { return "TapBranch"; }
  if tag == BH_TAPTWEAK { return "TapTweak"; }
  if tag == BH_TAPSIGHASH { return "TapSighash"; }
  return "";
}
// SHA256(SHA256(tag) || SHA256(tag) || input) for a tag ID.
bitcoin_tagged_checked :: (tag: int, input: [] u8, out: [] u8, overlap: bool) -> u32 {
  if tag < 0 || tag >= BH_TAGS { return SHA_BAD_TAG; }
  return sha256_tagged_checked(bitcoin_tag_name(tag), input, out, overlap);
}
// A streaming state for a tag ID that has already absorbed the 64-byte
// prefix. Update it with the message pieces, then finalize.
bitcoin_tagged_init :: (state: [] u8, tag: int) -> u32 {
  if tag < 0 || tag >= BH_TAGS { return SHA_BAD_TAG; }
  return sha256_tagged_init(state, bitcoin_tag_name(tag));
}
```

An unknown ID returns `SHA_BAD_TAG` and writes nothing.

### Worked example: a TapLeaf hash

BIP-341's fourth script-tree vector has a leaf with version `0xfa` (250) and
the 7-byte script `06424950333431`: a push of the six bytes `BIP341`. The
leaf hash's message is:

1. The leaf version: `fa`.
2. The script length as a compact size. 7 is less than 253, so it is one
   byte: `07`.
3. The script: `06424950333431`.

The message is the 9 bytes `fa0706424950333431`. With the 64-byte prefix,
the tagged hash reads 73 bytes, which is two blocks. Starting from the
`TapLeaf` midstate, it compresses one block: the 9 bytes, the padding and
the length.

### Worked example: a TapBranch hash

The same tree has two leaves, with hashes `8ad69ec7…` and `f224a923…`. A
branch hashes its two children in byte order, not tree order. The first
bytes are `0x8a` (138) and `0xf2` (242), and 138 is smaller, so `8ad69ec7…`
comes first. The branch hash is the tree's Merkle root, `6c2dc106…`.

## The example

The example is [bitcoin/examples/sha-256d.coax](../examples/sha-256d.coax).
It loads its vectors from
[coax/tests/bitcoin/sha256d_vectors.coax](../../coax/tests/bitcoin/sha256d_vectors.coax),
which `make_sha256d_vectors.py` copies from the pinned files. It hashes the
genesis header in two steps, computes the genesis block hash and coinbase
txid and three more txids, the empty-payload checksum, the tag hashes and
one hash for each BIP-341 tag. It then checks BIP-340 vector 0 with a
challenge this chapter computed, and shows seven refusals.

<!-- source: bitcoin/examples/sha-256d.coax example -->
```jai
#load "../../coax/lib/bitcoin/hash.coax";
#load "../../coax/lib/crypto/schnorr.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/sha256d_vectors.coax";
#import "Basic";

// Print bytes as lowercase hex and end the line.
hex_line :: (bytes: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%\n", cast(string) text[0..<r.written]);
}
// Print a label, then bytes as lowercase hex.
show :: (label: string, bytes: [] u8) { print("%: ", label); hex_line(bytes); }
// Decode hex into out and return the number of bytes written. Every
// literal here is valid hex, so a decoding error stops the program.
decode :: (text: string, out: [] u8) -> int {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return r.written;
}
// True when bytes, written as lowercase hex, are the text.
same_hex :: (bytes: [] u8, text: string) -> bool {
  hex: [128] u8;
  r := hex_encode_checked(cast(string) bytes, hex[..], false);
  return r.error == 0 && cast(string) hex[0..<r.written] == text;
}
// A txid or block hash as it is published: SHA-256d, bytes reversed.
published :: (input: [] u8, out: [] u8) {
  #assert sha256d_checked(input, out, false) == SHA_OK;
  #assert bitcoin_reverse32(out, out) == SHA_OK;
}
STATUS :: [8] string.["ok", "bad state", "bad input", "bad output",
  "already final", "overlap", "too long", "bad tag"];

main :: () {
  // The genesis block: an 80-byte header, a transaction count of 1 and
  // the coinbase transaction.
  block: [285] u8;
  print("genesis block: % bytes\n", decode(BH_GENESIS_BLOCK, block[..]));
  header := block[0..<80];
  show("header bytes 36-67 (Merkle root field)", header[36..<68]);
  first: [32] u8; twice: [32] u8; id: [32] u8;
  #assert sha256_hash_checked(header, first[..], false) == SHA_OK;
  show("SHA-256(header)", first[..]);
  #assert sha256_hash_checked(first[..], twice[..], false) == SHA_OK;
  show("SHA-256(SHA-256(header))", twice[..]);
  #assert sha256d_checked(header, id[..], false) == SHA_OK;
  print("sha256d_checked gives the same bytes: %\n", same_hex(id[..], "6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000"));
  #assert bitcoin_reverse32(id[..], id[..]) == SHA_OK;
  show("genesis block hash", id[..]);
  print("matches chainparams.cpp: %\n", same_hex(id[..], BH_GENESIS_HASH));

  // The coinbase transaction is the rest of the block after the count.
  print("transaction count byte: %\n", block[80]);
  published(block[81..<285], id[..]);
  show("coinbase txid", id[..]);
  print("matches the Merkle root in chainparams.cpp: %\n", same_hex(id[..], BH_GENESIS_MERKLE));

  // Three mainnet transactions from Bitcoin Core's tx_valid.json.
  for i: 0..<BH_TXS.count {
    tx: [512] u8;
    n := decode(BH_TXS[i].tx, tx[..]);
    published(tx[0..<n], id[..]);
    print("% bytes, txid ", n); hex_line(id[..]);
    print("  matches tx_valid.json: %\n", same_hex(id[..], BH_TXS[i].txid));
  }

  // The P2P checksum of an empty payload, the one every verack carries.
  check: [4] u8;
  #assert bitcoin_checksum_checked(block[0..<0], check[..], false) == SHA_OK;
  show("checksum of an empty payload", check[..]);

  // The tag hashes, and the midstate a tag leaves.
  for tag: 0..<BH_TAGS {
    if tag >= BH_CHALLENGE {
      t := S_hash(bitcoin_tag_name(tag)); bytes: [32] u8;
      #assert sha256_emit(t.words[..], bytes[..]) == SHA_OK;
      print("SHA-256(\"%\"): ", bitcoin_tag_name(tag)); hex_line(bytes[..]);
    }
  }
  state: [112] u8;
  #assert bitcoin_tagged_init(state[..], BH_TAPLEAF) == SHA_OK;
  print("TapLeaf state after the prefix: % bytes, % buffered\n", sha256_info(state[..]).total, sha256_info(state[..]).buffered);

  // TapLeaf: leaf version, script length, script.
  leaf := BH_LEAVES[3]; preimage: [9] u8; out: [32] u8;
  preimage[0] = cast(u8) leaf.version; preimage[1] = 7;
  #assert decode(leaf.script, preimage[2..<9]) == 7;
  show("TapLeaf preimage", preimage[..]);
  #assert sha256_update_checked(state[..], preimage[..], false) == SHA_OK;
  #assert sha256_final_checked(state[..], out[..], false) == SHA_OK;
  show("TapLeaf hash", out[..]);
  print("matches BIP-341: %\n", same_hex(out[..], leaf.hash));

  // TapBranch: the two child hashes, smaller first.
  a: [32] u8; b: [32] u8; pair: [64] u8;
  #assert decode(BH_LEAVES[2].hash, a[..]) == 32 && decode(BH_LEAVES[3].hash, b[..]) == 32;
  print("first bytes: % and %, so the % leaf goes first\n", a[0], b[0], ifx a[0] < b[0] then "left" else "right");
  for i: 0..<32 { pair[i] = a[i]; pair[32 + i] = b[i]; }
  #assert bitcoin_tagged_checked(BH_TAPBRANCH, pair[..], out[..], false) == SHA_OK;
  show("TapBranch hash", out[..]);
  print("matches BIP-341's Merkle root: %\n", same_hex(out[..], BH_TREES[3].merkle));

  // TapTweak: the internal key, then the Merkle root.
  #assert decode(BH_TREES[3].internal, pair[0..<32]) == 32;
  for i: 0..<32 { pair[32 + i] = out[i]; }
  #assert bitcoin_tagged_checked(BH_TAPTWEAK, pair[..], out[..], false) == SHA_OK;
  show("TapTweak hash", out[..]);
  print("matches BIP-341's tweak: %\n", same_hex(out[..], BH_TREES[3].tweak));

  // TapSighash over BIP-341's first signature message.
  message: [200] u8;
  n := decode(BH_SIGS[0].msg, message[..]);
  #assert bitcoin_tagged_checked(BH_TAPSIGHASH, message[0..<n], out[..], false) == SHA_OK;
  print("TapSighash of a % byte message: ", n); hex_line(out[..]);
  print("matches BIP-341's sigHash: %\n", same_hex(out[..], BH_SIGS[0].hash));

  // BIP0340/challenge: e for BIP-340 vector 0, and the signature checked
  // with that e.
  key: [32] u8; msg: [32] u8; sig: [64] u8; rpm: [96] u8;
  #assert decode(BH_BIP340_KEY, key[..]) == 32 && decode(BH_BIP340_MESSAGE, msg[..]) == 32;
  #assert decode(BH_BIP340_SIGNATURE, sig[..]) == 64;
  for i: 0..<32 { rpm[i] = sig[i]; rpm[32 + i] = key[i]; rpm[64 + i] = msg[i]; }
  #assert bitcoin_tagged_checked(BH_CHALLENGE, rpm[..], out[..], false) == SHA_OK;
  show("challenge hash for vector 0", out[..]);
  print("vector 0 verifies with this e: %\n", verifies_with(out[..], key[..], sig[..]));

  // Broken cases.
  bad: [80] u8; for i: 0..<80 { bad[i] = header[i]; }
  bad[79] ^= 1;
  published(bad[..], id[..]);
  show("header with its last bit flipped", id[..]);
  print("still the genesis hash: %\n", same_hex(id[..], BH_GENESIS_HASH));
  #assert sha256d_checked(header, id[..], false) == SHA_OK;
  print("digest bytes in hash order match the published hash: %\n", same_hex(id[..], BH_GENESIS_HASH));
  print("empty payload with its checksum: %\n", bitcoin_checksum_matches(block[0..<0], check[..]));
  check[0] ^= 1;
  print("empty payload with one checksum bit changed: %\n", bitcoin_checksum_matches(block[0..<0], check[..]));
  #assert sha256_hash_checked(message[0..<n], out[..], false) == SHA_OK;
  print("plain SHA-256 of the message matches sigHash: %\n", same_hex(out[..], BH_SIGS[0].hash));
  #assert sha256_tagged_checked("BIP0340/challengf", rpm[..], out[..], false) == SHA_OK;
  print("vector 0 verifies under the tag BIP0340/challengf: %\n", verifies_with(out[..], key[..], sig[..]));
  print("tag id 7: %\n", STATUS[bitcoin_tagged_checked(BH_TAGS, rpm[..], out[..], false)]);
  print("31-byte txid output: %\n", STATUS[sha256d_checked(header, out[0..<31], false)]);
  print("3-byte checksum output: %\n", STATUS[bitcoin_checksum_checked(header, check[0..<3], false)]);
}
// Chapter 23's verification equation, s*G - e*P has the x of r and an
// even y, with e reduced from a challenge hash this chapter computed.
verifies_with :: (challenge: [] u8, key: [] u8, sig: [] u8) -> bool {
  e := sc_reduce_bytes(challenge); s := sc_load(sig[32..<64]);
  r := sf_load_strict(sig[0..<32]); p := sg_lift_x_public(key);
  if !e.ok || !s.ok || !r.ok || !p.ok { return false; }
  return sv_prepared_matches_vartime(SvPrepared.{s = s.value, negative_e = sc_negate(e.value), point = p.value, r = r.value, ready = true});
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/sha-256d.coax
```

The output matches the values worked out by hand above:

```text
genesis block: 285 bytes
header bytes 36-67 (Merkle root field): 3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a
SHA-256(header): af42031e805ff493a07341e2f74ff58149d22ab9ba19f61343e2c86c71c5d66d
SHA-256(SHA-256(header)): 6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000
sha256d_checked gives the same bytes: true
genesis block hash: 000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f
matches chainparams.cpp: true
transaction count byte: 1
coinbase txid: 4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b
matches the Merkle root in chainparams.cpp: true
158 bytes, txid 23b397edccd3740a74adb603c9756370fafcde9bcc4483eb271ecad09a94dd63
  matches tx_valid.json: true
225 bytes, txid c99c49da4c38af669dea436d3e73780dfdb6c1ecf9958baa52960e8baee30e73
  matches tx_valid.json: true
402 bytes, txid f7fdd091fa6d8f5e7a8c2458f5c38faffff2d3f1406b6e4fe2c99dcc0d2d1cbb
  matches tx_valid.json: true
checksum of an empty payload: 5df6e0e2
SHA-256("BIP0340/challenge"): 7bb52d7a9fef58323eb1bf7a407db382d2f3f2d81bb1224f49fe518f6d48d37c
SHA-256("TapLeaf"): aeea8fdc4208983105734b58081d1e2638d35f1cb54008d4d357ca03be78e9ee
SHA-256("TapBranch"): 1941a1f2e56eb95fa2a9f194be5c01f7216f33ed82b091463490d05bf516a015
SHA-256("TapTweak"): e80fe1639c9ca050e3af1b39c143c63e429cbceb15d940fbb5c5a1f4af57c5e9
SHA-256("TapSighash"): f40a48df4b2a70c8b4924bf2654661ed3d95fd66a313eb87237597c628e4a031
TapLeaf state after the prefix: 64 bytes, 0 buffered
TapLeaf preimage: fa0706424950333431
TapLeaf hash: f224a923cd0021ab202ab139cc56802ddb92dcfc172b9212261a539df79a112a
matches BIP-341: true
first bytes: 138 and 242, so the left leaf goes first
TapBranch hash: 6c2dc106ab816b73f9d07e3cd1ef2c8c1256f519748e0813e4edd2405d277bef
matches BIP-341's Merkle root: true
TapTweak hash: 9e0517edc8259bb3359255400b23ca9507f2a91cd1e4250ba068b4eafceba4a9
matches BIP-341's tweak: true
TapSighash of a 175 byte message: 2514a6272f85cfa0f45eb907fcb0d121b808ed37c6ea160a5a9046ed5526d555
matches BIP-341's sigHash: true
challenge hash for vector 0: 6bb6b93a91f2ecc0cd924f4f9baabb5e6eb21745bb00f2cebdaac908bb5d86ce
vector 0 verifies with this e: true
header with its last bit flipped: 7423e9bb1c1ad68d63fed7b6256ced0271fbf363a86a618f0a894826de7ae1b4
still the genesis hash: false
digest bytes in hash order match the published hash: false
empty payload with its checksum: true
empty payload with one checksum bit changed: false
plain SHA-256 of the message matches sigHash: false
vector 0 verifies under the tag BIP0340/challengf: false
tag id 7: bad tag
31-byte txid output: bad output
3-byte checksum output: bad output
```

Every `matches` line compares a computed value with a published one:

- The genesis block hash and its Merkle root are the values Bitcoin Core's
  `chainparams.cpp` asserts.
- The three txids are the ones Bitcoin Core's `tx_valid.json` names in the
  comment before each transaction: mainnet transactions that the file keeps
  for their unusual signatures.
- The leaf hash, Merkle root, tweak and `sigHash` are BIP-341's
  `wallet-test-vectors.json` values. The `TapSighash` message is the
  vector's `sigMsg`, which starts with the epoch byte `00`.
- BIP-340 vector 0 verifies when e comes from this chapter's
  `BIP0340/challenge` hash. `verifies_with` runs [chapter 23](ch023-verify-a-schnorr-signature.md)'s
  equation, so a wrong challenge hash makes a valid signature fail.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/sha-256d.out](../fixtures/sha-256d.out).

## Broken cases

Each refusal or mismatch in the example shows one way to get these hashes
wrong:

- **A changed header.** Flipping the last bit of the nonce gives a hash
  with no leading zeros. Proof of work rests on this.
- **The wrong byte order.** The digest written in byte order isn't the
  published hash. Compare txids and block hashes in one order, and reverse
  only for display.
- **A changed checksum.** One flipped bit of the checksum fails
  `bitcoin_checksum_matches`.
- **No tag.** Plain SHA-256 of the signature message doesn't give its
  `sigHash`.
- **The wrong tag.** A challenge hashed under `BIP0340/challengf` makes
  vector 0, a valid signature, fail to verify.
- **An unknown tag ID, a 31-byte output and a 3-byte checksum output.**
  Each returns its status before it writes anything.

## How the code is checked

### Published vectors

The vectors are pinned byte for byte in `coax/tests/vectors/upstream/`, with
their source, Git revision, license and SHA-256 in
[coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md):

| File | What it gives |
| --- | --- |
| btcd `chaincfg/genesis_test.go` | The 285 bytes of the mainnet genesis block |
| Bitcoin Core `src/kernel/chainparams.cpp` | The genesis block hash and Merkle root |
| Bitcoin Core `src/test/data/tx_valid.json` | Mainnet transactions and the txids their comments name |
| BIP-341 `wallet-test-vectors.json` | Leaf hashes, Merkle roots, tweaks and signature hashes |
| BIP-340 `test-vectors.csv` | Vector 0's key, message and signature |

The genesis bytes come from btcd and the published hash from Bitcoin Core,
two independent implementations. To regenerate the Coax table from the
pinned files, and to check the pins:

```shell
python3 -I coax/tests/bitcoin/make_sha256d_vectors.py
python3 coax/tests/vectors/check_vectors.py
```

The generator refuses a file whose SHA-256 differs from its pin. It copies
values; it computes no hash.

### Checks in Coax

`hash.coax` checks SHA-256d of the empty string and `abc`, the four BIP-341
tags over `abc`, the checksum, the byte reversal in place, and that every
tag ID, the one-shot path and the midstate path give the same digest. Its
expected digests come from Python's `hashlib`. `coax mutate` leaves zero
survivors in `hash.coax` (102 mutants, run with `--root-only`).

The check file covers every vector in the pinned files, not only the ones
the example prints:

<!-- source: bitcoin/examples/checks/sha-256d.coax check -->
```jai
// Every pinned mainnet transaction hashes to the txid its tx_valid.json
// comment names, and the genesis block to chainparams.cpp's values.
#forall "pinned mainnet txids match their published values" i: 0..<3 {
  tx: [512] u8; id: [32] u8;
  n := decode(BH_TXS[i].tx, tx[..]);
  published(tx[0..<n], id[..]);
  #assert same_hex(id[..], BH_TXS[i].txid);
}
#test "the genesis block hash and coinbase txid match chainparams.cpp" {
  block: [285] u8; id: [32] u8;
  #assert decode(BH_GENESIS_BLOCK, block[..]) == 285;
  published(block[0..<80], id[..]); #assert same_hex(id[..], BH_GENESIS_HASH);
  published(block[81..<285], id[..]); #assert same_hex(id[..], BH_GENESIS_MERKLE);
}
// TapLeaf over leaf version, compact size and script (every script here is
// shorter than 253 bytes, so the size is one byte).
leaf_hash :: (leaf: BhLeaf, out: [] u8) {
  m: [64] u8; m[0] = cast(u8) leaf.version;
  n := decode(leaf.script, m[2..<64]); m[1] = cast(u8) n;
  #assert n < 253 && bitcoin_tagged_checked(BH_TAPLEAF, m[0..<n + 2], out, false) == SHA_OK;
}
// TapBranch over the two children, the smaller one first.
branch :: (a: [] u8, b: [] u8, out: [] u8) {
  left := true; decided := false;
  for i: 0..<32 { if !decided && a[i] != b[i] { left = a[i] < b[i]; decided = true; } }
  m: [64] u8;
  for i: 0..<32 { m[i] = ifx left then a[i] else b[i]; m[32 + i] = ifx left then b[i] else a[i]; }
  #assert bitcoin_tagged_checked(BH_TAPBRANCH, m[..], out, false) == SHA_OK;
}
// Every BIP-341 scriptPubKey case: each leaf hash, the Merkle root from
// the tree's shape, and the tweak over the internal key and that root.
#forall "BIP-341 leaf hashes, Merkle roots and tweaks match" t: 0..<7 {
  tree := BH_TREES[t];
  h: [3][32] u8; root: [32] u8; m: [64] u8; out: [32] u8;
  for k: 0..<tree.count {
    leaf_hash(BH_LEAVES[tree.first + k], h[k][..]);
    #assert same_hex(h[k][..], BH_LEAVES[tree.first + k].hash);
  }
  if tree.shape == 1 { for i: 0..<32 { root[i] = h[0][i]; } }
  if tree.shape == 2 { branch(h[0][..], h[1][..], root[..]); }
  if tree.shape == 3 { inner: [32] u8; branch(h[1][..], h[2][..], inner[..]); branch(h[0][..], inner[..], root[..]); }
  #assert decode(tree.internal, m[0..<32]) == 32;
  n := 32;
  if tree.shape != 0 {
    #assert same_hex(root[..], tree.merkle);
    for i: 0..<32 { m[32 + i] = root[i]; }
    n = 64;
  }
  #assert bitcoin_tagged_checked(BH_TAPTWEAK, m[0..<n], out[..], false) == SHA_OK;
  #assert same_hex(out[..], tree.tweak);
}
// Every BIP-341 key-path signature message gives its sigHash, in one call
// and split at every byte after a midstate.
#forall "BIP-341 signature hashes match, split at every byte" i: 0..<7 #steps 1000000000 {
  message: [200] u8; out: [32] u8;
  n := decode(BH_SIGS[i].msg, message[..]);
  #assert bitcoin_tagged_checked(BH_TAPSIGHASH, message[0..<n], out[..], false) == SHA_OK;
  #assert same_hex(out[..], BH_SIGS[i].hash);
  for cut: 0..<n + 1 {
    state: [112] u8;
    #assert bitcoin_tagged_init(state[..], BH_TAPSIGHASH) == SHA_OK;
    #assert sha256_update_checked(state[..], message[0..<cut], false) == SHA_OK;
    #assert sha256_update_checked(state[..], message[cut..<n], false) == SHA_OK;
    #assert sha256_final_checked(state[..], out[..], false) == SHA_OK;
    #assert same_hex(out[..], BH_SIGS[i].hash);
  }
}
// The challenge for BIP-340 vector 0 agrees with chapter 23's path, which
// computes the tag hash at compile time, and with the value Python's
// hashlib gives (chapter 23's check).
#test "the BIP0340/challenge hash agrees with chapter 23" #steps 1000000000 {
  key: [32] u8; msg: [32] u8; sig: [64] u8; rpm: [96] u8; out: [32] u8;
  #assert decode(BH_BIP340_KEY, key[..]) == 32 && decode(BH_BIP340_MESSAGE, msg[..]) == 32;
  #assert decode(BH_BIP340_SIGNATURE, sig[..]) == 64;
  for i: 0..<32 { rpm[i] = sig[i]; rpm[32 + i] = key[i]; rpm[64 + i] = msg[i]; }
  #assert bitcoin_tagged_checked(BH_CHALLENGE, rpm[..], out[..], false) == SHA_OK;
  #assert same_hex(out[..], "6bb6b93a91f2ecc0cd924f4f9baabb5e6eb21745bb00f2cebdaac908bb5d86ce");
  e := sv_challenge_vartime(msg[..], key[..], sig[0..<32]); mine := sc_reduce_bytes(out[..]);
  #assert e.ok && mine.ok && sc_equal(e.value, mine.value);
  #assert verifies_with(out[..], key[..], sig[..]);
  out[31] ^= 1; #assert !verifies_with(out[..], key[..], sig[..]);
}
```

- All three named txids and the genesis block.
- All seven BIP-341 script-tree vectors: twelve leaf hashes, six Merkle
  roots built from the tree's shape, and seven tweaks, one with no tree.
- All seven BIP-341 key-path signature messages, each in one call and split
  at every byte after the `TapSighash` midstate.
- The `BIP0340/challenge` hash for vector 0 against chapter 23's path and
  against the value Python's `hashlib` gives.

To run all the checks:

```shell
coax/bin/coax test bitcoin/examples/checks/sha-256d.coax
coax/bin/coax test coax/lib/bitcoin/hash.coax
coax/bin/coax mutate coax/lib/bitcoin/hash.coax --library --root-only
```

## Summary

- SHA-256d is SHA-256 of a SHA-256 digest. Txids, block hashes, Merkle
  nodes and P2P checksums use it.
- Txids and block hashes are shown with their bytes reversed, so the zeros
  of proof of work come first.
- A block with one transaction has the coinbase txid as its Merkle root.
- A tagged hash prefixes two copies of the tag's hash. The prefix is one
  block, so each tag has a fixed midstate.
- BIP-341 hashes leaves, branches, tweaks and signature messages each under
  its own tag.

## Dependencies

- [4. SHA-256](ch004-sha-256.md)

## Sources

- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki),
  "Design" and "Specification" (tagged hashes).
- [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki),
  "Specification" and its `wallet-test-vectors.json`.
- Bitcoin Core `src/kernel/chainparams.cpp` and `src/test/data/tx_valid.json`,
  and btcd `chaincfg/genesis_test.go`, pinned in
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
