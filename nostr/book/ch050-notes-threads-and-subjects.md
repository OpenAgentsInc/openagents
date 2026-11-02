# 50. Notes, threads and subjects

Text notes and replies are the most common events. A reply says which note
it answers with `e` tags and who should see it with `p` tags. This chapter
reads those tags into a thread, writes the tags of a new reply, and puts a
set of notes back into a tree.

Part VI, Every NIP. Result: Kind 1 notes, NIP-10 marked `e` tags (`root`,
`reply`, `mention`) with the deprecated positional form, `p` tags for
participants, NIP-14 `subject` and NIP-36 `content-warning`.

## What this chapter teaches

- Marked `e` tags: `["e", id, relay, "root"|"reply", pubkey]`.
- Positional `e` tags, deprecated, read for old events only.
- Reply `p` tags: the parent's author plus the parent's `p` tags.
- NIP-14 `subject` and NIP-36 `["content-warning", reason]`.
- Rebuilding a thread's tree from a set of notes that arrive in any order.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag(e, i)` | Reads tag `i`: an `e` tag's id, relay, NIP-10 marker and author (chapter 22) |
| [nip10.coax](../../coax/lib/nostr/nip10.coax) | `n10_thread(e)` | The root, the parent and the mentions of a kind 1 note |
| [nip10.coax](../../coax/lib/nostr/nip10.coax) | `n10_subject(e, out)`, `n10_warning(e, out)` | The first `subject` or `content-warning` tag's value |
| [nip10.coax](../../coax/lib/nostr/nip10.coax) | `n10_reply_tags(parent, relay, out)` | The tags of a reply to `parent` |
| [nip10.coax](../../coax/lib/nostr/nip10.coax) | `n10_parents(set, parent)` | Each note's parent as an index into the set |

`nip10.coax` is new with this chapter. It builds on `nostr_tag` and changes
no existing code. Like chapter 22's tags, a thread is a reading of the
signed tags: `n10_thread` fetches nothing and verifies no signature, so a
root or parent id is what the note claims to answer.

## Marked `e` tags

NIP-10 gives an `e` tag up to five values:

```jai
["e", <event-id>, <relay-url>, <marker>, <pubkey>]
```

The marker is `"root"` or `"reply"`. The root is the first note of the
thread. The reply marker names the parent, the note this one answers
directly. A direct reply to the root has a single `e` tag, marked `"root"`.
The pubkey is the referenced note's author, so a client can look for the
note on that author's relays (chapter 32).

Earlier versions of NIP-10 also had a `"mention"` marker. `nostr_tag` still
reads it, and `n10_thread` reads such a tag as a mention. Any other marker,
such as `"quote"`, is not lowercase hex either, so `nostr_tag` refuses the
tag with `NT_HEX` (chapter 22).

`n10_thread` applies these rules when any `e` tag carries `"root"` or
`"reply"`:

1. The root is the one tag marked `"root"`. A second one refuses with
   `N10_ROOTS`.
2. The parent is the one tag marked `"reply"`. A second one refuses with
   `N10_REPLIES`. Without one, the parent is the root.
3. A `"reply"` tag with no `"root"` tag names the parent and is read as the
   root too. NIP-10 says a root should be present; this reads the event
   rather than refusing it.
4. Every other `e` tag is a mention.

The tag order does not matter for marked tags. A note whose `"reply"` tag
comes before its `"root"` tag reads the same.

### Worked example: a reply to a reply

Note D answers note C, which answers B, which answers A. D's tags, in
order:

| Tag | Values |
| --- | --- |
| 0 | `["e", A, "wss://relay.example.com", "root", alice]` |
| 1 | `["e", C, "wss://relay.example.com", "reply", carol]` |
| 2 | `["p", carol]` |
| 3 | `["p", bob]` |
| 4 | `["p", alice]` |
| 5 | `["subject", "Re: Tea"]` |

1. Tag 0 is an `e` tag marked `"root"`, the first such: the root is A, at
   tag 0.
2. Tag 1 is an `e` tag marked `"reply"`, the first such: the parent is C,
   at tag 1.
3. Tags 2 to 5 are not `e` tags.
4. No other `e` tag is left, so D has no mentions.

D's thread: root A, parent C. The scheme is marked.

## Positional `e` tags

Before markers, the position of each unmarked `e` tag gave its meaning.
NIP-10 keeps the rule for old events only:

- No `e` tag: not a reply.
- One `e` tag: the note it answers.
- Two: the root, then the parent.
- More: the root first, the parent last, and mentions between.

`n10_thread` uses this rule only when no `e` tag is marked `"root"` or
`"reply"`. It counts the unmarked `e` tags; a tag marked `"mention"` stays a
mention and takes no position. With one unmarked tag, that note is both the
parent and the root: nothing else in the tags names a root.

### Worked example: D in the positional form

An old client writes D's reply as three unmarked tags: `["e", A]`,
`["e", B]`, `["e", C]`.

1. Three unmarked `e` tags, so the many-tag case applies.
2. The first, A at tag 0, is the root.
3. The last, C at tag 2, is the parent.
4. B, at tag 1, is between them: a mention.

The result is root A and parent C, as with the markers. But B is in the
chain, and the positional rule calls it a mention. The rule cannot tell a
note in the chain from a note that is only cited, which is why NIP-10
deprecates it. With two tags, `["e", A]` and `["e", C]`, the result is root
A, parent C and no mentions.

## The `p` tags of a reply

NIP-10: when you reply to a note E, the reply's `p` tags hold E's author and
all of E's `p` tags. Its example: a note by `a1` with `p` tags `p1`, `p2`,
`p3` gets a reply with `p` tags `a1`, `p1`, `p2`, `p3`.

`n10_reply_tags(parent, relay, out)` writes the parent's author first and
then each of the parent's `p` tags in order, each pubkey once. For D:

1. C's author is Carol: `["p", carol]`.
2. C's `p` tags are Bob, then Alice: `["p", bob]`, `["p", alice]`.

It writes up to `N10_MAX_P` (64) distinct pubkeys and refuses with
`N10_LIMIT` past that. It does not remove the replier's own pubkey: Alice
writes D, and Alice is in D's `p` tags because she wrote A.

The `e` tags follow the marked form. When the parent is a root, the reply
gets one tag, the parent marked `"root"` with its author. When the parent is
a reply, the reply gets the parent's root tag (its relay and author as the
parent wrote them, marked `"root"`), then the parent marked `"reply"` with
its author. `relay` is where you saw the parent. It is `""` or a `ws://` or
`wss://` URL; anything else refuses with `N10_TAG`.

## Subjects and content warnings

NIP-14 adds a `subject` tag to kind 1 notes, like an email subject. A reply
should copy it and may put `"Re:"` in front. `n10_reply_tags` copies it and
adds `"Re: "` unless the subject already starts with `"Re:"`, so a long
thread keeps one `"Re: "`:

| Parent's subject | Reply's subject |
| --- | --- |
| `Tea` | `Re: Tea` |
| `Re: Tea` | `Re: Tea` |
| `Re` | `Re: Re` |

NIP-14 says subjects should generally be shorter than 80 characters. That is
advice to clients, not a refusal. `n10_subject` returns any value up to
`NT_MAX_VALUE` (256) bytes, decoded from its JSON escapes, and refuses a
longer one with `N10_LIMIT`.

NIP-36's `["content-warning", reason]` asks a client to hide the content
until the reader chooses to see it. The reason is optional. `n10_warning`
reports whether the tag is present and its reason, empty when the tag has no
value. A reply does not inherit a content warning; its author adds one if
the reply needs it.

Both read the first tag with that name and its first value:

<!-- source: coax/lib/nostr/nip10.coax text -->
```jai
// The first value of the first tag with this name, unescaped into out.
// Absent reads as not present; a tag with no value as present and empty.
n10_text :: (e:NostrEvent,name:string,out:[]u8)->N10Text {
  r:N10Text;j:[1]Json;if !nt_start(j[..],e) {return r;}
  tok:=json_next(j[..]);
  while tok==JSON_ARRAY {
    v:=json_next(j[..]);
    if v!=JSON_STRING {return r;}
    if json_is(json_text(j[..]),name) {
      r.present=true;v=json_next(j[..]);
      if v==JSON_STRING {
        r.n=json_unescape(json_text(j[..]),out);
        if r.n<0 {r.n=0;r.status=N10_LIMIT;return r;}
      } else if v!=JSON_ARRAY_END {return r;}
      r.status=N10_OK;return r;
    }
    // v is the name's string; read past the values to the tag's end.
    while v==JSON_STRING {v=json_next(j[..]);}
    if v!=JSON_ARRAY_END {return r;}
    tok=json_next(j[..]);
  }
  if tok==JSON_ARRAY_END {r.status=N10_OK;}
  return r;
}
n10_subject :: (e:NostrEvent,out:[]u8)->N10Text {return n10_text(e,"subject",out);}
n10_warning :: (e:NostrEvent,out:[]u8)->N10Text {return n10_text(e,"content-warning",out);}
```

## Reading a thread

`n10_thread` returns this record:

<!-- source: coax/lib/nostr/nip10.coax record -->
```jai
N10Thread :: struct {
  status:int=N10_EVENT; tag_error:int; at:int=-1;  // the refusing tag
  scheme:int;                                      // none, marked, positional
  is_reply:bool;
  root_at:int=-1; parent_at:int=-1;                // tag indexes
  root:[32]u8; parent:[32]u8;                      // their ids
  mentions:int; mention_at:[N10_MAX_MENTIONS]int;
}
```

It reads only kind 1 notes; any other kind refuses with `N10_KIND`, because
NIP-10 says kind 1 replies must not answer other kinds (NIP-22 comments do,
chapter 52). Every tag goes through `nostr_tag` first, so a malformed `e`,
`p`, `a` or `d` tag refuses the whole reading with `N10_TAG`; `tag_error`
holds chapter 22's status and `at` the tag's index. An `e` tag with the
note's own id refuses with `N10_SELF`. More than `N10_MAX_MENTIONS` (16)
mentions refuse with `N10_LIMIT`.

<!-- source: coax/lib/nostr/nip10.coax thread -->
```jai
// The thread a kind 1 note says it belongs to. Pass 1 reads every tag
// (a broken standard tag refuses), finds the markers and counts the
// unmarked e tags; pass 2 places each e tag by the marked or the
// positional rule.
n10_thread :: (e:NostrEvent)->N10Thread {
  th:N10Thread;
  if e.error!=NE_OK {return th;}
  if e.kind!=1 {return n10_refuse(th,N10_KIND,-1);}
  plain:=0;i:=0;
  while true {
    t:=nostr_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error!=NT_OK {th.tag_error=t.error;return n10_refuse(th,N10_TAG,i);}
    if t.type==NT_E {
      if ns_equal32(t.id,e.id) {return n10_refuse(th,N10_SELF,i);}
      if t.marker==NT_MARK_ROOT {if th.root_at>=0 {return n10_refuse(th,N10_ROOTS,i);}th.root_at=i;}
      if t.marker==NT_MARK_REPLY {if th.parent_at>=0 {return n10_refuse(th,N10_REPLIES,i);}th.parent_at=i;}
      if t.marker==NT_MARK_NONE {plain+=1;}
    }
    i+=1;
  }
  marked:=th.root_at>=0 || th.parent_at>=0;
  if marked {
    th.scheme=N10_MARKED;
    if th.root_at<0 {th.root_at=th.parent_at;}
    if th.parent_at<0 {th.parent_at=th.root_at;}
  } else if plain>0 {th.scheme=N10_POSITIONAL;}
  seen:=0;
  for k:0..<i {
    t:=nostr_tag(e,k);
    if t.type!=NT_E {continue;}
    if marked || t.marker==NT_MARK_MENTION {
      if k!=th.root_at && k!=th.parent_at {th=n10_mention(th,k);}
      continue;
    }
    seen+=1;
    if seen==1 {th.root_at=k;}
    if seen==plain {th.parent_at=k;}
    if seen>1 && seen<plain {th=n10_mention(th,k);}
  }
  if th.mentions>N10_MAX_MENTIONS {return n10_refuse(th,N10_LIMIT,-1);}
  th.is_reply=th.parent_at>=0;
  if th.is_reply {th.root=nostr_tag(e,th.root_at).id;th.parent=nostr_tag(e,th.parent_at).id;}
  th.status=N10_OK;return th;
}
```

## Writing a reply

<!-- source: coax/lib/nostr/nip10.coax reply -->
```jai
// The tags of a reply to parent: a root e tag, a reply e tag when the
// parent is itself a reply, the parent's author and then its p tags each
// once, and its subject with "Re: " in front unless it starts with "Re:".
// relay is where the parent was seen, or "".
n10_reply_tags :: (parent:NostrEvent,relay:string,out:[]u8)->N10Reply {
  r:N10Reply;th:=n10_thread(parent);
  if th.status!=N10_OK {r.status=th.status;return r;}
  if relay.count>0 && !nt_relay_ok(relay) {r.status=N10_TAG;return r;}
  n:=json_raw(out,0,"[");
  if th.is_reply {
    t:=nostr_tag(parent,th.root_at);
    n=n10_e(out,n,t.id,cast(string)t.relay[0..<t.relay_n],"root",t.has_author,t.author);
    n=json_raw(out,n,",");n=n10_e(out,n,parent.id,relay,"reply",true,parent.public_key);
  } else {n=n10_e(out,n,parent.id,relay,"root",true,parent.public_key);}
  p:[N10_MAX_P][32]u8;p[0]=parent.public_key;count:=1;
  k:=0;
  while true {
    t:=nostr_tag(parent,k);k+=1;
    if t.error!=NT_OK {break;}
    if t.type!=NT_P {continue;}
    fresh:=true;for q:0..<count {if ns_equal32(p[q],t.pubkey) {fresh=false;}}
    if !fresh {continue;}
    if count==N10_MAX_P {r.status=N10_LIMIT;return r;}
    p[count]=t.pubkey;count+=1;
  }
  for q:0..<count {n=json_raw(out,n,",[\"p\",");n=n10_hex(out,n,p[q]);n=json_raw(out,n,"]");}
  subject:[NT_MAX_VALUE]u8;s:=n10_subject(parent,subject[..]);
  if s.status!=N10_OK {r.status=s.status;return r;}
  if s.present {
    text:=cast(string)subject[0..<s.n];
    re:[NT_MAX_VALUE+4]u8;m:=0;
    if text.count<3 || text[0..<3]!="Re:" {m=json_raw(re[..],0,"Re: ");}
    m=json_raw(re[..],m,text);
    n=json_raw(out,n,",[\"subject\",");n=json_write_string(out,n,cast(string)re[0..<m]);n=json_raw(out,n,"]");
  }
  n=json_raw(out,n,"]");
  if n== -1 {r.status=N10_OUTPUT;return r;}
  r.n=n;r.status=N10_OK;return r;
}
```

The parent must read as a thread first, so a parent that `n10_thread`
refuses gives the same refusal here. A buffer too small for the tags refuses
with `N10_OUTPUT`.

## Rebuilding the tree

Relays return notes in any order. `n10_parents` gives each note's parent as
an index into the same set. A note can name a parent that is not in the set,
because no relay sent it or it was deleted; that note gets `N10_MISSING`.

<!-- source: coax/lib/nostr/nip10.coax parents -->
```jai
// Each note's parent as an index into the set: N10_TOP when it answers
// nothing, N10_MISSING when its parent isn't in the set, N10_BROKEN when
// its thread tags refuse.
n10_parents :: (set:[]NostrEvent,parent:[]int) {
  for i:0..<set.count {
    th:=n10_thread(set[i]);
    parent[i]=ifx th.status!=N10_OK then N10_BROKEN else ifx th.is_reply then N10_MISSING else N10_TOP;
    if !th.is_reply {continue;}
    for k:0..<set.count {if ns_equal32(set[k].id,th.parent) {parent[i]=k;}}
  }
}
```

### Worked example: the tree from D, P, A, C, B

Take five notes: A, B, C and D from the first worked example, and P, an old
client's reply to B with the positional tags `["e", A]`, `["e", B]`. Give
them to `n10_parents` in the order D, P, A, C, B, so `set[0]` is D and
`set[4]` is B.

| Index | Note | Parent by its tags | Index of the parent |
| --- | --- | --- | --- |
| 0 | D | C (marked `"reply"`) | 3 |
| 1 | P | B (last of two unmarked tags) | 4 |
| 2 | A | none | `N10_TOP` |
| 3 | C | B (marked `"reply"`) | 4 |
| 4 | B | A (marked `"root"`, no `"reply"`) | 2 |

The only `N10_TOP` is A, at index 2. B points to A, C and P point to B,
and D points to C:

```text
A
  B
    P
    C
      D
```

D is three replies deep.

## The example

The example is [nostr/examples/threads.coax](../examples/threads.coax). It
signs the thread with the BIP-340 test vector keys 0, 1 and 2 as Alice, Bob
and Carol. B, C and D take their tags from `n10_reply_tags`; C adds a
content warning; P has positional tags. It then reads each note back,
rebuilds the tree from the shuffled set, reads D's thread in the positional
form, and shows the refusals. The refusals use unsigned events, because no
signer here would write them.

<!-- source: nostr/examples/threads.coax example -->
```jai
#load "../../coax/lib/nostr/nip10.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

STATUS :: [9] string.["ok", "event", "kind", "tag", "two roots", "two replies", "itself", "limit", "output"];
TAG_STATUS :: [8] string.["ok", "event", "index", "shape", "hex", "relay", "address", "limit"];
SCHEME :: [3] string.["none", "marked", "positional"];

// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}

// The notes of the thread, by letter, so the output names ids by letter.
Note :: struct { letter: string; event: NostrEvent; }
NOTES :: 5;

// Print an id as its note's letter, or as its first 4 bytes in hex.
show_id :: (notes: [] Note, id: [32] u8) {
  for n: notes { if n.event.error == NE_OK && ns_equal32(n.event.id, id) { print("%", n.letter); return; } }
  text: [8] u8; own := id; r := hex_encode_checked(cast(string) own[0..<4], text[..], false);
  print("%...", cast(string) text[..]);
}
// Unmarked e tags, one per note, in this order: the deprecated positional form.
positional :: (notes: [] Note, which: [] int, out: [] u8) -> string {
  n := json_raw(out, 0, "[");
  for w: 0..<which.count {
    hex: [64] u8; id := notes[which[w]].event.id;
    r := hex_encode_checked(cast(string) id[..], hex[..], false);
    if w > 0 { n = json_raw(out, n, ","); }
    n = json_raw(out, n, "[\"e\",\"");
    n = json_raw(out, n, cast(string) hex[..]);
    n = json_raw(out, n, "\"]");
  }
  n = json_raw(out, n, "]");
  #assert n > 0;
  return cast(string) out[0..<n];
}
// Print the tree under set[at], each child indented one step more.
show_tree :: (notes: [] Note, order: [] int, parent: [] int, at: int, depth: int) {
  for d: 0..<depth { print("  "); }
  print("%\n", notes[order[at]].letter);
  for k: 0..<parent.count { if parent[k] == at { show_tree(notes, order, parent, k, depth + 1); } }
}

// Sign a kind 1 note into out and read it back with its ID recomputed.
sign_note :: (key: [] u8, created_at: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  aux: [32] u8; work: [1] NostrSignWork; scratch: [4096] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key, aux[..], created_at, 1, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [4096] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified;
  return e;
}

// Print the thread n10_thread reads from one note.
show_thread :: (notes: [] Note, label: string, e: NostrEvent) {
  th := n10_thread(e);
  print("%: %", label, STATUS[th.status]);
  if th.status == N10_TAG { print(", tag % is %\n", th.at, TAG_STATUS[th.tag_error]); return; }
  if th.status != N10_OK { if th.at >= 0 { print(" at tag %", th.at); } print("\n"); return; }
  print(", %", SCHEME[th.scheme]);
  if th.is_reply {
    print(", root "); show_id(notes, th.root); print(" (tag %), parent ", th.root_at);
    show_id(notes, th.parent); print(" (tag %)", th.parent_at);
  } else { print(", not a reply"); }
  for m: 0..<th.mentions {
    print(", mention "); show_id(notes, nostr_tag(e, th.mention_at[m]).id); print(" (tag %)", th.mention_at[m]);
  }
  print("\n");
}
// Print a note's subject and content warning.
show_text :: (label: string, e: NostrEvent) {
  out: [NT_MAX_VALUE] u8;
  s := n10_subject(e, out[..]);
  print("%: subject %", label, ifx s.present then "present" else "absent");
  if s.present { print(" \"%\"", cast(string) out[0..<s.n]); }
  w := n10_warning(e, out[..]);
  print(", content-warning %", ifx w.present then "present" else "absent");
  if w.present { print(" \"%\"", cast(string) out[0..<w.n]); }
  print("\n");
}
// The tags of a reply to parent, written into out; it must succeed.
reply_tags :: (parent: NostrEvent, relay: string, out: [] u8) -> string {
  r := n10_reply_tags(parent, relay, out);
  #assert r.status == N10_OK;
  return cast(string) out[0..<r.n];
}
// The same tags with more appended before the closing bracket.
with_tag :: (tags: string, more: string, out: [] u8) -> string {
  n := json_raw(out, 0, tags[0..<tags.count - 1]);
  n = json_raw(out, n, ",");
  n = json_raw(out, n, more);
  n = json_raw(out, n, "]");
  #assert n > 0;
  return cast(string) out[0..<n];
}

// An unsigned note with these tags, parsed but not verified: its id and sig
// are zeros. Used for the refusals, which no signer would produce.
EVENT_HEAD :: "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"created_at\":1761686400,\"kind\":";
EVENT_TAIL :: ",\"content\":\"\",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}";
unsigned :: (kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, EVENT_HEAD);
  n = json_write_int(out, n, kind);
  n = json_raw(out, n, ",\"tags\":");
  n = json_raw(out, n, tags);
  n = json_raw(out, n, EVENT_TAIL);
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}
refuse :: (notes: [] Note, label: string, kind: int, tags: string) {
  buf: [2048] u8; show_thread(notes[..], label, unsigned(kind, tags, buf[..]));
}

// BIP-340 test vectors 0, 1 and 2's published secret keys. Never sign with
// a real key in an example.
ALICE :: "0000000000000000000000000000000000000000000000000000000000000003";
BOB :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
CAROL :: "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9";
RELAY :: "wss://relay.example.com";
T0 :: 1761686400;

main :: () {
  alice: [32] u8; bob: [32] u8; carol: [32] u8;
  decode_hex(ALICE, alice[..]); decode_hex(BOB, bob[..]); decode_hex(CAROL, carol[..]);
  notes: [NOTES] Note;
  buf: [NOTES][4096] u8; tags: [4096] u8; more: [4096] u8;

  print("a thread, three replies deep\n");
  notes[0].letter = "A"; notes[0].event = sign_note(alice[..], T0, "[[\"subject\",\"Tea\"]]", "Which tea keeps best?", buf[0][..]);
  t := reply_tags(notes[0].event, RELAY, tags[..]);
  print("B's tags: %\n", t);
  notes[1].letter = "B"; notes[1].event = sign_note(bob[..], T0 + 60, t, "Oolong, sealed.", buf[1][..]);
  t = reply_tags(notes[1].event, RELAY, tags[..]);
  t = with_tag(t, "[\"content-warning\",\"spoilers\"]", more[..]);
  print("C's tags: %\n", t);
  notes[2].letter = "C"; notes[2].event = sign_note(carol[..], T0 + 120, t, "It goes flat after a year.", buf[2][..]);
  t = reply_tags(notes[2].event, RELAY, tags[..]);
  print("D's tags: %\n", t);
  notes[3].letter = "D"; notes[3].event = sign_note(alice[..], T0 + 180, t, "Even sealed?", buf[3][..]);
  // An old client's reply to B: unmarked e tags, root then parent.
  ab := [2] int.[0, 1];
  t = positional(notes[..], ab[..], tags[..]);
  print("P's tags: e A, e B, unmarked\n");
  notes[4].letter = "P"; notes[4].event = sign_note(carol[..], T0 + 240, t, "Green tea fades fastest.", buf[4][..]);

  print("each note's id\n");
  for note: notes {
    hex: [64] u8; id := note.event.id;
    r := hex_encode_checked(cast(string) id[..], hex[..], false);
    print("%: %, verified %\n", note.letter, cast(string) hex[..], note.event.verified);
  }

  print("read back\n");
  for note: notes { show_thread(notes[..], note.letter, note.event); }
  for note: notes { show_text(note.letter, note.event); }

  print("the tree from the set, given out of order\n");
  order := [NOTES] int.[3, 4, 0, 2, 1];
  set: [NOTES] NostrEvent;
  for k: 0..<NOTES { set[k] = notes[order[k]].event; }
  parent: [NOTES] int;
  n10_parents(set[..], parent[..]);
  for k: 0..<NOTES {
    print("set[%] = %, parent ", k, notes[order[k]].letter);
    if parent[k] >= 0 { print("set[%] = %\n", parent[k], notes[order[parent[k]]].letter); }
    else { print("none\n"); }
  }
  for k: 0..<NOTES { if parent[k] == N10_TOP { show_tree(notes[..], order[..], parent[..], k, 0); } }

  print("D's thread in the positional form\n");
  old: [2048] u8;
  abc := [3] int.[0, 1, 2]; ac := [2] int.[0, 2];
  t = positional(notes[..], abc[..], tags[..]);
  show_thread(notes[..], "e A, e B, e C", unsigned(1, t, old[..]));
  t = positional(notes[..], ac[..], tags[..]);
  show_thread(notes[..], "e A, e C", unsigned(1, t, old[..]));

  print("refusals\n");
  refuse(notes[..], "two root markers", 1, "[[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"\",\"root\"],[\"e\",\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",\"\",\"root\"]]");
  refuse(notes[..], "two reply markers", 1, "[[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"\",\"reply\"],[\"e\",\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",\"\",\"reply\"]]");
  refuse(notes[..], "e id of 4 digits", 1, "[[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"\",\"root\"],[\"e\",\"bbbb\",\"\",\"reply\"]]");
  refuse(notes[..], "e marker quote", 1, "[[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"\",\"quote\"]]");
  refuse(notes[..], "e relay https://x", 1, "[[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"https://x\",\"root\"]]");
  refuse(notes[..], "kind 7", 7, "[[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"\",\"root\"]]");
  r := n10_reply_tags(notes[0].event, "https://relay.example.com", tags[..]);
  print("reply with relay https://relay.example.com: %\n", STATUS[r.status]);
  r = n10_reply_tags(notes[0].event, RELAY, tags[0..<100]);
  print("reply tags into 100 bytes: %\n", STATUS[r.status]);
  print("accepted\n");
  refuse(notes[..], "reply marker alone", 1, "[[\"e\",\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",\"\",\"reply\"]]");
  refuse(notes[..], "mention marker alone", 1, "[[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"\",\"mention\"]]");
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/threads.coax --run-steps 250000000
```

The output:

```text
a thread, three replies deep
B's tags: [["e","fed66d2f7eecb63f8b8d24c17e0a8f02288aa7f1e293f90d215f6056beed9eaf","wss://relay.example.com","root","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["subject","Re: Tea"]]
C's tags: [["e","fed66d2f7eecb63f8b8d24c17e0a8f02288aa7f1e293f90d215f6056beed9eaf","wss://relay.example.com","root","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["e","eb003b0398251e7fea6283eded00e61929df01e834355c0526d810a434b68878","wss://relay.example.com","reply","dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"],["p","dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["subject","Re: Tea"],["content-warning","spoilers"]]
D's tags: [["e","fed66d2f7eecb63f8b8d24c17e0a8f02288aa7f1e293f90d215f6056beed9eaf","wss://relay.example.com","root","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["e","3142a493c70b7c8b5f291d5122b9c7fb18420cdbd80f8b38ebe7058a25ea05ec","wss://relay.example.com","reply","dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"],["p","dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"],["p","dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["subject","Re: Tea"]]
P's tags: e A, e B, unmarked
each note's id
A: fed66d2f7eecb63f8b8d24c17e0a8f02288aa7f1e293f90d215f6056beed9eaf, verified true
B: eb003b0398251e7fea6283eded00e61929df01e834355c0526d810a434b68878, verified true
C: 3142a493c70b7c8b5f291d5122b9c7fb18420cdbd80f8b38ebe7058a25ea05ec, verified true
D: e5752940041c8b798e5c9551aaccc4f23a31dfda33eceb3ab39581c761bb2803, verified true
P: 2c31bdb33901ca6154bf1f51e459950b9319565b47b0ec36528d9a96eb350435, verified true
read back
A: ok, none, not a reply
B: ok, marked, root A (tag 0), parent A (tag 0)
C: ok, marked, root A (tag 0), parent B (tag 1)
D: ok, marked, root A (tag 0), parent C (tag 1)
P: ok, positional, root A (tag 0), parent B (tag 1)
A: subject present "Tea", content-warning absent
B: subject present "Re: Tea", content-warning absent
C: subject present "Re: Tea", content-warning present "spoilers"
D: subject present "Re: Tea", content-warning absent
P: subject absent, content-warning absent
the tree from the set, given out of order
set[0] = D, parent set[3] = C
set[1] = P, parent set[4] = B
set[2] = A, parent none
set[3] = C, parent set[4] = B
set[4] = B, parent set[2] = A
A
  B
    P
    C
      D
D's thread in the positional form
e A, e B, e C: ok, positional, root A (tag 0), parent C (tag 2), mention B (tag 1)
e A, e C: ok, positional, root A (tag 0), parent C (tag 1)
refusals
two root markers: two roots at tag 1
two reply markers: two replies at tag 1
e id of 4 digits: tag, tag 1 is hex
e marker quote: tag, tag 0 is hex
e relay https://x: tag, tag 0 is relay
kind 7: kind
reply with relay https://relay.example.com: tag
reply tags into 100 bytes: output
accepted
reply marker alone: ok, marked, root bbbbbbbb... (tag 0), parent bbbbbbbb... (tag 0)
mention marker alone: ok, none, not a reply, mention aaaaaaaa... (tag 0)
```

The `p` tags match the hand calculation: B gets Alice; C gets Bob, then
Alice; D gets Carol, Bob, then Alice. The pubkeys are BIP-340's published
public keys for vectors 0, 1 and 2: `f9308a01...bce036f9`,
`dff1d77f...502ba659` and `dd308afe...69774eb8`. D reads as root A and
parent C, and the tree is the one built by hand. D's positional form gives
root A, parent C and B as a mention, as the worked example says.

The refusals:

- Two `"root"` markers and two `"reply"` markers conflict, so `n10_thread`
  refuses at the second one, tag 1.
- An `e` id of 4 hex digits, a `"quote"` marker and an `https://` relay hint
  are malformed tags. `nostr_tag` refuses each, and `n10_thread` reports the
  tag's index and chapter 22's status: `hex`, `hex` and `relay`.
- A kind 7 event with a root tag is not a kind 1 note: `kind`.
- `n10_reply_tags` refuses an `https://` relay with `tag`, and a 100-byte
  buffer, too small for even a root `e` tag and a `p` tag, with `output`.

Two cases are read, not refused: a lone `"reply"` marker names the parent
and the root, and a lone `"mention"` marker is not a reply.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/threads.out](../fixtures/threads.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/threads.coax](../examples/checks/threads.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The tags and ids of A, B, C, D and P, from a Python script written from
  NIP-10 and NIP-14 that builds each reply's tags by those rules and hashes
  NIP-01's serialization with `hashlib`. The check rebuilds each note
  unsigned with Python's id, recomputes the id, and compares
  `n10_reply_tags` with Python's tags byte for byte. The ids are the ones
  the example signs and prints.
- The tree, from the set in reverse order, against the tree built by hand.
- NIP-10's four positional cases for 0 to 5 unmarked tags, written from the
  NIP's sentences.
- NIP-10's own `p` example: `a1` with `p1`, `p2`, `p3` gives `a1`, `p1`,
  `p2`, `p3`.

<!-- source: nostr/examples/checks/threads.coax check -->
```jai
// The thread's tags and ids from a Python script written from NIP-10 and
// NIP-14 (it shares no code with Coax): a reply gets the parent's root tag
// with the "root" marker, or the parent itself as root, then the parent with
// the "reply" marker; p tags for the parent's author and then its p tags,
// each once; the subject with "Re: " in front unless it starts with "Re:".
// The ids are hashlib.sha256 of json.dumps([0, pubkey, created_at, 1, tags,
// content], separators=(",", ":"), ensure_ascii=False).
PY_ID :: [5] string.[
  "fed66d2f7eecb63f8b8d24c17e0a8f02288aa7f1e293f90d215f6056beed9eaf",
  "eb003b0398251e7fea6283eded00e61929df01e834355c0526d810a434b68878",
  "3142a493c70b7c8b5f291d5122b9c7fb18420cdbd80f8b38ebe7058a25ea05ec",
  "e5752940041c8b798e5c9551aaccc4f23a31dfda33eceb3ab39581c761bb2803",
  "2c31bdb33901ca6154bf1f51e459950b9319565b47b0ec36528d9a96eb350435"];
PY_TAGS :: [5] string.[
  "[[\"subject\",\"Tea\"]]",
  "[[\"e\",\"fed66d2f7eecb63f8b8d24c17e0a8f02288aa7f1e293f90d215f6056beed9eaf\",\"wss://relay.example.com\",\"root\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"subject\",\"Re: Tea\"]]",
  "[[\"e\",\"fed66d2f7eecb63f8b8d24c17e0a8f02288aa7f1e293f90d215f6056beed9eaf\",\"wss://relay.example.com\",\"root\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"e\",\"eb003b0398251e7fea6283eded00e61929df01e834355c0526d810a434b68878\",\"wss://relay.example.com\",\"reply\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"],[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"subject\",\"Re: Tea\"],[\"content-warning\",\"spoilers\"]]",
  "[[\"e\",\"fed66d2f7eecb63f8b8d24c17e0a8f02288aa7f1e293f90d215f6056beed9eaf\",\"wss://relay.example.com\",\"root\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"e\",\"3142a493c70b7c8b5f291d5122b9c7fb18420cdbd80f8b38ebe7058a25ea05ec\",\"wss://relay.example.com\",\"reply\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"],[\"p\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"],[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"subject\",\"Re: Tea\"]]",
  "[[\"e\",\"fed66d2f7eecb63f8b8d24c17e0a8f02288aa7f1e293f90d215f6056beed9eaf\"],[\"e\",\"eb003b0398251e7fea6283eded00e61929df01e834355c0526d810a434b68878\"]]"];
// BIP-340 test vectors 0, 1 and 2's published public keys: Alice, Bob, Carol.
PUB :: [3] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
  "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"];
AUTHOR :: [5] int.[0, 1, 2, 0, 2];
CONTENT :: [5] string.["Which tea keeps best?", "Oolong, sealed.", "It goes flat after a year.", "Even sealed?", "Green tea fades fastest."];
// The tree by hand: A is the root; B answers A; C and P answer B; D answers C.
HAND_PARENT :: [5] int.[-1, 0, 1, 2, 1];

// Note i of the thread as an unsigned event carrying Python's id, read with
// its ID recomputed; the signature is not checked here.
py_note :: (i: int, out: [] u8, scratch: [] u8) -> NostrEvent {
  text: [2048] u8;
  n := json_raw(text[..], 0, "{\"id\":\"");
  n = json_raw(text[..], n, PY_ID[i]);
  n = json_raw(text[..], n, "\",\"pubkey\":\"");
  n = json_raw(text[..], n, PUB[AUTHOR[i]]);
  n = json_raw(text[..], n, "\",\"created_at\":");
  n = json_write_int(text[..], n, T0 + 60 * i);
  n = json_raw(text[..], n, ",\"kind\":1,\"tags\":");
  n = json_raw(text[..], n, PY_TAGS[i]);
  n = json_raw(text[..], n, ",\"content\":");
  n = json_write_string(text[..], n, CONTENT[i]);
  n = json_raw(text[..], n, ",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  for k: 0..<n { out[k] = text[k]; }
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}

#test "the ids are Python's and each reply's tags are Python's" {
  buf: [5][2048] u8; scratch: [4096] u8; notes: [5] NostrEvent;
  for i: 0..<5 {
    notes[i] = py_note(i, buf[i][..], scratch[..]);
    #assert notes[i].error == NE_OK && ns_equal32(notes[i].computed_id, notes[i].id);
  }
  out: [2048] u8;
  for i: 0..<3 {
    r := n10_reply_tags(notes[i], RELAY, out[..]);
    #assert r.status == N10_OK;
    // C's tags add a content warning after the reply tags.
    want := PY_TAGS[i + 1];
    if i == 1 { want = "[[\"e\",\"fed66d2f7eecb63f8b8d24c17e0a8f02288aa7f1e293f90d215f6056beed9eaf\",\"wss://relay.example.com\",\"root\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"e\",\"eb003b0398251e7fea6283eded00e61929df01e834355c0526d810a434b68878\",\"wss://relay.example.com\",\"reply\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"],[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"subject\",\"Re: Tea\"]]"; }
    #assert cast(string) out[0..<r.n] == want;
  }
  // The tree, from the set given in reverse, matches the hand-built one.
  set: [5] NostrEvent; parent: [5] int;
  for k: 0..<5 { set[k] = notes[4 - k]; }
  n10_parents(set[..], parent[..]);
  for k: 0..<5 {
    want := HAND_PARENT[4 - k];
    #assert (want < 0 && parent[k] == N10_TOP) || (want >= 0 && parent[k] == 4 - want);
  }
}

// NIP-10's positional sentences, by hand: no e tag, no reply; one, the reply;
// two, root then reply; more, root first, reply last, mentions between.
#test "positional reading follows NIP-10's four cases for 0 to 5 unmarked tags" {
  ids := "[\"e\",\"0000000000000000000000000000000000000000000000000000000000000000\"]";
  for k: 0..<6 {
    text: [1024] u8; n := json_raw(text[..], 0, "[");
    for i: 0..<k { if i > 0 { n = json_raw(text[..], n, ","); } n = json_raw(text[..], n, ids); }
    n = json_raw(text[..], n, "]");
    buf: [2048] u8; th := n10_thread(unsigned(1, cast(string) text[0..<n], buf[..]));
    // The zero id is the unsigned event's own id; give it another.
    #assert th.status == (ifx k == 0 then N10_OK else N10_SELF);
  }
  ids = "[\"e\",\"1111111111111111111111111111111111111111111111111111111111111111\"]";
  for k: 0..<6 {
    text: [1024] u8; n := json_raw(text[..], 0, "[");
    for i: 0..<k { if i > 0 { n = json_raw(text[..], n, ","); } n = json_raw(text[..], n, ids); }
    n = json_raw(text[..], n, "]");
    buf: [2048] u8; th := n10_thread(unsigned(1, cast(string) text[0..<n], buf[..]));
    #assert th.status == N10_OK && th.is_reply == (k > 0);
    if k == 0 { #assert th.scheme == N10_NONE && th.mentions == 0; }
    if k == 1 { #assert th.root_at == 0 && th.parent_at == 0 && th.mentions == 0; }
    if k >= 2 {
      #assert th.root_at == 0 && th.parent_at == k - 1 && th.mentions == k - 2;
      for m: 0..<k - 2 { #assert th.mention_at[m] == m + 1; }
    }
  }
}

// NIP-10's p example: a note by a1 with p tags p1, p2, p3; the reply's p tags
// are a1, p1, p2, p3.
#test "a reply's p tags are the parent's author and the parent's p tags" {
  e := n10_event(1, "[[\"p\",\"1111111111111111111111111111111111111111111111111111111111111111\"],[\"p\",\"2222222222222222222222222222222222222222222222222222222222222222\"],[\"p\",\"3333333333333333333333333333333333333333333333333333333333333333\"]]");
  for i: 0..<32 { e.id[i] = 0xee; e.public_key[i] = 0xa1; }
  out: [2048] u8; r := n10_reply_tags(e, "", out[..]);
  #assert r.status == N10_OK;
  reply := n10_event(1, cast(string) out[0..<r.n]);
  want := [4] u8.[0xa1, 0x11, 0x22, 0x33]; seen := 0;
  for k: 0..<8 {
    t := nostr_tag(reply, k);
    if t.error == NT_OK && t.type == NT_P { #assert t.pubkey[0] == want[seen] && t.pubkey[31] == want[seen]; seen += 1; }
  }
  #assert seen == 4;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/threads.coax --run-steps 250000000
```

`coax test` runs 358 checks, including the loaded files' own checks;
all pass.

### The library's own checks

`nip10.coax` has its own `#test` blocks: no `e` tags, the marked and
positional rules (with the reply marker first and a mention marker beside
unmarked tags), every refusal, subjects with escapes and over the bound,
the reply tags at each depth with the exact bytes, the `"Re:"` cases,
duplicate `p` tags, the 64-pubkey bound and the buffer bound, and
`n10_parents` with a missing and a broken note.

```shell
coax/bin/coax test coax/lib/nostr/nip10.coax
coax/bin/coax mutate coax/lib/nostr/nip10.coax --root-only
```

On 2026-11-01, `coax test` passed all 206 checks (with the loaded files'),
and `coax mutate --root-only` killed all 312 mutants, with none surviving
and none marked equivalent.

## Summary

- `n10_thread` reads a kind 1 note's root, parent and mentions from marked
  `e` tags, or from the deprecated positional rule when no tag is marked
  `"root"` or `"reply"`.
- Two `"root"` or two `"reply"` markers, a malformed standard tag, a
  self-reference, a kind other than 1 and more than 16 mentions refuse.
- `n10_reply_tags` writes a reply's marked `e` tags, the parent's author and
  `p` tags once each, and the subject with one `"Re: "`.
- `n10_subject` and `n10_warning` read NIP-14's subject and NIP-36's content
  warning.
- `n10_parents` rebuilds a thread's tree from notes in any order, and marks
  a note whose parent is not in the set.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Sources

- [NIP-10](../nips/official/10.md)
- [NIP-14](../nips/official/14.md)
- [NIP-36](../nips/official/36.md)
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
  test vectors 0, 1 and 2, for the example's keys.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
