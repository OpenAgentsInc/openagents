# 30. Output descriptors and their checksum

A wallet has to know which scripts are its own. Chapter 33 gives it a seed
and a path. It doesn't say which kind of script each key makes, whether the
wallet is a multisig, or which keys the other signers hold. An *output
descriptor* says all of that in one line of text:

```text
wpkh([73c5da0a/84h/0h/0h]xpub6CatWdiZ.../<0;1>/*)#qf45pmyh
```

This line describes a BIP-84 wallet. `wpkh` says each key makes a P2WPKH
output. The bracket names the seed's fingerprint and the path to the
account. The `xpub` is the account key, `<0;1>` gives the receiving and
change chains, `*` stands for every address index, and the eight
characters after `#` are a checksum that catches typing errors. BIP-380
defines the language and the checksum. BIP-381 to BIP-386 define the
functions, and BIP-389 the `<0;1>` step. Bitcoin Core imports and exports
wallets as descriptors, and so do most other wallets.

This chapter computes the checksum, reads every function except Taproot
script trees and miniscript, and expands a descriptor to the scriptPubKey
of any branch, index and output. It checks the result against the
vectors of BIP-380 to BIP-386 and BIP-389, against Bitcoin Core's
`descriptor_tests.cpp`, against BIP-380's own Python checksum code and
against a Python implementation.

## What this chapter teaches

- The descriptor checksum: a BCH code over a 95-character input set, which
  finds any error in up to four characters of a descriptor.
- The functions and the scripts they give: `pk`, `pkh`, `wpkh`, `sh`,
  `wsh`, `multi`, `sortedmulti`, `combo`, `addr`, `raw`, and `tr` with a
  key.
- Where each function may sit: `sh()` only at the top, `wpkh()` not inside
  `wsh()`, uncompressed keys only outside segwit.
- Key expressions: a hex public key, or an `xpub` with a path, a final
  `/*` for a range of addresses, and an optional key origin in brackets.
- BIP-389's multipath step `<0;1>`, which puts the receiving and change
  chains in one descriptor.
- Why descriptors here are public: private keys and hardened steps after
  an `xpub` refuse, each with its own code.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [descriptor.coax](../../coax/lib/bitcoin/descriptor.coax) | `descriptor_checksum_vartime(body, out)` | the eight checksum characters of a descriptor without its `#` |
| | `descriptor_info_vartime(text, network)` | whether a descriptor parses, and its shape: ranged, branches, outputs |
| | `descriptor_script_vartime(text, network, branch, index, nth, out)` | the scriptPubKey of one branch, one index and one output |

The file builds on earlier chapters: `path_step_vartime` from
[chapter 33](ch033-derivation-paths.md), `bip32_decode_ct` and
`bip32_descend_ct` from
[chapter 32](ch032-hierarchical-deterministic-keys.md), the script
builders and `address_decode_vartime` from
[chapter 29](ch029-addresses.md), `wif_decode_ct` from
[chapter 26](ch026-base58check.md), and the point decoders from
[chapter 17](ch017-point-encodings-compressed-uncompressed-and-x-only.md).

A descriptor is public: it holds public keys and paths, never a secret.
The code branches and indexes on it, and the `_vartime` suffix says so.
Each procedure returns a count or one negative `DS_*` refusal, and a
refusal writes nothing. Refusals from chapters 32 and 29 keep their codes,
offset by -40 and -60:

<!-- source: coax/lib/bitcoin/descriptor.coax codes -->
```jai
DS_CHECKSUM_CHARS :: 8;       // the characters after '#'
DS_SCRIPT_MAX :: 10000;       // the longest raw() script: BIP-141's witness script limit
DS_MULTI_MAX :: 20;           // keys in multi(): Bitcoin Core's MAX_PUBKEYS_PER_MULTISIG
DS_BARE_MULTI_MAX :: 3;       // keys in a bare multi(), outside sh() and wsh(): standardness
DS_INNER_MAX :: 1400;         // a buffer for the script inside sh() or wsh()

// Where an expression sits: a key's rules depend on it.
DS_TOP :: 0; DS_P2SH :: 1; DS_P2WSH :: 2; DS_P2WPKH :: 3; DS_P2TR :: 4;

// Refusals.
DS_BAD_CHAR :: -1;            // a character outside BIP-380's input set
DS_CHECKSUM_LENGTH :: -2;     // a '#' not followed by exactly 8 characters
DS_BAD_CHECKSUM :: -3;        // a checksum that isn't the body's
DS_SYNTAX :: -4;              // parentheses that don't pair, or text after the last ')'
DS_UNKNOWN_FUNCTION :: -5;    // a name this chapter doesn't read, or no function at all
DS_CONTEXT :: -6;             // a function where it isn't allowed: sh() in sh(), wpkh() in wsh()
DS_BAD_KEY :: -7;             // not a public key, a hybrid key, a point off the curve
DS_UNCOMPRESSED :: -8;        // an uncompressed key inside wpkh(), wsh() or tr()
DS_PRIVATE_KEY :: -9;         // a WIF, xprv or tprv: descriptors here are public
DS_BAD_ORIGIN :: -10;         // an origin without '[', with two ']' or a fingerprint not 8 hex
DS_BAD_STEP :: -11;           // a path step that isn't a number below 2^31 with h or '
DS_MULTIPATH :: -12;          // <a;b> in an origin, twice in a key, with one item or a repeat,
                              // or keys whose multipath lengths differ
DS_NEEDS_PRIVATE :: -13;      // a hardened step after an xpub, or /*h
DS_THRESHOLD :: -14;          // a multi() threshold that isn't a number from 1 to the key count
DS_KEY_COUNT :: -15;          // multi() with 0 or more than 20 keys, or a bare one with more than 3
DS_SCRIPT_TOO_LONG :: -16;    // a P2SH script over 520 bytes, or raw() over 10,000
DS_BAD_HEX :: -17;            // raw() that isn't hex
DS_BAD_ADDRESS :: -18;        // addr() that isn't an address
DS_WRONG_NETWORK :: -19;      // an xpub, tpub or address for another network
DS_TAPROOT_TREE :: -20;       // tr() with a script tree: chapter 41
DS_BAD_NETWORK :: -21;        // a network outside 1-3
DS_BAD_INDEX :: -22;          // an index below 0 or of 2^31 or more
DS_BAD_BRANCH :: -23;         // a branch below 0 or past the multipath items
DS_NO_OUTPUT :: -24;          // an output past the descriptor's scripts (combo() has 2 or 4)
DS_CAPACITY :: -25;           // an output buffer that is too small
// Refusals from the chapters underneath keep their codes, offset:
DS_BIP32 :: -40;              // plus chapter 32's BIP32_* code: -41 to -50
DS_ADDRESS :: -60;            // plus chapter 29's AD_* code: -61 to -68

// What descriptor_info_vartime finds.
Ds_Info :: struct {
  result: int;                // 0, or a refusal
  ranged: bool;               // a key ends in /*: one script per index
  branches: int;              // the multipath items, or 1
  outputs: int;               // scripts per branch and index: 4 or 2 for combo(), else 1
}
```

## The checksum

A descriptor is long, and people copy it by hand or between programs. The
checksum catches the copy that went wrong. It is a BCH code, like
chapter 27's Bech32 checksum, with a generator of degree 8 over 32
symbols. BIP-380 counts errors in *symbol errors*: any error of up to
three symbols is always found, an error of four symbols is always found
in a descriptor of up to 507 characters, and a random error is missed
with odds of one in 2^40.

Bech32 reads 32 symbols. A descriptor uses 95 characters: digits,
letters, brackets, punctuation and the space. BIP-380 lists them as three
groups of 32. Each character feeds the code its place within its group,
a symbol from 0 to 31. After every three characters, one more symbol
gives their three groups as a number in base 3. The first group holds
the characters that have no checksum of their own, hex digits and path
characters, so swapping one of them for another changes one symbol. The
groups line up the letters so that a case error, such as `A` for `a`,
changes only the group symbol: one symbol error.

<!-- source: coax/lib/bitcoin/descriptor.coax checksum -->
```jai
DS_INPUT :: "0123456789()[],'/*abcdefgh@:$%{}IJKLMNOPQRSTUVWXYZ&+-.;<=>?!^_|~ijklmnopqrstuvwxyzABCDEFGH`#\"\\ ";
DS_CHECK :: "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
DS_GENERATOR :: [5] u64.[0xf5dee51989, 0xa9fdca3312, 0x1bab10e32d, 0x3706b1677a, 0x644d626ffd];
// One step of the BCH code: shift in a 5-bit symbol, and reduce by the
// generator for each of the five bits that fell off the top.
ds_polymod :: (c: u64, symbol: u64) -> u64 {
  top := c >> 35;
  r := ((c & 0x7ffffffff) << 5) ^ symbol;
  for i: 0..<5 { if ((top >> cast(u64) i) & 1) != 0 { r ^= DS_GENERATOR[i]; } }
  return r;
}
// A character's place in the input set: 0 to 94, or -1.
ds_position :: (c: u8) -> int {
  for k: 0..<DS_INPUT.count { if DS_INPUT[k] == c { return k; } }
  return -1;
}
// The eight checksum characters of a body into out. Each character gives
// its place within its group of 32, and after every three characters one
// more symbol gives the three groups.
descriptor_checksum_vartime :: (body: string, out: [] u8) -> int #export_c {
  if out.count < DS_CHECKSUM_CHARS { return DS_CAPACITY; }
  c: u64 = 1; groups: u64 = 0; count := 0;
  for ch: body {
    p := ds_position(ch);
    if p < 0 { return DS_BAD_CHAR; }
    c = ds_polymod(c, cast(u64) (p & 31));
    groups = groups * 3 + cast(u64) (p >> 5); count += 1;
    if count == 3 { c = ds_polymod(c, groups); groups = 0; count = 0; }
  }
  if count > 0 { c = ds_polymod(c, groups); }
  for k: 0..<8 { c = ds_polymod(c, 0); }
  c ^= 1;
  for j: 0..<8 { out[j] = DS_CHECK[cast(int) ((c >> cast(u64) (5 * (7 - j))) & 31)]; }
  return DS_CHECKSUM_CHARS;
}
// The body of a descriptor: the text before '#', whose checksum must match
// when there is one. Returns the body's length or a refusal.
ds_body :: (text: string) -> int {
  hash := -1;
  for i: 0..<text.count { if text[i] == '#' && hash < 0 { hash = i; } }
  if hash < 0 {
    for ch: text { if ds_position(ch) < 0 { return DS_BAD_CHAR; } }
    return text.count;
  }
  if text.count - hash - 1 != DS_CHECKSUM_CHARS { return DS_CHECKSUM_LENGTH; }
  want: [8] u8;
  r := descriptor_checksum_vartime(text[0..<hash], want[..]);
  if r < 0 { return r; }
  for j: 0..<8 { if want[j] != text[hash + 1 + j] { return DS_BAD_CHECKSUM; } }
  return hash;
}
```

`ds_polymod` is chapter 27's polymod step with a 40-bit state. The final
eight zero symbols make room for the checksum, and the result is XORed
with 1 as Bech32's is. The eight 5-bit values of the result are written
with Bech32's alphabet.

The checksum is optional. Without one the descriptor parses as it is; with
one, the eight characters after the `#` must be the body's checksum.
`ds_body` returns the length of the body, the text the rest of the code
reads.

The Nostr book's chapter 104 uses the same checksum, as `ky_checksum` in
[nipkeys.coax](../../coax/lib/nostr/nipkeys.coax), for the wallet
descriptors that its key agents sign for. This chapter's version adds the
refusals and the exported entry point.

### Worked example: `raw(deadbeef)`

BIP-380 gives `raw(deadbeef)#89f8spxm`. The first group is the digits,
`()[],'/*`, `abcdefgh` and `@:$%{}`, so `(`, `)` and every letter of
`deadbeef` are in group 0. `r` and `w` are in group 2. The code reads 13
place symbols and, after each set of three characters and once for the
last one, a group symbol: `raw` gives 2·9 + 0·3 + 2 = 20, and the sets
after it give 0. Changing `deadbeef` to `deedbeef` changes one place
symbol, and the checksum no longer matches: the example below prints
`bad checksum`. Dropping the last checksum character prints
`checksum length`.

## Reading the text

A descriptor is a function name, `(`, its arguments separated by commas,
and `)`. An argument is a key, a number or another expression.
The helpers find the `)` that closes a `(`, the end of one argument (the
next comma outside parentheses), and read hex the way Bitcoin Core's
`IsHex` does: an even number of digits, in either case.

<!-- source: coax/lib/bitcoin/descriptor.coax text -->
```jai
// The first c at or after from, or -1.
ds_find :: (s: string, c: u8, from: int) -> int {
  for i: from..<s.count { if s[i] == c { return i; } }
  return -1;
}
// The ')' that closes the '(' at open, or -1.
ds_close :: (s: string, open: int) -> int {
  depth := 0;
  for i: open..<s.count {
    if s[i] == '(' { depth += 1; }
    if s[i] == ')' { depth -= 1; if depth == 0 { return i; } }
  }
  return -1;
}
// The end of the argument that starts at from: the next ',' outside
// parentheses, or the end of s.
ds_arg_end :: (s: string, from: int) -> int {
  depth := 0;
  for i: from..<s.count {
    if s[i] == '(' { depth += 1; }
    if s[i] == ')' { depth -= 1; }
    if s[i] == ',' && depth == 0 { return i; }
  }
  return s.count;
}
ds_hex_digit :: (c: u8) -> int {
  if c >= '0' && c <= '9' { return cast(int) (c - '0'); }
  if c >= 'a' && c <= 'f' { return cast(int) (c - 'a') + 10; }
  if c >= 'A' && c <= 'F' { return cast(int) (c - 'A') + 10; }
  return -1;
}
// Bitcoin Core's IsHex: an even number of hex digits, at least two, in
// either case.
ds_is_hex :: (s: string) -> bool {
  if s.count == 0 || s.count % 2 != 0 { return false; }
  for ch: s { if ds_hex_digit(ch) < 0 { return false; } }
  return true;
}
ds_unhex :: (s: string, out: [] u8) {
  for i: 0..<s.count / 2 { out[i] = cast(u8) (ds_hex_digit(s[2 * i]) * 16 + ds_hex_digit(s[2 * i + 1])); }
}
// A path step, or DS_BAD_STEP.
ds_step :: (s: string) -> int {
  r := path_step_vartime(s);
  return ifx r < 0 then DS_BAD_STEP else r;
}
ds_is_multipath :: (s: string) -> bool { return s.count >= 2 && s[0] == '<' && s[s.count - 1] == '>'; }
```

## Keys

### The key origin

A key may start with an origin in brackets: the fingerprint of the seed
it came from, eight hex characters, and the path from the seed to the
key. `[73c5da0a/84h/0h/0h]` says that the key after it is the seed's
`m/84h/0h/0h`. The origin changes no script. A signer uses it to find
which of its keys to sign with, so the code only checks it: the
fingerprint, each step with chapter 33's rules, at most 255 steps, and no
multipath step.

<!-- source: coax/lib/bitcoin/descriptor.coax origin -->
```jai
// A key origin's text between '[' and ']': 8 hex characters of fingerprint,
// then /steps. It names where the key came from and changes no script, so
// it is only checked. Returns 0 or a refusal.
ds_origin :: (s: string) -> int {
  slash := ds_find(s, '/', 0);
  if slash < 0 { slash = s.count; }
  if slash != 8 || !ds_is_hex(s[0..<8]) { return DS_BAD_ORIGIN; }
  steps := 0; at := slash;
  while at < s.count {
    end := ds_find(s, '/', at + 1);
    if end < 0 { end = s.count; }
    step := s[at + 1..<end];
    if ds_is_multipath(step) { return DS_MULTIPATH; }
    if ds_step(step) < 0 { return DS_BAD_STEP; }
    steps += 1;
    if steps > DP_MAX_DEPTH { return DS_BIP32 + BIP32_BAD_DEPTH; }
    at = end;
  }
  return 0;
}
```

### Hex keys

A hex key is a compressed key of 33 bytes or an uncompressed one of 65.
The point must be on the curve. A *hybrid* key, an uncompressed key with
prefix `06` or `07`, refuses: consensus still accepts one, but
descriptors, like Bitcoin Core's policy, do not. Inside `tr()` a key is 32 bytes of x coordinate, and a
33-byte key there gives its x coordinate.

An uncompressed key works only in legacy scripts: segwit rejects it as
nonstandard, so `wpkh()`, `wsh()` and `tr()` refuse one with
`DS_UNCOMPRESSED`.

<!-- source: coax/lib/bitcoin/descriptor.coax hexkey -->
```jai
// A key written in hex, or a WIF. In tr() a key is its 32-byte x
// coordinate. Returns the key's length or a refusal.
ds_plain_key :: (s: string, ctx: int, key_out: [] u8) -> int {
  if ds_is_hex(s) {
    if s.count > 130 { return DS_BAD_KEY; }
    b: [65] u8; n := s.count / 2;
    ds_unhex(s, b[..]);
    if n == SE_UNCOMPRESSED && (b[0] == 6 || b[0] == 7) { return DS_BAD_KEY; }   // hybrid
    if (n == SE_COMPRESSED || n == SE_UNCOMPRESSED) && se_decode(b[0..<n]).ok {
      if n == SE_UNCOMPRESSED && ctx != DS_TOP && ctx != DS_P2SH { return DS_UNCOMPRESSED; }
      if ctx == DS_P2TR { for i: 0..<32 { key_out[i] = b[1 + i]; } return SE_XONLY; }
      for i: 0..<n { key_out[i] = b[i]; }
      return n;
    }
    if n == SE_XONLY && ctx == DS_P2TR && se_decode_xonly(b[0..<n]).ok {
      for i: 0..<32 { key_out[i] = b[i]; }
      return SE_XONLY;
    }
    return DS_BAD_KEY;
  }
  // A WIF is a private key. It is read with chapter 26's constant-time
  // decoder only to refuse it, and the key is wiped.
  work: [WIF_WORK_BYTES] u8; k: [32] u8;
  r := wif_decode_ct(s, work[..], k[..]);
  for i: 0..<32 { k[i] = 0; }
  if r >= B58_MAIN_WIF { return DS_PRIVATE_KEY; }
  return DS_BAD_KEY;
}
```

A WIF private key is valid descriptor syntax. Bitcoin Core accepts one
when a wallet imports its keys. This chapter's descriptors are public, and
a descriptor with a private key in it is a secret that shouldn't be
pasted, logged or sent. So a WIF refuses with `DS_PRIVATE_KEY`. It is
still read with chapter 26's constant-time decoder, so that a valid WIF
and a malformed one refuse with different codes, and the decoded key is
wiped.

### Extended keys

An extended key is an `xpub` (or a `tpub` on test networks) followed by
steps. Each step is a number with chapter 33's rules. The last step may
be `*`, which means "every index": the descriptor is then *ranged*, and
a wallet asks for the script at index 0, 1, 2 and so on. One step may
be a multipath list `<a;b;...>` (BIP-389), which puts several chains in
one descriptor: `<0;1>` is the receiving chain and the change chain.

<!-- source: coax/lib/bitcoin/descriptor.coax extkey -->
```jai
// An xpub or tpub, then /steps, an optional <a;b> step and an optional
// final /*. Returns the child's key length or a refusal; with expand false
// the xpub's own key stands in for the child.
ds_ext_key :: (s: string, ctx: int, env: [] Ds_Env, key_out: [] u8) -> int {
  kend := ds_find(s, '/', 0);
  if kend < 0 { kend = s.count; }
  node: [BIP32_NODE_BYTES] u8; work: [BIP32_WORK_BYTES] u8;
  if kend != BIP32_TEXT || bip32_decode_ct(s[0..<kend], work[..], node[..]) < 0 { return DS_BAD_KEY; }
  if node[HDK_KEY] == 0 { for i: 0..<BIP32_NODE_BYTES { node[i] = 0; } return DS_PRIVATE_KEY; }
  mainnet := hdk_get32(node[..], HDK_VERSION) == BIP32_XPUB;
  if mainnet != (env[0].network == SW_MAINNET) { return DS_WRONG_NETWORK; }
  // The steps after the key. A last step of * takes the index.
  steps: [DP_MAX_DEPTH] int; count := 0;
  items: [DP_MAX_DEPTH] int; choices := 0; slot := -1;
  hardened := false; star := -1;
  at := kend;
  while at < s.count {
    end := ds_find(s, '/', at + 1);
    if end < 0 { end = s.count; }
    step := s[at + 1..<end];
    if count == DP_MAX_DEPTH { return DS_BIP32 + BIP32_BAD_DEPTH; }
    if end == s.count && step.count >= 1 && step[0] == '*' && (step.count == 1 || (step.count == 2 && (step[1] == 'h' || step[1] == '\''))) {
      if step.count == 2 { hardened = true; }
      star = count; steps[count] = 0;
    } else if ds_is_multipath(step) {
      if slot >= 0 { return DS_MULTIPATH; }
      slot = count; steps[count] = 0;
      list := step[1..<step.count - 1]; from := 0;
      while true {
        semi := ds_find(list, ';', from);
        if semi < 0 { semi = list.count; }
        if choices == DP_MAX_DEPTH { return DS_MULTIPATH; }
        v := ds_step(list[from..<semi]);
        if v < 0 { return v; }
        for j: 0..<choices { if items[j] == v { return DS_MULTIPATH; } }
        items[choices] = v; choices += 1;
        if v >= BIP32_HARDENED { hardened = true; }
        if semi == list.count { break; }
        from = semi + 1;
      }
      if choices < 2 { return DS_MULTIPATH; }
    } else {
      v := ds_step(step);
      if v < 0 { return v; }
      if v >= BIP32_HARDENED { hardened = true; }
      steps[count] = v;
    }
    count += 1; at = end;
  }
  if cast(int) node[HDK_DEPTH] + count > DP_MAX_DEPTH { return DS_BIP32 + BIP32_BAD_DEPTH; }
  if hardened { return DS_NEEDS_PRIVATE; }
  if slot >= 0 {
    if env[0].branches == 0 { env[0].branches = choices; }
    else if env[0].branches != choices { return DS_MULTIPATH; }
    steps[slot] = ifx env[0].expand then items[env[0].branch] else items[0];
  }
  if star >= 0 { env[0].ranged = true; steps[star] = ifx env[0].expand then env[0].index else 0; }
  if env[0].expand {
    r := bip32_descend_ct(node[..], steps[0..<count], work[..], node[..]);
    if r < 0 { return DS_BIP32 + r; }
  }
  if ctx == DS_P2TR { for i: 0..<32 { key_out[i] = node[HDK_KEY + 1 + i]; } return SE_XONLY; }
  for i: 0..<SE_COMPRESSED { key_out[i] = node[HDK_KEY + i]; }
  return SE_COMPRESSED;
}
```

The refusals follow from what an `xpub` can do:

- A hardened step needs the private key (chapter 32), so a hardened step
  after an `xpub`, or `*h`, refuses with `DS_NEEDS_PRIVATE`. An `xprv`
  or `tprv` refuses with `DS_PRIVATE_KEY`.
- An `xpub` for mainnet on testnet, or a `tpub` on mainnet, refuses with
  `DS_WRONG_NETWORK`.
- A multipath list needs at least two items, no item twice, and only one
  list per key. Every multipath key in a descriptor needs the same number
  of items, because branch 1 means item 1 of each.
- The key's depth plus the steps must stay at most 255.

The code makes two passes over a descriptor. The first one, with `expand`
false, checks the text and records its shape. It derives nothing: the
`xpub`'s own key stands in for its child, so a descriptor of 20
extended keys is checked without 20 derivations. The second pass, with
`expand` true, picks the branch's item for the multipath step and the
index for `*`, and derives the child with chapter 32's
`bip32_descend_ct`.

`ds_key` reads the origin and then tries the key as a hex key or a WIF
before trying it as an extended key:

<!-- source: coax/lib/bitcoin/descriptor.coax key -->
```jai
// A key expression: an optional [origin], then a hex key, a WIF or an
// extended key. Returns the key's length or a refusal.
ds_key :: (s: string, ctx: int, env: [] Ds_Env, key_out: [] u8) -> int {
  close := -1;
  for i: 0..<s.count { if s[i] == ']' { if close >= 0 { return DS_BAD_ORIGIN; } close = i; } }
  rest := s;
  if close >= 0 {
    if s[0] != '[' { return DS_BAD_ORIGIN; }
    r := ds_origin(s[1..<close]);
    if r < 0 { return r; }
    rest = s[close + 1..<s.count];
  }
  kend := ds_find(rest, '/', 0);
  if kend < 0 { kend = rest.count; }
  if kend == 0 || rest[0] == ' ' || rest[kend - 1] == ' ' { return DS_BAD_KEY; }
  if kend == rest.count {
    r := ds_plain_key(rest, ctx, key_out);
    if r != DS_BAD_KEY { return r; }
  }
  return ds_ext_key(rest, ctx, env, key_out);
}
```

## Scripts

### multi() and sortedmulti()

`multi(k,KEY_1,...,KEY_n)` is a bare k-of-n multisig script:
`OP_k <KEY_1> ... <KEY_n> OP_n OP_CHECKMULTISIG`. `sortedmulti()` writes
the same script with its keys in byte order (BIP-67), so that every
signer builds the same script whatever order they list the keys in.
`k` must be a number from 1 to `n`. `n` is at most 20, Bitcoin Core's
limit for `OP_CHECKMULTISIG`, and at most 3 for a bare multisig at the
top, which is the most a standard bare multisig output may have. Inside
`sh()` the script must fit in 520 bytes, the limit for a pushed redeem
script, which allows 15 compressed keys.

<!-- source: coax/lib/bitcoin/descriptor.coax multi -->
```jai
// A push of a small number: OP_1 to OP_16, or one byte pushed.
ds_number :: (n: int, out: [] u8, at: int) -> int {
  if n <= 16 { out[at] = cast(u8) (0x50 + n); return at + 1; }
  out[at] = 1; out[at + 1] = cast(u8) n; return at + 2;
}
// Whether key a sorts before key b, byte by byte (BIP-67).
ds_before :: (keys: [] u8, lengths: [] int, a: int, b: int) -> bool {
  la := lengths[a]; lb := lengths[b];
  m := ifx la < lb then la else lb;
  for i: 0..<m {
    x := keys[a * 65 + i]; y := keys[b * 65 + i];
    if x != y { return x < y; }
  }
  return la < lb;
}
// multi(k,KEY,...) and sortedmulti(): k OP_n keys OP_CHECKMULTISIG. Every
// key is read before the counts are checked, as Bitcoin Core does.
ds_multi :: (inner: string, ctx: int, sorted: bool, env: [] Ds_Env, out: [] u8) -> int {
  e := ds_arg_end(inner, 0);
  threshold := inner[0..<e];
  if threshold.count == 0 { return DS_THRESHOLD; }
  k := 0;
  for ch: threshold {
    if ch < '0' || ch > '9' { return DS_THRESHOLD; }
    if k <= DS_MULTI_MAX { k = k * 10 + cast(int) (ch - '0'); }   // past 20 the value only has to stay too big
  }
  keys: [DS_MULTI_MAX * 65] u8; lengths: [DS_MULTI_MAX] int; one: [65] u8;
  n := 0; at := e;
  while at < inner.count {
    end := ds_arg_end(inner, at + 1);
    r := ds_key(inner[at + 1..<end], ctx, env, one[..]);
    if r < 0 { return r; }
    if n < DS_MULTI_MAX { for i: 0..<r { keys[n * 65 + i] = one[i]; } lengths[n] = r; }
    n += 1; at = end;
  }
  if n == 0 || n > DS_MULTI_MAX { return DS_KEY_COUNT; }
  if k < 1 || k > n { return DS_THRESHOLD; }
  if ctx == DS_TOP && n > DS_BARE_MULTI_MAX { return DS_KEY_COUNT; }
  order: [DS_MULTI_MAX] int;
  for i: 0..<n { order[i] = i; }
  if sorted {
    for i: 1..<n {
      j := i;
      while j > 0 && ds_before(keys[..], lengths[..], order[j], order[j - 1]) {
        t := order[j]; order[j] = order[j - 1]; order[j - 1] = t; j -= 1;
      }
    }
  }
  size := (ifx k <= 16 then 1 else 2) + (ifx n <= 16 then 1 else 2) + 1;
  for i: 0..<n { size += lengths[i] + 1; }
  if ctx == DS_P2SH && size > AD_REDEEM_MAX { return DS_SCRIPT_TOO_LONG; }
  if out.count < size { return DS_CAPACITY; }
  w := ds_number(k, out, 0);
  for i: 0..<n {
    key := order[i];
    out[w] = cast(u8) lengths[key]; w += 1;
    for b: 0..<lengths[key] { out[w] = keys[key * 65 + b]; w += 1; }
  }
  w = ds_number(n, out, w);
  out[w] = 0xae;                                         // OP_CHECKMULTISIG
  return w + 1;
}
```

As Bitcoin Core does, the code reads every key before it checks the
counts, so a bad key in a descriptor with a bad threshold refuses for the
key.

### The functions

`ds_script` reads one expression in its context: at the top, inside
`sh()` or inside `wsh()`. Each function gives one script:

| Function | Script | Allowed in |
| --- | --- | --- |
| `pk(KEY)` | `<KEY> OP_CHECKSIG` | anywhere |
| `pkh(KEY)` | P2PKH | anywhere |
| `wpkh(KEY)` | P2WPKH | top, `sh()` |
| `sh(SCRIPT)` | P2SH of the inner script | top |
| `wsh(SCRIPT)` | P2WSH of the inner script | top, `sh()` |
| `multi(...)`, `sortedmulti(...)` | bare multisig | anywhere |
| `combo(KEY)` | P2PK, P2PKH, and for a compressed key P2WPKH and P2SH-P2WPKH | top |
| `addr(ADDRESS)` | the address's script (chapter 29) | top |
| `raw(HEX)` | the bytes as given, up to 10,000 | top |
| `tr(KEY)` | P2TR with the key-path tweak (BIP-86) | top |

<!-- source: coax/lib/bitcoin/descriptor.coax script -->
```jai
// A chapter 29 script builder's refusal, offset.
ds_address :: (n: int) -> int { return ifx n < 0 then DS_ADDRESS + n else n; }
// <key> OP_CHECKSIG.
ds_pk :: (key: [] u8, out: [] u8) -> int {
  if out.count < key.count + 2 { return DS_CAPACITY; }
  out[0] = cast(u8) key.count;
  for i: 0..<key.count { out[1 + i] = key[i]; }
  out[key.count + 1] = 0xac;
  return key.count + 2;
}
// The script of the expression s, which sits in ctx. nth picks one of
// combo()'s outputs.
ds_script :: (s: string, ctx: int, env: [] Ds_Env, nth: int, out: [] u8) -> int {
  open := ds_find(s, '(', 0);
  if open < 0 { return DS_UNKNOWN_FUNCTION; }
  if ds_close(s, open) != s.count - 1 { return DS_SYNTAX; }
  name := s[0..<open]; inner := s[open + 1..<s.count - 1];
  key: [65] u8;
  if name == "pk" || name == "pkh" {
    n := ds_key(inner, ctx, env, key[..]);
    if n < 0 { return n; }
    if name == "pk" { return ds_pk(key[0..<n], out); }
    return ds_address(p2pkh_script_vartime(key[0..<n], out));
  }
  if name == "wpkh" {
    if ctx != DS_TOP && ctx != DS_P2SH { return DS_CONTEXT; }
    n := ds_key(inner, DS_P2WPKH, env, key[..]);
    if n < 0 { return n; }
    return ds_address(p2wpkh_script_vartime(key[0..<n], out));
  }
  if name == "combo" {
    if ctx != DS_TOP { return DS_CONTEXT; }
    n := ds_key(inner, DS_TOP, env, key[..]);
    if n < 0 { return n; }
    env[0].outputs = ifx n == SE_COMPRESSED then 4 else 2;   // no segwit for an uncompressed key
    if nth >= env[0].outputs { return DS_NO_OUTPUT; }
    if nth == 0 { return ds_pk(key[0..<n], out); }
    if nth == 1 { return ds_address(p2pkh_script_vartime(key[0..<n], out)); }
    if nth == 2 { return ds_address(p2wpkh_script_vartime(key[0..<n], out)); }
    return ds_address(p2sh_p2wpkh_script_vartime(key[0..<n], out));
  }
  if name == "multi" || name == "sortedmulti" {
    return ds_multi(inner, ctx, name == "sortedmulti", env, out);
  }
  if name == "sh" || name == "wsh" {
    if name == "sh" && ctx != DS_TOP { return DS_CONTEXT; }
    if name == "wsh" && ctx != DS_TOP && ctx != DS_P2SH { return DS_CONTEXT; }
    buffer: [DS_INNER_MAX] u8;
    n := ds_script(inner, ifx name == "sh" then DS_P2SH else DS_P2WSH, env, 0, buffer[..]);
    if n < 0 { return n; }
    if name == "sh" {
      if n > AD_REDEEM_MAX { return DS_SCRIPT_TOO_LONG; }
      return ds_address(p2sh_script_vartime(buffer[0..<n], out));
    }
    return ds_address(p2wsh_script_vartime(buffer[0..<n], out));
  }
  if name == "addr" {
    if ctx != DS_TOP { return DS_CONTEXT; }
    a := address_decode_vartime(env[0].network, inner, out);
    if a.length == AD_WRONG_NETWORK { return DS_WRONG_NETWORK; }
    if a.length == AD_CAPACITY { return DS_CAPACITY; }
    if a.length < 0 { return DS_BAD_ADDRESS; }
    return a.length;
  }
  if name == "raw" {
    if ctx != DS_TOP { return DS_CONTEXT; }
    if !ds_is_hex(inner) { return DS_BAD_HEX; }
    if inner.count / 2 > DS_SCRIPT_MAX { return DS_SCRIPT_TOO_LONG; }
    if out.count < inner.count / 2 { return DS_CAPACITY; }
    ds_unhex(inner, out);
    return inner.count / 2;
  }
  if name == "tr" {
    if ctx != DS_TOP { return DS_CONTEXT; }
    if ds_arg_end(inner, 0) != inner.count { return DS_TAPROOT_TREE; }
    n := ds_key(inner, DS_P2TR, env, key[..]);
    if n < 0 { return n; }
    if !env[0].expand { return 34; }                      // the tweak waits for the real key
    return ds_address(p2tr_script_vartime(key[0..<SE_XONLY], out));
  }
  return DS_UNKNOWN_FUNCTION;
}
```

`combo()` is the one function with more than one output: a wallet that
holds one key and doesn't know which script it paid to watches all of
them. Its `nth` argument picks one. An uncompressed key gets only the
first two, because segwit doesn't allow it.

`tr()` with a key gives the same output as a BIP-86 address: chapter 29's
`p2tr_script_vartime` tweaks the key with an empty script tree, as
[chapter 41](ch041-the-key-tweak-and-the-key-path.md) explains. `tr()`
with a script tree, `tr(KEY,{...})`, refuses with `DS_TAPROOT_TREE`.
[Chapter 42](ch042-script-trees-tapleaves-and-tapbranches.md) builds
script trees, but their leaves are tapscripts, which this chapter doesn't
read. Any other name, including miniscript's `and_v` or `older`, refuses
with `DS_UNKNOWN_FUNCTION`; miniscript is [chapter 98](PLAN.md).

## The entry points

`descriptor_info_vartime` runs the first pass and reports the shape:
whether the descriptor is ranged, how many branches its multipath step
gives (1 without one), and how many outputs each branch and index gives
(4 or 2 for `combo()`, 1 for the rest). `descriptor_script_vartime`
checks the branch, index and output against that shape before it
derives anything, then runs the second pass:

<!-- source: coax/lib/bitcoin/descriptor.coax entry -->
```jai
// Whether text parses on network, and its shape. Keys aren't derived.
descriptor_info_vartime :: (text: string, network: int) -> Ds_Info #export_c {
  if network < SW_MAINNET || network > SW_REGTEST { return Ds_Info.{result = DS_BAD_NETWORK}; }
  body := ds_body(text);
  if body < 0 { return Ds_Info.{result = body}; }
  env: [1] Ds_Env;
  env[0] = Ds_Env.{network = network, expand = false, outputs = 1};
  buffer: [DS_SCRIPT_MAX] u8;
  r := ds_script(text[0..<body], DS_TOP, env[..], 0, buffer[..]);
  if r < 0 { return Ds_Info.{result = r}; }
  return Ds_Info.{result = 0, ranged = env[0].ranged,
    branches = ifx env[0].branches == 0 then 1 else env[0].branches, outputs = env[0].outputs};
}
// The scriptPubKey for one branch (a multipath item, 0 without one), one
// index (the /* child, ignored when there is none) and one output (0, or
// 0 to 3 for combo()). Returns its length or a refusal.
descriptor_script_vartime :: (text: string, network: int, branch: int, index: int, nth: int, out: [] u8) -> int #export_c {
  info := descriptor_info_vartime(text, network);
  if info.result < 0 { return info.result; }
  if branch < 0 || branch >= info.branches { return DS_BAD_BRANCH; }
  if index < 0 || index >= BIP32_HARDENED { return DS_BAD_INDEX; }
  if nth < 0 || nth >= info.outputs { return DS_NO_OUTPUT; }
  env: [1] Ds_Env;
  env[0] = Ds_Env.{network = network, branch = branch, index = index, expand = true, outputs = 1};
  buffer: [DS_SCRIPT_MAX] u8;
  n := ds_script(text[0..<ds_body(text)], DS_TOP, env[..], nth, buffer[..]);
  if n < 0 { return n; }
  if out.count < n { return DS_CAPACITY; }
  for i: 0..<n { out[i] = buffer[i]; }
  return n;
}
```

A wallet with the descriptor of the first line of this chapter calls
`descriptor_script_vartime(text, SW_MAINNET, 0, i, 0, out)` for its
receiving addresses and branch 1 for its change, and scans with
chapter 33's gap limit. A descriptor that isn't ranged ignores the index.

## Refusals

| Code | When |
| --- | --- |
| `DS_BAD_CHAR` | a character outside BIP-380's 95 |
| `DS_CHECKSUM_LENGTH` | a `#` not followed by exactly 8 characters |
| `DS_BAD_CHECKSUM` | a checksum that isn't the body's |
| `DS_SYNTAX` | parentheses that don't pair, or text after the last `)` |
| `DS_UNKNOWN_FUNCTION` | a name this chapter doesn't read, or no `(` |
| `DS_CONTEXT` | a function where it isn't allowed, such as `sh(sh(...))` |
| `DS_BAD_KEY` | not a key: wrong length, off the curve, hybrid, or not an extended key |
| `DS_UNCOMPRESSED` | an uncompressed key in `wpkh()`, `wsh()` or `tr()` |
| `DS_PRIVATE_KEY` | a WIF, `xprv` or `tprv` |
| `DS_BAD_ORIGIN` | an origin without `[`, two `]`, or a fingerprint that isn't 8 hex characters |
| `DS_BAD_STEP` | a step that isn't a number below 2^31 with an optional `h` or `'` |
| `DS_MULTIPATH` | a multipath step in an origin, twice in a key, with one item or a repeat, or keys with different numbers of items |
| `DS_NEEDS_PRIVATE` | a hardened step after an extended public key, or `*h` |
| `DS_THRESHOLD` | a `multi()` threshold that isn't a number from 1 to the key count |
| `DS_KEY_COUNT` | `multi()` with no keys, more than 20, or more than 3 at the top |
| `DS_SCRIPT_TOO_LONG` | a P2SH script over 520 bytes, or `raw()` over 10,000 |
| `DS_BAD_HEX` | `raw()` that isn't hex |
| `DS_BAD_ADDRESS` | `addr()` that isn't an address |
| `DS_WRONG_NETWORK` | an extended key or address for another network |
| `DS_TAPROOT_TREE` | `tr()` with a script tree |
| `DS_BAD_NETWORK` | a network other than mainnet, testnet or regtest |
| `DS_BAD_INDEX` | an index below 0 or of 2^31 or more |
| `DS_BAD_BRANCH` | a branch below 0 or past the multipath items |
| `DS_NO_OUTPUT` | an output past the descriptor's outputs |
| `DS_CAPACITY` | an output buffer that is too small |

## The example

[bitcoin/examples/descriptors.coax](../examples/descriptors.coax)
computes a checksum and checks two bad ones, expands BIP-84's and
BIP-86's account keys from chapter 33 as descriptors, lists `combo()`'s
four outputs, builds a 2-of-3 multisig on mainnet and testnet, and shows
nine refusals.

<!-- source: bitcoin/examples/descriptors.coax example -->
```jai
#load "../../coax/lib/bitcoin/descriptor.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

REFUSALS :: [25] string.["bad character", "checksum length", "bad checksum", "syntax", "unknown function",
  "context", "bad key", "uncompressed", "private key", "bad origin", "bad step", "multipath",
  "needs private", "threshold", "key count", "script too long", "bad hex", "bad address",
  "wrong network", "taproot tree", "bad network", "bad index", "bad branch", "no output", "capacity"];
refusal :: (code: int) -> string { return ifx code < 0 && code >= -25 then REFUSALS[-code - 1] else "?"; }
// A descriptor with its checksum appended, into out.
with_checksum :: (body: string, out: [] u8) -> string {
  for i: 0..<body.count { out[i] = body[i]; }
  out[body.count] = '#';
  #assert descriptor_checksum_vartime(body, out[body.count + 1..<body.count + 9]) == 8;
  return cast(string) out[0..<body.count + 9];
}
// One script of a descriptor, as hex and as its address when it has one.
show :: (label: string, text: string, network: int, branch: int, index: int, nth: int) {
  script: [DS_SCRIPT_MAX] u8; hex: [200] u8; addr: [90] u8;
  n := descriptor_script_vartime(text, network, branch, index, nth, script[..]);
  if n < 0 { print("%: %\n", label, refusal(n)); return; }
  a := address_encode_vartime(network, script[0..<n], addr[..]);
  if a > 0 { print("%: %\n", label, cast(string) addr[0..<a]); return; }
  r := hex_encode_checked(cast(string) script[0..<n], hex[..], false);
  print("%: %\n", label, cast(string) hex[0..<r.written]);
}
// Whether a descriptor parses, and its shape.
describe :: (text: string) {
  info := descriptor_info_vartime(text, SW_MAINNET);
  if info.result < 0 { print("% -> %\n", text, refusal(info.result)); return; }
  print("% -> ranged %, % branches, % outputs\n", text, info.ranged, info.branches, info.outputs);
}

// BIP-84's and BIP-86's account keys for the "abandon ... about" seed
// (chapter 33), under its master fingerprint 73c5da0a.
WALLET84 :: "wpkh([73c5da0a/84h/0h/0h]xpub6CatWdiZiodmUeTDp8LT5or8nmbKNcuyvz7WyksVFkKB4RHwCD3XyuvPEbvqAQY3rAPshWcMLoP2fMFMKHPJ4ZeZXYVUhLv1VMrjPC7PW6V/<0;1>/*)";
WALLET86 :: "tr([73c5da0a/86h/0h/0h]xpub6BgBgsespWvERF3LHQu6CnqdvfEvtMcQjYrcRzx53QJjSxarj2afYWcLteoGVky7D3UKDP9QyrLprQ3VCECoY49yfdDEHGCtMMj92pReUsQ/0/*)";
KEY :: "03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd";
MULTI :: "wsh(sortedmulti(2,03669b8afcec803a0d323e9a17f3ea8e68e8abe5a278020a929adbec52421adbd0,0260b2003c386519fc9eadf2b5cf124dd8eea4c4e68d5e154050a9346ea98ce600,0362a74e399c39ed5593852a30147f2959b56bb827dfa3e60e464b02ccf87dc5e8))";

main :: () {
  // The checksum: eight characters of a BCH code over the text.
  print("Checksums\n");
  text: [400] u8;
  print("%\n", with_checksum("raw(deadbeef)", text[..]));
  describe("raw(deadbeef)#89f8spxm");
  describe("raw(deedbeef)#89f8spxm");
  describe("raw(deadbeef)#89f8spx");

  // A BIP-84 wallet in one line: receiving and change chains, every index.
  print("A BIP-84 wallet\n");
  full := with_checksum(WALLET84, text[..]);
  print("%\n", full);
  describe(full);
  show("receive 0", full, SW_MAINNET, 0, 0, 0);
  show("receive 1", full, SW_MAINNET, 0, 1, 0);
  show("change 0", full, SW_MAINNET, 1, 0, 0);
  show("branch 2", full, SW_MAINNET, 2, 0, 0);

  // BIP-86: a Taproot key path from the account xpub.
  print("A BIP-86 wallet\n");
  describe(WALLET86);
  show("receive 0", WALLET86, SW_MAINNET, 0, 0, 0);

  // combo(): every script a single key can have.
  print("combo()\n");
  c: [100] u8;
  combo := cast(string) c[0..<KEY.count + 7];
  for i: 0..<6 { c[i] = "combo("[i]; }
  for i: 0..<KEY.count { c[6 + i] = KEY[i]; }
  c[KEY.count + 6] = ')';
  describe(combo);
  show("P2PK", combo, SW_MAINNET, 0, 0, 0);
  show("P2PKH", combo, SW_MAINNET, 0, 0, 1);
  show("P2WPKH", combo, SW_MAINNET, 0, 0, 2);
  show("P2SH-P2WPKH", combo, SW_MAINNET, 0, 0, 3);
  show("a fifth", combo, SW_MAINNET, 0, 0, 4);

  // A 2-of-3 multisig: the key order doesn't matter in sortedmulti().
  print("Multisig\n");
  show("wsh(sortedmulti(2,...))", MULTI, SW_MAINNET, 0, 0, 0);
  show("on testnet", MULTI, SW_TESTNET, 0, 0, 0);
  show("raw()", "raw(6a0568656c6c6f)", SW_MAINNET, 0, 0, 0);

  // Refusals.
  print("Refusals\n");
  describe("pkh(L4rK1yDtCWekvXuE6oXD9jCYfFNV2cWRpVuPLBcCU2z8TrisoyY1)");
  describe("pkh(xpub6CatWdiZiodmUeTDp8LT5or8nmbKNcuyvz7WyksVFkKB4RHwCD3XyuvPEbvqAQY3rAPshWcMLoP2fMFMKHPJ4ZeZXYVUhLv1VMrjPC7PW6V/0h)");
  describe("wpkh(04a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd5b8dec5235a0fa8722476c7709c02559e3aa73aa03918ba2d492eea75abea235)");
  describe("sh(sh(pk(03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd)))");
  describe("multi(3,03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd)");
  describe("tr(a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd,pk(03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd))");
  describe("wpkh(xpub6CatWdiZiodmUeTDp8LT5or8nmbKNcuyvz7WyksVFkKB4RHwCD3XyuvPEbvqAQY3rAPshWcMLoP2fMFMKHPJ4ZeZXYVUhLv1VMrjPC7PW6V/<0;1>/<2;3>)");
  describe("pkh([73c5da0a/<0;1>]03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd)");
  describe("and_v(v:pk(03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd),older(144))");
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/descriptors.coax
```

```text
Checksums
raw(deadbeef)#89f8spxm
raw(deadbeef)#89f8spxm -> ranged false, 1 branches, 1 outputs
raw(deedbeef)#89f8spxm -> bad checksum
raw(deadbeef)#89f8spx -> checksum length
A BIP-84 wallet
wpkh([73c5da0a/84h/0h/0h]xpub6CatWdiZiodmUeTDp8LT5or8nmbKNcuyvz7WyksVFkKB4RHwCD3XyuvPEbvqAQY3rAPshWcMLoP2fMFMKHPJ4ZeZXYVUhLv1VMrjPC7PW6V/<0;1>/*)#qf45pmyh
wpkh([73c5da0a/84h/0h/0h]xpub6CatWdiZiodmUeTDp8LT5or8nmbKNcuyvz7WyksVFkKB4RHwCD3XyuvPEbvqAQY3rAPshWcMLoP2fMFMKHPJ4ZeZXYVUhLv1VMrjPC7PW6V/<0;1>/*)#qf45pmyh -> ranged true, 2 branches, 1 outputs
receive 0: bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu
receive 1: bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g
change 0: bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el
branch 2: bad branch
A BIP-86 wallet
tr([73c5da0a/86h/0h/0h]xpub6BgBgsespWvERF3LHQu6CnqdvfEvtMcQjYrcRzx53QJjSxarj2afYWcLteoGVky7D3UKDP9QyrLprQ3VCECoY49yfdDEHGCtMMj92pReUsQ/0/*) -> ranged true, 1 branches, 1 outputs
receive 0: bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr
combo()
combo(03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd) -> ranged false, 1 branches, 4 outputs
P2PK: 2103a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bdac
P2PKH: 1F3sAm6ZtwLAUnj7d38pGFxtP3RVEvtsbV
P2WPKH: bc1qngw83fg8dz0k749cg7k3emc7v98wy0c74dlrkd
P2SH-P2WPKH: 3DnW8JGpPViEZdpqat8qky1zc26EKbXnmM
a fifth: no output
Multisig
wsh(sortedmulti(2,...)): bc1qp6su5w35zq6eaznr73y6nkmj5a6kn260zy2ghkq7l2kfwhltsl9sx70nsh
on testnet: tb1qp6su5w35zq6eaznr73y6nkmj5a6kn260zy2ghkq7l2kfwhltsl9s3keu2c
raw(): 6a0568656c6c6f
Refusals
pkh(L4rK1yDtCWekvXuE6oXD9jCYfFNV2cWRpVuPLBcCU2z8TrisoyY1) -> private key
pkh(xpub6CatWdiZiodmUeTDp8LT5or8nmbKNcuyvz7WyksVFkKB4RHwCD3XyuvPEbvqAQY3rAPshWcMLoP2fMFMKHPJ4ZeZXYVUhLv1VMrjPC7PW6V/0h) -> needs private
wpkh(04a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd5b8dec5235a0fa8722476c7709c02559e3aa73aa03918ba2d492eea75abea235) -> uncompressed
sh(sh(pk(03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd))) -> context
multi(3,03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd) -> threshold
tr(a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd,pk(03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd)) -> taproot tree
wpkh(xpub6CatWdiZiodmUeTDp8LT5or8nmbKNcuyvz7WyksVFkKB4RHwCD3XyuvPEbvqAQY3rAPshWcMLoP2fMFMKHPJ4ZeZXYVUhLv1VMrjPC7PW6V/<0;1>/<2;3>) -> multipath
pkh([73c5da0a/<0;1>]03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd) -> multipath
and_v(v:pk(03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd),older(144)) -> unknown function
```

What the lines show:

- `raw(deadbeef)` gets BIP-380's checksum, one changed character makes
  it wrong, and a short checksum refuses for its length.
- The BIP-84 wallet is ranged with two branches. Receive 0 and 1 and
  change 0 are BIP-84's published addresses, the same ones chapter 33
  derived from the seed, here from the account `xpub` alone. Branch 2
  doesn't exist.
- The BIP-86 descriptor gives BIP-86's first address.
- `combo()` has four outputs, and a fifth refuses.
- The multisig's script is the same on testnet; only the address prefix
  changes.
- Each refusal has its own code: a WIF, a hardened step after an `xpub`,
  an uncompressed key in segwit, `sh()` inside `sh()`, a threshold above
  the key count, a script tree, two multipath steps in one key, a
  multipath step in an origin, and miniscript.

The book's checks run this program in debug and release builds and compare
the output with
[bitcoin/fixtures/descriptors.out](../fixtures/descriptors.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/descriptors.coax](../examples/checks/descriptors.coax),
which loads the example and
[descriptor_vectors.coax](../../coax/tests/bitcoin/descriptor_vectors.coax).
That file holds the vectors of BIP-380 to BIP-386 and BIP-389, which
[descriptor.py](../../coax/tests/bitcoin/descriptor.py) generates from the
pinned files and checks are current. A published descriptor with a private
key appears as published, where it must refuse with `DS_PRIVATE_KEY`; its
scripts are checked from the public twin that the Python implementation
derives: the `xpub` at the last hardened step, which is what Bitcoin Core
calls the normalized descriptor, or each child key in hex where a hardened
step follows the multipath step.

<!-- source: bitcoin/examples/checks/descriptors.coax check -->
```jai
// One script of a descriptor, as hex, or the refusal code.
dsc_hex :: (text: string, network: int, branch: int, index: int, nth: int, out: [] u8) -> string {
  script: [DS_SCRIPT_MAX] u8;
  n := descriptor_script_vartime(text, network, branch, index, nth, script[..]);
  if n < 0 { return "refused"; }
  r := hex_encode_checked(cast(string) script[0..<n], out, false);
  return cast(string) out[0..<r.written];
}
// One script of a descriptor, as its address.
dsc_address :: (text: string, network: int, branch: int, index: int, out: [] u8) -> string {
  script: [DS_SCRIPT_MAX] u8;
  n := descriptor_script_vartime(text, network, branch, index, 0, script[..]);
  if n < 0 { return "refused"; }
  a := address_encode_vartime(network, script[0..<n], out);
  if a < 0 { return "no address"; }
  return cast(string) out[0..<a];
}

// BIP-380's checksum vectors: the valid ones parse, the rest are refused.
#forall "BIP-380 checksums" i: 0..<DSV_SUMS_COUNT #steps 20000000 {
  v := DSV_SUMS[i];
  #assert descriptor_info_vartime(v.text, SW_MAINNET).result == v.result;
}
// BIP-380's key expressions, each inside pk().
#forall "BIP-380 key expressions" i: 0..<DSV_KEYS_COUNT #steps 200000000 {
  v := DSV_KEYS[i];
  #assert descriptor_info_vartime(v.text, SW_MAINNET).result == v.result;
}
// BIP-381 to BIP-386 and BIP-389: every published script, in groups of 15
// so that each check stays within the evaluator's limits.
dsc_script_ok :: (i: int) -> bool {
  v := DSV_SCRIPTS[i];
  hex: [2 * DS_SCRIPT_MAX] u8;
  return dsc_hex(v.text, SW_MAINNET, v.branch, v.index, v.nth, hex[..]) == v.script;
}
#forall "published scripts 0 to 14" i: 0..<15 #steps 4000000000 { #assert dsc_script_ok(i); }
#forall "published scripts 15 to 29" i: 15..<30 #steps 4000000000 { #assert dsc_script_ok(i); }
#forall "published scripts 30 to 44" i: 30..<45 #steps 4000000000 { #assert dsc_script_ok(i); }
#forall "published scripts 45 to 59" i: 45..<60 #steps 4000000000 { #assert dsc_script_ok(i); }
#forall "published scripts 60 to 74" i: 60..<75 #steps 4000000000 { #assert dsc_script_ok(i); }
#forall "published scripts 75 to 77" i: 75..<78 #steps 4000000000 { #assert dsc_script_ok(i); }
#forall "published scripts 78 to 80" i: 78..<81 #steps 4000000000 { #assert dsc_script_ok(i); }
#forall "published scripts 81 to 82" i: 81..<83 #steps 4000000000 { #assert dsc_script_ok(i); }
#forall "published scripts 83 to 83" i: 83..<84 #steps 4000000000 { #assert dsc_script_ok(i); }
#forall "published scripts 84 to 84" i: 84..<85 #steps 4000000000 { #assert dsc_script_ok(i); }
#forall "published scripts 85 to 85" i: 85..<DSV_SCRIPTS_COUNT #steps 4000000000 { #assert dsc_script_ok(i); }
// The descriptors those BIPs list as invalid.
#forall "published refusals" i: 0..<DSV_REFUSED_COUNT #steps 200000000 {
  v := DSV_REFUSED[i];
  #assert descriptor_info_vartime(v.text, SW_MAINNET).result == v.result;
  #assert v.result < 0;
}
// The example's wallets give BIP-84's and BIP-86's published addresses.
#test "the example's wallets" #steps 8000000000 {
  a: [90] u8;
  #assert dsc_address(WALLET84, SW_MAINNET, 0, 0, a[..]) == "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu";
  #assert dsc_address(WALLET84, SW_MAINNET, 0, 1, a[..]) == "bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g";
  #assert dsc_address(WALLET84, SW_MAINNET, 1, 0, a[..]) == "bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el";
  #assert dsc_address(WALLET86, SW_MAINNET, 0, 0, a[..]) == "bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr";
  #assert dsc_address(WALLET86, SW_MAINNET, 0, 1, a[..]) == "bc1p4qhjn9zdvkux4e44uhx8tc55attvtyu358kutcqkudyccelu0was9fqzwh";
  #assert dsc_address(WALLET86, SW_MAINNET, 1, 0, a[..]) == "refused";
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/descriptors.coax
```

`coax test` runs 262 checks, including the loaded files' own checks, and all pass. On 2026-11-03, on a loaded machine, they took about 7.5 minutes of test time; the published scripts, each derived from its `xpub`s in the evaluator, take most of it. They run in groups so that each check stays inside the evaluator's limits.

### The library's own checks

`descriptor.coax` is an `#authority` file. Its own checks cover BIP-380's
checksum vectors, a case error, a character outside the set and the place
of each of the 95 characters; `raw()` and `addr()`, the 10,000-byte limit,
unpaired parentheses, unknown names, bad networks and every branch, index,
output and capacity refusal; BIP-381's and BIP-382's public vectors; where
each kind of key refuses: hybrid, short, uncompressed in segwit, x-only
outside `tr()`, a WIF, an `xprv`, a key with a path that can't have one,
and every origin error; every context refusal; BIP-383's vectors, the
threshold and key-count refusals, and a P2SH script one key too long;
`combo()` with compressed and uncompressed keys; extended keys with paths,
`/*`, `<0;1>`, every hardened, step and multipath refusal, the wrong
network and a depth that would pass 255; and BIP-386's `tr()` vectors.
Each refusal is checked through both entry points, with nothing written
to the output.

```shell
coax/bin/coax test coax/lib/bitcoin/descriptor.coax
```

`coax test` runs 245 checks, including those of the files it loads, and all pass. On 2026-11-03, on a loaded machine, they took about 6 minutes of test time.

### A Python implementation

[descriptor.py](../../coax/tests/bitcoin/descriptor.py) compiles
`descriptor.coax` to C in checked and release builds and runs its three
exported procedures against a Python implementation that shares no code
with Coax: Python's `str` and `int` for the text, and the `bip32.py`,
`address.py`, `base58.py` and `points.py` references of chapters 32, 29,
26 and 17 for keys and scripts. Before the Coax code runs, the Python
implementation must reproduce:

- BIP-380's checksum vectors and key expressions, and BIP-380's own
  `descsum_create` and `descsum_check`, run from the pinned text;
- every script of BIP-381 to BIP-386, and every invalid descriptor they
  list;
- BIP-389's branches and refusals;
- the in-scope `Check`, `CheckUnparsable` and `CheckMultipath` calls of
  Bitcoin Core's `descriptor_tests.cpp`: their scripts for every branch
  and index listed, their refusals, and their normalized descriptors.
  Calls with miniscript, script trees or MuSig2 are out of scope and are
  counted, not run.

The cases are the published descriptors, 1,500 seeded descriptors of every
function with seeded keys, origins, paths, multipath steps, networks,
branches, indexes and outputs, some with a checksum and some with a wrong
one, and seeded edits of every published descriptor: a character replaced,
inserted or removed. Every result and script must match, nothing may be
written past the output, every refusal code must occur, and the checksum
of every case must match BIP-380's Python code.

```shell
python3 coax/tests/bitcoin/descriptor.py
```

On 2026-11-03, on a loaded machine, all 3,817 cases agree with Python in the checked and release native builds, in about 6 minutes. The reference reproduces 100 scripts from BIP-381 to BIP-386 and BIP-389, and 104 of the 202 calls in Core's `descriptor_test` case: 215 descriptors, with 96 sets of scripts; the other 98 calls are out of scope. The chapter is qualified on native CPU only; the Wasm builds were not run.

Mutation testing was not run for this chapter (owner speed order,
2026-11-03).

## What this chapter leaves out

- Private descriptors. A wallet that signs keeps its `xprv` in the
  descriptor or beside it; this chapter refuses both, and signing waits
  for PSBTs in [chapter 94](PLAN.md).
- `tr()` with a script tree. [Chapter 42](ch042-script-trees-tapleaves-and-tapbranches.md)
  builds script trees; descriptors for their tapscript leaves wait for
  miniscript.
- Miniscript (BIP-379) and the functions built on it, such as `and_v`,
  `older` and `thresh`: [chapter 98](PLAN.md).
- `musig()` (BIP-390), `rawtr()` and `rawnode()`.
- Deriving the descriptor of a wallet's coins on its own node:
  [chapter 99](PLAN.md).

## Summary

- A descriptor is a function of keys that gives the scriptPubKeys a
  wallet watches, with an optional eight-character checksum after `#`.
- The checksum is a BCH code over 95 characters, read as a place within
  one of three groups of 32 and a group number for every three
  characters.
- `pk`, `pkh`, `wpkh`, `sh`, `wsh`, `multi`, `sortedmulti`, `combo`,
  `addr`, `raw` and `tr` each give their script, each in the contexts it
  is allowed in.
- A key is hex or an `xpub` with a path, with an optional origin, an
  optional `/*` for a range and an optional `<0;1>` for several chains.
- This chapter's descriptors are public: private keys and hardened steps
  after an `xpub` refuse.
- The BIP-380 to BIP-386 and BIP-389 vectors and Bitcoin Core's tests
  come out as published, and the code agrees with a Python
  implementation.

## Dependencies

- [29. Addresses: P2PKH, P2SH, P2WPKH, P2WSH and P2TR](ch029-addresses.md)
- [32. Hierarchical deterministic keys](ch032-hierarchical-deterministic-keys.md)
- [33. Derivation paths](ch033-derivation-paths.md)

## Sources

- [BIP-380](https://github.com/bitcoin/bips/blob/master/bip-0380.mediawiki),
  for the language, key expressions and checksum, pinned in
  [coax/tests/vectors/upstream/bip380/](../../coax/tests/vectors/upstream/bip380/bip-0380.mediawiki).
- [BIP-381](https://github.com/bitcoin/bips/blob/master/bip-0381.mediawiki)
  to [BIP-386](https://github.com/bitcoin/bips/blob/master/bip-0386.mediawiki),
  for `pk`, `pkh`, `sh`, `wpkh`, `wsh`, `multi`, `sortedmulti`, `combo`,
  `addr`, `raw` and `tr`, and
  [BIP-389](https://github.com/bitcoin/bips/blob/master/bip-0389.mediawiki),
  for multipath steps, pinned beside BIP-380.
- Bitcoin Core's
  [descriptor_tests.cpp](../../coax/tests/vectors/upstream/bitcoin-core/descriptor_tests.cpp),
  pinned at revision `ddea9196493d3b8fd841a36a6c53efd85dac7a05`, and its
  `src/script/descriptor.cpp` at the same revision, for the rules the BIPs
  leave to the implementation: the multisig limits, hybrid keys and the
  normalized form.
- [BIP-67](https://github.com/bitcoin/bips/blob/master/bip-0067.mediawiki),
  for the key order of `sortedmulti()`.
