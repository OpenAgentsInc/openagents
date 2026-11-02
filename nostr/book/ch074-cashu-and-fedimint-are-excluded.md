# 74. Cashu and Fedimint are excluded

Three pinned NIPs carry ecash over Nostr: NIP-60 keeps a Cashu wallet in
relays, NIP-61 sends Cashu tokens as nutzaps, and NIP-87 announces and
recommends Cashu mints and Fedimint federations. AGENTS.md allows
Lightning, on-chain Bitcoin, Spark and Taproot Assets, and names Cashu as
excluded. Fedimint is not on the allowed list either. So this stack builds
none of the three NIPs. A client still receives their events, so this
chapter builds a recognizer: a table of the nine kinds, a classifier that
marks those events and ecash payment values as excluded, and a book check
that fails if any other Coax file starts to name them.

Part VI, Every NIP. Result: NIP-60 Cashu wallets, NIP-61 nutzaps and NIP-87
mint discovery, recognized and never built: a classifier marks their nine
kinds, references to them and ecash payment values as excluded, and the
book check fails if another file names them.

## What this chapter teaches

- NIP-60's wallet event 17375, token events 7375, history events 7376 and
  quote events 7374.
- NIP-61's nutzap information event 10019 and nutzap events 9321.
- NIP-87's Cashu mint announcements 38172, Fedimint announcements 38173 and
  mint recommendations 38000.
- Who holds the money in each scheme, and why that puts them outside the
  repository's rule.
- How a client classifies a received event: excluded, referring to an
  excluded kind, or not ecash.
- How a publishing path and a payment path refuse ecash, and which earlier
  chapters already refuse it.
- How a check in the book's tooling keeps the rest of the library from
  naming these kinds without an owner decision.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_tag(e, i)` | Reads a tag of any name: the name and its first two values (chapter 55) |
| [ecash.coax](../../coax/lib/nostr/ecash.coax) | `EC_KIND`, `EC_NAME`, `EC_NIP`, `EC_HOLDER`, `EC_WORDS` | The nine excluded kinds and the words that name ecash |
| [ecash.coax](../../coax/lib/nostr/ecash.coax) | `ec_kind(kind)` | The row of an excluded kind |
| [ecash.coax](../../coax/lib/nostr/ecash.coax) | `ec_event(e)` | A received event's verdict |
| [ecash.coax](../../coax/lib/nostr/ecash.coax) | `ec_build(kind)` | The publishing check |
| [ecash.coax](../../coax/lib/nostr/ecash.coax) | `ec_option(type)`, `ec_value(text)` | A payment option's type and value |
| [book.py](../../psionic/tests/book.py) | `excluded(rule)` | The book check: the table against the pinned texts, and the library scanned for other files that name ecash |

`ecash.coax` is new with this chapter. It is an `#authority` file because it
decides policy: what a client may act on. It holds no wallet, parses no
token, contacts no mint and builds no event.

## Ecash and the money rule

Cashu and Fedimint are ecash systems. A user deposits bitcoin, usually over
Lightning, and receives blind-signed notes in return. The notes are bearer
tokens: whoever holds the secret can spend them, and the issuer cannot link
a note to the deposit.

- In Cashu, one mint issues and redeems the notes. The mint holds the
  bitcoin. NIP-61's own text warns that a nutzap sent to a mint the
  recipient does not read "may result in the recipient donating the tokens
  to the mint".
- In Fedimint, a federation of guardians holds the bitcoin under a
  threshold of their keys, and issues the notes together.

In both, a note is a claim on bitcoin that the issuer holds, not bitcoin
that the user controls by key or by a Lightning channel. AGENTS.md's rule
lists the money this repository builds: Lightning, on-chain Bitcoin, Spark
and Taproot Assets. Cashu is named as excluded, and Fedimint is not on the
list. The chapter applies the rule; it does not judge the systems.

## The nine kinds

The pinned official README's kind table lists nine kinds whose NIP column
is 60, 61 or 87. Each row below quotes that table and the first line of the
NIP's text that names the kind.

| Kind | README name | NIP | Source line in the NIP |
| --- | --- | --- | --- |
| 7374 | Reserved Cashu Wallet Tokens | [60](../nips/official/60.md) | line 183: `"kind": 7374,` (the quote event, "Redeeming a quote (optional)") |
| 7375 | Cashu Wallet Tokens | [60](../nips/official/60.md) | line 21: "A user has `kind:7375` events that represent the unspent proofs of the wallet." |
| 7376 | Cashu Wallet History | [60](../nips/official/60.md) | line 22: "A user has `kind:7376` events that represent the spending history of the wallet" |
| 9321 | Nutzap | [61](../nips/official/61.md) | line 17: "She publishes a `kind:9321` event to the relays Bob indicated with the proofs she minted." |
| 10019 | Nutzap Mint Recommendation | [61](../nips/official/61.md) | line 15: "Alice fetches event `kind:10019` from Bob to see the mints Bob trusts." |
| 17375 | Cashu Wallet Event | [60](../nips/official/60.md) | line 20: "A user has a `kind:17375` event that represents a wallet." |
| 38000 | Mint Recommendation | [87](../nips/official/87.md) | line 9: "This NIP describes `kind:38173`, `kind:38172` and `kind:38000`" |
| 38172 | Cashu Mint Announcement | [87](../nips/official/87.md) | line 61: "Cashu mints SHOULD publish `kind:38172` events" |
| 38173 | Fedimint Announcement | [87](../nips/official/87.md) | line 83: "Fedimints SHOULD publish `kind:38173` events" |

What each kind carries:

- **17375**, the wallet: a replaceable event whose content is NIP-44
  encrypted and holds the wallet's mint URLs and a `privkey` used to unlock
  P2PK-locked tokens. The text says this key is "not associated in any way
  to the user's Nostr private key".
- **7375**, tokens: encrypted unspent Cashu proofs for one mint, with a
  `del` list of token events they replace. Spending deletes the old event
  with a NIP-09 kind 5 that "MUST have a tag `["k", "7375"]`".
- **7376**, history: the direction, amount and unit of a balance change,
  with `e` tags marked `created`, `destroyed` or `redeemed`.
- **7374**, a quote: the encrypted ID of a mint quote, with an expiration
  of two weeks.
- **10019**, nutzap information: the relays, mints and P2PK public key
  others must use to send the user a nutzap.
- **9321**, a nutzap: `proof` tags holding P2PK-locked Cashu proofs, a `u`
  tag naming the mint, and `p` and `e` tags naming the recipient and the
  event.
- **38172** and **38173**, announcements: a mint's or federation's `d`
  identifier, `u` connection URLs or invite codes, supported `nuts` or
  `modules`, and an `n` network tag.
- **38000**, a recommendation: an addressable event with a `k` tag of 38172
  or 38173 and `a` tags pointing at the announcement.

The three texts also mention two kinds from other NIPs: kind 0 (NIP-01
metadata, for a mint's display) and kind 5 (NIP-09 deletion, for spent
token events). Those kinds are built in their own chapters and are not
excluded.

<!-- source: coax/lib/nostr/ecash.coax table -->
```jai
// The kinds, in the order of the pinned official README's kind table, with
// its name for each and the NIP that defines it.
EC_KIND :: [EC_N] int.[7374, 7375, 7376, 9321, 10019, 17375, 38000, 38172, 38173];
EC_NAME :: [EC_N] string.["Reserved Cashu Wallet Tokens", "Cashu Wallet Tokens", "Cashu Wallet History",
  "Nutzap", "Nutzap Mint Recommendation", "Cashu Wallet Event", "Mint Recommendation",
  "Cashu Mint Announcement", "Fedimint Announcement"];
EC_NIP :: [EC_N] string.["60", "60", "60", "61", "61", "60", "87", "87", "87"];
// Who holds the money an event of the kind speaks for.
EC_HOLDER :: [EC_N] string.["a Cashu mint", "a Cashu mint", "a Cashu mint", "a Cashu mint", "a Cashu mint",
  "a Cashu mint", "a Cashu mint or a Fedimint federation", "a Cashu mint", "a Fedimint federation"];
// Payment option types and value prefixes that name ecash.
EC_WORDS :: [4] string.["cashu", "ecash", "fedimint", "nutzap"];
```

## Classifying an event

A client that receives an event asks `ec_event` before it renders or acts
on it. There are four answers:

1. **Excluded: not built** (`EC_EXCLUDED`). The event's kind is one of the
   nine. The client shows it as unsupported and does nothing else: no
   decryption, no token swap, no mint contact, no reply that builds on it.
   The tags are not read at all.
2. **Refers to an excluded kind** (`EC_REFERS`). The event is of another
   kind, but a `k` or `K` tag names one of the nine, or an `a` or `A` tag's
   address starts with one. The client handles the event as its own kind
   and never follows the reference. NIP-60's kind 5 deletion with
   `["k", "7375"]` is the common case.
3. **Not ecash** (`EC_OK`). Its own chapter decides.
4. **Tags don't read** (`EC_EVENT`). The tags are not valid JSON strings,
   or the value of a `k`, `K`, `a` or `A` tag is too long to read
   (over 256 bytes). A value too long in any other tag is skipped.

<!-- source: coax/lib/nostr/ecash.coax classify -->
```jai
// The row of an excluded kind, or EC_NONE.
ec_kind :: (kind:int)->int {
  for i:0..<EC_N {if EC_KIND[i]==kind {return i;}}
  return EC_NONE;
}
// A received event: EC_EXCLUDED for an excluded kind, EC_REFERS when a k, K,
// a or A tag names one (the first such tag), EC_OK otherwise, or EC_EVENT
// when its tags don't read.
ec_event :: (e:NostrEvent)->EcVerdict {
  v:EcVerdict;if e.error!=NE_OK {return v;}
  v.row=ec_kind(e.kind);
  if v.row!=EC_NONE {v.status=EC_EXCLUDED;return v;}
  i:=0;t:=n32_tag(e,0);
  while t.error!=NT_INDEX {
    name:=cast(string)t.v[0].b[0..<t.v[0].n];
    // A value too long to read is skipped, unless it is the one a k, K, a
    // or A tag would name a kind with.
    // A value not read is empty, and names no kind.
    if t.error==NT_LIMIT && ec_names(name) {return v;}
    if t.error!=NT_OK && t.error!=NT_SHAPE && t.error!=NT_LIMIT {return v;}
    row:=ec_tag_kind(name,cast(string)t.v[1].b[0..<t.v[1].n]);
    if row!=EC_NONE {v.status=EC_REFERS;v.row=row;v.at=i;return v;}
    i+=1;t=n32_tag(e,i);
  }
  v.status=EC_OK;return v;
}
// The publishing check: an event of an excluded kind is never built.
ec_build :: (kind:int)->int {return ifx ec_kind(kind)==EC_NONE then EC_OK else EC_EXCLUDED;}
// A payment option's type: EC_EXCLUDED when it contains an ecash word in
// any case.
ec_option :: (type:string)->int {
  for w:EC_WORDS {if ec_holds_ci(type,w) {return EC_EXCLUDED;}}
  return EC_OK;
}
// A payment value: EC_EXCLUDED for a Cashu token (cashuA..., cashuB...,
// any case) or a Fedimint invite code (fed11...).
ec_value :: (text:string)->int {
  if ec_starts_ci(text,"cashu") || ec_starts_ci(text,"fed11") {return EC_EXCLUDED;}
  return EC_OK;
}
```

`ec_option` matches any of the four words anywhere in a payment option's
type, in any case, so `cashu`, `Cashu` and `cashu-token` all refuse.
`ec_value` matches the start of a value: NUT-00 serializes Cashu tokens
with the prefix `cashuA` (version 3) or `cashuB` (version 4), and NIP-87's
example Fedimint invite codes start with `fed11`.

### Worked example: four events

Work these out before running the program. The table has nine rows, in the
order 7374, 7375, 7376, 9321, 10019, 17375, 38000, 38172, 38173, so row 0 is
7374 and row 8 is 38173.

1. **E1**, NIP-61's example nutzap: kind 9321 with tags `unit`, `u` and `p`
   (its `proof` tag left out). `ec_kind(9321)` walks the table: 7374, 7375,
   7376 differ, row 3 is 9321. The verdict is excluded, row 3, "Nutzap",
   NIP-61, money held by a Cashu mint. The tags are never read.
2. **E2**, NIP-87's example Cashu mint announcement: kind 38172.
   `ec_kind` finds row 7. Excluded: "Cashu Mint Announcement", NIP-87.
3. **E3**, the NIP-09 deletion NIP-60 requires: kind 5 with tags
   `["e", "aa…a"]` and `["k", "7375"]`. `ec_kind(5)` finds no row, so the
   tags are read in order. Tag 0 is named `e`: not `k`, `K`, `a` or `A`, so
   `ec_tag_kind` returns no row. Tag 1 is named `k` with value `7375`.
   `ec_number("7375")` reads four digits: 7, 73, 737, 7375. `ec_kind(7375)`
   is row 1. The verdict is "refers to an excluded kind", tag 1, kind 7375.
   A client may still apply the deletion to events it holds; it does not
   look up the token event.
4. **E4**, a NIP-57 zap request for a kind 1 note: kind 9734 with tags `p`,
   `e`, `["k", "1"]` and `amount`. No row for 9734. Tags `p` and `e` name no
   kind. Tag 2 is `k` with value `1`; `ec_kind(1)` finds no row. Tag 3,
   `amount`, names no kind. The verdict is "not ecash": chapter 71 handles
   the zap, which is paid over Lightning.

## The example

The example is
[nostr/examples/ecash-excluded.coax](../examples/ecash-excluded.coax). It
prints the table, classifies E1 to E4 and a reaction to a Fedimint
announcement, checks five payment options, then tries to build three
excluded kinds and accept two ecash values. Events are built from their
tags with `n32_event`; a client reads them from relays and verifies them
first (chapter 21).

<!-- source: nostr/examples/ecash-excluded.coax example -->
```jai
#load "../../coax/lib/nostr/ecash.coax";
#import "Basic";

STATUS :: [4] string.["not ecash", "excluded: not built", "refers to an excluded kind", "tags don't read"];

show :: (label: string, e: NostrEvent) {
  v := ec_event(e);
  print("%: kind %\n  %", label, e.kind, STATUS[v.status]);
  if v.status == EC_REFERS { print(", tag % names kind %", v.at, EC_KIND[v.row]); }
  if v.row != EC_NONE { print(" (%, NIP-%, money held by %)", EC_NAME[v.row], EC_NIP[v.row], EC_HOLDER[v.row]); }
  print("\n");
}
option :: (type: string, value: string) {
  t := ec_option(type); w := ec_value(value);
  print("  type %, value %: ", type, value);
  if t == EC_EXCLUDED || w == EC_EXCLUDED { print("%\n", STATUS[EC_EXCLUDED]); }
  else { print("not ecash (its own chapter decides)\n"); }
}
build :: (label: string, kind: int) {
  print("build %: ", label);
  if ec_build(kind) == EC_EXCLUDED { print("refused, %\n", STATUS[EC_EXCLUDED]); }
  else { print("may be built\n"); }
}
accept :: (value: string) {
  print("accept %: ", value);
  if ec_value(value) == EC_EXCLUDED { print("refused, %\n", STATUS[EC_EXCLUDED]); }
  else { print("not ecash\n"); }
}

main :: () {
  print("excluded kinds\n");
  for r: 0..<EC_N { print("  %  NIP-%  %, money held by %\n", EC_KIND[r], EC_NIP[r], EC_NAME[r], EC_HOLDER[r]); }

  print("four events\n");
  // E1: NIP-61's example nutzap, without its proof tag.
  show("E1", n32_event(9321, "[[\"unit\",\"sat\"],[\"u\",\"https://stablenut.umint.cash\"],[\"p\",\"e9fbced3a42dcf551486650cc752ab354347dd413b307484e4fd1818ab53f991\"]]"));
  // E2: NIP-87's example Cashu mint announcement.
  show("E2", n32_event(38172, "[[\"d\",\"mint-pubkey\"],[\"u\",\"https://cashu.example.com\"],[\"nuts\",\"1,2,3,4,5,6,7\"],[\"n\",\"mainnet\"]]"));
  // E3: the NIP-09 deletion NIP-60 requires when a token event is spent.
  show("E3", n32_event(5, "[[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"],[\"k\",\"7375\"]]"));
  // E4: a NIP-57 zap request for a kind 1 note, paid over Lightning.
  show("E4", n32_event(9734, "[[\"p\",\"e9fbced3a42dcf551486650cc752ab354347dd413b307484e4fd1818ab53f991\"],[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"],[\"k\",\"1\"],[\"amount\",\"21000\"]]"));
  print("an event that only refers to ecash\n");
  show("E5, a reaction to a Fedimint announcement", n32_event(7, "[[\"a\",\"38173:fedimint-pubkey:federation-id\"],[\"k\",\"38173\"]]"));

  print("payment options\n");
  option("cashu", "cashuAeyJ0b2tlbiI6W119");
  option("Fedimint", "fed11abc");
  option("url", "cashuBo2F0");
  option("ln", "lnbc430u1x");
  option("btc", "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");

  print("refusals\n");
  build("a nutzap (kind 9321)", 9321);
  build("a Cashu wallet event (kind 17375)", 17375);
  build("a mint recommendation (kind 38000)", 38000);
  accept("cashuAeyJ0b2tlbiI6W119");
  accept("fed11abc");
  build("a kind 1 note", 1);
  show("tags that don't read", n32_event(1, "[[\"k\",7375]]"));
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/ecash-excluded.coax
```

The output, checked byte for byte in debug and release, is
[nostr/fixtures/ecash-excluded.out](../fixtures/ecash-excluded.out):

```text
excluded kinds
  7374  NIP-60  Reserved Cashu Wallet Tokens, money held by a Cashu mint
  7375  NIP-60  Cashu Wallet Tokens, money held by a Cashu mint
  7376  NIP-60  Cashu Wallet History, money held by a Cashu mint
  9321  NIP-61  Nutzap, money held by a Cashu mint
  10019  NIP-61  Nutzap Mint Recommendation, money held by a Cashu mint
  17375  NIP-60  Cashu Wallet Event, money held by a Cashu mint
  38000  NIP-87  Mint Recommendation, money held by a Cashu mint or a Fedimint federation
  38172  NIP-87  Cashu Mint Announcement, money held by a Cashu mint
  38173  NIP-87  Fedimint Announcement, money held by a Fedimint federation
four events
E1: kind 9321
  excluded: not built (Nutzap, NIP-61, money held by a Cashu mint)
E2: kind 38172
  excluded: not built (Cashu Mint Announcement, NIP-87, money held by a Cashu mint)
E3: kind 5
  refers to an excluded kind, tag 1 names kind 7375 (Cashu Wallet Tokens, NIP-60, money held by a Cashu mint)
E4: kind 9734
  not ecash
an event that only refers to ecash
E5, a reaction to a Fedimint announcement: kind 7
  refers to an excluded kind, tag 0 names kind 38173 (Fedimint Announcement, NIP-87, money held by a Fedimint federation)
payment options
  type cashu, value cashuAeyJ0b2tlbiI6W119: excluded: not built
  type Fedimint, value fed11abc: excluded: not built
  type url, value cashuBo2F0: excluded: not built
  type ln, value lnbc430u1x: not ecash (its own chapter decides)
  type btc, value bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4: not ecash (its own chapter decides)
refusals
build a nutzap (kind 9321): refused, excluded: not built
build a Cashu wallet event (kind 17375): refused, excluded: not built
build a mint recommendation (kind 38000): refused, excluded: not built
accept cashuAeyJ0b2tlbiI6W119: refused, excluded: not built
accept fed11abc: refused, excluded: not built
build a kind 1 note: may be built
tags that don't read: kind 1
  tags don't read
```

The refusals are the broken cases. Building a nutzap, a wallet event or a
mint recommendation refuses with `excluded: not built`; so does accepting
`cashuAeyJ0b2tlbiI6W119` (the prefix and base64url of `{"token":[]}`) or
`fed11abc`. A kind 1 note may be built. An event whose `k` tag value is
the number `7375` instead of the string `"7375"` refuses as "tags don't
read".

## Refusals elsewhere

Two earlier chapters already refuse ecash on their own paths. This chapter
links to them and does not repeat them:

- [Chapter 66](ch066-markets-and-orders.md): a NIP-15 payment option of
  type `cashu` is not a Bitcoin rail (`n15_rail` returns
  `N15_RAIL_NONE`), and `n15_write_request` refuses to write one
  (`N15_W_RAIL`).
- [Chapter 73](ch073-payment-targets.md): a NIP-A3 `payto` target of type
  `cashu` reads as an unknown type (`A3_UNKNOWN`), and `a3_pay_uri` writes
  no link for it.

The checks below confirm both, next to `ec_option`.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/ecash-excluded.coax](../examples/checks/ecash-excluded.coax),
which loads the example, `nip15.coax` and `nipa3.coax`. They compare the
code with values it did not produce:

- the nine rows of the pinned README's kind table, copied by hand, against
  `EC_KIND`, `EC_NAME` and `EC_NIP`;
- the hand classification of E1 to E4 above;
- every excluded kind refused by `ec_build`, and NUT-00 token prefixes and
  NIP-87's invite codes refused by `ec_value`, with Lightning, on-chain and
  LNURL values kept;
- chapter 66's and chapter 73's refusals of `cashu`.

The first of these, the README rows, sits above the listing below: its
strings are Markdown table rows, quoted exactly as the pinned file has
them, with each cell trimmed and compared with the table.

<!-- source: nostr/examples/checks/ecash-excluded.coax check -->
```jai
// The four events the chapter classifies by hand.
#test "the hand classification of E1 to E4" {
  e1 := n32_event(9321, "[[\"unit\",\"sat\"],[\"u\",\"https://stablenut.umint.cash\"],[\"p\",\"e9fbced3a42dcf551486650cc752ab354347dd413b307484e4fd1818ab53f991\"]]");
  v := ec_event(e1); #assert v.status == EC_EXCLUDED && EC_KIND[v.row] == 9321 && v.at == EC_NONE;
  e2 := n32_event(38172, "[[\"d\",\"mint-pubkey\"],[\"u\",\"https://cashu.example.com\"],[\"nuts\",\"1,2,3,4,5,6,7\"],[\"n\",\"mainnet\"]]");
  v = ec_event(e2); #assert v.status == EC_EXCLUDED && EC_KIND[v.row] == 38172 && EC_HOLDER[v.row] == "a Cashu mint";
  e3 := n32_event(5, "[[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"],[\"k\",\"7375\"]]");
  v = ec_event(e3); #assert v.status == EC_REFERS && EC_KIND[v.row] == 7375 && v.at == 1;
  e4 := n32_event(9734, "[[\"p\",\"e9fbced3a42dcf551486650cc752ab354347dd413b307484e4fd1818ab53f991\"],[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"],[\"k\",\"1\"],[\"amount\",\"21000\"]]");
  v = ec_event(e4); #assert v.status == EC_OK && v.row == EC_NONE && v.at == EC_NONE;
}

// Every attempt to build one of the kinds, or to accept a token, refuses.
#forall "every excluded kind refuses to be built" i: 0..<EC_N {
  #assert ec_build(EC_KIND[i]) == EC_EXCLUDED;
}
#test "tokens and invite codes refuse; Bitcoin values don't" {
  // NUT-00 token prefixes (V3 cashuA, V4 cashuB) and NIP-87's invite codes.
  refuse := [5] string.["cashuAeyJ0b2tlbiI6W119", "cashuBo2F0", "fed11abc..", "fed11xyz..", "CashuA"];
  for s: refuse { #assert ec_value(s) == EC_EXCLUDED; }
  keep := [4] string.["lnbc430u1x", "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4", "alice@example.com", "lnurl1dp68gurn"];
  for s: keep { #assert ec_value(s) == EC_OK && ec_option(s) == EC_OK; }
}

// The refusals that already exist, which this chapter links to rather
// than repeats: chapter 66's NIP-15 rails and chapter 73's NIP-A3 targets.
#test "chapters 66 and 73 refuse cashu too" {
  #assert n15_rail("cashu") == N15_RAIL_NONE && ec_option("cashu") == EC_EXCLUDED;
  out: [512] u8;
  types := [2] string.["ln", "cashu"]; links := [2] string.["lnbc430u1x", "cashuAeyJ0b2tlbiI6W119"];
  #assert n15_write_request(out[..], "o1", "", types[..], links[..]) == N15_W_RAIL;
  t := a3_try("cashu", "cashuAeyJ0b2tlbiI6W119");
  uri: [300] u8;
  #assert t.status == A3_UNKNOWN && a3_pay_uri(t, uri[..]) == -1;
}
```

### The book check

A `#test` cannot read files, so the check that keeps the library clean is
in the book's tooling, which runs on every structure check
(`python3 psionic/tests/book.py --book nostr`). `excluded` in `book.py`
reads the table from `ecash.coax` and:

1. compares it with the rows of the pinned
   [README](../nips/official/README.md) whose NIP column is 60, 61 or 87,
   in order, with the same names;
2. reads every `kind:N` and `"kind": N` in the pinned NIP-60, NIP-61 and
   NIP-87 texts, and fails on a kind that is in neither the table nor the
   two other-NIP kinds (0 and 5), or on a table kind its NIP never names;
3. scans every `.coax` file under `coax/lib/nostr/` and `nostr/examples/`
   for an excluded kind as a whole number or for `cashu`, `ecash`,
   `fedimint` or `nutzap` in any case, and fails on any file not in its
   list of allowed files.

The allowed files are `ecash.coax`, this chapter's example and checks, and
chapter 66's `nip15.coax` and `markets-orders.coax`, which name `cashu`
only to refuse it. A file that builds one of these kinds gets a row only
with an owner decision recorded in `NEEDS_OWNER.md`. The check also fails
if an allowed file stops naming ecash, so the list cannot go stale.

Python here is tooling, as AGENTS.md allows: it never ships and is never
needed at run time.

### Checks in the library

`ecash.coax` has `#test` checks written before its code: each table row
found by `ec_kind` and refused by `ec_build`, and the twelve kinds next to
them not; decimal kinds in tags at their bounds (empty, seven digits, a
sign, a space); `k`, `K`, `a` and `A` tags against `e`, `t` and two-letter
names; an `a` value with no colon or a leading colon; an empty tag, a tag
with only a name, a non-string value, and a value over 256 bytes in an
`alt` tag and in `a` and `K` tags; and payment types and values in mixed
case, with a letter changed at the start and the end.

```shell
coax/bin/coax test coax/lib/nostr/ecash.coax
coax/bin/coax test nostr/examples/checks/ecash-excluded.coax
coax/bin/coax mutate coax/lib/nostr/ecash.coax --root-only
python3 psionic/tests/book.py --book nostr
```

On 2026-11-01, `coax test` passed all 206 checks of `ecash.coax` (with the
loaded files') and all 251 of the example's checks. `coax mutate
--root-only` killed all 134 mutants of `ecash.coax`, with none surviving
and none marked equivalent. Its first run left 9 survivors; each was
killed by a new check or by removing a condition the code did not need (a
values count that an empty unread value already covers, a loop bound one
past the last match). The example ran in debug and release with the same
output. These runs used the cached release compiler in
`~/.cache/coax-release`. The full mutation run (`COAX_MUTATION_FULL=1`)
was not run.

### Not built

By rule, nothing of NIP-60, NIP-61 or NIP-87 beyond recognition:

- no wallet event, token event, history event or quote event, and no
  decryption of their content;
- no Cashu proof parsing, DLEQ verification, P2PK locking, minting,
  swapping or redeeming;
- no nutzap sending, receiving or counting, and no kind 10019 publishing;
- no mint or federation discovery, connection or recommendation.

## Summary

- NIP-60, NIP-61 and NIP-87 define nine kinds: 7374, 7375, 7376, 17375
  (Cashu wallet), 9321 and 10019 (nutzaps), and 38000, 38172 and 38173
  (mint and federation discovery).
- In each, a mint or a federation holds the bitcoin, which puts them
  outside AGENTS.md's Bitcoin-only rule.
- `ec_event` marks an event of those kinds "excluded: not built", an event
  whose `k` or `a` tag names one "refers to an excluded kind", and any
  other event "not ecash".
- `ec_build`, `ec_option` and `ec_value` refuse building those kinds and
  accepting ecash payment types, tokens and invite codes.
- The book check compares the table with the pinned texts and fails when
  any other Coax file names these kinds or ecash without an owner
  decision.

## Dependencies

- [55. Labels, reports and trusted assertions](ch055-labels-reports-and-trusted-assertions.md)
- [66. Markets and orders](ch066-markets-and-orders.md)
- [73. Payment targets](ch073-payment-targets.md)

## Sources

- [NIP-60](../nips/official/60.md), Cashu wallets.
- [NIP-61](../nips/official/61.md), nutzaps.
- [NIP-87](../nips/official/87.md), Cashu and Fedimint discoverability.
- The pinned official [README](../nips/official/README.md), its kind
  table.
- [AGENTS.md](../../AGENTS.md), "Bitcoin-only money".
- [NUT-00](https://github.com/cashubtc/nuts/blob/main/00.md), the Cashu
  token serialization (`cashuA`, `cashuB`).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
