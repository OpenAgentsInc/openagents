# 52. Reposts, reactions and comments

People respond to events in three ways: they pass an event on to their
followers (a repost), they react to it with a like, a dislike or an emoji (a
reaction), or they write about it (a comment). This chapter reads and writes
each one, and counts the reactions to an event with one per person.

Part X, Every NIP. Result: Kind 6 and 16 reposts, kind 7 reactions (`+`, `-`,
emoji) and kind 17 website reactions, and NIP-22 kind 1111 comments with
uppercase root and lowercase parent scope tags.

## What this chapter teaches

- NIP-18: kind 6 reposts a kind 1 note, kind 16 any other kind with a `k`
  tag, and the content holds the reposted event's JSON.
- NIP-25: kind 7 content `+`, `-` or an emoji; `e`, `p` and `k` tags; kind 17
  for external content such as websites; NIP-30 custom emoji.
- Counting reactions: one per author and target, the latest one.
- NIP-22: `E`/`A`/`I`, `K` and `P` for the root scope, `e`/`a`/`i`, `k` and
  `p` for the parent, and why a parent's kind must be the root's or 1111.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag(e, i)` | Reads tag `i`: an `e`, `p` or `a` tag's values (chapter 22) |
| [nip18.coax](../../coax/lib/nostr/nip18.coax) | `n18_repost(e, inner, scratch)` | A repost's target, author, kind and address, and the embedded event checked against them |
| [nip18.coax](../../coax/lib/nostr/nip18.coax) | `n18_repost_tags(target, relay, out)` | The tags of a repost of `target`, and its kind, 6 or 16 |
| [nip25.coax](../../coax/lib/nostr/nip25.coax) | `n25_reaction(e)` | A reaction's value, target or external content, author and kind |
| [nip25.coax](../../coax/lib/nostr/nip25.coax) | `n25_tally(set, target)` | Likes, dislikes and other reactions to one event, one per author |
| [nip22.coax](../../coax/lib/nostr/nip22.coax) | `n22_comment(e)` | A comment's root and parent scopes, their kinds and authors |
| [nip34-event.coax](../../coax/lib/nostr/nip34-event.coax) | `nt_event_of(e)` | A `NostrEvent` as the `NtEvent` that `nip22.coax` reads |

`nip18.coax` and `nip25.coax` are new with this chapter. Like chapter 50's
`nip10.coax`, they read a parsed `NostrEvent` through `nostr_tag` and verify
no signature. `nip22.coax` existed: OpenAgents' project pages use it to read
issue comments. This chapter adds one refusal to it, `N22_KINDS`, described
below. `nip22.coax` is an `#authority` file, so the change came with its
checks first and a mutation run.

## Reposts

A repost tells your followers that an event is worth reading. NIP-18 uses
two kinds:

- Kind 6 reposts a kind 1 note.
- Kind 16, a generic repost, reposts any other kind. It should carry a `k`
  tag with the reposted event's kind. When the reposted event is replaceable
  or addressable, it should also carry an `a` tag with the event's address,
  `kind:pubkey:d`.

Both must have an `e` tag with the reposted event's id and a relay URL where
it can be fetched, and should have a `p` tag with its author. The content is
the reposted event's JSON, as a string. It may be empty, and NIP-18 says a
repost of a NIP-70 protected event should be empty (chapter 42).

### Worked example: Bob reposts Alice's note

Alice's note N has id `6b3c564c...0c764321`. Her pubkey is
`f9308a01...bce036f9`. Bob saw N on `wss://relay.example.com`.

1. N is kind 1, so the repost is kind 6 and gets no `k` tag.
2. The `e` tag: `["e", "6b3c564c...0c764321", "wss://relay.example.com"]`.
3. The `p` tag: `["p", "f9308a01...bce036f9"]`.
4. Kind 1 is neither replaceable nor addressable, so there is no `a` tag.
5. The content is N's JSON text, exactly as Alice signed it.

Alice's article R is kind 30023 with `d` tag `tea-storage`. Kind 30023 is
not 1, so Carol's repost of R is kind 16 with `["k", "30023"]`. Kind 30023 is
addressable (30000 to 39999), so it also gets
`["a", "30023:f9308a01...bce036f9:tea-storage"]`.

### Reading a repost

A reader can't take the content on trust: anyone can write any JSON into a
repost. `n18_repost` checks that the repost agrees with itself:

1. The kind is 6 or 16, or it refuses with `N18_KIND`.
2. Every tag reads through `nostr_tag`, or it refuses with `N18_TAG` and the
   tag's index.
3. The first `e` tag names the target. Without one it refuses with
   `N18_MISSING`; without a relay URL, with `N18_RELAY`.
4. Empty content: the repost names its target and embeds nothing.
5. Otherwise the content is read as an event with `nostr_event_read`, which
   recomputes its id (chapter 21). Content that is no event, or whose id
   doesn't match its text, refuses with `N18_INNER`.
6. Kind 6 must hold kind 1 and kind 16 any other kind, or it refuses with
   `N18_INNER_KIND`.
7. The embedded event's id must be the `e` tag's id, its author the first
   `p` tag's, its kind the `k` tag's, and its address the `a` tag's, when
   those tags are present. Any difference refuses with `N18_MISMATCH`.

It does not verify the embedded event's signature. The id check proves the
content is the event the `e` tag names; whether Alice signed it is a
separate question, which the caller answers with
`nostr_event_verify_vartime` on `inner.original`. The example does that.

<!-- source: coax/lib/nostr/nip18.coax record -->
```jai
N18Repost :: struct {
  status:int=N18_EVENT; tag_error:int; at:int=-1;   // the refusing tag
  target:[32]u8;                                    // the e tag's id
  has_author:bool; author:[32]u8;                   // the first p tag
  kind:int=-1;                                      // the k tag, or -1
  has_address:bool; address:NostrTag;               // the first a tag
  embedded:bool; inner:NostrEvent;                  // the content, read
}
```

<!-- source: coax/lib/nostr/nip18.coax read -->
```jai
// A repost: the first e tag (with a relay) is the target; the first p, a
// and k tags are kept. Non-empty content is unescaped into inner and read
// as an event with its id recomputed (scratch holds its serialization),
// and it must agree with the tags.
n18_repost :: (e:NostrEvent,inner:[]u8,scratch:[]u8)->N18Repost {
  r:N18Repost;
  if e.error!=NE_OK {return r;}
  if e.kind!=6 && e.kind!=16 {return n18_refuse(r,N18_KIND,-1);}
  found:=false;i:=0;
  while true {
    t:=nostr_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error!=NT_OK {r.tag_error=t.error;return n18_refuse(r,N18_TAG,i);}
    if t.type==NT_E && !found {
      if !t.has_relay {return n18_refuse(r,N18_RELAY,i);}
      found=true;r.target=t.id;
    }
    if t.type==NT_P && !r.has_author {r.has_author=true;r.author=t.pubkey;}
    if t.type==NT_A && !r.has_address {r.has_address=true;r.address=t;}
    i+=1;
  }
  if !found {return n18_refuse(r,N18_MISSING,-1);}
  k:[NT_MAX_VALUE]u8;kt:=n10_text(e,"k",k[..]);
  if kt.present {r.kind=n18_kind(cast(string)k[0..<kt.n]);}
  n:=json_unescape(e.original[e.content_from..<e.content_to],inner);
  if n<0 {return n18_refuse(r,N18_OUTPUT,-1);}
  if n==0 {r.status=N18_OK;return r;}
  r.embedded=true;r.inner=nostr_event_read(cast(string)inner[0..<n],scratch,false);
  if r.inner.error!=NE_OK {r.tag_error=r.inner.error;return n18_refuse(r,N18_INNER,-1);}
  if (e.kind==6)!=(r.inner.kind==1) {return n18_refuse(r,N18_INNER_KIND,-1);}
  agree:=ns_equal32(r.inner.id,r.target);
  if r.has_author && !ns_equal32(r.author,r.inner.public_key) {agree=false;}
  if r.kind>=0 && r.kind!=r.inner.kind {agree=false;}
  if r.has_address {
    d:=nostr_tag_d(r.inner);a:=r.address;
    if a.address_kind!=r.inner.kind || !ns_equal32(a.pubkey,r.inner.public_key) {agree=false;}
    if cast(string)a.d[0..<a.d_n]!=cast(string)d.d[0..<d.d_n] {agree=false;}
  }
  if !agree {return n18_refuse(r,N18_MISMATCH,-1);}
  r.status=N18_OK;return r;
}
```

### Writing a repost

<!-- source: coax/lib/nostr/nip18.coax write -->
```jai
// The tags of a repost of target, seen on relay: kind 6 for a kind 1
// note, kind 16 otherwise with a k tag, and an a tag for a replaceable or
// addressable target. The content to sign is target.original.
n18_repost_tags :: (target:NostrEvent,relay:string,out:[]u8)->N18Tags {
  r:N18Tags;
  if target.error!=NE_OK {return r;}
  if !nt_relay_ok(relay) {r.status=N18_RELAY;return r;}
  r.kind=ifx target.kind==1 then 6 else 16;
  n:=json_raw(out,0,"[[\"e\",");n=n10_hex(out,n,target.id);n=json_raw(out,n,",");n=json_write_string(out,n,relay);
  n=json_raw(out,n,"],[\"p\",");n=n10_hex(out,n,target.public_key);n=json_raw(out,n,"]");
  if r.kind==16 {n=json_raw(out,n,",[\"k\",\"");n=json_write_int(out,n,target.kind);n=json_raw(out,n,"\"]");}
  class:=ns_class(target.kind);
  if class==NS_ADDRESSABLE || class==NS_REPLACEABLE {
    pub:[66]u8;m:=n10_hex(pub[..],0,target.public_key);
    d:=nostr_tag_d(target);
    n=json_raw(out,n,",[\"a\",\"");n=json_write_int(out,n,target.kind);n=json_raw(out,n,":");
    n=json_raw(out,n,cast(string)pub[1..<65]);n=json_raw(out,n,":");
    // The identifier, escaped, ends the value: its closing quote is kept
    // and its opening one dropped.
    id:[6*NT_MAX_VALUE+2]u8;q:=json_write_string(id[..],0,cast(string)d.d[0..<d.d_n]);
    n=json_raw(out,n,cast(string)id[1..<q]);n=json_raw(out,n,"]");
  }
  n=json_raw(out,n,"]");
  if n== -1 {r.status=N18_OUTPUT;return r;}
  r.n=n;r.status=N18_OK;return r;
}
```

The relay is required, because NIP-18 says the `e` tag must include one;
`""` or anything but a `ws://` or `wss://` URL refuses with `N18_RELAY`. A
buffer too small refuses with `N18_OUTPUT`. NIP-18's quote reposts, `q` tags
on an event that cites another in its content, are chapter 51's subject and
are not read here.

## Reactions

A reaction is a kind 7 event. NIP-25 gives its content one of these
meanings:

| Content | Meaning | `value` |
| --- | --- | --- |
| `+` or empty | a like | `N25_LIKE` |
| `-` | a dislike | `N25_DISLIKE` |
| `:shortcode:` | a NIP-30 custom emoji | `N25_CUSTOM` |
| anything else | an emoji or text, neither like nor dislike | `N25_TEXT` |

A reaction must have an `e` tag with the target's id, and should have a `p`
tag with the target's author and a `k` tag with its kind. A reaction to an
addressable event should also have an `a` tag. When a client adds other `e`
or `p` tags, which NIP-25 does not recommend, the target's are the last
ones, so `n25_reaction` reads the last `e` tag as the target and the last
`p` tag as its author.

A custom emoji reaction names a shortcode of letters, digits, `-` and `_`
between colons, and the event's emoji tag gives the image:
`["emoji", "tea", "https://example.com/tea.png"]`. NIP-25 says there should
be one emoji tag. `n25_reaction` refuses with `N25_EMOJI` when the first
emoji tag doesn't name the content's shortcode, because a client then has no
image to show. Content such as `:)` or `:a b:` is not a shortcode and reads
as text.

Kind 17 reacts to content outside Nostr, named by NIP-73 `k` and `i` tags:
`["k", "web"]`, `["i", "https://example.com"]`. It has no `e` tag. The reader
keeps the first `i` value in `external` and refuses with `N25_MISSING` when
the `k` or `i` tag is missing or empty.

<!-- source: coax/lib/nostr/nip25.coax record -->
```jai
N25Reaction :: struct {
  status:int=N25_EVENT; tag_error:int; at:int=-1;   // the refusing tag
  value:int;                                        // like, dislike, text, custom
  content:[NT_MAX_VALUE]u8; content_n:int;          // decoded
  target:[32]u8;                                    // kind 7: the last e tag
  has_author:bool; author:[32]u8;                   // kind 7: the last p tag
  kind:int=-1;                                      // the k tag, or -1
  has_address:bool; address:NostrTag;               // kind 7: the first a tag
  external:[NT_MAX_VALUE]u8; external_n:int;        // kind 17: the i value
}
N25Tally :: struct {
  likes:int; dislikes:int; others:int;              // one per author
  superseded:int;                                   // older ones by a counted author
  refused:int;                                      // kind 7 or 17 events that don't read
}
```

<!-- source: coax/lib/nostr/nip25.coax read -->
```jai
// A reaction: kind 7 names an event by its last e tag, kind 17 external
// content by its i and k tags. Every standard tag is read and checked.
n25_reaction :: (e:NostrEvent)->N25Reaction {
  r:N25Reaction;
  if e.error!=NE_OK || e.content_from<0 || e.content_from>e.content_to || e.content_to>e.original.count {return r;}
  if e.kind!=7 && e.kind!=17 {return n25_refuse(r,N25_KIND,-1);}
  found:=false;i:=0;
  while true {
    t:=nostr_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error!=NT_OK {r.tag_error=t.error;return n25_refuse(r,N25_TAG,i);}
    if t.type==NT_E {found=true;r.target=t.id;}
    if t.type==NT_P {r.has_author=true;r.author=t.pubkey;}
    if t.type==NT_A && !r.has_address {r.has_address=true;r.address=t;}
    i+=1;
  }
  n:=json_unescape(e.original[e.content_from..<e.content_to],r.content[..]);
  if n<0 {return n25_refuse(r,N25_LIMIT,-1);}
  r.content_n=n;text:=cast(string)r.content[0..<n];
  k:[NT_MAX_VALUE]u8;kt:=n10_text(e,"k",k[..]);
  if kt.status!=N10_OK {return n25_refuse(r,N25_LIMIT,-1);}
  if e.kind==17 {
    x:=n10_text(e,"i",r.external[..]);
    if x.status!=N10_OK {return n25_refuse(r,N25_LIMIT,-1);}
    if !x.present || x.n==0 || !kt.present || kt.n==0 {return n25_refuse(r,N25_MISSING,-1);}
    r.external_n=x.n;
  } else {
    if !found {return n25_refuse(r,N25_MISSING,-1);}
    if kt.present {r.kind=n18_kind(cast(string)k[0..<kt.n]);}
  }
  r.value=ifx text=="" || text=="+" then N25_LIKE else ifx text=="-" then N25_DISLIKE else N25_TEXT;
  if n25_shortcode(text) {
    name:[NT_MAX_VALUE]u8;m:=n10_text(e,"emoji",name[..]);
    if m.status!=N10_OK {return n25_refuse(r,N25_LIMIT,-1);}
    if !m.present || cast(string)name[0..<m.n]!=text[1..<text.count-1] {return n25_refuse(r,N25_EMOJI,-1);}
    r.value=N25_CUSTOM;
  }
  r.status=N25_OK;return r;
}
```

Other refusals: a kind other than 7 or 17 with `N25_KIND`, a malformed
standard tag with `N25_TAG` and its index, a kind 7 with no `e` tag with
`N25_MISSING`, and content, `k`, `i` or emoji values over 256 bytes with
`N25_LIMIT`.

## Counting reactions

A person can react to the same event more than once, for example a dislike
and later a like. NIP-25 does not say how to count them. `n25_tally` counts
one reaction per author and target: the latest by `created_at`, and on a
tie the one with the lower id, which is NIP-01's rule for replaceable
events. The others count as superseded. A kind 7 or 17 event that doesn't
read counts as refused; events of other kinds are passed over.

<!-- source: coax/lib/nostr/nip25.coax tally -->
```jai
// Whether a is newer than b: a later created_at, or the same one and a
// lower id.
n25_newer :: (a:NostrEvent,b:NostrEvent)->bool {
  if a.created_at>b.created_at {return true;}
  if a.created_at<b.created_at {return false;}
  for i:0..<32 {
    if a.id[i]<b.id[i] {return true;}
    if a.id[i]>b.id[i] {return false;}
  }
  return false;
}
// The reactions in set to target, one per author: the newest of each
// author's readable kind 7 reactions to it. An event that appears twice
// counts once. Events of other kinds are passed over.
n25_tally :: (set:[]NostrEvent,target:[32]u8)->N25Tally {
  t:N25Tally;
  for i:0..<set.count {
    r:=n25_reaction(set[i]);
    if r.status==N25_KIND {continue;}
    if r.status!=N25_OK {t.refused+=1;continue;}
    if set[i].kind!=7 || !ns_equal32(r.target,target) {continue;}
    counted:=true;
    for j:0..<set.count {
      if !ns_equal32(set[j].public_key,set[i].public_key) {continue;}
      o:=n25_reaction(set[j]);
      if o.status!=N25_OK || set[j].kind!=7 || !ns_equal32(o.target,target) {continue;}
      // The same event earlier in the set is the one counted.
      same:=ns_equal32(set[j].id,set[i].id) && set[j].created_at==set[i].created_at;
      if n25_newer(set[j],set[i]) || (same && j<i) {counted=false;}
    }
    if !counted {t.superseded+=1;continue;}
    if r.value==N25_LIKE {t.likes+=1;} else if r.value==N25_DISLIKE {t.dislikes+=1;} else {t.others+=1;}
  }
  return t;
}
```

The tally trusts each event's pubkey and `created_at`. A caller that counts
reactions from relays verifies each event's signature first, or anyone can
write reactions in someone else's name.

### Worked example: the reactions to N

Four reactions name N:

| Event | Author | `created_at` | Content | Value |
| --- | --- | --- | --- | --- |
| X1 | Bob | T0 + 240 | `+` | like |
| X2 | Carol | T0 + 300 | `-` | dislike |
| X3 | Carol | T0 + 360 | `+` | like |
| X4 | Alice | T0 + 420 | `:tea:` | custom emoji |

1. Bob has one reaction, X1: a like.
2. Carol has two. X3 is later than X2, so X3 counts, a like, and X2 is
   superseded.
3. Alice has one, X4: neither a like nor a dislike, so it counts under
   others.

The tally: 2 likes, 0 dislikes, 1 other, 1 superseded.

## Comments

A NIP-22 comment, kind 1111, is plain text about something: an event, an
address, or an external identifier such as a URL. Its tags name two things:

- The root scope, in uppercase: `E` (an event id), `A` (an address) or `I`
  (an external identifier), with `K`, the root's kind, and `P`, the root's
  author when it has one.
- The parent, the item the comment answers directly, in lowercase: `e`,
  `a` or `i`, with `k` and `p`.

A top-level comment's parent is the root itself, so the uppercase and
lowercase tags name the same item. A reply to a comment keeps the root
scope and names the comment as its parent, with `k` 1111. For an `I` or `i`
scope, the kind is a NIP-73 type such as `web`.

`n22_comment` reads the first of `E`, `A` and `I` as the root and the first
of `e`, `a` and `i` as the parent. Both, with their `K` and `k`, are
required, or it refuses with `N34_MISSING`. An `E` or `e` must hold a
64-digit hex id, an `A` or `a` an address, and either of those a decimal
kind; a `P` or `p` must hold a pubkey. Otherwise it refuses with
`N34_MALFORMED`.

### The parent's kind

A comment answers either the root item or another comment under it. So its
parent is either kind 1111, or the root, with the root's kind. A comment
whose `k` is neither 1111 nor its `K` describes a parent that can't exist
in its scope, and the scopes disagree. `n22_kinds` refuses it with
`N22_KINDS`:

<!-- source: coax/lib/nostr/nip22.coax kinds -->
```jai
// A comment's parent is the root item (a top-level comment) or another
// comment under it (kind 1111). Any other parent kind must be the root's
// kind, as spelled in K, or the comment refuses.
n22_kinds :: (e:NtEvent,c:N22Comment)->int {
 if c.parent.kind_value==N34_KIND_COMMENT {return N34_OK;}
 if nt_raw(e,c.parent.kind.v[0])!=nt_raw(e,c.root.kind.v[0]) {return N22_KINDS;}
 return N34_OK;
}
```

The comparison is byte for byte, so `K` `web` and `k` `web` agree and
`30023` and `3002` do not. A missing or malformed scope refuses as that
before the kinds are compared. All six of NIP-22's examples read with this
rule (see the checks). OpenAgents' issue comments (`K` and `k` 1621) and
project notes (`K` and `k` 30621) are top-level comments, so they read as
before.

The pinned NIP-22 text doesn't forbid comments on kind 1 notes, so `K` 1 is
not refused. Replies to kind 1 notes use NIP-10's `e` tags (chapter 50).

### Worked example: a reply to a comment on an article

Alice's article R is kind 30023 with `d` tag `tea-storage`, so its address
is `30023:f9308a01...bce036f9:tea-storage`. Bob writes C1, a top-level
comment on R. Carol replies to C1 with C2. Work out C2's tags:

1. The root scope is C1's root scope, copied as C1 wrote it. C1's root is
   the article's address, so C2 gets
   `["A", "30023:f9308a01...bce036f9:tea-storage", "wss://relay.example.com"]`.
2. The root kind is the article's: `["K", "30023"]`.
3. The root author is Alice: `["P", "f9308a01...bce036f9", "wss://relay.example.com"]`.
4. The parent is C1, a regular event, so it is named by id with an `e` tag:
   `["e", "78f44c6c...a0c835ad", "wss://relay.example.com", "dff1d77f...502ba659"]`.
   The fourth value is C1's author, Bob.
5. The parent kind is C1's: `["k", "1111"]`.
6. The parent author is Bob: `["p", "dff1d77f...502ba659", "wss://relay.example.com"]`.

Check the kinds: `k` is 1111, a comment, so C2 reads. C1 itself has `K`
30023 and `k` 30023: its parent is the root. Had Carol written `k` 1, the
parent would be a kind 1 event in an article's scope, which is neither the
root nor a comment, and the comment would refuse with `N22_KINDS`.

C1 follows NIP-22's blog example: its parent has both the article's address
(`a`) and its id (`e`), because the parent is addressable. `n22_comment`
reads `e` first.

## The example

The example is [nostr/examples/responses.coax](../examples/responses.coax).
It signs Alice's note N and article R with the BIP-340 test vector keys 0, 1
and 2 as Alice, Bob and Carol. Bob reposts N and Carol reposts R, with tags
from `n18_repost_tags`. Bob, Carol and Alice react to N with tags written as
NIP-25's example code writes them. Bob comments on R and Carol replies.
Then it reads everything back, tallies the reactions and shows the
refusals. The refusals use unsigned events, because no signer here would
write them.

<!-- source: nostr/examples/responses.coax example -->
```jai
#load "../../coax/lib/nostr/nip25.coax";
#load "../../coax/lib/nostr/nip34-event.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

REPOST :: [10] string.["ok", "event", "kind", "tag", "missing e", "no relay", "content is no event", "embedded kind", "mismatch", "output"];
REACTION :: [7] string.["ok", "event", "kind", "tag", "missing", "no emoji tag", "limit"];
VALUE :: [4] string.["like", "dislike", "text", "custom emoji"];
COMMENT :: [7] string.["ok", "event", "kind", "missing", "malformed", "output", "kinds disagree"];
TAG_STATUS :: [8] string.["ok", "event", "index", "shape", "hex", "relay", "address", "limit"];
EVENT_STATUS :: [9] string.["ok", "json", "shape", "limit", "hex", "output", "alias", "id does not match", "signature"];
// BIP-340 test vectors 0, 1 and 2's published public keys.
PEOPLE :: [3] string.["Alice", "Bob", "Carol"];
PUB :: [3] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
  "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"];

// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
hex32 :: (id: [32] u8, out: [] u8) -> string {
  own := id; r := hex_encode_checked(cast(string) own[..], out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<64];
}

// The events, by name, so the output names ids by name.
Named :: struct { name: string; event: NostrEvent; }
EVENTS :: 10;
show_id :: (all: [] Named, id: [32] u8) {
  for n: all { if n.event.error == NE_OK && ns_equal32(n.event.id, id) { print("%", n.name); return; } }
  for k: 0..<3 { key: [32] u8; decode_hex(PUB[k], key[..]); if ns_equal32(key, id) { print("%", PEOPLE[k]); return; } }
  text: [64] u8; print("%...", hex32(id, text[..])[0..<8]);
}

// Sign an event into out and read it back with its ID and signature checked.
sign :: (key: [] u8, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  aux: [32] u8; work: [1] NostrSignWork; scratch: [8192] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key, aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [8192] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified;
  return e;
}

// A reaction's tags, as NIP-25's example code writes them: the target with
// a relay hint and its author, the author with the hint, and its kind.
reaction_tags :: (target: NostrEvent, relay: string, extra: string, out: [] u8) -> string {
  n := json_raw(out, 0, "[[\"e\",");
  n = n10_hex(out, n, target.id); n = json_raw(out, n, ",");
  n = json_write_string(out, n, relay); n = json_raw(out, n, ",");
  n = n10_hex(out, n, target.public_key); n = json_raw(out, n, "],[\"p\",");
  n = n10_hex(out, n, target.public_key); n = json_raw(out, n, ",");
  n = json_write_string(out, n, relay); n = json_raw(out, n, "],[\"k\",\"");
  n = json_write_int(out, n, target.kind); n = json_raw(out, n, "\"]");
  n = json_raw(out, n, extra); n = json_raw(out, n, "]");
  #assert n > 0;
  return cast(string) out[0..<n];
}

// One scope tag: name, then each value of the tag t as its raw JSON text.
// A comma goes before every tag but the first, which follows the "[" at 0.
copy_tag :: (out: [] u8, at: int, name: string, e: NtEvent, t: NtTag) -> int {
  n := ifx at > 1 then json_raw(out, at, ",") else at;
  n = json_raw(out, n, "[\""); n = json_raw(out, n, name); n = json_raw(out, n, "\"");
  for v: 0..<t.values {
    n = json_raw(out, n, ",\""); n = json_raw(out, n, nt_raw(e, t.v[v])); n = json_raw(out, n, "\"");
  }
  return json_raw(out, n, "]");
}
// A top-level comment on an addressable event: its address is the root A
// and the parent a, its id the parent e, its kind K and k, its author P and p.
comment_on :: (root: NostrEvent, relay: string, out: [] u8) -> string {
  pub: [64] u8; d := nostr_tag_d(root);
  address: [512] u8;
  m := json_write_int(address[..], 0, root.kind); m = json_raw(address[..], m, ":");
  m = json_raw(address[..], m, hex32(root.public_key, pub[..])); m = json_raw(address[..], m, ":");
  m = json_raw(address[..], m, cast(string) d.d[0..<d.d_n]);
  coord := cast(string) address[0..<m];
  id: [64] u8;
  n := json_raw(out, 0, "[");
  names := [2] string.["A", "a"]; kinds := [2] string.["K", "k"]; authors := [2] string.["P", "p"];
  for s: 0..<2 {
    if s > 0 { n = json_raw(out, n, ","); }
    n = json_raw(out, n, "[\""); n = json_raw(out, n, names[s]); n = json_raw(out, n, "\",");
    n = json_write_string(out, n, coord); n = json_raw(out, n, ","); n = json_write_string(out, n, relay); n = json_raw(out, n, "]");
    if s > 0 {
      n = json_raw(out, n, ",[\"e\",\""); n = json_raw(out, n, hex32(root.id, id[..]));
      n = json_raw(out, n, "\","); n = json_write_string(out, n, relay); n = json_raw(out, n, "]");
    }
    n = json_raw(out, n, ",[\""); n = json_raw(out, n, kinds[s]); n = json_raw(out, n, "\",\"");
    n = json_write_int(out, n, root.kind); n = json_raw(out, n, "\"]");
    n = json_raw(out, n, ",[\""); n = json_raw(out, n, authors[s]); n = json_raw(out, n, "\",\"");
    n = json_raw(out, n, hex32(root.public_key, pub[..])); n = json_raw(out, n, "\","); n = json_write_string(out, n, relay); n = json_raw(out, n, "]");
  }
  n = json_raw(out, n, "]");
  #assert n > 0;
  return cast(string) out[0..<n];
}
// A reply to a comment: the parent's root scope, K and P copied as they
// are, then the parent itself as e (with its author), k 1111 and p.
reply_to_comment :: (parent: NostrEvent, relay: string, out: [] u8) -> string {
  e := nt_event_of(parent); c := n22_comment(e);
  #assert c.status == N34_OK;
  root_name: [1] u8; root_name[0] = c.root.at;
  n := json_raw(out, 0, "[");
  n = copy_tag(out, n, cast(string) root_name[..], e, c.root.ref);
  n = copy_tag(out, n, "K", e, c.root.kind);
  if c.root.author.n > 0 { n = copy_tag(out, n, "P", e, c.root.author); }
  id: [64] u8; pub: [64] u8;
  n = json_raw(out, n, ",[\"e\",\""); n = json_raw(out, n, hex32(parent.id, id[..]));
  n = json_raw(out, n, "\","); n = json_write_string(out, n, relay);
  n = json_raw(out, n, ",\""); n = json_raw(out, n, hex32(parent.public_key, pub[..]));
  n = json_raw(out, n, "\"],[\"k\",\"1111\"],[\"p\",\""); n = json_raw(out, n, hex32(parent.public_key, pub[..]));
  n = json_raw(out, n, "\","); n = json_write_string(out, n, relay); n = json_raw(out, n, "]]");
  #assert n > 0;
  return cast(string) out[0..<n];
}

// Print what n18_repost reads from a repost.
show_repost :: (all: [] Named, label: string, e: NostrEvent) {
  inner: [8192] u8; scratch: [8192] u8;
  r := n18_repost(e, inner[..], scratch[..]);
  print("%: %", label, REPOST[r.status]);
  if r.status == N18_TAG { print(", tag % is %\n", r.at, TAG_STATUS[r.tag_error]); return; }
  if r.status == N18_RELAY { print(" at tag %\n", r.at); return; }
  if r.status == N18_INNER { print(", %\n", EVENT_STATUS[r.inner.error]); return; }
  if r.status != N18_OK { print("\n"); return; }
  print(", kind %, target ", e.kind); show_id(all, r.target);
  if r.has_author { print(", author "); show_id(all, r.author); }
  if r.kind >= 0 { print(", k %", r.kind); }
  if r.has_address { print(", a %:...:%", r.address.address_kind, cast(string) r.address.d[0..<r.address.d_n]); }
  if r.embedded {
    // The reading checks the embedded id; its signature is the caller's to check.
    check: [8192] u8;
    v := nostr_event_verify_vartime(r.inner.original, check[..], false);
    print(", embedded kind % signature %", r.inner.kind, ifx v.error == NE_OK && v.verified then "valid" else "invalid");
  }
  print("\n");
}
// Print what n25_reaction reads from a reaction.
show_reaction :: (all: [] Named, label: string, e: NostrEvent) {
  r := n25_reaction(e);
  print("%: %", label, REACTION[r.status]);
  if r.status == N25_TAG { print(", tag % is %\n", r.at, TAG_STATUS[r.tag_error]); return; }
  if r.status != N25_OK { print("\n"); return; }
  print(", % \"%\"", VALUE[r.value], cast(string) r.content[0..<r.content_n]);
  if e.kind == 17 { print(", external %\n", cast(string) r.external[0..<r.external_n]); return; }
  print(", target "); show_id(all, r.target);
  if r.kind >= 0 { print(", k %", r.kind); }
  print("\n");
}
// Print what n22_comment reads from a comment.
show_comment :: (all: [] Named, label: string, ev: NostrEvent) {
  e := nt_event_of(ev); c := n22_comment(e);
  print("%: %", label, COMMENT[c.status]);
  if c.status != N34_OK { print("\n"); return; }
  scopes := [2] N22Scope.[c.root, c.parent];
  for s: scopes {
    ref := nt_raw(e, s.ref.v[0]);
    letter: [1] u8; letter[0] = s.at;
    print(", % ", cast(string) letter[..]);
    if s.at == 'E' || s.at == 'e' {
      id: [32] u8; decode_hex(ref, id[..]); show_id(all, id);
    } else { print("%", ref); }
    print(" kind %", nt_raw(e, s.kind.v[0]));
    if s.author.n > 0 { key: [32] u8; decode_hex(nt_raw(e, s.author.v[0]), key[..]); print(" by "); show_id(all, key); }
  }
  print("\n");
}

// An unsigned event with these tags and this content (a JSON string's raw
// text), parsed but not verified: its id and sig are zeros. Used for the
// refusals, which no signer here would write.
EVENT_HEAD :: "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\",\"created_at\":1761686400,\"kind\":";
SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
unsigned :: (kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, EVENT_HEAD);
  n = json_write_int(out, n, kind);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, content);
  n = json_raw(out, n, ",\"sig\":\""); n = json_raw(out, n, SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

// BIP-340 test vectors 0, 1 and 2's published secret keys. Never sign with
// a real key in an example.
ALICE :: "0000000000000000000000000000000000000000000000000000000000000003";
BOB :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
CAROL :: "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9";
RELAY :: "wss://relay.example.com";
T0 :: 1761686400;
EMOJI :: "[\"emoji\",\"tea\",\"https://example.com/tea.png\"]";

main :: () {
  alice: [32] u8; bob: [32] u8; carol: [32] u8;
  decode_hex(ALICE, alice[..]); decode_hex(BOB, bob[..]); decode_hex(CAROL, carol[..]);
  all: [EVENTS] Named;
  buf: [EVENTS][8192] u8; tags: [4096] u8;

  print("a note and an article by Alice\n");
  all[0].name = "N"; all[0].event = sign(alice[..], T0, 1, "[]", "Oolong keeps for years.", buf[0][..]);
  all[1].name = "R"; all[1].event = sign(alice[..], T0 + 60, 30023, "[[\"d\",\"tea-storage\"],[\"title\",\"Storing tea\"]]", "Keep tea dry, dark and sealed.", buf[1][..]);

  print("reposts\n");
  t := n18_repost_tags(all[0].event, RELAY, tags[..]); #assert t.status == N18_OK;
  print("S6 (kind %) tags: %\n", t.kind, cast(string) tags[0..<t.n]);
  all[2].name = "S6"; all[2].event = sign(bob[..], T0 + 120, t.kind, cast(string) tags[0..<t.n], all[0].event.original, buf[2][..]);
  t = n18_repost_tags(all[1].event, RELAY, tags[..]); #assert t.status == N18_OK;
  print("S16 (kind %) tags: %\n", t.kind, cast(string) tags[0..<t.n]);
  all[3].name = "S16"; all[3].event = sign(carol[..], T0 + 180, t.kind, cast(string) tags[0..<t.n], all[1].event.original, buf[3][..]);

  print("reactions to N\n");
  print("tags: %\n", reaction_tags(all[0].event, RELAY, "", tags[..]));
  all[4].name = "X1"; all[4].event = sign(bob[..], T0 + 240, 7, reaction_tags(all[0].event, RELAY, "", tags[..]), "+", buf[4][..]);
  all[5].name = "X2"; all[5].event = sign(carol[..], T0 + 300, 7, reaction_tags(all[0].event, RELAY, "", tags[..]), "-", buf[5][..]);
  all[6].name = "X3"; all[6].event = sign(carol[..], T0 + 360, 7, reaction_tags(all[0].event, RELAY, "", tags[..]), "+", buf[6][..]);
  all[7].name = "X4"; all[7].event = sign(alice[..], T0 + 420, 7, reaction_tags(all[0].event, RELAY, ",[\"emoji\",\"tea\",\"https://example.com/tea.png\"]", tags[..]), ":tea:", buf[7][..]);

  print("comments on R\n");
  c := comment_on(all[1].event, RELAY, tags[..]);
  print("C1 tags: %\n", c);
  all[8].name = "C1"; all[8].event = sign(bob[..], T0 + 480, 1111, c, "Does this hold for green tea?", buf[8][..]);
  c = reply_to_comment(all[8].event, RELAY, tags[..]);
  print("C2 tags: %\n", c);
  all[9].name = "C2"; all[9].event = sign(carol[..], T0 + 540, 1111, c, "Green tea fades in months.", buf[9][..]);

  print("each event's id\n");
  for e: all { hex: [64] u8; print("%: %, verified %\n", e.name, hex32(e.event.id, hex[..]), e.event.verified); }

  print("read back\n");
  show_repost(all[..], "S6", all[2].event);
  show_repost(all[..], "S16", all[3].event);
  for k: 4..<8 { show_reaction(all[..], all[k].name, all[k].event); }
  show_comment(all[..], "C1", all[8].event);
  show_comment(all[..], "C2", all[9].event);

  print("the tally for N, from X1..X4 and the rest\n");
  set: [EVENTS] NostrEvent;
  for k: 0..<EVENTS { set[k] = all[EVENTS - 1 - k].event; }
  tally := n25_tally(set[..], all[0].event.id);
  print("likes %, dislikes %, others %, superseded %, refused %\n", tally.likes, tally.dislikes, tally.others, tally.superseded, tally.refused);

  print("refusals\n");
  bad: [8192] u8; more: [4096] u8;
  // C2's tags without its K tag.
  e := unsigned(1111, "[[\"A\",\"30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-storage\"],[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"],[\"k\",\"1111\"]]", "", bad[..]);
  show_comment(all[..], "comment without K", e);
  e = unsigned(1111, "[[\"A\",\"30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-storage\"],[\"K\",\"30023\"],[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"],[\"k\",\"1\"]]", "", bad[..]);
  show_comment(all[..], "comment K 30023, k 1", e);
  e = unsigned(1111, "[[\"I\",\"https://example.com\"],[\"K\",\"web\"],[\"i\",\"https://example.com\"],[\"k\",\"30023\"]]", "", bad[..]);
  show_comment(all[..], "comment K web, k 30023", e);
  // A kind 6 whose e tag names R but whose content is N.
  e = unsigned(6, n18_tags_named(all[1].event, more[..]), all[0].event.original, bad[..]);
  show_repost(all[..], "kind 6, e R, content N", e);
  t = n18_repost_tags(all[0].event, RELAY, tags[..]);
  e = unsigned(6, n18_tags_named(all[1].event, more[..]), all[1].event.original, bad[..]);
  show_repost(all[..], "kind 6 holding R", e);
  e = unsigned(16, cast(string) tags[0..<t.n], all[0].event.original, bad[..]);
  show_repost(all[..], "kind 16 holding N", e);
  // N's text with one word changed: its id no longer recomputes.
  edited: [4096] u8; n := json_raw(edited[..], 0, all[0].event.original);
  for i: 0..<n - 5 { if cast(string) edited[i..<i + 5] == "years" { edited[i] = 'Y'; } }
  e = unsigned(6, cast(string) tags[0..<t.n], cast(string) edited[0..<n], bad[..]);
  show_repost(all[..], "kind 6, N edited", e);
  e = unsigned(6, "[[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"]]", "", bad[..]);
  show_repost(all[..], "kind 6, e without relay", e);
  e = unsigned(7, "[[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"]]", "+", bad[..]);
  show_reaction(all[..], "reaction without e", e);
  e = unsigned(7, reaction_tags(all[0].event, RELAY, "", tags[..]), ":tea:", bad[..]);
  show_reaction(all[..], "reaction :tea: without emoji tag", e);
  e = unsigned(1, reaction_tags(all[0].event, RELAY, "", tags[..]), "+", bad[..]);
  show_reaction(all[..], "reaction of kind 1", e);
  print("accepted\n");
  e = unsigned(17, "[[\"k\",\"web\"],[\"i\",\"https://example.com\"]]", "⭐", bad[..]);
  show_reaction(all[..], "kind 17", e);
  t = n18_repost_tags(all[0].event, RELAY, tags[..]);
  e = unsigned(6, cast(string) tags[0..<t.n], "", bad[..]);
  show_repost(all[..], "kind 6, empty content", e);
}

// The tags n18_repost_tags writes for target, without the k and a tags:
// an e tag for target and a p tag for its author, as a kind 6 would carry.
n18_tags_named :: (target: NostrEvent, out: [] u8) -> string {
  n := json_raw(out, 0, "[[\"e\","); n = n10_hex(out, n, target.id);
  n = json_raw(out, n, ",\"wss://relay.example.com\"],[\"p\","); n = n10_hex(out, n, target.public_key); n = json_raw(out, n, "]]");
  #assert n > 0;
  return cast(string) out[0..<n];
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/responses.coax --run-steps 250000000
```

The output:

```text
a note and an article by Alice
reposts
S6 (kind 6) tags: [["e","6b3c564c3417f70992cd4dc0ddd12bb5448ab1bf1e327fb11575b5db0c764321","wss://relay.example.com"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"]]
S16 (kind 16) tags: [["e","fdb6fe34ceb4cf646cb68bf987ca2577d3aaecd1c7dde5eb636d6bb647e532e1","wss://relay.example.com"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["k","30023"],["a","30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-storage"]]
reactions to N
tags: [["e","6b3c564c3417f70992cd4dc0ddd12bb5448ab1bf1e327fb11575b5db0c764321","wss://relay.example.com","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","wss://relay.example.com"],["k","1"]]
comments on R
C1 tags: [["A","30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-storage","wss://relay.example.com"],["K","30023"],["P","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","wss://relay.example.com"],["a","30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-storage","wss://relay.example.com"],["e","fdb6fe34ceb4cf646cb68bf987ca2577d3aaecd1c7dde5eb636d6bb647e532e1","wss://relay.example.com"],["k","30023"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","wss://relay.example.com"]]
C2 tags: [["A","30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-storage","wss://relay.example.com"],["K","30023"],["P","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","wss://relay.example.com"],["e","78f44c6caec878080ccaca6ce225181b5a7d56d72b18a5e9aed90240a0c835ad","wss://relay.example.com","dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"],["k","1111"],["p","dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659","wss://relay.example.com"]]
each event's id
N: 6b3c564c3417f70992cd4dc0ddd12bb5448ab1bf1e327fb11575b5db0c764321, verified true
R: fdb6fe34ceb4cf646cb68bf987ca2577d3aaecd1c7dde5eb636d6bb647e532e1, verified true
S6: 6cf3a79287688ebbf66feae868458bd728a83bd7ef658fa49adf6fb7dcd90654, verified true
S16: 1ab0caa7f9f11bd8346c3322075fa287488b0feec038dbe290a07195e0fa43ce, verified true
X1: 2efea399f1bbe153269ce4f2d529bfe7b4ffa674b9b059a8bdfa4df63b794684, verified true
X2: 3c187a4773d49e218578670712e79420f595b99296d94c20547e4867e7a87b47, verified true
X3: c2fd961356c6b7dc639c6b6ce83b7102ea6cb6a5de791e02f16912ccf9d7a566, verified true
X4: 744446136e3d8d34da980aa99804bd7ffb918cafd728a74116b9256515c62339, verified true
C1: 78f44c6caec878080ccaca6ce225181b5a7d56d72b18a5e9aed90240a0c835ad, verified true
C2: 001b02a79825014c0eeb2964959c9bfef42aeb1bca6a8cb3afbe0b88b9638834, verified true
read back
S6: ok, kind 6, target N, author Alice, embedded kind 1 signature valid
S16: ok, kind 16, target R, author Alice, k 30023, a 30023:...:tea-storage, embedded kind 30023 signature valid
X1: ok, like "+", target N, k 1
X2: ok, dislike "-", target N, k 1
X3: ok, like "+", target N, k 1
X4: ok, custom emoji ":tea:", target N, k 1
C1: ok, A 30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-storage kind 30023 by Alice, e R kind 30023 by Alice
C2: ok, A 30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-storage kind 30023 by Alice, e C1 kind 1111 by Bob
the tally for N, from X1..X4 and the rest
likes 2, dislikes 0, others 1, superseded 1, refused 0
refusals
comment without K: missing
comment K 30023, k 1: kinds disagree
comment K web, k 30023: kinds disagree
kind 6, e R, content N: mismatch
kind 6 holding R: embedded kind
kind 16 holding N: embedded kind
kind 6, N edited: content is no event, id does not match
kind 6, e without relay: no relay at tag 0
reaction without e: missing
reaction :tea: without emoji tag: no emoji tag
reaction of kind 1: kind
accepted
kind 17: ok, text "⭐", external https://example.com
kind 6, empty content: ok, kind 6, target N, author Alice
```

The repost tags, C2's tags and the tally match the worked examples. Both
embedded events check out: their ids are the `e` tags' ids and their
signatures are Alice's. The tally reads the whole set of ten events in
reverse; the notes, reposts and comments are other kinds and are passed
over.

The refusals:

- A comment without `K`: `missing`.
- `K` 30023 with `k` 1, and `K` web with `k` 30023: the parent is neither
  the root nor a comment, `kinds disagree`.
- A kind 6 whose `e` tag names R but whose content is N: `mismatch`.
- A kind 6 holding R, and a kind 16 holding N: `embedded kind`.
- N's JSON with one letter changed: the id no longer matches the text, so
  the content is no event, `id does not match`.
- An `e` tag without a relay: `no relay` at tag 0.
- A reaction without an `e` tag: `missing`.
- `:tea:` without an emoji tag: `no emoji tag`.
- A reaction of kind 1: `kind`.

Two cases are read, not refused: a kind 17 star for a website, and a kind 6
with empty content, which names its target and embeds nothing.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/responses.out](../fixtures/responses.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/responses.coax](../examples/checks/responses.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The ids of N, R, X1 to X4, C1 and C2 and the tags of every event, from a
  Python script written from NIP-01, NIP-18, NIP-22 and NIP-25
  (`hashlib.sha256` over NIP-01's serialization). The script can't sign, so
  for S6 and S16 it gives tags only.
- The tally of N by hand, in both orders and without X3.
- A kind 6 holding Python's N reads N back.
- NIP-22's six examples (a blog post, a file, a reply to a comment, a
  website, a podcast episode and a reply to a podcast comment), each with
  the root and parent the NIP describes.
- NIP-25's examples: the website star, the podcast like, and `:soapbox:`.
  The NIP prints the custom emoji example without an `e` tag; the check adds
  one, because NIP-25 says there must always be one.

<!-- source: nostr/examples/checks/responses.coax check -->
```jai
// The ids and tags from a Python script written from NIP-01, NIP-18,
// NIP-22 and NIP-25 (it shares no code with Coax): the ids are
// hashlib.sha256 of json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False). Python can't sign, so S6 and
// S16, whose content holds a signed event, have tags here and no id.
PY_ID :: [8] string.[
  "6b3c564c3417f70992cd4dc0ddd12bb5448ab1bf1e327fb11575b5db0c764321",
  "fdb6fe34ceb4cf646cb68bf987ca2577d3aaecd1c7dde5eb636d6bb647e532e1",
  "2efea399f1bbe153269ce4f2d529bfe7b4ffa674b9b059a8bdfa4df63b794684",
  "3c187a4773d49e218578670712e79420f595b99296d94c20547e4867e7a87b47",
  "c2fd961356c6b7dc639c6b6ce83b7102ea6cb6a5de791e02f16912ccf9d7a566",
  "744446136e3d8d34da980aa99804bd7ffb918cafd728a74116b9256515c62339",
  "78f44c6caec878080ccaca6ce225181b5a7d56d72b18a5e9aed90240a0c835ad",
  "001b02a79825014c0eeb2964959c9bfef42aeb1bca6a8cb3afbe0b88b9638834"];
PY_NAME :: [8] string.["N", "R", "X1", "X2", "X3", "X4", "C1", "C2"];
PY_AUTHOR :: [8] int.[0, 0, 1, 2, 2, 0, 1, 2];
PY_AT :: [8] int.[0, 60, 240, 300, 360, 420, 480, 540];
PY_KIND :: [8] int.[1, 30023, 7, 7, 7, 7, 1111, 1111];
PY_CONTENT :: [8] string.["Oolong keeps for years.", "Keep tea dry, dark and sealed.", "+", "-", "+", ":tea:", "Does this hold for green tea?", "Green tea fades in months."];
PY_LIKE :: "[[\"e\",\"6b3c564c3417f70992cd4dc0ddd12bb5448ab1bf1e327fb11575b5db0c764321\",\"wss://relay.example.com\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"wss://relay.example.com\"],[\"k\",\"1\"]]";
PY_TAGS :: [8] string.[
  "[]",
  "[[\"d\",\"tea-storage\"],[\"title\",\"Storing tea\"]]",
  PY_LIKE, PY_LIKE, PY_LIKE,
  "[[\"e\",\"6b3c564c3417f70992cd4dc0ddd12bb5448ab1bf1e327fb11575b5db0c764321\",\"wss://relay.example.com\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"wss://relay.example.com\"],[\"k\",\"1\"],[\"emoji\",\"tea\",\"https://example.com/tea.png\"]]",
  "[[\"A\",\"30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-storage\",\"wss://relay.example.com\"],[\"K\",\"30023\"],[\"P\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"wss://relay.example.com\"],[\"a\",\"30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-storage\",\"wss://relay.example.com\"],[\"e\",\"fdb6fe34ceb4cf646cb68bf987ca2577d3aaecd1c7dde5eb636d6bb647e532e1\",\"wss://relay.example.com\"],[\"k\",\"30023\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"wss://relay.example.com\"]]",
  "[[\"A\",\"30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-storage\",\"wss://relay.example.com\"],[\"K\",\"30023\"],[\"P\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"wss://relay.example.com\"],[\"e\",\"78f44c6caec878080ccaca6ce225181b5a7d56d72b18a5e9aed90240a0c835ad\",\"wss://relay.example.com\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"],[\"k\",\"1111\"],[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\",\"wss://relay.example.com\"]]"];
PY_S6 :: "[[\"e\",\"6b3c564c3417f70992cd4dc0ddd12bb5448ab1bf1e327fb11575b5db0c764321\",\"wss://relay.example.com\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"]]";
PY_S16 :: "[[\"e\",\"fdb6fe34ceb4cf646cb68bf987ca2577d3aaecd1c7dde5eb636d6bb647e532e1\",\"wss://relay.example.com\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"k\",\"30023\"],[\"a\",\"30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-storage\"]]";

// An event with this id, pubkey, time, kind, tags and content and a zero
// signature, read with its ID recomputed; the signature is not checked.
py_event :: (id: string, pub: string, at: int, kind: int, tags: string, content: string, out: [] u8, scratch: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, id);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, pub);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, at);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, kind);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, content);
  n = json_raw(out, n, ",\"sig\":\""); n = json_raw(out, n, SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}
py :: (i: int, out: [] u8, scratch: [] u8) -> NostrEvent {
  return py_event(PY_ID[i], PUB[PY_AUTHOR[i]], T0 + PY_AT[i], PY_KIND[i], PY_TAGS[i], PY_CONTENT[i], out, scratch);
}

#test "the ids are Python's, and so are the tags the example writes" {
  buf: [8][4096] u8; scratch: [4096] u8; ev: [8] NostrEvent;
  for i: 0..<8 { ev[i] = py(i, buf[i][..], scratch[..]); #assert ev[i].error == NE_OK && ns_equal32(ev[i].computed_id, ev[i].id); }
  out: [4096] u8;
  t := n18_repost_tags(ev[0], RELAY, out[..]); #assert t.status == N18_OK && t.kind == 6 && cast(string) out[0..<t.n] == PY_S6;
  t = n18_repost_tags(ev[1], RELAY, out[..]); #assert t.status == N18_OK && t.kind == 16 && cast(string) out[0..<t.n] == PY_S16;
  #assert reaction_tags(ev[0], RELAY, "", out[..]) == PY_LIKE;
  #assert reaction_tags(ev[0], RELAY, ",[\"emoji\",\"tea\",\"https://example.com/tea.png\"]", out[..]) == PY_TAGS[5];
  #assert comment_on(ev[1], RELAY, out[..]) == PY_TAGS[6];
  #assert reply_to_comment(ev[6], RELAY, out[..]) == PY_TAGS[7];
}

// The tally by hand: Bob likes (X1); Carol dislikes (X2) and later likes
// (X3), so only X3 counts; Alice's :tea: (X4) is a custom emoji. Likes 2,
// dislikes 0, others 1, one superseded. The notes and comments in the set
// are other kinds and don't count.
#test "the tally of N matches the hand count, in any order" {
  buf: [8][4096] u8; scratch: [4096] u8; ev: [8] NostrEvent;
  for i: 0..<8 { ev[i] = py(i, buf[i][..], scratch[..]); }
  t := n25_tally(ev[..], ev[0].id);
  #assert t.likes == 2 && t.dislikes == 0 && t.others == 1 && t.superseded == 1 && t.refused == 0;
  rev: [8] NostrEvent; for k: 0..<8 { rev[k] = ev[7 - k]; }
  t = n25_tally(rev[..], ev[0].id);
  #assert t.likes == 2 && t.dislikes == 0 && t.others == 1 && t.superseded == 1 && t.refused == 0;
  // Without X3, Carol's dislike is her latest.
  less: [7] NostrEvent; k := 0; for i: 0..<8 { if i != 4 { less[k] = ev[i]; k += 1; } }
  t = n25_tally(less[..], ev[0].id);
  #assert t.likes == 1 && t.dislikes == 1 && t.others == 1 && t.superseded == 0;
  // No reactions name R.
  t = n25_tally(ev[..], ev[1].id); #assert t.likes == 0 && t.dislikes == 0 && t.others == 0;
}

// NIP-18: a kind 6 holding Python's N reads N back with the id it names.
#test "a repost of Python's N reads N back" {
  nb: [4096] u8; scratch: [4096] u8; n := py(0, nb[..], scratch[..]);
  b: [8192] u8; inner: [4096] u8; s2: [4096] u8;
  r := n18_repost(unsigned(6, PY_S6, n.original, b[..]), inner[..], s2[..]);
  #assert r.status == N18_OK && r.embedded && ns_equal32(r.target, n.id) && cast(string) r.inner.original == n.original;
}

// NIP-22's six examples, tags as the NIP prints them. Each reads, with the
// root and parent the NIP's comments describe. The blog comment's parent has
// both a and e; the reader takes e, the first of e, a and i.
NIP22_BLOG :: "[[\"A\",\"30023:3c9849383bdea883b0bd16fece1ed36d37e37cdde3ce43b17ea4e9192ec11289:f9347ca7\",\"wss://example.relay\"],[\"K\",\"30023\"],[\"P\",\"3c9849383bdea883b0bd16fece1ed36d37e37cdde3ce43b17ea4e9192ec11289\",\"wss://example.relay\"],[\"a\",\"30023:3c9849383bdea883b0bd16fece1ed36d37e37cdde3ce43b17ea4e9192ec11289:f9347ca7\",\"wss://example.relay\"],[\"e\",\"5b4fc7fed15672fefe65d2426f67197b71ccc82aa0cc8a9e94f683eb78e07651\",\"wss://example.relay\"],[\"k\",\"30023\"],[\"p\",\"3c9849383bdea883b0bd16fece1ed36d37e37cdde3ce43b17ea4e9192ec11289\",\"wss://example.relay\"]]";
NIP22_FILE :: "[[\"E\",\"768ac8720cdeb59227cf95e98b66560ef03d8bc9a90d721779e76e68fb42f5e6\",\"wss://example.relay\",\"3721e07b079525289877c366ccab47112bdff3d1b44758ca333feb2dbbbbe5bb\"],[\"K\",\"1063\"],[\"P\",\"3721e07b079525289877c366ccab47112bdff3d1b44758ca333feb2dbbbbe5bb\"],[\"e\",\"768ac8720cdeb59227cf95e98b66560ef03d8bc9a90d721779e76e68fb42f5e6\",\"wss://example.relay\",\"3721e07b079525289877c366ccab47112bdff3d1b44758ca333feb2dbbbbe5bb\"],[\"k\",\"1063\"],[\"p\",\"3721e07b079525289877c366ccab47112bdff3d1b44758ca333feb2dbbbbe5bb\"]]";
NIP22_REPLY :: "[[\"E\",\"768ac8720cdeb59227cf95e98b66560ef03d8bc9a90d721779e76e68fb42f5e6\",\"wss://example.relay\",\"fd913cd6fa9edb8405750cd02a8bbe16e158b8676c0e69fdc27436cc4a54cc9a\"],[\"K\",\"1063\"],[\"P\",\"fd913cd6fa9edb8405750cd02a8bbe16e158b8676c0e69fdc27436cc4a54cc9a\"],[\"e\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\",\"wss://example.relay\",\"93ef2ebaaf9554661f33e79949007900bbc535d239a4c801c33a4d67d3e7f546\"],[\"k\",\"1111\"],[\"p\",\"93ef2ebaaf9554661f33e79949007900bbc535d239a4c801c33a4d67d3e7f546\"]]";
NIP22_WEB :: "[[\"I\",\"https://abc.com/articles/1\"],[\"K\",\"web\"],[\"i\",\"https://abc.com/articles/1\"],[\"k\",\"web\"]]";
NIP22_PODCAST :: "[[\"I\",\"podcast:item:guid:d98d189b-dc7b-45b1-8720-d4b98690f31f\",\"https://fountain.fm/episode/z1y9TMQRuqXl2awyrQxg\"],[\"K\",\"podcast:item:guid\"],[\"i\",\"podcast:item:guid:d98d189b-dc7b-45b1-8720-d4b98690f31f\",\"https://fountain.fm/episode/z1y9TMQRuqXl2awyrQxg\"],[\"k\",\"podcast:item:guid\"]]";
NIP22_PODCAST_REPLY :: "[[\"I\",\"podcast:item:guid:d98d189b-dc7b-45b1-8720-d4b98690f31f\",\"https://fountain.fm/episode/z1y9TMQRuqXl2awyrQxg\"],[\"K\",\"podcast:item:guid\"],[\"e\",\"80c48d992a38f9c445b943a9c9f1010b396676013443765750431a9004bdac05\",\"wss://example.relay\",\"252f10c83610ebca1a059c0bae8255eba2f95be4d1d7bcfa89d7248a82d9f111\"],[\"k\",\"1111\"],[\"p\",\"252f10c83610ebca1a059c0bae8255eba2f95be4d1d7bcfa89d7248a82d9f111\"]]";
#test "NIP-22's six examples read with the scopes the NIP describes" {
  tags := [6] string.[NIP22_BLOG, NIP22_FILE, NIP22_REPLY, NIP22_WEB, NIP22_PODCAST, NIP22_PODCAST_REPLY];
  root := [6] u8.['A', 'E', 'E', 'I', 'I', 'I'];
  parent := [6] u8.['e', 'e', 'e', 'i', 'i', 'e'];
  root_kind := [6] string.["30023", "1063", "1063", "web", "podcast:item:guid", "podcast:item:guid"];
  parent_kind := [6] string.["30023", "1063", "1111", "web", "podcast:item:guid", "1111"];
  for i: 0..<6 {
    e := n34_event(tags[i], 1111); c := n22_comment(e);
    #assert c.status == N34_OK && c.root.at == root[i] && c.parent.at == parent[i];
    #assert nt_raw(e, c.root.kind.v[0]) == root_kind[i] && nt_raw(e, c.parent.kind.v[0]) == parent_kind[i];
  }
}

// NIP-25's examples: a star for a website, a like for a podcast episode (the
// first i is the target), and :soapbox: with its emoji tag. The NIP's emoji
// example prints only the emoji tag; an e tag is added because NIP-25 says
// there must always be one.
#test "NIP-25's examples read as the NIP says" {
  b: [4096] u8;
  r := n25_reaction(unsigned(17, "[[\"k\",\"web\"],[\"i\",\"https://example.com\"]]", "⭐", b[..]));
  #assert r.status == N25_OK && r.value == N25_TEXT && cast(string) r.external[0..<r.external_n] == "https://example.com";
  r = n25_reaction(unsigned(17, "[[\"k\",\"podcast:guid\"],[\"i\",\"podcast:guid:917393e3-1b1e-5cef-ace4-edaa54e1f810\",\"https://fountain.fm/show/QRT0l2EfrKXNGDlRrmjL\"],[\"k\",\"podcast:item:guid\"],[\"i\",\"podcast:item:guid:PC20-229\",\"https://fountain.fm/episode/DQqBg5sD3qFGMCZoSuLF\"]]", "+", b[..]));
  #assert r.status == N25_OK && r.value == N25_LIKE && cast(string) r.external[0..<r.external_n] == "podcast:guid:917393e3-1b1e-5cef-ace4-edaa54e1f810";
  r = n25_reaction(unsigned(7, "[[\"e\",\"6b3c564c3417f70992cd4dc0ddd12bb5448ab1bf1e327fb11575b5db0c764321\"],[\"emoji\",\"soapbox\",\"https://gleasonator.com/emoji/Gleasonator/soapbox.png\"]]", ":soapbox:", b[..]));
  #assert r.status == N25_OK && r.value == N25_CUSTOM;
  // "+" or empty is a like, "-" a dislike, anything else neither.
  values := [5] string.["+", "", "-", "❤", ":-)"];
  want := [5] int.[N25_LIKE, N25_LIKE, N25_DISLIKE, N25_TEXT, N25_TEXT];
  for i: 0..<5 { #assert n25_reaction(unsigned(7, PY_LIKE, values[i], b[..])).value == want[i]; }
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/responses.coax --run-steps 250000000
```

`coax test` runs 401 checks, including the loaded files' own checks; all
pass.

### The library's own checks

`nip18.coax`, `nip25.coax` and `nip22.coax` have their own `#test` blocks:
each reading and refusal, the first and last tag rules, the embedded event's
id, kind, author and address, custom emoji at their edges, kind 17, values
over the bound, forged content spans, the tally's ties, duplicates and
other targets, and for `nip22.coax` the new kind rule beside the existing
scope checks.

```shell
coax/bin/coax test coax/lib/nostr/nip18.coax
coax/bin/coax test coax/lib/nostr/nip25.coax
coax/bin/coax test coax/lib/nostr/nip22.coax
coax/bin/coax mutate coax/lib/nostr/nip18.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip25.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip22.coax --root-only
```

On 2026-11-01, `coax test` passed all 211, 217 and 49 checks (with the
loaded files'). `coax mutate --root-only` killed all 182 mutants of
`nip18.coax`, all 235 of `nip25.coax` and all 136 of `nip22.coax`, with none
surviving and none marked equivalent. The checks of `nip34-event.coax`,
`project-fold.coax` and `nip-guide-observe.coax`, which load `nip22.coax`,
also pass. The full mutation run (`COAX_MUTATION_FULL=1`) was not run.

## Summary

- `n18_repost` reads a kind 6 or 16 repost's target and checks the embedded
  event against its `e`, `p`, `k` and `a` tags, with its id recomputed.
  `n18_repost_tags` writes a repost's tags and picks kind 6 or 16.
- `n25_reaction` reads a kind 7 reaction's value and target (the last `e`
  tag), or a kind 17 reaction's external content, and refuses a custom emoji
  without its emoji tag.
- `n25_tally` counts one reaction per author and target, the latest, with
  ties to the lower id.
- `n22_comment` reads a comment's root and parent scopes and refuses with
  `N22_KINDS` when the parent's kind is neither 1111 nor the root's.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [50. Notes, threads and subjects](ch050-notes-threads-and-subjects.md)

## Sources

- [NIP-18](../nips/official/18.md)
- [NIP-25](../nips/official/25.md)
- [NIP-22](../nips/official/22.md)
- [NIP-30](../nips/official/30.md)
- [NIP-73](../nips/official/73.md)
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
