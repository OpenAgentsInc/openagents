# 21. Strict DER

Script carries every pre-Taproot signature as DER followed by one sighash
type byte. Until 2015, Bitcoin let OpenSSL decide which encodings were
acceptable, and OpenSSL accepted many that DER forbids, differently from
one version to the next. BIP-66 made strict DER a consensus rule. This
chapter builds Bitcoin Core's `IsValidSignatureEncoding` rule by rule, with
a distinct refusal code for each rule, adds the low-S and sighash-type
policy checks beside it, and runs every DER case of Bitcoin Core's
`script_tests.json` to its expected result.

## What this chapter teaches

- The layout of a script signature, and the 9- and 73-byte bounds on it.
- Each of the fourteen rules of `IsValidSignatureEncoding`, in Bitcoin
  Core's order.
- Why lax DER parsing was a consensus risk, and how BIP-66 was deployed.
- How the encoder writes minimal integers, with `0x00` in front of a high
  bit, so that its output always passes.
- Low-S (`IsLowDERSignature`) and the sighash type
  (`IsDefinedHashtypeSignature`): policy rules that sit beside the
  consensus rule.
- How `CHECKSIG` and `CHECKMULTISIG` apply the checks under script flags,
  and the lax parser that still reads every signature for verification.

## Where script reads a signature

A spender of a P2PKH, P2WPKH or multisig output pushes signatures in the
scriptSig or witness. `OP_CHECKSIG`, `OP_CHECKMULTISIG` and their `VERIFY`
forms (chapter 19 lists the outputs) check each signature's encoding, then
verify it. A script signature is:

```text
30 len  02 rlen r-bytes  02 slen s-bytes  sighash
```

The DER part is chapter 19's `SEQUENCE` of two `INTEGER`s. The last byte is
the sighash type, which chapters 37 and 38 use to build the hash the
signature signs: `SIGHASH_ALL` (1), `SIGHASH_NONE` (2) or `SIGHASH_SINGLE`
(3), optionally with `SIGHASH_ANYONECANPAY` (`0x80`). It is outside the
DER, so the sequence length is the size minus 3: the tag, the length byte
and the sighash byte.

The smallest DER integer has one byte and the largest useful one has 33,
so a script signature has 9 to 73 bytes. `30 06 02 01 01 02 01 01 01`
(`r = s = 1`, `SIGHASH_ALL`) has 9; two 33-byte integers give 73.

Bitcoin Core applies the encoding checks under three script verification
flags:

| Flag | Rule | Status |
| --- | --- | --- |
| `SCRIPT_VERIFY_DERSIG` | BIP-66: `IsValidSignatureEncoding` | Consensus from block 363,725 |
| `SCRIPT_VERIFY_LOW_S` | BIP-66, then `s ≤ (n-1)/2` | Policy |
| `SCRIPT_VERIFY_STRICTENC` | BIP-66, then a defined sighash type (and SEC 1 key shapes) | Policy |

Any of the three applies BIP-66. A policy rule decides what a node relays
and mines, not what a block may contain: a high-S signature does not relay,
but a block that contains one is valid.

## Why lax DER was a consensus risk

Bitcoin Core used OpenSSL to parse and verify signatures until release 0.12
in 2016. OpenSSL's parser read BER, the looser encoding DER restricts, and
accepted more than BER allows: negative integers, needless leading zeros,
lengths in long form, bytes after the signature. None of that was written
down as a Bitcoin rule. The set of valid signatures, and so the set of valid
blocks, was whatever the linked OpenSSL accepted.

In January 2015, OpenSSL 1.0.0p and 1.0.1k fixed CVE-2014-8275 by refusing
some non-DER encodings. A node linked against them refused signatures that
other nodes accepted, and so could reject the chain that the rest of the
network followed. Bitcoin Core's own release binaries were not affected,
but builds against a system OpenSSL were. Pieter Wuille later disclosed that
OpenSSL had also read some encodings differently on different platforms,
so before BIP-66 a crafted signature could have split nodes that ran the
same Bitcoin Core release.

BIP-66, by Pieter Wuille, removed the dependency instead of tracking
OpenSSL: every signature that `CHECKSIG` or `CHECKMULTISIG` checks must be
strict DER, defined by a short function that any implementation can copy.
It was deployed by version-3 blocks under BIP-34's supermajority rule and
enforced from block 363,725 on 4 July 2015. The same day, a miner produced
a block that the new rules refused, and pools that mined on blocks without
validating them extended it by several blocks before the valid chain
overtook it. The rule itself held. Bitcoin Core now hardcodes the height
(`BIP66Height = 363725` in `chainparams.cpp`).

Blocks before that height still contain lax signatures, so script still
needs a lax reader. Core's `ecdsa_signature_parse_der_lax`, in `pubkey.cpp`,
accepts every violation present in the chain before BIP-66 and nothing
else that matters: after BIP-66, `IsValidSignatureEncoding` has already
refused everything else. This chapter builds both.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [der.coax](../../coax/lib/bitcoin/der.coax) | `der_check_vartime(sig)` | `IsValidSignatureEncoding`: `DER_OK` or the code of the first rule that refuses |
| | `der_low_s_vartime(sig)` | the low-S test of `IsLowDERSignature` |
| | `der_hashtype_defined_vartime(sig)` | `IsDefinedHashtypeSignature` |
| | `der_signature_check_vartime(sig, flags)` | `CheckSignatureEncoding`: `SIG_OK`, `SIG_DER`, `SIG_HIGH_S` or `SIG_HASHTYPE` |
| | `der_checksig_vartime(hash, public_key, sig)` | `CHECKSIG`'s verification: lax read, either half of `s` |
| | `der_read_lax_vartime(der, out)` | Core's lax DER parser |
| [ecdsa_sign.coax](../../coax/lib/crypto/ecdsa_sign.coax) | `ecdsa_der(sig, out)` | chapter 20's DER encoder |
| [ecdsa_verify.coax](../../coax/lib/crypto/ecdsa_verify.coax) | `ecdsa_der_read_vartime(der, out)` | chapter 19's strict reader, without the sighash byte |

Every input is public, so every procedure runs in variable time
(chapter 3).

## The rules

`der_check_vartime` returns one of fifteen codes:

<!-- source: coax/lib/bitcoin/der.coax codes -->
```jai
// Refusal codes of der_check_vartime, one per rule of Bitcoin Core's
// IsValidSignatureEncoding, in the order Core tests them.
DER_OK :: 0;
DER_TOO_SHORT :: 1;       // fewer than 9 bytes
DER_TOO_LONG :: 2;        // more than 73 bytes
DER_NOT_SEQUENCE :: 3;    // the first byte is not 0x30
DER_BAD_LENGTH :: 4;      // the sequence length is not the size minus 3
DER_R_TOO_LONG :: 5;      // R's length leaves no room for S's length byte
DER_LENGTHS :: 6;         // R's and S's lengths plus 7 are not the size
DER_R_NOT_INTEGER :: 7;   // R's tag is not 0x02
DER_R_EMPTY :: 8;         // R has length 0
DER_R_NEGATIVE :: 9;      // R's first byte has its top bit set
DER_R_PADDED :: 10;       // R starts with a 0x00 that no top bit needs
DER_S_NOT_INTEGER :: 11;  // the same four rules for S
DER_S_EMPTY :: 12;
DER_S_NEGATIVE :: 13;
DER_S_PADDED :: 14;

// Script verification flags and the errors der_signature_check_vartime
// returns, named as Bitcoin Core names them.
SCRIPT_DERSIG :: 1;       // BIP-66, consensus since block 363,725
SCRIPT_LOW_S :: 2;        // policy
SCRIPT_STRICTENC :: 4;    // policy
SIG_OK :: 0;
SIG_DER :: 1;
SIG_HIGH_S :: 2;
SIG_HASHTYPE :: 3;
```

The codes follow Core's order, which is not the order of the bytes. Core
first checks the size, the sequence tag and the sequence length. It then
reads both lengths and requires that they account for every byte. Only
then does it look at the tags and values, R before S:

<!-- source: coax/lib/bitcoin/der.coax check -->
```jai
// The four rules for one INTEGER: its tag at sig[at], its length at
// sig[at+1], its value from sig[at+2]. Returns DER_OK, or first plus the
// rule's place: first is DER_R_NOT_INTEGER for R and DER_S_NOT_INTEGER for S.
// The caller has checked that the value lies inside sig.
dr_integer_vartime :: (sig: [] u8, at: int, first: int) -> int {
  length := cast(int) sig[at+1]; value := at + 2;
  if sig[at] != 0x02 { return first; }                                  // not an INTEGER
  if length == 0 { return first + 1; }                                  // empty
  if (sig[value] & 0x80) != 0 { return first + 2; }                     // negative
  if length > 1 && sig[value] == 0 && (sig[value+1] & 0x80) == 0 { return first + 3; }   // padded
  return DER_OK;
}
// Bitcoin Core's IsValidSignatureEncoding on a script signature: DER, then
// one sighash type byte. Returns DER_OK or the code of the first rule that
// refuses, testing the rules in Core's order. It does not read r or s as
// numbers: a value of 0 or at least n is well encoded.
der_check_vartime :: (sig: [] u8) -> int #export_c {
  size := sig.count;
  if size < 9 { return DER_TOO_SHORT; }
  if size > 73 { return DER_TOO_LONG; }
  if sig[0] != 0x30 { return DER_NOT_SEQUENCE; }
  if cast(int) sig[1] != size - 3 { return DER_BAD_LENGTH; }            // the sighash byte is outside
  length_r := cast(int) sig[3];
  if 5 + length_r >= size { return DER_R_TOO_LONG; }
  length_s := cast(int) sig[5+length_r];
  if length_r + length_s + 7 != size { return DER_LENGTHS; }
  r := dr_integer_vartime(sig, 2, DER_R_NOT_INTEGER);
  if r != DER_OK { return r; }
  return dr_integer_vartime(sig, length_r + 4, DER_S_NOT_INTEGER);
}
```

Each rule maps to one test in `interpreter.cpp`:

| Code | Core's test | Why |
| --- | --- | --- |
| 1, 2 | `sig.size() < 9`, `> 73` | the size bounds |
| 3 | `sig[0] != 0x30` | a `SEQUENCE` |
| 4 | `sig[1] != sig.size() - 3` | a short-form length that covers everything but the sighash byte |
| 5 | `5 + lenR >= sig.size()` | S's length byte lies inside |
| 6 | `lenR + lenS + 7 != sig.size()` | no byte is missing or extra |
| 7, 11 | `sig[2] != 0x02`, `sig[lenR + 4] != 0x02` | `INTEGER` tags |
| 8, 12 | `lenR == 0`, `lenS == 0` | DER has no empty integer |
| 9, 13 | top bit of the first byte | a DER integer is signed; `r` and `s` are not negative |
| 10, 14 | `0x00` followed by a byte below `0x80` | the shortest encoding |

The checks that come first make the later reads safe. After rule 6, both
values lie inside the signature, so the value tests never read past it.

Strict DER fixes the encoding, not the numbers. `r = 0`, `s = 0` and values
of `n` or more are well encoded, and `30 06 02 01 00 02 01 00 01` passes.
The verifier refuses them later (chapter 19). An `INTEGER` may also be
longer than 33 bytes as long as the total stays within 73; such a value is
at least 2^256 and no verifier accepts it.

### Worked example: a script signature, field by field

The `P2PK` row of `script_tests.json` signs with the key `G`:

```text
30 44                      SEQUENCE of 68 bytes: 71 - 3
02 20 0a5c6163...7f15ffa1  r, 32 bytes, first byte 0x0a: no padding
02 20 1fd9b4ea...966b7420  s, 32 bytes, first byte 0x1f
01                         SIGHASH_ALL
```

1. Size 71: between 9 and 73.
2. `sig[0] = 0x30`; `sig[1] = 0x44 = 68 = 71 - 3`.
3. `lenR = sig[3] = 32`; `5 + 32 = 37 < 71`.
4. `lenS = sig[37] = 32`; `32 + 32 + 7 = 71`.
5. `sig[2] = 0x02`, `lenR ≠ 0`, `sig[4] = 0x0a < 0x80`, and `sig[4] ≠ 0`.
6. `sig[36] = 0x02`, `lenS ≠ 0`, `sig[38] = 0x1f < 0x80`, and `sig[38] ≠ 0`.

Every rule passes, and `der_check_vartime` returns `DER_OK`. Without the
sighash byte, chapter 19's reader gives the same `r` and `s`, and chapter
20's encoder writes the same 70 bytes back.

## The encoder

Chapter 20's `ecdsa_der` writes the DER part from a compact `r || s`. Each
integer drops its leading zero bytes and gets one `0x00` in front when its
first remaining byte has the top bit set, which is the only encoding rules
9, 10, 13 and 14 allow:

<!-- source: coax/lib/crypto/ecdsa_sign.coax der -->
```jai
// The DER encoding of a compact r || s, into out (72 bytes are always
// enough); its length, or -1 when r or s is not in [1, n-1] or out is short.
ecdsa_der :: (sig:[]u8,out:[]u8)->int #export_c {
  if sig.count!=64 || out.count<72 {return -1;}
  r:=sc_load(sig[0..<32]);s:=sc_load(sig[32..<64]);
  if sc_is_zero(r.value) || sc_is_zero(s.value) {return -1;}    // sc_load zeroes a value at least n
  n:=ec_der_int(sig[32..<64],out,ec_der_int(sig[0..<32],out,2));
  out[0]=0x30;out[1]=cast(u8)(n-2);
  return n;
}
```

| `r` | DER `INTEGER` | Why |
| --- | --- | --- |
| `0x01` | `02 01 01` | one byte |
| `0x7f` | `02 01 7f` | top bit clear |
| `0x80` | `02 02 00 80` | top bit set: `0x00` in front |
| `0x00ff` | `02 02 00 ff` | the zero dropped, then one put back for the top bit |

With `s = 1`, these give signatures of 8 and 9 bytes. Two 32-byte values
give 70 bytes when both top bits are clear and 72 when both are set, so
with the sighash byte the encoder's output always has 9 to 73 bytes. The
example's checks encode every pair of lengths from 1 to 32 with each top
bit set and clear, and every result passes `der_check_vartime` and reads
back.

A signer that keeps `s` low (chapter 20) never needs the pad on `s`: `s ≤
(n-1)/2 < 2^255`, so its top bit is clear. The pad on `r` is needed in about
half of all signatures, which is why most script signatures have 71 or 72
bytes.

## Low-S and the sighash type

Two policy checks follow BIP-66 in `CheckSignatureEncoding`:

<!-- source: coax/lib/bitcoin/der.coax policy -->
```jai
// IsLowDERSignature after the encoding check: s, read by the lax parser
// without the sighash byte, is at most (n-1)/2. A value of s at least n
// reads as zero (the whole signature does), so it counts as low; the
// verifier refuses it.
der_low_s_vartime :: (sig: [] u8) -> bool #export_c {
  compact: [64] u8;
  if sig.count == 0 || !der_read_lax_vartime(sig[0..<sig.count-1], compact[..]) { return false; }
  return !ev_high_s_vartime(sc_load(compact[32..<64]).value);
}
// IsDefinedHashtypeSignature: the last byte, without ANYONECANPAY (0x80),
// is SIGHASH_ALL (1), SIGHASH_NONE (2) or SIGHASH_SINGLE (3).
der_hashtype_defined_vartime :: (sig: [] u8) -> bool #export_c {
  if sig.count == 0 { return false; }
  base := cast(int) sig[sig.count-1] & 0x7f;
  return base >= 1 && base <= 3;
}
```

`IsLowDERSignature` reads `s` with the lax parser, as Core's
`CPubKey::CheckLowS` does, and refuses it when `secp256k1_ecdsa_signature_normalize`
would change it: when `s > (n-1)/2`. Chapter 19 explains why `(r, n - s)`
is the other valid signature for the same key and hash, and why the low
half was chosen. One edge follows from the lax parser: a well-encoded `s`
of `n` or more makes the whole signature read as zero, which counts as low.
The verifier refuses it anyway.

The sighash test masks off `ANYONECANPAY` and accepts only 1, 2 and 3.
Consensus accepts every byte: legacy signature hashing treats the
undefined types as `SIGHASH_ALL` but still hashes the byte itself
(chapter 37).

| Signature | `DERSIG` | `LOW_S` | `STRICTENC` |
| --- | --- | --- | --- |
| strict DER, low `s`, sighash 1 | OK | OK | OK |
| strict DER, high `s` | OK | `SIG_HIGH_S` | OK |
| strict DER, sighash 4 | OK | OK | `SIG_HASHTYPE` |
| not strict DER | `SIG_DER` | `SIG_DER` | `SIG_DER` |

## The script check

`CHECKSIG` and `CHECKMULTISIG` call one function for each signature before
they verify it:

<!-- source: coax/lib/bitcoin/der.coax script -->
```jai
// CheckSignatureEncoding: what CHECKSIG and CHECKMULTISIG check before they
// verify a signature, under the script flags. An empty signature passes:
// script uses it to fail a check on purpose. Any of the three flags applies
// BIP-66; LOW_S adds the low-S rule and STRICTENC the sighash type.
der_signature_check_vartime :: (sig: [] u8, flags: int) -> int #export_c {
  if sig.count == 0 { return SIG_OK; }
  if (flags & (SCRIPT_DERSIG | SCRIPT_LOW_S | SCRIPT_STRICTENC)) != 0 && der_check_vartime(sig) != DER_OK { return SIG_DER; }
  if (flags & SCRIPT_LOW_S) != 0 && !der_low_s_vartime(sig) { return SIG_HIGH_S; }
  if (flags & SCRIPT_STRICTENC) != 0 && !der_hashtype_defined_vartime(sig) { return SIG_HASHTYPE; }
  return SIG_OK;
}
// The verification CHECKSIG does after that check, for a hash the caller
// computed for the signature's sighash type: drop the sighash byte, read
// the rest with the lax parser, and verify with either half of s, as Core
// does after secp256k1_ecdsa_signature_normalize. The key is SEC 1
// (chapter 17); hybrid keys (0x06, 0x07) refuse here and are chapter 36's.
der_checksig_vartime :: (hash: [] u8, public_key: [] u8, sig: [] u8) -> bool #export_c {
  compact: [64] u8;
  if sig.count == 0 || !der_read_lax_vartime(sig[0..<sig.count-1], compact[..]) { return false; }
  return ecdsa_verify_vartime(hash, public_key, compact[..], false);
}
```

An empty signature passes every flag. Script uses it as the cheap way to
fail a check on purpose, as in `0 <key> CHECKSIG NOT`. `CHECKMULTISIG`
checks signatures one at a time, from the top of the stack, and stops when
too few keys remain. A badly encoded signature that it never reaches does
not fail the script, and `script_tests.json` has rows for that (BIP-66's
examples 11 and 12).

After the check, `der_checksig_vartime` does what Core's
`CheckECDSASignature` does: it drops the sighash byte, reads the rest with
the lax parser, and verifies with chapter 19's verifier with the low-S rule
off, because Core normalizes `s` before it verifies. Without the `DERSIG`
flag, only the lax parser stands between the bytes and the verifier, which
is how blocks before 363,725 are checked.

## The lax parser

<!-- source: coax/lib/bitcoin/der.coax lax -->
```jai
// One INTEGER length at der[at]: below 0x80 it is the length; 0x80 + k is
// followed by k bytes, big-endian. Core skips their leading zeros and
// refuses when 4 or more bytes remain, that is, when the length is at
// least 2^24, which is the test here. ok is false where Core returns 0.
dl_length_vartime :: (der: [] u8, at: int) -> DlLength {
  if at == der.count { return DlLength.{}; }
  first := cast(int) der[at]; pos := at + 1;
  if first < 0x80 { return DlLength.{value=first,next=pos,ok=true}; }
  count := first - 0x80;
  if count > der.count - pos { return DlLength.{}; }
  value := 0;
  for i: 0..<count {
    value = value * 256 + cast(int) der[pos+i];
    if value >= 0x1000000 { return DlLength.{}; }
  }
  return DlLength.{value=value,next=pos+count,ok=true};
}
// The value der[start..<start+length], right-aligned in out (32 bytes);
// false when a nonzero byte lies to the left of out. The bytes are read
// from the left, so a refusal comes before any write.
dl_copy_vartime :: (der: [] u8, start: int, length: int, out: [] u8) -> bool {
  for i: 0..<length {
    place := length - 1 - i;                                            // bytes to its right
    if place >= out.count {
      if der[start+i] != 0 { return false; }
    } else {
      out[out.count-1-place] = der[start+i];
    }
  }
  return true;
}
// Bitcoin Core's ecdsa_signature_parse_der_lax (pubkey.cpp), the parser
// script uses before it verifies: 0x30, a sequence length it skips, then
// two INTEGERs with short or long lengths and anything after them. It
// refuses only a structure it cannot follow. A value of more than 32 bytes
// after its zeros, or at least n, makes the whole r || s zero, which no
// verifier accepts. out (64 bytes) is written only when the result is true.
der_read_lax_vartime :: (der: [] u8, out: [] u8) -> bool #export_c {
  if out.count != 64 || der.count == 0 || der[0] != 0x30 || der.count == 1 { return false; }
  pos := 2;
  if (der[1] & 0x80) != 0 { pos += cast(int) der[1] - 0x80; }          // long form: skip its bytes
  if pos >= der.count || der[pos] != 0x02 { return false; }
  r := dl_length_vartime(der, pos + 1);
  if !r.ok { return false; }
  pos = r.next + r.value;
  if pos >= der.count || der[pos] != 0x02 { return false; }
  s := dl_length_vartime(der, pos + 1);
  if !s.ok || s.value > der.count - s.next { return false; }
  compact: [64] u8;
  r_fits := dl_copy_vartime(der, r.next, r.value, compact[0..<32]);
  s_fits := dl_copy_vartime(der, s.next, s.value, compact[32..<64]);
  if !r_fits || !s_fits || !sc_load(compact[0..<32]).ok || !sc_load(compact[32..<64]).ok {
    for i: 0..<64 { compact[i] = 0; }
  }
  for i: 0..<64 { out[i] = compact[i]; }
  return true;
}
```

Compared with the rules above, the lax parser accepts:

- any sequence length, in short or long form, without checking it;
- `INTEGER` lengths in long form, with leading zero bytes;
- empty integers, read as zero;
- integers with any number of leading zeros, and "negative" ones, read as
  unsigned;
- any bytes after `s`.

It refuses only what it cannot follow: a missing tag, or a length that
runs past the end. A value longer than 32 bytes after its zeros, or one of
`n` or more, does not refuse. It makes the whole result zero, a signature
that parses and never verifies, as `secp256k1_ecdsa_signature_parse_compact`
does.

Core checks that a skipped sequence length fits, and that R's value fits,
before it looks for the next tag. The code here checks once, that the
position of the next tag is inside the input. The two refuse the same
inputs.

One constant matters across platforms: a long-form length may have at
most 3 bytes after its zeros. libsecp256k1's `contrib/lax_der_parsing.c`,
from which Core copied the parser, allows `sizeof(size_t) - 1` bytes, which
is 7 on a 64-bit machine and 3 on a 32-bit one. Core fixes it at 3 so that
every build reads the same signatures.

At most 3 bytes after the zeros means a length below 2^24, and that is what
`dl_length_vartime` tests, with no loop to skip the zeros.
`dl_copy_vartime` works the same way: instead of skipping a value's zeros,
it checks that every byte left of the 32 it keeps is zero. A length of 2^24
or more needs an input of at least 16 MiB to fit, so Core and the 64-bit
contrib parser differ only on inputs far beyond a script element's 520
bytes. The library's checks build one such input to show that the rule is
there.

## The example

The example is [bitcoin/examples/strict-der.coax](../examples/strict-der.coax).
It reads a script signature field by field, encodes small and large
values, shows one signature for each refusal code, reads six encodings
that only the lax parser accepts, applies the low-S and sighash-type
policy, runs all 89 pinned rows of `script_tests.json`, and runs the
bitcoinjs/bip66 fixtures.

Its table,
[coax/tests/bitcoin/der_vectors.coax](../../coax/tests/bitcoin/der_vectors.coax),
is written by
[make_der_vectors.py](../../coax/tests/bitcoin/make_der_vectors.py) from
the pinned `script_tests.json` and bip66 `fixtures.json`, and refuses a
file whose SHA-256 differs from its pin. It takes every row whose
scriptSig is only pushes and whose scriptPubKey is pushes followed by
`CHECKSIG` or `CHECKMULTISIG`, optionally with `NOT`. Rows that share their
scripts form a group, and it keeps each group in which some row sets
`DERSIG` or `LOW_S`, or expects `SIG_DER`, `SIG_HIGH_S` or `SIG_HASHTYPE`.
That gives 89 rows: the BIP-66 cases and the same scripts under other
flags. The table copies each row's stack, flags and expected result. It
computes one thing, the legacy signature hash of Core's test transaction
for each signature, which chapter 37 builds in Coax.

`run_row` is `CHECKSIG` and `CHECKMULTISIG` from `interpreter.cpp`, just
enough to run these rows: the encoding check, `STRICTENC`'s key shapes,
verification, `NULLFAIL` and `NULLDUMMY`. The full interpreter is chapter
36's.

<!-- source: bitcoin/examples/strict-der.coax example -->
```jai
#load "../../coax/lib/bitcoin/der.coax";
#load "../../coax/lib/crypto/ecdsa_sign.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/der_vectors.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [200] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode hex into out and return the filled part. Every literal here and
// in the vector table is valid hex, so a decoding error stops the program.
decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
bytes_same :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}
// The name of each der_check_vartime code.
DER_NAMES :: [15] string.["ok", "fewer than 9 bytes", "more than 73 bytes", "not a SEQUENCE",
  "sequence length is not size - 3", "R length leaves no room for S", "lengths do not add up",
  "R not an INTEGER", "R empty", "R negative", "R padded", "S not an INTEGER", "S empty",
  "S negative", "S padded"];
SIG_NAMES :: [4] string.["OK", "SIG_DER", "SIG_HIGH_S", "SIG_HASHTYPE"];

// ---- the comma-separated stack items of a script_tests.json row
// Item k lives at data[80*k..] with lengths[k] bytes. Returns the count.
ITEM :: 80;
parse_items :: (text: string, data: [] u8, lengths: [] int) -> int {
  n := 0; at := 0; i := 0;
  while true {
    if i == text.count || text[i] == ',' {
      lengths[n] = at; n += 1; at = 0;
      if i == text.count { return n; }
      i += 1; continue;
    }
    pair: [1] u8;
    data[ITEM*n+at] = decode(cast(string) text[i..<i+2], pair[..])[0];
    at += 1; i += 2;
  }
  return n;
}
// A CScriptNum of up to 4 bytes: little-endian, sign in the top bit.
script_number :: (b: [] u8) -> int {
  value := 0;
  for i: 0..<b.count { value += cast(int) b[i] << (8 * i); }
  if b.count > 0 && (b[b.count-1] & 0x80) != 0 { return -(value - (0x80 << (8 * (b.count - 1)))); }
  return value;
}
// IsCompressedOrUncompressedPubKey: STRICTENC's key shape.
key_shape :: (key: [] u8) -> bool {
  return (key.count == 33 && (key[0] == 2 || key[0] == 3)) || (key.count == 65 && key[0] == 4);
}
NULLFAIL :: 8;
NULLDUMMY :: 16;

// ---- CHECKSIG and CHECKMULTISIG, enough to run the rows
// Script's two ECDSA opcodes as Bitcoin Core's interpreter.cpp runs them,
// on a stack of pushes, with the flags this chapter's rows use. The hash
// of each signature comes from the table (chapter 37 computes it). The
// result is "OK", "EVAL_FALSE" or the script error.
run_row :: (row: DvRow) -> string {
  data: [5120] u8; lengths: [64] int; hdata: [5120] u8; hlengths: [64] int;
  n := parse_items(row.stack, data[..], lengths[..]);
  #assert parse_items(row.hashes, hdata[..], hlengths[..]) == n;
  ok := false; size := n;
  if !row.multisig {
    s := n - 2; k := n - 1;
    sig := data[ITEM*s..<ITEM*s+lengths[s]]; key := data[ITEM*k..<ITEM*k+lengths[k]];
    error := der_signature_check_vartime(sig, row.flags);
    if error != SIG_OK { return SIG_NAMES[error]; }
    if (row.flags & SCRIPT_STRICTENC) != 0 && !key_shape(key) { return "PUBKEYTYPE"; }
    ok = der_checksig_vartime(hdata[ITEM*s..<ITEM*s+32], key, sig);
    if !ok && (row.flags & NULLFAIL) != 0 && sig.count > 0 { return "NULLFAIL"; }
    size = n - 2;
  } else {
    // Positions count from the top of the stack: 1 is the top item.
    i := 1; keys := script_number(data[ITEM*(n-i)..<ITEM*(n-i)+lengths[n-i]]);
    ikey := i + 1; ikey2 := keys + 2; i += keys + 1;
    sigs := script_number(data[ITEM*(n-i)..<ITEM*(n-i)+lengths[n-i]]);
    isig := i + 1; i += sigs + 1;
    #assert sigs >= 0 && sigs <= keys && n >= i;
    ok = true;
    while ok && sigs > 0 {
      s := n - isig; k := n - ikey;
      sig := data[ITEM*s..<ITEM*s+lengths[s]]; key := data[ITEM*k..<ITEM*k+lengths[k]];
      error := der_signature_check_vartime(sig, row.flags);
      if error != SIG_OK { return SIG_NAMES[error]; }
      if (row.flags & SCRIPT_STRICTENC) != 0 && !key_shape(key) { return "PUBKEYTYPE"; }
      if der_checksig_vartime(hdata[ITEM*s..<ITEM*s+32], key, sig) { isig += 1; sigs -= 1; }
      ikey += 1; keys -= 1;
      if sigs > keys { ok = false; }                       // too many have failed
    }
    // NULLFAIL: after a failure, every signature must be empty.
    while i > 1 {
      i -= 1;
      if !ok && (row.flags & NULLFAIL) != 0 && ikey2 == 0 && lengths[size-1] > 0 { return "NULLFAIL"; }
      if ikey2 > 0 { ikey2 -= 1; }
      size -= 1;
    }
    if (row.flags & NULLDUMMY) != 0 && lengths[size-1] > 0 { return "SIG_NULLDUMMY"; }   // the extra item
    size -= 1;
  }
  if row.negate { ok = !ok; }
  return ifx ok then "OK" else "EVAL_FALSE";
}

// A script signature from script_tests.json ("P2PK"), and the same
// signing key's high-S signature ("P2PK with high S").
SIG_P2PK :: "304402200a5c6163f07b8d3b013c4d1d6dba25e780b39658d79ba37af7057a3b7f15ffa102201fd9b4eaa9943f734928b99a83592c2e7bf342ea2680f6a2bb705167966b742001";
SIG_HIGH :: "304502203e4516da7253cf068effec6b95c41221c0cf3a8e6ccb8cbf1725b562e9afde2c022100ab1e3da73d67e32045a20e0b999e049978ea8d6ee5480d485fcf2ce0d03b2ef001";
// One signature per refusal code, in order, each from a well-formed one.
REFUSALS :: [14] string.["3006020101020101", DR_74, "310602010102010101", "300702010102010101",
  "300602040102010101", "300602010102020101", "300603010102010101", "300602000202010101",
  "300602018102010101", "30070202000102010101", "300602010103010101", "300602020101020001",
  "300602010102018101", "30070201010202000101"];
// Encodings the lax parser reads and BIP-66 refuses.
LAX :: [6] string.["30440220d7a0417c3f6d1a15094d1cf2a3378ca0503eb8a57630953a9e2987e21ddd0a6502207a6266d686c99090920249991d3d42065b6d43eb70187b219c0db82e4f94d1a2",
  "304402200060558477337b9022e70534f1fea71a318caf836812465a2509931c5e7c4987022078ec32bd50ac9e03a349ba953dfd9fe1c8d2dd8bdb1d38ddca844d3d5c78c118",
  "3000020101020102", "3006020101020102ffff", "308106020101020102", "300802810101020102"];
VALUES :: [4] string.["01", "7f", "80", "00ff"];
EXPECTED :: [7] string.["OK", "EVAL_FALSE", "SIG_DER", "SIG_HIGH_S", "SIG_HASHTYPE", "NULLFAIL", "SIG_NULLDUMMY"];

main :: () {
  // A script signature, field by field.
  print("a script signature from script_tests.json (P2PK)\n");
  b: [80] u8; sig := decode(SIG_P2PK, b[..]);
  length_r := cast(int) sig[3];
  print("size %: 0x30, length %, 0x02, length %, r, 0x02, length %, s, sighash %\n",
    sig.count, sig[1], length_r, sig[5+length_r], sig[sig.count-1]);
  show("r", sig[4..<4+length_r]);
  show("s", sig[6+length_r..<sig.count-1]);
  print("der_check_vartime: % (%)\n", der_check_vartime(sig), DER_NAMES[der_check_vartime(sig)]);
  compact: [64] u8;
  print("chapter 19's reader on the DER part: %\n", ecdsa_der_read_vartime(sig[0..<sig.count-1], compact[..]));
  out: [72] u8;
  print("chapter 20's encoder gives the same bytes back: %\n", bytes_same(out[0..<ecdsa_der(compact[..], out[..])], sig[0..<sig.count-1]));

  // The encoder: minimal integers, 0x00 in front of a high bit.
  print("encoding (r, s)\n");
  for v: 0..<4 {
    for i: 0..<64 { compact[i] = 0; }
    one: [2] u8; bytes := decode(VALUES[v], one[..]);
    for i: 0..<bytes.count { compact[32-bytes.count+i] = bytes[i]; }
    compact[63] = 1;
    n := ecdsa_der(compact[..], out[..]);
    text: [40] u8; written := hex_encode_checked(cast(string) out[0..<n], text[..], false).written;
    print("r = 0x%: % bytes, %\n", VALUES[v], n, cast(string) text[0..<written]);
  }
  for i: 0..<64 { compact[i] = 0xff; }
  compact[0] = 0x7f; compact[32] = 0x7f;                    // r and s below n with a top byte of 0x7f
  print("r, s of 32 bytes, top bit clear: % bytes; set: ", ecdsa_der(compact[..], out[..]));
  compact[0] = 0x80; compact[32] = 0x80;
  print("% bytes\n", ecdsa_der(compact[..], out[..]));

  // Each rule, and its code.
  print("refusals\n");
  for i: 0..<14 {
    code := der_check_vartime(decode(REFUSALS[i], b[..]));
    print("% bytes: code %, %\n", REFUSALS[i].count / 2, code, DER_NAMES[code]);
  }

  // What lax parsing accepted.
  print("read by the lax parser, refused by BIP-66\n");
  for i: 0..<6 {
    der := decode(LAX[i], b[..]);
    b[der.count] = 1; with_type := b[0..<der.count+1];
    print("%... (% bytes): lax %, BIP-66 code % (%)\n", cast(string) LAX[i][0..<16], der.count,
      der_read_lax_vartime(der, compact[..]), der_check_vartime(with_type), DER_NAMES[der_check_vartime(with_type)]);
  }

  // Low-S beside the consensus rule.
  print("low-S\n");
  c: [80] u8; high := decode(SIG_HIGH, c[..]);
  print("high-S signature: DER code %, DERSIG %, LOW_S %\n", der_check_vartime(high),
    SIG_NAMES[der_signature_check_vartime(high, SCRIPT_DERSIG)], SIG_NAMES[der_signature_check_vartime(high, SCRIPT_LOW_S)]);
  #assert ecdsa_der_read_vartime(high[0..<high.count-1], compact[..]);
  low := sc_negate(sc_load(compact[32..<64]).value);
  #assert sc_store(low, compact[32..<64]);
  n := ecdsa_der(compact[..], out[..]); out[n] = high[high.count-1];
  print("with n - s: % bytes, LOW_S %\n", n + 1, SIG_NAMES[der_signature_check_vartime(out[0..<n+1], SCRIPT_LOW_S)]);
  sig = decode(SIG_P2PK, b[..]); sig[sig.count-1] = 0x04;
  print("sighash type 0x04: DERSIG %, STRICTENC %\n", SIG_NAMES[der_signature_check_vartime(sig, SCRIPT_DERSIG)],
    SIG_NAMES[der_signature_check_vartime(sig, SCRIPT_STRICTENC)]);

  // Bitcoin Core's script_tests.json rows.
  print("Bitcoin Core script_tests.json: % rows\n", DV_ROWS.count);
  agree := 0; count: [7] int; right: [7] int;
  for i: 0..<DV_ROWS.count {
    row := DV_ROWS[i]; got := run_row(row);
    for e: 0..<7 { if row.expected == EXPECTED[e] { count[e] += 1; if got == row.expected { right[e] += 1; } } }
    if got == row.expected { agree += 1; }
    else { print("row %: expected %, got %\n", row.row, row.expected, got); }
  }
  for e: 0..<7 { print("%: % of %\n", EXPECTED[e], right[e], count[e]); }
  print("% of % rows give the expected result\n", agree, DV_ROWS.count);

  // bitcoinjs/bip66's fixtures, with a sighash byte added.
  print("bip66 fixtures\n");
  good := 0;
  for i: 0..<DV_VALID.count {
    v := DV_VALID[i]; der := decode(v.der, b[..]); b[der.count] = 1;
    rv: [33] u8; sv: [33] u8;
    r := decode(v.r, rv[..]); s := decode(v.s, sv[..]);
    lr := cast(int) der[3];
    if der_check_vartime(b[0..<der.count+1]) == DER_OK && bytes_same(der[4..<4+lr], r) && bytes_same(der[6+lr..<der.count], s) { good += 1; }
  }
  print("valid: % of % accepted, r and s as listed\n", good, DV_VALID.count);
  for i: 0..<DV_INVALID.count {
    v := DV_INVALID[i]; der := decode(v.der, b[..]); b[der.count] = 1;
    code := der_check_vartime(b[0..<der.count+1]);
    print("\"%\": code % (%)\n", cast(string) v.reason[7..<v.reason.count], code, DER_NAMES[code]);
  }

  // Refusals of the script check.
  print("refusals\n");
  print("empty signature, all flags: %\n", SIG_NAMES[der_signature_check_vartime(b[0..<0], 7)]);
  print("8 bytes with DERSIG: %\n", SIG_NAMES[der_signature_check_vartime(decode("3006020101020101", b[..]), SCRIPT_DERSIG)]);
  print("8 bytes without flags: %\n", SIG_NAMES[der_signature_check_vartime(decode("3006020101020101", b[..]), 0)]);
  print("lax parser, 33 nonzero bytes of r: %, r = s = 0\n",
    der_read_lax_vartime(decode("30280221010000000000000000000000000000000000000000000000000000000000000000020101", b[..]), compact[..]) && compact[31] == 0 && compact[63] == 0);
}
```

```shell
coax/bin/coax run bitcoin/examples/strict-der.coax --run-steps 250000000
```

The output:

```text
a script signature from script_tests.json (P2PK)
size 71: 0x30, length 68, 0x02, length 32, r, 0x02, length 32, s, sighash 1
r: 0a5c6163f07b8d3b013c4d1d6dba25e780b39658d79ba37af7057a3b7f15ffa1
s: 1fd9b4eaa9943f734928b99a83592c2e7bf342ea2680f6a2bb705167966b7420
der_check_vartime: 0 (ok)
chapter 19's reader on the DER part: true
chapter 20's encoder gives the same bytes back: true
encoding (r, s)
r = 0x01: 8 bytes, 3006020101020101
r = 0x7f: 8 bytes, 300602017f020101
r = 0x80: 9 bytes, 300702020080020101
r = 0x00ff: 9 bytes, 3007020200ff020101
r, s of 32 bytes, top bit clear: 70 bytes; set: 72 bytes
refusals
8 bytes: code 1, fewer than 9 bytes
74 bytes: code 2, more than 73 bytes
9 bytes: code 3, not a SEQUENCE
9 bytes: code 4, sequence length is not size - 3
9 bytes: code 5, R length leaves no room for S
9 bytes: code 6, lengths do not add up
9 bytes: code 7, R not an INTEGER
9 bytes: code 8, R empty
9 bytes: code 9, R negative
10 bytes: code 10, R padded
9 bytes: code 11, S not an INTEGER
9 bytes: code 12, S empty
9 bytes: code 13, S negative
10 bytes: code 14, S padded
read by the lax parser, refused by BIP-66
30440220d7a0417c... (70 bytes): lax true, BIP-66 code 9 (R negative)
3044022000605584... (70 bytes): lax true, BIP-66 code 10 (R padded)
3000020101020102... (8 bytes): lax true, BIP-66 code 4 (sequence length is not size - 3)
3006020101020102... (10 bytes): lax true, BIP-66 code 4 (sequence length is not size - 3)
3081060201010201... (9 bytes): lax true, BIP-66 code 4 (sequence length is not size - 3)
3008028101010201... (9 bytes): lax true, BIP-66 code 4 (sequence length is not size - 3)
low-S
high-S signature: DER code 0, DERSIG OK, LOW_S SIG_HIGH_S
with n - s: 71 bytes, LOW_S OK
sighash type 0x04: DERSIG OK, STRICTENC SIG_HASHTYPE
Bitcoin Core script_tests.json: 89 rows
OK: 43 of 43
EVAL_FALSE: 9 of 9
SIG_DER: 30 of 30
SIG_HIGH_S: 1 of 1
SIG_HASHTYPE: 2 of 2
NULLFAIL: 3 of 3
SIG_NULLDUMMY: 1 of 1
89 of 89 rows give the expected result
bip66 fixtures
valid: 9 of 9 accepted, r and s as listed
"DER sequence length is too short": code 1 (fewer than 9 bytes)
"DER sequence length is too long": code 2 (more than 73 bytes)
"Expected DER sequence (48)": code 3 (not a SEQUENCE)
"DER sequence length is invalid": code 4 (sequence length is not size - 3)
"DER sequence length is invalid": code 4 (sequence length is not size - 3)
"Expected DER integer (2)": code 7 (R not an INTEGER)
"Expected DER integer (2)": code 6 (lengths do not add up)
"R length is zero": code 8 (R empty)
"S length is zero": code 12 (S empty)
"R length is too long": code 5 (R length leaves no room for S)
"S length is invalid": code 6 (lengths do not add up)
"R value is negative": code 9 (R negative)
"S value is negative": code 13 (S negative)
"R value excessively padded": code 10 (R padded)
"S value excessively padded": code 14 (S padded)
refusals
empty signature, all flags: OK
8 bytes with DERSIG: SIG_DER
8 bytes without flags: OK
lax parser, 33 nonzero bytes of r: true, r = s = 0
```

Every rule refuses its own signature, and every bip66 fixture refuses. The
fixtures name bitcoinjs's reason, which follows the order of the bytes, so
one differs from Core's first rule: `300c020200ffffff020400ffffff` has an
`R` length of 2, which bitcoinjs reports as a bad tag for S and Core as
lengths that do not add up. All 89 rows give their expected result: 30 `SIG_DER`, one `SIG_HIGH_S`
and two `SIG_HASHTYPE`, decided by the encoding check, and the rest
decided by verification, `NULLFAIL` or `NULLDUMMY` after the check passes.

The refusals:

- An empty signature passes under every flag; it is script's way to fail a
  check.
- Eight bytes refuse under `DERSIG` and pass without flags: before BIP-66,
  nothing checked the encoding.
- 33 nonzero bytes of `r` read laxly as `r = s = 0`, which never verifies.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/strict-der.out](../fixtures/strict-der.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/strict-der.coax](../examples/checks/strict-der.coax),
which loads the example:

- the encoder's output, for 256 combinations of lengths and top bits,
  passes every rule with a sighash byte and reads back through chapter 19's
  reader and the lax parser; one more byte breaks the sequence length;
- each of the fourteen refusal signatures gives its own code;
- the six lax encodings read laxly and refuse strictly;
- the bip66 fixtures: 9 valid encodings pass and 15 refuse, each with the
  code of Core's first failing rule;
- in the 61 `CHECKSIG` rows, the encoding check under each row's flags
  gives exactly the 25 encoding errors the rows expect;
- the 68 rows that no signature verification decides run through
  `run_row` with their expected result; the other 21 need the curve, and
  the example and the native harness run them.

<!-- source: bitcoin/examples/checks/strict-der.coax check -->
```jai
// The encoder's output, with a sighash byte, passes every BIP-66 rule and
// reads back the same r || s through chapter 19's strict reader and the lax
// parser, for every length of r and s from 1 to 32 bytes, with the top bit
// of each first byte set or clear: 3 + 2 + 2 + (1 to 33) + (1 to 33) + 1 bytes.
#forall "encoder output passes BIP-66 and reads back" k: 0..<256 {
  length_r := 1 + k % 32; length_s := 1 + (k / 2) % 32;
  high_r := (k / 64) % 2 == 1; high_s := (k / 128) % 2 == 1;
  compact: [64] u8;
  for i: 0..<length_r { compact[32-length_r+i] = 0x5a; }
  for i: 0..<length_s { compact[64-length_s+i] = 0x3c; }
  compact[32-length_r] = ifx high_r then 0x80 else 0x41;
  compact[64-length_s] = ifx high_s then 0x81 else 0x42;
  out: [80] u8;
  n := ecdsa_der(compact[..], out[..]);
  pad_r := ifx high_r then 1 else 0; pad_s := ifx high_s then 1 else 0;
  #assert n == 6 + length_r + pad_r + length_s + pad_s;
  out[n] = 0x01;
  #assert der_check_vartime(out[0..<n+1]) == DER_OK;
  #assert der_signature_check_vartime(out[0..<n+1], SCRIPT_DERSIG | SCRIPT_STRICTENC) == SIG_OK;
  canonical: [64] u8; lax: [64] u8;
  #assert ecdsa_der_read_vartime(out[0..<n], canonical[..]) && bytes_same(canonical[..], compact[..]);
  #assert der_read_lax_vartime(out[0..<n], lax[..]) && bytes_same(lax[..], compact[..]);
  // One byte more breaks the sequence length, or the size bound at 74.
  out[n+1] = 0x01;
  #assert der_check_vartime(out[0..<n+2]) == (ifx n + 2 > 73 then DER_TOO_LONG else DER_BAD_LENGTH);
  #assert der_check_vartime(out[0..<n]) != DER_OK;
}
// Each entry of REFUSALS refuses with the code of its place, and the
// fourteen codes are distinct and nonzero.
#forall "one signature per refusal code" i: 0..<14 {
  b: [80] u8;
  #assert der_check_vartime(decode(REFUSALS[i], b[..])) == i + 1;
  #assert DER_NAMES[i+1].count > 0;
}
// The six lax encodings: the lax parser reads each one, and BIP-66
// refuses each one with a sighash byte added.
#forall "the lax encodings read laxly and refuse strictly" i: 0..<6 {
  b: [80] u8; out: [64] u8;
  der := decode(LAX[i], b[..]);
  #assert der_read_lax_vartime(der, out[..]);
  b[der.count] = 0x01;
  #assert der_check_vartime(b[0..<der.count+1]) != DER_OK;
  #assert !ecdsa_der_read_vartime(der, out[..]);
}
// bitcoinjs/bip66: its 9 valid encodings pass with a sighash byte, and
// its 15 refused decodings are refused, each by the rule its reason names
// where Core tests that rule first.
BIP66_CODES :: [15] int.[1, 2, 3, 4, 4, 7, 6, 8, 12, 5, 6, 9, 13, 10, 14];
#test "bip66 fixtures" {
  b: [80] u8;
  for i: 0..<DV_VALID.count {
    der := decode(DV_VALID[i].der, b[..]); b[der.count] = 0x01;
    #assert der_check_vartime(b[0..<der.count+1]) == DER_OK;
  }
  for i: 0..<DV_INVALID.count {
    der := decode(DV_INVALID[i].der, b[..]); b[der.count] = 0x01;
    #assert der_check_vartime(b[0..<der.count+1]) == BIP66_CODES[i];
  }
}
// The CHECKSIG rows of script_tests.json that expect an encoding error are
// decided by der_signature_check_vartime under the row's flags; every other
// CHECKSIG row's signature passes it. There are 61 CHECKSIG rows, 25 of
// them refused by the check.
#test "CHECKSIG rows: the encoding check decides exactly the encoding errors" {
  rows := 0; refused := 0;
  for i: 0..<DV_ROWS.count {
    row := DV_ROWS[i];
    if row.multisig { continue; }
    data: [5120] u8; lengths: [64] int;
    n := parse_items(row.stack, data[..], lengths[..]);
    error := SIG_NAMES[der_signature_check_vartime(data[ITEM*(n-2)..<ITEM*(n-2)+lengths[n-2]], row.flags)];
    encoding := row.expected == "SIG_DER" || row.expected == "SIG_HIGH_S" || row.expected == "SIG_HASHTYPE";
    #assert (ifx encoding then row.expected else "OK") == error;
    rows += 1;
    if encoding { refused += 1; }
  }
  #assert rows == 61 && refused == 25;
}
// The 68 rows that no signature verification decides (each signature
// script examines is empty, refused by its encoding, has r = 0, or meets a
// key that does not decode) run through run_row here, each with its
// expected result. The other 21 need the curve and run in the example and
// in der.py. CHEAP lists positions in DV_ROWS.
CHEAP :: [68] int.[0, 1, 2, 3, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 37, 39, 41, 43, 45, 47, 49, 50, 51, 52, 53, 55, 57, 58, 59, 60, 66, 68, 69, 70, 71, 72, 74, 76, 78, 80, 81, 82, 83, 84, 85, 86, 87, 88];
#forall "rows no verification decides give their expected result" i: 0..<68 #steps 400000000 {
  row := DV_ROWS[CHEAP[i]];
  #assert run_row(row) == row.expected;
}
// script_number reads CScriptNum's sign bit; key_shape is STRICTENC's rule.
#test "script_number and key_shape" {
  b: [65] u8;
  #assert script_number(b[0..<0]) == 0 && script_number(decode("14", b[..])) == 20;
  #assert script_number(decode("81", b[..])) == -1 && script_number(decode("ff00", b[..])) == 255;
  #assert script_number(decode("0080", b[..])) == 0 && script_number(decode("0181", b[..])) == -257;
  b[0] = 2; #assert key_shape(b[0..<33]) && !key_shape(b[0..<65]) && !key_shape(b[0..<32]);
  b[0] = 3; #assert key_shape(b[0..<33]);
  b[0] = 4; #assert key_shape(b[0..<65]) && !key_shape(b[0..<33]);
  b[0] = 6; #assert !key_shape(b[0..<65]) && !key_shape(b[0..<33]);
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/strict-der.coax --run-steps 250000000
coax/bin/coax mutate bitcoin/examples/checks/strict-der.coax --root-only --run-steps 250000000
```

On 2026-11-02, `coax test` ran 195 checks, including the loaded files' own
checks, in about 90 seconds on a loaded machine. The 68 rows run through
`run_row` carry a budget of 400 million steps. Root-only mutation of the
checks file finds no mutation sites: the file holds only checks and one
table, and the procedures they call belong to the example and to
`der.coax`, whose mutation run is below.

### The library's own checks

`der.coax` has its own checks: each rule alone with its code, the size
bounds at 9, 73 and 74, the zero rules at each edge, the lax parser on
every accepted violation and every refusal (with the output unwritten),
each length at its boundary (an empty `R` from `0x80`, length bytes that end
the input, a two-byte length of 260, a 16 MiB input with four length
bytes), the overflows that zero the result, low-S at `(n-1)/2` and `(n+1)/2`, all
256 sighash bytes, which flag applies which rule, and five verifications
through `der_checksig_vartime`: Wycheproof case 346 as is, with a padded
`r`, with `s` replaced by `n - s`, with a wrong `s`, and with a broken
sequence tag.

```shell
coax/bin/coax test coax/lib/bitcoin/der.coax
coax/bin/coax mutate coax/lib/bitcoin/der.coax --root-only
```

On 2026-11-02, `coax test` ran 94 checks, including the loaded files', in
under 3 minutes on a loaded machine. Root-only mutation killed all 271
mutants of `der.coax`, with none marked equivalent. The first run left 25
survivors:

- 7 in the test helper `lax_case`, which compared too little to notice a
  wrong result. It now compares all 64 bytes, and its own checks show that
  it refuses wrong results.
- 9 at boundaries that no check reached: `5 + lenR` one byte short of the
  size, an `R` length of exactly `0x80`, length bytes that end the input, a
  two-byte length, and four length bytes in an input long enough to hold
  the length. Each has a check now.
- 7 in comparisons that no input can tell apart, such as a skipped
  sequence length that ends exactly at the end of the input, which refuses
  at the next tag either way. The parser now checks the next tag's
  position once, tests the top bit of the sequence length with `& 0x80`,
  and sizes the copy by `out.count`.
- 2 in Core's loops that skip leading zeros, where skipping the last zero
  and reading it as a digit give the same value. The length test and the
  copy above replace those loops.

### Python and libsecp256k1, natively and in Wasm

[der.py](../../coax/tests/bitcoin/der.py) compiles `der.coax` to C in
checked and release builds and compares it with
[der_reference.py](../../coax/tests/bitcoin/der_reference.py), a Python
transliteration of Core's functions, and with libsecp256k1 `v0.8.0`, on
38,095 inputs: every stack item of the 89 rows, the bip66 fixtures with a
sighash byte, the library's own check vectors, every one-byte change,
deletion, insertion and truncation of 43 well-formed signatures, 6,000
structured encodings with lengths, tags, padding and long forms sometimes
wrong, and random bytes. For each input it compares:

- the refusal code with Python's `IsValidSignatureEncoding`, rule by rule;
- the script check under all eight combinations of `DERSIG`, `LOW_S` and
  `STRICTENC` with Python's `CheckSignatureEncoding`;
- the lax read, byte for byte, with Python's and with libsecp256k1's
  `contrib/lax_der_parsing.c`, the parser Core copied;
- the BIP-66 verdict with libsecp256k1's strict parser: where that parser
  reads nonzero `r` and `s`, BIP-66 accepts exactly the encodings that
  `secp256k1_ecdsa_signature_serialize_der` gives back unchanged;
- chapter 19's strict reader, which accepts nothing that BIP-66 refuses.

On 2026-11-02, 18,559 inputs passed BIP-66, 25,634 read laxly, and 18,314
were compared with libsecp256k1's strict parser, with no disagreement in
the checked or release build. All 89 rows give their expected result in
Python, and the 62 verifications they need agree between Python,
libsecp256k1 and `der_checksig_vartime`; 17 of them succeed. The same C,
built for `wasm32-wasip1` and run in Node, agrees on all 38,095 inputs in
both builds. The run takes about 4 minutes.

```shell
python3 tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed   # once: libsecp256k1 v0.8.0
python3 coax/tests/bitcoin/der.py
```

## Summary

- A script signature is DER plus a sighash byte, 9 to 73 bytes. BIP-66's
  fourteen rules, in Core's order, make its encoding unique, and
  `der_check_vartime` names the rule that refuses.
- OpenSSL's lax parsing made validity depend on the library version and
  platform. BIP-66 replaced it with a short rule, enforced from block
  363,725.
- The encoder writes minimal integers with a `0x00` pad for a high bit, so
  its output always passes.
- Low-S and the sighash type are policy, under `LOW_S` and `STRICTENC`;
  `DERSIG` is consensus.
- The lax parser still reads signatures for verification and decides the
  blocks before BIP-66.
- All 89 pinned `script_tests.json` rows give their expected result, and
  Python and libsecp256k1 agree on every input, natively and in Wasm.

## Dependencies

- [19. Verify ECDSA](ch019-verify-ecdsa.md)
- [20. Sign with ECDSA](ch020-sign-with-ecdsa.md), for the encoder

## Sources

- [BIP-66](https://github.com/bitcoin/bips/blob/master/bip-0066.mediawiki)
  (strict DER signatures) and
  [BIP-62](https://github.com/bitcoin/bips/blob/master/bip-0062.mediawiki)
  (low-S, among the malleability rules it lists).
- Bitcoin Core's `src/script/interpreter.cpp` (`IsValidSignatureEncoding`,
  `IsLowDERSignature`, `IsDefinedHashtypeSignature`,
  `CheckSignatureEncoding`) and `src/pubkey.cpp`
  (`ecdsa_signature_parse_der_lax`, `CheckLowS`), pinned with the
  `script_tests.json` revision `ddea9196` in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/),
  listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- bitcoinjs/bip66 `test/fixtures.json`, pinned at `08169640` in
  [coax/tests/vectors/upstream/bip66/](../../coax/tests/vectors/upstream/bip66/fixtures.json).
- [libsecp256k1](https://github.com/bitcoin-core/secp256k1), release
  `v0.8.0`, and its `contrib/lax_der_parsing.c`, pinned in
  [UPSTREAM.json](../../tools/coax-nostr/fuzz/UPSTREAM.json); tests only.
- [CVE-2014-8275](https://www.openssl.org/news/secadv/20150108.txt), the
  OpenSSL change that refused non-DER encodings.
