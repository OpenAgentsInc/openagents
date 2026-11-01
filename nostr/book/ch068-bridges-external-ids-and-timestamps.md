# 68. Bridges, external IDs and timestamps

Some events point outside Nostr. An event can name a book, a web page or a
Bitcoin transaction it is about. It can say that it was copied in from
another network by a bridge. And another event can carry a proof that it
existed by the time a Bitcoin block was mined. This chapter reads the tags
and the proof that say these things, and says exactly what each one proves
and what it does not.

Part X, Every NIP. Result: NIP-48 `proxy` tags for bridged events, NIP-73
`i` and `k` tags for external content IDs (URLs, ISBNs, podcasts,
geohashes), and NIP-03 kind 1040 OpenTimestamps attestations.

## What this chapter teaches

- NIP-73: one normal form per identifier type, such as `isbn:` without
  hyphens and `geo:` in lower case, so that two events about one thing
  carry the same `i` tag; and how an ISBN's check digit is computed.
- NIP-48: `["proxy", id, protocol]` for `activitypub`, `atproto`, `rss`
  and `web`, and the form of each protocol's id.
- NIP-03: kind 1040 with an `e` tag and base64 `.ots` content. The
  OpenTimestamps file format: its header, its operations (sha256, append,
  prepend) and its attestations, and how replaying the operations from an
  event id reaches a Bitcoin merkle root.
- What a parser can check and what it cannot: a proof that replays to a
  root says nothing until the caller compares that root with the block
  header. The pinned NIP-03 text marks it `unrecommended` and vulnerable to
  one specific attack.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_tag(e, i)` | Reads a tag of any name: the name and its first two values (chapter 55) |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag(e, i)` | Reads and checks an `e` tag (chapter 22) |
| [sha256.coax](../../coax/lib/crypto/sha256.coax) | `sha256_hash_checked` | SHA-256 (chapter 4) |
| [base64.coax](../../coax/lib/nostr/base64.coax) | `base64_decode_checked` | Base64 (chapter 1) |
| [nip73.coax](../../coax/lib/nostr/nip73.coax) | `n73_type(s)`, `n73_normalize(raw)` | An identifier's type, and its normal form |
| [nip73.coax](../../coax/lib/nostr/nip73.coax) | `n73_tags(e)` | An event's `i` and `k` tags |
| [nip48.coax](../../coax/lib/nostr/nip48.coax) | `n48_id_ok(protocol, id)`, `n48_read(e)` | A proxy id's form, and an event's proxy tags |
| [nip03.coax](../../coax/lib/nostr/nip03.coax) | `n03_varuint(r)`, `n03_apply(m, op, arg)` | A number in a proof, and one operation on a message |
| [nip03.coax](../../coax/lib/nostr/nip03.coax) | `n03_parse(proof)` | An `.ots` file: header, operations replayed, attestations |
| [nip03.coax](../../coax/lib/nostr/nip03.coax) | `n03_event(e, buf)` | A kind 1040 event: its `e` tag, its `k` tag and its proof |

The three library files are new with this chapter. None is an `#authority`
file: none decides money, admission or policy. Each reads and reports, and
the caller decides what to do with an event that names a book, came from a
bridge or claims a timestamp. Every procedure runs in a check, and
`coax mutate` leaves no survivors. No file fetches anything or verifies a
signature. The caller verifies the events (chapter 21) and passes them in.

## The records

Each reader returns a record with a status, the tag or byte where it
refused, and what it read. `N32Text` is chapter 55's decoded string of at
most 256 bytes.

<!-- source: coax/lib/nostr/nip73.coax record -->
```jai
N73Id :: struct {status:int=N73_SHAPE; type:int; value:N32Text;}   // value: the normal form
N73Tags :: struct {
  status:int=N73_EVENT; at:int=-1;      // the refusing tag
  ids:int; first:N73Id;                 // i tags, and the first
  hints:int;                            // i tags with a URL hint
  kinds:int;                            // k tags
  i_types:int; k_types:int;             // bit t set: an i or k tag of type t
  missing_k:int;                        // i types without their k tag
}
```

<!-- source: coax/lib/nostr/nip48.coax record -->
```jai
N48Proxy :: struct {
  status:int=N48_EVENT; at:int=-1;   // the refusing tag
  proxies:int;                       // proxy tags: one or more means bridged
  protocol:int; id:N32Text;          // the first proxy tag's
}
```

<!-- source: coax/lib/nostr/nip03.coax record -->
```jai
N03Attestation :: struct {
  type:int;                  // N03_BITCOIN, N03_PENDING or N03_UNKNOWN
  height:int=-1;             // Bitcoin: the block
  root:[32]u8;               // Bitcoin: the message there, the claimed merkle root
  at:int;                    // its offset in the proof
}
N03Proof :: struct {
  status:int=N03_MAGIC; at:int=-1;     // the refusal and its offset
  digest:[32]u8;                       // what the proof is about
  ops:int;                             // operations replayed
  attestations:[N03_MAX_ATTEST]N03Attestation; n:int;
  bitcoin:int; pending:int; unknown:int;
}
N03Event :: struct {
  status:int=N03_EVENT; tag_error:int;
  target:[32]u8;                       // the e tag's event id
  kind:int=-1;                         // the k tag, when present
  proof:N03Proof;
}
```

## External content IDs

NIP-73 puts an identifier from outside Nostr in an `i` tag and its type in
a `k` tag. A filter can match single-letter tags (chapter 23), so a client can
ask for every event about one ISBN with the filter `{"#i":
["isbn:9780765382030"]}`, or for every event about any book with `{"#k":
["isbn"]}`. That only works when everyone writes an identifier the same
way. An ISBN printed on a book as `978-0-7653-8203-0` and the same ISBN as
a search API returns it, `9780765382030`, are different strings, and a
relay compares strings. So NIP-73 gives each type one normal form.

These are the types this chapter reads, with the `i` tag's prefix and the
`k` tag:

<!-- source: coax/lib/nostr/nip73.coax table -->
```jai
// The i tag's prefix and the k tag of each type. A web URL has no prefix:
// its scheme, http or https, says what it is.
N73_PREFIX :: [N73_TYPES] string.["", "", "isbn:", "geo:", "iso3166:", "isan:",
  "doi:", "#", "podcast:guid:", "podcast:item:guid:", "podcast:publisher:guid:",
  "bitcoin:tx:", "bitcoin:address:"];
N73_K :: [N73_TYPES] string.["", "web", "isbn", "geo", "iso3166", "isan", "doi",
  "#", "podcast:guid", "podcast:item:guid", "podcast:publisher:guid",
  "bitcoin:tx", "bitcoin:address"];
```

The normal form of each:

| Type | Normal form | Checked |
| --- | --- | --- |
| web | the URL with its scheme and host in lower case, `/` for an empty path, no fragment | `http` or `https`, a host |
| isbn | the 10 or 13 digits without hyphens; a last `X` in upper case | the check digit |
| geo | the geohash in lower case | 1 to 12 characters of the geohash alphabet |
| iso3166 | the code in upper case | two letters, optionally `-` and one to three letters or digits |
| isan | the root, episode and check character in upper case; the version part dropped | four groups of four hex digits and a check character |
| doi | the DOI in lower case | `10.`, digits, `/` and a suffix |
| `#` | the topic in lower case (ASCII letters only) | no spaces, no second `#` |
| podcast | the GUID in lower case | a UUID, 8-4-4-4-12 hex digits |
| bitcoin:tx | the txid in lower case | 64 hex digits |
| bitcoin:address | bech32 (`bc1`...) in lower case; base58 as written | the alphabet and length, not the checksum |

NIP-73 also lists `<blockchain>:tx` and `<blockchain>:address` for other
chains, with examples on Ethereum. This repository is Bitcoin-only, so
any chain but Bitcoin is refused with its own status, `N73_CHAIN`, and
never read as a type. The type comes from the prefix:

<!-- source: coax/lib/nostr/nip73.coax type -->
```jai
// The type an identifier's prefix names, or N73_NONE.
n73_type :: (s:string)->int {
  scheme:[8]u8;n:=0;
  while n<s.count && n<8 {scheme[n]=n73_lower(s[n]);n+=1;}
  if n73_starts(cast(string)scheme[0..<n],"http://") || n73_starts(cast(string)scheme[0..<n],"https://") {return N73_WEB;}
  for t:N73_ISBN..<N73_TYPES {if n73_starts(s,N73_PREFIX[t]) {return t;}}
  return N73_NONE;
}
```

`n73_normalize` writes the prefix, then the body in its normal form, and
refuses a body that has the wrong shape:

<!-- source: coax/lib/nostr/nip73.coax normalize -->
```jai
// An identifier in its type's normal form, or the reason it has none.
n73_normalize :: (raw:string)->N73Id {
  id:N73Id;id.type=n73_type(raw);
  if id.type==N73_NONE {return n73_fail(id,ifx n73_chain(raw) then N73_CHAIN else N73_TYPE);}
  if raw.count>NT_MAX_VALUE {return n73_fail(id,N73_LIMIT);}
  p:=N73_PREFIX[id.type];
  for i:0..<p.count {id=n73_put(id,p[i]);}
  body:=raw[p.count..<raw.count];
  if id.type==N73_WEB {return n73_web(id,raw);}
  if id.type==N73_ISBN {return n73_isbn(id,body);}
  if id.type==N73_ISAN {return n73_isan(id,body);}
  if id.type==N73_BITCOIN_ADDRESS {return n73_address(id,body);}
  // Every other type is its body in one case, checked character by character.
  upper:=id.type==N73_ISO3166;
  for i:0..<body.count {id=n73_put(id,ifx upper then n73_upper(body[i]) else n73_lower(body[i]));}
  v:=id.value;s:=cast(string)v.b[p.count..<v.n];
  ok:bool;
  if id.type==N73_GEO {ok=s.count>=1 && s.count<=12;for i:0..<s.count {ok=ok && n73_in(N73_GEOHASH,s[i]);}}
  else if id.type==N73_ISO3166 {ok=n73_iso3166(s);}
  else if id.type==N73_DOI {ok=n73_doi(s);}
  else if id.type==N73_HASHTAG {ok=s.count>0;for i:0..<s.count {ok=ok && s[i]>' ' && s[i]!='#';}}
  else if id.type==N73_BITCOIN_TX {ok=s.count==64;for i:0..<s.count {ok=ok && n73_hex(s[i]);}}
  else {ok=n73_uuid(s);}
  if !ok {return n73_fail(id,N73_SHAPE);}
  return n73_fail(id,N73_OK);
}
```

### Worked example: an ISBN

The book in NIP-73's example is printed as `978-0-7653-8203-0`. To
normalize it:

1. Drop the hyphens: `9780765382030`, 13 digits. A 13-digit ISBN is an
   ISBN-13.
2. Weight the first 12 digits 1, 3, 1, 3, ... and add them:

   | Digit | 9 | 7 | 8 | 0 | 7 | 6 | 5 | 3 | 8 | 2 | 0 | 3 |
   | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
   | Weight | 1 | 3 | 1 | 3 | 1 | 3 | 1 | 3 | 1 | 3 | 1 | 3 |
   | Product | 9 | 21 | 8 | 0 | 7 | 18 | 5 | 9 | 8 | 6 | 0 | 9 |

   The sum is 100.
3. The check digit is the one that brings the sum to a multiple of 10:
   `(10 - 100 mod 10) mod 10 = 0`. The last digit is 0, so the ISBN is
   valid, and its normal form is `isbn:9780765382030`, which is the NIP's
   example.

If the last digit were 1 (`978-0-7653-8203-1`), the check would fail and
the code refuses it with `N73_CHECK`, so a typo doesn't become a new book.
An ISBN-10 weights its first nine digits 10, 9, ..., 2, and its check digit
brings the sum to a multiple of 11; a check digit of 10 is written `X`.
For `0-8044-2957-X` the nine digits give 199, `199 mod 11 = 1`, and
`(11 - 1) mod 11 = 10`, so the last character must be `X`:

<!-- source: coax/lib/nostr/nip73.coax isbn -->
```jai
// An ISBN without hyphens, its check digit checked: ISBN-13 digits weighted
// 1, 3, 1, 3, ... sum to a multiple of 10; ISBN-10 digits weighted 10, 9,
// ..., 1 sum to a multiple of 11, where a last X is 10.
n73_isbn :: (id:N73Id,body:string)->N73Id {
  r:=id;n:=0;last:int;sum13:=0;sum10:=0;
  for i:0..<body.count {
    c:=n73_upper(body[i]);
    if c=='-' {continue;}
    x:=c=='X' && n==9 && i==body.count-1;
    if !n73_digit(c) && !x {return n73_fail(r,N73_SHAPE);}
    last=ifx x then 10 else cast(int)(c-'0');
    if n<12 {sum13+=last*(ifx n%2==0 then 1 else 3);}   // every digit but the check digit
    if n<9 {sum10+=last*(10-n);}
    n+=1;r=n73_put(r,c);
  }
  // The check digit is the one that makes the sum come out even.
  if n==13 {if (10-sum13%10)%10!=last {return n73_fail(r,N73_CHECK);}}
  else if n==10 {if (11-sum10%11)%11!=last {return n73_fail(r,N73_CHECK);}}
  else {return n73_fail(r,N73_SHAPE);}
  return n73_fail(r,N73_OK);
}
```

### URLs

NIP-73's normal form for a URL is "normalized, no fragment". The code
lower-cases the parts that are case-insensitive, the scheme and the host,
adds the `/` that an empty path means, and drops everything from `#`:
`HTTPS://MyBlog.Example.com/post?q=1#comments` becomes
`https://myblog.example.com/post?q=1`. The path and query keep their case,
because a server may treat `/Post` and `/post` as different pages.
Percent-encoding (`%7E` and `~`) and default ports (`:443`) are not
normalized.

<!-- source: coax/lib/nostr/nip73.coax web -->
```jai
// A URL: the scheme and host in lower case, a / for an empty path, and no
// fragment.
n73_web :: (id:N73Id,raw:string)->N73Id {
  r:=id;i:=0;
  while raw[i]!=':' {r=n73_put(r,n73_lower(raw[i]));i+=1;}
  for k:0..<3 {r=n73_put(r,raw[i]);i+=1;}           // ://
  host:=i;
  while i<raw.count && raw[i]!='/' && raw[i]!='?' && raw[i]!='#' {r=n73_put(r,n73_lower(raw[i]));i+=1;}
  if i==host {return n73_fail(r,N73_SHAPE);}
  if i==raw.count || raw[i]!='/' {
    if r.value.n==NT_MAX_VALUE {return n73_fail(r,N73_LIMIT);}
    r=n73_put(r,'/');
  }
  while i<raw.count && raw[i]!='#' {
    if r.value.n==NT_MAX_VALUE {return n73_fail(r,N73_LIMIT);}
    r=n73_put(r,raw[i]);i+=1;
  }
  return n73_fail(r,N73_OK);
}
```

### Reading an event's tags

`n73_tags` reads every `i` and `k` tag. It refuses an `i` tag of a type it
doesn't know, one that is not already in normal form (a client that wrote
it should have normalized it first) and a `k` tag that names no type. It
counts URL hints, the optional second value, and reports in `missing_k`
the types that have an `i` tag but no `k` tag, since NIP-73 pairs them but
NIP-84 highlights, for one, carry an `i` tag alone.

<!-- source: coax/lib/nostr/nip73.coax tags -->
```jai
// An event's i and k tags: every i tag in normal form and of a known type,
// every k tag a known type, and the i types that have no k tag.
n73_tags :: (e:NostrEvent)->N73Tags {
  r:N73Tags;if e.error!=NE_OK {return r;}
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n73_refuse(r,N73_LIMIT,i);}
    if t.error!=NT_OK {return n73_refuse(r,N73_EVENT,i);}
    v:=t.v[1];value:=cast(string)v.b[0..<v.n];
    if n32_is(t.v[0],"i") {
      id:=n73_normalize(value);
      if id.status!=N73_OK {return n73_refuse(r,id.status,i);}
      if !n32_is(id.value,value) {return n73_refuse(r,N73_NORMAL,i);}
      if r.ids==0 {r.first=id;}
      r.ids+=1;r.i_types|=1<<id.type;
      if t.v[2].n>0 {r.hints+=1;}
    } else if n32_is(t.v[0],"k") {
      kind:=N73_NONE;
      for k:N73_WEB..<N73_TYPES {if value==N73_K[k] {kind=k;}}
      if kind==N73_NONE {return n73_refuse(r,ifx n73_chain_kind(value) then N73_CHAIN else N73_KIND,i);}
      r.kinds+=1;r.k_types|=1<<kind;
    }
    i+=1;
  }
  r.missing_k=r.i_types & (r.i_types ^ r.k_types);r.status=N73_OK;return r;
}
```

## Bridged events

A bridge copies posts from another network into Nostr, signing each with a
key it holds for the original author. NIP-48's `proxy` tag names the post
it came from, so that a client can link to it, or notice that two bridges
copied the same post and show it once:

```jai
["proxy", <id>, <protocol>]
```

NIP-48 lists four protocols. The id's form depends on the protocol:

| Protocol | Id | Example |
| --- | --- | --- |
| `activitypub` | an `http` or `https` URL | `https://gleasonator.com/objects/9f524868-c1a0-4ee7-ad51-aaa23d68b526` |
| `atproto` | an `at://` URI | `at://did:plc:zhbjlbmir5dganqhueg7y4i3/app.bsky.feed.post/3jt5hlibeol2i` |
| `rss` | a URL with the item's guid as the fragment | `https://soapbox.pub/rss/feed.xml#https%3A%2F%2Fsoapbox.pub%2Fblog%2Fmostr-fediverse-nostr-bridge` |
| `web` | an `http` or `https` URL | `https://twitter.com/jack/status/20` |

<!-- source: coax/lib/nostr/nip48.coax id -->
```jai
// Whether id has the form NIP-48 gives its protocol: an http or https URL
// with a host for activitypub and web, the same with a fragment (the item's
// guid) for rss, and an at:// URI for atproto. No spaces in any.
n48_id_ok :: (protocol:int,id:string)->bool {
  rest:="";
  if protocol==N48_ATPROTO {
    if !n48_starts(id,"at://") {return false;}
    rest=id[5..<id.count];
  } else if protocol<N48_ACTIVITYPUB || protocol>N48_WEB {return false;}
  else if n48_starts(id,"https://") {rest=id[8..<id.count];}
  else if n48_starts(id,"http://") {rest=id[7..<id.count];}
  else {return false;}
  // rest starts with the host (or the DID), never empty.
  if rest.count==0 || rest[0]=='/' || rest[0]=='?' || rest[0]=='#' {return false;}
  fragment:=false;
  for i:0..<rest.count {
    if rest[i]<=' ' {return false;}
    if rest[i]=='#' && i+1<rest.count {fragment=true;}
  }
  return protocol!=N48_RSS || fragment;
}
```

NIP-48 says its list may grow. A protocol it does not list yet is refused
with `N48_PROTOCOL`, so that a client never treats an id it can't
interpret as a link. A proxy tag is the signer's claim. It does not prove
that the source post exists or that it says what the event says, and
nothing here fetches it.

<!-- source: coax/lib/nostr/nip48.coax read -->
```jai
// An event's proxy tags: each with an id and a protocol NIP-48 lists, the
// id in that protocol's form.
n48_read :: (e:NostrEvent)->N48Proxy {
  p:N48Proxy;if e.error!=NE_OK {return p;}
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n48_refuse(p,N48_LIMIT,i);}
    if t.error!=NT_OK {return n48_refuse(p,N48_EVENT,i);}
    if n32_is(t.v[0],"proxy") {
      if t.values<3 {return n48_refuse(p,N48_SHAPE,i);}
      v:=t.v[1];w:=t.v[2];
      kind:=n48_protocol(cast(string)w.b[0..<w.n]);
      if kind==N48_NONE {return n48_refuse(p,N48_PROTOCOL,i);}
      if !n48_id_ok(kind,cast(string)v.b[0..<v.n]) {return n48_refuse(p,N48_ID,i);}
      if p.proxies==0 {p.protocol=kind;p.id=t.v[1];}
      p.proxies+=1;
    }
    i+=1;
  }
  p.status=N48_OK;return p;
}
```

## OpenTimestamps attestations

An event's `created_at` is whatever its signer wrote. OpenTimestamps
proves something weaker but harder to fake: that some data existed before
a Bitcoin block was mined. A calendar server collects many digests,
builds a merkle tree over them and puts the tree's root in a Bitcoin
transaction. A proof for one digest is the list of operations that turn
that digest into the transaction's id, and from there into the block's
merkle root.

NIP-03 wraps such a proof in an event:

```jai
{
  "kind": 1040,
  "tags": [["e", <target-event-id>, <relay-url>], ["k", "<target-event-kind>"]],
  "content": <base64-encoded OTS file data>
}
```

The proof must be about the `e` tag's event: the digest it starts from is
that event's id, which is itself a SHA-256 hash of the event (chapter
21). It should hold one Bitcoin attestation and no pending ones.

### The file format

An `.ots` file is a header and a tree. The constants:

<!-- source: coax/lib/nostr/nip03.coax format -->
```jai
// The magic: a zero byte, "OpenTimestamps", two zeros, "Proof", a zero
// and eight random bytes.
N03_HEADER :: [31]u8.[0x00, 0x4f, 0x70, 0x65, 0x6e, 0x54, 0x69, 0x6d, 0x65, 0x73, 0x74, 0x61, 0x6d, 0x70, 0x73, 0x00, 0x00, 0x50, 0x72, 0x6f, 0x6f, 0x66, 0x00, 0xbf, 0x89, 0xe2, 0xe8, 0x84, 0xe8, 0x92, 0x94];
N03_FORK :: 0xff; N03_ATTEST :: 0x00;
N03_SHA256 :: 0x08; N03_APPEND :: 0xf0; N03_PREPEND :: 0xf1;
N03_SHA1 :: 0x02; N03_RIPEMD160 :: 0x03; N03_KECCAK256 :: 0x67;
N03_REVERSE :: 0xf2; N03_HEXLIFY :: 0xf3;
N03_TAG_BITCOIN :: [8]u8.[0x05, 0x88, 0x96, 0x0d, 0x73, 0xd7, 0x19, 0x01];
N03_TAG_PENDING :: [8]u8.[0x83, 0xdf, 0xe3, 0x0d, 0x2e, 0xf9, 0x0c, 0x8e];
```

1. The header: 31 magic bytes, the version as a number (1), the hash
   operation that made the digest (`08`, sha256) and the 32-byte digest.
2. The tree. Each byte is a tag. An operation tag is followed by its
   argument, if it has one, and then by the rest of the tree from the new
   message. Attestation tag `00` is followed by an eight-byte attestation
   type and a payload, and ends a branch.
3. A fork: tag `ff` means "another branch follows this one, from the same
   message". The branch after `ff` runs to its attestation, then the next
   tag continues from the message at the fork.

Numbers, such as an argument's length and a block height, are unsigned
LEB128: seven bits a byte, low bits first, with the top bit set on every
byte but the last. 810391 is `0xc5d97`; its seven-bit groups from the
bottom are `0x17`, `0x3b` and `0x31`, so it is written `97 bb 31`.

<!-- source: coax/lib/nostr/nip03.coax varuint -->
```jai
// An unsigned LEB128 number: seven bits a byte, low bits first, the high
// bit set on every byte but the last. At most five bytes and 32 bits.
// -1: the proof ends inside it; -2: too long or too large.
n03_varuint :: (r:[]N03Read)->int {
  v:=0;
  for k:0..<5 {
    if r[0].at>=r[0].p.count {return -1;}
    b:=cast(int)r[0].p[r[0].at];r[0].at+=1;
    v|=(b&127)<<(7*k);
    if b<128 {return ifx v>0xffffffff then -2 else v;}
  }
  return -2;
}
```

### Worked example: replaying a proof by hand

The short proof in the example is 132 bytes. It proves the event NIP-03's
own example verifies, `e71c6ea7...c323`:

| Offset | Bytes | Meaning |
| --- | --- | --- |
| 0 | `004f70656e...e89294` | the magic |
| 31 | `01` | version 1 |
| 32 | `08` | the digest is a SHA-256 hash |
| 33 | `e71c6ea7...c323` | the digest, the event id |
| 65 | `f0 10 0123...cdef` | append these 16 bytes |
| 83 | `08` | sha256 |
| 84 | `f1 20 1111...1111` | prepend these 32 bytes |
| 118 | `08` | sha256 |
| 119 | `00 0588960d73d71901 03 97bb31` | Bitcoin attestation, block 810391 |

Replay it from the digest:

1. Start with the event id, 32 bytes:
   `e71c6ea722987debdb60f81f9ea4f604b5ac0664120dd64fb9d23abc4ec7c323`.
2. Append `0123456789abcdef0123456789abcdef`: the message is the id
   followed by those 16 bytes, 48 bytes. A calendar adds such a nonce so
   that the proof doesn't reveal the neighbouring digests.
3. SHA-256 of those 48 bytes:
   `16e47a8e06f2405557a7a5ce4232678c1474b58171e944f3d1bd4f9d3abb244f`.
4. Prepend 32 bytes of `11`: the sibling in the merkle tree goes first,
   because this message is the right child. 64 bytes.
5. SHA-256 of those 64 bytes:
   `1c3154ac118df43baa72f6410d0937b5fb8787b431526a55ad67f9882a9a91f9`.
6. The attestation says that this message is the merkle root of block
   810391.

Steps 3 and 5 are the only ones you can't do on paper; the example and
Python's `hashlib` give the same hashes. Every other step is copying bytes.
The proof is valid only if block 810391's header holds that root. A real
proof continues past step 5 through the calendar's transaction: prepend
the bytes before the commitment, append the bytes after it, sha256 twice
for the txid, then one append or prepend and two sha256 per level of the
block's merkle tree. The example's long proof does one level.

`n03_apply` is one step:

<!-- source: coax/lib/nostr/nip03.coax apply -->
```jai
// One operation on the message m: sha256, or append or prepend arg. False
// when the result would be too long.
n03_apply :: (m:[]N03Msg,op:int,arg:string)->bool {
  if op==N03_SHA256 {
    h:[32]u8;status:=sha256_hash_checked(m[0].b[0..<m[0].n],h[..],false);
    #assert status==SHA_OK;
    for i:0..<32 {m[0].b[i]=h[i];}
    m[0].n=32;return true;
  }
  if m[0].n+arg.count>N03_MAX_MSG {return false;}
  if op==N03_APPEND {for i:0..<arg.count {m[0].b[m[0].n+i]=arg[i];}}
  else {
    i:=m[0].n;while i>0 {i-=1;m[0].b[i+arg.count]=m[0].b[i];}
    for k:0..<arg.count {m[0].b[k]=arg[k];}
  }
  m[0].n+=arg.count;return true;
}
```

`n03_parse` reads the header and replays every branch. It keeps the
message at each open fork, at most eight at once, and reports each
attestation with the message it reached:

<!-- source: coax/lib/nostr/nip03.coax parse -->
```jai
// An .ots file: its header, then every branch of its tree replayed from the
// digest, each ending at an attestation.
n03_parse :: (proof:string)->N03Proof {
  p:N03Proof;r:[1]N03Read;r[0].p=proof;
  header:=N03_HEADER;
  if proof.count<header.count {return n03_refuse(p,N03_TRUNCATED,0);}
  if !n03_match(proof,0,header[..]) {return n03_refuse(p,N03_MAGIC,0);}
  r[0].at=header.count;
  version:=n03_varuint(r[..]);
  if version== -1 {return n03_refuse(p,N03_TRUNCATED,r[0].at);}
  if version!=1 {return n03_refuse(p,N03_VERSION,header.count);}
  if r[0].at>=proof.count {return n03_refuse(p,N03_TRUNCATED,r[0].at);}
  if proof[r[0].at]!=N03_SHA256 {return n03_refuse(p,N03_FILE_OP,r[0].at);}
  r[0].at+=1;
  if r[0].at+32>proof.count {return n03_refuse(p,N03_TRUNCATED,r[0].at);}
  m:[1]N03Msg;
  for i:0..<32 {p.digest[i]=proof[r[0].at+i];m[0].b[i]=p.digest[i];}
  m[0].n=32;r[0].at+=32;
  forks:[N03_MAX_FORKS]N03Msg;open:=0;   // the message at each open fork
  while true {
    at:=r[0].at;
    if at>=proof.count {return n03_refuse(p,N03_TRUNCATED,at);}
    op:=cast(int)proof[at];r[0].at+=1;
    if op==N03_FORK {
      // Another branch follows this one: save the message for it.
      if open==N03_MAX_FORKS {return n03_refuse(p,N03_LIMIT,at);}
      forks[open].n=m[0].n;for i:0..<m[0].n {forks[open].b[i]=m[0].b[i];}
      open+=1;at=r[0].at;
      if at>=proof.count {return n03_refuse(p,N03_TRUNCATED,at);}
      op=cast(int)proof[at];r[0].at+=1;
    }
    if op==N03_ATTEST {
      a:[1]N03Attestation;a[0].at=at;
      status:=n03_attestation(r[..],m[0],a[..]);
      if status!=N03_OK {return n03_refuse(p,status,at);}
      if p.n==N03_MAX_ATTEST {return n03_refuse(p,N03_LIMIT,at);}
      p.attestations[p.n]=a[0];p.n+=1;
      if a[0].type==N03_BITCOIN {p.bitcoin+=1;} else if a[0].type==N03_PENDING {p.pending+=1;} else {p.unknown+=1;}
      if open==0 {break;}
      // The branch ends; the next one starts from the fork's message.
      open-=1;m[0].n=forks[open].n;for i:0..<m[0].n {m[0].b[i]=forks[open].b[i];}
      continue;
    }
    arg:="";
    if op==N03_APPEND || op==N03_PREPEND {
      size:=n03_varuint(r[..]);
      if size== -1 {return n03_refuse(p,N03_TRUNCATED,r[0].at);}
      if size<1 || size>N03_MAX_MSG {return n03_refuse(p,N03_LIMIT,at+1);}
      if r[0].at+size>proof.count {return n03_refuse(p,N03_TRUNCATED,r[0].at);}
      arg=proof[r[0].at..<r[0].at+size];r[0].at+=size;
    } else if op==N03_SHA1 || op==N03_RIPEMD160 || op==N03_KECCAK256 || op==N03_REVERSE || op==N03_HEXLIFY {
      return n03_refuse(p,N03_UNSUPPORTED,at);
    } else if op!=N03_SHA256 {return n03_refuse(p,N03_OP,at);}
    if !n03_apply(m[..],op,arg) {return n03_refuse(p,N03_LIMIT,at);}
    p.ops+=1;
  }
  if r[0].at!=proof.count {return n03_refuse(p,N03_TRAILING,r[0].at);}
  p.status=N03_OK;return p;
}
```

Its refusals, each with the byte where it stopped:

| Status | When |
| --- | --- |
| `N03_MAGIC` | the first 31 bytes are not the magic |
| `N03_VERSION` | the version is not 1 |
| `N03_FILE_OP` | the digest is not a SHA-256 hash |
| `N03_TRUNCATED` | the proof ends inside the header, a number, an argument or an attestation, or a branch ends without one |
| `N03_OP` | a tag that is no operation, such as `09`, or `ff` twice |
| `N03_UNSUPPORTED` | sha1 (`02`), ripemd160 (`03`), keccak256 (`67`), reverse (`f2`) or hexlify (`f3`): real operations this chapter does not build |
| `N03_LIMIT` | an argument of 0 or over 4096 bytes, a message over 4096 bytes, a payload over 8192 bytes, more than 8 open forks or 16 attestations, a number over 32 bits |
| `N03_ATTESTATION` | a Bitcoin payload that is not exactly a height, or a Bitcoin attestation on a message that is not 32 bytes |
| `N03_TRAILING` | bytes after the tree |

### The event

`n03_event` reads the `e` tag, the optional `k` tag (a decimal kind),
decodes the content from base64 and parses the proof. Done means: a proof
that does not start from the `e` tag's id is refused with `N03_DIGEST`,
and a proof with no Bitcoin attestation with `N03_NO_BITCOIN`.

<!-- source: coax/lib/nostr/nip03.coax event -->
```jai
// A kind 1040 event: its e tag, its k tag, and its content decoded into buf
// and parsed. The proof must be about the e tag's event and hold at least
// one Bitcoin attestation.
n03_event :: (e:NostrEvent,buf:[]u8)->N03Event {
  a:N03Event;if e.error!=NE_OK {return a;}
  if e.kind!=N03_KIND_ATTESTATION {a.status=N03_KIND;return a;}
  found:=false;i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error!=NT_OK {break;}
    if n32_is(t.v[0],"e") && !found {
      s:=nostr_tag(e,i);
      if s.error!=NT_OK {a.tag_error=s.error;a.status=N03_TAG;return a;}
      found=true;a.target=s.id;
    }
    if n32_is(t.v[0],"k") && a.kind<0 {
      v:=t.v[1];d:=n03_kind(cast(string)v.b[0..<v.n]);
      if d<0 {a.status=N03_TAG;return a;}
      a.kind=d;
    }
    i+=1;
  }
  if n32_tag(e,i).error!=NT_INDEX {return a;}
  if !found {a.status=N03_TAG;return a;}
  if e.content_from<0 || e.content_to>e.original.count || e.content_from>e.content_to {return a;}
  text:=e.original[e.content_from..<e.content_to];
  size:=base64_decode_size(text);
  if size<0 {a.status=N03_BASE64;return a;}
  if size>buf.count {a.status=N03_LIMIT;return a;}
  r:=base64_decode_checked(text,buf,false);
  #assert r.error==NC_OK;
  a.proof=n03_parse(cast(string)buf[0..<r.written]);
  if a.proof.status!=N03_OK {a.status=a.proof.status;return a;}
  if !ns_equal32(a.proof.digest,a.target) {a.status=N03_DIGEST;return a;}
  if a.proof.bitcoin==0 {a.status=N03_NO_BITCOIN;return a;}
  a.status=N03_OK;return a;
}
```

### What is checked and what is not

When `n03_event` returns `N03_OK`, these hold: the proof is well formed,
it starts from the event's id, and replaying it with SHA-256 reaches the
reported root at each Bitcoin attestation. That is all. The proof is only
evidence once the caller has the block header at that height and finds
the root in its merkle root field. The header stores it in its own byte
order, which is the reverse of how block explorers print it: the example
prints both. Fetching headers, and checking that the header belongs to the
chain with the most work, needs a Bitcoin node or a header chain, which
this stack does not have yet.

Without that check, anyone can write a proof that replays to a root of
their choosing: the example flips one bit in the short proof's sibling and
the proof still parses, to a different root. The parse can't tell the two
apart; only the header can.

The pinned NIP-03 text carries the warning `unrecommended`: "vulnerable to
one specific attack, needs update". It does not say which attack. Treat a
timestamp as evidence about when an event existed at the latest, never as
proof of who wrote it or of when it was first written.

## The example

The example is
[nostr/examples/bridges-timestamps.coax](../examples/bridges-timestamps.coax).
It normalizes ten written identifiers and refuses five. It signs four
events with the published test keys: Alice's note with `i` and `k` tags
for a book and a web page, Bob's bridged note with NIP-48's own proxy
tag, and two kind 1040 attestations by Carol carrying the long proof: one
naming the event the proof is about, one naming Alice's note. It replays
the short proof one operation at a time, then parses both proofs. The
other refusals use unsigned events and edited proofs.

<!-- source: nostr/examples/bridges-timestamps.coax example -->
```jai
#load "../../coax/lib/nostr/nip73.coax";
#load "../../coax/lib/nostr/nip48.coax";
#load "../../coax/lib/nostr/nip03.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

IDS :: [9] string.["ok", "event", "unknown type", "bad shape", "bad check digit", "not Bitcoin", "not in normal form", "unknown k", "limit"];
PROXY :: [6] string.["ok", "event", "bad shape", "unknown protocol", "bad id", "limit"];
OTS :: [16] string.["ok", "not an .ots file", "unknown version", "file hash not sha256", "truncated", "unknown operation", "unsupported operation", "limit", "trailing bytes", "bad attestation", "event", "not kind 1040", "bad e or k tag", "bad base64", "proof is not about this event", "no Bitcoin attestation"];
ATTESTATION :: [4] string.["", "Bitcoin", "pending", "unknown"];
// BIP-340 test vectors 0 to 2's public keys and their secret keys:
// published test values. Never sign with a real key in an example.
PEOPLE :: [3] string.["Alice", "Bob", "Carol"];
PUB :: [3] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
  "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"];
SECRET :: [3] string.[
  "0000000000000000000000000000000000000000000000000000000000000003",
  "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef",
  "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9"];
ALICE :: 0; BOB :: 1; CAROL :: 2;
T0 :: 1761686400;

// The event NIP-03's example verifies, e71c6ea7..., and two proofs for it
// built by hand (the chapter's worked example and its "How the code is
// checked" section give them byte by byte). SHORT: append NONCE, sha256,
// prepend SIBLING, sha256, Bitcoin attestation at 810391. LONG: append,
// sha256, then a fork: a pending attestation, or a transaction around the
// message, its id, one merkle step and the Bitcoin attestation.
TARGET :: "e71c6ea722987debdb60f81f9ea4f604b5ac0664120dd64fb9d23abc4ec7c323";
NONCE :: "0123456789abcdef0123456789abcdef";
SIBLING :: "1111111111111111111111111111111111111111111111111111111111111111";
// SHORT in parts: the header (31 bytes), version 01, sha256 08, the
// target; f0 10 NONCE; 08; f1 20 SIBLING; 08; 00, Bitcoin's tag
// 0588960d73d71901, a 3-byte payload 03 holding 810391 as 97bb31.
SHORT :: "004f70656e54696d657374616d7073000050726f6f6600bf89e2e884e892940108e71c6ea722987debdb60f81f9ea4f604b5ac0664120dd64fb9d23abc4ec7c323f0100123456789abcdef0123456789abcdef08f120111111111111111111111111111111111111111111111111111111111111111108000588960d73d719010397bb31";
LONG :: "004f70656e54696d657374616d7073000050726f6f6600bf89e2e884e892940108e71c6ea722987debdb60f81f9ea4f604b5ac0664120dd64fb9d23abc4ec7c323f0100123456789abcdef0123456789abcdef08ff0083dfe30d2ef90c8e2e2d68747470733a2f2f616c6963652e6274632e63616c656e6461722e6f70656e74696d657374616d70732e6f7267f10a01000000010000000000f005ffffffff000808f02022222222222222222222222222222222222222222222222222222222222222220808000588960d73d719010397bb31";

// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) -> string {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<r.written];
}
show_hex :: (bytes: [] u8) {
  text: [8192] u8; r := hex_encode_checked(cast(string) bytes, text[..], false);
  #assert r.error == NC_OK;
  print("%", cast(string) text[0..<r.written]);
}
say :: (t: N32Text) { own := t; print("%", cast(string) own.b[0..<own.n]); }

// Sign an event into out and read it back with its ID and signature checked.
sign :: (who: int, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  key: [32] u8; r := decode_hex(SECRET[who], key[..]);
  aux: [32] u8; work: [1] NostrSignWork; scratch: [8192] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [8192] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified;
  return e;
}
// An unsigned event with these tags, parsed but not verified: its id and
// sig are zeros. Used for the refusals, which no signer here would write.
unsigned :: (kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"");
  n = json_raw(out, n, PUB[BOB]); n = json_raw(out, n, "\",\"created_at\":1761686400,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":\"\",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

show_tags :: (label: string, e: NostrEvent) {
  t := n73_tags(e);
  print("%: %", label, IDS[t.status]);
  if t.at >= 0 { print(" at tag %", t.at); }
  if t.status != N73_OK { print("\n"); return; }
  print(", i tags %, k tags %, first ", t.ids, t.kinds); say(t.first.value);
  print(" (k %), i types without a k tag %\n", N73_K[t.first.type], t.missing_k);
}
show_proxy :: (label: string, e: NostrEvent) {
  p := n48_read(e);
  print("%: %", label, PROXY[p.status]);
  if p.at >= 0 { print(" at tag %", p.at); }
  if p.status != N48_OK { print("\n"); return; }
  if p.proxies == 0 { print(", not bridged\n"); return; }
  print(", % proxy tag, from % ", p.proxies, N48_PROTOCOLS[p.protocol]); say(p.id); print("\n");
}
show_proof :: (label: string, p: N03Proof) {
  print("%: %", label, OTS[p.status]);
  if p.at >= 0 { print(" at byte %", p.at); }
  if p.status != N03_OK { print("\n"); return; }
  print(", % operations\n", p.ops);
  for k: 0..<p.n {
    a := p.attestations[k];
    print("  % attestation at byte %", ATTESTATION[a.type], a.at);
    if a.type == N03_BITCOIN {
      print(": block %, merkle root ", a.height); show_hex(a.root[..]);
      // A block explorer prints the root in reverse byte order.
      rev: [32] u8; for i: 0..<32 { rev[i] = a.root[31 - i]; }
      print("\n    as an explorer prints it "); show_hex(rev[..]);
    }
    print("\n");
  }
}
show_attestation :: (label: string, e: NostrEvent) {
  buf: [1024] u8; a := n03_event(e, buf[..]);
  print("%: %", label, OTS[a.status]);
  if a.status != N03_OK { print("\n"); return; }
  print(", kind %, target ", a.kind); show_hex(a.target[0..<4]);
  print("..., Bitcoin attestations %, pending %\n", a.proof.bitcoin, a.proof.pending);
}

// book:begin trace
// The short proof replayed one operation at a time, as the worked example
// does by hand.
trace :: () {
  m: [1] N03Msg; b: [64] u8;
  id := decode_hex(TARGET, b[..]); for i: 0..<32 { m[0].b[i] = id[i]; } m[0].n = 32;
  print("start, the event id (32 bytes)\n  "); show_hex(m[0].b[0..<m[0].n]); print("\n");
  ops := [4] int.[N03_APPEND, N03_SHA256, N03_PREPEND, N03_SHA256];
  args := [4] string.[NONCE, "", SIBLING, ""];
  names := [4] string.["append", "sha256", "prepend", "sha256"];
  for k: 0..<4 {
    a: [32] u8; arg := decode_hex(args[k], a[..]);
    done := n03_apply(m[..], ops[k], arg);
    #assert done;
    print("% ", names[k]); if arg.count > 0 { print("% ", args[k]); }
    print("(% bytes)\n  ", m[0].n); show_hex(m[0].b[0..<m[0].n]); print("\n");
  }
}
// book:end trace

main :: () {
  print("NIP-73: written forms and their normal forms\n");
  written := [10] string.["isbn:978-0-7653-8203-0", "geo:EZS42E44YX96", "iso3166:us-ca", "isan:0000-0000-401A-0000-7-0000-0000-6",
    "doi:10.1000/XYZ123", "#Nostr", "HTTPS://MyBlog.Example.com/post?q=1#comments", "http://Example.com",
    "bitcoin:tx:A1075DB55D416D3CA199F55B6084E2115B9345E16C5CF302FC80E9D5FBF5D48D", "podcast:guid:C90E609A-DF1E-596A-BD5E-57BCC8AAD6CC"];
  for k: 0..<written.count {
    id := n73_normalize(written[k]);
    print("% -> ", written[k]); say(id.value); print(" (k %)\n", N73_K[id.type]);
  }
  refused := [5] string.["isbn:978-0-7653-8203-1", "imdb:tt0120737", "ethereum:1:tx:0x98f7812be496f97f80e2e98d66358d1fc733cf34176a8356d171ea7fbbe97ccd", "geo:ezs42a", "ftp://example.com/file"];
  for k: 0..<refused.count { print("% -> %\n", refused[k], IDS[n73_normalize(refused[k]).status]); }

  print("signed events\n");
  buf: [4][8192] u8; tags: [1024] u8; b64: [1024] u8; proof: [1024] u8;
  n := alice_note_tags(tags[..]);
  note := sign(ALICE, T0, 1, cast(string) tags[0..<n], "Reading this.", buf[0][..]);
  bridged := sign(BOB, T0 + 10, 1, "[[\"proxy\",\"https://gleasonator.com/objects/8f6fac53-4f66-4c6e-ac7d-92e5e78c3e79\",\"activitypub\"]]", "I'm vegan btw", buf[1][..]);
  long := decode_hex(LONG, proof[..]);
  r := base64_encode_checked(long, b64[..], false); #assert r.error == NC_OK;
  content := cast(string) b64[0..<r.written];
  n = attest_tags(TARGET, tags[..]);
  attest := sign(CAROL, T0 + 20, 1040, cast(string) tags[0..<n], content, buf[2][..]);
  hex: [64] u8; text := hex_encode_checked(cast(string) note.id[..], hex[..], false);
  n = attest_tags(cast(string) hex[0..<64], tags[..]);
  wrong := sign(CAROL, T0 + 30, 1040, cast(string) tags[0..<n], content, buf[3][..]);
  show_tags("N, Alice's note", note);
  show_proxy("B, Bob's bridged note", bridged);
  show_proxy("N", note);
  print("O, Carol's kind 1040, content % bytes of base64, % bytes of proof\n", r.written, long.count);
  show_attestation("O", attest);

  print("NIP-03: the short proof replayed by hand\n");
  trace();
  short: [256] u8; s := decode_hex(SHORT, short[..]);
  show_proof("the short proof parsed", n03_parse(s));
  show_proof("the long proof parsed", n03_parse(long));

  print("refusals\n");
  bad: [2048] u8;
  show_tags("i tag with an unknown type", unsigned(1, "[[\"i\",\"imdb:tt0120737\"],[\"k\",\"imdb\"]]", bad[..]));
  show_tags("i tag with hyphens in its ISBN", unsigned(1, "[[\"i\",\"isbn:978-0-7653-8203-0\"]]", bad[..]));
  show_tags("i tag naming an Ethereum address", unsigned(1, "[[\"i\",\"ethereum:1:address:0xd8da6bf26964af9d7eed9e03e53415d37aa96045\"]]", bad[..]));
  show_proxy("proxy tag with protocol nntp", unsigned(1, "[[\"proxy\",\"news:comp.lang.c\",\"nntp\"]]", bad[..]));
  show_proxy("proxy tag without a protocol", unsigned(1, "[[\"proxy\",\"https://x.example/1\"]]", bad[..]));
  edit: [256] u8; for i: 0..<s.count { edit[i] = s[i]; }
  edit[83] = 0x09; show_proof("short proof, byte 83 set to 0x09", n03_parse(cast(string) edit[0..<s.count]));
  edit[83] = N03_RIPEMD160; show_proof("short proof, byte 83 set to ripemd160", n03_parse(cast(string) edit[0..<s.count]));
  show_proof("short proof cut to 100 bytes", n03_parse(s[0..<100]));
  edit[83] = N03_SHA256; edit[100] ^= 1;
  p := n03_parse(cast(string) edit[0..<s.count]);
  print("short proof, one bit of the sibling flipped: %, root ", OTS[p.status]); show_hex(p.attestations[0].root[0..<4]); print("..., not 1c3154ac...\n");
  show_attestation("W, Carol's kind 1040 naming Alice's note", wrong);
}

alice_note_tags :: (out: [] u8) -> int {
  return json_raw(out, 0, "[[\"i\",\"isbn:9780765382030\"],[\"k\",\"isbn\"],[\"i\",\"https://myblog.example.com/post/2012-03-27/hello-world\"],[\"k\",\"web\"]]");
}
attest_tags :: (id: string, out: [] u8) -> int {
  n := json_raw(out, 0, "[[\"e\",\""); n = json_raw(out, n, id);
  return json_raw(out, n, "\",\"wss://nostr-pub.wellorder.net\"],[\"k\",\"1\"]]");
}
```

Run it:

```shell
coax/bin/coax run nostr/examples/bridges-timestamps.coax --run-steps 250000000
```

The output, which the book's checks compare in debug and release
([fixture](../fixtures/bridges-timestamps.out)):

```text
NIP-73: written forms and their normal forms
isbn:978-0-7653-8203-0 -> isbn:9780765382030 (k isbn)
geo:EZS42E44YX96 -> geo:ezs42e44yx96 (k geo)
iso3166:us-ca -> iso3166:US-CA (k iso3166)
isan:0000-0000-401A-0000-7-0000-0000-6 -> isan:0000-0000-401A-0000-7 (k isan)
doi:10.1000/XYZ123 -> doi:10.1000/xyz123 (k doi)
#Nostr -> #nostr (k #)
HTTPS://MyBlog.Example.com/post?q=1#comments -> https://myblog.example.com/post?q=1 (k web)
http://Example.com -> http://example.com/ (k web)
bitcoin:tx:A1075DB55D416D3CA199F55B6084E2115B9345E16C5CF302FC80E9D5FBF5D48D -> bitcoin:tx:a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d (k bitcoin:tx)
podcast:guid:C90E609A-DF1E-596A-BD5E-57BCC8AAD6CC -> podcast:guid:c90e609a-df1e-596a-bd5e-57bcc8aad6cc (k podcast:guid)
isbn:978-0-7653-8203-1 -> bad check digit
imdb:tt0120737 -> unknown type
ethereum:1:tx:0x98f7812be496f97f80e2e98d66358d1fc733cf34176a8356d171ea7fbbe97ccd -> not Bitcoin
geo:ezs42a -> bad shape
ftp://example.com/file -> unknown type
signed events
N, Alice's note: ok, i tags 2, k tags 2, first isbn:9780765382030 (k isbn), i types without a k tag 0
B, Bob's bridged note: ok, 1 proxy tag, from activitypub https://gleasonator.com/objects/8f6fac53-4f66-4c6e-ac7d-92e5e78c3e79
N: ok, not bridged
O, Carol's kind 1040, content 284 bytes of base64, 211 bytes of proof
O: ok, kind 1, target e71c6ea7..., Bitcoin attestations 1, pending 1
NIP-03: the short proof replayed by hand
start, the event id (32 bytes)
  e71c6ea722987debdb60f81f9ea4f604b5ac0664120dd64fb9d23abc4ec7c323
append 0123456789abcdef0123456789abcdef (48 bytes)
  e71c6ea722987debdb60f81f9ea4f604b5ac0664120dd64fb9d23abc4ec7c3230123456789abcdef0123456789abcdef
sha256 (32 bytes)
  16e47a8e06f2405557a7a5ce4232678c1474b58171e944f3d1bd4f9d3abb244f
prepend 1111111111111111111111111111111111111111111111111111111111111111 (64 bytes)
  111111111111111111111111111111111111111111111111111111111111111116e47a8e06f2405557a7a5ce4232678c1474b58171e944f3d1bd4f9d3abb244f
sha256 (32 bytes)
  1c3154ac118df43baa72f6410d0937b5fb8787b431526a55ad67f9882a9a91f9
the short proof parsed: ok, 4 operations
  Bitcoin attestation at byte 119: block 810391, merkle root 1c3154ac118df43baa72f6410d0937b5fb8787b431526a55ad67f9882a9a91f9
    as an explorer prints it f9919a2a88f967ad556a5231b48787fbb537090d41f672aa3bf48d11ac54311c
the long proof parsed: ok, 9 operations
  pending attestation at byte 85
  Bitcoin attestation at byte 198: block 810391, merkle root 83da99158d06a048a526346b27ce2daeebf6081cbe76820135613bd8ca8fb40c
    as an explorer prints it 0cb48fcad83b6135018276be1c08f6ebae2dce276b3426a548a0068d1599da83
refusals
i tag with an unknown type: unknown type at tag 0
i tag with hyphens in its ISBN: not in normal form at tag 0
i tag naming an Ethereum address: not Bitcoin at tag 0
proxy tag with protocol nntp: unknown protocol at tag 0
proxy tag without a protocol: bad shape at tag 0
short proof, byte 83 set to 0x09: unknown operation at byte 83
short proof, byte 83 set to ripemd160: unsupported operation at byte 83
short proof cut to 100 bytes: truncated at byte 86
short proof, one bit of the sibling flipped: ok, root daed0701..., not 1c3154ac...
W, Carol's kind 1040 naming Alice's note: proof is not about this event
```

Reading the output:

- The trace prints each message in full, and the last one is the root
  that the parsed short proof reports at block 810391.
- `short proof cut to 100 bytes: truncated at byte 86`: the cut falls
  inside the prepend's argument, which starts at byte 86.
- `one bit of the sibling flipped: ok`: the proof still parses, to root
  `daed0701...`. Only a block header can tell this proof from the real
  one.
- `W, Carol's kind 1040 naming Alice's note: proof is not about this
  event`: the same proof under another `e` tag is refused, because its
  digest is `e71c6ea7...`, not Alice's note's id.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/bridges-timestamps.coax](../examples/checks/bridges-timestamps.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- Each message of the short proof's replay, both proofs' merkle roots, and
  the long proof in base64, from a Python script written from the `.ots`
  format with `hashlib` and `base64`. The proofs themselves were built by
  that script, byte by byte. No proof from the OpenTimestamps project is
  pinned in this repository, and none was downloaded for this chapter.
- Done means: the long proof under three other event ids (the last bit
  changed, one bit of the first byte changed, all zeros) refuses with `N03_DIGEST`,
  and under its own id reads.
- Done means: every identifier NIP-73 prints is already in its normal
  form, and NIP-73's URL-hint example reads with one hint. The ISBN check
  digit by hand.
- NIP-48's four example ids read under their protocols.

<!-- source: nostr/examples/checks/bridges-timestamps.coax check -->
```jai
// The values from a Python script written from the .ots format and
// hashlib (it shares no code with Coax): each message of the short proof's
// replay, both proofs' merkle roots, and the long proof in base64.
PY_STEPS :: [4] string.[
  "e71c6ea722987debdb60f81f9ea4f604b5ac0664120dd64fb9d23abc4ec7c3230123456789abcdef0123456789abcdef",
  "16e47a8e06f2405557a7a5ce4232678c1474b58171e944f3d1bd4f9d3abb244f",
  "111111111111111111111111111111111111111111111111111111111111111116e47a8e06f2405557a7a5ce4232678c1474b58171e944f3d1bd4f9d3abb244f",
  "1c3154ac118df43baa72f6410d0937b5fb8787b431526a55ad67f9882a9a91f9"];
PY_SHORT_ROOT :: "1c3154ac118df43baa72f6410d0937b5fb8787b431526a55ad67f9882a9a91f9";
PY_LONG_ROOT :: "83da99158d06a048a526346b27ce2daeebf6081cbe76820135613bd8ca8fb40c";
PY_LONG_B64 :: "AE9wZW5UaW1lc3RhbXBzAABQcm9vZgC/ieLohOiSlAEI5xxupyKYfevbYPgfnqT2BLWsBmQSDdZPudI6vE7HwyPwEAEjRWeJq83vASNFZ4mrze8I/wCD3+MNLvkMji4taHR0cHM6Ly9hbGljZS5idGMuY2FsZW5kYXIub3BlbnRpbWVzdGFtcHMub3Jn8QoBAAAAAQAAAAAA8AX/////AAgI8CAiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIggIAAWIlg1z1xkBA5e7MQ==";

is_hex :: (bytes: [] u8, hex: string) -> bool {
  b: [256] u8; want := decode_hex(hex, b[..]);
  return cast(string) bytes == want;
}

#test "the replay matches Python's, step by step" {
  m: [1] N03Msg; b: [64] u8;
  id := decode_hex(TARGET, b[..]); for i: 0..<32 { m[0].b[i] = id[i]; } m[0].n = 32;
  ops := [4] int.[N03_APPEND, N03_SHA256, N03_PREPEND, N03_SHA256];
  args := [4] string.[NONCE, "", SIBLING, ""];
  for k: 0..<4 {
    a: [32] u8; arg := decode_hex(args[k], a[..]);
    #assert n03_apply(m[..], ops[k], arg) && is_hex(m[0].b[0..<m[0].n], PY_STEPS[k]);
  }
}
#test "both proofs parse to Python's roots" {
  b: [256] u8; s := n03_parse(decode_hex(SHORT, b[..]));
  #assert s.status == N03_OK && s.n == 1 && s.attestations[0].height == 810391 && is_hex(s.attestations[0].root[..], PY_SHORT_ROOT);
  #assert is_hex(s.digest[..], TARGET);
  l := n03_parse(decode_hex(LONG, b[..]));
  #assert l.status == N03_OK && l.n == 2 && l.pending == 1 && l.attestations[1].height == 810391 && is_hex(l.attestations[1].root[..], PY_LONG_ROOT);
  t: [512] u8; r := base64_encode_checked(decode_hex(LONG, b[..]), t[..], false);
  #assert r.error == NC_OK && cast(string) t[0..<r.written] == PY_LONG_B64;
}
// Done means: an OTS proof that does not commit to the event ID refuses.
#test "the proof refuses under any other event id" {
  out: [2048] u8; buf: [1024] u8; tags: [256] u8;
  ids := [3] string.[
    "e71c6ea722987debdb60f81f9ea4f604b5ac0664120dd64fb9d23abc4ec7c322",
    "f71c6ea722987debdb60f81f9ea4f604b5ac0664120dd64fb9d23abc4ec7c323",
    "0000000000000000000000000000000000000000000000000000000000000000"];
  for k: 0..<3 {
    n := attest_tags(ids[k], tags[..]);
    e := kind1040(cast(string) tags[0..<n], PY_LONG_B64, out[..]);
    a := n03_event(e, buf[..]);
    #assert a.status == N03_DIGEST && a.proof.status == N03_OK;
  }
  n := attest_tags(TARGET, tags[..]);
  #assert n03_event(kind1040(cast(string) tags[0..<n], PY_LONG_B64, out[..]), buf[..]).status == N03_OK;
}
// Done means: NIP-73's examples normalize to their stated forms. Each
// identifier as NIP-73 prints it is already in normal form, and the
// written forms in the example become the NIP's.
#test "NIP-73's examples are their own normal forms" {
  nip := [13] string.["https://myblog.example.com/post/2012-03-27/hello-world", "geo:ezs42e44yx96", "iso3166:VE",
    "iso3166:US-CA", "isbn:9780765382030", "podcast:guid:c90e609a-df1e-596a-bd5e-57bcc8aad6cc",
    "podcast:item:guid:d98d189b-dc7b-45b1-8720-d4b98690f31f", "podcast:publisher:guid:18bcbf10-6701-4ffb-b255-bc057390d738",
    "isan:0000-0000-401A-0000-7", "bitcoin:tx:a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d",
    "bitcoin:address:1HQ3Go3ggs8pFnXuHVHRytPCq5fGG8Hbhx", "#nostr", "doi:10.1000/xyz123"];
  for k: 0..<nip.count { id := n73_normalize(nip[k]); #assert id.status == N73_OK && n32_is(id.value, nip[k]); }
  b: [1024] u8;
  e := unsigned(1, "[[\"i\",\"bitcoin:tx:a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d\"],[\"k\",\"bitcoin:tx\"],[\"i\",\"podcast:item:guid:d98d189b-dc7b-45b1-8720-d4b98690f31f\",\"https://fountain.fm/episode/z1y9TMQRuqXl2awyrQxg\"]]", b[..]);
  t := n73_tags(e); #assert t.status == N73_OK && t.ids == 2 && t.hints == 1 && t.missing_k == (1 << N73_PODCAST_ITEM);
  // The ISBN check digit by hand: 9780765382030's first twelve digits
  // weighted 1, 3, 1, 3, ... sum to 100, and (10 - 100 % 10) % 10 = 0.
  #assert n73_normalize("isbn:9780765382030").status == N73_OK && n73_normalize("isbn:9780765382039").status == N73_CHECK;
}
#test "NIP-48's examples read" {
  b: [1024] u8;
  rows := [4] string.["[[\"proxy\",\"https://gleasonator.com/objects/9f524868-c1a0-4ee7-ad51-aaa23d68b526\",\"activitypub\"]]",
    "[[\"proxy\",\"at://did:plc:zhbjlbmir5dganqhueg7y4i3/app.bsky.feed.post/3jt5hlibeol2i\",\"atproto\"]]",
    "[[\"proxy\",\"https://soapbox.pub/rss/feed.xml#https%3A%2F%2Fsoapbox.pub%2Fblog%2Fmostr-fediverse-nostr-bridge\",\"rss\"]]",
    "[[\"proxy\",\"https://twitter.com/jack/status/20\",\"web\"]]"];
  for k: 0..<4 { p := n48_read(unsigned(1, rows[k], b[..])); #assert p.status == N48_OK && p.protocol == k + 1 && p.proxies == 1; }
}

// A kind 1040 event with these tags and this content, parsed but not
// verified.
kind1040 :: (tags: string, content: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, tags); n = json_write_string(out, n, content);
  e: NostrEvent; e.error = NE_OK; e.original = cast(string) out[0..<n];
  e.tags_to = tags.count; e.content_from = tags.count + 1; e.content_to = n - 1; e.kind = 1040;
  return e;
}
```

### Checks in the library

Each library file has `#test` checks written before its code: NIP-73's
examples and written forms, every refusal and its boundary (the shortest
and longest geohash, Bitcoin address and ISO 3166 code, the 256-byte limit
on a normalized value); NIP-48's examples and id forms; and for NIP-03 the
two proofs, a tree with nested forks, an unknown attestation and a branch
that changes the message's length, every header and operation refusal at
its byte, every truncation of the short proof, the 4096-byte message
limit, eight open forks against nine, 16 attestations against 17, and the
kind 1040 event's refusals.

```shell
coax/bin/coax test coax/lib/nostr/nip73.coax
coax/bin/coax test coax/lib/nostr/nip48.coax
coax/bin/coax test coax/lib/nostr/nip03.coax
coax/bin/coax test nostr/examples/checks/bridges-timestamps.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/nip73.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip48.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip03.coax --root-only
```

On 2026-11-01, `coax test` passed all 208, 205 and 215 checks of the
three files (with the loaded files'), and all 378 of the example's checks.
`coax mutate --root-only` killed all 547 mutants of `nip73.coax`, all 111
of `nip48.coax` and all 510 of `nip03.coax`, with none surviving and none
marked equivalent. The example ran in debug and release with the same
output. The full mutation run (`COAX_MUTATION_FULL=1`) and a mutation run
of the example were not run.

### Not built

- Checking a Bitcoin attestation against a block header, and fetching
  headers. The caller does this with the reported height and root.
- The sha1, ripemd160 and keccak256 operations, and the deprecated reverse
  and hexlify. A proof that uses one, as proofs from public calendars may,
  is refused as unsupported until a later chapter builds it.
- Reading a pending attestation's calendar URL, or upgrading a pending
  proof by asking the calendar. Pending attestations are counted.
- Litecoin and Ethereum attestation types. They read as unknown.
- Bitcoin address checksums (chapter 15 has bech32), ISAN check
  characters, URL percent-encoding and default ports, and lower case
  beyond ASCII in hashtags.
- Fetching a proxy tag's source post.
- NIP-39 external identities (`i` tags on kind 0 such as `github:...`),
  which use the same tag name for another purpose. They are not read here.

## Summary

- `n73_normalize` writes an external identifier in its type's normal
  form: ISBNs without hyphens and with their check digit checked, geohashes
  and DOIs in lower case, country codes in upper case, ISANs without their
  version, URLs without their fragment. Other chains than Bitcoin are
  refused.
- `n73_tags` reads an event's `i` and `k` tags and refuses an `i` tag of an
  unknown type or not in normal form.
- `n48_read` reads `proxy` tags for the four protocols NIP-48 lists, each
  id in its protocol's form, and refuses any other protocol.
- `n03_parse` reads an `.ots` file and replays its sha256, append and
  prepend operations from the digest, through forks, to each attestation,
  and reports each Bitcoin attestation's height and merkle root.
- `n03_event` reads a kind 1040 event and refuses a proof that does not
  start from the `e` tag's event id. Comparing the root with the block
  header is the caller's job.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [4. SHA-256](ch004-sha-256.md)

## Sources

- [NIP-48](../nips/official/48.md)
- [NIP-73](../nips/official/73.md)
- [NIP-03](../nips/official/03.md)
- The OpenTimestamps format, as `python-opentimestamps` serializes it
  (header magic, LEB128 numbers, operation and attestation tags):
  <https://github.com/opentimestamps/python-opentimestamps>.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
