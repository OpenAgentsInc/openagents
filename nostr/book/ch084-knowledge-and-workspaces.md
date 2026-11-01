# 84. Knowledge and workspaces

Agents share what they learned and edit files together. Two OpenAgents
NIPs cover those steps. NIP-KB publishes the entries of a knowledge base
as signed events: each version of an entry is a kind 3190 event, the
author's current version is pointed at by a kind 30190 head, and a kind
3191 withdrawal ends one exact version. NIP-WS binds a workspace's
documents to the host that owns them, and lets a client change a document
only on the condition that it is still the version the client read. This
chapter builds the reader that checks a KB record, the rule that decides
which version of an entry a reader shows, and the host's decision on a
conditional document change. It runs them on one entry: two people edit
its file at the same version, one edit is refused as stale and merged by
hand, and the merged file is published, withdrawn and replaced.

Part X, Every NIP. Result: NIP-KB entry versions (3190), heads (30190)
and withdrawals (3191) read and checked, equivocation reported, the
current version chosen without a withdrawn or conflicting version, and
the reader's trust rule; NIP-WS conditional text replacements under the
host's grants, with stale, conflict and idempotency refusals. Evidence,
snapshots, private entries, ranges, checkpoints, worktrees and
projections are not built yet; see [What is not built](#what-is-not-built).

## What this chapter teaches

- Entry identity: the pair (author, ID) names an entry, a version is a
  positive integer the author raises, and an event's `x` tag is the
  SHA-256 of the exact document bytes.
- How a reader checks a KB record: the signature, then the body, the tags
  and the document's front matter must all agree.
- Why a withdrawal is irreversible and why a head never brings a
  withdrawn version back.
- Equivocation: two versions with the same number and different digests.
  A reader reports both and shows neither, whatever their timestamps.
- Trust is per reader: a signature proves who wrote an entry, not that it
  is right, so another author's entry is at most `candidate`.
- A conditional change is compare-and-swap at the host: the expected
  version must be the current one, or the change refuses as `stale`.
  There is no overwrite fallback and no implicit merge.
- Membership in a room is not a grant: only a grant for this actor, this
  resource and writing lets a change through.
- Idempotency: the same command again returns the retained result;
  different bytes under the same command refuse.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [entry.coax](../../coax/lib/nostr/kb/entry.coax) | `kb_read(text, out, a)` | Verifies a KB event's id and signature, then `kb_admit` |
| [entry.coax](../../coax/lib/nostr/kb/entry.coax) | `kb_admit(e, a)` | Checks the kind, body, tags, digest, front matter and signer of a verified event |
| [current.coax](../../coax/lib/nostr/kb/current.coax) | `kb_current(records, author, id)` | Chooses the version to show: head or highest, never withdrawn or in conflict |
| [current.coax](../../coax/lib/nostr/kb/current.coax) | `kb_status(record, reader, trusted)` | The status a reader shows: the document's, or at most `candidate` |
| [change.coax](../../coax/lib/nostr/ws/change.coax) | `ws_change(binding, resources, grants, kept, n, change, now)` | The host's decision on one conditional text replacement |

All three files are new with this chapter, and each is an `#authority`
file: `entry.coax` decides which records a reader admits, `current.coax`
decides what it shows and how much it trusts it, and `change.coax` decides
whether a document changes. The checks came first, and a root-only
mutation run kills every mutant (see
[How the code is checked](#how-the-code-is-checked)). `entry.coax` reuses
chapter 21's event reader (`nostr_event_verify_vartime`, which verifies
with chapter 11's BIP-340), chapter 17's JSON parser and chapter 4's
SHA-256; the other two files load no crypto.

## Knowledge entries

An entry is one Markdown file with YAML front matter. It is reference
text: it never runs, never grants anything and never overrides an
instruction. NIP-KB carries each version in a kind 3190 event whose
content is a JSON body:

```text
{"v": 1, "requires": [], "type": "entry",
 "id": "statistics.mmd-estimators", "version": 1, "kind": "method",
 "document": "---\nid: statistics.mmd-estimators\nversion: 1\nkind: method\n...---\n## Details\n..."}
```

and whose tags index it: exactly one `d` (the ID), exactly one `x` (the
lowercase hex SHA-256 of the document bytes), the marker
`t oa:kb:entry:v1`, one `t oa:kb:kind:<kind>`, and one lowercase `t` per
topic. A head (30190) and a withdrawal (3191) have the same `v`,
`requires`, `id` and `version`, plus `entry`, the exact 3190 they bind,
and a withdrawal's `reason`. Their tags are one `d`, one `e` naming that
3190 and their own marker.

The reader returns one record for all three types. Every string is
written into the caller's arena:

<!-- source: coax/lib/nostr/kb/entry.coax record -->
```jai
KbText :: struct {buf:[]u8;at:int;}
KbRecord :: struct {
  error:u32=KB_EVENT; type:int;
  event:string; author:string; created_at:int;   // the signed event
  id:string; version:int;                        // the entry this names
  kind:int; digest:string; document:string; status:int=KS_CANDIDATE;  // an entry: x, the document
  computed:string;                               // the SHA-256 of the document, as read
  entry:string; entry_author:string; reason:string;  // a head or withdrawal: the 3190 it binds
}
```

<!-- source: coax/lib/nostr/kb/entry.coax codes -->
```jai
KB_OK :: cast(u32)0;
KB_EVENT :: cast(u32)1;      // not a NIP-01 event with its own id
KB_SIGNATURE :: cast(u32)2;
KB_KIND :: cast(u32)3;       // not 3190, 30190 or 3191
KB_SHAPE :: cast(u32)4;      // the body: a missing, repeated, unknown or mistyped key; v, requires or type
KB_FIELD :: cast(u32)5;      // a field breaks its rule: ID grammar, version, entry kind, hex, reason length
KB_TAGS :: cast(u32)6;       // the d, x, e or t tags disagree with the body or miscount
KB_DIGEST :: cast(u32)7;     // x is not the SHA-256 of the document
KB_DOCUMENT :: cast(u32)8;   // the front matter is missing or disagrees with the body
KB_SIGNER :: cast(u32)9;     // a head or withdrawal not signed by the entry's author
KB_LIMIT :: cast(u32)10;     // the caller's arena is full

KB_ENTRY :: 0; KB_HEAD :: 1; KB_WITHDRAWAL :: 2;
KB_EVENT_KINDS :: [3]int.[3190,30190,3191];
KB_TYPES :: [3]string.["entry","head","withdrawal"];
KB_MARKERS :: [3]string.["oa:kb:entry:v1","oa:kb:head:v1","oa:kb:withdrawal:v1"];
KB_KINDS :: [5]string.["method","edge-case","slip","environment","tool"];
KS_WITHDRAWN :: 0; KS_CANDIDATE :: 1; KS_ADMITTED :: 2;   // the document's status, lowest first
KB_STATUS :: [3]string.["withdrawn","candidate","admitted"];
KB_MAX_VERSION :: 9007199254740991;
KB_MAX_REASON :: 1000;   // characters (code points)
```

An entry ID starts with a lowercase letter and holds at most 128
lowercase letters, digits, `.` and `-`. It never holds `_`, so the
qualified component ID `<pubkey>:kb/<slug>`, where the slug replaces each
`.` with `_`, maps back to exactly one entry:

<!-- source: coax/lib/nostr/kb/entry.coax id -->
```jai
// ^[a-z][a-z0-9.-]{0,127}$: no '_', so the qualified slug mapping reverses.
kb_id_ok :: (s:string)->bool {
  if s.count<1 || s.count>128 || s[0]<'a' {return false;}  // digits, '.' and '-' sort below 'a'
  for c:s {if !((c>='a' && c<='z') || (c>='0' && c<='9') || c=='.' || c=='-') {return false;}}
  return true;
}
```

### The body

The body is read key by key. Each key is looked up in the list of nine
names. An unknown or repeated key refuses as `shape`, and at the end the
set of keys seen must be exactly the type's set (`KB_MASKS`): a `reason`
on an entry, a `document` on a head or a missing `version` all refuse
there. `v` must be the number `1`, `requires` the empty list, and `type`
the word the kind names:

<!-- source: coax/lib/nostr/kb/entry.coax body -->
```jai
// The fields each type has, as bits of KB_FIELDS' order.
KB_FIELDS :: [9]string.["v","requires","type","id","version","kind","document","entry","reason"];
KB_MASKS :: [3]int.[0x7f,0x9f,0x19f];
kb_body :: (body:string,t:int,r:[]KbRecord,a:[]KbText)->u32 {
  j:[1]Json;json_start(j[..],body);
  if json_next(j[..])!=JSON_OBJECT {return KB_SHAPE;}
  seen:=0;k:=json_next(j[..]);
  while k==JSON_STRING {
    f:=-1;key:=json_text(j[..]);for i:0..<9 {if key==KB_FIELDS[i] {f=i;}}
    if f<0 || (seen&(1<<f))!=0 {return KB_SHAPE;}  // a field of another type fails seen==mask below
    seen|=1<<f;v:=json_next(j[..]);
    if f==0 {if v!=JSON_NUMBER || json_text(j[..])!="1" {return KB_SHAPE;}}
    else if f==1 {if v!=JSON_ARRAY || json_next(j[..])!=JSON_ARRAY_END {return KB_SHAPE;}}
    else if f==7 {if v!=JSON_OBJECT {return KB_SHAPE;}e:=kb_ref(j[..],r,a);if e!=KB_OK {return e;}}
    else if f==4 {
      if v!=JSON_NUMBER {return KB_SHAPE;}
      n:=json_int(json_text(j[..]));if !n.ok || n.value<1 || n.value>KB_MAX_VERSION {return KB_FIELD;}
      r[0].version=n.value;
    } else {
      if v!=JSON_STRING {return KB_SHAPE;}
      s:=kb_string(j[..],a);if !s.ok {return KB_LIMIT;}
      if f==2 {if s.s!=KB_TYPES[t] {return KB_SHAPE;}}
      else if f==3 {if !kb_id_ok(s.s) {return KB_FIELD;}r[0].id=s.s;}
      else if f==5 {
        r[0].kind=-1;for i:0..<5 {if s.s==KB_KINDS[i] {r[0].kind=i;}}
        if r[0].kind<0 {return KB_FIELD;}
      }
      else if f==6 {
        r[0].document=s.s;d:[32]u8;
        if sha256_hash_checked(a[0].buf[a[0].at-s.s.count..<a[0].at],d[..],false)!=SHA_OK {return KB_LIMIT;}
        r[0].computed=kb_hex(a,d[..]);if r[0].computed=="" {return KB_LIMIT;}
      }
      else {if kb_chars(s.s)>KB_MAX_REASON {return KB_FIELD;}r[0].reason=s.s;}
    }
    k=json_next(j[..]);
  }
  if k!=JSON_OBJECT_END || seen!=KB_MASKS[t] || json_next(j[..])!=JSON_END {return KB_SHAPE;}
  return KB_OK;
}
// entry: exactly {id, pubkey, kind: 3190}.
kb_ref :: (j:[]Json,r:[]KbRecord,a:[]KbText)->u32 {
  seen:=0;k:=json_next(j);
  while k==JSON_STRING {
    key:=json_text(j);f:=ifx key=="id" then 0 else ifx key=="pubkey" then 1 else ifx key=="kind" then 2 else -1;
    if f<0 || (seen&(1<<f))!=0 {return KB_SHAPE;}
    seen|=1<<f;v:=json_next(j);
    if f==2 {if v!=JSON_NUMBER {return KB_SHAPE;}if json_text(j)!="3190" {return KB_FIELD;}}
    else {
      if v!=JSON_STRING {return KB_SHAPE;}
      s:=kb_string(j,a);if !s.ok {return KB_LIMIT;}
      if !kb_hex64(s.s) {return KB_FIELD;}
      if f==0 {r[0].entry=s.s;} else {r[0].entry_author=s.s;}
    }
    k=json_next(j);
  }
  if k!=JSON_OBJECT_END || seen!=7 {return KB_SHAPE;}
  return KB_OK;
}
```

Unknown keys matter for more than tidiness. NIP-KB says a reader computes
its own embeddings for retrieval and never accepts a vector from an
author. A body that adds an `embedding` field is refused, so a crafted
vector never reaches the reader's index.

### The tags

The tags must agree with the body. `kb_tags` counts each kind of tag and
compares each value: the `d` with the ID, the `x` (an entry's only) with
64 lowercase hex characters, which it keeps, and the `e` (a head's or
withdrawal's only) with `entry.id`. Every `t` must be lowercase. A `t`
that starts with `oa:kb:` must be the record's own marker or, on an entry,
the kind tag; any other `oa:kb:` value, such as a head marker on an entry,
refuses:

<!-- source: coax/lib/nostr/kb/entry.coax tags -->
```jai
kb_prefix :: (s:string,p:string)->bool {return s.count>=p.count && s[0..<p.count]==p;}
kb_find :: (s:string,from:int,c:u8)->int {for i:from..<s.count {if s[i]==c {return i;}}return -1;}
kb_lower :: (s:string)->bool {for c:s {if c>='A' && c<='Z' {return false;}}return true;}
kb_tags :: (tags:string,t:int,r:[]KbRecord,a:[]KbText)->u32 {
  value:[256]u8;j:[1]Json;json_start(j[..],tags);
  if json_next(j[..])!=JSON_ARRAY {return KB_TAGS;}
  ds:=0;xs:=0;es:=0;markers:=0;kinds:=0;
  g:=json_next(j[..]);
  while g==JSON_ARRAY {
    i:=0;name:="";v:=json_next(j[..]);
    while v==JSON_STRING {
      if i<2 {
        n:=json_unescape(json_text(j[..]),value[..]);if n<0 {return KB_TAGS;}
        s:=cast(string)value[0..<n];
        if i==0 {name=ifx s=="d" then "d" else ifx s=="x" then "x" else ifx s=="e" then "e" else ifx s=="t" then "t" else "-";}
        else if name=="d" {ds+=1;if s!=r[0].id {return KB_TAGS;}}
        else if name=="x" && t==KB_ENTRY {xs+=1;if !kb_hex64(s) {return KB_TAGS;}r[0].digest=kb_keep(a,s);if r[0].digest=="" {return KB_LIMIT;}}
        else if name=="e" && t!=KB_ENTRY {es+=1;if s!=r[0].entry {return KB_TAGS;}}
        else if name=="t" {
          if !kb_lower(s) {return KB_TAGS;}
          if s==KB_MARKERS[t] {markers+=1;}
          else if t==KB_ENTRY && kb_prefix(s,"oa:kb:kind:") {kinds+=1;if s[11..<s.count]!=KB_KINDS[r[0].kind] {return KB_TAGS;}}
          else if kb_prefix(s,"oa:kb:") {return KB_TAGS;}
        }
      }
      i+=1;v=json_next(j[..]);
    }
    if v!=JSON_ARRAY_END {return KB_TAGS;}
    g=json_next(j[..]);
  }
  if g!=JSON_ARRAY_END || ds!=1 || markers!=1 {return KB_TAGS;}
  if t==KB_ENTRY {if xs!=1 || kinds!=1 {return KB_TAGS;}}
  else if es!=1 {return KB_TAGS;}
  return KB_OK;
}
```

### The front matter

The document's front matter is the author's own statement of what the
entry is, and it must say the same as the body. `kb_front` reads only the
four keys it needs and skips every other line, so a title, a folded
summary or a list of citations doesn't confuse it:

<!-- source: coax/lib/nostr/kb/entry.coax front -->
```jai
// The document's YAML front matter: "---" alone on the first line, then
// lines up to "---" alone again. A line that starts with "id:",
// "version:", "kind:" or "status:" sets that key; every other line
// (titles, indented text, lists, comments) is skipped. A value stops at
// " #" and is trimmed. id, version and kind must each appear once and equal
// the body's; status, when present, is one of three words.
KB_FRONT_KEYS :: [4]string.["id:","version:","kind:","status:"];
kb_value :: (line:string,from:int)->string {
  to:=line.count;
  for i:from..<line.count-1 {if to==line.count && line[i]==' ' && line[i+1]=='#' {to=i;}}
  at:=from;
  while at<to && line[at]==' ' {at+=1;}
  while to>at && line[to-1]==' ' {to-=1;}
  return line[at..<to];
}
kb_front :: (doc:string,r:[]KbRecord)->u32 {
  if !kb_prefix(doc,"---\n") {return KB_DOCUMENT;}
  at:=4;seen:=0;
  while true {
    nl:=kb_find(doc,at,'\n');
    if nl== -1 {return KB_DOCUMENT;}
    line:=doc[at..<nl];at=nl+1;
    if line=="---" {break;}
    f:=-1;for i:0..<4 {if kb_prefix(line,KB_FRONT_KEYS[i]) {f=i;}}
    if f<0 {continue;}
    if (seen&(1<<f))!=0 {return KB_DOCUMENT;}
    seen|=1<<f;value:=kb_value(line,KB_FRONT_KEYS[f].count);
    if f==0 {if value!=r[0].id {return KB_DOCUMENT;}}
    else if f==1 {n:=json_int(value);if !n.ok || n.value!=r[0].version {return KB_DOCUMENT;}}
    else if f==2 {if value!=KB_KINDS[r[0].kind] {return KB_DOCUMENT;}}
    else {
      v:=-1;for i:0..<3 {if value==KB_STATUS[i] {v=i;}}
      if v<0 {return KB_DOCUMENT;}
      r[0].status=v;
    }
  }
  if (seen&7)!=7 {return KB_DOCUMENT;}
  return KB_OK;
}
```

The document's `status` is read too (a missing status is `candidate`),
but it is the author's claim, not the reader's decision; see
[Trust](#trust).

### Reading a record

`kb_read` verifies the NIP-01 id and the BIP-340 signature, and
`kb_admit` does the rest in a fixed order: the kind, the body, the tags,
then for an entry the digest and the front matter, and for a head or
withdrawal the signer. Only the author binds a head or a withdrawal to a
version, so `entry.pubkey` must be the event's signer:

<!-- source: coax/lib/nostr/kb/entry.coax read -->
```jai
kb_fail :: (r:KbRecord,error:u32)->KbRecord {out:=r;out.error=error;return out;}
// out is the event reader's scratch (at least the event's serialized size);
// a holds every string the record returns.
kb_read :: (text:string,out:[]u8,a:[]KbText)->KbRecord {
  r:KbRecord;
  e:=nostr_event_verify_vartime(text,out,false);
  if e.error==NE_SIGNATURE {return kb_fail(r,KB_SIGNATURE);}
  if e.error!=NE_OK {return kb_fail(r,KB_EVENT);}
  return kb_admit(e,a);
}
// The KB checks on an event whose id (and, from kb_read, signature) verified.
kb_admit :: (e:NostrEvent,a:[]KbText)->KbRecord {
  r:=[1]KbRecord.[KbRecord.{}];text:=e.original;id:=e.id;key:=e.public_key;
  t:=-1;for i:0..<3 {if e.kind==KB_EVENT_KINDS[i] {t=i;}}
  if t<0 {return kb_fail(r[0],KB_KIND);}
  r[0].type=t;r[0].created_at=e.created_at;
  r[0].event=kb_hex(a,id[..]);r[0].author=kb_hex(a,key[..]);
  if r[0].author=="" {return kb_fail(r[0],KB_LIMIT);}
  bn:=json_unescape(text[e.content_from..<e.content_to],a[0].buf[a[0].at..<a[0].buf.count]);
  if bn<0 {return kb_fail(r[0],KB_LIMIT);}
  body:=cast(string)a[0].buf[a[0].at..<a[0].at+bn];a[0].at+=bn;
  b:=kb_body(body,t,r[..],a);if b!=KB_OK {return kb_fail(r[0],b);}
  g:=kb_tags(text[e.tags_from..<e.tags_to],t,r[..],a);if g!=KB_OK {return kb_fail(r[0],g);}
  if t==KB_ENTRY {
    if r[0].computed!=r[0].digest {return kb_fail(r[0],KB_DIGEST);}
    f:=kb_front(r[0].document,r[..]);if f!=KB_OK {return kb_fail(r[0],f);}
  }
  // Only the author binds a head or withdrawal to a version.
  else if r[0].entry_author!=r[0].author {return kb_fail(r[0],KB_SIGNER);}
  return kb_fail(r[0],KB_OK);
}
```

`KB_OK` says the record is well formed and signed by its author. It is not
a trust decision.

## Which version is current

A reader collects every record it admitted, from any relays, in any order.
For one entry it then decides which version to show. NIP-KB gives the
rules, and `current.coax` applies them:

1. A version is withdrawn when a withdrawal by the same author binds its
   exact event, ID and version. Withdrawal is irreversible.
2. A version is in conflict when the same author published the same ID
   and version with another digest. That is equivocation: the reader
   reports both events and shows neither, whatever their timestamps.
3. The newest head (the greatest `created_at`, then the lowest event id,
   NIP-01's rule for addressable events) is followed when the reader holds
   the exact version it names and that version is neither withdrawn nor in
   conflict.
4. Otherwise the reader takes the highest version that is neither.

<!-- source: coax/lib/nostr/kb/current.coax rules -->
```jai
kb_less :: (a:string,b:string)->bool {
  for i:0..<a.count {if i>=b.count {return false;}if a[i]<b[i] {return true;}if a[i]>b[i] {return false;}}
  return a.count<b.count;
}
kb_mine :: (r:KbRecord,type:int,author:string,id:string)->bool {
  return r.error==KB_OK && r.type==type && r.author==author && r.id==id;
}
// A withdrawal by the version's own author, bound to its exact event.
kb_withdrawn :: (rs:[]KbRecord,v:KbRecord)->bool {
  for w:rs {if kb_mine(w,KB_WITHDRAWAL,v.author,v.id) && w.entry==v.event && w.version==v.version {return true;}}
  return false;
}
// The same author, ID and version under another digest.
kb_conflict :: (rs:[]KbRecord,v:KbRecord)->bool {
  for u:rs {if kb_mine(u,KB_ENTRY,v.author,v.id) && u.version==v.version && u.digest!=v.digest {return true;}}
  return false;
}
```

<!-- source: coax/lib/nostr/kb/current.coax codes -->
```jai
KC_CURRENT :: cast(u32)0;  // event and version are the one to show
KC_NONE :: cast(u32)1;     // none held, or every version withdrawn or in conflict
KbCurrent :: struct {
  error:u32=KC_NONE;
  event:string; version:int; digest:string; via_head:bool;
  withdrawn:int; conflicts:int;   // version events of this entry withdrawn, and in conflict
}
```

<!-- source: coax/lib/nostr/kb/current.coax current -->
```jai
kb_current :: (rs:[]KbRecord,author:string,id:string)->KbCurrent {
  c:KbCurrent;
  head:=-1;
  for i:0..<rs.count {
    h:=rs[i];
    if kb_mine(h,KB_HEAD,author,id) {
      if head== -1 || h.created_at>rs[head].created_at || (h.created_at==rs[head].created_at && kb_less(h.event,rs[head].event)) {head=i;}
    }
  }
  best:=-1;
  for i:0..<rs.count {
    v:=rs[i];
    if !kb_mine(v,KB_ENTRY,author,id) {continue;}
    out:=kb_withdrawn(rs,v);split:=kb_conflict(rs,v);
    if out {c.withdrawn+=1;}
    if split {c.conflicts+=1;}
    if out || split {continue;}
    if head>=0 && v.event==rs[head].entry && v.version==rs[head].version {c.via_head=true;best=i;}
    if c.via_head {continue;}
    if best== -1 || v.version>rs[best].version || (v.version==rs[best].version && kb_less(v.event,rs[best].event)) {best=i;}
  }
  if best== -1 {return c;}
  c.error=KC_CURRENT;c.event=rs[best].event;c.version=rs[best].version;c.digest=rs[best].digest;
  return c;
}
```

A withdrawn version is never current: it is skipped before a head or a
version number is considered. Publishing the head again changes nothing,
because the head still names the withdrawn event. To replace a withdrawn
or conflicting version, the author publishes a new, higher version.

### Trust

A signature proves who wrote an entry, not that the entry is right. Each
reader keeps its own list of trusted authors. Entries by the reader's own
key or a trusted author keep the status their document states; any other
author's entries are at most `candidate`, whatever the document says. A
reader must not show them as admitted:

<!-- source: coax/lib/nostr/kb/current.coax status -->
```jai
kb_status :: (r:KbRecord,reader:string,trusted:[]string)->int {
  if r.author==reader {return r.status;}
  for t:trusted {if r.author==t {return r.status;}}
  if r.status==KS_ADMITTED {return KS_CANDIDATE;}
  return r.status;
}
```

Admission on the reader's side comes from evidence (kind 3189, not built
here) by evaluators it trusts, or from its own review.

## Conditional document changes

NIP-WS doesn't add event kinds. It binds a workspace's documents to the
host that owns their actual state, and defines the records a client and a
host exchange. The host keeps a workspace binding (its authority, the
workspace, a generation and a revision), each document's current
observation, the grants it established independently, and the commands it
has retained. Here a document's observation is reduced to its version and
the SHA-256 of its exact bytes:

<!-- source: coax/lib/nostr/ws/change.coax record -->
```jai
WsBinding :: struct {authority:string;workspace:string;generation:int;revision:int;}
WsResource :: struct {resource:string;lifecycle:int;editable:bool;version:int;digest:string;}
WsGrant :: struct {authorization:string;actor:string;resource:string;write:bool;expires_at:int;revoked:bool;}
WsChange :: struct {
  command:string; fingerprint:string; actor:string; signer:string;
  authority:string; workspace:string; resource:string; generation:int; revision:int;
  expected_version:int; expected_digest:string;   // the observation the actor read
  snapshot_version:int; snapshot_digest:string;   // the replacement's snapshot
  content:string;                                 // the SHA-256 of the new bytes
  authorization:string; issued_at:int; expires_at:int;
}
WsRetained :: struct {authority:string;actor:string;command:string;fingerprint:string;before_version:int;after_version:int;after_digest:string;}
WsResult :: struct {code:u32;before_version:int;after_version:int;after_digest:string;}
```

A change request names the observation the actor read (`expected`) and
the replacement's snapshot, which must be that same observation, and
carries the new bytes' digest. The codes are NIP-WS's common refusal
codes, plus `expired` for the request's own window:

<!-- source: coax/lib/nostr/ws/change.coax codes -->
```jai
WS_COMPLETED :: cast(u32)0;    // the version moved: before and after are returned
WS_DUPLICATE :: cast(u32)1;    // an identical retransmission: the retained result, no new effect
WS_MALFORMED :: cast(u32)2;    // a bad window, or the snapshot is not the expected observation
WS_IDENTITY :: cast(u32)3;     // identity_mismatch: the signer is not the actor, or another authority or workspace
WS_NOT_ADMITTED :: cast(u32)4; // no grant for this actor, resource and write
WS_REVOKED :: cast(u32)5;
WS_EXPIRED :: cast(u32)6;      // the request or its grant has expired by the host's clock
WS_UNAVAILABLE :: cast(u32)7;  // no such resource, or not available
WS_UNSUPPORTED :: cast(u32)8;  // unsupported_feature: the snapshot is not editable
WS_CONFLICT :: cast(u32)9;     // the workspace binding moved
WS_STALE :: cast(u32)10;       // the expected observation is not the current one
WS_IDEMPOTENCY :: cast(u32)11; // idempotency_conflict: other bytes under a retained command
WS_LIMIT :: cast(u32)12;       // limit_exceeded: no room to retain the command

WS_AVAILABLE :: 0; WS_UNAVAILABLE_NOW :: 1; WS_REMOVED :: 2; WS_UNKNOWN :: 3;  // lifecycle
WS_WINDOW :: 3600;  // this host's longest request window, seconds
WS_SKEW :: 60;      // how far ahead of the host's clock a request may be dated
```

The host decides in a fixed order:

1. The signer must be the actor.
2. The request's window must be well formed and open on the host's clock.
3. The authority and workspace must be the binding's.
4. A grant named by the request's authorization must be for this actor,
   this resource and writing, unrevoked and unexpired. Being in the room,
   or sharing a cursor, is not a grant.
5. Idempotency by (authority, actor, command): the same fingerprint
   returns the retained result; another fingerprint refuses.
6. The resource must exist and be available.
7. The binding's generation and revision must be current, or the change
   refuses as `conflict`.
8. The snapshot must be the expected observation, and editable: complete,
   exact bytes.
9. Last, at the mutation boundary, the expected observation must be the
   current one, or the change refuses as `stale`.

<!-- source: coax/lib/nostr/ws/change.coax admit -->
```jai
ws_out :: (code:u32)->WsResult {r:WsResult;r.code=code;return r;}
ws_change :: (b:WsBinding,res:[]WsResource,grants:[]WsGrant,kept:[]WsRetained,n:[]int,c:WsChange,now:int)->WsResult {
  if c.signer!=c.actor {return ws_out(WS_IDENTITY);}
  if c.expires_at<=c.issued_at || c.expires_at-c.issued_at>WS_WINDOW || c.issued_at>now+WS_SKEW {return ws_out(WS_MALFORMED);}
  if now>=c.expires_at {return ws_out(WS_EXPIRED);}
  if c.authority!=b.authority || c.workspace!=b.workspace {return ws_out(WS_IDENTITY);}
  g:=-1;for i:0..<grants.count {if grants[i].authorization==c.authorization {g=i;}}
  if g<0 || grants[g].actor!=c.actor || grants[g].resource!=c.resource || !grants[g].write {return ws_out(WS_NOT_ADMITTED);}
  if grants[g].revoked {return ws_out(WS_REVOKED);}
  if now>=grants[g].expires_at {return ws_out(WS_EXPIRED);}
  for i:0..<n[0] {
    k:=kept[i];
    if k.authority==c.authority && k.actor==c.actor && k.command==c.command {
      if k.fingerprint!=c.fingerprint {return ws_out(WS_IDEMPOTENCY);}
      r:=ws_out(WS_DUPLICATE);r.before_version=k.before_version;r.after_version=k.after_version;r.after_digest=k.after_digest;return r;
    }
  }
  t:=-1;for i:0..<res.count {if res[i].resource==c.resource {t=i;}}
  if t<0 || res[t].lifecycle!=WS_AVAILABLE {return ws_out(WS_UNAVAILABLE);}
  if c.generation!=b.generation || c.revision!=b.revision {return ws_out(WS_CONFLICT);}
  if c.snapshot_version!=c.expected_version || c.snapshot_digest!=c.expected_digest {return ws_out(WS_MALFORMED);}
  if !res[t].editable {return ws_out(WS_UNSUPPORTED);}
  // The mutation boundary: compare and swap against the current observation.
  if c.expected_version!=res[t].version || c.expected_digest!=res[t].digest {return ws_out(WS_STALE);}
  if n[0]>=kept.count {return ws_out(WS_LIMIT);}
  r:=ws_out(WS_COMPLETED);r.before_version=res[t].version;
  res[t].version+=1;res[t].digest=c.content;
  r.after_version=res[t].version;r.after_digest=c.content;
  kept[n[0]]=WsRetained.{authority=c.authority,actor=c.actor,command=c.command,fingerprint=c.fingerprint,before_version=r.before_version,after_version=r.after_version,after_digest=r.after_digest};
  n[0]+=1;
  return r;
}
```

Idempotency comes before the version check. A retransmission of a change
that already completed would be stale against its own effect; checking
the retained command first returns the original result instead. It comes
after the grant check, so an actor whose grant was revoked can't read
retained results.

NIP-WS forbids an overwrite fallback and an implicit merge. When two
actors edit the same version, the first change moves the version and the
second is stale. The second actor reads the new version, merges by hand
and submits a new change against it.

### Worked example: two edits, one merge, one entry

The entry `statistics.mmd-estimators` version 1 is the file `mmd.md` in
the workspace `team-kb`, at version 4. Its digest, from Python's
`hashlib`, is `19952efa5bd4a152…`. Alice and Bob both read version 4.

| Step | Request | Expected | Current | Decision | After |
| --- | --- | --- | --- | --- | --- |
| 1 | Alice `a-1`: fixes the denominator, new digest `b3673995…` | 4, `19952efa…` | 4, `19952efa…` | equal: completed | 5, `b3673995…` |
| 2 | Bob `b-1`: adds a check section, `f6b5ef98…` | 4, `19952efa…` | 5, `b3673995…` | 4 ≠ 5: stale | 5 |
| 3 | Alice `a-1` again | (retained) | 5 | same command, same fingerprint: duplicate, 4 → 5 | 5 |
| 4 | Bob `b-2`: both edits by hand, front matter `version: 2`, `92b7aeb9…` | 5, `b3673995…` | 5, `b3673995…` | equal: completed | 6, `92b7aeb9…` |

The author publishes the merged file as version 2 of the entry. Its `x`
tag is `92b7aeb9…`, the workspace's version 6. The reader then holds these
records, all by the author:

| Records held | Head | Withdrawn | In conflict | Current |
| --- | --- | --- | --- | --- |
| v1, v2, head → v2 | v2 (held, clean) | none | none | v2, by the head |
| + withdrawal of v2 | v2 (withdrawn: passed over) | v2 | none | v1, the highest left |
| + v3 | passed over | v2 | none | v3, the highest |
| v1, v2, head → v2, a second v2 (`eea50d64…`) | v2 (in conflict) | none | both v2 | v1 |

The withdrawal row is the first "Done means" line: version 2 is withdrawn,
so it is not served as current even though the head still names it.
Step 2 is the second: a conditional change against a stale version
refuses.

## What is not built

- Evidence (kind 3189): the NIP-EVAL publication citing entry versions,
  its inline report and the `#e` index.
- Curated snapshots (NIP-EXT package releases of entry sets).
- Private entries in the encrypted 3188 artifact envelope. Chapter 79's
  carriage seals and opens a JSON payload; an entry's bytes are Markdown,
  so it needs the envelope's non-JSON form first.
- The topic `t` tags are checked for lowercase but not compared with the
  front matter's `tags`.
- The reader's own content lint (a cited source, a bounded summary, no
  benchmark task names) and its embeddings.
- NIP-WS beyond text replacement: create, move and delete operations,
  document ranges, Git and external-service sources, POL approvals,
  remote source confirmation, the change-result and admission wrapper
  records, worktrees, checkpoints and restoration, projections, cuts,
  pages and deltas, and command visibility.
- NIP-11 advertisement of `nip-kb-v1`.

## The example

[nostr/examples/knowledge-workspaces.coax](../examples/knowledge-workspaces.coax)
runs the worked example at a fixed clock. The workspace digests are
computed by Coax's SHA-256; the KB events were signed by an independent
BIP-340 in Python
([kb_vectors.py](../../coax/tests/nostr/kb_vectors.py)).

The digest helper and the host's workspace:

<!-- source: nostr/examples/knowledge-workspaces.coax digest -->
```jai
// The SHA-256 of a text, as lowercase hex: the observation's digest and the x tag.
digest_of :: (text:string,out:[]u8)->string {
  b:[1024]u8;for i:0..<text.count {b[i]=text[i];}
  d:[32]u8;s:=sha256_hash_checked(b[0..<text.count],d[..],false);
  r:=hex_encode_checked(cast(string)d[..],out,false);return cast(string)out[0..<r.written];
}
```

<!-- source: nostr/examples/knowledge-workspaces.coax workspace -->
```jai
// The host's workspace: one binding, the entry's file at version 4, and
// the grants it established. Carol is in the room with no grant; Bob also
// holds a read-only grant.
binding :: ()->WsBinding {return WsBinding.{authority=HOST,workspace="team-kb",generation=1,revision=3};}
host_grants :: ()->[3]WsGrant {
  return [3]WsGrant.[WsGrant.{authorization="g-alice",actor=ALICE,resource="mmd.md",write=true,expires_at=T0+86400},
                     WsGrant.{authorization="g-bob",actor=BOB,resource="mmd.md",write=true,expires_at=T0+86400},
                     WsGrant.{authorization="g-bob-read",actor=BOB,resource="mmd.md",write=false,expires_at=T0+86400}];
}
// A replacement of mmd.md read at (version, digest), with the new bytes' digest.
change :: (actor:string,command:string,version:int,read:string,content:string)->WsChange {
  c:WsChange;c.command=command;c.fingerprint=command;c.actor=actor;c.signer=actor;
  c.authority=HOST;c.workspace="team-kb";c.resource="mmd.md";c.generation=1;c.revision=3;
  c.expected_version=version;c.expected_digest=read;c.snapshot_version=version;c.snapshot_digest=read;
  c.content=content;c.authorization=ifx actor==ALICE then "g-alice" else "g-bob";c.issued_at=T0;c.expires_at=T0+600;
  return c;
}
show_change :: (label:string,r:WsResult) {
  if r.code==WS_COMPLETED || r.code==WS_DUPLICATE {print("%: %, version % -> % (%…)\n",label,WS_WORDS[r.code],r.before_version,r.after_version,r.after_digest[0..<16]);}
  else {print("%: %\n",label,WS_WORDS[r.code]);}
}
```

The reader's side:

<!-- source: nostr/examples/knowledge-workspaces.coax knowledge -->
```jai
show_current :: (label:string,rs:[]KbRecord) {
  c:=kb_current(rs,KBV_AUTHOR,"statistics.mmd-estimators");
  if c.error==KC_NONE {print("%: none (withdrawn %, in conflict %)\n",label,c.withdrawn,c.conflicts);return;}
  via:=ifx c.via_head then "the head" else "the highest version";
  print("%: version % (event %…, by %), withdrawn %, in conflict %\n",label,c.version,c.event[0..<16],via,c.withdrawn,c.conflicts);
}
show_read :: (label:string,text:string,out:[]u8,store:[]u8) {
  a:=[1]KbText.[KbText.{buf=store}];r:=kb_read(text,out,a[..]);
  print("%: %\n",label,KB_WORDS[r.error]);
}
```

<!-- source: nostr/examples/knowledge-workspaces.coax main -->
```jai
main :: () {
  h:[64]u8;
  print("WS: mmd.md is at version 4; Alice and Bob both read it\n");
  docs:=[1]WsResource.[WsResource.{resource="mmd.md",lifecycle=WS_AVAILABLE,editable=true,version=4,digest=digest_of(KBV_DOC1,h[..])}];
  ha:[64]u8;hb:[64]u8;hm:[64]u8;
  d4:=docs[0].digest;da:=digest_of(KBV_TEXT_A,ha[..]);db:=digest_of(KBV_TEXT_B,hb[..]);dm:=digest_of(KBV_DOC2,hm[..]);
  print("version 4 %…; Alice's text %…; Bob's text %…\n",d4[0..<16],da[0..<16],db[0..<16]);
  g:=host_grants();kept:[8]WsRetained;n:[1]int;b:=binding();
  show_change("Alice replaces version 4",ws_change(b,docs[..],g[..],kept[..],n[..],change(ALICE,"a-1",4,d4,da),T0+10));
  show_change("Bob replaces version 4",ws_change(b,docs[..],g[..],kept[..],n[..],change(BOB,"b-1",4,d4,db),T0+11));
  show_change("Alice's request again",ws_change(b,docs[..],g[..],kept[..],n[..],change(ALICE,"a-1",4,d4,da),T0+12));
  print("Bob reads version 5 and merges both edits by hand (%…)\n",dm[0..<16]);
  show_change("Bob replaces version 5",ws_change(b,docs[..],g[..],kept[..],n[..],change(BOB,"b-2",5,da,dm),T0+20));
  print("mmd.md: version %, digest %…, % commands retained\n",docs[0].version,docs[0].digest[0..<16],n[0]);

  print("KB: the author publishes the merged file as version 2\n");
  out:[16384]u8;store:[16384]u8;a:=[1]KbText.[KbText.{buf=store[..]}];
  rs:[8]KbRecord;
  events:=[6]string.[KBV_E1,KBV_E2,KBV_HEAD2,KBV_WITHDRAW2,KBV_E3,KBV_EQUIVOCATION];
  for i:0..<6 {rs[i]=kb_read(events[i],out[..],a[..]);}
  for i:0..<3 {
    r:=rs[i];what:=ifx r.type==KB_ENTRY then "digest" else "names event";
    print("% v%: %, event %…, % %…\n",TYPE_WORDS[r.type],r.version,KB_WORDS[r.error],r.event[0..<16],what,ifx r.type==KB_ENTRY then r.digest[0..<16] else r.entry[0..<16]);
  }
  print("version 2's document is the workspace's version 6: %\n",rs[1].digest==docs[0].digest);
  show_current("current",rs[0..<3]);
  trusted:=[1]string.[KBV_AUTHOR];
  print("status shown: % to the author's own reader, % to a stranger, % to one who trusts the author\n",
    KB_STATUS[kb_status(rs[1],KBV_AUTHOR,trusted[0..<0])],KB_STATUS[kb_status(rs[1],KBV_OTHER,trusted[0..<0])],KB_STATUS[kb_status(rs[1],KBV_OTHER,trusted[..])]);
  print("KB: the author withdraws version 2 (\"%\")\n",rs[3].reason);
  print("withdrawal: %, binds event %…\n",KB_WORDS[rs[3].error],rs[3].entry[0..<16]);
  show_current("current",rs[0..<4]);
  show_current("current, with version 3",rs[0..<5]);
  print("KB: a second version 2 from the same author\n");
  print("entry v2: %, digest %…\n",KB_WORDS[rs[5].error],rs[5].digest[0..<16]);
  eq:=[4]KbRecord.[rs[0],rs[1],rs[2],rs[5]];
  show_current("current",eq[..]);

  print("broken inputs\n");
  show_read("version 2 with a changed document",KBV_TAMPERED_DOCUMENT,out[..],store[..]);
  show_read("version 1 with a changed signature",KBV_TAMPERED_SIGNATURE,out[..],store[..]);
  show_read("version 1 whose d tag names another entry",KBV_TAG_DISAGREES,out[..],store[..]);
  show_read("version 3 whose front matter says version 2",KBV_FRONT_DISAGREES,out[..],store[..]);
  show_read("a head signed by another author",KBV_HEAD_BY_OTHER,out[..],store[..]);
  show_read("a withdrawal signed by another author",KBV_WITHDRAWAL_BY_OTHER,out[..],store[..]);
  show_read("version 1 with an embedding field",KBV_VECTOR_FIELD,out[..],store[..]);
  c:=change(CAROL,"c-1",6,dm,da);c.authorization="g-carol";
  show_change("Carol, in the room with no grant",ws_change(b,docs[..],g[..],kept[..],n[..],c,T0+30));
  c=change(BOB,"b-3",6,dm,da);c.authorization="g-bob-read";
  show_change("Bob under his read-only grant",ws_change(b,docs[..],g[..],kept[..],n[..],c,T0+30));
  c=change(ALICE,"a-1",6,dm,db);c.fingerprint="a-1, other bytes";
  show_change("Alice's command a-1 with other bytes",ws_change(b,docs[..],g[..],kept[..],n[..],c,T0+30));
  b2:=b;b2.generation=2;
  show_change("after the workspace's roots were remapped",ws_change(b2,docs[..],g[..],kept[..],n[..],change(ALICE,"a-2",6,dm,da),T0+30));
  show_change("a request issued at T0, at T0 + 600",ws_change(b,docs[..],g[..],kept[..],n[..],change(ALICE,"a-3",6,dm,da),T0+600));
  print("mmd.md: version %, % commands retained\n",docs[0].version,n[0]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/knowledge-workspaces.coax --run-steps 250000000
```

Its output, in debug and release builds alike
([nostr/fixtures/knowledge-workspaces.out](../fixtures/knowledge-workspaces.out)):

```text
WS: mmd.md is at version 4; Alice and Bob both read it
version 4 19952efa5bd4a152…; Alice's text b36739956f4d7586…; Bob's text f6b5ef98863f56a9…
Alice replaces version 4: completed, version 4 -> 5 (b36739956f4d7586…)
Bob replaces version 4: stale
Alice's request again: duplicate, version 4 -> 5 (b36739956f4d7586…)
Bob reads version 5 and merges both edits by hand (92b7aeb9a48bdc3a…)
Bob replaces version 5: completed, version 5 -> 6 (92b7aeb9a48bdc3a…)
mmd.md: version 6, digest 92b7aeb9a48bdc3a…, 2 commands retained
KB: the author publishes the merged file as version 2
entry v1: ok, event 52ac5361940b3f61…, digest 19952efa5bd4a152…
entry v2: ok, event 59f409e9f04cba6c…, digest 92b7aeb9a48bdc3a…
head v2: ok, event 673448e56b2faccd…, names event 59f409e9f04cba6c…
version 2's document is the workspace's version 6: true
current: version 2 (event 59f409e9f04cba6c…, by the head), withdrawn 0, in conflict 0
status shown: admitted to the author's own reader, candidate to a stranger, admitted to one who trusts the author
KB: the author withdraws version 2 ("The check section's claim needs a citation.")
withdrawal: ok, binds event 59f409e9f04cba6c…
current: version 1 (event 52ac5361940b3f61…, by the highest version), withdrawn 1, in conflict 0
current, with version 3: version 3 (event b8e1e59a16a42b55…, by the highest version), withdrawn 1, in conflict 0
KB: a second version 2 from the same author
entry v2: ok, digest eea50d64059008ba…
current: version 1 (event 52ac5361940b3f61…, by the highest version), withdrawn 0, in conflict 2
broken inputs
version 2 with a changed document: digest
version 1 with a changed signature: signature
version 1 whose d tag names another entry: tags
version 3 whose front matter says version 2: document
a head signed by another author: signer
a withdrawal signed by another author: signer
version 1 with an embedding field: shape
Carol, in the room with no grant: not_admitted
Bob under his read-only grant: not_admitted
Alice's command a-1 with other bytes: idempotency_conflict
after the workspace's roots were remapped: conflict
a request issued at T0, at T0 + 600: expired
mmd.md: version 6, 2 commands retained
```

The first part is the worked example's table: Alice's change completes,
Bob's is stale, Alice's retransmission returns the retained result, and
Bob's merge completes against version 5. The second part is the reader's
table: the head is followed to version 2, the withdrawal takes version 2
out of service, version 3 replaces it, and a second version 2 is
reported as a conflict with neither shown.

The broken inputs are NIP-KB's client fixtures and the host's refusals.
The changed document no longer matches its `x` tag (`digest`). The changed
signature fails BIP-340 (`signature`). The `d` tag naming another entry
disagrees with the body (`tags`). Version 3 whose front matter says
version 2 fails the front matter check (`document`). The head and
withdrawal signed by another author name the author's 3190, so their
signer is not `entry.pubkey` (`signer`). The body with an `embedding`
field has an unknown key (`shape`). Carol is in the room but holds no
grant, and Bob's read-only grant does not write (`not_admitted`). Alice's
command `a-1` with other bytes is an `idempotency_conflict`. A remapped
workspace (generation 2) is a `conflict`. A request issued at T0 with a
600-second window is `expired` at T0 + 600.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/knowledge-workspaces.coax](../examples/checks/knowledge-workspaces.coax),
which loads the example:

- The event ids and document digests from
  [kb_vectors.py](../../coax/tests/nostr/kb_vectors.py), which signs with
  [nip59_reference.py](../../coax/tests/nostr/nip59_reference.py)'s
  integer secp256k1 and BIP-340 and hashes with `hashlib`. Coax must
  verify the Python events, recompute their ids and find the same digests.
- NIP-KB's own body examples, as printed. They are illustrative: the
  entry's document stops at `...` before its kind and closing line, and
  the head names `<3190 event id>` where a 64-hex id belongs. NIP-KB
  publishes no event ids or signatures to compare with; these placeholders
  are the examples' only wrong values, and the check records exactly where
  each refuses.
- The worked example's versions and decisions by hand.

<!-- source: nostr/examples/checks/knowledge-workspaces.coax check -->
```jai
// From coax/tests/nostr/kb_vectors.py, which signs with nip59_reference.py's
// integer secp256k1 and BIP-340 (no code shared with Coax): each event id as
// hashlib.sha256 of json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":")), and each document digest as hashlib.sha256 of its
// UTF-8 bytes.
PY_E1_ID :: "52ac5361940b3f613adb25e50901bd1d3366ecf71b1f22a82da66ecebe465963";
PY_E2_ID :: "59f409e9f04cba6c7c4395c41871d7ea8bfa16c565fde24695455cd309d0c3e5";
PY_DOC1_SHA :: "19952efa5bd4a152bec6f05fb32966c874f877c62075df3592e246447c600277";
PY_DOC2_SHA :: "92b7aeb9a48bdc3aa9bc5f778a3642ad9adc881886b78250ce7e671b58ead52e";

#test "the event ids and digests are the Python reference's" #steps 1000000000 {
  out:[16384]u8;store:[16384]u8;a:=[1]KbText.[KbText.{buf=store[..]}];
  r1:=kb_read(KBV_E1,out[..],a[..]);r2:=kb_read(KBV_E2,out[..],a[..]);
  #assert r1.error==KB_OK && r1.event==PY_E1_ID && r1.digest==PY_DOC1_SHA && r1.computed==PY_DOC1_SHA;
  #assert r2.error==KB_OK && r2.event==PY_E2_ID && r2.digest==PY_DOC2_SHA && r2.computed==PY_DOC2_SHA;
  h:[64]u8;#assert digest_of(KBV_DOC2,h[..])==PY_DOC2_SHA && KBV_E1_ID==PY_E1_ID && KBV_DOC1_SHA==PY_DOC1_SHA;
}

// NIP-KB's own body examples are illustrative: the entry's document is cut
// off ("...") before its kind and closing line, and the head and withdrawal
// carry the placeholder "<3190 event id>" where a 64-hex id belongs. Read as
// published, the bodies refuse at exactly those points.
NIP_ENTRY :: "{\"v\":1,\"requires\":[],\"type\":\"entry\",\"id\":\"statistics.mmd-estimators\",\"version\":1,\"kind\":\"method\",\"document\":\"---\\nid: statistics.mmd-estimators\\nversion: 1\\n...\"}";
NIP_HEAD :: "{\"v\":1,\"requires\":[],\"type\":\"head\",\"id\":\"statistics.mmd-estimators\",\"version\":2,\"entry\":{\"id\":\"<3190 event id>\",\"pubkey\":\"<author>\",\"kind\":3190}}";
#test "NIP-KB's published examples, as printed" {
  s:[4096]u8;a:=[1]KbText.[KbText.{buf=s[..]}];r:=[1]KbRecord.[KbRecord.{}];
  #assert kb_body(NIP_ENTRY,KB_ENTRY,r[..],a[..])==KB_OK && kb_front(r[0].document,r[..])==KB_DOCUMENT;
  b:=[1]KbText.[KbText.{buf=s[..]}];q:=[1]KbRecord.[KbRecord.{}];
  #assert kb_body(NIP_HEAD,KB_HEAD,q[..],b[..])==KB_FIELD;
}

// By hand: both read version 4. Alice's compare-and-swap finds 4 = 4 and
// moves the file to 5; Bob's finds 4 != 5 and is stale; Bob's merge, read
// at 5, finds 5 = 5 and moves it to 6. The KB reader follows the head to
// version 2; after the withdrawal of version 2 the highest version left is
// 1; with version 3 published it is 3.
#test "the worked example's values, by hand" {
  docs:=[1]WsResource.[WsResource.{resource="mmd.md",lifecycle=WS_AVAILABLE,editable=true,version=4,digest=PY_DOC1_SHA}];
  g:=host_grants();k:[4]WsRetained;n:[1]int;b:=binding();
  #assert ws_change(b,docs[..],g[..],k[..],n[..],change(ALICE,"a-1",4,PY_DOC1_SHA,KBV_TEXT_A_SHA),T0+10).code==WS_COMPLETED && docs[0].version==5;
  #assert ws_change(b,docs[..],g[..],k[..],n[..],change(BOB,"b-1",4,PY_DOC1_SHA,KBV_TEXT_B_SHA),T0+11).code==WS_STALE && docs[0].version==5;
  #assert ws_change(b,docs[..],g[..],k[..],n[..],change(BOB,"b-2",5,KBV_TEXT_A_SHA,PY_DOC2_SHA),T0+20).code==WS_COMPLETED;
  #assert docs[0].version==6 && docs[0].digest==PY_DOC2_SHA && n[0]==2;
  rs:=[5]KbRecord.[kbh_v(1,PY_E1_ID,PY_DOC1_SHA),kbh_v(2,PY_E2_ID,PY_DOC2_SHA),kbh_h(),kbh_w(),kbh_v(3,KBV_E3_ID,"d3")];
  c:=kb_current(rs[0..<3],KBV_AUTHOR,"statistics.mmd-estimators");#assert c.version==2 && c.via_head;
  c=kb_current(rs[0..<4],KBV_AUTHOR,"statistics.mmd-estimators");#assert c.version==1 && !c.via_head && c.withdrawn==1;
  c=kb_current(rs[..],KBV_AUTHOR,"statistics.mmd-estimators");#assert c.version==3 && c.event==KBV_E3_ID;
}
kbh_v :: (version:int,event:string,digest:string)->KbRecord {
  r:KbRecord;r.error=KB_OK;r.type=KB_ENTRY;r.author=KBV_AUTHOR;r.id="statistics.mmd-estimators";r.version=version;r.event=event;r.digest=digest;return r;
}
kbh_h :: ()->KbRecord {r:=kbh_v(2,KBV_HEAD2_ID,"");r.type=KB_HEAD;r.entry=PY_E2_ID;return r;}
kbh_w :: ()->KbRecord {r:=kbh_v(2,KBV_WITHDRAW2_ID,"");r.type=KB_WITHDRAWAL;r.entry=PY_E2_ID;return r;}
```

```shell
coax/bin/coax test nostr/examples/checks/knowledge-workspaces.coax --run-steps 250000000
```

`coax test` runs 208 checks, including the loaded files' own checks; all
pass.

### The library's own checks

`entry.coax` reads every Python-signed event and each of NIP-KB's client
fixtures (a tampered document, a tampered signature, a body and tag
disagreement, a front matter disagreement, a head and a withdrawal signed
by someone else), an unknown body key, another kind, empty content and
every arena boundary. It checks each body field's type and bound (the
version at 2^53 − 1 and 2^53, the reason at 1,000 and 1,001 characters),
each tag count and value, and the front matter's keys, comments,
duplicates and skipped lines. Only one check verifies signatures; the rest
read events with the id check alone, so they run in milliseconds.
`current.coax` checks the head rule (newest, then lowest id, held, the
right version), withdrawals bound to the exact event, version and author,
equivocation, the byte order of event ids and the trust rule.
`change.coax` checks each step of the order, the concurrent edits,
retransmission and idempotency, and a full retained table.

```shell
coax/bin/coax test coax/lib/nostr/kb/entry.coax --library --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/kb/entry.coax --root-only --library --run-steps 250000000
coax/bin/coax test coax/lib/nostr/kb/current.coax --library --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/kb/current.coax --root-only --library --run-steps 250000000
coax/bin/coax test coax/lib/nostr/ws/change.coax --library
coax/bin/coax mutate coax/lib/nostr/ws/change.coax --root-only --library
```

On 2026-11-01, each file's checks passed (`coax test` runs the loaded files' checks too: 190 for `entry.coax`, 198 for `current.coax`, 7 for `change.coax`), and root-only `coax mutate` killed every mutant: 531 of 531 in `entry.coax`, 89 of 89 in `current.coax` and 75 of 75 in `change.coax`. None survived and none is marked equivalent. Earlier runs left survivors where two tests decided the same thing (a field-of-another-type check that `seen==mask` repeats, front matter skip rules that the key match repeats, a `< 0` test on an index that is never 0); the code was simplified so each decision is made once, and tests were added for the rest. The test builders live in `coax/tests/nostr/kb_entry_fixtures.coax`, `kb_current_fixtures.coax` and `ws_change_fixtures.coax`, so root-only mutation mutates the library code. The example matches its transcript in debug and release. The full mutation run (`COAX_MUTATION_FULL=1`) and the whole-book example run were not run.

## Summary

- `kb_read` admits a KB record only when its signature, kind, body, tags,
  digest and front matter agree, and a head or withdrawal only from the
  entry's own author. Unknown body keys refuse, so an author can't add a
  retrieval vector.
- `kb_current` never shows a withdrawn version or either side of an
  equivocation; it follows the newest head when it can and otherwise the
  highest clean version.
- `kb_status` caps any untrusted author's entry at `candidate`.
- `ws_change` is compare-and-swap at the host: a change against a stale
  version refuses, a stale writer merges by hand, a retransmission returns
  the retained result, and only a grant, not membership, lets a change
  through.

## Dependencies

- [76. Work, coordination and control](ch076-work-coordination-and-control.md)

## Sources

- [OpenAgents NIP-KB](../nips/openagents/NIP-KB.md)
- [OpenAgents NIP-WS](../nips/openagents/NIP-WS.md)
- [OpenAgents shared contracts](../nips/openagents/contracts.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
