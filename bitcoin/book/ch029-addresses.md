# 29. Addresses: P2PKH, P2SH, P2WPKH, P2WSH and P2TR

An address is a short name for a scriptPubKey, the script that locks an
output. Bitcoin has five standard kinds of output that a wallet gives an
address to. Each one commits to something different (a key's hash, a
script's hash or a tweaked key) and uses one of two encodings: chapter 26's
Base58Check or chapter 28's segwit Bech32 and Bech32m. This chapter builds
each scriptPubKey from what it commits to, turns any of them into its
address on mainnet, testnet or regtest, and turns an address back into its
scriptPubKey. It checks the result against every address in Bitcoin Core's
key I/O tests, BIP-86's Taproot vectors and a Python implementation.

## What this chapter teaches

- The five output types: what each scriptPubKey looks like, what it
  commits to (HASH160 of a compressed or uncompressed key, HASH160 of a
  redeem script, SHA-256 of a witness script, or an x-only tweaked key),
  and which encoding its address uses.
- scriptPubKey to address and back, for every type, on mainnet, testnet
  and regtest.
- P2SH-wrapped segwit (P2SH-P2WPKH), worked through BIP-49's vector.
- The Taproot key-path tweak Q = P + H_TapTweak(P)·G, for a key with no
  script tree.
- Why segwit refuses uncompressed keys, a policy rule.
- Unknown witness versions as addresses for outputs that future soft forks
  can define.
- Eight refusals of the chapter's own, with chapter 26's and chapter 28's
  refusals passed through under codes of their own.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [address.coax](../../coax/lib/bitcoin/address.coax) | `p2pkh_script_vartime(key, out)` | P2PKH from a 33- or 65-byte public key |
| | `p2wpkh_script_vartime(key, out)` | P2WPKH from a 33-byte public key |
| | `p2sh_script_vartime(redeem, out)` | P2SH from a redeem script of up to 520 bytes |
| | `p2wsh_script_vartime(witness_script, out)` | P2WSH from a witness script of up to 10,000 bytes |
| | `p2sh_p2wpkh_script_vartime(key, out)` | P2SH around a P2WPKH redeem script |
| | `taproot_tweak_vartime(internal, output)` | BIP-341's key-path tweak; writes Q's x and returns its y parity |
| | `p2tr_script_vartime(internal, out)` | P2TR from an x-only internal key |
| | `script_kind_vartime(script)` | Which of the five types (or a future witness version) a scriptPubKey is |
| | `address_encode_vartime(network, script, out)` | A scriptPubKey's address on a network |
| | `address_decode_vartime(network, text, script_out)` | An address's scriptPubKey and kind, on the network the caller expects |

The file builds on earlier chapters rather than repeating them:
`hash160_checked` from [chapter 6](ch006-ripemd-160-and-hash160.md),
`bitcoin_tagged_checked` from [chapter 5](ch005-sha-256d-and-tagged-hashes.md),
`se_decode`, `se_decode_xonly` and `se_encode` from
[chapter 17](ch017-point-encodings-compressed-uncompressed-and-x-only.md),
`ecmult_vartime` from the verifier's arithmetic in
[chapter 16](ch016-scalar-multiplication.md),
`base58_address_encode_vartime` and `base58_address_decode_vartime` from
[chapter 26](ch026-base58check.md), and `segwit_encode_vartime`,
`segwit_decode_vartime` and `sw_network` from [chapter 28](ch028-bech32m.md).

Keys, scripts and addresses are public, so every procedure here may branch
and index on its input; the `_vartime` suffix says so. Each procedure that
writes takes a caller-owned buffer and returns a count, or one negative
`AD_*` refusal, and a refusal writes nothing.

## Five outputs, five commitments

| Type | scriptPubKey | Commits to | Encoding | Mainnet address |
| --- | --- | --- | --- | --- |
| P2PKH | `OP_DUP OP_HASH160 <20> OP_EQUALVERIFY OP_CHECKSIG` (25 bytes) | HASH160 of a public key, compressed or uncompressed | Base58Check, version `0x00` (test networks `0x6f`) | `1…` |
| P2SH | `OP_HASH160 <20> OP_EQUAL` (23 bytes) | HASH160 of a redeem script | Base58Check, version `0x05` (test networks `0xc4`) | `3…` |
| P2WPKH | `OP_0 <20>` (22 bytes) | HASH160 of a compressed public key | Bech32, version 0 | `bc1q…` (42 characters) |
| P2WSH | `OP_0 <32>` (34 bytes) | SHA-256 of a witness script | Bech32, version 0 | `bc1q…` (62 characters) |
| P2TR | `OP_1 <32>` (34 bytes) | The x coordinate of the tweaked key Q | Bech32m, version 1 | `bc1p…` |

The legacy types put a hash between opcodes. Spending a P2PKH output means
giving a key whose HASH160 matches and a signature by it; spending a P2SH
output means giving a script whose HASH160 matches and the inputs that make
it succeed (BIP-16). The segwit types are a witness version and a witness
program, the pattern that BIP-141 gives meaning to: version 0 with a
20-byte program is a key hash, version 0 with a 32-byte program is a script
hash, and version 1 with a 32-byte program is a Taproot output key
(BIP-341).

P2WSH uses SHA-256 rather than HASH160 because a 20-byte script hash is
open to a collision attack when several parties build the script together:
finding two scripts with one HASH160 takes about 2⁸⁰ work, and with one
SHA-256 about 2¹²⁸. A key hash needs no such margin, because the key's
owner alone chooses it, so P2WPKH keeps HASH160 and its shorter address.

### The templates

Three shapes cover all five types: a hash between P2PKH's opcodes, a hash
between P2SH's, and a witness program. A witness version is one opcode:
`OP_0` (`0x00`) for version 0, and `OP_1` to `OP_16` (`0x51` to `0x60`)
for versions 1 to 16. The program follows as one push.

<!-- source: coax/lib/bitcoin/address.coax templates -->
```jai
// The two legacy templates around a 20-byte hash, and a witness program:
// the version as one opcode (OP_0, or OP_1 to OP_16 as 0x51 to 0x60), then
// one push of the program.
ad_hash_script :: (p2pkh: bool, hash: [] u8, out: [] u8) -> int {
  n := ifx p2pkh then 25 else 23;
  if out.count < n { return AD_CAPACITY; }
  at := 0;
  if p2pkh { out[0] = 0x76; at = 1; }                    // OP_DUP
  out[at] = 0xa9; out[at + 1] = 20;                      // OP_HASH160, push 20
  for i: 0..<20 { out[at + 2 + i] = hash[i]; }
  if p2pkh { out[23] = 0x88; out[24] = 0xac; }           // OP_EQUALVERIFY OP_CHECKSIG
  else { out[22] = 0x87; }                               // OP_EQUAL
  return n;
}
ad_witness_script :: (version: int, program: [] u8, out: [] u8) -> int {
  if out.count < program.count + 2 { return AD_CAPACITY; }
  out[0] = cast(u8) (ifx version == 0 then 0 else 0x50 + version);
  out[1] = cast(u8) program.count;
  for i: 0..<program.count { out[2 + i] = program[i]; }
  return program.count + 2;
}
```

## From a key: P2PKH and P2WPKH

Both commit to HASH160 of the key's encoding, so the same key in its two
forms gives two different P2PKH outputs. The generator G is the public key
of the private key 1:

| Form | HASH160 | P2PKH address |
| --- | --- | --- |
| Compressed, `0279be66…` | `751e76e8199196d454941c45d1b3a323f1433bd6` | `1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH` |
| Uncompressed, `0479be66…` | `91b24bf9f5288532960ac687abb035127b1d28a5` | `1EHNa6Q4Jz2uvNExL497mE43ikXhwF6kZm` |

A wallet must remember which form it hashed. Before compressed keys became
the default in 2012, wallets used the uncompressed form, and recovering
their coins needs that form, not the compressed one.

<!-- source: coax/lib/bitcoin/address.coax keys -->
```jai
// P2PKH commits to HASH160 of the key as given, compressed (33 bytes) or
// uncompressed (65): the two forms of one key are two different addresses.
p2pkh_script_vartime :: (key: [] u8, out: [] u8) -> int #export_c {
  if !se_decode(key).ok { return AD_BAD_KEY; }
  hash: [20] u8;
  #assert hash160_checked(key, hash[..], false) == RMD_OK;
  return ad_hash_script(true, hash[..], out);
}
// P2WPKH is version 0 with HASH160 of the key, which must be compressed.
p2wpkh_script_vartime :: (key: [] u8, out: [] u8) -> int #export_c {
  if !se_decode(key).ok { return AD_BAD_KEY; }
  if key.count != SE_COMPRESSED { return AD_UNCOMPRESSED; }
  hash: [20] u8;
  #assert hash160_checked(key, hash[..], false) == RMD_OK;
  return ad_witness_script(0, hash[..], out);
}
```

Each builder first decodes the key with chapter 17's `se_decode`, which
refuses a key of the wrong length, a hybrid prefix (`06` or `07`) and a
point not on the curve. A typo in a key would otherwise give a valid
address that nobody can spend from.

### Why segwit refuses uncompressed keys

Segwit version 0 makes uncompressed keys non-standard: BIP-143's
`WITNESS_PUBKEYTYPE` policy rule. A node that relays transactions refuses
to relay a spend from P2WPKH (or P2WSH) whose key is 65 bytes, so coins sent
to the HASH160 of an uncompressed key in a P2WPKH output can be confirmed
only by a miner who accepts that transaction directly. It is a policy rule,
not a consensus rule, which is why it is a refusal here and not part of
`script_kind_vartime`: a P2WPKH scriptPubKey holds only the hash, and
nothing in it says which form was hashed.

`p2wpkh_script_vartime` and `p2sh_p2wpkh_script_vartime` refuse a valid
65-byte key with `AD_UNCOMPRESSED`, so a wallet never creates such an
output. Taproot has no uncompressed form at all: its keys are 32-byte x
coordinates.

## From a script: P2SH and P2WSH

<!-- source: coax/lib/bitcoin/address.coax scripts -->
```jai
// P2SH commits to HASH160 of a redeem script; the spender reveals the
// script, so it must fit in one 520-byte push.
p2sh_script_vartime :: (redeem: [] u8, out: [] u8) -> int #export_c {
  if redeem.count > AD_REDEEM_MAX { return AD_SCRIPT_TOO_LONG; }
  hash: [20] u8;
  #assert hash160_checked(redeem, hash[..], false) == RMD_OK;
  return ad_hash_script(false, hash[..], out);
}
// P2WSH is version 0 with SHA-256 of a witness script: 32 bytes, so a
// collision costs 2^128 work instead of HASH160's 2^80.
p2wsh_script_vartime :: (witness_script: [] u8, out: [] u8) -> int #export_c {
  if witness_script.count > AD_WITNESS_MAX { return AD_SCRIPT_TOO_LONG; }
  hash: [32] u8;
  #assert sha256_hash_checked(witness_script, hash[..], false) == SHA_OK;
  return ad_witness_script(0, hash[..], out);
}
// P2SH-P2WPKH: the P2WPKH scriptPubKey becomes a P2SH redeem script, so a
// wallet that only knows Base58 addresses can pay a segwit key.
p2sh_p2wpkh_script_vartime :: (key: [] u8, out: [] u8) -> int #export_c {
  redeem: [22] u8;
  n := p2wpkh_script_vartime(key, redeem[..]);
  if n != 22 { return n; }
  return p2sh_script_vartime(redeem[..], out);
}
```

A P2SH spend pushes the redeem script as one stack element, and a push is
at most 520 bytes, so a longer redeem script could never be spent. A P2WSH
witness script may be up to 10,000 bytes (BIP-141). Both limits are
refusals here, before any output exists. The OP_TRUE script `51` shows the
two hashes side by side: its HASH160 is `da1745e9…` and its SHA-256 is
`4ae81572…`.

### Worked example: P2SH-P2WPKH

When segwit activated in 2017, many wallets could pay only Base58
addresses. P2SH-P2WPKH lets them pay a segwit key: the P2WPKH scriptPubKey
becomes a redeem script, and the output is a P2SH of it. The spender puts
the 22-byte redeem script in the scriptSig and the signature and key in the
witness. BIP-49 gives the derivation for testnet:

1. The key is `03a1af804ac108a8a51782198c2d034b28bf90c8803f5a53f76276fa69a4eae77f`.
2. Its HASH160 is `38971f73930f6c141d977ac4fd4a727c854935b3`.
3. The redeem script is `OP_0` and a push of that hash:
   `001438971f73930f6c141d977ac4fd4a727c854935b3`, the P2WPKH scriptPubKey.
4. The redeem script's HASH160 is `336caa13e08b96080a32b5d818d59b4ab3b36742`.
5. The scriptPubKey is `a914336caa13e08b96080a32b5d818d59b4ab3b3674287`.
6. Base58Check with the test P2SH version `0xc4` gives
   `2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2`. On mainnet, with `0x05`, the
   same output is `36NvZTcMsMowbt78wPzJaHHWaNiyR73Y4g`.

The address does not say that the script is segwit. Only the spend reveals
it. P2SH-P2WSH works the same way, with a P2WSH scriptPubKey as the redeem
script: `p2sh_script_vartime` of the result of `p2wsh_script_vartime`.

## The Taproot key-path tweak

A P2TR output commits to a key Q. BIP-341 builds Q from an internal key P
and an optional script tree; this chapter covers the key path with no
script tree, which BIP-86 specifies for single-key wallets:

1. P is `lift_x(x)`: the point with x coordinate x and an even y
   (chapter 17).
2. t is `H_TapTweak(x)`, the BIP-341 tagged hash of P's 32 bytes
   (chapter 5). With a script tree, the tree's Merkle root would follow x
   in the hash.
3. Q = P + t·G. BIP-341 refuses t ≥ n and Q at infinity; no known key
   reaches either.
4. The output key is Q's x coordinate, and the scriptPubKey is
   `OP_1 <32>` with it.

Q's y may be odd. The scriptPubKey keeps only x, and a script-path spend
reveals the parity in its control block, so `taproot_tweak_vartime`
returns it. A key-path signature uses the private key tweaked the same way
(chapter 24's signer, through the tweak in
[sighash.coax](../../coax/lib/bitcoin/sighash.coax)).

<!-- source: coax/lib/bitcoin/address.coax tweak -->
```jai
// BIP-341's key-path tweak with no script tree: P = lift_x(internal),
// t = H_TapTweak(internal) and Q = P + tG. Writes Q's x (32 bytes) and
// returns Q's y parity, 0 or 1, which a script-path spend reveals in its
// control block.
taproot_tweak_vartime :: (internal: [] u8, output: [] u8) -> int #export_c {
  if output.count < SE_XONLY { return AD_CAPACITY; }
  if internal.count != SE_XONLY { return AD_BAD_KEY; }
  p := se_decode_xonly(internal);
  if !p.ok { return AD_BAD_KEY; }
  t: [32] u8;
  #assert bitcoin_tagged_checked(BH_TAPTWEAK, internal, t[..], false) == SHA_OK;
  tweak := sc_load(t[..]);
  // t >= n has probability below 2^-127, and Q = infinity needs tG = -P:
  // no known key reaches either, but BIP-341 refuses both.
  if !tweak.ok { return AD_BAD_TWEAK; }  // mutant: equivalent (unreachable for any known key)
  q := ecmult_vartime(tweak.value, p.value, sc_small(1));   // tG + 1P
  c: [33] u8;
  if se_encode(q.value, SE_COMPRESSED, c[..]) != SE_COMPRESSED { return AD_BAD_TWEAK; }  // mutant: equivalent (as above)
  for i: 0..<32 { output[i] = c[1 + i]; }
  return cast(int) (c[0] & 1);
}
// P2TR is version 1 with the tweaked key's x.
p2tr_script_vartime :: (internal: [] u8, out: [] u8) -> int #export_c {
  if out.count < 34 { return AD_CAPACITY; }
  q: [32] u8;
  parity := taproot_tweak_vartime(internal, q[..]);
  if parity < 0 { return parity; }
  return ad_witness_script(1, q[..], out);
}
```

All the inputs are public, so Q = t·G + 1·P is computed with
`ecmult_vartime`, the verifier's two-scalar multiplication. The tweak of a
private key is a different procedure, because the key is secret.

Why tweak at all, when there is no script tree? Without the tweak, someone
who built the internal key from several parties' keys could hide a script
path in it that the other parties never agreed to. Committing to an empty
tree with `H_TapTweak(P)` proves there is none. BIP-86 requires it.

### Worked example: BIP-86's first address

BIP-86's first receiving key, `m/86'/0'/0'/0/0`, has the internal key
`cc8a4bc64d897bddc5fbc2f670f7a8ba0b386779106cf1223c6fc5d7cd6fc115`. Its tweak
is `2ca01ed85cf6b6526f73d39a1111cd80333bfdc00ce98992859848a90a6f0258`, and
Q's x is `a60869f0dbcf1dc659c9cecbaf8050135ea9e8cdc487053f1dc6880949dc684c`,
with an odd y. The scriptPubKey is `5120` and that x, and the address is
`bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr`.

## Unknown witness versions

Versions 2 to 16, and version 1 with a program that is not 32 bytes, have
no meaning yet. A soft fork can give one a meaning later: Taproot was
version 1 before it activated in 2021. Until then, consensus lets anyone
spend such an output, so a wallet must never create one for itself.

They still have addresses, and a wallet should still pay them. BIP-350
asks for this so that a new output type works in existing wallets on the
day it activates, without each wallet waiting for an upgrade. Here they
are the kind `AD_WITNESS`: they encode and decode like any segwit output.
BIP-350's `BC1SW50QGDZ25J`, version 16 with the 2-byte program `751e`, is
the scriptPubKey `6002751e`.

Version 0 has no such room: BIP-141 makes a version 0 program of any length
but 20 or 32 bytes fail, so `script_kind_vartime` gives it no address.

## From scriptPubKey to address

`script_kind_vartime` recognizes the two legacy templates byte for byte
and the witness program pattern from BIP-141: a version opcode, then one
push of 2 to 40 bytes that ends the script. Anything else, such as a P2PK
output (`<key> OP_CHECKSIG`) or an `OP_RETURN` output, has no address.

<!-- source: coax/lib/bitcoin/address.coax classify -->
```jai
// The output type of a scriptPubKey, or AD_NO_ADDRESS. A witness program is
// a version opcode then one push of 2 to 40 bytes that ends the script;
// version 0 has only two lengths.
script_kind_vartime :: (s: [] u8) -> int #export_c {
  n := s.count;
  if n == 25 && s[0] == 0x76 && s[1] == 0xa9 && s[2] == 20 && s[23] == 0x88 && s[24] == 0xac { return AD_P2PKH; }
  if n == 23 && s[0] == 0xa9 && s[1] == 20 && s[22] == 0x87 { return AD_P2SH; }
  if n < 4 || n > AD_SCRIPT_MAX || cast(int) s[1] != n - 2 { return AD_NO_ADDRESS; }
  if s[0] == 0 {
    if n == 22 { return AD_P2WPKH; }
    if n == 34 { return AD_P2WSH; }
    return AD_NO_ADDRESS;
  }
  if s[0] < 0x51 || s[0] > 0x60 { return AD_NO_ADDRESS; }
  if s[0] == 0x51 && n == 34 { return AD_P2TR; }
  return AD_WITNESS;
}
```

<!-- source: coax/lib/bitcoin/address.coax encode -->
```jai
// The address of a scriptPubKey on a network. Testnet and regtest share
// the Base58 versions; segwit gives each its own HRP.
address_encode_vartime :: (network: int, script: [] u8, out: [] u8) -> int #export_c {
  if network < SW_MAINNET || network > SW_REGTEST { return AD_BAD_NETWORK; }
  kind := script_kind_vartime(script);
  if kind < AD_P2PKH { return kind; }
  n: int;
  if kind == AD_P2PKH {
    n = base58_address_encode_vartime(ifx network == SW_MAINNET then B58_MAIN_P2PKH else B58_TEST_P2PKH, script[3..<23], out);
  } else if kind == AD_P2SH {
    n = base58_address_encode_vartime(ifx network == SW_MAINNET then B58_MAIN_P2SH else B58_TEST_P2SH, script[2..<22], out);
  } else {
    version := ifx script[0] == 0 then 0 else cast(int) script[0] - 0x50;
    n = segwit_encode_vartime(network, version, script[2..<script.count], out);
  }
  // The kind is checked, so the only refusal left is the buffer's size.
  if n == B58_CAPACITY || n == SW_CAPACITY { return AD_CAPACITY; }
  return n;
}
```

Testnet, testnet4, signet and regtest share the Base58 versions, so their
legacy addresses are the same. The segwit HRPs separate regtest (`bcrt`)
from the rest (`tb`).

## From address to scriptPubKey

The decoder takes the network the caller expects, as Bitcoin Core does. An
address for another network refuses with `AD_WRONG_NETWORK`, so a testnet
address pasted into a mainnet wallet never becomes a payment.

A string is read as segwit when the part before its last `1` is `bc`, `tb`
or `bcrt`, in either case; anything else is read as Base58Check. A valid
Base58 address with one of the four versions starts with `1`, `3`, `m`,
`n` or `2`, so it is never read as segwit. Bitcoin Core tries Base58 first
and then Bech32; the two orders accept and refuse the same strings and
differ only in which reason a refusal gives.

<!-- source: coax/lib/bitcoin/address.coax decode -->
```jai
// Whether text is read as segwit: the part before its last '1' is bc, tb
// or bcrt in either case. A Base58 address with a known version starts
// with 1, 3, m, n or 2, so it never is.
ad_is_segwit :: (text: string) -> bool {
  if text.count == 0 { return false; }
  at := text.count - 1;
  while at > 0 && text[at] != '1' { at -= 1; }  // mutant: equivalent (stopping at 0 or 1 leaves no HRP)
  return sw_network(text[0..<at]) != SW_UNKNOWN_HRP;
}
// The scriptPubKey an address names on a network, and its kind.
address_decode_vartime :: (network: int, text: string, script_out: [] u8) -> Address_Script #export_c {
  if network < SW_MAINNET || network > SW_REGTEST { return Address_Script.{length = AD_BAD_NETWORK}; }
  script: [AD_SCRIPT_MAX] u8;
  n: int;
  if ad_is_segwit(text) {
    program: [40] u8;
    a := segwit_decode_vartime(text, program[..]);
    if a.length < 0 { return Address_Script.{length = AD_SEGWIT + a.length}; }  // mutant: equivalent (a program is never empty)
    if a.network != network { return Address_Script.{length = AD_WRONG_NETWORK}; }
    n = ad_witness_script(a.version, program[0..<a.length], script[..]);
  } else {
    hash: [20] u8;
    kind := base58_address_decode_vartime(text, hash[..]);
    if kind < B58_MAIN_P2PKH { return Address_Script.{length = AD_BASE58 + kind}; }
    mainnet := kind == B58_MAIN_P2PKH || kind == B58_MAIN_P2SH;
    if mainnet != (network == SW_MAINNET) { return Address_Script.{length = AD_WRONG_NETWORK}; }
    n = ad_hash_script(kind == B58_MAIN_P2PKH || kind == B58_TEST_P2PKH, hash[..], script[..]);
  }
  if script_out.count < n { return Address_Script.{length = AD_CAPACITY}; }
  for i: 0..<n { script_out[i] = script[i]; }
  return Address_Script.{length = n, kind = script_kind_vartime(script[0..<n])};
}
```

## Refusals

<!-- source: coax/lib/bitcoin/address.coax kinds -->
```jai
// The output types. A witness program of version 1 to 16 that is not
// Taproot's has an address but no meaning yet: AD_WITNESS.
AD_P2PKH :: 1; AD_P2SH :: 2; AD_P2WPKH :: 3; AD_P2WSH :: 4; AD_P2TR :: 5; AD_WITNESS :: 6;
AD_SCRIPT_MAX :: 42;        // the longest scriptPubKey with an address: OP_16, a push of 40, 40 bytes
AD_REDEEM_MAX :: 520;       // the largest push, so the largest P2SH redeem script
AD_WITNESS_MAX :: 10000;    // the largest P2WSH witness script (BIP-141)
```

<!-- source: coax/lib/bitcoin/address.coax refusals -->
```jai
AD_BAD_NETWORK :: -1;       // a network outside 1-3
AD_WRONG_NETWORK :: -2;     // a valid address for another network
AD_NO_ADDRESS :: -3;        // a scriptPubKey that no address encodes
AD_BAD_KEY :: -4;           // not a public key: a wrong length or prefix, or a point not on the curve
AD_UNCOMPRESSED :: -5;      // an uncompressed key where segwit wants a compressed one
AD_SCRIPT_TOO_LONG :: -6;   // a redeem script over 520 bytes or a witness script over 10,000
AD_BAD_TWEAK :: -7;         // a TapTweak hash of at least n, or Q at infinity
AD_CAPACITY :: -8;          // the output buffer is too small
// Refusals from the encodings underneath keep their own codes, offset:
AD_BASE58 :: -20;           // plus chapter 26's B58_* code: -21 to -27
AD_SEGWIT :: -40;           // plus chapter 28's SW_* code: -41 to -52
```

A refusal from the encoding underneath keeps chapter 26's or chapter 28's
reason, offset so that no two codes collide: a Base58 checksum failure is
`AD_BASE58 + B58_BAD_CHECKSUM`, -22, and a Bech32m checksum on a version 0
address is `AD_SEGWIT + SW_WRONG_VARIANT`, -48. `AD_BAD_TWEAK` exists
because BIP-341 defines the failure; no test can reach it.

## The example

[bitcoin/examples/addresses.coax](../examples/addresses.coax) builds
each type from a key or script, prints its address on all three networks,
works the P2SH-P2WPKH and Taproot examples, shows scripts with no address,
decodes addresses back and prints one refusal of each common kind.

<!-- source: bitcoin/examples/addresses.coax example -->
```jai
#load "../../coax/lib/bitcoin/address.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [200] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Hex into a buffer.
unhex :: (hex: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(hex, out, false);
  #assert r.error == NC_OK;
  return out[0..<r.written];
}
KIND_NAMES :: [7] string.["", "P2PKH", "P2SH", "P2WPKH", "P2WSH", "P2TR", "witness"];
NETWORK_NAMES :: [5] string.["", "mainnet", "testnet", "regtest", "network 4"];
// The name of a refusal.
refusal :: (code: int) -> string {
  own := [8] string.["bad network", "wrong network", "no address", "bad key", "uncompressed key",
                     "script too long", "bad tweak", "capacity"];
  base58 := [7] string.["Base58 bad character", "Base58 bad checksum", "Base58 bad length",
                        "Base58 unknown version", "Base58 bad flag", "Base58 capacity", "Base58 too long"];
  segwit := [12] string.["segwit too long", "segwit mixed case", "segwit bad format", "segwit bad checksum",
                         "segwit unknown HRP", "segwit no version", "segwit bad version", "segwit wrong variant",
                         "segwit bad padding", "segwit bad length", "segwit bad v0 length", "segwit capacity"];
  if code < 0 && code >= -8 { return own[-code - 1]; }
  if code < AD_BASE58 && code >= AD_BASE58 - 7 { return base58[AD_BASE58 - code - 1]; }
  if code < AD_SEGWIT && code >= AD_SEGWIT - 12 { return segwit[AD_SEGWIT - code - 1]; }
  return "?";
}
// A scriptPubKey, its kind and its address on each network.
show_script :: (label: string, script: [] u8) {
  show(label, script);
  kind := script_kind_vartime(script);
  if kind < 0 { print("  %\n", refusal(kind)); return; }
  print("  kind: %\n", KIND_NAMES[kind]);
  text: [90] u8;
  for network: SW_MAINNET..<SW_REGTEST + 1 {
    n := address_encode_vartime(network, script, text[..]);
    print("  %: %\n", NETWORK_NAMES[network], cast(string) text[0..<n]);
  }
}
// Decode an address on a network and print its scriptPubKey, or the refusal.
try_decode :: (network: int, text: string) {
  script: [AD_SCRIPT_MAX] u8;
  a := address_decode_vartime(network, text, script[..]);
  if a.length < 0 { print("% on %: %\n", text, NETWORK_NAMES[network], refusal(a.length)); return; }
  print("% on %: %, ", text, NETWORK_NAMES[network], KIND_NAMES[a.kind]);
  show("script", script[0..<a.length]);
}

// G compressed and uncompressed: the public key of the private key 1.
G_C :: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
G_U :: "0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8";
// BIP-49's key and BIP-86's first internal key.
BIP49_KEY :: "03a1af804ac108a8a51782198c2d034b28bf90c8803f5a53f76276fa69a4eae77f";
BIP86_INTERNAL :: "cc8a4bc64d897bddc5fbc2f670f7a8ba0b386779106cf1223c6fc5d7cd6fc115";

main :: () {
  b1: [65] u8; b2: [65] u8; script: [AD_SCRIPT_MAX] u8; hash: [32] u8;
  compressed := unhex(G_C, b1[..]);
  uncompressed := unhex(G_U, b2[..]);

  // P2PKH: one key, two addresses.
  print("P2PKH\n");
  #assert hash160_checked(compressed, hash[0..<20], false) == RMD_OK;
  show("HASH160(G compressed)", hash[0..<20]);
  show_script("scriptPubKey", script[0..<p2pkh_script_vartime(compressed, script[..])]);
  #assert hash160_checked(uncompressed, hash[0..<20], false) == RMD_OK;
  show("HASH160(G uncompressed)", hash[0..<20]);
  show_script("scriptPubKey", script[0..<p2pkh_script_vartime(uncompressed, script[..])]);

  // P2WPKH: the same HASH160, as a version 0 program.
  print("P2WPKH\n");
  show_script("scriptPubKey", script[0..<p2wpkh_script_vartime(compressed, script[..])]);
  print("uncompressed: %\n", refusal(p2wpkh_script_vartime(uncompressed, script[..])));

  // P2SH-P2WPKH, BIP-49's vector.
  print("P2SH-P2WPKH\n");
  b3: [33] u8; redeem: [22] u8;
  key := unhex(BIP49_KEY, b3[..]);
  #assert hash160_checked(key, hash[0..<20], false) == RMD_OK;
  show("key hash", hash[0..<20]);
  n := p2wpkh_script_vartime(key, redeem[..]);
  show("redeem script", redeem[0..<n]);
  #assert hash160_checked(redeem[..], hash[0..<20], false) == RMD_OK;
  show("HASH160(redeem script)", hash[0..<20]);
  show_script("scriptPubKey", script[0..<p2sh_p2wpkh_script_vartime(key, script[..])]);

  // P2SH and P2WSH of the one-byte script OP_TRUE.
  print("P2SH and P2WSH of OP_TRUE (51)\n");
  op_true := [1] u8.[0x51];
  show_script("P2SH", script[0..<p2sh_script_vartime(op_true[..], script[..])]);
  show_script("P2WSH", script[0..<p2wsh_script_vartime(op_true[..], script[..])]);
  long: [521] u8;
  print("a 521-byte redeem script: %\n", refusal(p2sh_script_vartime(long[..], script[..])));

  // P2TR: BIP-86's first key, tweaked with no script tree.
  print("P2TR\n");
  b4: [32] u8; q: [32] u8;
  internal := unhex(BIP86_INTERNAL, b4[..]);
  show("internal key P", internal);
  #assert bitcoin_tagged_checked(BH_TAPTWEAK, internal, hash[..], false) == SHA_OK;
  show("t = H_TapTweak(P)", hash[..]);
  parity := taproot_tweak_vartime(internal, q[..]);
  show("Q = P + tG, x", q[..]);
  print("Q's y parity: %\n", parity);
  show_script("scriptPubKey", script[0..<p2tr_script_vartime(internal, script[..])]);
  print("G compressed as an internal key: %\n", refusal(p2tr_script_vartime(compressed, script[..])));

  // A future witness version: an address, and no meaning yet.
  print("Unknown witness versions\n");
  v2 := [18] u8.[0x52, 16, 0x75, 0x1e, 0x76, 0xe8, 0x19, 0x91, 0x96, 0xd4, 0x54, 0x94, 0x1c, 0x45, 0xd1, 0xb3, 0xa3, 0x23];
  show_script("version 2", v2[..]);

  // Scripts with no address.
  print("No address\n");
  p2pk := [35] u8.[0x21, 0x02, 0x79, 0xbe, 0x66, 0x7e, 0xf9, 0xdc, 0xbb, 0xac, 0x55, 0xa0, 0x62, 0x95, 0xce, 0x87,
                   0x0b, 0x07, 0x02, 0x9b, 0xfc, 0xdb, 0x2d, 0xce, 0x28, 0xd9, 0x59, 0xf2, 0x81, 0x5b, 0x16, 0xf8,
                   0x17, 0x98, 0xac];
  show_script("P2PK", p2pk[..]);
  op_return := [6] u8.[0x6a, 4, 0xde, 0xad, 0xbe, 0xef];
  show_script("OP_RETURN", op_return[..]);
  v0_21 := [23] u8.[0, 21, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21];
  show_script("version 0, 21 bytes", v0_21[..]);

  // Addresses back to scriptPubKeys.
  print("Decoding\n");
  try_decode(SW_MAINNET, "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH");
  try_decode(SW_TESTNET, "2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2");
  try_decode(SW_REGTEST, "2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2");
  try_decode(SW_MAINNET, "BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4");
  try_decode(SW_MAINNET, "bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr");
  try_decode(SW_MAINNET, "BC1SW50QGDZ25J");
  try_decode(SW_REGTEST, "bcrt1qdavt4j2sd7dlhqsavtnfxvzppw6k7qy97tmnu9");

  // Refusals.
  print("Refusals\n");
  try_decode(4, "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH");
  try_decode(SW_MAINNET, "2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2");
  try_decode(SW_TESTNET, "bcrt1qdavt4j2sd7dlhqsavtnfxvzppw6k7qy97tmnu9");
  try_decode(SW_MAINNET, "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMJ");
  try_decode(SW_MAINNET, "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqh2y7hd");
  b5: [24] u8;
  print("P2PKH into 24 bytes: %\n", refusal(address_decode_vartime(SW_MAINNET, "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH", b5[..]).length));
  print("bad key: %\n", refusal(p2pkh_script_vartime(compressed[1..<33], script[..])));
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/addresses.coax
```

```text
P2PKH
HASH160(G compressed): 751e76e8199196d454941c45d1b3a323f1433bd6
scriptPubKey: 76a914751e76e8199196d454941c45d1b3a323f1433bd688ac
  kind: P2PKH
  mainnet: 1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH
  testnet: mrCDrCybB6J1vRfbwM5hemdJz73FwDBC8r
  regtest: mrCDrCybB6J1vRfbwM5hemdJz73FwDBC8r
HASH160(G uncompressed): 91b24bf9f5288532960ac687abb035127b1d28a5
scriptPubKey: 76a91491b24bf9f5288532960ac687abb035127b1d28a588ac
  kind: P2PKH
  mainnet: 1EHNa6Q4Jz2uvNExL497mE43ikXhwF6kZm
  testnet: mtoKs9V381UAhUia3d7Vb9GNak8Qvmcsme
  regtest: mtoKs9V381UAhUia3d7Vb9GNak8Qvmcsme
P2WPKH
scriptPubKey: 0014751e76e8199196d454941c45d1b3a323f1433bd6
  kind: P2WPKH
  mainnet: bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4
  testnet: tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx
  regtest: bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080
uncompressed: uncompressed key
P2SH-P2WPKH
key hash: 38971f73930f6c141d977ac4fd4a727c854935b3
redeem script: 001438971f73930f6c141d977ac4fd4a727c854935b3
HASH160(redeem script): 336caa13e08b96080a32b5d818d59b4ab3b36742
scriptPubKey: a914336caa13e08b96080a32b5d818d59b4ab3b3674287
  kind: P2SH
  mainnet: 36NvZTcMsMowbt78wPzJaHHWaNiyR73Y4g
  testnet: 2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2
  regtest: 2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2
P2SH and P2WSH of OP_TRUE (51)
P2SH: a914da1745e9b549bd0bfa1a569971c77eba30cd5a4b87
  kind: P2SH
  mainnet: 3MaB7QVq3k4pQx3BhsvEADgzQonLSBwMdj
  testnet: 2ND8PB9RrfCaAcjfjP1Y6nAgFd9zWHYX4DN
  regtest: 2ND8PB9RrfCaAcjfjP1Y6nAgFd9zWHYX4DN
P2WSH: 00204ae81572f06e1b88fd5ced7a1a000945432e83e1551e6f721ee9c00b8cc33260
  kind: P2WSH
  mainnet: bc1qft5p2uhsdcdc3l2ua4ap5qqfg4pjaqlp250x7us7a8qqhrxrxfsq2gp3gp
  testnet: tb1qft5p2uhsdcdc3l2ua4ap5qqfg4pjaqlp250x7us7a8qqhrxrxfsqaqh7jw
  regtest: bcrt1qft5p2uhsdcdc3l2ua4ap5qqfg4pjaqlp250x7us7a8qqhrxrxfsqseac85
a 521-byte redeem script: script too long
P2TR
internal key P: cc8a4bc64d897bddc5fbc2f670f7a8ba0b386779106cf1223c6fc5d7cd6fc115
t = H_TapTweak(P): 2ca01ed85cf6b6526f73d39a1111cd80333bfdc00ce98992859848a90a6f0258
Q = P + tG, x: a60869f0dbcf1dc659c9cecbaf8050135ea9e8cdc487053f1dc6880949dc684c
Q's y parity: 1
scriptPubKey: 5120a60869f0dbcf1dc659c9cecbaf8050135ea9e8cdc487053f1dc6880949dc684c
  kind: P2TR
  mainnet: bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr
  testnet: tb1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqp3mvzv
  regtest: bcrt1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqvg32hk
G compressed as an internal key: bad key
Unknown witness versions
version 2: 5210751e76e8199196d454941c45d1b3a323
  kind: witness
  mainnet: bc1zw508d6qejxtdg4y5r3zarvaryvaxxpcs
  testnet: tb1zw508d6qejxtdg4y5r3zarvaryv5p829g
  regtest: bcrt1zw508d6qejxtdg4y5r3zarvaryv2wuatf
No address
P2PK: 210279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798ac
  no address
OP_RETURN: 6a04deadbeef
  no address
version 0, 21 bytes: 00150102030405060708090a0b0c0d0e0f101112131415
  no address
Decoding
1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH on mainnet: P2PKH, script: 76a914751e76e8199196d454941c45d1b3a323f1433bd688ac
2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2 on testnet: P2SH, script: a914336caa13e08b96080a32b5d818d59b4ab3b3674287
2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2 on regtest: P2SH, script: a914336caa13e08b96080a32b5d818d59b4ab3b3674287
BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4 on mainnet: P2WPKH, script: 0014751e76e8199196d454941c45d1b3a323f1433bd6
bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr on mainnet: P2TR, script: 5120a60869f0dbcf1dc659c9cecbaf8050135ea9e8cdc487053f1dc6880949dc684c
BC1SW50QGDZ25J on mainnet: witness, script: 6002751e
bcrt1qdavt4j2sd7dlhqsavtnfxvzppw6k7qy97tmnu9 on regtest: P2WPKH, script: 00146f58bac9506f9bfb821d62e69330410bb56f0085
Refusals
1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH on network 4: bad network
2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2 on mainnet: wrong network
bcrt1qdavt4j2sd7dlhqsavtnfxvzppw6k7qy97tmnu9 on testnet: wrong network
1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMJ on mainnet: Base58 bad checksum
bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqh2y7hd on mainnet: segwit wrong variant
P2PKH into 24 bytes: capacity
bad key: bad key
```

What the lines show:

- One key, G, gives two P2PKH addresses: one for each form. Testnet and
  regtest share the legacy address.
- P2WPKH of G is BIP-173's first address, `bc1qw508d6…`, with the same
  HASH160 as the compressed P2PKH. The HRP changes the checksum on each
  network. The uncompressed key refuses.
- The P2SH-P2WPKH lines are BIP-49's vector, step by step.
- The OP_TRUE script hashes to a 20-byte P2SH and a 32-byte P2WSH, and a
  521-byte redeem script refuses.
- The P2TR lines are BIP-86's first address: the tweak, Q's x with an odd
  y, the scriptPubKey and the address. A 33-byte key is not an internal
  key.
- A version 2 program has an address on each network.
- P2PK, `OP_RETURN` and a 21-byte version 0 program have none.
- Decoding gives back each scriptPubKey and its kind; a test-network legacy
  address decodes on testnet and regtest alike.
- The refusals: a network outside 1 to 3, a testnet address on mainnet, a
  regtest address on testnet, a Base58 checksum, a Bech32 checksum on a
  version 1 address, a 24-byte buffer for a 25-byte script, and a key
  without its prefix byte.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/addresses.out](../fixtures/addresses.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/addresses.coax](../examples/checks/addresses.coax),
which loads the example, chapter 28's
[bech32m_vectors.coax](../../coax/tests/bitcoin/bech32m_vectors.coax) and
[address_vectors.coax](../../coax/tests/bitcoin/address_vectors.coax).
[make_address_vectors.py](../../coax/tests/bitcoin/make_address_vectors.py)
writes the last by copying keys, scripts and addresses from Bitcoin Core's
`key_io_valid.json`, BIP-86, BIP-341's `wallet-test-vectors.json`, BIP-84
and BIP-49, after checking each file's SHA-256. The checks:

- all 54 addresses of `key_io_valid.json` (22 Base58 and 32 segwit, on
  mainnet, testnet4, signet and regtest) decode to their scriptPubKey on
  their network and encode back exactly. The 32 segwit ones also decode in
  the other case. On another network each refuses, except that a
  test-network Base58 address decodes on both testnet and regtest;
- all 70 strings of `key_io_invalid.json` refuse on every network, with
  nothing written;
- every BIP-350 and BIP-173 segwit address: each valid one gives the
  scriptPubKey of its version and program and encodes back in lower case,
  and each invalid one refuses with chapter 28's code, offset;
- BIP-86's three key-path outputs and BIP-341's case with no script tree:
  the internal key tweaks to the published output key, which gives the
  published scriptPubKey and address, and BIP-341's tweak hash matches;
- BIP-84's three keys give its P2WPKH addresses. Uncompressed, they refuse
  as P2WPKH and P2SH-P2WPKH, and as P2PKH give a different hash;
- BIP-49's P2SH-P2WPKH vector, step by step.

<!-- source: bitcoin/examples/checks/addresses.coax check -->
```jai
// Every address in key_io_valid.json decodes on its network to its
// scriptPubKey and encodes back exactly; a segwit one also decodes in
// upper case. On another network it refuses, except that testnet and
// regtest share the Base58 versions.
#forall "every address in key_io_valid.json, both ways" i: 0..<54 #steps 200000000 {
  c := AD_KEY_IO[i];
  script: [AD_SCRIPT_MAX] u8; want: [AD_SCRIPT_MAX] u8; text: [90] u8;
  a := address_decode_vartime(c.network, c.text, script[..]);
  w := unhex(c.script, want[..]);
  #assert a.length == w.count && a.kind == script_kind_vartime(w) && a.kind > 0;
  for k: 0..<w.count { #assert script[k] == w[k]; }
  n := address_encode_vartime(c.network, w, text[..]);
  #assert n == c.text.count;
  for k: 0..<n { #assert text[k] == c.text[k]; }
  if c.flip {
    for k: 0..<n { text[k] = ifx text[k] >= 'a' && text[k] <= 'z' then text[k] - 32 else text[k]; }
    #assert address_decode_vartime(c.network, cast(string) text[0..<n], script[..]).length == w.count;
  }
  for other: SW_MAINNET..<SW_REGTEST + 1 {
    shared := !ad_is_segwit(c.text) && other != SW_MAINNET && c.network != SW_MAINNET;
    r := address_decode_vartime(other, c.text, script[..]);
    if other == c.network || shared { #assert r.length == w.count && r.kind == a.kind; }
    else { #assert r.length == AD_WRONG_NETWORK; }
  }
}
#forall "every string in key_io_invalid.json refuses on every network" i: 0..<70, network: 1..<4 #steps 200000000 {
  out: [AD_SCRIPT_MAX] u8;
  code := address_decode_vartime(network, SW_KEY_IO_INVALID[i], out[..]).length;
  #assert code < 0 && ad_refuses(network, SW_KEY_IO_INVALID[i], code);
}
// Every BIP-350 and BIP-173 segwit address: a valid one gives the
// scriptPubKey of its version and program and encodes back in lower case;
// an invalid one refuses with chapter 28's code, offset.
#forall "every BIP-173 and BIP-350 segwit address" i: 0..<71 #steps 200000000 {
  c := SW_CASES[i];
  script: [AD_SCRIPT_MAX] u8; program: [40] u8; text: [90] u8;
  if c.code == 0 {
    a := address_decode_vartime(c.network, c.text, script[..]);
    p := sw_unhex(c.program, program[..]);
    #assert a.length == p.count + 2 && script[0] == cast(u8) (ifx c.version == 0 then 0 else 0x50 + c.version);
    #assert cast(int) script[1] == p.count && sw_same(script[2..<a.length], cast(string) p);
    n := address_encode_vartime(c.network, script[0..<a.length], text[..]);
    #assert n == c.text.count;
    for k: 0..<n { #assert text[k] == b32_lower(c.text[k]); }
  } else {
    code := address_decode_vartime(SW_MAINNET, c.text, script[..]).length;
    #assert code < 0 && ad_refuses(SW_MAINNET, c.text, code);
    if ad_is_segwit(c.text) { #assert code == AD_SEGWIT + c.code; }
  }
}
// BIP-86's three key-path outputs and BIP-341's case with no script tree:
// the internal key tweaks to the output key, which makes the scriptPubKey
// and the address, and the address decodes back.
#forall "BIP-86 and BIP-341 key-path Taproot outputs" i: 0..<4 #steps 1000000000 {
  c := ifx i < 3 then AD_BIP86[i] else AD_BIP341;
  internal: [32] u8; q: [32] u8; script: [AD_SCRIPT_MAX] u8; text: [90] u8; want: [32] u8;
  #assert taproot_tweak_vartime(unhex(c.internal, internal[..]), q[..]) >= 0;
  #assert sw_same(q[..], cast(string) unhex(c.output, want[..]));
  n := p2tr_script_vartime(internal[..], script[..]);
  #assert n == 34 && ad_hex_is(script[0..<n], c.script);
  m := address_encode_vartime(SW_MAINNET, script[0..<n], text[..]);
  #assert m == c.address.count && sw_same(text[0..<m], c.address);
  #assert ad_round_trip(SW_MAINNET, c.address, AD_P2TR, c.script, c.address);
}
#test "BIP-341's TapTweak hash" {
  internal: [32] u8; t: [32] u8;
  #assert bitcoin_tagged_checked(BH_TAPTWEAK, unhex(AD_BIP341.internal, internal[..]), t[..], false) == SHA_OK;
  #assert ad_hex_is(t[..], AD_BIP341_TWEAK);
}
// BIP-84's keys make its P2WPKH addresses. The same keys uncompressed
// refuse as P2WPKH, and as P2PKH give a different hash from the compressed
// form's.
#forall "BIP-84's P2WPKH addresses, and the uncompressed keys" i: 0..<3 #steps 400000000 {
  key: [33] u8; full: [65] u8; script: [AD_SCRIPT_MAX] u8; other: [AD_SCRIPT_MAX] u8; text: [90] u8;
  k := unhex(AD_BIP84[i].key, key[..]);
  n := p2wpkh_script_vartime(k, script[..]);
  m := address_encode_vartime(SW_MAINNET, script[0..<n], text[..]);
  #assert n == 22 && m == AD_BIP84[i].address.count && sw_same(text[0..<m], AD_BIP84[i].address);
  #assert se_recode(k, SE_UNCOMPRESSED, full[..]) == 65;
  #assert p2wpkh_script_vartime(full[..], other[..]) == AD_UNCOMPRESSED;
  #assert p2sh_p2wpkh_script_vartime(full[..], other[..]) == AD_UNCOMPRESSED;
  #assert p2pkh_script_vartime(full[..], other[..]) == 25 && p2pkh_script_vartime(k, script[..]) == 25;
  #assert !sw_same(other[3..<23], cast(string) script[3..<23]);
}
// BIP-49's worked P2SH-P2WPKH example, step by step.
#test "BIP-49's P2SH-P2WPKH address" {
  key: [33] u8; hash: [20] u8; redeem: [22] u8; script: [AD_SCRIPT_MAX] u8; text: [40] u8;
  k := unhex(AD_BIP49_KEY, key[..]);
  #assert hash160_checked(k, hash[..], false) == RMD_OK && ad_hex_is(hash[..], AD_BIP49_KEYHASH);
  #assert p2wpkh_script_vartime(k, redeem[..]) == 22 && ad_hex_is(redeem[..], AD_BIP49_REDEEM);
  #assert hash160_checked(redeem[..], hash[..], false) == RMD_OK && ad_hex_is(hash[..], AD_BIP49_HASH);
  n := p2sh_p2wpkh_script_vartime(k, script[..]);
  #assert n == 23 && ad_hex_is(script[2..<22], AD_BIP49_HASH);
  m := address_encode_vartime(SW_TESTNET, script[0..<n], text[..]);
  #assert m == AD_BIP49_ADDRESS.count && sw_same(text[0..<m], AD_BIP49_ADDRESS);
  #assert ad_round_trip(SW_TESTNET, AD_BIP49_ADDRESS, AD_P2SH, "a914336caa13e08b96080a32b5d818d59b4ab3b3674287", AD_BIP49_ADDRESS);
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/addresses.coax
coax/bin/coax mutate bitcoin/examples/checks/addresses.coax --root-only
```

`coax test` runs 174 checks, including the loaded files' own checks, in about 46 seconds of CPU time on 2026-11-02; the four Taproot tweaks at compile time take most of it. The checks file has no procedures of its own, so root-only mutation finds no sites in it; the example's procedures were not mutated.

### The library's own checks

`address.coax` has 11 checks of its own: the check helpers against
deliberate differences; scriptPubKeys from G in both forms, from BIP-49's
key and from keys that are not keys; P2SH and P2WSH at their size limits
and short buffers; the tweak for two BIP-86 keys (one with an odd y and one
with an even one), BIP-341's key, G and two keys that are not internal
keys; every script kind with each P2PKH and P2SH template byte changed;
addresses of every type both ways on all three networks; one decoding
refusal per code; the encoding refusals; and every witness version from 0
to 16 with every program length from 0 to 41 on each network, decoded
back and refused on the next network.

```shell
coax/bin/coax test coax/lib/bitcoin/address.coax
coax/bin/coax mutate coax/lib/bitcoin/address.coax --root-only
```

On 2026-11-02, root-only mutation of `address.coax` kills 427 of its 432 mutants with no survivor. The other 5 are on lines marked `// mutant: equivalent`: the two `AD_BAD_TWEAK` returns, which no known key reaches; the backward scan in `ad_is_segwit`, where stopping at index 0 or 1 leaves no HRP either way; and the segwit refusal test in `address_decode_vartime`, where a decoded program is never 0 bytes, so `< 0` and `< 1` agree.

### A Python implementation, natively and in Wasm

[address.py](../../coax/tests/bitcoin/address.py) compiles `address.coax`
to C in checked and release builds and runs its ten exported procedures
against a Python implementation that shares no code with Coax. It uses
hashlib for SHA-256, RIPEMD-160 and the tagged hash, Python integers for
the curve (chapter 17's `points.py`), chapter 26's Base58 reference and
chapter 28's classifier, and BIP-350's reference `segwit_addr.py` for
Bech32 and Bech32m. Before the Coax code runs, the Python implementation
must give every published answer: each `key_io_valid.json` address both
ways, each `key_io_invalid.json` string refused, BIP-86's and BIP-341's
output keys and addresses, BIP-84's and BIP-49's addresses. The cases are:

- every published key, script and address above, and every string in
  BIP-350's and BIP-173's test-vector sections;
- 60 seeded keys in both forms and as x-only internal keys, 40 random x
  coordinates (about half have no point), and corrupted keys: wrong
  lengths, hybrid and wrong prefixes, x at least p, x with no point and
  single flipped bits;
- redeem and witness scripts of 0 to 10,001 bytes, around both limits;
- every witness version and program length, the scriptPubKeys built above
  with bits flipped, a byte removed or a byte added, and scripts with no
  address;
- every address produced, on all three networks and two that do not exist,
  and each with one seeded edit: a character replaced, inserted or
  removed, or its case flipped;
- output buffers one byte or character too small.

Every result, kind and output must match, and nothing may be written past
the output. It then builds the same C for wasm32 and runs every case again
in Node. On 2026-11-02, all 50,725 cases agree natively and in Wasm, in checked and release builds.

```shell
python3 coax/tests/bitcoin/address.py
```

## Nostr's own address reader

Nostr's NIP-A3 reader,
[nipa3.coax](../../coax/lib/nostr/nipa3.coax), decodes the Bitcoin
addresses in a payment-target event with code of its own: Bech32 and
Bech32m on top of `bech32-core.coax`, and Base58Check for versions `0x00`
and `0x05`. It answers a narrower question than this chapter, whether a
client may offer the address as a way to pay: mainnet only, P2PKH, P2SH,
P2WPKH, P2WSH and P2TR, and future witness versions refused. This chapter
leaves that file and its behavior as they are; moving it onto
`address_decode_vartime` is a separate change for the Nostr book.

## What this chapter leaves out

- Script trees. A Taproot output with scripts commits to their Merkle root
  in the tweak; that is the Taproot part of the [plan](PLAN.md).
- Spending. Which signatures and witnesses unlock each type is the
  transaction and script chapters' subject.
- Output descriptors, which name a scriptPubKey with its keys and
  derivation paths: [chapter 30](PLAN.md).
- Pay-to-anchor (`OP_1 <4e73>`), a version 1 program of 2 bytes that
  Bitcoin Core treats as a standard anchor output. Here it is an
  `AD_WITNESS` output like any other unknown program.
- BIP-21 `bitcoin:` URIs, silent payments (BIP-352) and the other address
  formats built on top of these.

## Summary

- P2PKH and P2SH commit to a HASH160 and use Base58Check; P2WPKH, P2WSH
  and P2TR are witness programs and use Bech32 (version 0) or Bech32m
  (version 1).
- One key gives two P2PKH addresses, one per form. Segwit takes only the
  compressed form, a policy rule that the builders enforce.
- P2SH-P2WPKH wraps a P2WPKH scriptPubKey as a P2SH redeem script, so a
  Base58-only wallet can pay a segwit key.
- A P2TR output key is Q = P + H_TapTweak(P)·G for a key with no script
  tree, and the scriptPubKey keeps only Q's x.
- Witness versions 2 to 16 decode and encode for future output types.
- Every address in Bitcoin Core's key I/O tests, every BIP-173 and BIP-350
  segwit vector, and BIP-86's, BIP-84's and BIP-49's vectors map to their
  scriptPubKeys and back, and the code agrees with a Python implementation
  natively and in Wasm.

## Dependencies

- [6. RIPEMD-160 and HASH160](ch006-ripemd-160-and-hash160.md)
- [17. Point encodings: compressed, uncompressed and x-only](ch017-point-encodings-compressed-uncompressed-and-x-only.md)
- [26. Base58Check](ch026-base58check.md)
- [28. Bech32m](ch028-bech32m.md)

## Sources

- [BIP-13](https://github.com/bitcoin/bips/blob/master/bip-0013.mediawiki)
  and [BIP-16](https://github.com/bitcoin/bips/blob/master/bip-0016.mediawiki),
  for P2SH addresses and the redeem script.
- [BIP-141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
  and [BIP-143](https://github.com/bitcoin/bips/blob/master/bip-0143.mediawiki),
  for witness programs, P2WPKH, P2WSH and the compressed-key policy.
- [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki),
  for the Taproot tweak; its `wallet-test-vectors.json` is pinned in
  [coax/tests/vectors/upstream/bip341/](../../coax/tests/vectors/upstream/bip341/wallet-test-vectors.json).
- [BIP-86](https://github.com/bitcoin/bips/blob/master/bip-0086.mediawiki),
  pinned in
  [coax/tests/vectors/upstream/bip86/](../../coax/tests/vectors/upstream/bip86/bip-0086.mediawiki).
- [BIP-84](https://github.com/bitcoin/bips/blob/master/bip-0084.mediawiki)
  and [BIP-49](https://github.com/bitcoin/bips/blob/master/bip-0049.mediawiki),
  pinned in
  [coax/tests/vectors/upstream/bip84/](../../coax/tests/vectors/upstream/bip84/bip-0084.mediawiki)
  and
  [coax/tests/vectors/upstream/bip49/](../../coax/tests/vectors/upstream/bip49/bip-0049.mediawiki).
- [BIP-173](https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki)
  and [BIP-350](https://github.com/bitcoin/bips/blob/master/bip-0350.mediawiki),
  pinned for chapters 27 and 28.
- Bitcoin Core `key_io_valid.json` and `key_io_invalid.json`, pinned for
  chapter 26 in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/key_io_valid.json).
