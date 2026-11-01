# 53. Long-form, drafts and wiki

Long documents on Nostr are addressable events: kind, author and a `d` tag
name the document, and a newer event with the same three replaces the older
one. This chapter builds three NIPs on that idea. NIP-23 articles (kind
30023) carry a title, a summary, an image and a first publication time in
their tags. NIP-37 draft wraps (kind 31234) keep an unfinished event
encrypted with NIP-44 to your own key, so a relay stores it but only you can
read it. NIP-54 wiki articles (kind 30818) are named by a normalized `d`
tag, so that everyone who writes about "Proof of Work" lands on the same
name, `proof-of-work`.

Part X, Every NIP. Result: Kind 30023 articles and 30024 drafts in Markdown,
NIP-37 kind 31234 draft wraps encrypted with NIP-44, and NIP-54 kind 30818
wiki articles with `d`-tag normalization.

## What this chapter teaches

- NIP-23: the `d`, `title`, `summary`, `image` and `published_at` tags read
  and written, an article's `a` coordinate, and which of several versions
  is the current one.
- NIP-37: a draft sealed with NIP-44 to your own public key, the wrap's `d`
  and `k` tags, and the checks that open it, in order.
- NIP-54: the `d`-tag normalization rule, exactly as the NIP states it, and
  a reader that refuses a wiki article whose `d` tag isn't normalized.
- Why a NIP-37 draft decrypts only for its author: the conversation key
  with yourself is `x(s·s·G)`, which only the holder of `s` can compute.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip54.coax](../../coax/lib/nostr/nip54.coax) | `nip54_normalize(title, out)` | A title's NIP-54 `d` value |
| | `n54_class(cp)`, `n54_lower(cp)` | What one code point becomes: kept, `-`, or removed; and its lowercase |
| | `nip54_is_normal(d)` | Whether `d` is its own normal form and not empty |
| | `nip54_read(e, buf)` | A kind 30818 article's `d` and `title`; refuses a `d` that isn't normalized |
| [nip23.coax](../../coax/lib/nostr/nip23.coax) | `nip23_read(e, buf)` | A kind 30023 or 30024 article's metadata |
| | `nip23_tags(m, out)` | The tags of an article, from its metadata |
| | `nip23_address(kind, author, d, out)` | The `a` coordinate `kind:author:d` |
| | `nip23_current(set, kind, author, d)` | The current version of one article in a set of events |
| [nip37.coax](../../coax/lib/nostr/nip37.coax) | `nip37_seal(secret, nonce, draft, keys, work, out, overlap)` | A draft encrypted to your own key: the wrap's content |
| | `nip37_tags(d, kind, expiration, out)` | The wrap's `d`, `k` and `expiration` tags |
| | `nip37_open(secret, text, keys, scratch, work, out, overlap)` | A wrap opened by its author, checked in order |
| [nip23_fixtures.coax](../../coax/tests/nostr/nip23_fixtures.coax), [nip37_fixtures.coax](../../coax/tests/nostr/nip37_fixtures.coax), [nip54_fixtures.coax](../../coax/tests/nostr/nip54_fixtures.coax) | | Event records, keys, Python-computed payloads and range tables for the library's checks |

`nip37.coax` decides who can read a draft, so it is marked `#authority`.
The three files build on the tag reader of
[chapter 22](ch022-kinds-and-tags.md) (`nostr_tag_d`, `nt_address`,
`ns_class`, `ns_newer`), the first-value reader `n10_text` of
[chapter 50](ch050-notes-threads-and-subjects.md), the decimal kind reader
`n18_kind` of [chapter 52](ch052-reposts-reactions-and-comments.md), and
NIP-44 from [chapter 20](ch020-nip-44-encrypted-payloads.md)
(`nip44_conversation_key_from_keys_checked`, `nip44_encrypt_checked`,
`nip44_decrypt_checked`). They add no JSON, hash, cipher or curve code of
their own.

## Wiki names: NIP-54 normalization

NIP-54 says that "articles are identified by lowercase, normalized `d`
tags", and gives seven rules:

1. All letters with uppercase/lowercase variants MUST be converted to
   lowercase.
2. Whitespace MUST be converted to `-`.
3. Punctuation and symbols SHOULD be removed.
4. Multiple consecutive `-` SHOULD be collapsed to a single `-`.
5. Leading and trailing `-` SHOULD be removed.
6. Non-ASCII letters (e.g., Japanese, Chinese, Arabic, Cyrillic) MUST be
   preserved as UTF-8.
7. Numbers MUST be preserved.

The code treats the SHOULD rules as MUST: two clients that disagree on
`What's Up?` would put the same subject under two names.

### One code point at a time

`nip54_normalize` reads the title as UTF-8, one code point at a time, and
puts each code point in one of three classes. Whitespace becomes `-`.
Punctuation and symbols are removed. Everything else is kept, lowercased
when it has a lowercase form.

The classes are a table of ranges. A code point in no range is kept.

<!-- source: coax/lib/nostr/nip54.coax class -->
```jai
// Code points that become "-" (N54_SPACE) or are removed (N54_DROP), as
// from, to, class. Everything else is kept. No two ranges overlap; the class
// is the sum over the ranges that hold cp, so an overlap would give neither.
N54_RANGES :: [117]int.[
  0x00,0x08,2, 0x09,0x0D,1, 0x0E,0x1F,2, 0x20,0x20,1, 0x21,0x2C,2, 0x2E,0x2F,2,
  0x3A,0x40,2, 0x5B,0x60,2, 0x7B,0x84,2, 0x85,0x85,1, 0x86,0x9F,2, 0xA0,0xA0,1,
  0xA1,0xA9,2, 0xAB,0xB1,2, 0xB4,0xB4,2, 0xB6,0xB8,2, 0xBB,0xBB,2, 0xBF,0xBF,2,
  0xD7,0xD7,2, 0xF7,0xF7,2, 0x1680,0x1680,1, 0x2000,0x200A,1, 0x200B,0x2027,2,
  0x2028,0x2029,1, 0x202A,0x202E,2, 0x202F,0x202F,1, 0x2030,0x205E,2, 0x205F,0x205F,1,
  0x2190,0x2BFF,2, 0x3000,0x3000,1, 0x3001,0x3003,2, 0x3008,0x3011,2, 0x3014,0x301F,2,
  0xFE00,0xFE0F,2, 0xFF01,0xFF0F,2, 0xFF1A,0xFF20,2, 0xFF3B,0xFF40,2, 0xFF5B,0xFF65,2,
  0x1F000,0x1FAFF,2];
// Whether cp is kept, becomes "-", or is removed.
n54_class :: (cp:int)->int {
  c:=N54_KEEP;r:=N54_RANGES;i:=0;
  while i<r.count {if cp>=r[i] && cp<=r[i+1] {c+=r[i+2];}i+=3;}
  return c;
}
```

The table covers ASCII punctuation and symbols, the C0 and C1 control
characters, Latin-1 signs (but not the ordinal letters `ª` and `º`, the
micro sign, or the number signs `²`, `³`, `¹`, `¼`, `½`, `¾`), general
punctuation and zero-width characters, CJK and fullwidth punctuation,
arrows through dingbats (U+2190 to U+2BFF), variation selectors and emoji
(U+1F000 to U+1FAFF). The hyphen-minus `-` stays a separator. Other dashes
(U+2010 to U+2015, including the em dash) are punctuation and are removed.

No two ranges overlap, and the class is the sum of the classes of the
ranges that hold the code point. An overlap would give 3, which is neither
class, and the checks would see it.

Lowercasing covers ASCII, Latin-1, Latin Extended-A, Greek, Cyrillic
U+0400 to U+042F and fullwidth Latin. Latin Extended-A alternates
uppercase and lowercase, with the uppercase letter on an even code point in
some runs and on an odd one in others; `İ` (U+0130) and `Ÿ` (U+0178) map
outside their run.

<!-- source: coax/lib/nostr/nip54.coax lower -->
```jai
// The lowercase of cp in the ranges this file maps; cp itself otherwise.
n54_lower :: (cp:int)->int {
  if cp>='A' && cp<='Z' {return cp+32;}
  if cp>=0xC0 && cp<=0xDE && cp!=0xD7 {return cp+32;}
  // Latin Extended-A: pairs, uppercase first, at even or odd code points.
  even:=(cp>=0x100 && cp<0x130) || (cp>=0x132 && cp<0x138) || (cp>=0x14A && cp<0x178);
  odd:=(cp>=0x139 && cp<0x149) || (cp>=0x179 && cp<0x17F);
  if (even && cp%2==0) || (odd && cp%2==1) {return cp+1;}
  if cp==0x130 {return 'i';}
  if cp==0x178 {return 0xFF;}
  if cp==0x386 {return 0x3AC;}
  if cp>=0x388 && cp<=0x38A {return cp+37;}
  if cp==0x38C {return 0x3CC;}
  if cp==0x38E || cp==0x38F {return cp+63;}
  if cp>=0x391 && cp<=0x3AB && cp!=0x3A2 {return cp+32;}
  if cp>=0x400 && cp<=0x40F {return cp+80;}
  if cp>=0x410 && cp<=0x42F {return cp+32;}
  if cp>=0xFF21 && cp<=0xFF3A {return cp+32;}
  return cp;
}
```

### Collapsing and trimming as it goes

The normalizer doesn't build the string and then clean it. It keeps one
flag, `dash`: a separator is pending. Whitespace and `-` set the flag, but
only when something has been written (`dash=n>0`), so leading whitespace
never produces a `-`. The pending `-` is written just before the next kept
code point, so a run of separators gives one `-` and trailing separators
give none. Removed code points don't touch the flag, so `Rock & Roll`
keeps one `-` between the words.

<!-- source: coax/lib/nostr/nip54.coax normalize -->
```jai
// The NIP-54 d value of a title into out. A "-" is written only before a
// kept code point that follows something kept, so runs collapse and the
// ends are trimmed as it goes.
nip54_normalize :: (title:string,out:[]u8)->N54Name {
  r:N54Name;n:=0;dash:=false;i:=0;
  while i<title.count {
    k:=json_utf8(title,i);if k==0 {r.error=N54_UTF8;return r;}
    cp:=n54_cp(title,i,k);i+=k;
    c:=n54_class(cp);
    if c==N54_SPACE || cp=='-' {dash=n>0;continue;}
    if c==N54_DROP {continue;}
    if dash {if n>=out.count {return r;}out[n]='-';n+=1;dash=false;}
    n=json_put_cp(out,n,n54_lower(cp));if n== -1 {return r;}
  }
  if n==0 {r.error=N54_EMPTY;return r;}
  r.error=N54_OK;r.n=n;return r;
}
```

A title with nothing left, such as `?!`, refuses as `N54_EMPTY`: an empty
`d` tag names no subject. Bytes that aren't UTF-8 refuse as `N54_UTF8`.

### Worked example: three titles by hand

Normalize three titles before the program runs. Each row is one code
point; `dash` is the pending flag after it.

`Rock & Roll (1950s)`:

| Code point | Class | Writes | `dash` | Output so far |
| --- | --- | --- | --- | --- |
| `R`, `o`, `c`, `k` | kept | `r`, `o`, `c`, `k` | no | `rock` |
| space | space | | yes | `rock` |
| `&` | removed | | yes | `rock` |
| space | space | | yes | `rock` |
| `R`, `o`, `l`, `l` | kept | `-`, then `roll` | no | `rock-roll` |
| space | space | | yes | `rock-roll` |
| `(` | removed | | yes | `rock-roll` |
| `1`, `9`, `5`, `0`, `s` | kept | `-`, then `1950s` | no | `rock-roll-1950s` |
| `)` | removed | | no | `rock-roll-1950s` |

`Москва — столица!`: `М` is U+041C, in U+0410 to U+042F, so it becomes
U+041C + 32 = U+043C, `м` (bytes `D0 9C` become `D0 BC`). `оскв` and `а`
are already lowercase. The space sets `dash`; the em dash U+2014 is in the
removed range U+200B to U+2027; the second space leaves `dash` set. `с` is
kept, so `-` is written before it. `!` is removed. The result is
`москва-столица`.

`  ¿Qué es Nostr?  `: the two leading spaces come before anything is
written, so `dash` stays off. `¿` (U+00BF) is removed. `Q` becomes `q`;
`é` (U+00E9) has no lowercase mapping to apply and is kept. The spaces
between words give single `-`. `N` becomes `n`. `?` is removed, and the
trailing spaces set `dash`, but no kept code point follows, so no `-` is
written. The result is `qué-es-nostr`.

### NIP-54's examples

| Title | NIP-54's `d` | What the rule does |
| --- | --- | --- |
| `Wiki Article` | `wiki-article` | lowercase; space to `-` |
| `What's Up?` | `whats-up` | `'` and `?` removed |
| `  Hello  World  ` | `hello-world` | ends trimmed; two spaces collapse |
| `Article 1` | `article-1` | digits kept |
| `ウィキペディア` | `ウィキペディア` | no case in Japanese |
| `Ñoño` | `ñoño` | U+00D1 + 32 = U+00F1 |
| `Москва` | `москва` | Cyrillic lowercased |
| `日本語 Article` | `日本語-article` | mixed scripts |

NIP-54 uses the same rule for wikilinks: a reference-style link with no
reference, such as `[proof of work][]`, links to the article whose `d` is
`proof-of-work`. `[Japanese Article][日本語 記事]` links to `日本語-記事`, and
`[Биткойн][]` to `биткойн`. The checks compare all of these.

### Reading a wiki article

<!-- source: coax/lib/nostr/nip54.coax read -->
```jai
// A kind 30818 article: its d tag, which must be normalized, and its title,
// unescaped into buf.
nip54_read :: (e:NostrEvent,buf:[]u8)->N54Article {
  a:N54Article;
  d:=n10_text(e,"d",buf);
  if d.status==N10_LIMIT {a.error=N54_LIMIT;return a;}
  if d.status!=N10_OK {return a;}
  if e.kind!=N54_WIKI {a.error=N54_KIND;return a;}
  if !d.present {a.error=N54_NO_D;return a;}
  if !nip54_is_normal(cast(string)buf[0..<d.n]) {a.error=N54_NOT_NORMAL;return a;}
  t:=n10_text(e,"title",buf[d.n..<buf.count]);
  if t.status==N10_LIMIT {a.error=N54_LIMIT;return a;}
  if t.status!=N10_OK {return a;}
  a.error=N54_OK;a.d_n=d.n;a.has_title=t.present;a.title_n=t.n;return a;
}
```

The reader refuses a kind 30818 event with no `d` tag (`N54_NO_D`) and one
whose `d` tag isn't its own normal form (`N54_NOT_NORMAL`), such as
`Wiki Article`. A client can normalize a title it is given, but it can't
safely rename someone else's signed article: the `d` tag is part of the
address, and two clients that normalize differently would show the event
under different names.

## Articles: NIP-23

An article's `content` is Markdown. Its metadata are tags: NIP-23
standardizes `title`, `image`, `summary` and `published_at`, "the
timestamp in unix seconds (stringified) of the first time the article was
published". `created_at` is the date of the last update. The `d` tag names
the article, so a new version with the same `d` replaces the old one.

<!-- source: coax/lib/nostr/nip23.coax record -->
```jai
// A tag value unescaped into the caller's buffer at [from..<to].
N23Field :: struct {present:bool; from:int; to:int;}
N23Article :: struct {
  error:int=N23_EVENT; kind:int;
  d:N23Field; title:N23Field; summary:N23Field; image:N23Field;
  has_published:bool; published_at:int;
}
// What the writer puts in the tags. Empty strings and a negative
// published_at are left out; d is required.
N23Meta :: struct {d:string; title:string; summary:string; image:string; published_at:int=-1;}
N23Tags :: struct {error:int=N23_OUTPUT; n:int;}
```

### Reading

<!-- source: coax/lib/nostr/nip23.coax read -->
```jai
// A kind 30023 or 30024 article's metadata, unescaped into buf.
nip23_read :: (e:NostrEvent,buf:[]u8)->N23Article {
  a:N23Article;at:=0;
  names:=[5]string.["d","title","summary","image","published_at"];
  fields:[5]N23Field;
  // Each field: the first tag with that name, its value unescaped at buf[at..].
  for i:0..<5 {
    t:=n10_text(e,names[i],buf[at..<buf.count]);
    if t.status==N10_LIMIT {a.error=N23_LIMIT;return a;}
    if t.status!=N10_OK {return a;}
    fields[i]=N23Field.{present=t.present,from=at,to=at+t.n};at+=t.n;
  }
  if e.kind!=N23_ARTICLE && e.kind!=N23_DRAFT {a.error=N23_KIND;return a;}
  if fields[0].to==fields[0].from {a.error=N23_NO_D;return a;}
  a.kind=e.kind;a.d=fields[0];a.title=fields[1];a.summary=fields[2];a.image=fields[3];
  p:=fields[4];
  if p.present {
    s:=cast(string)buf[p.from..<p.to];v:=json_int(s);
    if !v.ok || s[0]=='-' {a.error=N23_PUBLISHED;return a;}
    if v.value>e.created_at {a.error=N23_LATER;return a;}
    a.has_published=true;a.published_at=v.value;
  }
  a.error=N23_OK;return a;
}
```

The reader takes the first tag of each name and unescapes its value into
the caller's buffer. It refuses:

- a kind other than 30023 or 30024 (`N23_KIND`);
- no `d` tag, or an empty one (`N23_NO_D`). NIP-23 says articles "should
  include a `d` tag". Without one, the article has the empty identifier,
  and each new article by the same author would replace the last;
- a `published_at` that isn't decimal unix seconds (`N23_PUBLISHED`): not
  a JSON integer, negative, or with a fraction or a leading zero;
- a `published_at` later than `created_at` (`N23_LATER`). The first
  publication can't follow the last update. NIP-23 doesn't state this
  refusal; it follows from the two definitions.

Kind 30024 reads with the same tags. NIP-23 now calls it deprecated: its
content was encrypted with NIP-04, and NIP-37 is the preferred way to keep
drafts. The reader doesn't decrypt it.

### Writing

<!-- source: coax/lib/nostr/nip23.coax write -->
```jai
// The tags of an article, as a JSON array into out.
nip23_tags :: (m:N23Meta,out:[]u8)->N23Tags {
  r:N23Tags;
  if m.d.count==0 {r.error=N23_NO_D;return r;}
  n:=json_raw(out,0,"[[\"d\",");n=json_write_string(out,n,m.d);n=json_raw(out,n,"]");
  n=n23_pair(out,n,"title",m.title);n=n23_pair(out,n,"summary",m.summary);n=n23_pair(out,n,"image",m.image);
  if m.published_at>=0 {
    num:[24]u8;k:=json_write_int(num[..],0,m.published_at);
    n=n23_pair(out,n,"published_at",cast(string)num[0..<k]);
  }
  n=json_raw(out,n,"]");
  if n==NE_WRITE_FAILED {return r;}
  r.error=N23_OK;r.n=n;return r;
}
// ,["name","value"] after out[0..<n], unless value is empty.
n23_pair :: (out:[]u8,n:int,name:string,value:string)->int {
  if value.count==0 {return n;}
  m:=json_raw(out,n,",[\"");m=json_raw(out,m,name);m=json_raw(out,m,"\",");
  m=json_write_string(out,m,value);return json_raw(out,m,"]");
}
```

The writer escapes every value with `json_write_string` and leaves out
empty fields and a negative `published_at`. It refuses an empty `d`.

### The `a` coordinate

NIP-23 links an article with an `a` tag and the NIP-19 `naddr` code. The
`a` value is `kind:author:d`, the same address
[chapter 22](ch022-kinds-and-tags.md) reads.

<!-- source: coax/lib/nostr/nip23.coax address -->
```jai
// The a coordinate "kind:author:d" of an addressable event into out: its
// length, or -1 for a kind outside 30000-39999 or a short out.
nip23_address :: (kind:int,author:[32]u8,d:string,out:[]u8)->int {
  if ns_class(kind)!=NS_ADDRESSABLE {return -1;}
  hx:[64]u8;a:=author;
  h:=hex_encode_checked(cast(string)a[..],hx[..],false);
  n:=json_write_int(out,0,kind);n=json_raw(out,n,":");n=json_raw(out,n,cast(string)hx[..]);
  n=json_raw(out,n,":");n=json_raw(out,n,d);
  return n;
}
```

By hand, for the example's article by key 1 (public key `79be…1798`, the
x coordinate of secp256k1's generator) with `d` `lorem-ipsum`: `30023`,
`:`, the 64 hex digits, `:`, `lorem-ipsum`. That is 5 + 1 + 64 + 1 + 11 =
82 bytes:

```text
30023:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:lorem-ipsum
```

The checks read it back with `nt_address`. A kind outside 30000 to 39999
has no `d` in its address and refuses.

### The current version

A relay that implements addressable events keeps only the newest version.
NIP-23 asks clients that read from other relays to "take care to hide old
versions of the same article". `nip23_current` picks the version NIP-01
keeps: the newest `created_at`, and on a tie the lowest ID. It uses the
store's comparison `ns_newer`, so the client and the store agree.

<!-- source: coax/lib/nostr/nip23.coax current -->
```jai
// Which event in set is the current version of the article kind:author:d:
// the newest created_at, and on a tie the lowest id (NIP-01's rule, as the
// store applies it with ns_newer). N23_NONE when no event has that address.
// An event whose record refused, or whose d tag doesn't read, is skipped.
nip23_current :: (set:[]NostrEvent,kind:int,author:[32]u8,d:string)->int {
  best:=N23_NONE;top:NsMeta;
  for i:0..<set.count {
    e:=set[i];
    if e.error!=NE_OK || e.kind!=kind || !ns_equal32(e.public_key,author) {continue;}
    t:=nostr_tag_d(e);
    if t.error!=NT_OK || cast(string)t.d[0..<t.d_n]!=d {continue;}
    m:NsMeta;m.id=e.id;m.created=e.created_at;
    if best==N23_NONE || ns_newer(m,top) {best=i;top=m;}
  }
  return best;
}
```

The example signs three versions with key 1. Before it runs, decide by
hand which one is current, from the times and the IDs it prints:

| Version | `created_at` | ID starts | Current? |
| --- | --- | --- | --- |
| 1 | 1675642635 | `e71d4117` | no: older |
| 2 | 1675643235 | `f9fe58d4` | no: same time as 3, larger ID |
| 3 | 1675643235 | `1166786c` | yes: newest, and `11` < `f9` |

## Drafts: NIP-37

A draft wrap is kind 31234. Its content is a draft event (unsigned, any
kind) "JSON-stringified, NIP44-encrypted to the signer's public key". Its
tags are `d` (the wrap's own identifier), `k` (the draft's kind, required)
and, recommended, a NIP-40 `expiration`. "A blanked `.content` field
signals that the draft has been deleted."

<!-- source: coax/lib/nostr/nip37.coax record -->
```jai
Nip37Sealed :: struct {error:int=N37_INPUT; written:int; kind:int;}
// out[0..<written] holds the draft JSON; kind is its kind (the k tag's).
Nip37Opened :: struct {error:int=N37_INPUT; written:int; kind:int;}
// Key work: the ECDH scratch, your public key and the conversation key.
Nip37Keys :: struct {ec:[1]EcdhScratch; self:[32]u8; ck:[32]u8;}
```

### Why only the author can read it

NIP-44's conversation key between a secret `a` and a public key `B` is
`HKDF-extract(salt="nip44-v2", x(a·B))`. Encrypting to yourself, with
secret `s` and public key `P = s·G`, uses `x(s·P) = x(s·s·G)`.

Work it for key 1 (`s = 1`): `P = G`, `x(1·G)` is `79be…1798`, and the
conversation key is HMAC-SHA256 with key `nip44-v2` over those 32 bytes,
`3b4610cb…b54e`. Key 3 knows `P` but not `s`. The best it can compute is
its own conversation key with key 1, `x(3·G)` = `f9308a01…36f9`, which
gives `68ace26d…1b9d`. That key derives different ChaCha20 and HMAC keys,
so NIP-44's MAC check fails, and nothing is decrypted.

`nip37_seal` doesn't take your public key as an input. It derives it from
the secret with an ECDH against the generator, whose result's x coordinate
is `x(s·G)`, so a draft can't be sealed to anyone else by mistake.

<!-- source: coax/lib/nostr/nip37.coax keys -->
```jai
// Your x-only public key into keys.self: x(secret*G), an ECDH with the
// generator. Then the conversation key from secret to that key into
// keys.ck. N37_KEY for a secret that is 0 or not below n.
n37_keys :: (secret:[]u8,keys:[]Nip37Keys)->int {
  g:=NIP44_KEY_GENERATOR_X;
  if ecdh_stage(secret,g[..],keys[0].ec[..])!=EC_OK {return N37_KEY;}
  for i:0..<32 {keys[0].self[i]=keys[0].ec[0].staged[i];}
  #assert nip44_conversation_key_from_keys_checked(secret,keys[0].self[..],keys[0].ec[..],keys[0].ck[..],false)==NIP44_OK;
  return N37_OK;
}
n37_payload :: (status:u32)->int {
  if status==NIP44_OUTPUT || status==NIP44_CAPACITY {return N37_OUTPUT;}
  return N37_PAYLOAD;
}
```

### Sealing

The draft must be a JSON object with one integer `kind`, so the `k` tag
can be written from it.

<!-- source: coax/lib/nostr/nip37.coax draft -->
```jai
// The kind of a draft event's JSON: an object whose one "kind" member is an
// integer 0-65535. -1 for anything else.
n37_draft_kind :: (text:string)->int {
  j:[1]Json;json_start(j[..],text);
  if json_next(j[..])!=JSON_OBJECT {return -1;}
  kind:=-1;seen:=false;tok:=json_next(j[..]);
  while tok==JSON_STRING {
    named:=json_is(json_text(j[..]),"kind");
    v:=json_next(j[..]);
    if named {
      if seen || v!=JSON_NUMBER {return -1;}
      seen=true;kind=n18_kind(json_text(j[..]));
    }
    if !json_skip(j[..]) {return -1;}
    tok=json_next(j[..]);
  }
  if tok!=JSON_OBJECT_END || json_next(j[..])!=JSON_END {return -1;}
  return kind;
}
```

<!-- source: coax/lib/nostr/nip37.coax seal -->
```jai
// The content of a draft wrap: draft (an unsigned event's JSON) NIP-44
// encrypted with nonce from secret to its own public key, into out. work
// holds NIP-44's encryption work. overlap: any two views share memory.
nip37_seal :: (secret:[]u8,nonce:[]u8,draft:[]u8,keys:[]Nip37Keys,work:[]u8,out:[]u8,overlap:bool)->Nip37Sealed {
  r:Nip37Sealed;
  if secret.count!=32 || nonce.count!=32 || keys.count!=1 || overlap {return r;}
  kind:=n37_draft_kind(cast(string)draft);
  if kind<0 {r.error=N37_DRAFT;}
  else {r.error=n37_keys(secret,keys);}
  if r.error==N37_OK {
    e:=nip44_encrypt_checked(NIP44_COMPAT,keys[0].ck[..],nonce,draft,work,out,false);
    r.error=ifx e.error==NIP44_OK then N37_OK else n37_payload(e.error);
    if r.error==N37_OK {r.written=e.written;r.kind=kind;}
  }
  keys[0]=Nip37Keys.{};
  return r;
}
```

<!-- source: coax/lib/nostr/nip37.coax tags -->
```jai
// The wrap's tags [["d",d],["k",kind]] and, when expiration >= 0,
// ["expiration",expiration], into out: its length, or -1.
nip37_tags :: (d:string,kind:int,expiration:int,out:[]u8)->int {
  if d.count==0 || kind<0 || kind>65535 {return -1;}
  n:=json_raw(out,0,"[[\"d\",");n=json_write_string(out,n,d);n=json_raw(out,n,"],[\"k\",\"");
  n=json_write_int(out,n,kind);n=json_raw(out,n,"\"]");
  if expiration>=0 {n=json_raw(out,n,",[\"expiration\",\"");n=json_write_int(out,n,expiration);n=json_raw(out,n,"\"]");}
  return json_raw(out,n,"]");
}
```

The nonce must be 32 fresh random bytes from the host for every seal. The
example uses a fixed nonce so that its output is the same on every run.

The ECDH scratch, your public key and the conversation key live in the
caller's `Nip37Keys`, which every return zeroes, so the checks can see that
nothing is left behind.

### Opening

<!-- source: coax/lib/nostr/nip37.coax open -->
```jai
// A draft wrap opened by the holder of secret: the draft JSON into out.
// scratch holds the wrap's serialization; work holds NIP-44's decryption
// work.
nip37_open :: (secret:[]u8,text:string,keys:[]Nip37Keys,scratch:[]u8,work:[]u8,out:[]u8,overlap:bool)->Nip37Opened {
  r:Nip37Opened;
  if secret.count!=32 || keys.count!=1 || overlap {return r;}
  r.error=n37_keys(secret,keys);
  if r.error==N37_OK {r=n37_check(keys[0].self[..],keys[0].ck[..],text,scratch,work,out);}
  keys[0]=Nip37Keys.{};
  return r;
}
n37_check :: (self:[]u8,ck:[]u8,text:string,scratch:[]u8,work:[]u8,out:[]u8)->Nip37Opened {
  r:Nip37Opened;
  e:=nostr_event_read(text,scratch,false);if e.error!=NE_OK {r.error=N37_EVENT;return r;}
  if e.kind!=N37_WRAP {r.error=N37_KIND;return r;}
  pub:=e.public_key;
  for i:0..<32 {if pub[i]!=self[i] {r.error=N37_NOT_SELF;return r;}}
  tb:[NT_MAX_VALUE]u8;
  d:=n10_text(e,"d",tb[..]);if !d.present || d.n==0 {r.error=N37_NO_D;return r;}
  k:=n10_text(e,"k",tb[..]);kind:=-1;
  if k.present {kind=n18_kind(cast(string)tb[0..<k.n]);}
  if kind<0 {r.error=N37_K;return r;}
  if e.content_to==e.content_from {r.error=N37_DELETED;return r;}
  // Base64 has no characters JSON must escape, but a writer may escape "/".
  // The reader bounds the content, so it fits.
  payload:[N37_MAX_CONTENT]u8;
  n:=json_unescape(e.original[e.content_from..<e.content_to],payload[..]);
  p:=nip44_decrypt_checked(NIP44_COMPAT,ck,cast(string)payload[0..<n],work,out,false);
  if p.error!=NIP44_OK {r.error=n37_payload(p.error);return r;}
  got:=n37_draft_kind(cast(string)out[0..<p.written]);
  if got<0 || got!=kind {
    for i:0..<out.count {out[i]=0;}
    r.error=ifx got<0 then N37_DRAFT else N37_MISMATCH;return r;
  }
  r.error=N37_OK;r.written=p.written;r.kind=kind;return r;
}
```

`nip37_open` checks, in order:

1. The secret is a valid key (`N37_KEY`).
2. The wrap reads as an event whose ID is its own (`N37_EVENT`). The
   signature is the caller's to verify, as relays and the store do.
3. Its kind is 31234 (`N37_KIND`).
4. Its `pubkey` is the opener's own public key (`N37_NOT_SELF`). A wrap
   signed by someone else wasn't sealed to you, and nothing is decrypted.
5. It has a non-empty `d` tag (`N37_NO_D`).
6. It has a `k` tag with a kind from 0 to 65535 (`N37_K`).
7. Its content isn't blank (`N37_DELETED`): a deleted draft.
8. The content decrypts (`N37_PAYLOAD`, or `N37_OUTPUT` when `out` is too
   small).
9. The plaintext is a JSON object with an integer `kind` (`N37_DRAFT`).
10. That kind is the `k` tag's (`N37_MISMATCH`).

After a refusal at step 9 or 10, `out` is zeroed: it held plaintext that
didn't check out.

## The example

The example is [nostr/examples/long-form.coax](../examples/long-form.coax).
It runs offline. It prints NIP-54's examples and the three hand-worked
titles, signs a NIP-23 article with key 1 and reads it back, prints its `a`
coordinate, signs two more versions and picks the current one, seals a
draft to key 1, signs the wrap, opens it as key 1 and as key 3, and shows
the refusals.

<!-- source: nostr/examples/long-form.coax example -->
```jai
#load "../../coax/lib/nostr/event_sign.coax";
#load "../../coax/lib/nostr/nip23.coax";
#load "../../coax/lib/nostr/nip37.coax";
#load "../../coax/lib/nostr/nip54.coax";
#import "Basic";

// Test keys, never real ones. Key 1 is the scalar 1 (public key x(G));
// key 3 is BIP-340 test vector 0's.
KEY_1 :: "0000000000000000000000000000000000000000000000000000000000000001";
KEY_3 :: "0000000000000000000000000000000000000000000000000000000000000003";
AUX :: "0000000000000000000000000000000000000000000000000000000000000000";
// NIP-23's example times: created_at (the last update) and published_at.
CREATED :: 1675642635;
PUBLISHED :: 1296962229;
// A fixed NIP-44 nonce, so the output is the same on every run. A client
// takes 32 fresh random bytes from the host for every seal.
NONCE :: "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
// The draft: an unsigned kind 30023 event, as a client would keep it.
DRAFT :: "{\"kind\":30023,\"created_at\":1675650000,\"tags\":[[\"d\",\"lorem-ipsum\"],[\"title\",\"Lorem Ipsum, revised\"]],\"content\":\"A second draft, not yet published.\"}";

text_of :: (n: int, out: [] u8) -> string { #assert n > 0; return cast(string) out[0..<n]; }
hex_of :: (bytes: [32] u8, out: [] u8) -> string {
  b := bytes; #assert hex_encode_checked(cast(string) b[..], out, false).error == NC_OK;
  return cast(string) out[0..<64];
}
bytes_of :: (hex: string) -> [32] u8 {
  b: [32] u8; #assert hex_decode_checked(hex, b[..], false).error == NC_OK; return b;
}
// Signs one event with a test key into out.
sign :: (key_hex: string, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> string {
  key := bytes_of(key_hex); aux := bytes_of(AUX);
  work: [1] NostrSignWork; scratch: [4096] u8; k := EVENT_MORE;
  while k == EVENT_MORE {
    k = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  return text_of(k, out);
}
name54 :: (e: int) -> string {
  names := [9] string.["N54_OK", "N54_UTF8", "N54_EMPTY", "N54_OUTPUT", "N54_EVENT", "N54_KIND", "N54_NO_D", "N54_NOT_NORMAL", "N54_LIMIT"];
  return names[e];
}
name23 :: (e: int) -> string {
  names := [8] string.["N23_OK", "N23_EVENT", "N23_KIND", "N23_NO_D", "N23_PUBLISHED", "N23_LATER", "N23_LIMIT", "N23_OUTPUT"];
  return names[e];
}
name37 :: (e: int) -> string {
  names := [13] string.["N37_OK", "N37_INPUT", "N37_KEY", "N37_EVENT", "N37_KIND", "N37_NOT_SELF", "N37_NO_D", "N37_K",
    "N37_DELETED", "N37_PAYLOAD", "N37_DRAFT", "N37_MISMATCH", "N37_OUTPUT"];
  return names[e];
}
// A title and its d value, or the refusal. label is how the title prints.
show_as :: (title: string, label: string) {
  b: [128] u8; r := nip54_normalize(title, b[..]);
  if r.error == N54_OK { print("  \"%\" -> \"%\"\n", label, cast(string) b[0..<r.n]); }
  else { print("  \"%\" -> %\n", label, name54(r.error)); }
}
show_d :: (title: string) { show_as(title, title); }
// A wiki event (unsigned; the reader checks tags, not signatures).
wiki :: (tags: string) -> string {
  e := nt_event(tags); e.kind = N54_WIKI; b: [256] u8;
  a := nip54_read(e, b[..]);
  if a.error != N54_OK { return name54(a.error); }
  return "N54_OK";
}
// An article read back: its fields, or the refusal.
show_article :: (event: string) {
  s: [8192] u8; e := nostr_event_read(event, s[..], false); #assert e.error == NE_OK;
  b: [512] u8; a := nip23_read(e, b[..]);
  if a.error != N23_OK { print("  nip23_read: %\n", name23(a.error)); return; }
  print("  kind %, d \"%\", title \"%\", summary \"%\", published_at %\n", a.kind, n23_text(a.d, b[..]),
        n23_text(a.title, b[..]), n23_text(a.summary, b[..]), a.published_at);
}

main :: () {
  print("== 1. NIP-54 d values\n");
  ins := [8] string.["Wiki Article", "What's Up?", "  Hello  World  ", "Article 1", "ウィキペディア", "Ñoño", "Москва", "日本語 Article"];
  for t: ins { show_d(t); }
  print("worked by hand in the chapter:\n");
  show_d("Rock & Roll (1950s)"); show_d("Москва — столица!"); show_d("  ¿Qué es Nostr?  ");
  print("wikilink targets:\n");
  show_d("proof of work"); show_d("日本語 記事"); show_d("Биткойн");
  print("refused:\n");
  show_d("?!"); bad := [3] u8.['a', 0xC3, 'b']; show_as(cast(string) bad[..], "a\\xC3b");
  print("wiki articles:\n");
  print("  d \"wiki\": %\n", wiki("[[\"d\",\"wiki\"],[\"title\",\"Wiki\"]]"));
  print("  d \"Wiki Article\": %\n", wiki("[[\"d\",\"Wiki Article\"]]"));
  print("  no d tag: %\n", wiki("[[\"title\",\"Wiki\"]]"));

  print("\n== 2. A long-form article (kind 30023)\n");
  m: N23Meta; m.d = "lorem-ipsum"; m.title = "Lorem Ipsum"; m.summary = "A placeholder article."; m.published_at = PUBLISHED;
  tb: [256] u8; t := nip23_tags(m, tb[..]); #assert t.error == N23_OK;
  tags := cast(string) tb[0..<t.n];
  print("tags %\n", tags);
  e1b: [2048] u8; v1 := sign(KEY_1, CREATED, N23_ARTICLE, tags, "Lorem ipsum dolor sit amet.", e1b[..]);
  print("signed by key 1:\n  %\n", v1);
  show_article(v1);
  key1 := bytes_of("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798");
  ab: [128] u8; an := nip23_address(N23_ARTICLE, key1, "lorem-ipsum", ab[..]);
  print("a coordinate %\n", text_of(an, ab[..]));

  print("\n== 3. Which version is current\n");
  e2b: [2048] u8; v2 := sign(KEY_1, CREATED + 600, N23_ARTICLE, tags, "Lorem ipsum dolor sit amet, edited.", e2b[..]);
  e3b: [2048] u8; v3 := sign(KEY_1, CREATED + 600, N23_ARTICLE, tags, "Lorem ipsum dolor sit amet, edited again.", e3b[..]);
  s1: [8192] u8; s2: [8192] u8; s3: [8192] u8; hx: [64] u8;
  set: [3] NostrEvent;
  set[0] = nostr_event_read(v1, s1[..], false); set[1] = nostr_event_read(v2, s2[..], false); set[2] = nostr_event_read(v3, s3[..], false);
  for i: 0..<3 { print("  version %: created_at %, id %\n", i + 1, set[i].created_at, hex_of(set[i].id, hx[..])); }
  cur := nip23_current(set[..], N23_ARTICLE, key1, "lorem-ipsum");
  print("current: version %\n", cur + 1);
  print("versions 1 and 2 only: version %\n", nip23_current(set[0..<2], N23_ARTICLE, key1, "lorem-ipsum") + 1);
  print("by key 3's address: %\n", nip23_current(set[..], N23_ARTICLE, bytes_of("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"), "lorem-ipsum"));

  print("\n== 4. A draft wrap (kind 31234), sealed to yourself\n");
  k1 := bytes_of(KEY_1); k3 := bytes_of(KEY_3); nonce := bytes_of(NONCE);
  db: [512] u8; for i: 0..<DRAFT.count { db[i] = DRAFT[i]; }
  work: [4096] u8; pb: [1024] u8; keys: [1] Nip37Keys;
  sl := nip37_seal(k1[..], nonce[..], db[0..<DRAFT.count], keys[..], work[..], pb[..], false); #assert sl.error == N37_OK;
  payload := cast(string) pb[0..<sl.written];
  print("draft %\n", DRAFT);
  print("content (NIP-44, % characters) %\n", sl.written, payload);
  wt: [128] u8; wn := nip37_tags("lorem-ipsum-draft", sl.kind, CREATED + 90 * 86400, wt[..]);
  wtags := text_of(wn, wt[..]);
  print("tags %\n", wtags);
  wb: [2048] u8; wrap := sign(KEY_1, CREATED, N37_WRAP, wtags, payload, wb[..]);
  print("signed by key 1:\n  %\n", wrap);
  sc: [8192] u8; ob: [1024] u8;
  o := nip37_open(k1[..], wrap, keys[..], sc[..], work[..], ob[..], false);
  print("key 1 opens it: %, kind %\n  %\n", name37(o.error), o.kind, cast(string) ob[0..<o.written]);
  o3 := nip37_open(k3[..], wrap, keys[..], sc[..], work[..], ob[..], false);
  print("key 3 opens it: %\n", name37(o3.error));
  ck: [32] u8; ec: [1] EcdhScratch;
  #assert nip44_conversation_key_from_keys_checked(k3[..], key1[..], ec[..], ck[..], false) == NIP44_OK;
  d3 := nip44_decrypt_checked(NIP44_COMPAT, ck[..], payload, work[..], ob[..], false);
  print("key 3 decrypts the content with its key and key 1's public key: NIP44 error % (NIP44_MAC is %)\n", d3.error, NIP44_MAC);

  print("\n== 5. Broken events\n");
  bb: [2048] u8;
  show_article(sign(KEY_1, CREATED, N23_ARTICLE, "[[\"title\",\"No identifier\"]]", "x", bb[..]));
  show_article(sign(KEY_1, CREATED, N23_ARTICLE, "[[\"d\",\"a\"],[\"published_at\",\"1675642636\"]]", "x", bb[..]));
  show_article(sign(KEY_1, CREATED, N23_ARTICLE, "[[\"d\",\"a\"],[\"published_at\",\"yesterday\"]]", "x", bb[..]));
  show_article(sign(KEY_1, CREATED, 1, tags, "x", bb[..]));
  blank := sign(KEY_1, CREATED + 1, N37_WRAP, wtags, "", bb[..]);
  print("  blanked draft wrap: %\n", name37(nip37_open(k1[..], blank, keys[..], sc[..], work[..], ob[..], false).error));
  cb: [2048] u8; nok := sign(KEY_1, CREATED, N37_WRAP, "[[\"d\",\"lorem-ipsum-draft\"]]", payload, cb[..]);
  print("  draft wrap with no k tag: %\n", name37(nip37_open(k1[..], nok, keys[..], sc[..], work[..], ob[..], false).error));
  kb: [2048] u8; wrongk := sign(KEY_1, CREATED, N37_WRAP, "[[\"d\",\"lorem-ipsum-draft\"],[\"k\",\"1\"]]", payload, kb[..]);
  print("  draft wrap whose k tag says 1: %\n", name37(nip37_open(k1[..], wrongk, keys[..], sc[..], work[..], ob[..], false).error));
}
```

Run it with:

```shell
coax/bin/coax run nostr/examples/long-form.coax --run-steps 250000000
```

Its output, the same as [the fixture](../fixtures/long-form.out):

```text
== 1. NIP-54 d values
  "Wiki Article" -> "wiki-article"
  "What's Up?" -> "whats-up"
  "  Hello  World  " -> "hello-world"
  "Article 1" -> "article-1"
  "ウィキペディア" -> "ウィキペディア"
  "Ñoño" -> "ñoño"
  "Москва" -> "москва"
  "日本語 Article" -> "日本語-article"
worked by hand in the chapter:
  "Rock & Roll (1950s)" -> "rock-roll-1950s"
  "Москва — столица!" -> "москва-столица"
  "  ¿Qué es Nostr?  " -> "qué-es-nostr"
wikilink targets:
  "proof of work" -> "proof-of-work"
  "日本語 記事" -> "日本語-記事"
  "Биткойн" -> "биткойн"
refused:
  "?!" -> N54_EMPTY
  "a\xC3b" -> N54_UTF8
wiki articles:
  d "wiki": N54_OK
  d "Wiki Article": N54_NOT_NORMAL
  no d tag: N54_NO_D

== 2. A long-form article (kind 30023)
tags [["d","lorem-ipsum"],["title","Lorem Ipsum"],["summary","A placeholder article."],["published_at","1296962229"]]
signed by key 1:
  {"id":"e71d4117133381631fbafbd92c1ccd72b69ee07f47f6332458310d74ba856591","pubkey":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","created_at":1675642635,"kind":30023,"tags":[["d","lorem-ipsum"],["title","Lorem Ipsum"],["summary","A placeholder article."],["published_at","1296962229"]],"content":"Lorem ipsum dolor sit amet.","sig":"dd92288ffc7b91cbcc553c07efdd47e988ee9d0669812e80d47a5457a1d3b34572ee0a44c3305e7fe1a0a73a4803b1a4b13536bbdceb5a6c9ac45c94c5072245"}
  kind 30023, d "lorem-ipsum", title "Lorem Ipsum", summary "A placeholder article.", published_at 1296962229
a coordinate 30023:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:lorem-ipsum

== 3. Which version is current
  version 1: created_at 1675642635, id e71d4117133381631fbafbd92c1ccd72b69ee07f47f6332458310d74ba856591
  version 2: created_at 1675643235, id f9fe58d4fc08bbe74f30053cadd93e4846fc6165f534aec0c99344881eba6350
  version 3: created_at 1675643235, id 1166786c8fefe9035aa46551909f4e4c2929a306f8c0c73c1cca18c1638bc4e4
current: version 3
versions 1 and 2 only: version 2
by key 3's address: -1

== 4. A draft wrap (kind 31234), sealed to yourself
draft {"kind":30023,"created_at":1675650000,"tags":[["d","lorem-ipsum"],["title","Lorem Ipsum, revised"]],"content":"A second draft, not yet published."}
content (NIP-44, 304 characters) AgABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fbgrW1PBXWEOJoAzWm+GbZ1h8NhsC4kv1spIKwySRhkbzlNV0C/yEB2Q0z53wJ3El8OeWsSy/MRXEh49F54iIyBx9ycGqWVw/uqrKprM3uF37XoWG2opxOVWwqKaClSJ1oezIkS5uDCm5l3qgwXtcvKcIBppcqlhjNji5rHYfeIZpPvbl+TFs7ABan0q0uNi5w9dyzxkokycpdRVevz5fKDd+ZsKc0KNEXbrwCbMqd9Gv2tI1MrZdC9Zuh8s1H623Ekc=
tags [["d","lorem-ipsum-draft"],["k","30023"],["expiration","1683418635"]]
signed by key 1:
  {"id":"94794d50ab04bb1c6731ec03efab1e2a6c3adedae858311465b6248a7ea7642c","pubkey":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","created_at":1675642635,"kind":31234,"tags":[["d","lorem-ipsum-draft"],["k","30023"],["expiration","1683418635"]],"content":"AgABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fbgrW1PBXWEOJoAzWm+GbZ1h8NhsC4kv1spIKwySRhkbzlNV0C/yEB2Q0z53wJ3El8OeWsSy/MRXEh49F54iIyBx9ycGqWVw/uqrKprM3uF37XoWG2opxOVWwqKaClSJ1oezIkS5uDCm5l3qgwXtcvKcIBppcqlhjNji5rHYfeIZpPvbl+TFs7ABan0q0uNi5w9dyzxkokycpdRVevz5fKDd+ZsKc0KNEXbrwCbMqd9Gv2tI1MrZdC9Zuh8s1H623Ekc=","sig":"06070f9cf6b733410d832b24bcf892695dd895f6b8380ed4f2387e75f4642ba981a11ca822071434d473ee9e8d27c63c99e2d2cfab5142b1d88530b2a0928ac1"}
key 1 opens it: N37_OK, kind 30023
  {"kind":30023,"created_at":1675650000,"tags":[["d","lorem-ipsum"],["title","Lorem Ipsum, revised"]],"content":"A second draft, not yet published."}
key 3 opens it: N37_NOT_SELF
key 3 decrypts the content with its key and key 1's public key: NIP44 error 4 (NIP44_MAC is 4)

== 5. Broken events
  nip23_read: N23_NO_D
  nip23_read: N23_LATER
  nip23_read: N23_PUBLISHED
  nip23_read: N23_KIND
  blanked draft wrap: N37_DELETED
  draft wrap with no k tag: N37_K
  draft wrap whose k tag says 1: N37_MISMATCH
```

Key 1 and key 3 are test keys: anyone can sign with them. Never use them
for real events, and never put a real key in a program.

The broken cases:

- `"a\xC3b"` has a UTF-8 lead byte with no continuation byte:
  `N54_UTF8`. `"?!"` has nothing left: `N54_EMPTY`.
- A wiki article with `d` `Wiki Article` is `N54_NOT_NORMAL`; one with no
  `d` tag is `N54_NO_D`.
- Key 3 opening key 1's wrap is `N37_NOT_SELF`. When key 3 decrypts the
  content anyway, with its own conversation key for key 1, NIP-44 refuses
  with `NIP44_MAC` (4).
- An article with no `d` tag is `N23_NO_D`; one whose `published_at` is one
  second after its `created_at` is `N23_LATER`; one whose `published_at` is
  `yesterday` is `N23_PUBLISHED`; the article's tags on kind 1 are
  `N23_KIND`.
- A wrap with blank content is `N37_DELETED`; one with no `k` tag is
  `N37_K`; one whose `k` tag says 1 for a kind 30023 draft is
  `N37_MISMATCH`.

The program's exit status is 0.

## How the code is checked

### Checks in the library

Each file holds its own checks, written before the code.

- `nip54.coax`: NIP-54's eight examples and six wikilink targets; dashes,
  punctuation and control whitespace; empty results, bad UTF-8 and short
  outputs; every lowercase range at both ends and just outside; every class
  range at both ends and the code points just outside, against a second copy
  of the table in the fixtures; idempotence (a normalized name normalizes to
  itself); Greek, Latin Extended-A, fullwidth and emoji titles; and the
  reader's refusals.
- `nip23.coax`: NIP-23's example tags; all four metadata tags with escapes;
  `published_at` at and after `created_at`, and eight malformed values;
  every field against a buffer one byte short; the writer's output, escapes
  and refusals; `a` coordinates for addressable and other kinds; and the
  current version among several, with other authors, kinds, `d` values and
  broken records that must never win.
- `nip37.coax`: draft kinds in many JSON shapes; the wrap's tags; views,
  drafts and bad secrets refused before any curve work; key 1 sealing a
  draft to itself, compared byte for byte with a payload computed by an
  independent Python NIP-44; opening it, and every refusal in the order
  above, including public keys that differ from key 1's only in the first or
  the last byte; and the key workspace zeroed after every return.

```shell
coax/bin/coax test coax/lib/nostr/nip54.coax
coax/bin/coax test coax/lib/nostr/nip23.coax
coax/bin/coax test coax/lib/nostr/nip37.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/nip54.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip23.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip37.coax --root-only --run-steps 250000000
```

On 2026-11-01, `coax test` ran 218 checks for `nip54.coax`, 221 for
`nip23.coax` and 343 for `nip37.coax`, each count including the loaded
files' own, in about 5, 5 and 24 seconds. Root-only mutation killed 250 of
250 mutants in `nip54.coax`, 117 of 117 in `nip23.coax` and 179 of 179 in
`nip37.coax`. The first runs left survivors of four sorts:

- Mutants in the test helpers. The helpers moved to the fixtures files, as
  chapter 42's did.
- Range ends that a mutant could move without changing any result, such as
  an even range that ended on an odd code point, or a removed range that
  ended next to a whitespace one. The Latin Extended-A ranges became
  exclusive, and the classes became one table whose overlaps the checks
  see.
- Cleanup of secrets in locals that no check could observe. The key work
  moved into the caller's `Nip37Keys`, which the checks fill with 165 and
  then find zeroed.
- Kind 0 drafts, and public keys that differ from the opener's only in the
  first byte. New checks cover both.

### Checks in the example

The example's checks compare the code with values computed outside Coax,
with text copied from NIP-54 and with the hand work above.

<!-- source: nostr/examples/checks/long-form.coax check -->
```jai
// NIP-54's normalization examples and wikilink targets, copied from
// nostr/nips/official/54.md.
NIP54_IN :: [14] string.["Wiki Article", "What's Up?", "  Hello  World  ", "Article 1", "ウィキペディア", "Ñoño", "Москва", "日本語 Article",
  "cryptocurrency", "proof of work", "Lightning Network", "ビットコイン", "日本語 記事", "Биткойн"];
NIP54_OUT :: [14] string.["wiki-article", "whats-up", "hello-world", "article-1", "ウィキペディア", "ñoño", "москва", "日本語-article",
  "cryptocurrency", "proof-of-work", "lightning-network", "ビットコイン", "日本語-記事", "биткойн"];
// The three titles the chapter normalizes by hand, and its hand results.
HAND_IN :: [3] string.["Rock & Roll (1950s)", "Москва — столица!", "  ¿Qué es Nostr?  "];
HAND_OUT :: [3] string.["rock-roll-1950s", "москва-столица", "qué-es-nostr"];
// The a coordinate written out by hand: kind, key 1's public key (SEC 2's
// generator x), d.
HAND_A :: "30023:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:lorem-ipsum";
// IDs that Python's hashlib computed from
// json.dumps([0, pubkey, created_at, kind, tags, content], separators=(",", ":"),
// ensure_ascii=False). They share no code with Coax.
PY_V1 :: "e71d4117133381631fbafbd92c1ccd72b69ee07f47f6332458310d74ba856591";
PY_V2 :: "f9fe58d4fc08bbe74f30053cadd93e4846fc6165f534aec0c99344881eba6350";
PY_V3 :: "1166786c8fefe9035aa46551909f4e4c2929a306f8c0c73c1cca18c1638bc4e4";
PY_WRAP :: "94794d50ab04bb1c6731ec03efab1e2a6c3adedae858311465b6248a7ea7642c";
// A pure-Python NIP-44 v2 (stdlib hmac and hashlib, a hand ChaCha20 and
// secp256k1, checked first against nip44.vectors.json encrypt_decrypt row
// 0): key 1's conversation key with itself, key 3's with key 1, and DRAFT
// sealed by key 1 to itself with NONCE.
PY_CK_SELF :: "3b4610cb7189beb9cc29eb3716ecc6102f1247e8f3101a03a1787d8908aeb54e";
PY_CK_3_TO_1 :: "68ace26dd21fd98a8781d65588d0a7bfb3746974cafdfbf8ea797fb42d251b9d";
PY_PAYLOAD :: "AgABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fbgrW1PBXWEOJoAzWm+GbZ1h8NhsC4kv1spIKwySRhkbzlNV0C/yEB2Q0z53wJ3El8OeWsSy/MRXEh49F54iIyBx9ycGqWVw/uqrKprM3uF37XoWG2opxOVWwqKaClSJ1oezIkS5uDCm5l3qgwXtcvKcIBppcqlhjNji5rHYfeIZpPvbl+TFs7ABan0q0uNi5w9dyzxkokycpdRVevz5fKDd+ZsKc0KNEXbrwCbMqd9Gv2tI1MrZdC9Zuh8s1H623Ekc=";

same_id :: (id: [32] u8, text: string) -> bool {
  want := bytes_of(text);
  for i: 0..<32 { if id[i] != want[i] { return false; } }
  return true;
}
d_of :: (title: string, out: [] u8) -> string {
  r := nip54_normalize(title, out); #assert r.error == N54_OK; return cast(string) out[0..<r.n];
}

#forall "NIP-54's examples give the stated d values" i: 0..<14 {
  b: [64] u8; #assert d_of(NIP54_IN[i], b[..]) == NIP54_OUT[i];
}
#forall "the chapter's three hand-normalized titles" i: 0..<3 {
  b: [64] u8; #assert d_of(HAND_IN[i], b[..]) == HAND_OUT[i];
}
#test "the article's a coordinate is the hand-written one and reads back" {
  out: [128] u8; n := nip23_address(N23_ARTICLE, bytes_of("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"), "lorem-ipsum", out[..]);
  #assert cast(string) out[0..<n] == HAND_A;
  t := nt_address(HAND_A); #assert t.error == NT_OK && t.address_kind == 30023 && cast(string) t.d[0..<t.d_n] == "lorem-ipsum";
}
#test "three versions: Python's ids; the current one is the newest, then the lowest id" #steps 1500000000 {
  m: N23Meta; m.d = "lorem-ipsum"; m.title = "Lorem Ipsum"; m.summary = "A placeholder article."; m.published_at = PUBLISHED;
  tb: [256] u8; t := nip23_tags(m, tb[..]); tags := cast(string) tb[0..<t.n];
  b1: [2048] u8; b2: [2048] u8; b3: [2048] u8; s1: [8192] u8; s2: [8192] u8; s3: [8192] u8;
  set: [3] NostrEvent;
  set[0] = nostr_event_verify_vartime(sign(KEY_1, CREATED, N23_ARTICLE, tags, "Lorem ipsum dolor sit amet.", b1[..]), s1[..], false);
  set[1] = nostr_event_verify_vartime(sign(KEY_1, CREATED + 600, N23_ARTICLE, tags, "Lorem ipsum dolor sit amet, edited.", b2[..]), s2[..], false);
  set[2] = nostr_event_verify_vartime(sign(KEY_1, CREATED + 600, N23_ARTICLE, tags, "Lorem ipsum dolor sit amet, edited again.", b3[..]), s3[..], false);
  for i: 0..<3 { #assert set[i].error == NE_OK && set[i].verified; }
  #assert same_id(set[0].id, PY_V1) && same_id(set[1].id, PY_V2) && same_id(set[2].id, PY_V3);
  // By hand: 2 and 3 share the newest created_at; 3's id starts 11, 2's f9.
  key1 := set[0].public_key;
  #assert nip23_current(set[..], N23_ARTICLE, key1, "lorem-ipsum") == 2;
}
#test "the draft: Python's conversation keys, payload and wrap id; key 3 can't read it" #steps 2000000000 {
  k1 := bytes_of(KEY_1); k3 := bytes_of(KEY_3); nonce := bytes_of(NONCE);
  pub1 := bytes_of("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798");
  ec: [1] EcdhScratch; ck: [32] u8;
  #assert nip44_conversation_key_from_keys_checked(k1[..], pub1[..], ec[..], ck[..], false) == NIP44_OK && same_id(ck, PY_CK_SELF);
  #assert nip44_conversation_key_from_keys_checked(k3[..], pub1[..], ec[..], ck[..], false) == NIP44_OK && same_id(ck, PY_CK_3_TO_1);
  db: [512] u8; for i: 0..<DRAFT.count { db[i] = DRAFT[i]; }
  work: [4096] u8; pb: [1024] u8; keys: [1] Nip37Keys;
  sl := nip37_seal(k1[..], nonce[..], db[0..<DRAFT.count], keys[..], work[..], pb[..], false);
  #assert sl.error == N37_OK && cast(string) pb[0..<sl.written] == PY_PAYLOAD;
  wt: [128] u8; wn := nip37_tags("lorem-ipsum-draft", 30023, CREATED + 90 * 86400, wt[..]);
  wb: [2048] u8; s: [8192] u8;
  w := nostr_event_verify_vartime(sign(KEY_1, CREATED, N37_WRAP, cast(string) wt[0..<wn], PY_PAYLOAD, wb[..]), s[..], false);
  #assert w.error == NE_OK && w.verified && same_id(w.id, PY_WRAP);
  ob: [1024] u8;
  d := nip44_decrypt_checked(NIP44_COMPAT, ck[..], PY_PAYLOAD, work[..], ob[..], false);
  #assert d.error == NIP44_MAC && d.written == 0;
}
#test "same_id notices a change in the first or last byte" {
  id: [32] u8; zero := "0000000000000000000000000000000000000000000000000000000000000000";
  #assert same_id(id, zero);
  id[0] = 1; #assert !same_id(id, zero);
  id[0] = 0; id[31] = 1; #assert !same_id(id, zero);
}
```

- All fourteen NIP-54 examples and wikilink targets give the NIP's `d`
  values, and the three hand-worked titles give the hand results.
- The `a` coordinate equals the hand-written string and reads back through
  chapter 22's address reader.
- The three versions verify, have the IDs Python's `hashlib` computes, and
  the current one is version 3, as the hand table says.
- Key 1's conversation key with itself, key 3's with key 1, the sealed
  payload and the wrap's ID equal the values of a pure-Python NIP-44
  (stdlib `hmac` and `hashlib`, a hand ChaCha20 and secp256k1), which was
  first checked against `nip44.vectors.json`'s `encrypt_decrypt` row 0. Key
  3's conversation key fails NIP-44's MAC check on the payload.

```shell
coax/bin/coax test nostr/examples/checks/long-form.coax --run-steps 250000000
coax/bin/coax mutate nostr/examples/checks/long-form.coax --root-only --run-steps 250000000
```

On 2026-11-01, `coax test` ran 482 checks in about 68 seconds. Root-only mutation killed 9 of 9 mutants in the checks file.
Mutation of the example's own `main` was not run; its output is pinned by
the fixture.

### What is not built

- Markdown and Djot. The content of a NIP-23 article is Markdown and a
  NIP-54 article is Djot; neither is parsed or rendered. Wikilink
  resolution is only the normalization of the target.
- NIP-54 merge requests (kind 818), redirects (kind 30819), `fork` and
  `defer` markers, and the ways a client may rank many articles with the
  same name (reactions, relay lists, contact lists).
- NIP-37 checkpoints (kind 1234), the private relay list (kind 10013) and
  enforcing the `expiration` tag (NIP-40). `nip37_tags` writes the tag; the
  store of [chapter 34](ch034-admit-an-event.md) decides expiry.
- Decrypting kind 30024 drafts, which used NIP-04.
- `naddr` encoding of the coordinate. NIP-19's TLV codes are
  [chapter 16](ch016-nip-19-entities-and-nostr-uris.md)'s.
- Lowercasing for cased scripts beyond the ranges listed above (Latin
  Extended-B and Additional, Armenian, Georgian, Cherokee, Deseret). A
  title in those scripts keeps its case. Full Unicode case folding needs the
  Unicode character database.

## Summary

- Articles, drafts and wiki pages are addressable events: kind, author and
  `d` name them, and the newest version, then the lowest ID, is current.
- NIP-23 metadata are tags; `published_at` is the first publication and
  can't follow `created_at`.
- An article's `a` coordinate is `kind:author:d`, 82 bytes for the
  example's article.
- A NIP-37 draft is sealed with NIP-44 to your own key. The conversation
  key is `x(s·s·G)`, so only the author opens it; another key fails the MAC.
- NIP-54 `d` values are normalized one code point at a time: lowercase,
  whitespace to one `-`, punctuation and symbols removed, ends trimmed.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)

## Sources

- [NIP-23](../nips/official/23.md), pinned, with its example event.
- [NIP-37](../nips/official/37.md).
- [NIP-54](../nips/official/54.md), with its normalization and wikilink
  examples.
- [NIP-44](../nips/official/44.md) for the conversation key.
- [NIP-01](../nips/official/01.md) for addressable events and replacement.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
