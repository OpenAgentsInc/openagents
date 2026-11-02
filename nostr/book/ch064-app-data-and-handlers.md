# 64. App data and handlers

Apps store their own data on relays, advertise which event kinds they can
open, and publish static websites as manifests of file hashes. This chapter
reads and writes all three, finds the app that should open an event of a
kind your client doesn't know, fills that app's URL template with the
event's NIP-19 entity, and refuses a site file whose hash differs from its
manifest.

Part VI, Every NIP. Result: NIP-78 kind 30078 application data, NIP-89
handler recommendations (31989) and announcements (31990), NIP-5A static
websites (35128 and 15128 manifests), and NIP-CD slash commands (31992,
10992), which are described and not built.

## What this chapter teaches

- NIP-78: kind 30078 with any `d`, whose content belongs to the app, and
  why a relay serves it only to its author.
- NIP-89: kind 31990 lists the kinds an app handles in `k` tags and gives
  a URL template per platform; kind 31989 recommends handlers for one
  kind; the `client` tag names the app that published an event.
- Handler resolution: which apps a client offers for a kind, with the
  recommendations of the user's follows first, and which recommendations
  it ignores.
- Filling a template: the NIP-19 entity (`naddr` or `nevent`) built byte
  by byte, and the templates a client must refuse to open.
- NIP-5A: site manifests that map paths to SHA-256 hashes, the aggregate
  hash over them, and the host's check of every fetched file.
- NIP-CD: slash command definitions, what they would need, and why they
  are not built here.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag(e, i)`, `nostr_tag_d(e)` | Reads tag `i`: an `a` tag's address and relay; the `d` identifier (chapter 22) |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_tag(e, i)` | Reads tag `i` of any name: the name and its first two values (chapter 55) |
| [nip23.coax](../../coax/lib/nostr/nip23.coax) | `nip23_address`, `nip23_current` | Writes an `a` coordinate; picks the current version of an addressable event (chapter 53) |
| [nip19-core.coax](../../coax/lib/nostr/nip19-core.coax) | `nip19_encode_checked`, `n19_decode` | Encodes and decodes NIP-19 strings (chapter 16) |
| [nip78.coax](../../coax/lib/nostr/nip78.coax) | `n78_read(e)`, `n78_serve`, `n78_tags` | A kind 30078 or 78 event's `d`; whether a relay serves it to a client; its tags |
| [nip89.coax](../../coax/lib/nostr/nip89.coax) | `n89_handler(e)`, `n89_recommend(e)`, `n89_client(e)` | Read a kind 31990, a kind 31989 and the `client` tag |
| [nip89.coax](../../coax/lib/nostr/nip89.coax) | `n89_resolve(kind, follows, recs, handlers, out)` | Which handlers open a kind, best first |
| [nip89.coax](../../coax/lib/nostr/nip89.coax) | `n89_entity(e, relay, out)`, `n89_url(h, platform, entity, out)` | An event's `naddr` or `nevent`; a handler's URL for it, or the refusal |
| [nip89.coax](../../coax/lib/nostr/nip89.coax) | `n89_handler_tags`, `n89_recommend_tags`, `n89_client_tag` | Write a kind 31990's tags, a kind 31989's tags and a `client` tag |
| [nip5a.coax](../../coax/lib/nostr/nip5a.coax) | `n5a_read(e)`, `n5a_aggregate` | A kind 15128, 35128 or 5128 manifest: its files and aggregate hash |
| [nip5a.coax](../../coax/lib/nostr/nip5a.coax) | `n5a_lookup(s, request)`, `n5a_check(s, file, blob)` | The file a request names; whether a fetched blob may be served |
| [nip5a.coax](../../coax/lib/nostr/nip5a.coax) | `n5a_tags` | Write a manifest's tags with its aggregate `x` tag |

`nip78.coax`, `nip89.coax` and `nip5a.coax` are new with this chapter.
Each is an `#authority` file: the first decides who may read app data, the
second which URL a client opens, the third which bytes a host serves. Every
procedure runs in a check and `coax mutate` leaves no survivors. They
verify no signature and fetch nothing. The caller verifies the events
(chapter 21) before it passes them in, and fetches the blobs.

## NIP-78: application data

NIP-78 gives an app a place to keep its own data, such as settings, on the
user's relays. It defines two kinds:

- Kind 30078, an addressable event. Its `d` tag names the app and the
  context, for example `coax-reader/settings`. Each new version replaces
  the last one with the same author and `d`.
- Kind 78, a regular event, for an app that stores many records of one
  type. The app groups them with a tag of its choice.

The content and the other tags can be anything. Nothing in this chapter
reads them: they are the app's own format, and other apps aren't meant to
understand them. The reader takes the first `d` tag and refuses a kind
30078 without one or with an empty one, because every app's data under one
key would then replace every other app's.

NIP-78 also says the data is private to the user: a relay SHOULD require
NIP-42 AUTH (chapter 38) and serve these kinds only to a client that
authenticated as the author. `n78_serve` makes that decision from the
pubkey the caller authenticated.

<!-- source: coax/lib/nostr/nip78.coax record -->
```jai
N78Data :: struct {
  error:int=N78_EVENT;
  kind:int;
  has_d:bool; d:N32Text;              // the first d tag's value
}
```

<!-- source: coax/lib/nostr/nip78.coax read -->
```jai
// A kind 30078 or 78 event's d tag.
n78_read :: (e:NostrEvent)->N78Data {
  r:N78Data;if e.error!=NE_OK {return r;}
  t:=n10_text(e,"d",r.d.b[..]);
  if t.status==N10_LIMIT {r.error=N78_LIMIT;return r;}
  if t.status!=N10_OK {return r;}
  r.has_d=t.present;r.d.n=t.n;
  if e.kind!=N78_APP && e.kind!=N78_REGULAR {r.error=N78_KIND;return r;}
  r.kind=e.kind;
  if e.kind==N78_APP && r.d.n==0 {r.error=N78_NO_D;return r;}
  r.error=N78_OK;return r;
}
// Whether a relay serves e to a client that authenticated (NIP-42) as who:
// app data only to its author, everything else to anyone.
n78_serve :: (e:NostrEvent,authenticated:bool,who:[32]u8)->bool {
  if e.kind!=N78_APP && e.kind!=N78_REGULAR {return true;}
  return authenticated && ns_equal32(e.public_key,who);
}
```

The data is still in the clear on the relay. An app that needs privacy
from the relay operator encrypts the content first, for example to the
user's own key with NIP-44 (chapter 20), as chapter 54 does for private
list items.

## NIP-89: handlers and recommendations

A client sees an event whose kind it doesn't display, such as a kind 30023
article in a client built for short notes. NIP-89 lets it find another app
that does display it. Three parties take part:

1. The app publishes a kind 31990 *handler information* event: the kinds
   it handles, one `k` tag each, and a URL template per platform.
2. A user who uses the app publishes a kind 31989 *recommendation* for one
   kind. Its `d` is the kind, and its `a` tags name 31990 handlers.
3. A second user's client fetches the 31989 recommendations of that user's
   follows for the kind, then the 31990 events they name, and opens the
   chosen app's URL with the event's NIP-19 entity in it.

The tags, from the NIP:

| Kind | Tag | Meaning |
| --- | --- | --- |
| 31990 | `["d", <id>]` | The handler's identifier |
| 31990 | `["k", <kind>]` | A kind it handles; one tag per kind |
| 31990 | `[<platform>, <url>, <entity type>]` | A URL template for `web`, `ios`, `android`, ...; the optional third value names the NIP-19 type it takes |
| 31989 | `["d", <kind>]` | The kind recommended for, in decimal |
| 31989 | `["a", "31990:<pubkey>:<d>", <relay>, <platform>]` | A recommended handler |
| any | `["client", <name>, "31990:<pubkey>:<d>", <relay>]` | The app that published this event |

A template contains `<bech32>`, which the client replaces with the entity
to load. A template tag without a third value is the generic one for any
entity that no other tag of that platform names.

<!-- source: coax/lib/nostr/nip89.coax record -->
```jai
N89Handler :: struct {
  error:int=N89_EVENT; at:int=-1;     // the refusing tag
  d:N32Text;
  k:[N89_MAX_K]int; nk:int;           // the kinds its k tags list
}
N89Ref :: struct {author:[32]u8; d:N32Text; relay:N32Text; platform:N32Text;}   // 31990:author:d
N89Recommend :: struct {
  error:int=N89_EVENT; at:int=-1;
  kind:int;                           // the d tag: the kind recommended for
  a:[N89_MAX_A]N89Ref; n:int;
}
N89Client :: struct {
  error:int=N89_EVENT; at:int=-1;
  present:bool; name:N32Text;
  has_handler:bool; handler:N89Ref;   // the 31990 address, when given
}
// One resolved handler: its index in the caller's handler events.
N89Choice :: struct {handler:int; votes:int; rank:int; last:int;}
N89Found :: struct {error:int=N89_OK; n:int; not_handled:int; missing:int; broken:int;}
N89Url :: struct {error:int=N89_EVENT; at:int=-1; n:int;}
// What the writers take.
N89Template :: struct {platform:string; url:string; entity:string;}
N89Pick :: struct {author:[32]u8; d:string; relay:string; platform:string;}
```

### Reading kinds and tags

A kind in a `d` or `k` tag is written in decimal. `n89_kind_of` accepts
`0` to `65535` with no sign and no leading zero, so `030023` and `30023`
can't both name the same kind.

<!-- source: coax/lib/nostr/nip89.coax kind -->
```jai
// A kind written in decimal, as a d or k tag holds it: 0..65535 with no
// sign and no leading zero, or -1.
n89_kind_of :: (s:string)->int {
  if s.count==0 || (s[0]=='0' && s.count>1) {return -1;}
  v:=0;
  for c:s {
    if c<'0' || c>'9' {return -1;}
    v=v*10+cast(int)(c-'0');
    if v>65535 {return -1;}
  }
  return v;
}
```

The readers take the first `d` tag, every `k` or `a` tag, and the first
`client` tag. A handler with no `k` tag handles nothing and is refused. A
recommendation's `a` tag must name a kind 31990; one that names anything
else is refused at its tag.

<!-- source: coax/lib/nostr/nip89.coax read -->
```jai
// A kind 31990 handler: its d and the kinds its k tags list.
n89_handler :: (e:NostrEvent)->N89Handler {
  h:N89Handler;if e.error!=NE_OK {return h;}
  if e.kind!=N89_HANDLER {return n89_h_fail(h,N89_KIND,-1);}
  i:=0;has_d:=false;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n89_h_fail(h,N89_LIMIT,i);}
    if t.error!=NT_OK {return n89_h_fail(h,N89_EVENT,i);}
    if n32_is(t.v[0],"d") && !has_d {has_d=true;h.d=t.v[1];}
    if n32_is(t.v[0],"k") {
      k:=n89_kind_of(cast(string)t.v[1].b[0..<t.v[1].n]);
      if k<0 || t.values<2 {return n89_h_fail(h,N89_K,i);}
      if h.nk==N89_MAX_K {return n89_h_fail(h,N89_LIMIT,i);}
      h.k[h.nk]=k;h.nk+=1;
    }
    i+=1;
  }
  if h.d.n==0 {return n89_h_fail(h,N89_NO_D,-1);}
  if h.nk==0 {return n89_h_fail(h,N89_K,-1);}
  return n89_h_fail(h,N89_OK,-1);
}
n89_handles :: (h:N89Handler,kind:int)->bool {
  for i:0..<h.nk {if h.k[i]==kind {return true;}}
  return false;
}
// A kind 31989 recommendation: the kind its d names and the 31990
// handlers its a tags name, each with a relay hint and a platform.
n89_recommend :: (e:NostrEvent)->N89Recommend {
  r:N89Recommend;if e.error!=NE_OK {return r;}
  if e.kind!=N89_RECOMMEND {return n89_r_fail(r,N89_KIND,-1);}
  i:=0;d:N32Text;has_d:=false;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n89_r_fail(r,N89_LIMIT,i);}
    if t.error!=NT_OK {return n89_r_fail(r,N89_EVENT,i);}
    if n32_is(t.v[0],"d") && !has_d {has_d=true;d=t.v[1];}
    if n32_is(t.v[0],"a") {
      s:=nostr_tag(e,i);
      if s.error!=NT_OK {return n89_r_fail(r,N89_TAG,i);}
      if s.address_kind!=N89_HANDLER {return n89_r_fail(r,N89_ADDRESS,i);}
      if r.n==N89_MAX_A {return n89_r_fail(r,N89_LIMIT,i);}
      a:N89Ref;a.author=s.pubkey;a.d=n53_copy(s.d,s.d_n);a.relay=n53_copy(s.relay,s.relay_n);
      a.platform.n=n53_value(e,i,3,a.platform.b[..]);
      r.a[r.n]=a;r.n+=1;
    }
    i+=1;
  }
  if d.n==0 {return n89_r_fail(r,N89_NO_D,-1);}
  r.kind=n89_kind_of(cast(string)d.b[0..<d.n]);
  if r.kind<0 {return n89_r_fail(r,N89_K,-1);}
  return n89_r_fail(r,N89_OK,-1);
}
// The first client tag: a name, then optionally a 31990 address and a
// relay hint. No client tag reads as not present.
n89_client :: (e:NostrEvent)->N89Client {
  c:N89Client;if e.error!=NE_OK {return c;}
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n89_c_fail(c,N89_LIMIT,i);}
    if t.error!=NT_OK {return n89_c_fail(c,N89_EVENT,i);}
    if n32_is(t.v[0],"client") {
      c.present=true;c.name=t.v[1];
      if c.name.n==0 {return n89_c_fail(c,N89_TAG,i);}
      if t.v[2].n>0 {
        a:=nt_address(cast(string)t.v[2].b[0..<t.v[2].n]);
        if a.error!=NT_OK {return n89_c_fail(c,N89_TAG,i);}
        if a.address_kind!=N89_HANDLER {return n89_c_fail(c,N89_ADDRESS,i);}
        c.has_handler=true;c.handler.author=a.pubkey;c.handler.d=n53_copy(a.d,a.d_n);
        c.handler.relay.n=n53_value(e,i,3,c.handler.relay.b[..]);
        if c.handler.relay.n>0 && !nt_relay_ok(cast(string)c.handler.relay.b[0..<c.handler.relay.n]) {return n89_c_fail(c,N89_TAG,i);}
      }
      return n89_c_fail(c,N89_OK,-1);
    }
    i+=1;
  }
  return n89_c_fail(c,N89_OK,-1);
}
```

## Handler resolution

`n89_resolve` takes the kind, the user's follows and the 31989 and 31990
events the caller fetched (for example with the NIP's filters
`{"kinds": [31989], "#d": ["30023"], "authors": [<follows>]}` and
`{"kinds": [31990], "#k": ["30023"]}`). It returns handler indexes, best
first:

1. **Recommended by follows.** Each follow's current 31989 for the kind
   gives one vote to each handler it names, once per follow. A handler
   counts only when its current 31990 lists the kind in a `k` tag; a
   recommendation can't add a kind the app never claimed. More votes go
   first.
2. **A follow's own app.** A current 31990 for the kind whose author is a
   follow and that no follow recommends.
3. **Unvetted.** Every other current 31990 for the kind. NIP-89 warns that
   querying 31990 directly invites spam, so a client shows these last or
   not at all; the rank lets it choose.

A recommendation from someone the user doesn't follow is ignored. A
follow's recommendation that doesn't read is skipped and counted in
`broken`, so one bad event can't hide every handler. Recommendations whose
handler isn't in the fetched set are counted in `missing`, and ones whose
handler doesn't list the kind in `not_handled`.

<!-- source: coax/lib/nostr/nip89.coax resolve -->
```jai
n89_follows :: (follows:[][32]u8,key:[32]u8)->bool {
  for f:follows {if ns_equal32(f,key) {return true;}}
  return false;
}
// Whether handler event e reads and lists kind in a k tag.
n89_lists :: (e:NostrEvent,kind:int)->bool {h:=n89_handler(e);return h.error==N89_OK && n89_handles(h,kind);}
// Whether e is the current version of its own address among set.
n89_current :: (set:[]NostrEvent,i:int)->bool {
  e:=set[i];t:=nostr_tag_d(e);
  if t.error!=NT_OK {return false;}
  return nip23_current(set,e.kind,e.public_key,cast(string)t.d[0..<t.d_n])==i;
}
// The choice for handler index h in out[0..<n], adding it at out[n] when
// new: its position, or -1 when out is full.
n89_choice :: (out:[]N89Choice,n:int,h:int,rank:int)->int {
  for c:0..<n {if out[c].handler==h {return c;}}
  if n==out.count {return -1;}
  out[n]=N89Choice.{handler=h,rank=rank,last=-1};return n;
}
// Which handlers open kind, from recommendations and handler events the
// caller fetched and verified, best first, into out. A follow's
// recommendation that doesn't read is skipped and counted in broken, so
// one bad event can't hide every handler.
n89_resolve :: (kind:int,follows:[][32]u8,recs:[]NostrEvent,handlers:[]NostrEvent,out:[]N89Choice)->N89Found {
  f:N89Found;
  if kind<0 || kind>65535 {f.error=N89_K;return f;}
  // 1. Each follow's current recommendation for kind votes once for each
  //    handler it names whose current 31990 lists the kind.
  for i:0..<recs.count {
    e:=recs[i];
    if e.kind!=N89_RECOMMEND || !n89_follows(follows,e.public_key) || !n89_current(recs,i) {continue;}
    r:=n89_recommend(e);
    if r.error!=N89_OK {f.broken+=1;continue;}
    if r.kind!=kind {continue;}
    for a:0..<r.n {
      h:=nip23_current(handlers,N89_HANDLER,r.a[a].author,cast(string)r.a[a].d.b[0..<r.a[a].d.n]);
      if h==N23_NONE {f.missing+=1;continue;}
      if !n89_lists(handlers[h],kind) {f.not_handled+=1;continue;}
      c:=n89_choice(out,f.n,h,N89_BY_FOLLOWS);
      if c<0 {f.error=N89_LIMIT;return f;}
      if c==f.n {f.n+=1;}
      if out[c].last!=i {out[c].votes+=1;out[c].last=i;}
    }
  }
  // 2. Every other current handler that lists the kind: a follow's app,
  //    then the rest.
  for h:0..<handlers.count {
    e:=handlers[h];
    if !n89_current(handlers,h) || !n89_lists(e,kind) {continue;}
    rank:=ifx n89_follows(follows,e.public_key) then N89_FOLLOW_APP else N89_UNVETTED;
    c:=n89_choice(out,f.n,h,rank);
    if c<0 {f.error=N89_LIMIT;return f;}
    if c==f.n {f.n+=1;}
  }
  // 3. Best first: rank, then more votes; ties keep the order found.
  for i:1..<f.n {
    j:=i;
    while j>0 && (out[j].rank<out[j-1].rank || (out[j].rank==out[j-1].rank && out[j].votes>out[j-1].votes)) {
      t:=out[j];out[j]=out[j-1];out[j-1]=t;j-=1;
    }
  }
  return f;
}
```

### Worked example: which app opens Bob's article?

Alice follows Bob and Carol. Bob publishes a kind 30023 article, `d`
`coax-intro`. The fetched events are:

| Event | Author | Says |
| --- | --- | --- |
| H1 | Dave | 31990 `reads`: `k` 30023 and 30024; web templates for `naddr` and generic |
| H2 | Eve | 31990 `spam`: `k` 30023 |
| H3 | Carol | 31990 `notes`: `k` 1 only |
| R1 | Bob | 31989 for 30023: Dave's `reads` |
| R2 | Carol | 31989 for 30023: Dave's `reads`, Carol's `notes` |
| R3 | Eve | 31989 for 30023: Eve's `spam` |

Work through the steps by hand for kind 30023:

1. R1 is by Bob, a follow. It names `reads`; H1 lists 30023. `reads` has
   1 vote.
2. R2 is by Carol, a follow. It names `reads`: 2 votes. It names `notes`;
   H3 lists only kind 1, so `notes` is not returned and `not_handled` is 1.
3. R3 is by Eve, who Alice doesn't follow. It is ignored.
4. The remaining 31990 events for 30023: H2 by Eve. Eve isn't a follow,
   so H2 is unvetted.

The answer: H1 (`reads`, recommended by follows, 2 votes), then H2
(`spam`, unvetted, 0 votes); 1 not handled, 0 missing. Section 4 of the
example prints the same.

## Filling the template

The handler loads the event from a NIP-19 entity (chapter 16). For an
addressable kind such as 30023 it is an `naddr`, which names the event by
kind, author and `d`, so it still opens the newest version after Bob
edits the article. For any other kind it is an `nevent`, which names one
event by id. `n89_entity` writes both TLV forms in the same order: the
special field (type 0: `d` or the id), an optional relay (type 1), the
author (type 2) and the kind as 4 big-endian bytes (type 3).

<!-- source: coax/lib/nostr/nip89.coax entity -->
```jai
// The NIP-19 entity a handler loads for event e: naddr (d, relay, author,
// kind) for an addressable kind, else nevent (id, relay, author, kind).
// An empty relay is left out; the kind is 4 bytes big-endian, and NIP-01
// kinds fit in the low two. Its length in out, or -1.
n89_entity :: (e:NostrEvent,relay:string,out:[]u8)->int {
  if e.error!=NE_OK || relay.count>255 || e.kind<0 || e.kind>65535 {return -1;}
  tlv:[2+NT_MAX_VALUE+2+255+2+32+2+4]u8;n:=34;
  addressable:=ns_class(e.kind)==NS_ADDRESSABLE;
  if addressable {
    d:=nostr_tag_d(e);
    if d.error!=NT_OK || d.d_n>255 {return -1;}
    tlv[1]=cast(u8)d.d_n;for i:0..<d.d_n {tlv[2+i]=d.d[i];}n=2+d.d_n;
  } else {
    tlv[1]=32;for i:0..<32 {tlv[2+i]=e.id[i];}
  }
  if relay.count>0 {
    tlv[n]=1;tlv[n+1]=cast(u8)relay.count;
    for i:0..<relay.count {tlv[n+2+i]=relay[i];}n+=2+relay.count;
  }
  tlv[n]=2;tlv[n+1]=32;for i:0..<32 {tlv[n+2+i]=e.public_key[i];}n+=34;
  tlv[n]=3;tlv[n+1]=4;tlv[n+4]=cast(u8)((e.kind>>8)&255);tlv[n+5]=cast(u8)(e.kind&255);n+=6;
  w:=nip19_encode_checked(ifx addressable then N19_ADDR else N19_EVENT,cast(string)tlv[0..<n],out,false);
  if w.error!=NC_OK {return -1;}
  return w.written;
}
```

### Worked example: the naddr byte by byte

Bob's article with the relay hint `wss://relay.example.com`:

| Bytes | Type | Length | Value |
| --- | --- | --- | --- |
| `00 0a` | 0, `d` | 10 | `coax-intro` = `63 6f 61 78 2d 69 6e 74 72 6f` |
| `01 17` | 1, relay | 23 | `wss://relay.example.com` |
| `02 20` | 2, author | 32 | Bob's key `c6047f94…709ee5` |
| `03 04` | 3, kind | 4 | 30023 = 0x7547 = `00 00 75 47` |

That is 2 + 10 + 2 + 23 + 2 + 32 + 2 + 4 = 77 bytes. Bech32 with the
prefix `naddr` turns them into
`naddr1qq9xxmmp0qkkjmn5wfhsz9mhwden5te0wfjkccte9ejhsctdwpkx2tnrdaksygxxq3legs0d04knq32qd62uqlxct3mcujuvau7202avpxu4cuy7u5psgqqqw4rss6c9tf`.
The first data characters follow from the first bytes: `00 0a 63 6f` are
the bits `00000000 00001010 01100011 01101111`, read in groups of five as
0, 0, 5, 6, 6, which are `q q 9 x x` in the Bech32 alphabet.

H1 has two `web` tags. The entity's type is `naddr`, and tag 3 names
`naddr`, so it wins over the generic tag 4. Replacing `<bech32>` in
`https://reads.example.com/a/<bech32>` gives the URL the client opens:

```text
https://reads.example.com/a/naddr1qq9xxmmp0qkkjmn5wfhsz9mhwden5te0wfjkccte9ejhsctdwpkx2tnrdaksygxxq3legs0d04knq32qd62uqlxct3mcujuvau7202avpxu4cuy7u5psgqqqw4rss6c9tf
```

For Alice's kind 1 note no tag names `nevent`, so the generic tag 4 is
used.

### Templates a client must not open

The template comes from whoever signed the 31990, and the client is about
to open it. `n89_template` decides whether it is safe, and `n89_url`
refuses before any URL is written:

- The scheme is a lowercase letter, then letters, digits, `+`, `-` or
  `.`, up to the first `:`. `javascript`, `data`, `vbscript` and `file`
  are refused on every platform: each runs code or reads local files in
  the client's context.
- A `web` template must be `https://` and a host.
- After the scheme, only printable ASCII without `"`, `<`, `>`, `\` or a
  backtick, so the URL can't break out of an attribute or a quoted string.
- The template must contain `<bech32>`. Without it, every event would open
  the same page.
- The entity must be a valid NIP-19 string, and never an `nsec`: a secret
  key must not end up in a URL.

`n89_handler_tags` applies the same check, so this code never writes a
handler that it would refuse to open.

<!-- source: coax/lib/nostr/nip89.coax url -->
```jai
N89_DENIED :: [4] string.["javascript", "data", "vbscript", "file"];
N89_SCHEME_CHARS :: "abcdefghijklmnopqrstuvwxyz0123456789+-.";
// Whether a template is safe to fill for platform: N89_OK or the refusal.
// The scheme is a lowercase letter, then letters, digits, + - or ., up to
// the first ':'; after it, printable ASCII without " < > \ ` except in the
// placeholder.
n89_template :: (url:string,platform:string)->int {
  scheme_part:=true;scheme:="";found:=false;i:=0;
  while i<url.count {
    c:=url[i];
    if scheme_part {
      if c==':' {
        if i==0 {return N89_SCHEME;}
        scheme_part=false;scheme=url[0..<i];
      }
      else {
        k:=-1;for j:0..<N89_SCHEME_CHARS.count {if N89_SCHEME_CHARS[j]==c {k=j;}}
        if k<0 || (i==0 && k>=26) {return N89_SCHEME;}
      }
    } else if url.count-i>=N89_PLACEHOLDER.count && url[i..<i+N89_PLACEHOLDER.count]==N89_PLACEHOLDER {found=true;i+=N89_PLACEHOLDER.count;continue;}
    else if c<=' ' || c>'~' || c=='"' || c=='<' || c=='>' || c=='\\' || c=='`' {return N89_URL;}
    i+=1;
  }
  if scheme_part {return N89_SCHEME;}
  for d:N89_DENIED {if scheme==d {return N89_SCHEME;}}
  if platform=="web" {
    if scheme!="https" {return N89_SCHEME;}
    // "https" is 5 bytes: "://" and a host byte that isn't another /.
    if url.count<9 || url[6..<8]!="//" || url[8]=='/' {return N89_URL;}
  }
  if !found {return N89_NO_BECH32;}
  return N89_OK;
}
// The template with every <bech32> replaced by entity, into out.
n89_fill :: (url:string,platform:string,entity:string,out:[]u8)->N89Url {
  u:N89Url;u.error=n89_template(url,platform);
  if u.error!=N89_OK {return u;}
  n:=0;i:=0;
  while i<url.count {
    if url.count-i>=N89_PLACEHOLDER.count && url[i..<i+N89_PLACEHOLDER.count]==N89_PLACEHOLDER {n=json_raw(out,n,entity);i+=N89_PLACEHOLDER.count;}
    else {n=json_raw(out,n,url[i..<i+1]);i+=1;}
  }
  if n== -1 {u.error=N89_OUTPUT;return u;}
  u.n=n;return u;
}
// The URL a handler event gives for platform and a NIP-19 entity, into out.
n89_url :: (h:NostrEvent,platform:string,entity:string,out:[]u8)->N89Url {
  u:N89Url;if h.error!=NE_OK {return u;}
  dec:[N19_MAX_BYTES]u8;r:=n19_decode(entity,dec[..],false,false);
  if r.error!=NC_OK {return n89_u_fail(u,N89_ENTITY,-1);}
  type:=n19_prefix(r.type);
  exact:=-1;generic:=-1;i:=0;
  while exact<0 {
    t:=n32_tag(h,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n89_u_fail(u,N89_LIMIT,i);}
    if t.error!=NT_OK {return n89_u_fail(u,N89_EVENT,i);}
    if n32_is(t.v[0],platform) {
      if t.values<2 {return n89_u_fail(u,N89_TAG,i);}
      if n32_is(t.v[2],type) {exact=i;}
      else if t.v[2].n==0 && generic<0 {generic=i;}
    }
    i+=1;
  }
  at:=ifx exact>=0 then exact else generic;
  if at<0 {return n89_u_fail(u,N89_NO_TEMPLATE,-1);}
  t:=n32_tag(h,at);
  f:=n89_fill(cast(string)t.v[1].b[0..<t.v[1].n],platform,entity,out);
  return n89_u_fail(f,f.error,at);
}
```

## NIP-5A: static websites

An nsite is a static website whose files are Blossom blobs, named by their
SHA-256. The site itself is an event that maps paths to hashes:

- Kind 15128: the root site, one per key, with no `d` tag.
- Kind 35128: a named site. Its `d` matches `^[a-z0-9-]{1,13}$` and
  doesn't end in `-`, because it becomes part of one DNS label.
- Kind 5128: a snapshot of either, with exactly one `a` tag naming the
  site and exactly one aggregate `x` tag.

Each `["path", "/absolute/file.ext", <sha256>]` tag maps a path to a file.
A copied site carries one `a` tag (the parent it was copied from) and one
`A` tag (the origin of the copy lineage).

<!-- source: coax/lib/nostr/nip5a.coax record -->
```jai
N5aFile :: struct {path:N32Text; hash:[32]u8;}
N5aSite :: struct {
  error:int=N5A_EVENT; at:int=-1;     // the refusing tag
  kind:int;
  d:N32Text;                          // a named site's identifier
  files:[N5A_MAX_PATHS]N5aFile; n:int;
  aggregate:[32]u8;                   // computed from the path tags
  has_x:bool; x:[32]u8;               // the aggregate x tag, when present
  parents:int; origins:int;           // a and A tags
}
N5aHit :: struct {error:int=N5A_NOT_FOUND; file:int=-1; fallback:bool;}
// What the writer takes.
N5aEntry :: struct {path:string; hash:[32]u8;}
```

The reader refuses a path that isn't absolute, doesn't end in a file name
with an extension, or has control bytes; a hash that isn't 64 lowercase hex
digits; and a path listed twice, since a host couldn't tell which file to
serve. It counts the `a` and `A` tags and requires both or neither on a
site; it doesn't follow them.

<!-- source: coax/lib/nostr/nip5a.coax rules -->
```jai
// A named site's d: 1 to 13 of a-z, 0-9 and -, not ending in -.
n5a_d_ok :: (d:string)->bool {
  if d.count<1 || d.count>13 || d[d.count-1]=='-' {return false;}
  for c:d {if !((c>='a' && c<='z') || (c>='0' && c<='9') || c=='-') {return false;}}
  return true;
}
// An absolute path ending in a file name with an extension: it starts
// with /, doesn't end in / or ., has no control bytes, and its last
// segment has a . in it (a dot file such as /.htaccess counts).
n5a_path_ok :: (p:string)->bool {
  if p.count==0 || p.count>NT_MAX_VALUE || p[0]!='/' || p[p.count-1]=='/' || p[p.count-1]=='.' {return false;}
  for c:p {if c<' ' || c==127 {return false;}}
  last:=p.count-1;
  while p[last-1]!='/' {last-=1;}
  for i:last..<p.count {if p[i]=='.' {return true;}}
  return false;
}
// Whether file a's line "<hash> <path>\n" sorts before b's. Lowercase hex
// sorts as the bytes it encodes, so the hashes compare as bytes.
n5a_before :: (a:N5aFile,b:N5aFile)->bool {
  for i:0..<32 {
    if a.hash[i]<b.hash[i] {return true;}
    if a.hash[i]>b.hash[i] {return false;}
  }
  // The paths, each followed by "\n" (byte 10): the first byte that differs.
  i:=0;
  while i<a.path.n && i<b.path.n && a.path.b[i]==b.path.b[i] {i+=1;}
  x:=ifx i<a.path.n then cast(int)a.path.b[i] else 10;
  y:=ifx i<b.path.n then cast(int)b.path.b[i] else 10;
  return x<y;
}
```

### The aggregate hash

The aggregate hash identifies one version of a site, whoever published
it. NIP-5A defines it as the SHA-256 of the lines `<hash> <path>\n` of
every `path` tag, sorted in ascending byte order and concatenated. A
manifest SHOULD carry it as `["x", <hex>, "aggregate"]`. When it does,
the reader requires that it matches.

Lowercase hex sorts in the same order as the bytes it encodes, so
`n5a_before` compares the raw hashes first and then the paths, each
followed by its `\n`.

<!-- source: coax/lib/nostr/nip5a.coax aggregate -->
```jai
// SHA-256 of the sorted lines "<hash> <path>\n" of files[0..<n].
n5a_aggregate :: (files:[]N5aFile,n:int)->[32]u8 {
  state:[SHA_STATE_BYTES]u8;ok:=sha256_init(state[..]);
  line:[64+1+NT_MAX_VALUE+1]u8;used:[N5A_MAX_PATHS]bool;
  for k:0..<n {
    // The k'th line in order: the smallest file not yet hashed.
    best:=-1;
    for i:0..<n {if !used[i] && (best<0 || n5a_before(files[i],files[best])) {best=i;}}
    used[best]=true;f:=files[best];
    h:=hex_encode_checked(cast(string)f.hash[..],line[..],false);
    line[64]=' ';for i:0..<f.path.n {line[65+i]=f.path.b[i];}line[65+f.path.n]=10;
    ok=sha256_update_checked(state[..],line[0..<66+f.path.n],false);
  }
  digest:[32]u8;ok=sha256_final_checked(state[..],digest[..],false);
  return digest;
}
```

<!-- source: coax/lib/nostr/nip5a.coax read -->
```jai
// A kind 15128, 35128 or 5128 manifest: its files and aggregate hash.
n5a_read :: (e:NostrEvent)->N5aSite {
  s:N5aSite;if e.error!=NE_OK {return s;}
  if e.kind!=N5A_ROOT && e.kind!=N5A_NAMED && e.kind!=N5A_SNAPSHOT {return n5a_fail(s,N5A_KIND,-1);}
  s.kind=e.kind;has_d:=false;i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n5a_fail(s,N5A_LIMIT,i);}
    if t.error!=NT_OK {return n5a_fail(s,N5A_EVENT,i);}
    if n32_is(t.v[0],"d") {
      if e.kind==N5A_ROOT {return n5a_fail(s,N5A_D,i);}
      if !has_d {has_d=true;s.d=t.v[1];}
    }
    if n32_is(t.v[0],"path") {
      f:N5aFile;f.path=t.v[1];
      if !n5a_path_ok(cast(string)f.path.b[0..<f.path.n]) || !ne_lower_hex(cast(string)t.v[2].b[0..<t.v[2].n],f.hash[..]) {return n5a_fail(s,N5A_PATH,i);}
      for k:0..<s.n {if n32_text_order(s.files[k].path,f.path)==0 {return n5a_fail(s,N5A_PATH,i);}}
      if s.n==N5A_MAX_PATHS {return n5a_fail(s,N5A_LIMIT,i);}
      s.files[s.n]=f;s.n+=1;
    }
    if n32_is(t.v[0],"x") && n32_is(t.v[2],"aggregate") {
      if s.has_x || !ne_lower_hex(cast(string)t.v[1].b[0..<t.v[1].n],s.x[..]) {return n5a_fail(s,N5A_X,i);}
      s.has_x=true;
    }
    if n32_is(t.v[0],"a") {s.parents+=1;}
    if n32_is(t.v[0],"A") {s.origins+=1;}
    i+=1;
  }
  if e.kind==N5A_NAMED && !n5a_d_ok(cast(string)s.d.b[0..<s.d.n]) {return n5a_fail(s,N5A_D,-1);}
  if s.n==0 {return n5a_fail(s,N5A_NO_PATH,-1);}
  if s.origins>1 {return n5a_fail(s,N5A_LINEAGE,-1);}
  if e.kind==N5A_SNAPSHOT {
    if s.parents!=1 {return n5a_fail(s,N5A_LINEAGE,-1);}
    if !s.has_x {return n5a_fail(s,N5A_X,-1);}
  } else if s.parents!=s.origins {return n5a_fail(s,N5A_LINEAGE,-1);}
  s.aggregate=n5a_aggregate(s.files[..],s.n);
  if s.has_x && !ns_equal32(s.x,s.aggregate) {return n5a_fail(s,N5A_AGGREGATE,-1);}
  return n5a_fail(s,N5A_OK,-1);
}
```

### Worked example: Dave's site

Dave's named site `reads` has two files:

| Path | Content | SHA-256 |
| --- | --- | --- |
| `/index.html` | `<h1>Reads</h1>\n` (15 bytes) | `7b5c4b28…98a8fe26` |
| `/reads.css` | `h1 { color: green; }\n` (21 bytes) | `f7d891fb…9c6603e0` |

The lines are `7b5c4b28… /index.html\n` and `f7d891fb… /reads.css\n`. They
start with `7` and `f`, and `7` (0x37) sorts before `f` (0x66), so
`/index.html`'s line comes first. The SHA-256 of the two 77- and 76-byte
lines, concatenated, is the aggregate
`bcceb55cfa4d5d933a33ee2411d212d90981186b288f292672279893c7334a4a`, which
`n5a_tags` writes into the `x` tag.

### Serving a file

A host finds the manifest, then the file for the request path:
`n5a_lookup` maps a path that ends in `/` to its `index.html` and falls
back to `/404.html`. It fetches the blob by its hash, and then checks
what it received: NIP-5A says the host SHOULD verify that the blob's
SHA-256 matches the `path` tag, and this host does. A Blossom server that
returns other bytes, by mistake or on purpose, gets `N5A_HASH`, and the
host serves nothing.

<!-- source: coax/lib/nostr/nip5a.coax serve -->
```jai
n5a_find :: (s:N5aSite,path:string)->int {
  for k:0..<s.n {
    p:=s.files[k].path;
    if cast(string)p.b[0..<p.n]==path {return k;}
  }
  return -1;
}
// The file a request path names: the path itself, or its index.html when
// it ends in /, else /404.html with fallback set.
n5a_lookup :: (s:N5aSite,request:string)->N5aHit {
  h:N5aHit;if s.error!=N5A_OK {h.error=N5A_EVENT;return h;}
  want:[NT_MAX_VALUE]u8;n:=json_raw(want[..],0,request);
  if request.count>0 && request[request.count-1]=='/' {n=json_raw(want[..],n,"index.html");}
  if n!= -1 {h.file=n5a_find(s,cast(string)want[0..<n]);}
  if h.file<0 {h.file=n5a_find(s,"/404.html");h.fallback=true;}
  if h.file<0 {h.fallback=false;return h;}
  h.error=N5A_OK;return h;
}
// Whether blob, fetched for file index, may be served: N5A_OK when its
// SHA-256 is the manifest's hash for that file, else N5A_HASH.
n5a_check :: (s:N5aSite,file:int,blob:[]u8)->int {
  if s.error!=N5A_OK {return N5A_EVENT;}
  if file<0 || file>=s.n {return N5A_NOT_FOUND;}
  digest:[32]u8;ok:=sha256_hash_checked(blob,digest[..],false);
  if ok!=SHA_OK || !ns_equal32(digest,s.files[file].hash) {return N5A_HASH;}
  return N5A_OK;
}
```

In the example, changing byte 4 of `/index.html` (`R` to `Z`) gives the
SHA-256 `39d2749e…94066fcd` instead of `7b5c4b28…98a8fe26`, and
`n5a_check` refuses it.

## NIP-CD: slash commands

NIP-CD lets a service publish a kind 31992 definition of a slash command:
`command`, `title`, `description`, ordered `arg` tags with a type
(`pubkey`, `event`, `address`, `relay`, `number`, `bool`, `enum`, `word`
or `text`) and `s` scopes that say which events can invoke it (`kind:`,
`relay:`, `author:`, `tag:`). A user invokes it with plain text at the
start of any event's content, such as `/ban@nostr:npub1… nostr:npub1…
spamming`, and keeps the commands they want in a kind 10992 list of `a`
tags. The executor reads the events in its scope and parses invocations.

This chapter doesn't build NIP-CD. Its parts are a definition reader, an
invocation parser for each argument type (most of which are NIP-19 forms
from chapter 16) and the scope filter, which combines like a NIP-01 filter
(chapter 23). They belong to an executor, which this book doesn't have yet.

## Not built

- **NIP-CD**, as described above.
- **Fetching.** Nothing here sends a REQ or fetches a blob. The caller
  fetches the 31989 and 31990 events, the manifest and each blob, and
  verifies the events' signatures.
- **Handler metadata.** A 31990's content (kind 0 style metadata), its
  `latest` and `next` site references, and a recommendation's relay hints
  are read or kept but not followed. Choosing among several platform tags
  beyond the entity type, for example by the user's device, is the
  caller's choice of `platform`.
- **nsite hosting.** The base36 host labels (`<pubkeyB36><dTag>`),
  Blossom `server` tags and BUD-03 server lists, the `Content-Type`
  rules, and the `title`, `description`, `source` and `app` tags are not
  built. The lineage of copied sites is counted, not followed.
- **App data encryption.** NIP-78 leaves the content to the app; this
  chapter doesn't encrypt it.

## The example

The example is
[nostr/examples/app-handlers.coax](../examples/app-handlers.coax). It
signs ten events with the secret keys 1 to 5 as Alice, Bob, Carol, Dave
and Eve: Bob's article, the three handlers and three recommendations of
the worked example, Alice's app settings, Dave's site manifest and an
Alice note with a `client` tag. Each NIP's writer writes its events'
tags. It reads every event back, resolves the handler for Bob's article,
fills the template, serves Dave's site and then shows each refusal on an
unsigned event, because no signer here would write one.

<!-- source: nostr/examples/app-handlers.coax example -->
```jai
#load "../../coax/lib/nostr/nip78.coax";
#load "../../coax/lib/nostr/nip89.coax";
#load "../../coax/lib/nostr/nip5a.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

E89 :: [15] string.["ok", "event", "kind", "no d", "bad k", "a names the wrong kind", "bad tag", "kind not handled",
  "no template", "no <bech32>", "unsafe scheme", "unsafe character", "bad entity", "limit", "output"];
E5A :: [13] string.["ok", "event", "kind", "bad d", "bad path", "no path", "bad x", "aggregate mismatch", "bad lineage",
  "limit", "not found", "hash mismatch", "output"];
E78 :: [5] string.["ok", "event", "kind", "no d", "limit"];
RANK :: [3] string.["recommended by follows", "a follow's app", "unvetted"];
// The public keys of the secret keys 1 to 5: published test values.
// Never sign with a real key in an example.
PEOPLE :: [5] string.["Alice", "Bob", "Carol", "Dave", "Eve"];
PUB :: [5] string.[
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13",
  "2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4"];
ALICE :: 0; BOB :: 1; CAROL :: 2; DAVE :: 3; EVE :: 4;
T0 :: 1762000000;
RELAY :: "wss://relay.example.com";
// The two files of Dave's site.
INDEX_HTML :: "<h1>Reads</h1>\n";
READS_CSS :: "h1 { color: green; }\n";

decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
hex32 :: (id: [32] u8, out: [] u8) -> string {
  own := id; r := hex_encode_checked(cast(string) own[..], out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<64];
}
key_of :: (who: int) -> [32] u8 { k: [32] u8; decode_hex(PUB[who], k[..]); return k; }
say :: (t: N32Text) { own := t; print("%", cast(string) own.b[0..<own.n]); }
person :: (key: [32] u8) -> string {
  for k: 0..<PEOPLE.count { if ns_equal32(key_of(k), key) { return PEOPLE[k]; } }
  return "someone else";
}
file_hash :: (text: string) -> [32] u8 {
  b: [64] u8; for i: 0..<text.count { b[i] = text[i]; }
  d: [32] u8; ok := sha256_hash_checked(b[0..<text.count], d[..], false);
  #assert ok == SHA_OK;
  return d;
}

// Sign an event with secret key who+1 and read it back with its ID and
// signature checked.
sign :: (signer: int, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  key: [32] u8; key[31] = cast(u8) (signer + 1);
  aux: [32] u8; work: [1] NostrSignWork; scratch: [8192] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [8192] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified && ns_equal32(e.public_key, key_of(signer));
  return e;
}
// An unsigned event by Eve, parsed but not verified. Used for the
// refusals, which no signer here would write.
unsigned :: (kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"");
  n = json_raw(out, n, PUB[EVE]); n = json_raw(out, n, "\",\"created_at\":1762000000,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":\"\",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

// The events: who signs, when (after T0), the kind and the content.
EVENTS :: 10;
NAME :: [EVENTS] string.["A", "H1", "H2", "H3", "R1", "R2", "R3", "S", "M", "C"];
WHO :: [EVENTS] int.[BOB, DAVE, EVE, CAROL, BOB, CAROL, EVE, ALICE, DAVE, ALICE];
AT :: [EVENTS] int.[0, 10, 20, 30, 40, 50, 60, 70, 80, 90];
KIND :: [EVENTS] int.[30023, 31990, 31990, 31990, 31989, 31989, 31989, 30078, 35128, 1];
CONTENT :: [EVENTS] string.["Coax is a language for writing Nostr from scratch.", "", "", "", "", "", "", "{\"theme\":\"dark\"}", "",
  "Reading Bob's article."];

// The tags of event k, each written by its NIP's writer.
tags_of :: (k: int, out: [] u8) -> string {
  n := 0;
  if k == 0 { m: N23Meta; m.d = "coax-intro"; m.title = "Coax, an introduction"; t := nip23_tags(m, out); #assert t.error == N23_OK; n = t.n; }
  if k == 1 {
    kinds := [2] int.[30023, 30024];
    t := [2] N89Template.[N89Template.{platform = "web", url = "https://reads.example.com/a/<bech32>", entity = "naddr"},
                          N89Template.{platform = "web", url = "https://reads.example.com/e/<bech32>"}];
    n = n89_handler_tags("reads", kinds[..], t[..], out);
  }
  if k == 2 {
    kinds := [1] int.[30023]; t := [1] N89Template.[N89Template.{platform = "web", url = "https://spam.example.com/<bech32>"}];
    n = n89_handler_tags("spam", kinds[..], t[..], out);
  }
  if k == 3 {
    kinds := [1] int.[1]; t := [1] N89Template.[N89Template.{platform = "web", url = "https://notes.example.com/<bech32>"}];
    n = n89_handler_tags("notes", kinds[..], t[..], out);
  }
  if k == 4 { p := [1] N89Pick.[N89Pick.{author = key_of(DAVE), d = "reads", relay = RELAY, platform = "web"}]; n = n89_recommend_tags(30023, p[..], out); }
  if k == 5 {
    p := [2] N89Pick.[N89Pick.{author = key_of(DAVE), d = "reads", relay = RELAY, platform = "web"}, N89Pick.{author = key_of(CAROL), d = "notes"}];
    n = n89_recommend_tags(30023, p[..], out);
  }
  if k == 6 { p := [1] N89Pick.[N89Pick.{author = key_of(EVE), d = "spam"}]; n = n89_recommend_tags(30023, p[..], out); }
  if k == 7 { n = n78_tags("coax-reader/settings", out); }
  if k == 8 {
    f := [2] N5aEntry.[N5aEntry.{path = "/index.html", hash = file_hash(INDEX_HTML)}, N5aEntry.{path = "/reads.css", hash = file_hash(READS_CSS)}];
    n = n5a_tags(N5A_NAMED, "reads", f[..], out);
  }
  if k == 9 {
    c: [512] u8; m := n89_client_tag("Reads", key_of(DAVE), "reads", RELAY, c[..]);
    n = json_raw(out, 0, "["); n = json_raw(out, n, cast(string) c[0..<m]); n = json_raw(out, n, "]");
  }
  #assert n > 0;
  return cast(string) out[0..<n];
}

show_handler :: (name: string, e: NostrEvent) {
  h := n89_handler(e);
  print("%: %, by %, d ", name, E89[h.error], person(e.public_key)); say(h.d); print(", kinds");
  for i: 0..<h.nk { print(" %", h.k[i]); }
  print("\n");
}
show_recommend :: (name: string, e: NostrEvent) {
  r := n89_recommend(e);
  print("%: %, by %, for kind %:", name, E89[r.error], person(e.public_key), r.kind);
  for i: 0..<r.n { print(" 31990:%:", person(r.a[i].author)); say(r.a[i].d); if r.a[i].platform.n > 0 { print(" ("); say(r.a[i].platform); print(")"); } }
  print("\n");
}
index_of :: (name: string) -> int { for k: 0..<EVENTS { if NAME[k] == name { return k; } } return -1; }
show_url :: (what: string, h: NostrEvent, platform: string, entity: string) {
  out: [1024] u8; u := n89_url(h, platform, entity, out[..]);
  print("%: %", what, E89[u.error]);
  if u.at >= 0 { print(" at tag %", u.at); }
  if u.error == N89_OK { print("\n  %", cast(string) out[0..<u.n]); }
  print("\n");
}
show_site :: (name: string, e: NostrEvent) {
  s := n5a_read(e);
  print("%: %", name, E5A[s.error]);
  if s.at >= 0 { print(" at tag %", s.at); }
  print("\n");
}

main :: () {
  all: [EVENTS] NostrEvent; buf: [EVENTS][4096] u8; tags: [2048] u8;
  for k: 0..<EVENTS { all[k] = sign(WHO[k], T0 + AT[k], KIND[k], tags_of(k, tags[..]), CONTENT[k], buf[k][..]); }
  print("== 1. Each event's id (T0 = %)\n", T0);
  for k: 0..<EVENTS { hex: [64] u8; print("%: kind %, by %, at T0+%, id %\n", NAME[k], KIND[k], PEOPLE[WHO[k]], AT[k], hex32(all[k].id, hex[..])); }
  print("H1's tags: %\n", tags_of(1, tags[..]));
  print("R2's tags: %\n", tags_of(5, tags[..]));

  print("\n== 2. App data\n");
  s := n78_read(all[7]);
  print("S: kind %, d ", s.kind); say(s.d); print(", content % (the app's own format)\n", CONTENT[7]);
  print("served to Alice after AUTH: %; to Bob after AUTH: %; to Alice without AUTH: %\n",
        n78_serve(all[7], true, key_of(ALICE)), n78_serve(all[7], true, key_of(BOB)), n78_serve(all[7], false, key_of(ALICE)));

  print("\n== 3. Handlers and recommendations\n");
  show_handler("H1", all[1]); show_handler("H2", all[2]); show_handler("H3", all[3]);
  show_recommend("R1", all[4]); show_recommend("R2", all[5]); show_recommend("R3", all[6]);

  print("\n== 4. Which app opens Bob's article? Alice follows Bob and Carol\n");
  follows: [2][32] u8; follows[0] = key_of(BOB); follows[1] = key_of(CAROL);
  recs := all[4..<7]; handlers := all[1..<4]; choices: [8] N89Choice;
  f := n89_resolve(30023, follows[..], recs, handlers, choices[..]);
  print("kind 30023: %, % handlers, % recommended but not handling the kind, % missing\n", E89[f.error], f.n, f.not_handled, f.missing);
  for i: 0..<f.n {
    c := choices[i]; h := n89_handler(handlers[c.handler]);
    print("  %. H% by %, d ", i + 1, c.handler + 1, person(handlers[c.handler].public_key)); say(h.d); print(": %, % votes\n", RANK[c.rank], c.votes);
  }

  print("\n== 5. The entity and the URL\n");
  entity: [512] u8; en := n89_entity(all[0], RELAY, entity[..]);
  dec: [512] u8; r := nip19_decode_checked(cast(string) entity[0..<en], dec[..], false);
  hex: [1024] u8; hx := hex_encode_checked(cast(string) dec[0..<r.written], hex[..], false);
  print("A as naddr, % TLV bytes: %\n", r.written, cast(string) hex[0..<hx.written]);
  print("  %\n", cast(string) entity[0..<en]);
  best := handlers[choices[0].handler];
  show_url("H1 web for naddr", best, "web", cast(string) entity[0..<en]);
  note: [512] u8; nn := n89_entity(all[9], "", note[..]);
  show_url("H1 web for C's nevent", best, "web", cast(string) note[0..<nn]);
  show_url("H1 ios", best, "ios", cast(string) entity[0..<en]);
  c := n89_client(all[9]);
  print("C's client tag: %, \"", E89[c.error]); say(c.name); print("\", handler 31990:%:", person(c.handler.author)); say(c.handler.d); print(", relay "); say(c.handler.relay); print("\n");

  print("\n== 6. Dave's site\n");
  site := n5a_read(all[8]);
  agg: [64] u8;
  print("M: %, named site ", E5A[site.error]); say(site.d); print(", % files, aggregate %\n", site.n, hex32(site.aggregate, agg[..]));
  for i: 0..<site.n { h: [64] u8; print("  "); say(site.files[i].path); print(" %\n", hex32(site.files[i].hash, h[..])); }
  requests := [3] string.["/", "/reads.css", "/missing.html"];
  for q: requests {
    hit := n5a_lookup(site, q);
    print("GET %: %", q, E5A[hit.error]);
    if hit.error == N5A_OK { print(", file "); say(site.files[hit.file].path); if hit.fallback { print(" (fallback)"); } }
    print("\n");
  }
  blob: [64] u8; for i: 0..<INDEX_HTML.count { blob[i] = INDEX_HTML[i]; }
  hit := n5a_lookup(site, "/");
  print("serve the fetched /index.html: %\n", E5A[n5a_check(site, hit.file, blob[0..<INDEX_HTML.count])]);
  blob[4] = 'Z';
  print("serve a blob with one byte changed: %\n", E5A[n5a_check(site, hit.file, blob[0..<INDEX_HTML.count])]);

  print("\n== 7. Refusals\n");
  bad: [2048] u8; out: [512] u8;
  show_url("javascript: template", unsigned(31990, "[[\"d\",\"x\"],[\"k\",\"30023\"],[\"web\",\"javascript:alert(document.cookie)//<bech32>\"]]", bad[..]), "web", cast(string) entity[0..<en]);
  show_url("template without <bech32>", unsigned(31990, "[[\"d\",\"x\"],[\"k\",\"30023\"],[\"web\",\"https://x.example.com/open\"]]", bad[..]), "web", cast(string) entity[0..<en]);
  show_url("web template over http", unsigned(31990, "[[\"d\",\"x\"],[\"k\",\"30023\"],[\"web\",\"http://x.example.com/<bech32>\"]]", bad[..]), "web", cast(string) entity[0..<en]);
  show_url("template with a quote", unsigned(31990, "[[\"d\",\"x\"],[\"k\",\"30023\"],[\"web\",\"https://x.example.com/\\\"<bech32>\"]]", bad[..]), "web", cast(string) entity[0..<en]);
  show_url("an nsec as the entity", best, "web", "nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5");
  k1 := [1] int.[30023]; t1 := [1] N89Template.[N89Template.{platform = "web", url = "javascript:alert(1)//<bech32>"}];
  print("writing a handler with a javascript: template: %\n", E89[-n89_handler_tags("x", k1[..], t1[..], out[..])]);
  h := n89_handler(unsigned(31990, "[[\"d\",\"x\"],[\"k\",\"long-form\"]]", bad[..]));
  print("handler with k long-form: % at tag %\n", E89[h.error], h.at);
  rr := n89_recommend(unsigned(31989, "[[\"d\",\"30023\"],[\"a\",\"30023:c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5:coax-intro\"]]", bad[..]));
  print("recommendation naming an article: % at tag %\n", E89[rr.error], rr.at);
  d := n78_read(unsigned(30078, "[]", bad[..]));
  print("app data without d: %\n", E78[d.error]);
  show_site("named site with d Reads", unsigned(35128, "[[\"d\",\"Reads\"],[\"path\",\"/index.html\",\"186ea5fd14e88fd1ac49351759e7ab906fa94892002b60bf7f5a428f28ca1c99\"]]", bad[..]));
  show_site("root site with a d", unsigned(15128, "[[\"d\",\"reads\"],[\"path\",\"/index.html\",\"186ea5fd14e88fd1ac49351759e7ab906fa94892002b60bf7f5a428f28ca1c99\"]]", bad[..]));
  show_site("path without a file name", unsigned(15128, "[[\"path\",\"/blog/\",\"186ea5fd14e88fd1ac49351759e7ab906fa94892002b60bf7f5a428f28ca1c99\"]]", bad[..]));
  show_site("x that isn't the aggregate", unsigned(15128, "[[\"path\",\"/index.html\",\"186ea5fd14e88fd1ac49351759e7ab906fa94892002b60bf7f5a428f28ca1c99\"],[\"x\",\"0000000000000000000000000000000000000000000000000000000000000000\",\"aggregate\"]]", bad[..]));
}
```

Run it with:

```shell
coax/bin/coax run nostr/examples/app-handlers.coax --run-steps 250000000
```

It prints this transcript, which `nostr/fixtures/app-handlers.out` holds
and the book's checks compare in debug and release. It exits with status
0.

```text
== 1. Each event's id (T0 = 1762000000)
A: kind 30023, by Bob, at T0+0, id f8d88455f78c403c3a9814aeb54d4225d29b39e9b66689120cb193376ed76564
H1: kind 31990, by Dave, at T0+10, id 7c0d38b09867c43d31957108beb03fefdc69073c74f567ae8283452ca1ceef57
H2: kind 31990, by Eve, at T0+20, id d24bb6ee181ea301d72e2b281a8d0aa1724534b44101db0dfe66106117dd1fdc
H3: kind 31990, by Carol, at T0+30, id 51e78e445cd00d3637f097eb430adfd443f47ebee09a8c4f67071cff33da9f60
R1: kind 31989, by Bob, at T0+40, id a4d3c5f394d40b010e8310f5b2bb8add585a49667a68aeffd63f3bf1f3a3f743
R2: kind 31989, by Carol, at T0+50, id ac2549b1e7c5fb6aa62b22b24b9a7a8b6909c68eb4496c3ce89190b325cbc7b4
R3: kind 31989, by Eve, at T0+60, id 3e9e66d59712466cf07255ff368b7e37362454c60104ef91ebe47b4a4e7d6d0d
S: kind 30078, by Alice, at T0+70, id 9c228a0b110efbed49507e112325cee38f84c652cadbb61cd1dde4af10fa39de
M: kind 35128, by Dave, at T0+80, id 91f12a59d81279c6d1f4fb44f58c4c0b8088dd8fe8364c36e3d2d0043718e5ca
C: kind 1, by Alice, at T0+90, id c5f0df3482bdb7d1152059999e6008d5076ffc170652b32f51d56f07efd4a4da
H1's tags: [["d","reads"],["k","30023"],["k","30024"],["web","https://reads.example.com/a/<bech32>","naddr"],["web","https://reads.example.com/e/<bech32>"]]
R2's tags: [["d","30023"],["a","31990:e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13:reads","wss://relay.example.com","web"],["a","31990:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:notes","",""]]

== 2. App data
S: kind 30078, d coax-reader/settings, content {"theme":"dark"} (the app's own format)
served to Alice after AUTH: true; to Bob after AUTH: false; to Alice without AUTH: false

== 3. Handlers and recommendations
H1: ok, by Dave, d reads, kinds 30023 30024
H2: ok, by Eve, d spam, kinds 30023
H3: ok, by Carol, d notes, kinds 1
R1: ok, by Bob, for kind 30023: 31990:Dave:reads (web)
R2: ok, by Carol, for kind 30023: 31990:Dave:reads (web) 31990:Carol:notes
R3: ok, by Eve, for kind 30023: 31990:Eve:spam

== 4. Which app opens Bob's article? Alice follows Bob and Carol
kind 30023: ok, 2 handlers, 1 recommended but not handling the kind, 0 missing
  1. H1 by Dave, d reads: recommended by follows, 2 votes
  2. H2 by Eve, d spam: unvetted, 0 votes

== 5. The entity and the URL
A as naddr, 77 TLV bytes: 000a636f61782d696e74726f01177773733a2f2f72656c61792e6578616d706c652e636f6d0220c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5030400007547
  naddr1qq9xxmmp0qkkjmn5wfhsz9mhwden5te0wfjkccte9ejhsctdwpkx2tnrdaksygxxq3legs0d04knq32qd62uqlxct3mcujuvau7202avpxu4cuy7u5psgqqqw4rss6c9tf
H1 web for naddr: ok at tag 3
  https://reads.example.com/a/naddr1qq9xxmmp0qkkjmn5wfhsz9mhwden5te0wfjkccte9ejhsctdwpkx2tnrdaksygxxq3legs0d04knq32qd62uqlxct3mcujuvau7202avpxu4cuy7u5psgqqqw4rss6c9tf
H1 web for C's nevent: ok at tag 4
  https://reads.example.com/e/nevent1qqsvtuxlxjptmd73z5s9nxv7vqyd2pm0lstsv54n9aga2mc8al22fkszypumuen7l8wthtz45p3ftn58pvrs9xlumvkuu2xet8egzkcklqtesqcyqqqqqqgjr4s5l
H1 ios: no template
C's client tag: ok, "Reads", handler 31990:Dave:reads, relay wss://relay.example.com

== 6. Dave's site
M: ok, named site reads, 2 files, aggregate bcceb55cfa4d5d933a33ee2411d212d90981186b288f292672279893c7334a4a
  /index.html 7b5c4b28567b11747205eedb2fe28364e863eaacebf52d4bd868540e98a8fe26
  /reads.css f7d891fb00e73ad2d6ccc238a771f4c0e69a701dad896fa5a569d33f9c6603e0
GET /: ok, file /index.html
GET /reads.css: ok, file /reads.css
GET /missing.html: not found
serve the fetched /index.html: ok
serve a blob with one byte changed: hash mismatch

== 7. Refusals
javascript: template: unsafe scheme at tag 2
template without <bech32>: no <bech32> at tag 2
web template over http: unsafe scheme at tag 2
template with a quote: unsafe character at tag 2
an nsec as the entity: bad entity
writing a handler with a javascript: template: unsafe scheme
handler with k long-form: bad k at tag 1
recommendation naming an article: a names the wrong kind at tag 1
app data without d: no d
named site with d Reads: bad d
root site with a d: bad d at tag 0
path without a file name: bad path at tag 0
x that isn't the aggregate: aggregate mismatch
```

Sections 4, 5 and 6 match the worked examples. The refusals show each
broken input and its exact diagnostic:

- `javascript: template`: the only `web` tag is a `javascript:` URL, at
  tag 2.
- `template without <bech32>`: every event would open the same page.
- `web template over http`: a web template must be `https`.
- `template with a quote`: a `"` after the host, at tag 2.
- `an nsec as the entity`: a secret key is never put in a URL.
- `writing a handler with a javascript: template`: the writer refuses it
  too.
- `handler with k long-form`: a `k` value must be a decimal kind, at tag
  1.
- `recommendation naming an article`: an `a` tag that names a 30023, not
  a 31990.
- `app data without d`: a kind 30078 needs a `d`.
- `named site with d Reads`: uppercase isn't allowed in a site's `d`.
- `root site with a d`: a kind 15128 must not have one, at tag 0.
- `path without a file name`: `/blog/` ends in `/`.
- `x that isn't the aggregate`: the `x` tag doesn't match the paths.

Section 6 also shows the deliberately broken file: `/index.html` with one
byte changed is refused with `hash mismatch`.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/app-handlers.coax](../examples/checks/app-handlers.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it, from a Python
script written from NIP-01, NIP-19, NIP-89, NIP-5A and BIP-173:

- The public keys of the secret keys 1 to 5 (integer secp256k1 in
  Python), and the ids of all ten events (`hashlib.sha256` over NIP-01's
  serialization). The checks read each event with Python's id and require
  that the recomputed id matches.
- The `naddr` of Bob's article and the `nevent` of Alice's note, from the
  script's own Bech32 encoder, and the 77 TLV bytes of the worked example,
  written out field by field.
- The resolution of the worked example, and the URL the client opens.
- NIP-89's recommendation example (its `<relay-url>` placeholder filled).
- The SHA-256 of Dave's two files and the site's aggregate (`hashlib`),
  the refused changed byte, and the aggregate of NIP-5A's own two-line
  example input. NIP-5A's examples print `<site-aggregate-sha256>` as a
  placeholder, so there is no published aggregate to compare with.

<!-- source: nostr/examples/checks/app-handlers.coax check -->
```jai
// From a Python script written from NIP-01, NIP-19, NIP-89, NIP-5A and
// BIP-173 (it shares no code with Coax): the public keys of secret keys 1
// to 5 by integer secp256k1, each event's id as hashlib.sha256 of
// json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False), the bech32 strings from its
// own BIP-173 encoder, and the file and aggregate hashes from hashlib.
PY_ID :: [EVENTS] string.[
  "f8d88455f78c403c3a9814aeb54d4225d29b39e9b66689120cb193376ed76564",
  "7c0d38b09867c43d31957108beb03fefdc69073c74f567ae8283452ca1ceef57",
  "d24bb6ee181ea301d72e2b281a8d0aa1724534b44101db0dfe66106117dd1fdc",
  "51e78e445cd00d3637f097eb430adfd443f47ebee09a8c4f67071cff33da9f60",
  "a4d3c5f394d40b010e8310f5b2bb8add585a49667a68aeffd63f3bf1f3a3f743",
  "ac2549b1e7c5fb6aa62b22b24b9a7a8b6909c68eb4496c3ce89190b325cbc7b4",
  "3e9e66d59712466cf07255ff368b7e37362454c60104ef91ebe47b4a4e7d6d0d",
  "9c228a0b110efbed49507e112325cee38f84c652cadbb61cd1dde4af10fa39de",
  "91f12a59d81279c6d1f4fb44f58c4c0b8088dd8fe8364c36e3d2d0043718e5ca",
  "c5f0df3482bdb7d1152059999e6008d5076ffc170652b32f51d56f07efd4a4da"];
PY_NADDR :: "naddr1qq9xxmmp0qkkjmn5wfhsz9mhwden5te0wfjkccte9ejhsctdwpkx2tnrdaksygxxq3legs0d04knq32qd62uqlxct3mcujuvau7202avpxu4cuy7u5psgqqqw4rss6c9tf";
PY_NEVENT :: "nevent1qqsvtuxlxjptmd73z5s9nxv7vqyd2pm0lstsv54n9aga2mc8al22fkszypumuen7l8wthtz45p3ftn58pvrs9xlumvkuu2xet8egzkcklqtesqcyqqqqqqgjr4s5l";
PY_INDEX :: "7b5c4b28567b11747205eedb2fe28364e863eaacebf52d4bd868540e98a8fe26";
PY_CSS :: "f7d891fb00e73ad2d6ccc238a771f4c0e69a701dad896fa5a569d33f9c6603e0";
PY_AGGREGATE :: "bcceb55cfa4d5d933a33ee2411d212d90981186b288f292672279893c7334a4a";
SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

same_id :: (id: [32] u8, text: string) -> bool { want: [32] u8; decode_hex(text, want[..]); return ns_equal32(id, want); }

// Event k with Python's id and a zero signature, read with its id
// recomputed; the signature is not checked.
py :: (k: int, out: [] u8, scratch: [] u8) -> NostrEvent {
  tags: [2048] u8;
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, PY_ID[k]);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB[WHO[k]]);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, T0 + AT[k]);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, KIND[k]);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags_of(k, tags[..]));
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, CONTENT[k]);
  n = json_raw(out, n, ",\"sig\":\""); n = json_raw(out, n, SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}
read_all :: (ev: [] NostrEvent, buf: [][4096] u8, scratch: [][4096] u8) {
  for k: 0..<EVENTS { ev[k] = py(k, buf[k][..], scratch[k][..]); }
}

#test "the ids are Python's" {
  buf: [EVENTS][4096] u8; s: [EVENTS][4096] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..], s[..]);
  for k: 0..<EVENTS { #assert ev[k].error == NE_OK && ns_equal32(ev[k].computed_id, ev[k].id); }
}
// By hand, from the chapter: the naddr's TLV bytes for Bob's article with
// the relay hint, field by field.
#test "the naddr's TLV bytes match the hand values, its string Python's" {
  buf: [EVENTS][4096] u8; s: [EVENTS][4096] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..], s[..]);
  hand: [77] u8; n := 0;
  hand[0] = 0; hand[1] = 10; n = json_raw(hand[..], 2, "coax-intro");                 // d
  hand[n] = 1; hand[n + 1] = 23; n = json_raw(hand[..], n + 2, RELAY);                 // relay
  hand[n] = 2; hand[n + 1] = 32; bob := key_of(BOB); for i: 0..<32 { hand[n + 2 + i] = bob[i]; } n += 34;   // author
  hand[n] = 3; hand[n + 1] = 4; hand[n + 2] = 0; hand[n + 3] = 0; hand[n + 4] = 0x75; hand[n + 5] = 0x47; n += 6;   // 30023
  #assert n == 77;
  out: [512] u8; en := n89_entity(ev[0], RELAY, out[..]);
  #assert cast(string) out[0..<en] == PY_NADDR;
  dec: [512] u8; r := nip19_decode_checked(cast(string) out[0..<en], dec[..], false);
  #assert r.error == NC_OK && r.written == 77;
  for i: 0..<77 { #assert dec[i] == hand[i]; }
  nn := n89_entity(ev[9], "", out[..]);
  #assert cast(string) out[0..<nn] == PY_NEVENT;
}
#test "resolution: Dave's app first with two votes, Eve's unvetted, Carol's notes refused" {
  buf: [EVENTS][4096] u8; s: [EVENTS][4096] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..], s[..]);
  follows: [2][32] u8; follows[0] = key_of(BOB); follows[1] = key_of(CAROL);
  out: [8] N89Choice; f := n89_resolve(30023, follows[..], ev[4..<7], ev[1..<4], out[..]);
  #assert f.error == N89_OK && f.n == 2 && f.not_handled == 1 && f.missing == 0 && f.broken == 0;
  #assert out[0].handler == 0 && out[0].votes == 2 && out[0].rank == N89_BY_FOLLOWS;
  #assert out[1].handler == 1 && out[1].votes == 0 && out[1].rank == N89_UNVETTED;
  // Alice follows nobody: both apps are unvetted, in the order found.
  f = n89_resolve(30023, follows[0..<0], ev[4..<7], ev[1..<4], out[..]);
  #assert f.n == 2 && out[0].rank == N89_UNVETTED && out[1].rank == N89_UNVETTED;
  // The URL the client opens, written out from Python's naddr.
  u: [1024] u8; e: [512] u8; en := n89_entity(ev[0], RELAY, e[..]);
  url := n89_url(ev[1], "web", cast(string) e[0..<en], u[..]);
  want: [512] u8; wn := json_raw(want[..], 0, "https://reads.example.com/a/"); wn = json_raw(want[..], wn, PY_NADDR);
  #assert url.error == N89_OK && url.at == 3 && cast(string) u[0..<url.n] == cast(string) want[0..<wn];
}
// NIP-89's recommendation example, its <relay-url> placeholder filled.
#test "NIP-89's recommendation example reads" {
  b: [512] u8;
  e := unsigned(31989, "[[\"d\",\"31337\"],[\"a\",\"31990:1743058db7078661b94aaf4286429d97ee5257d14a86d6bfa54cb0482b876fb0:abcd\",\"wss://relay.example.com\",\"web\"]]", b[..]);
  r := n89_recommend(e);
  #assert r.error == N89_OK && r.kind == 31337 && r.n == 1 && n32_is(r.a[0].d, "abcd") && n32_is(r.a[0].platform, "web");
  #assert r.a[0].author[0] == 0x17 && r.a[0].author[31] == 0xb0;
}
#test "the site's hashes are Python's; a changed byte is refused" {
  buf: [EVENTS][4096] u8; s: [EVENTS][4096] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..], s[..]);
  site := n5a_read(ev[8]);
  #assert site.error == N5A_OK && site.has_x && same_id(site.aggregate, PY_AGGREGATE) && same_id(site.x, PY_AGGREGATE);
  #assert same_id(site.files[0].hash, PY_INDEX) && same_id(site.files[1].hash, PY_CSS);
  b: [64] u8; for i: 0..<INDEX_HTML.count { b[i] = INDEX_HTML[i]; }
  #assert n5a_check(site, 0, b[0..<INDEX_HTML.count]) == N5A_OK && n5a_check(site, 1, b[0..<INDEX_HTML.count]) == N5A_HASH;
  b[4] = 'Z'; #assert n5a_check(site, 0, b[0..<INDEX_HTML.count]) == N5A_HASH;
  // NIP-5A's two-line example input, hashed by Python.
  e := unsigned(15128, "[[\"path\",\"/index.html\",\"186ea5fd14e88fd1ac49351759e7ab906fa94892002b60bf7f5a428f28ca1c99\"],[\"path\",\"/favicon.ico\",\"fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321\"]]", buf[0][..]);
  #assert same_id(n5a_read(e).aggregate, "c2ff582b672a4c689c5e1753528f03dd31b95ec1fdcc3d82d25e7d91e8769638");
}
```

```shell
coax/bin/coax test nostr/examples/checks/app-handlers.coax --run-steps 250000000
```

`coax test` runs 427 checks, including the loaded files' own checks; all
pass.

### The library's own checks

Each new file has its own `#test` blocks. `nip78.coax`: both kinds, the
first `d`, the owner-only rule and the writer. `nip89.coax`: NIP-89's
recommendation example, every refusal and the tag it reports, the kind
number edges, the client tag, every template rule at its edge (scheme
characters, the denied schemes, `https` and its host, each forbidden byte),
the choice between exact and generic templates, both entity forms decoded
again, the 255-byte TLV bound, and a test world of six recommendations and
five handlers for the ranking, replaced versions, broken records and the
output bound. `nip5a.coax`: NIP-5A's root example and its aggregate from
Python, sorting by hash and then path, named-site and snapshot rules,
`x` tags and lineage, path refusals, lookup with `index.html` and
`/404.html`, the hash check and the writer.

```shell
coax/bin/coax test coax/lib/nostr/nip78.coax
coax/bin/coax test coax/lib/nostr/nip89.coax
coax/bin/coax test coax/lib/nostr/nip5a.coax
coax/bin/coax mutate coax/lib/nostr/nip89.coax --root-only
```

On 2026-11-01, `coax test` passed all checks of the three files (243, 262
and 248, with the loaded files'). `coax mutate --root-only` killed all 46
mutants of `nip78.coax`, all 720 of `nip89.coax` and all 367 of
`nip5a.coax`, with none surviving and none marked equivalent. The full
mutation run (`COAX_MUTATION_FULL=1`) and a mutation run of the example
were not run.

## Summary

- `n78_read` reads app data by its `d`, and `n78_serve` gives it only to
  its authenticated author.
- `n89_resolve` ranks handlers for a kind: recommendations from follows by
  votes, then a follow's own app, then the rest, and drops a recommended
  handler that doesn't list the kind.
- `n89_entity` builds the `naddr` or `nevent` byte by byte, and `n89_url`
  fills the template only when its scheme, host, characters and
  placeholder are safe.
- `n5a_read` checks a manifest's paths and aggregate hash, and `n5a_check`
  refuses any blob whose SHA-256 differs from its `path` tag.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Sources

- [NIP-78](../nips/official/78.md)
- [NIP-89](../nips/official/89.md)
- [NIP-5A](../nips/official/5A.md)
- [NIP-CD](../nips/official/CD.md)
- [NIP-19](../nips/official/19.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
