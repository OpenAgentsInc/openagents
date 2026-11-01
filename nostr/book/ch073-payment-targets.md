# 73. Payment targets

NIP-A3 lets a profile list where to pay it, one `payto` tag per target. A
client that reads the list decides which targets it offers as a way to pay.
Under this repository's Bitcoin-only rule, that decision is money: a wrong
address loses the payment, and a target on another network must never be
offered. This chapter reads the list, checks Bitcoin addresses down to
their checksums, and divides one payment between several recipients by
weight.

Part X, Every NIP. Result: NIP-A3 kind 10133 `payto` tags, with only the
Bitcoin and Lightning target types acted on under the repository rule.

## What this chapter teaches

- `["payto", type, address]` tags in a kind 10133 event, as NIP-A3 defines
  them after RFC 8905's `payto:` URIs.
- Which types are acted on: `bitcoin` and `lightning`. Every other type is
  kept and shown with the reason it is not offered: not Bitcoin, a Bitcoin
  type this reader does not build, or unknown.
- How a segwit address decodes: the human-readable part, Bech32 words, the
  BIP-350 rule that version 0 uses Bech32 and versions 1 to 16 use
  Bech32m, the witness program's padding and length rules.
- How a base58check address decodes: 25 bytes, a version byte and a
  double SHA-256 checksum.
- How to split a whole number of millisatoshis by weight so the shares sum
  exactly to the total, with a stated rule for the remainder.
- That the list is a statement by its signer: the client confirms an
  address before paying.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_tag(e, i)` | Reads a tag of any name: the name and its first two values (chapter 55) |
| [bech32-core.coax](../../coax/lib/nostr/bech32-core.coax) | `b32_prefix`, `b32_step`, `b32_digit` | The Bech32 checksum polymod and alphabet (chapter 15) |
| [sha256.coax](../../coax/lib/crypto/sha256.coax) | `sha256_hash_checked` | SHA-256 (chapter 4) |
| [nipa3.coax](../../coax/lib/nostr/nipa3.coax) | `a3_list(e)` | A kind 10133 event's `payto` targets, each with a status |
| [nipa3.coax](../../coax/lib/nostr/nipa3.coax) | `a3_target(type, address)` | One target: its type sorted, its address checked |
| [nipa3.coax](../../coax/lib/nostr/nipa3.coax) | `a3_segwit(s)`, `a3_base58(s)` | A Bitcoin address: version, program, form, network |
| [nipa3.coax](../../coax/lib/nostr/nipa3.coax) | `a3_pay_uri(t, out)` | The `bitcoin:` or `lightning:` link for a target that may be offered |
| [nipa3.coax](../../coax/lib/nostr/nipa3.coax) | `a3_weights(texts, out)`, `a3_split(total, weights, out)` | NIP-57 zap split weights, and the shares of a total |

`nipa3.coax` is new with this chapter. It is an `#authority` file: it
decides where money may go. It does not fetch anything, resolve a
Lightning address's domain, or verify a signature. The caller verifies the
event (chapter 21) and passes it in.

Chapter 15's `bech32-core.coax` reads Bech32 only. This chapter does not
change it. `a3_segwit` calls its polymod (`b32_prefix`, `b32_step`) and
compares the residue with both constants itself, so the library gains
Bech32m without a new code path in chapter 15.

## The record

`a3_list` returns one `A3Target` per `payto` tag, in tag order. `status` is
`A3_OK` only for a target that may be offered. A refused target keeps its
type and address as written, so that a client can still show it.

<!-- source: coax/lib/nostr/nipa3.coax record -->
```jai
A3Target :: struct {
  status:int=A3_SHAPE; at:int;             // at: its tag's index
  type:N32Text; address:N32Text;           // as written
  form:int;                                // A3_P2PKH ... A3_LN_ADDRESS
  version:int; program:[40]u8; n:int;      // the witness version and program, or the base58 hash
}
A3List :: struct {status:int=A3_EVENT; at:int=-1; count:int; payable:int; t:[A3_MAX]A3Target;}
```

`version`, `program` and `n` hold the witness version and program of a
segwit address, or the 20-byte hash of a base58 address.

## Types

NIP-A3 lists 16 commonly used types. The reader sorts them into three
groups:

<!-- source: coax/lib/nostr/nipa3.coax types -->
```jai
// The types NIP-A3 lists as commonly used, sorted by what this reader does.
A3_ACTED :: [2] string.["bitcoin", "lightning"];
A3_BITCOIN_UNBUILT :: [2] string.["bip352", "bip353"];
A3_NOT_BITCOIN_TYPES :: [12] string.["bitcoincash", "cashme", "ethereum", "litecoin",
  "monero", "nano", "paypal", "revolut", "solana", "tron", "venmo", "zcash"];
```

| Status | Meaning | Offered |
| --- | --- | --- |
| `A3_OK` | A mainnet Bitcoin address of a defined output type, or a Lightning address | yes |
| `A3_NOT_BITCOIN` | A type NIP-A3 lists that is another network or a fiat service | no |
| `A3_NOT_BUILT` | `bip352` (silent payments) or `bip353` (DNS payment names): Bitcoin, but not read here | no |
| `A3_UNKNOWN` | A type nobody listed, such as `unknowntype` or `spark` | no |
| `A3_SHAPE` | Fewer than three values, an empty address, or a type with upper case or other characters | no |
| `A3_CHECKSUM`, `A3_ADDRESS`, `A3_LENGTH` | A Bitcoin address with a bad checksum, bad syntax, or a witness program of the wrong length | no |
| `A3_NETWORK` | A valid testnet or regtest address | no |
| `A3_FUTURE` | A valid segwit address of a version with no output type yet | no |
| `A3_LIMIT` | A type or address longer than 256 bytes | no |

An unknown type is not offered even when its address looks like Bitcoin.
Spark and Taproot Assets have no NIP-A3 type; a `spark` target reads as
unknown until a type is defined and a reader is built for it.

<!-- source: coax/lib/nostr/nipa3.coax target -->
```jai
// One payto target: its type sorted, and a bitcoin or lightning address
// checked. Only status A3_OK may be offered as a way to pay.
a3_target :: (type:N32Text,address:N32Text)->A3Target {
  t:A3Target;t.type=type;t.address=address;
  ty:=cast(string)t.type.b[0..<t.type.n];ad:=cast(string)t.address.b[0..<t.address.n];
  if !a3_type_ok(ty) || ad.count==0 {return t;}
  nb:=A3_NOT_BITCOIN_TYPES;ub:=A3_BITCOIN_UNBUILT;
  if a3_is_in(nb[..],ty) {return a3_fail(t,A3_NOT_BITCOIN);}
  if a3_is_in(ub[..],ty) {return a3_fail(t,A3_NOT_BUILT);}
  if ty=="lightning" {return a3_lightning(t,ad);}
  if ty!="bitcoin" {return a3_fail(t,A3_UNKNOWN);}
  a:A3Target;
  if a3_starts_ci(ad,"bc1") || a3_starts_ci(ad,"tb1") || a3_starts_ci(ad,"bcrt1") {a=a3_segwit(ad);}
  else {a=a3_base58(ad);}
  a.type=t.type;a.address=t.address;return a;
}
```

`a3_list` reads every tag with `n32_tag` and keeps the `payto` ones. One
bad target does not refuse the list; only an unreadable event, a kind other
than 10133 or more than 16 targets does.

<!-- source: coax/lib/nostr/nipa3.coax list -->
```jai
// The payto tags of a kind 10133 event, in tag order. A bad target is kept
// with its status; only an unreadable event, the wrong kind or more than
// A3_MAX targets refuses the list.
a3_list :: (e:NostrEvent)->A3List {
  r:A3List;if e.error!=NE_OK {return r;}
  if e.kind!=A3_LIST_KIND {r.status=A3_KIND;return r;}
  i:=0;
  while true {
    g:=n32_tag(e,i);
    if g.error==NT_INDEX {break;}
    if g.error!=NT_OK && g.error!=NT_LIMIT {r.at=i;return r;}
    if n32_is(g.v[0],"payto") {
      if r.count==A3_MAX {r.status=A3_LIMIT;r.at=i;return r;}
      t:A3Target;
      if g.error==NT_LIMIT {t.status=A3_LIMIT;}
      else if g.values>=3 {t=a3_target(g.v[1],g.v[2]);}
      t.at=i;r.t[r.count]=t;r.count+=1;
      if t.status==A3_OK {r.payable+=1;}
    }
    i+=1;
  }
  r.status=A3_OK;return r;
}
```

## Segwit addresses

A segwit address is Bech32 text (chapter 15): a human-readable part, the
separator `1`, data words of 5 bits each, and six checksum words. For a
Bitcoin address:

1. The human-readable part is `bc` on mainnet, `tb` on testnet and `bcrt`
   on regtest, in either case but not mixed.
2. The first data word is the witness version, 0 to 16.
3. The other data words, read as one bit string, are the witness program,
   8 bits per byte. At most 4 bits are left over, and they must be zero.
4. The program is 2 to 40 bytes. Version 0 allows only 20 bytes (P2WPKH)
   or 32 bytes (P2WSH).
5. The checksum polymod over the whole string leaves 1 for Bech32 and
   `0x2bc830a3` for Bech32m (BIP-350). Version 0 must use Bech32 and
   versions 1 to 16 must use Bech32m.

Version 1 with 32 bytes is Taproot (P2TR). Other versions and lengths are
valid addresses that no soft fork has given meaning yet. BIP-350 asks
wallets to send to them, so that future output types work without a
software update. This reader decodes them and refuses them with
`A3_FUTURE`: an output of an undefined version can be spent by anyone
until a rule protects it, and nobody receiving a payment should publish
one.

### Worked example: BC1SW50QGDZ25J

BIP-350's shortest valid address, decoded by hand:

1. In lower case, `bc1sw50qgdz25j`. The last `1` is at position 2, so the
   human-readable part is `bc` (mainnet) and the data is `sw50qgdz25j`.
2. Each character's position in `qpzry9x8gf2tvdw0s3jn54khce6mua7l`:
   `s` 16, `w` 14, `5` 20, `0` 15, `q` 0, `g` 8, `d` 13, `z` 2, `2` 10,
   `5` 20, `j` 18.
3. The last six, 8 13 2 10 20 18, are the checksum. The data words are
   16 14 20 15 0.
4. The version is 16.
5. The program words 14 20 15 0 in 5 bits each are
   `01110 10100 01111 00000`: 20 bits. The first 16 are `01110101` = `0x75`
   and `00011110` = `0x1e`. The last 4 bits, `0000`, are padding: 4 bits
   or fewer, all zero.
6. The program is 2 bytes, `751e`, which is within 2 to 40. Version 16 has
   no 20-or-32 rule.
7. Version 16 needs Bech32m, so the polymod residue must be `0x2bc830a3`,
   which is 734539939. The example computes it.

The address is valid and pays the script `60 02 75 1e`: `OP_16`, a 2-byte
push, the program. That is BIP-350's stated scriptPubKey `6002751e`. Version
16 has no output type, so the status is `A3_FUTURE`, and the address is
never offered.

The BIP-350 mainnet Taproot address
`bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0`
decodes the same way to version 1 and the 32 bytes `79be667e...16f81798`,
the x coordinate of the generator `G` (chapter 9). That one is offered.

<!-- source: coax/lib/nostr/nipa3.coax segwit -->
```jai
// A segwit address (BIP-173, BIP-350). The checksum residue is 1 for
// Bech32 and A3_BECH32M for Bech32m; version 0 needs the first, versions 1
// to 16 the second. Program, version and form are filled in before the
// network is checked, so a testnet address reports what it holds.
a3_segwit :: (s:string)->A3Target {
  t:A3Target;t.status=A3_ADDRESS;
  if s.count>90 {return t;}
  sep:=s.count;upper:=false;lower:=false;   // no 1: the whole string is the hrp
  for i:0..<s.count {
    c:=s[i];
    if c=='1' {sep=i;}
    if b32_lower(c)!=c {upper=true;}        // A to Z
    if b32_lower(c^32)==c {lower=true;}     // a to z: its upper case lowers back to it
  }
  hrp:=s[0..<sep];net:=A3_OK;
  if !a3_is_ci(hrp,"bc") {
    if !a3_is_ci(hrp,"tb") && !a3_is_ci(hrp,"bcrt") {return t;}
    net=A3_NETWORK;
  }
  if upper && lower {return t;}
  words:[90]u8;n:=s.count-sep-1;
  if n<7 {return t;}                        // a version word and six checksum words
  state:=b32_prefix(hrp);
  for i:0..<n {
    d:=b32_digit(b32_lower(s[sep+1+i]));
    if d<0 {return t;}
    words[i]=cast(u8)d;state=b32_step(state,words[i]);
  }
  if state!=1 && state!=A3_BECH32M {return a3_fail(t,A3_CHECKSUM);}
  if words[0]>16 {return t;}
  t.version=cast(int)words[0];
  // Five bits per word into bytes; at most 4 bits of zero padding remain.
  acc:u32;bits:=0;
  for i:1..<n-6 {
    acc=((acc<<5)|cast(u32)words[i])&0xfff;bits+=5;
    if bits>=8 {bits-=8;if t.n==40 {return a3_fail(t,A3_LENGTH);}t.program[t.n]=cast(u8)((acc>>bits)&255);t.n+=1;}
  }
  if bits>4 || (acc&cast(u32)((1<<bits)-1))!=0 {return t;}
  if t.n<2 || (t.version==0 && t.n!=20 && t.n!=32) {return a3_fail(t,A3_LENGTH);}
  if (t.version==0)!=(state==1) {return a3_fail(t,A3_CHECKSUM);}
  t.form=ifx t.version==0 && t.n==20 then A3_P2WPKH else ifx t.version==0 then A3_P2WSH
    else ifx t.version==1 && t.n==32 then A3_P2TR else A3_SEGWIT_FUTURE;
  if net!=A3_OK {return a3_fail(t,net);}
  return a3_fail(t,ifx t.form==A3_SEGWIT_FUTURE then A3_FUTURE else A3_OK);
}
```

The checks run in this order so that each refusal names its cause: the
90-character limit, the human-readable part (`A3_ADDRESS`), mixed case,
at least seven data words, the characters, the checksum against either constant (`A3_CHECKSUM`), the version, the padding,
the program length (`A3_LENGTH`), the version's constant (`A3_CHECKSUM`),
and last the network (`A3_NETWORK`). A testnet address reports its program
and form before it is refused.

## Base58check addresses

The older addresses start with `1` (P2PKH) or `3` (P2SH). Each is a
25-byte number in base 58: a version byte, a 20-byte hash and a 4-byte
checksum, the first four bytes of `SHA-256(SHA-256(version and hash))`.
Each leading zero byte is written as a leading `1`. Version `0x00` is
P2PKH and `0x05` is P2SH on mainnet; `0x6f` and `0xc4` are testnet.

<!-- source: coax/lib/nostr/nipa3.coax base58 -->
```jai
// A base58check address: 25 bytes, version, 20-byte hash and checksum,
// with one leading "1" per leading zero byte.
A3_BASE58 :: "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
a3_base58 :: (s:string)->A3Target {
  t:A3Target;t.status=A3_ADDRESS;
  b:[25]u8;
  for c:s {
    d:=-1;for k:0..<58 {if A3_BASE58[k]==c {d=k;}}
    if d<0 {return t;}
    carry:=d;
    for k:0..<25 {carry+=cast(int)b[24-k]*58;b[24-k]=cast(u8)(carry&255);carry=carry>>8;}
    if carry!=0 {return t;}                  // over 25 bytes
  }
  ones:=0;while ones<s.count && s[ones]=='1' {ones+=1;}
  zeros:=0;while zeros<25 && b[zeros]==0 {zeros+=1;}
  if ones!=zeros {return t;}
  h:[32]u8;h2:[32]u8;
  if sha256_hash_checked(b[0..<21],h[..],false)!=SHA_OK {return t;}
  if sha256_hash_checked(h[..],h2[..],false)!=SHA_OK {return t;}
  for k:0..<4 {if h2[k]!=b[21+k] {return a3_fail(t,A3_CHECKSUM);}}
  for k:1..<21 {t.program[t.n]=b[k];t.n+=1;}
  v:=b[0];
  if v==0x00 || v==0x6f {t.form=A3_P2PKH;}
  else if v==0x05 || v==0xc4 {t.form=A3_P2SH;}
  else {return t;}
  return a3_fail(t,ifx v==0x00 || v==0x05 then A3_OK else A3_NETWORK);
}
```

`1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2` decodes to
`00 77bff20c...004e839a f415766b`: version 0, the hash, and a checksum
that matches. Changing its last character to `3` changes the checksum to
`f415766c`, and the address refuses with `A3_CHECKSUM`.

## Lightning addresses

A `lightning` target is a Lightning address (LUD-16): `name@domain`. The
name uses `a` to `z`, `0` to `9`, `-`, `_` and `.`; the domain is two or
more labels of `a` to `z`, `0` to `9` and `-`, separated by dots. Upper
case is refused, so `Alice@example.com` is not offered. Paying one means
fetching `https://domain/.well-known/lnurlp/name` and asking that server
for an invoice; that is chapter 71's zap flow and is not done here.

## Offering a target

A target with `A3_OK` gets a link: `bitcoin:` and the address (segwit in
lower case), or `lightning:` and the address. Every other target gets
nothing, so a client that builds its buttons from `a3_pay_uri` cannot offer
a non-Bitcoin target. NIP-A3 suggests a `payto://type/address` link for
other types; a client may show that text, but not as a way to pay.

<!-- source: coax/lib/nostr/nipa3.coax uri -->
```jai
// The URI a client may offer as a payment link: bitcoin:<address> (bech32
// in lower case) or lightning:<address>. -1 when the target must not be
// offered or out is too small; nothing is written then.
a3_pay_uri :: (t:A3Target,out:[]u8)->int {
  if t.status!=A3_OK {return -1;}
  scheme:=ifx t.form==A3_LN_ADDRESS then "lightning:" else "bitcoin:";
  c:=t;ad:=cast(string)c.address.b[0..<c.address.n];
  if scheme.count+ad.count>out.count {return -1;}
  for i:0..<scheme.count {out[i]=scheme[i];}
  lower:=t.form>A3_P2SH;                   // segwit; a Lightning address is lower case already
  for i:0..<ad.count {out[scheme.count+i]=ifx lower then b32_lower(ad[i]) else ad[i];}
  return scheme.count+ad.count;
}
```

## Splitting a payment

NIP-57 appendix G lets an event name several zap recipients, each with an
optional weight: `["zap", pubkey, relay, weight]`. A client divides the
amount in proportion to the weights. Without any weight, every recipient
gets an equal share; when some tags have a weight, a tag without one gets
nothing. Chapter 71 reads `zap` tags. This chapter does the arithmetic:
`a3_weights` reads the weight strings and `a3_split` divides the amount.

<!-- source: coax/lib/nostr/nipa3.coax weights -->
```jai
// NIP-57 appendix G's weights, one string per zap tag ("" when the tag has
// no weight). No weights at all: equal shares. Some weights: a tag without
// one gets 0. A weight is a whole number 0 to A3_MAX_WEIGHT written without
// a sign or leading zeros.
a3_weights :: (texts:[]string,out:[]int)->int {
  if texts.count<1 || texts.count>A3_MAX || out.count<texts.count {return A3_LIMIT;}
  any:=false;for s:texts {if s.count>0 {any=true;}}
  positive:=false;
  for i:0..<texts.count {
    s:=texts[i];w:=0;
    if !any {w=1;}
    else if s.count>0 {
      if s[0]=='0' && s.count>1 {return A3_WEIGHT;}
      for c:s {
        if c<'0' || c>'9' {return A3_WEIGHT;}
        w=w*10+cast(int)(c-'0');
        if w>A3_MAX_WEIGHT {return A3_WEIGHT;}
      }
    }
    out[i]=w;if w>0 {positive=true;}
  }
  if !positive {return A3_ZERO;}
  return A3_OK;
}
```

A Lightning payment is a whole number of millisatoshis, so the shares must
be whole numbers that add up to the total. The rule:

1. Each recipient gets `total × w / W` rounded down, where `W` is the sum
   of the weights.
2. The millisatoshis left over, fewer than the number of recipients, go
   one each to the recipients with the largest remainders
   `total × w mod W`, the earlier tag first on a tie.

A recipient with weight 0 has remainder 0 and never receives a leftover
millisatoshi. The code computes `total × w` as `q × w + r × w / W`, with
`q` and `r` the quotient and remainder of `total / W`, so no product
exceeds 64 bits for any total up to 21 million bitcoin.

<!-- source: coax/lib/nostr/nipa3.coax split -->
```jai
// total millisatoshis shared by weight. Each share is the exact share
// total·w/W rounded down; the msat left over (fewer than the number of
// recipients) go one each to the largest remainders total·w mod W, the
// earlier tag first on a tie. Computed as q·w + r·w/W with q, r = total
// divided by W, so nothing overflows 64 bits.
a3_split :: (total:int,weights:[]int,out:[]int)->int {
  n:=weights.count;
  if n<1 || n>A3_MAX || out.count<n {return A3_LIMIT;}
  if total<1 || total>A3_MAX_MSAT {return A3_AMOUNT;}
  sum:=0;
  for w:weights {if w<0 || w>A3_MAX_WEIGHT {return A3_WEIGHT;}sum+=w;}
  if sum==0 {return A3_ZERO;}
  q:=total/sum;r:=total%sum;
  rest:[A3_MAX]int;given:[A3_MAX]bool;left:=total;
  for i:0..<n {
    out[i]=q*weights[i]+r*weights[i]/sum;rest[i]=r*weights[i]%sum;left-=out[i];
  }
  while left>0 {
    best:=-1;
    for i:0..<n {if !given[i] && (best<0 || rest[i]>rest[best]) {best=i;}}
    given[best]=true;out[best]+=1;left-=1;
  }
  return A3_OK;
}
```

### Worked example: 1000 sats over 1:2:3

The total is 1,000,000 msat and `W = 6`, so `q = 166,666` and `r = 4`.

| Weight | `q × w` | `r × w / W` | Share rounded down | Remainder `r × w mod W` |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 166,666 | 4 / 6 = 0 | 166,666 | 4 |
| 2 | 333,332 | 8 / 6 = 1 | 333,333 | 2 |
| 3 | 499,998 | 12 / 6 = 2 | 500,000 | 0 |

The shares sum to 999,999, so 1 msat is left. The largest remainder is 4,
the first recipient's, which gets it. The shares are 166,667, 333,333 and
500,000 msat, which sum to 1,000,000. NIP-57's own example, weights 1, 1
and 2, gives 25%, 25% and 50%: 5,250, 5,250 and 10,500 msat of 21 sats.

A weight is a whole number from 0 to 1,000,000, written without a sign or
leading zeros. `-1` and `1.5` refuse with `A3_WEIGHT`, and weights that are
all zero refuse with `A3_ZERO`, because there is no one to pay. A total of
0 or less, or over 21 million bitcoin, refuses with `A3_AMOUNT`.

## The example

The example is
[nostr/examples/payment-targets.coax](../examples/payment-targets.coax). It
decodes `BC1SW50QGDZ25J` word by word, reads NIP-A3's example event, an
event with every kind of Bitcoin target and some that are kept but not
offered, and an event of refusals, then computes six splits and four
refused ones. Events are built from their tags with `n32_event`; a client
reads them from relays and verifies them first.

<!-- source: nostr/examples/payment-targets.coax example -->
```jai
#load "../../coax/lib/nostr/nipa3.coax";
#import "Basic";

STATUS :: [16] string.["ok", "event", "not kind 10133", "bad payto tag", "not Bitcoin",
  "unknown type", "Bitcoin type not built", "bad checksum", "bad address", "not mainnet",
  "segwit version not yet defined", "limit", "bad weight", "all weights zero", "bad amount",
  "bad witness program length"];
FORM :: [8] string.["none", "P2PKH", "P2SH", "P2WPKH", "P2WSH", "P2TR", "future segwit", "Lightning address"];

say :: (t: N32Text) { own := t; print("%", cast(string) own.b[0..<own.n]); }
show_program :: (t: A3Target) {
  d := "0123456789abcdef"; c := t; text: [80] u8;
  for i: 0..<c.n { text[2 * i] = d[c.program[i] >> 4]; text[2 * i + 1] = d[c.program[i] & 15]; }
  print("%", cast(string) text[0..<2 * c.n]);
}
// One target: its type and address, then what may be done with it.
show :: (t: A3Target) {
  print("  "); say(t.type); print(" "); say(t.address); print("\n    ");
  uri: [300] u8; n := a3_pay_uri(t, uri[..]);
  if n > 0 { print("offer %", cast(string) uri[0..<n]); }
  else { print("not offered: %", STATUS[t.status]); }
  if t.form != A3_NONE {
    print(" (%", FORM[t.form]);
    if t.form != A3_LN_ADDRESS { print(", "); show_program(t); }
    print(")");
  }
  print("\n");
}
show_list :: (label: string, e: NostrEvent) {
  l := a3_list(e);
  print("%: %, % targets, % offered\n", label, STATUS[l.status], l.count, l.payable);
  for i: 0..<l.count { show(l.t[i]); }
}
show_split :: (label: string, total: int, weights: [] string) {
  w: [A3_MAX] int; s: [A3_MAX] int;
  status := a3_weights(weights, w[..]);
  if status == A3_OK { status = a3_split(total, w[0..<weights.count], s[..]); }
  print("%: %", label, STATUS[status]);
  if status == A3_OK { print(" ->"); for i: 0..<weights.count { print(" %", s[i]); } }
  print("\n");
}

// book:begin trace
// The shortest BIP-350 address, decoded word by word.
trace :: () {
  s := "BC1SW50QGDZ25J";
  print("trace %\n  words after the separator:", s);
  for i: 3..<s.count { print(" %", b32_digit(b32_lower(s[i]))); }
  state := b32_prefix("bc");
  for i: 3..<s.count { state = b32_step(state, cast(u8) b32_digit(b32_lower(s[i]))); }
  print("\n  checksum residue: % (Bech32m is %)\n", state, A3_BECH32M);
  t := a3_try("bitcoin", s);
  print("  version %, program ", t.version); show_program(t);
  print(", %\n", STATUS[t.status]);
}
// book:end trace

main :: () {
  trace();

  // NIP-A3's example event, as the pinned text gives it.
  show_list("NIP-A3 example", n32_event(10133,
    "[[\"payto\",\"bitcoin\",\"bc1qxq66e0t8d7ugdecwnmv58e90tpry23nc84pg9k\"],[\"payto\",\"nano\",\"nano_1dctqbmqxfppo9pswbm6kg9d4s4mbraqn8i4m7ob9gnzz91aurmuho48jx3c\"],[\"payto\",\"unknowntype\",\"l7tbta5b9xze6ckkfc99uohzxd009b0r\"]]"));

  // Every kind of Bitcoin address, and the targets that are kept but never offered.
  show_list("Bitcoin targets", n32_event(10133,
    "[[\"payto\",\"bitcoin\",\"BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4\"],[\"payto\",\"bitcoin\",\"bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0\"],[\"payto\",\"bitcoin\",\"1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2\"],[\"payto\",\"bitcoin\",\"3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy\"],[\"payto\",\"lightning\",\"alice@example.com\"],[\"payto\",\"bitcoin\",\"tb1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3q0sl5k7\"],[\"payto\",\"bitcoin\",\"bc1zw508d6qejxtdg4y5r3zarvaryvaxxpcs\"],[\"payto\",\"bip353\",\"alice@example.com\"]]"));

  // Refusals: one changed character, a 16-byte v0 program, Bech32 on a v1
  // address, a non-Bitcoin type and a malformed Lightning address.
  show_list("refusals", n32_event(10133,
    "[[\"payto\",\"bitcoin\",\"BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T5\"],[\"payto\",\"bitcoin\",\"BC1QR508D6QEJXTDG4Y5R3ZARVARYV98GJ9P\"],[\"payto\",\"bitcoin\",\"bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqh2y7hd\"],[\"payto\",\"bitcoin\",\"1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN3\"],[\"payto\",\"ethereum\",\"0xd8da6bf26964af9d7eed9e03e53415d37aa96045\"],[\"payto\",\"lightning\",\"Alice@example.com\"]]"));
  show_list("a kind 1 event", n32_event(1, "[[\"payto\",\"bitcoin\",\"1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2\"]]"));

  // Zap splits in millisatoshis.
  w123 := [3] string.["1", "2", "3"];      show_split("1000 sats over 1:2:3", 1000000, w123[..]);
  w112 := [3] string.["1", "1", "2"];      show_split("21 sats over NIP-57's 1:1:2", 21000, w112[..]);
  none := [3] string.["", "", ""];         show_split("7 msat, no weights", 7, none[..]);
  some := [3] string.["", "1", "1"];       show_split("10 msat, one tag without a weight", 10, some[..]);
  neg := [3] string.["1", "-1", "1"];      show_split("weight -1", 1000, neg[..]);
  half := [3] string.["1", "1.5", "1"];    show_split("weight 1.5", 1000, half[..]);
  zero := [3] string.["0", "0", "0"];      show_split("weights 0:0:0", 1000, zero[..]);
  show_split("0 msat", 0, w123[..]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/payment-targets.coax
```

The output matches the values worked out by hand: the words
`16 14 20 15 0` and the checksum words, the residue 734539939
(`0x2bc830a3`), the program `751e`, and the shares 166,667, 333,333 and
500,000:

```text
trace BC1SW50QGDZ25J
  words after the separator: 16 14 20 15 0 8 13 2 10 20 18
  checksum residue: 734539939 (Bech32m is 734539939)
  version 16, program 751e, segwit version not yet defined
NIP-A3 example: ok, 3 targets, 1 offered
  bitcoin bc1qxq66e0t8d7ugdecwnmv58e90tpry23nc84pg9k
    offer bitcoin:bc1qxq66e0t8d7ugdecwnmv58e90tpry23nc84pg9k (P2WPKH, 3035acbd676fb886e70e9ed943e4af5846454678)
  nano nano_1dctqbmqxfppo9pswbm6kg9d4s4mbraqn8i4m7ob9gnzz91aurmuho48jx3c
    not offered: not Bitcoin
  unknowntype l7tbta5b9xze6ckkfc99uohzxd009b0r
    not offered: unknown type
Bitcoin targets: ok, 8 targets, 5 offered
  bitcoin BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4
    offer bitcoin:bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4 (P2WPKH, 751e76e8199196d454941c45d1b3a323f1433bd6)
  bitcoin bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0
    offer bitcoin:bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0 (P2TR, 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798)
  bitcoin 1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2
    offer bitcoin:1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2 (P2PKH, 77bff20c60e522dfaa3350c39b030a5d004e839a)
  bitcoin 3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy
    offer bitcoin:3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy (P2SH, b472a266d0bd89c13706a4132ccfb16f7c3b9fcb)
  lightning alice@example.com
    offer lightning:alice@example.com (Lightning address)
  bitcoin tb1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3q0sl5k7
    not offered: not mainnet (P2WSH, 1863143c14c5166804bd19203356da136c985678cd4d27a1b8c6329604903262)
  bitcoin bc1zw508d6qejxtdg4y5r3zarvaryvaxxpcs
    not offered: segwit version not yet defined (future segwit, 751e76e8199196d454941c45d1b3a323)
  bip353 alice@example.com
    not offered: Bitcoin type not built
refusals: ok, 6 targets, 0 offered
  bitcoin BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T5
    not offered: bad checksum
  bitcoin BC1QR508D6QEJXTDG4Y5R3ZARVARYV98GJ9P
    not offered: bad witness program length
  bitcoin bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqh2y7hd
    not offered: bad checksum
  bitcoin 1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN3
    not offered: bad checksum
  ethereum 0xd8da6bf26964af9d7eed9e03e53415d37aa96045
    not offered: not Bitcoin
  lightning Alice@example.com
    not offered: bad address
a kind 1 event: not kind 10133, 0 targets, 0 offered
1000 sats over 1:2:3: ok -> 166667 333333 500000
21 sats over NIP-57's 1:1:2: ok -> 5250 5250 10500
7 msat, no weights: ok -> 3 2 2
10 msat, one tag without a weight: ok -> 0 5 5
weight -1: bad weight
weight 1.5: bad weight
weights 0:0:0: all weights zero
0 msat: bad amount
```

The refusals:

- `...V8F3T5` is BIP-173's mainnet P2WPKH address with its last character
  changed: `A3_CHECKSUM`.
- `BC1QR508D6QEJXTDG4Y5R3ZARVARYV98GJ9P` has a valid checksum and a
  16-byte version 0 program: `A3_LENGTH`.
- `bc1p0xlx...vqh2y7hd` is the Taproot address of `G` with a Bech32
  checksum instead of Bech32m: `A3_CHECKSUM`.
- `...xJaNVN3` is the P2PKH address with its last character changed:
  `A3_CHECKSUM`.
- `ethereum` is not Bitcoin, and `Alice@example.com` has an upper-case
  name. Neither is offered.
- A `payto` tag on a kind 1 event is not a payment target list.
- The weights `-1`, `1.5` and `0:0:0`, and a total of 0 msat, refuse.

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/payment-targets.out](../fixtures/payment-targets.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/payment-targets.coax](../examples/checks/payment-targets.coax),
which loads the example. They compare the code with values it did not
produce:

- BIP-350's 8 valid segwit addresses against the scriptPubKey each one
  pays: the version from its first byte, the length from its second and the
  program from the rest;
- BIP-350's 15 invalid segwit addresses, each refused for the reason
  BIP-350 states;
- BIP-350's 7 generic Bech32m strings, whose residue through chapter 15's
  polymod is `0x2bc830a3`;
- BIP-173's segwit vectors from the pinned
  `coax/tests/vectors/upstream/bip173/bip-0173.mediawiki`: its v0
  addresses decode, and its v1 to v16 addresses, which carry Bech32
  checksums, refuse as BIP-350 requires;
- four base58 addresses against a Python script's decoding (arbitrary-
  precision integers and `hashlib`);
- six splits against Python's `total × w // W` with the remainder rule,
  computed without the code's `q × w + r × w / W` form, and NIP-57's 1:1:2
  example.

<!-- source: nostr/examples/checks/payment-targets.coax check -->
```jai
// BIP-350's valid segwit addresses and the scriptPubKey each one pays,
// copied from bitcoin/bips at 24e96e870fffaa257b465ce1f0370c14aac588e8
// (bip-0350.mediawiki, SHA-256 63634b06...a7a0dec). The scriptPubKey's
// first byte is the version (0x00, or 0x50 + v for v 1 to 16), its second
// the program's length, then the program.
BIP350_VALID :: [16] string.[
  "BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4", "0014751e76e8199196d454941c45d1b3a323f1433bd6",
  "tb1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3q0sl5k7", "00201863143c14c5166804bd19203356da136c985678cd4d27a1b8c6329604903262",
  "bc1pw508d6qejxtdg4y5r3zarvary0c5xw7kw508d6qejxtdg4y5r3zarvary0c5xw7kt5nd6y", "5128751e76e8199196d454941c45d1b3a323f1433bd6751e76e8199196d454941c45d1b3a323f1433bd6",
  "BC1SW50QGDZ25J", "6002751e",
  "bc1zw508d6qejxtdg4y5r3zarvaryvaxxpcs", "5210751e76e8199196d454941c45d1b3a323",
  "tb1qqqqqp399et2xygdj5xreqhjjvcmzhxw4aywxecjdzew6hylgvsesrxh6hy", "0020000000c4a5cad46221b2a187905e5266362b99d5e91c6ce24d165dab93e86433",
  "tb1pqqqqp399et2xygdj5xreqhjjvcmzhxw4aywxecjdzew6hylgvsesf3hn0c", "5120000000c4a5cad46221b2a187905e5266362b99d5e91c6ce24d165dab93e86433",
  "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0", "512079be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"];
// What this reader decides for each: mainnet v0 and v1 Taproot are offered,
// testnet refuses for its network, other versions for having no output type.
BIP350_STATUS :: [8] int.[A3_OK, A3_NETWORK, A3_FUTURE, A3_FUTURE, A3_FUTURE, A3_NETWORK, A3_NETWORK, A3_OK];
spk_digit :: (c: u8) -> int { if c <= '9' { return cast(int)(c - '0'); } return cast(int)(c - 'a') + 10; }
#forall "BIP-350's valid addresses give their scriptPubKey" i: 0..<8 {
  t := a3_try("bitcoin", BIP350_VALID[2 * i]);
  spk := BIP350_VALID[2 * i + 1];
  first := spk_digit(spk[0]) * 16 + spk_digit(spk[1]);
  #assert t.status == BIP350_STATUS[i];
  #assert t.version == ifx first == 0 then 0 else first - 0x50;
  #assert t.n == spk_digit(spk[2]) * 16 + spk_digit(spk[3]);
  #assert a3_program_is(t, spk[4..<spk.count]);
}
// BIP-350's invalid segwit addresses: none may be offered.
BIP350_INVALID :: [15] string.[
  "tc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vq5zuyut",
  "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqh2y7hd",
  "tb1z0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqglt7rf",
  "BC1S0XLXVLHEMJA6C4DQV22UAPCTQUPFHLXM9H8Z3K2E72Q4K9HCZ7VQ54WELL",
  "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kemeawh",
  "tb1q0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vq24jc47",
  "bc1p38j9r5y49hruaue7wxjce0updqjuyyx0kh56v8s25huc6995vvpql3jow4",
  "BC130XLXVLHEMJA6C4DQV22UAPCTQUPFHLXM9H8Z3K2E72Q4K9HCZ7VQ7ZWS8R",
  "bc1pw5dgrnzv",
  "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7v8n0nx0muaewav253zgeav",
  "BC1QR508D6QEJXTDG4Y5R3ZARVARYV98GJ9P",
  "tb1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vq47Zagq",
  "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7v07qwwzcrf",
  "tb1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vpggkg4j",
  "bc1gmk9yu"];
// The reason BIP-350 gives, as this reader's status.
BIP350_WHY :: [15] int.[A3_ADDRESS, A3_CHECKSUM, A3_CHECKSUM, A3_CHECKSUM, A3_CHECKSUM, A3_CHECKSUM,
  A3_ADDRESS, A3_ADDRESS, A3_LENGTH, A3_LENGTH, A3_LENGTH, A3_ADDRESS, A3_ADDRESS, A3_ADDRESS, A3_ADDRESS];
#forall "BIP-350's invalid addresses refuse for their stated reason" i: 0..<15 {
  #assert a3_try("bitcoin", BIP350_INVALID[i]).status == BIP350_WHY[i];
}
// BIP-350's generic Bech32m strings leave the residue 0x2bc830a3, through
// chapter 15's polymod.
BECH32M_VALID :: [7] string.["A1LQFN3A", "a1lqfn3a",
  "an83characterlonghumanreadablepartthatcontainsthetheexcludedcharactersbioandnumber11sg7hg6",
  "abcdef1l7aum6echk45nj3s0wdvt2fg8x9yrzpqzd3ryx",
  "11llllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllludsr8",
  "split1checkupstagehandshakeupstreamerranterredcaperredlc445v", "?1v759aa"];
#forall "Bech32m strings have residue 0x2bc830a3" i: 0..<7 {
  s := BECH32M_VALID[i];
  sep := 0; for k: 0..<s.count { if s[k] == '1' { sep = k; } }
  state := b32_prefix(s[0..<sep]);
  for k: sep + 1..<s.count { state = b32_step(state, cast(u8) b32_digit(b32_lower(s[k]))); }
  #assert state == A3_BECH32M;
}
// BIP-173's segwit vectors (the pinned coax/tests/vectors/upstream/bip173/
// bip-0173.mediawiki). Its v0 addresses still decode; its v1 to v16
// addresses carry Bech32 checksums, which BIP-350 made invalid.
#test "BIP-173's v0 addresses decode and its v1+ addresses refuse" {
  #assert a3_try("bitcoin", "bc1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3qccfmv3").status == A3_OK;
  #assert a3_try("bitcoin", "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx").status == A3_NETWORK;
  old := [3] string.["bc1pw508d6qejxtdg4y5r3zarvary0c5xw7kw508d6qejxtdg4y5r3zarvary0c5xw7k7grplx",
    "BC1SW50QA3JX3S", "bc1zw508d6qejxtdg4y5r3zarvaryvg6kdaj"];
  for s: old { #assert a3_try("bitcoin", s).status == A3_CHECKSUM; }
  #assert a3_try("bitcoin", "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t5").status == A3_CHECKSUM;
  #assert a3_try("bitcoin", "tb1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3q0sL5k7").status == A3_ADDRESS;
}
// Base58check addresses decoded by a Python script (int, hashlib), which
// shares no code with Coax: the version byte and hash160.
#test "base58 addresses match Python's decoding" {
  t := a3_try("bitcoin", "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2");
  #assert t.status == A3_OK && t.form == A3_P2PKH && a3_program_is(t, "77bff20c60e522dfaa3350c39b030a5d004e839a");
  t = a3_try("bitcoin", "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy");
  #assert t.status == A3_OK && t.form == A3_P2SH && a3_program_is(t, "b472a266d0bd89c13706a4132ccfb16f7c3b9fcb");
  t = a3_try("bitcoin", "mipcBbFg9gMiCh81Kj8tqqdgoZub1ZJRfn");
  #assert t.status == A3_NETWORK && a3_program_is(t, "243f1394f44554f4ce3fd68649c19adc483ce924");
  t = a3_try("bitcoin", "2MzQwSSnBHWHqSAqtTVQ6v47XtaisrJa1Vc");
  #assert t.status == A3_NETWORK && t.form == A3_P2SH && a3_program_is(t, "4e9f39ca4688ff102128ea4ccda34105324305b0");
}
// Shares computed by Python as total*w//W with arbitrary-precision
// integers, then one msat to each of the largest remainders total*w % W,
// the earlier first on a tie: the rule stated in nipa3.coax, computed
// without its q·w + r·w/W split.
#test "splits match Python's" {
  s: [16] int;
  w1 := [4] int.[7, 11, 13, 17]; #assert a3_split(999, w1[..], s[..]) == A3_OK && s[0] == 146 && s[1] == 229 && s[2] == 270 && s[3] == 354;
  w2 := [2] int.[5, 5]; #assert a3_split(1, w2[..], s[..]) == A3_OK && s[0] == 1 && s[1] == 0;
  w3 := [5] int.[3, 0, 4, 0, 5]; #assert a3_split(123456789, w3[..], s[..]) == A3_OK && s[0] == 30864197 && s[1] == 0 && s[2] == 41152263 && s[3] == 0 && s[4] == 51440329;
  w4 := [3] int.[1000000, 333333, 1]; #assert a3_split(A3_MAX_MSAT, w4[..], s[..]) == A3_OK && s[0] == 1574999212500393750 && s[1] == 524999212500393750 && s[2] == 1574999212500;
  w5: [16] int; for i: 0..<16 { w5[i] = 1; }
  #assert a3_split(100, w5[..], s[..]) == A3_OK && s[3] == 7 && s[4] == 6 && s[15] == 6;
  w6 := [6] int.[2, 3, 5, 7, 11, 13]; #assert a3_split(17, w6[..], s[..]) == A3_OK && s[0] == 1 && s[1] == 1 && s[2] == 2 && s[3] == 3 && s[4] == 5 && s[5] == 5;
}
// NIP-57 appendix G's example: weights 1, 1 and 2 are 25%, 25% and 50%.
#test "NIP-57's zap split example" {
  w: [3] int; s: [3] int;
  #assert a3_w3("1", "1", "2", w[..]) == A3_OK && a3_split(4000, w[..], s[..]) == A3_OK;
  #assert s[0] == 1000 && s[1] == 1000 && s[2] == 2000;
}
```

The BIP-350 vectors are copied from `bip-0350.mediawiki` at bitcoin/bips
commit `24e96e870fffaa257b465ce1f0370c14aac588e8` (SHA-256
`63634b06aa8bae88b31929674736e74964c4598684269ac2b0b140c43a7a0dec`). The
file is not yet pinned in `coax/tests/vectors/upstream/`.

### Checks in the library

`nipa3.coax` has `#test` checks written before its code: NIP-A3's example
event; every BIP-350 vector; the 90-character limit, a version with no
program, 5 bits of padding, regtest and look-alike human-readable parts,
and one letter's case flipped at each end of the alphabet; base58's
testnet versions, a wrong first checksum byte, a string of only `1`s and a
Litecoin version byte; every boundary character of a Lightning address;
every type group; the list's kind, shape and 16-target limit; and weights
and splits at their bounds, with a `#forall` over 624 totals and weights
that the shares sum to the total and each is within 1 msat of the exact
share.

```shell
coax/bin/coax test coax/lib/nostr/nipa3.coax
coax/bin/coax test nostr/examples/checks/payment-targets.coax
coax/bin/coax mutate coax/lib/nostr/nipa3.coax --root-only
```

On 2026-11-01, `coax test` passed all 219 checks of `nipa3.coax` (with the
loaded files') and all 226 of the example's checks. `coax mutate
--root-only` killed all 573 mutants of `nipa3.coax`, with none surviving
and none marked equivalent. Its first run left 73 survivors; each was
killed by a new check or by removing a test the code did not need
(base58 length bounds that the leading-zero rule and the 25-byte limit
already enforce, counters where a flag was enough). The example ran in debug and release with the same output.
These runs used the cached release compiler in `~/.cache/coax-release`.
The full mutation run (`COAX_MUTATION_FULL=1`) and a mutation run of the
example were not run.

### Not built

- `bip352` silent payment addresses and `bip353` DNS payment names. Both
  are Bitcoin; they read as `A3_NOT_BUILT` and are not offered.
- Sending to segwit versions with no output type, which BIP-350 asks
  wallets to support. They are refused on purpose.
- LNURL-pay (`lnurl1...`) in a `lightning` target, and resolving a
  Lightning address's domain. Chapter 71 fetches the LNURL-pay endpoint.
- Spark and Taproot Assets targets: NIP-A3 lists no type for them.
- Reading `zap` tags from an event, and `lud06` and `lud16` in profiles:
  chapters 71 and 45.
- Testnet and signet payments: every non-mainnet address refuses.

## Summary

- `a3_list` reads a kind 10133 event's `payto` tags in order and keeps
  every target with a status. Only `bitcoin` and `lightning` targets can
  be offered.
- `a3_segwit` decodes `bc1` addresses: Bech32 for version 0, Bech32m for
  versions 1 to 16, padding of at most 4 zero bits, programs of 2 to 40
  bytes and 20 or 32 for version 0. Only P2WPKH, P2WSH and P2TR are
  offered.
- `a3_base58` decodes `1` and `3` addresses and checks the double SHA-256
  checksum.
- `a3_pay_uri` writes a `bitcoin:` or `lightning:` link only for a target
  that may be offered.
- `a3_split` divides a total in millisatoshis by weight into whole shares
  that sum exactly to the total; the leftover goes to the largest
  remainders, the earlier tag first.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [15. Bech32](ch015-bech32.md)
- [4. SHA-256](ch004-sha-256.md)

## Sources

- [NIP-A3](../nips/official/A3.md)
- [NIP-57](../nips/official/57.md), appendix G (zap splits).
- [BIP-173](https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki)
  and its pinned copy in
  [coax/tests/vectors/upstream/bip173/](../../coax/tests/vectors/upstream/bip173/bip-0173.mediawiki).
- [BIP-350](https://github.com/bitcoin/bips/blob/master/bip-0350.mediawiki),
  Bech32m and its test vectors.
- [LUD-16](https://github.com/lnurl/luds/blob/luds/16.md), Lightning
  addresses.
- [RFC 8905](https://www.rfc-editor.org/rfc/rfc8905.html), the `payto:`
  URI scheme.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
