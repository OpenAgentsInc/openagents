# 89. Channels

A NIP-29 group (chapter 59) gives a relay members, roles and admission.
Block's channel NIPs add what a chat client needs on top: a channel type and
labels that ordinary filters can find, a timeline that pages exactly, a DM
the viewer can hide without leaving, and an icon for the whole workspace.
This chapter builds the relay's and the client's decisions for all four in
Coax, on top of `nip29.coax`.

Part VI, Every NIP. Result: NIP-CL channel identity and discovery (the
identity tags on 39000 to 39002, labels, the oldest-channel rule, system
channels and the legacy-type listing rule, including 44100 and 44101
notices), NIP-CW channel and thread windows with composite cursors and
exact `has_more`, NIP-DV DM visibility snapshots and their read gates, and
NIP-WP's workspace icon command. What is not built is listed under
[What is not built](#what-is-not-built).

## What this chapter teaches

- CL: the identity tags a relay writes on every channel-state event, why a
  client reads the type from the first `t` tag, the label rules, the one
  channel every reader picks among several, and which reads see a system
  channel.
- CW: which events are top-level, the total order `created_at` descending
  then `id` ascending, the composite `(created_at, id)` cursor, the
  `limit + 1` probe that makes `has_more` exact, and why a response
  without its bounds overlay is not an empty channel.
- DV: a per-viewer, relay-signed snapshot of hidden DMs, and the two gates
  that keep it from anyone but its owner.
- WP: a role-gated command whose value every client renders, so the relay
  checks it on the way in.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip29.coax](../../coax/lib/nostr/nip29.coax) | `g29_admit`, `g29_tag` | NIP-29 admission and the tag reader this chapter reuses (chapter 59) |
| [nipcl.coax](../../coax/lib/nostr/nipcl.coax) | `cl_labels(e)` | The label set of a create or edit command: keep, set or clear |
| [nipcl.coax](../../coax/lib/nostr/nipcl.coax) | `cl_admit(g, c, e, relay, now, g29)` | A 9007 or 9002 checked for labels and type, then admitted by `g29_admit`; records type, creator and creation time |
| [nipcl.coax](../../coax/lib/nostr/nipcl.coax) | `cl_write_metadata(g, c, out)` | The 39000 tags the relay signs, identity tags included |
| [nipcl.coax](../../coax/lib/nostr/nipcl.coax) | `cl_read(e, relay)` | A client's reading of a relay-signed 39000 to 39002 |
| [nipcl.coax](../../coax/lib/nostr/nipcl.coax) | `cl_oldest(cs)` | The one channel every reader picks |
| [nipcl.coax](../../coax/lib/nostr/nipcl.coax) | `cl_listed`, `cl_notice` | Whether a read sees a channel or a membership notice |
| [nipcw.coax](../../coax/lib/nostr/nipcw.coax) | `cw_ingest(rows, n, e)` | Classifies and stores one channel event: depth, root, broadcast |
| [nipcw.coax](../../coax/lib/nostr/nipcw.coax) | `cw_check(q)` | Validates a window request and its cursor |
| [nipcw.coax](../../coax/lib/nostr/nipcw.coax) | `cw_window(rows, n, q, readable, out)` | One channel or thread page, with `has_more` and the next cursor |
| [nipcw.coax](../../coax/lib/nostr/nipcw.coax) | `cw_aux` | The reactions, deletions and edits of the returned rows |
| [nipcw.coax](../../coax/lib/nostr/nipcw.coax) | `cw_bounds_d`, `cw_bounds_content` | The 39006 bounds overlay's `d` tag and content |
| [nipcw.coax](../../coax/lib/nostr/nipcw.coax) | `cw_read_bounds`, `cw_view` | What a client may show from a response |
| [nipdv.coax](../../coax/lib/nostr/nipdv.coax) | `dv_command`, `dv_snapshot` | Hide and re-open, and the 30622 snapshot's tags |
| [nipdv.coax](../../coax/lib/nostr/nipdv.coax) | `dv_query`, `dv_deliver`, `dv_client_kind` | The read gates and the write protection |
| [nipdv.coax](../../coax/lib/nostr/nipdv.coax) | `dv_hidden`, `dv_visible` | The client's reading of its snapshot and its filtered DM list |
| [nipwp.coax](../../coax/lib/nostr/nipwp.coax) | `wp_admit(e, role, out)` | One 9033 workspace icon command |

All four library files start with `#authority;`: each decides admission or
who may read what.

## Channel identity

NIP-29 tells a client a group's name and flags, but not who created it,
when, or what kind of channel it is. NIP-CL adds those facts to every
channel-state event the relay signs, as tags in a fixed order before any
other `t` or `P` tag:

```jai
["t", <type>], ["t", <label>]..., ["P", <creator>], ["created_at", <unix seconds>]
```

`created_at` appears on 39000 only. The type is one of five names, and the
first four are the *legacy types*, the ones every client knew before
NIP-CL:

<!-- source: coax/lib/nostr/nipcl.coax types -->
```jai
CL_TYPES :: [5]string.["stream","forum","dm","workflow","system"];
CL_LEGACY :: 4;   // the first four: the types every client knew before NIP-CL
```

The relay keeps these facts beside the group's NIP-29 state. A client reads
the same structure back, with `has_*` false for a fact the event does not
carry:

<!-- source: coax/lib/nostr/nipcl.coax state -->
```jai
// What the relay records for a channel beside its NIP-29 state, and what a
// client reads back from a 39000 it signed. has_* false means unknown.
ClChannel :: struct {
  status:int=CL_OK; id:G29Name;
  has_type:bool; type:G29Name;
  labels:[CL_MAX_LABELS]G29Name; labels_n:int;
  has_creator:bool; creator:[32]u8;
  has_created:bool; created:int;
}
ClLabels :: struct {status:int=CL_OK; mode:int=CL_KEEP; labels:[CL_MAX_LABELS]G29Name; n:int;}
```

### Labels

A label lets an agent find its own channels again, such as "the config
channel this agent created". A label is 1 to 64 bytes from `a-z`, `0-9`,
`.`, `:` and `-`, and never a type name, so a label cannot make a channel
look like another type:

<!-- source: coax/lib/nostr/nipcl.coax one-label -->
```jai
// One label value: 1 to 64 bytes from a-z, 0-9, '.', ':' and '-', and no type name.
cl_label :: (t:G29Name)->int {
  c:=t;if c.n<1 || c.n>CL_MAX_LABEL {return CL_LABEL;}
  for i:0..<c.n {
    b:=c.b[i];
    ok:=(b>='a' && b<='z') || (b>='0' && b<=':') || b=='.' || b=='-';   // ':' follows '9'
    if !ok {return CL_LABEL;}
  }
  if cl_type_index(t)>=0 {return CL_RESERVED;}
  return CL_OK;
}
```

On a create (9007) or edit (9002), the `t` tags carry the labels. No `t`
tag keeps the set, `["t", ""]` alone clears it, and any other `t` tags
replace it. A repeat counts once, so it does not count toward the 8. Any
fault refuses the whole event:

<!-- source: coax/lib/nostr/nipcl.coax label-set -->
```jai
// The t tags of a create or edit command: keep (no t tag), clear (only
// ["t", ""]) or set (the distinct labels in order). Any fault refuses the
// whole event.
cl_labels :: (e:NostrEvent)->ClLabels {
  l:ClLabels;k:=0;cleared:=false;
  while true {
    t:=g29_tag(e,k);k+=1;
    if t.error==NT_INDEX {break;}
    if t.error!=NT_OK {return cl_fail(l,CL_SHAPE);}
    if !g29_tag_is(t,0,"t") {continue;}
    if t.values<2 || t.v[1].n>CL_MAX_LABEL {return cl_fail(l,CL_LABEL);}
    v:=g29_text(cast(string)t.v[1].b[0..<t.v[1].n]);
    if v.n==0 {cleared=true;continue;}
    s:=cl_label(v);if s!=CL_OK {return cl_fail(l,s);}
    seen:=false;for i:0..<l.n {if g29_same(l.labels[i],v) {seen=true;}}
    if seen {continue;}
    if l.n==CL_MAX_LABELS {return cl_fail(l,CL_TOO_MANY);}
    l.labels[l.n]=v;l.n+=1;
  }
  if cleared && l.n>0 {return cl_fail(l,CL_MIXED);}
  if cleared {l.mode=CL_CLEAR;} else if l.n>0 {l.mode=CL_SET;}
  return l;
}
```

### Creating and editing a channel

`cl_admit` checks the labels and the channel type first, and then passes
the event to `g29_admit`, which decides membership and permissions as in
chapter 59. So only an owner or admin can change labels: NIP-29 refuses a
member's 9002 before any label is applied. The creator and the creation
time come from the relay's own record of the 9007, never from a client
tag; `now` is the relay's clock, which the checks fix.

Where NIP-CL leaves a choice open, this relay decides: a 9007 without a
`channel_type` tag creates a `stream`, a `dm` is always private, and a
`system` channel is private unless the 9007 carries
`["visibility", "open"]`. A private channel also gets NIP-29's `hidden`
flag.

<!-- source: coax/lib/nostr/nipcl.coax admit -->
```jai
// A verified 9007 or 9002 for the channel c[0] and group g[0]: labels and
// type checked first, then NIP-29's decision. CL_GROUP: g29_admit refused,
// with its code in g29[0]. Other kinds go to g29_admit unchanged.
cl_admit :: (g:[]G29Group,c:[]ClChannel,e:NostrEvent,relay:[32]u8,now:int,g29:[]int)->int {
  g29[0]=G29_OK;
  if e.kind!=G29_CREATE_GROUP && e.kind!=G29_EDIT_METADATA {
    g29[0]=g29_admit(g,e,relay);return ifx g29[0]==G29_OK then CL_OK else CL_GROUP;
  }
  if e.error!=NE_OK || !e.verified {g29[0]=G29_EVENT;return CL_GROUP;}
  l:=cl_labels(e);if l.status!=CL_OK {return l.status;}
  type:=g29_text("stream");
  if e.kind==G29_CREATE_GROUP {
    given:=cl_value(e,"channel_type");
    if given.n>0 {type=given;}
    if cl_type_index(type)<0 {return CL_TYPE;}
  }
  g29[0]=g29_admit(g,e,relay);
  if g29[0]!=G29_OK {return CL_GROUP;}
  if e.kind==G29_CREATE_GROUP {
    fresh:ClChannel;c[0]=fresh;
    c[0].id=g[0].id;c[0].has_type=true;c[0].type=type;
    c[0].has_creator=true;c[0].creator=e.public_key;c[0].has_created=true;c[0].created=now;
    open:=g29_is(cl_value(e,"visibility"),"open");
    if g29_is(type,"dm") || (g29_is(type,"system") && !open) {g[0].is_private=true;g[0].hidden=true;}
  }
  if l.mode!=CL_KEEP {c[0].labels=l.labels;c[0].labels_n=l.n;}
  return CL_OK;
}
```

The relay writes NIP-29's metadata tags and then the identity tags:

<!-- source: coax/lib/nostr/nipcl.coax write -->
```jai
// The 39000 tags the relay signs: NIP-29's metadata, then the identity tags.
cl_write_metadata :: (g:G29Group,c:ClChannel,out:[]u8)->int {
  n:=g29_write_metadata(g,out)-1;   // drop its closing ]; -1 stays a failure
  n=json_raw(out,n,",[\"t\",");n=g29_put_name(out,n,c.type);n=json_raw(out,n,"]");
  for i:0..<c.labels_n {n=json_raw(out,n,",[\"t\",");n=g29_put_name(out,n,c.labels[i]);n=json_raw(out,n,"]");}
  n=json_raw(out,n,",[\"P\",");n=g29_put_key(out,n,c.creator);n=json_raw(out,n,"]");
  digits:[20]u8;d:=0;v:=c.created;
  while true {digits[19-d]=cast(u8)('0'+v%10);d+=1;v/=10;if v==0 {break;}}
  n=json_raw(out,n,",[\"created_at\",\"");n=json_raw(out,n,cast(string)digits[20-d..<20]);
  return json_raw(out,n,"\"]]");
}
```

### Reading a channel back

A client trusts these tags only on an event the relay's key signed, so
`cl_read` refuses any other signer. A missing tag stays unknown: a channel
published before the relay supported NIP-CL has no `P`, and that does not
mean that some key did not create it. A malformed tag, such as a `P` that
is not 64 lowercase hex digits or a `created_at` with a leading zero,
refuses.

<!-- source: coax/lib/nostr/nipcl.coax read -->
```jai
// A channel-state event read back by a client. It must be a 39000 to 39002
// signed by the relay's key. A missing tag stays unknown; a malformed one
// refuses.
cl_read :: (e:NostrEvent,relay:[32]u8)->ClChannel {
  c:ClChannel;
  if e.error!=NE_OK || !e.verified {c.status=CL_SHAPE;return c;}
  if e.kind<G29_METADATA || e.kind>G29_MEMBERS {c.status=CL_KIND;return c;}
  if !ns_equal32(e.public_key,relay) {c.status=CL_SIGNER;return c;}
  k:=0;
  while true {
    t:=g29_tag(e,k);k+=1;
    if t.error==NT_INDEX {return c;}
    if t.error!=NT_OK {c.status=CL_SHAPE;return c;}
    // A valueless P or created_at fails its own reading below.
    if (g29_tag_is(t,0,"d") || g29_tag_is(t,0,"t")) && t.values<2 {c.status=CL_SHAPE;return c;}
    one:[1]G29Name;
    if g29_tag_is(t,0,"d") {if g29_name(t.v[1],one[..])!=G29_OK {c.status=CL_SHAPE;return c;}c.id=one[0];}
    else if g29_tag_is(t,0,"t") {
      if g29_name(t.v[1],one[..])!=G29_OK {c.status=CL_SHAPE;return c;}
      if !c.has_type {c.type=one[0];c.has_type=true;}
      else {if c.labels_n==CL_MAX_LABELS {c.status=CL_TOO_MANY;return c;}c.labels[c.labels_n]=one[0];c.labels_n+=1;}
    }
    else if g29_tag_is(t,0,"P") {if !g29_key(t.v[1],c.creator[..]) {c.status=CL_SHAPE;return c;}c.has_creator=true;}
    else if g29_tag_is(t,0,"created_at") && e.kind==G29_METADATA {
      x:[1]int;if !cl_decimal(t.v[1],x[..]) {c.status=CL_SHAPE;return c;}
      c.created=x[0];c.has_created=true;
    }
  }
  return c;
}
```

### Choosing one channel

Labels are not unique. Two processes of one agent can both create
`agent-config` at the same moment, and both creates succeed. Every reader
must then pick the same one: the lowest `created_at` tag; on a tie, the
lowest channel id compared as a string; and a channel without a
`created_at` tag ranks after every channel with one. The event's own
`created_at` changes on every edit, so it is never used.

<!-- source: coax/lib/nostr/nipcl.coax oldest -->
```jai
// Does a rank before b? Lower created_at first; a known time before an
// unknown one; then the lower channel id.
cl_before :: (a:ClChannel,b:ClChannel)->bool {
  if a.has_created!=b.has_created {return a.has_created;}
  if a.has_created {if a.created<b.created {return true;}if a.created>b.created {return false;}}
  return cl_id_before(a.id,b.id);
}
// The index of the oldest channel, or -1 for none.
cl_oldest :: (cs:[]ClChannel)->int {
  best:= -1;
  for i:0..<cs.count {if best<0 || cl_before(cs[i],cs[best]) {best=i;}}
  return best;
}
```

### System channels and what a read lists

A system channel exists only to control who can read some data. If it
appeared in every member's sidebar and unread counts, it would be noise.
So the relay leaves it out of every read that does not name it. More
generally, a read that does not name a channel returns only the four legacy
types, so a type added later stays hidden from clients that do not know
it. A filter on kinds 39000 to 39003 with `#t` `system` or any `#P` opts
into system channels, and only system channels. A filter that names the
channel, by `#h` or by `#d` on 39000 to 39003, reads it under the ordinary
rule: a member, or an open channel.

Membership notices (44100 added, 44101 removed) follow the same idea. A
filter with `#t` matches a notice of one of those types; a filter without
`#t` matches only legacy types; and a notice stored before this rule has
no `t` tag and counts as every legacy type.

<!-- source: coax/lib/nostr/nipcl.coax listed -->
```jai
// A read filter, as far as NIP-CL's visibility rule needs it.
ClFilter :: struct {
  names:bool;           // #h has the channel id, or #d with kinds all in 39000-39003
  state_only:bool;      // every kind in the filter is 39000-39003
  t:[CL_MAX_LABELS]G29Name; t_n:int;
  has_P:bool;
}
cl_has_t :: (f:ClFilter,s:string)->bool {for i:0..<f.t_n {if g29_is(f.t[i],s) {return true;}}return false;}
// Does this filter return the channel to a reader? member: the reader is
// in the channel's member list. Matching the filter's own tags is NIP-01's.
cl_listed :: (g:G29Group,c:ClChannel,f:ClFilter,member:bool)->bool {
  readable:=member || !g.is_private;
  if f.names {return readable;}
  i:=cl_type_index(c.type);
  if i>=0 && i<CL_LEGACY {return readable;}
  if g29_is(c.type,"system") && f.state_only && (cl_has_t(f,"system") || f.has_P) {return readable;}
  return false;
}
// Does a 44100 or 44101 notice reach a filter? has_type false: a notice
// stored before the rule, which is about a legacy type.
cl_notice :: (has_type:bool,type:G29Name,f:ClFilter)->bool {
  if !has_type {
    if f.t_n==0 {return true;}
    for i:0..<CL_LEGACY {if cl_has_t(f,CL_TYPES[i]) {return true;}}
    return false;
  }
  if f.t_n>0 {c:=type;return cl_has_t(f,cast(string)c.b[0..<c.n]);}
  i:=cl_type_index(type);return i>=0 && i<CL_LEGACY;
}
```

A `dm` is private, so `cl_listed` never returns it to a reader who is not
a participant, even to a filter that names it.

## Channel windows

A NIP-01 filter can match a tag but cannot express its absence, so
"messages in this channel that are not replies" has no filter. A client
that pages the raw stream gets pages of 50 events that hold anywhere from
0 to 50 top-level messages. And a timestamp cursor (`until`) loses or
repeats messages at a page boundary whenever several share one second.
NIP-CW lets the relay, which classifies every event as it arrives, serve
the top-level timeline directly, with a composite cursor and an exact end.

### Which events are top-level

An event is a reply only when it carries an `e` tag marked `reply`. A
root-only `e` tag, a positional one or none at all is not a reply. A
reply's depth is its parent's depth plus one, at most 100; the parent must
already be stored in the channel, and a `root`-marked tag must name the
parent's thread root. `["broadcast", "1"]` exactly puts a depth-1 reply on
the channel timeline as well as in its thread.

<!-- source: coax/lib/nostr/nipcw.coax classify -->
```jai
// An event's thread tags: the reply-marked e tag (at most one), the
// root-marked one, and ["broadcast", "1"] exactly.
cw_tags :: (e:NostrEvent)->CwTags {
  r:CwTags;k:=0;
  while true {
    t:=nostr_tag(e,k);g:=g29_tag(e,k);k+=1;
    if t.error==NT_INDEX {return r;}
    if t.error!=NT_OK {r.status=CW_SHAPE;return r;}
    if t.type==NT_E && t.marker==NT_MARK_REPLY {
      if r.reply {r.status=CW_SHAPE;return r;}
      r.reply=true;r.parent=t.id;
    }
    if t.type==NT_E && t.marker==NT_MARK_ROOT {r.has_root=true;r.root=t.id;}
    if g29_tag_is(g,0,"broadcast") && g.values==2 && g29_tag_is(g,1,"1") {r.broadcast=true;}
  }
  return r;
}
```

<!-- source: coax/lib/nostr/nipcw.coax ingest -->
```jai
// Store one verified event of the channel in rows[0..<n[0]].
cw_ingest :: (rows:[]CwRow,n:[]int,e:NostrEvent)->int {
  if e.error!=NE_OK || !e.verified {return CW_EVENT;}
  if e.kind>=39005 && e.kind<=39007 {return CW_RELAY_ONLY;}
  t:=cw_tags(e);if t.status!=CW_OK {return t.status;}
  r:CwRow;r.created_at=e.created_at;r.id=e.id;r.kind=e.kind;r.broadcast=t.broadcast;
  if t.reply {
    p:=cw_find(rows,n[0],t.parent);if p<0 {return CW_PARENT;}
    r.depth=rows[p].depth+1;
    if r.depth>CW_MAX_DEPTH {return CW_DEPTH;}
    r.parent=t.parent;r.root=ifx rows[p].depth==0 then rows[p].id else rows[p].root;
    if t.has_root && !ns_equal32(t.root,r.root) {return CW_ROOT;}
  }
  if n[0]==rows.count {return CW_FULL;}
  rows[n[0]]=r;n[0]+=1;return CW_OK;
}
```

The relay stores each event as a row. A row stored before the relay
classified events has depth `CW_UNKNOWN` (−1) and counts as top-level, so
it never vanishes from every window:

<!-- source: coax/lib/nostr/nipcw.coax row -->
```jai
// One stored channel event, as the window reads it.
CwRow :: struct {
  created_at:int; id:[32]u8; kind:int;
  depth:int; broadcast:bool; deleted:bool;
  parent:[32]u8; root:[32]u8;     // for a reply: its parent and its thread's root
}
```

### The request

A window request names exactly one channel. Its cursor is two fields,
`until` and `before_id`, sent together or not at all: a timestamp alone is
the lossy cursor NIP-CW exists to remove, so half a cursor refuses. A
malformed cursor also refuses rather than becoming a head request. Channel
mode clamps `limit` into 1 to 200 (default 50) and records that it did;
thread mode refuses a value outside 1 to 200, a `depth_limit` outside 1 to
100, and kinds other than one to four of 9, 40002, 45001 and 45003, which
it sorts and deduplicates.

<!-- source: coax/lib/nostr/nipcw.coax request -->
```jai
CwRequest :: struct {
  thread:bool;                          // thread_window: true, else top_level: true
  channels:int;                         // values in #h
  roots:int; root:[32]u8;               // values in #e (thread mode)
  has_limit:bool; limit:int;
  has_depth:bool; depth_limit:int;
  kinds:[4]int; kinds_n:int;
  has_until:bool; until:int;
  has_before:bool; before:string;       // 64 hex digits as sent
}
CwPage :: struct {
  status:int=CW_OK; served:bool;
  limit:int; clamped:bool;              // clamped: channel mode moved limit into 1..200
  depth_limit:int; kinds:[4]int; kinds_n:int;
  head:bool; at:int; after:[32]u8;      // the request cursor
  n:int; has_more:bool; next_at:int; next_id:[32]u8;
}
```

<!-- source: coax/lib/nostr/nipcw.coax check -->
```jai
cw_check :: (r:CwRequest)->CwPage {
  p:CwPage;p.head=true;
  if r.channels!=1 {return cw_refuse(p,CW_CHANNELS);}
  if r.has_until!=r.has_before {return cw_refuse(p,CW_CURSOR);}
  if r.has_until {
    if r.until<0 || r.before.count!=64 {return cw_refuse(p,CW_CURSOR);}
    for i:0..<32 {
      hi:=cw_nibble(r.before[2*i]);lo:=cw_nibble(r.before[2*i+1]);
      if hi<0 || lo<0 {return cw_refuse(p,CW_CURSOR);}
      p.after[i]=cast(u8)(hi*16+lo);
    }
    p.head=false;p.at=r.until;
  }
  p.limit=ifx r.has_limit then r.limit else CW_DEFAULT_LIMIT;
  if !r.thread {
    if p.limit<1 {p.limit=1;p.clamped=true;}
    if p.limit>CW_MAX_LIMIT {p.limit=CW_MAX_LIMIT;p.clamped=true;}
    p.kinds=r.kinds;p.kinds_n=r.kinds_n;return p;
  }
  if r.roots!=1 {return cw_refuse(p,CW_ROOTS);}
  if p.limit<1 || p.limit>CW_MAX_LIMIT {return cw_refuse(p,CW_LIMIT);}
  p.depth_limit=ifx r.has_depth then r.depth_limit else CW_MAX_DEPTH;
  if p.depth_limit<1 || p.depth_limit>CW_MAX_DEPTH {return cw_refuse(p,CW_LIMIT);}
  if r.kinds_n<1 || r.kinds_n>4 {return cw_refuse(p,CW_KINDS);}
  // Sorted and distinct: walk the four allowed kinds in order.
  for k:CW_THREAD_KINDS {
    asked:=false;for i:0..<r.kinds_n {if r.kinds[i]==k {asked=true;}}
    if asked {p.kinds[p.kinds_n]=k;p.kinds_n+=1;}
  }
  for i:0..<r.kinds_n {if !cw_kind_in(CW_THREAD_KINDS,4,r.kinds[i]) {return cw_refuse(p,CW_KINDS);}}
  return p;
}
```

### The order and the page

The total order is `created_at` descending, then `id` ascending, compared
byte by byte. A row comes after a cursor `(ts, id)` when its `created_at`
is less than `ts`, or equal with a greater id. Getting that inequality
backwards drops or repeats the rows of one second.

<!-- source: coax/lib/nostr/nipcw.coax order -->
```jai
// Is id a before id b, bytewise?
cw_id_less :: (a:[32]u8,b:[32]u8)->bool {
  for i:0..<32 {if a[i]<b[i] {return true;}if a[i]>b[i] {return false;}}
  return false;
}
// Does row r come after the position (at, id) in created_at DESC, id ASC?
cw_after :: (r:CwRow,at:int,id:[32]u8)->bool {
  if r.created_at<at {return true;}
  if r.created_at>at {return false;}
  return cw_id_less(id,r.id);
}
// Does a come before b in the window's order?
cw_first :: (a:CwRow,b:CwRow)->bool {return cw_after(b,a.created_at,a.id);}
```

<!-- source: coax/lib/nostr/nipcw.coax eligible -->
```jai
// Is the row one this window may return?
cw_eligible :: (r:CwRow,q:CwRequest,p:CwPage)->bool {
  if r.deleted {return false;}
  if p.kinds_n>0 && !cw_kind_in(p.kinds,p.kinds_n,r.kind) {return false;}
  if q.thread {return r.depth>=1 && r.depth<=p.depth_limit && ns_equal32(r.root,q.root);}
  return r.depth<=0 || (r.depth==1 && r.broadcast);
}
```

`cw_window` takes the next row in the order, again and again, after every
predicate (access, deletion, kinds and the top-level or thread rule). It
probes one row past the limit: if that row exists, `has_more` is true and
the row is dropped; otherwise `has_more` is false. So a final page of
exactly `limit` rows says `has_more` false, and a client never asks for an
empty page to find out. The next cursor is the last row of the page.

A reader who cannot read the channel gets no rows and no bounds at all, the
same as for a channel that does not exist. A thread window whose root is
missing, is itself a reply, or is not a conversation kind is not served
either.

<!-- source: coax/lib/nostr/nipcw.coax window -->
```jai
// The page for request q against the channel's rows[0..<n]: row indexes in
// out. readable: the reader may read the channel (a member, or it is open).
cw_window :: (rows:[]CwRow,n:int,q:CwRequest,readable:bool,out:[]int)->CwPage {
  p:=cw_check(q);if p.status!=CW_OK || !readable {return p;}
  if q.thread {
    root:=cw_find(rows,n,q.root);
    if root<0 || rows[root].depth!=0 || !cw_kind_in(CW_THREAD_KINDS,4,rows[root].kind) {return p;}
  }
  p.served=true;
  at:=p.at;id:=p.after;
  k:=0;
  while true {
    best:= -1;
    for i:0..<n {
      if !cw_eligible(rows[i],q,p) {continue;}
      if !p.head || k>0 {if !cw_after(rows[i],at,id) {continue;}}
      if best<0 || cw_first(rows[i],rows[best]) {best=i;}
    }
    if best<0 {break;}
    if k==p.limit {p.has_more=true;p.next_at=at;p.next_id=id;break;}
    out[k]=best;p.n=k+1;at=rows[best].created_at;id=rows[best].id;k+=1;
  }
  return p;
}
```

### Reactions, deletions and edits

With `include_aux`, a page also carries the events that change how its
rows look: reactions (7), deletions (5 and 9005) and edits (40003) whose
`e` tag names a returned row, and then the deletions of those, such as a
deleted reaction. They don't count against `limit`. A deleted row itself
leaves every window (`cw_delete`).

<!-- source: coax/lib/nostr/nipcw.coax aux -->
```jai
// Auxiliary events: one per stored reaction, deletion or edit, with the id
// its e tag names.
CwAux :: struct {id:[32]u8; kind:int; target:[32]u8;}
cw_aux_kind :: (k:int)->bool {return k==7 || k==5 || k==9005 || k==40003;}
cw_is_delete :: (k:int)->bool {return k==5 || k==9005;}
// Hop 1: aux events naming a returned row; hop 2: deletions naming a hop-1
// event. Indexes into aux in out; the count. An event names one target, a
// row or a hop-1 event, so it appears at most once.
cw_aux :: (rows:[]CwRow,page:[]int,aux:[]CwAux,out:[]int)->int {
  m:=0;
  for i:0..<aux.count {
    if !cw_aux_kind(aux[i].kind) {continue;}
    for r:page {if ns_equal32(aux[i].target,rows[r].id) {out[m]=i;m+=1;break;}}
  }
  hop1:=m;
  for i:0..<aux.count {
    if !cw_is_delete(aux[i].kind) {continue;}
    for j:0..<hop1 {if ns_equal32(aux[i].target,aux[out[j]].id) {out[m]=i;m+=1;break;}}
  }
  return m;
}
```

### The bounds overlay

Every served window ends with exactly one relay-signed 39006. Its `d` tag
binds it to the request's cursor (`<channel>:head`, or
`<channel>:<created_at>:<id>`), and its content carries `has_more` and the
next cursor, `null` exactly when `has_more` is false:

<!-- source: coax/lib/nostr/nipcw.coax bounds -->
```jai
// The 39006 d tag: "<channel>:head" or "<channel>:<created_at>:<id hex>".
cw_put_int :: (out:[]u8,at:int,v:int)->int {
  digits:[20]u8;d:=0;x:=v;
  while true {digits[19-d]=cast(u8)('0'+x%10);d+=1;x/=10;if x==0 {break;}}
  return json_raw(out,at,cast(string)digits[20-d..<20]);
}
cw_put_hex :: (out:[]u8,at:int,id:[32]u8)->int {
  hex:[64]u8;d:="0123456789abcdef";
  for i:0..<32 {hex[2*i]=d[id[i]>>4];hex[2*i+1]=d[id[i]&15];}
  return json_raw(out,at,cast(string)hex[..]);
}
cw_bounds_d :: (channel:string,p:CwPage,out:[]u8)->int {
  n:=json_raw(out,0,channel);n=json_raw(out,n,":");
  if p.head {return json_raw(out,n,"head");}
  n=cw_put_int(out,n,p.at);n=json_raw(out,n,":");return cw_put_hex(out,n,p.after);
}
// The 39006 content: has_more and next_cursor, null exactly when has_more is false.
cw_bounds_content :: (p:CwPage,out:[]u8)->int {
  if !p.has_more {return json_raw(out,0,"{\"has_more\":false,\"next_cursor\":null}");}
  n:=json_raw(out,0,"{\"has_more\":true,\"next_cursor\":{\"created_at\":");n=cw_put_int(out,n,p.next_at);
  n=json_raw(out,n,",\"id\":\"");n=cw_put_hex(out,n,p.next_id);return json_raw(out,n,"\"}}");
}
```

### What a client shows

The 39006 is the only authority on the end of a channel. A response
without one is not a page: either the reader cannot read the channel, or
the relay does not implement NIP-CW. A client must fall back to a plain
filter, and it must never show that response as an empty channel. A
response with two overlays, an overlay bound to another cursor, or content
that breaks the `has_more` and `next_cursor` agreement is discarded. Only
a served head page with no rows and `has_more` false is an empty channel.

<!-- source: coax/lib/nostr/nipcw.coax view -->
```jai
// A 39006 as a client reads it: the d tag and the content's two fields.
CwBounds :: struct {ok:bool; d:string; has_more:bool; next:bool; next_at:int; next_id:[32]u8;}
// Read a 39006's content strictly: an object with has_more (a boolean) and
// next_cursor (null, or an object with an integer created_at and a 64-hex
// id); unknown fields are skipped.
cw_read_bounds :: (d:string,content:string)->CwBounds {
  b:CwBounds;b.d=d;j:[1]Json;json_start(j[..],content);
  if json_next(j[..])!=JSON_OBJECT {return b;}
  seen:=0;tok:=json_next(j[..]);
  while tok==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);
    if json_is(key,"has_more") {
      if v!=JSON_TRUE && v!=JSON_FALSE {return b;}
      b.has_more=v==JSON_TRUE;seen|=1;
    } else if json_is(key,"next_cursor") {
      seen|=2;
      if v==JSON_OBJECT {
        b.next=true;have:=0;f:=json_next(j[..]);
        while f==JSON_STRING {
          name:=json_text(j[..]);x:=json_next(j[..]);
          if json_is(name,"created_at") {
            if x!=JSON_NUMBER {return b;}
            i:=json_int(json_text(j[..]));if !i.ok || i.value<0 {return b;}
            b.next_at=i.value;have|=1;
          } else if json_is(name,"id") {
            hex:[64]u8;if x!=JSON_STRING || json_unescape(json_text(j[..]),hex[..])!=64 || !ne_lower_hex(cast(string)hex[..],b.next_id[..]) {return b;}
            have|=2;
          } else if !json_skip(j[..]) {return b;}
          f=json_next(j[..]);
        }
        if f!=JSON_OBJECT_END || have!=3 {return b;}
      } else if v!=JSON_NULL {return b;}
    } else if !json_skip(j[..]) {return b;}
    tok=json_next(j[..]);
  }
  b.ok=tok==JSON_OBJECT_END && json_next(j[..])==JSON_END && seen==3 && b.has_more==b.next;
  return b;
}
// What a client shows: bounds holds the 39006 overlays the response carried,
// sent is the d tag the request's cursor binds, rows the rows it carried.
cw_view :: (bounds:[]CwBounds,sent:string,head:bool,rows:int)->int {
  if bounds.count==0 {return CW_VIEW_NONE;}
  if bounds.count>1 || !bounds[0].ok || bounds[0].d!=sent {return CW_VIEW_DISCARD;}
  if head && rows==0 && !bounds[0].has_more {return CW_VIEW_EMPTY;}
  return CW_VIEW_PAGE;
}
```

## DM visibility

Hiding a DM is presentation, not membership: the viewer still receives its
messages and can re-open it. So the 39002 member list keeps the viewer, and
a client that rebuilds its DM list from 39002 alone shows a hidden DM
again on the next refresh. NIP-DV publishes the relay's private hide state
as one relay-signed 30622 per viewer, the whole hidden set each time.

<!-- source: coax/lib/nostr/nipdv.coax state -->
```jai
// One viewer's membership of one channel, as the relay records it.
DvMember :: struct {viewer:[32]u8; channel:G29Name; dm:bool; active:bool; hidden:bool;}
```

A hide (41012) or re-open (41010) changes an active DM membership; the
relay then recomputes the snapshot from the whole state:

<!-- source: coax/lib/nostr/nipdv.coax command -->
```jai
// Apply a verified hide (41012) or open (41010) by viewer for channel.
// DV_OK: the hidden set changed, so publish the snapshot again.
dv_command :: (ms:[]DvMember,viewer:[32]u8,kind:int,channel:G29Name)->int {
  if kind!=DV_HIDE && kind!=DV_OPEN {return DV_KIND;}
  for i:0..<ms.count {
    if !ns_equal32(ms[i].viewer,viewer) || !g29_same(ms[i].channel,channel) {continue;}
    if !ms[i].dm || !ms[i].active {return DV_NOT_MEMBER;}
    hide:=kind==DV_HIDE;
    if ms[i].hidden==hide {return DV_UNCHANGED;}
    ms[i].hidden=hide;return DV_OK;
  }
  return DV_NOT_MEMBER;
}
```

<!-- source: coax/lib/nostr/nipdv.coax snapshot -->
```jai
// The 30622 tags for viewer: d, p, then each hidden active DM.
dv_snapshot :: (ms:[]DvMember,viewer:[32]u8,out:[]u8)->int {
  n:=json_raw(out,0,"[[\"d\",");n=g29_put_key(out,n,viewer);
  n=json_raw(out,n,"],[\"p\",");n=g29_put_key(out,n,viewer);n=json_raw(out,n,"]");
  for m:ms {
    if !ns_equal32(m.viewer,viewer) || !m.dm || !m.active || !m.hidden {continue;}
    n=json_raw(out,n,",[\"h\",");n=g29_put_name(out,n,m.channel);n=json_raw(out,n,"]");
  }
  return json_raw(out,n,"]");
}
```

A snapshot names the DMs its owner chose to hide, and its id is not bound
to the owner's key, so knowing the id is no permission. Two gates keep it
private: a filter that could return a 30622 must have `#p` equal to the
reader alone, and every delivery path hands a 30622 only to the key in its
`p` tag, whatever the filter. A client can never submit one.

<!-- source: coax/lib/nostr/nipdv.coax gates -->
```jai
// A read filter as the 30622 gate sees it: whether it can return a 30622
// (its kinds include 30622, or it has no kinds), and its #p values.
DvQuery :: struct {may_return:bool; p:[4][32]u8; p_n:int;}
// The filter gate: #p must be the reader's key and nothing else.
dv_query :: (q:DvQuery,reader:[32]u8)->bool {
  if !q.may_return {return true;}
  return q.p_n==1 && ns_equal32(q.p[0],reader);
}
// The result gate, on every delivery path: a 30622 goes only to its owner.
dv_deliver :: (e:NostrEvent,reader:[32]u8)->bool {
  if e.kind!=DV_SNAPSHOT {return true;}
  r:=g29_read(e);
  return r.status==G29_OK && r.has_p && ns_equal32(r.p,reader);
}
// May a client submit an event of this kind? Never a snapshot.
dv_client_kind :: (kind:int)->bool {return kind!=DV_SNAPSHOT;}
```

The client reads its own snapshot, checks the relay's key and that `d` and
`p` both name the viewer, and drops those DMs from its list. A non-DM
channel is never dropped, and no snapshot means nothing is hidden.

<!-- source: coax/lib/nostr/nipdv.coax client -->
```jai
// A d or p tag of the snapshot: it must name the viewer.
dv_viewer_tag :: (t:G29Tag,viewer:[32]u8)->int {
  key:[32]u8;
  if t.values<2 || !g29_key(t.v[1],key[..]) {return DV_SHAPE;}
  return ifx ns_equal32(key,viewer) then DV_OK else DV_VIEWER;
}
// The hidden channel ids of viewer's snapshot e, into out: their count, or
// a negative status. e must be a verified 30622 signed by the relay, with
// exactly one d and one p, both the viewer. h tags are a set.
dv_hidden :: (e:NostrEvent,relay:[32]u8,viewer:[32]u8,out:[]G29Name)->int {
  if e.error!=NE_OK || !e.verified {return -DV_EVENT;}
  if e.kind!=DV_SNAPSHOT {return -DV_KIND;}
  if !ns_equal32(e.public_key,relay) {return -DV_SIGNER;}
  n:=0;ds:=0;ps:=0;k:=0;
  while true {
    t:=g29_tag(e,k);k+=1;
    if t.error==NT_INDEX {break;}
    if t.error!=NT_OK {return -DV_SHAPE;}
    if g29_tag_is(t,0,"d") {s:=dv_viewer_tag(t,viewer);if s!=DV_OK {return -s;}ds+=1;}
    else if g29_tag_is(t,0,"p") {s:=dv_viewer_tag(t,viewer);if s!=DV_OK {return -s;}ps+=1;}
    else if g29_tag_is(t,0,"h") {
      one:[1]G29Name;if t.values<2 || t.v[1].n==0 || g29_name(t.v[1],one[..])!=G29_OK {return -DV_SHAPE;}
      seen:=false;for i:0..<n {if g29_same(out[i],one[0]) {seen=true;}}
      if seen {continue;}
      if n==out.count {return -DV_LIMIT;}
      out[n]=one[0];n+=1;
    }
  }
  if ds!=1 || ps!=1 {return -DV_SHAPE;}
  return n;
}
// One entry of the client's channel list.
DvItem :: struct {channel:G29Name; dm:bool;}
// The indexes of the items still shown: every non-DM, and every DM not hidden.
dv_visible :: (items:[]DvItem,hidden:[]G29Name,out:[]int)->int {
  m:=0;
  for i:0..<items.count {
    drop:=false;
    if items[i].dm {for h:hidden {if g29_same(h,items[i].channel) {drop=true;}}}
    if !drop {out[m]=i;m+=1;}
  }
  return m;
}
```

## The workspace icon

On Buzz the relay is the workspace. NIP-WP lets an admin or owner set its
icon with a 9033 command; the relay serves the last accepted value in its
NIP-11 document. Every member's client renders that value as an image, so
the relay checks it on the way in: `https` or `http` up to 2048 bytes, or
`data:image/` up to 96 KiB, with no whitespace or control bytes. A
`javascript:` or non-image `data:` URL refuses.

<!-- source: coax/lib/nostr/nipwp.coax icon -->
```jai
WpIcon :: struct {status:int=WP_OK; clear:bool; n:int;}   // the value is out[0..<n]
wp_starts :: (s:string,p:string)->bool {return s.count>=p.count && s[0..<p.count]==p;}
// An icon value: scheme, bytes, size.
wp_value :: (s:string)->int {
  for c:s {if c<=32 || c==127 {return WP_BYTES;}}
  if wp_starts(s,"https://") || wp_starts(s,"http://") {return ifx s.count<=WP_MAX_URL then WP_OK else WP_SIZE;}
  if wp_starts(s,"data:image/") {return ifx s.count<=WP_MAX_DATA then WP_OK else WP_SIZE;}
  return WP_SCHEME;
}
```

<!-- source: coax/lib/nostr/nipwp.coax admit -->
```jai
// Decide a verified 9033 from a signer holding role; the icon is decoded
// into out (at least WP_MAX_DATA bytes for the largest accepted value).
wp_admit :: (e:NostrEvent,role:int,out:[]u8)->WpIcon {
  r:WpIcon;
  if e.error!=NE_OK || !e.verified {r.status=WP_EVENT;return r;}
  if e.kind!=WP_SET {r.status=WP_KIND;return r;}
  if role!=WP_ADMIN && role!=WP_OWNER {r.status=WP_ROLE;return r;}
  j:[1]Json;if !nt_start(j[..],e) {r.status=WP_SHAPE;return r;}
  icons:=0;tok:=json_next(j[..]);
  while tok==JSON_ARRAY {
    v:=json_next(j[..]);at:=0;icon:=false;
    while v==JSON_STRING {
      if at==0 && json_is(json_text(j[..]),"icon") {icons+=1;icon=true;}
      if at==1 && icon {
        n:=json_unescape(json_text(j[..]),out);if n<0 {r.status=WP_SIZE;return r;}
        r.n=n;
      }
      at+=1;v=json_next(j[..]);
    }
    if v!=JSON_ARRAY_END {r.status=WP_SHAPE;return r;}
    tok=json_next(j[..]);
  }
  if icons>1 {r.status=WP_SHAPE;return r;}
  if r.n==0 {r.clear=true;return r;}
  r.status=wp_value(cast(string)out[0..<r.n]);return r;
}
```

## Worked example: one channel paged by hand

Six messages arrive in `general` (`T0` = 1790000000). The ids are the
SHA-256 of each event's serialization, computed in Python; only their first
8 hex digits matter here.

| Event | Author | `created_at` | id | Tags |
| --- | --- | --- | --- | --- |
| m1 | Alice | T0 | `d14a53f2` | `h` |
| m2 | Bob | T0+60 | `1c1bfdde` | `h` |
| m3 | Alice | T0+60 | `31048ee3` | `h` |
| m4 | Bob | T0+60 | `cc85adb2` | `h` |
| r1 | Bob | T0+90 | `971b69e8` | `h`, root m3, reply m3 |
| r2 | Alice | T0+120 | `4b46ba5c` | `h`, root m3, reply m3, broadcast 1 |

1. Classify. m1 to m4 have no reply-marked `e` tag: depth 0, top-level. r1
   replies to m3: depth 1, not broadcast, so it is not top-level. r2
   replies to m3 with `["broadcast", "1"]`: depth 1 and broadcast, so it
   is top-level. Five rows are eligible.
2. Order. r2 (T0+120) first. Then the three at T0+60 by id ascending:
   `1c1b` < `3104` < `cc85`, so m2, m3, m4. Then m1 (T0).
3. Page 1, head, limit 2. Take r2 and m2; probe a third row: m3 exists, so
   `has_more` is true and m3 is dropped. The next cursor is the last row,
   (T0+60, `1c1bfdde…`). The overlay's `d` is `general:head`.
4. Page 2, cursor (T0+60, `1c1b…`). A row comes after it when its
   `created_at` is less than T0+60, or equal with an id greater than
   `1c1b…`: m3, m4 and m1. Take m3 and m4; probe: m1 exists, so `has_more`
   is true, next cursor (T0+60, `cc85adb2…`). With `until` alone, m3 and
   m4 (same second as m2) would be lost or m2 repeated.
5. Page 3, cursor (T0+60, `cc85…`). After it: only m1. Take m1; probe:
   nothing, so `has_more` is false and the next cursor is `null`. The page
   has fewer rows than the limit, but only `has_more` says the channel has
   ended.
6. The thread window under m3 takes its replies, newest first: r2
   (T0+120), then r1 (T0+90). r2 is in both views; r1 only in the thread.

Then three reads of the same channel that must not show "empty": a reader
who cannot read `general` gets no rows and no 39006, so the client falls
back; a response whose 39006 answers another cursor is discarded; and only
a served head page of a channel with no messages, with
`{"has_more":false,"next_cursor":null}`, shows an empty channel.

## What is not built

- NIP-CW: the 39005 thread summaries (`reply_count`, `descendant_count`,
  `last_reply_at`, `participants`); the 39007 thread bounds and their
  SHA-256 request binding; the legacy oldest-first thread path; thread
  resource limits and the retryable access-change error; summary recovery
  after deletion (`resolve_thread_roots`); signing the overlays with the
  relay's key; the HTTP `POST /query` surface. Rows are a fixed array
  searched in full, not an index.
- NIP-CL: the `unlisted` setting, the push-notification (NIP-PL) lease
  rule, NIP-11 `supported_extensions`, COUNT and NIP-50 search paths, and
  parsing filters from JSON (`ClFilter` is filled by the caller).
- NIP-DV: storing and replacing the snapshot event, and signing it.
- NIP-WP: serving the icon in NIP-11 and the relay's access-control state
  (the caller passes the signer's role).

## The example

[nostr/examples/channels.coax](../examples/channels.coax) runs the worked
example at a fixed clock. The events were signed by an independent BIP-340
in Python ([channels_vectors.py](../../coax/tests/nostr/channels_vectors.py)),
and the example verifies each one before using it.

<!-- source: nostr/examples/channels.coax keys -->
```jai
key_of :: (hex:string)->[32]u8 {k:[32]u8;ok:=ne_lower_hex(hex,k[..]);return k;}
// A signed event, read and verified (chapter 21).
verified :: (text:string,out:[]u8)->NostrEvent {return nostr_event_verify_vartime(text,out,false);}
id8 :: (id:[32]u8,out:[]u8)->string {n:=cw_put_hex(out,0,id);return cast(string)out[0..<8];}
```

The channel: Alice's create, the relay's 39000, two refused creates, a
system channel and the oldest-channel rule:

<!-- source: nostr/examples/channels.coax channel -->
```jai
show_channel :: (given:ClChannel) {
  c:=given;
  labels:[256]u8;n:=0;
  for i:0..<c.labels_n {if i>0 {n=json_raw(labels[..],n,",");}n=json_raw(labels[..],n,cast(string)c.labels[i].b[0..<c.labels[i].n]);}
  h:[64]u8;
  print("  type %, labels [%], creator %…, created_at %\n",cast(string)c.type.b[0..<c.type.n],cast(string)labels[0..<n],id8(c.creator,h[..]),c.created);
}
part_cl :: (relay:[32]u8) {
  print("NIP-CL: Alice creates general with the label team\n");
  g:[1]G29Group;c:[1]ClChannel;n:[1]int;out:[16384]u8;
  s:=cl_admit(g[..],c[..],verified(CHV_CREATE,out[..]),relay,T0,n[..]);
  print("create: %\n",CL_WORDS[s]);
  tags:[1024]u8;m:=cl_write_metadata(g[0],c[0],tags[..]);
  print("the relay's 39000 tags: %\n",cast(string)tags[0..<m]);
  e:=verified(CHV_META,out[..]);
  same:=cast(string)e.original[e.tags_from..<e.tags_to]==cast(string)tags[0..<m];
  print("the signed 39000 (%…) carries the same tags: %\n",CHV_META_ID[0..<8],same);
  print("a client reads it back:\n");show_channel(cl_read(e,relay));
  h:[1]G29Group;d:[1]ClChannel;
  print("Bob's create with the label \"Team Chat\": %\n",CL_WORDS[cl_admit(h[..],d[..],verified(CHV_CREATE_SPACE,out[..]),relay,T0,n[..])]);
  print("Bob's create with the label \"dm\": %\n",CL_WORDS[cl_admit(h[..],d[..],verified(CHV_CREATE_DM_LABEL,out[..]),relay,T0,n[..])]);
  print("the same 39000 signed by Bob instead of the relay: %\n",CL_WORDS[cl_read(e,key_of(CHV_BOB)).status]);
  // The system channel and who lists it.
  sg:[1]G29Group;sc:[1]ClChannel;
  s=cl_admit(sg[..],sc[..],verified(CHV_CREATE_SYSTEM,out[..]),relay,T0+5,n[..]);
  print("Alice creates the system channel agent-cfg: %, private %\n",CL_WORDS[s],sg[0].is_private);
  plain:ClFilter;plain.state_only=true;
  ask:=plain;ask.t[0]=g29_text("system");ask.t_n=1;
  by_p:=plain;by_p.has_P=true;
  print("a plain channel list shows general % and agent-cfg %\n",cl_listed(g[0],c[0],plain,false),cl_listed(sg[0],sc[0],plain,true));
  print("#t system shows agent-cfg to Alice %, to Bob %; #P shows it to Alice %\n",cl_listed(sg[0],sc[0],ask,true),cl_listed(sg[0],sc[0],ask,false),cl_listed(sg[0],sc[0],by_p,true));
  // Three channels share Alice's label team: every reader picks the same one.
  cs:=[3]ClChannel.[c[0],c[0],c[0]];
  cs[1].id=g29_text("chat-2");cs[1].created=T0-100;cs[2].id=g29_text("chat-1");cs[2].created=T0-100;
  print("oldest of general (%), chat-2 (%), chat-1 (%): %\n",cs[0].created,cs[1].created,cs[2].created,cast(string)cs[cl_oldest(cs[..])].id.b[0..<6]);
}
```

The window: six messages, three pages, the thread, and the broken inputs:

<!-- source: nostr/examples/channels.coax window -->
```jai
show_page :: (rows:[]CwRow,p:CwPage,out:[]int) {
  h:[64]u8;ids:[256]u8;n:=0;
  for i:0..<p.n {if i>0 {n=json_raw(ids[..],n,",");}n=json_raw(ids[..],n,id8(rows[out[i]].id,h[..]));}
  d:[160]u8;dn:=cw_bounds_d("general",p,d[..]);b:[200]u8;bn:=cw_bounds_content(p,b[..]);
  print("  rows [%]\n  39006 d %\n  39006 content %\n",cast(string)ids[0..<n],cast(string)d[0..<dn],cast(string)b[0..<bn]);
}
part_cw :: () {
  print("NIP-CW: six messages arrive in general\n");
  rows:[16]CwRow;n:[1]int;out:[16384]u8;
  texts:=[6]string.[CHV_M1,CHV_M2,CHV_M3,CHV_M4,CHV_R1,CHV_R2];
  names:=[6]string.["m1","m2","m3","m4","r1","r2"];
  h:[64]u8;
  for i:0..<6 {
    e:=verified(texts[i],out[..]);s:=cw_ingest(rows[..],n[..],e);
    r:=rows[n[0]-1];
    print("% %… at T0+%: %, depth %, broadcast %\n",names[i],id8(r.id,h[..]),r.created_at-T0,CW_WORDS[s],r.depth,r.broadcast);
  }
  q:CwRequest;q.channels=1;q.has_limit=true;q.limit=2;idx:[8]int;
  // The next request's cursor views hex, so hex outlives every page.
  bounds:[1]CwBounds;sent:[160]u8;hex:[64]u8;
  for page:1..<4 {
    p:=cw_window(rows[..],n[0],q,true,idx[..]);
    print("page %:\n",page);show_page(rows[..],p,idx[..]);
    dn:=cw_bounds_d("general",p,sent[..]);b:[200]u8;bn:=cw_bounds_content(p,b[..]);
    bounds[0]=cw_read_bounds(cast(string)sent[0..<dn],cast(string)b[0..<bn]);
    print("  the client sees %\n",VIEW_WORDS[cw_view(bounds[..],cast(string)sent[0..<dn],p.head,p.n)]);
    if !p.has_more {break;}
    x:=cw_put_hex(hex[..],0,p.next_id);
    q.has_until=true;q.until=p.next_at;q.has_before=true;q.before=cast(string)hex[0..<64];
  }
  t:CwRequest;t.thread=true;t.channels=1;t.roots=1;t.root=rows[2].id;t.kinds[0]=9;t.kinds_n=1;
  p:=cw_window(rows[..],n[0],t,true,idx[..]);
  print("thread window under m3: % replies, newest first: %…, %…; has_more %\n",p.n,id8(rows[idx[0]].id,h[..]),id8(rows[idx[1]].id,sent[..]),p.has_more);
  print("broken inputs\n");
  half:=q;half.has_before=false;
  print("until without before_id: %\n",CW_WORDS[cw_window(rows[..],n[0],half,true,idx[..]).status]);
  bad:=q;bad.before="1c1bfdde";
  print("a before_id of 8 hex digits: %\n",CW_WORDS[cw_window(rows[..],n[0],bad,true,idx[..]).status]);
  two:=q;two.channels=2;
  print("two channels in #h: %\n",CW_WORDS[cw_window(rows[..],n[0],two,true,idx[..]).status]);
  print("a client-submitted 39006: %\n",CW_WORDS[cw_ingest(rows[..],n[..],cw_fake(39006))]);
  head:CwRequest;head.channels=1;
  p=cw_window(rows[..],n[0],head,false,idx[..]);
  none:=bounds[0..<0];
  print("a reader who cannot read general: served %, % rows; the client sees %\n",p.served,p.n,VIEW_WORDS[cw_view(none,"general:head",true,0)]);
  p=cw_window(rows[0..<0],0,head,true,idx[..]);
  dn:=cw_bounds_d("empty",p,sent[..]);b:[200]u8;bn:=cw_bounds_content(p,b[..]);
  bounds[0]=cw_read_bounds(cast(string)sent[0..<dn],cast(string)b[0..<bn]);
  print("a channel with no messages: served %, % rows, %; the client sees %\n",p.served,p.n,cast(string)b[0..<bn],VIEW_WORDS[cw_view(bounds[..],cast(string)sent[0..<dn],true,0)]);
  print("that bounds overlay answering another cursor: the client must %\n",VIEW_WORDS[cw_view(bounds[..],"empty:1790000060:1c1b",true,0)]);
}
// A 39006 a client tries to store, already verified.
cw_fake :: (kind:int)->NostrEvent {e:=nt_event("[]");e.kind=kind;e.verified=true;return e;}
```

DM visibility and the workspace icon:

<!-- source: nostr/examples/channels.coax visibility -->
```jai
part_dv :: (relay:[32]u8) {
  print("NIP-DV: Alice hides the DM dm-ab\n");
  alice:=key_of(CHV_ALICE);bob:=key_of(CHV_BOB);carol:=g29_k(0xcc);
  ms:=[3]DvMember.[DvMember.{viewer=alice,channel=g29_text("dm-ab"),dm=true,active=true},
                   DvMember.{viewer=bob,channel=g29_text("dm-ab"),dm=true,active=true},
                   DvMember.{viewer=alice,channel=g29_text("general"),dm=false,active=true}];
  print("hide: republish %\n",dv_command(ms[..],alice,DV_HIDE,g29_text("dm-ab"))==DV_OK);
  tags:[512]u8;m:=dv_snapshot(ms[..],alice,tags[..]);
  out:[16384]u8;e:=verified(CHV_SNAPSHOT,out[..]);
  print("the snapshot's tags equal the signed 30622 (%…): %\n",CHV_SNAPSHOT_ID[0..<8],cast(string)e.original[e.tags_from..<e.tags_to]==cast(string)tags[0..<m]);
  print("delivered to Alice %, to Bob %, to Carol %\n",dv_deliver(e,alice),dv_deliver(e,bob),dv_deliver(e,carol));
  q:DvQuery;q.may_return=true;q.p[0]=alice;q.p_n=1;
  print("Bob's filter {kinds:[30622], #p:[Alice]} passes the gate: %\n",dv_query(q,bob));
  hidden:[4]G29Name;k:=dv_hidden(e,relay,alice,hidden[..]);
  items:=[2]DvItem.[DvItem.{channel=g29_text("dm-ab"),dm=true},DvItem.{channel=g29_text("general"),dm=false}];
  shown:[2]int;v:=dv_visible(items[..],hidden[0..<k],shown[..]);
  print("Alice's list: % of 2 shown (general); Bob's list: % of 2 shown\n",v,dv_visible(items[..],hidden[0..<0],shown[..]));
  dm:ClChannel;dm.type=g29_text("dm");g:G29Group;g.is_private=true;plain:ClFilter;named:ClFilter;named.names=true;
  print("Carol lists dm-ab %, or names it %; Alice is still a member: %\n",cl_listed(g,dm,plain,false),cl_listed(g,dm,named,false),cl_listed(g,dm,named,true));
  print("a client-submitted 30622: allowed %\n",dv_client_kind(DV_SNAPSHOT));
}
part_wp :: () {
  print("NIP-WP: the workspace icon\n");
  out:[16384]u8;icon:[256]u8;words:=[8]string.["ok","event","kind","role","shape","scheme","bytes","size"];
  r:=wp_admit(verified(CHV_ICON,out[..]),WP_ADMIN,icon[..]);
  print("Alice as admin: %, icon %\n",words[r.status],cast(string)icon[0..<r.n]);
  print("the same command from a member: %\n",words[wp_admit(verified(CHV_ICON,out[..]),WP_MEMBER,icon[..]).status]);
  print("a javascript: icon: %\n",words[wp_admit(verified(CHV_ICON_SCRIPT,out[..]),WP_OWNER,icon[..]).status]);
}
```

Run it:

```shell
coax/bin/coax run nostr/examples/channels.coax
```

Output:

```text
NIP-CL: Alice creates general with the label team
create: ok
the relay's 39000 tags: [["d","general"],["t","stream"],["t","team"],["P","79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"],["created_at","1790000000"]]
the signed 39000 (7640cda4…) carries the same tags: true
a client reads it back:
  type stream, labels [team], creator 79be667e…, created_at 1790000000
Bob's create with the label "Team Chat": label
Bob's create with the label "dm": reserved
the same 39000 signed by Bob instead of the relay: signer
Alice creates the system channel agent-cfg: ok, private true
a plain channel list shows general true and agent-cfg false
#t system shows agent-cfg to Alice true, to Bob false; #P shows it to Alice true
oldest of general (1790000000), chat-2 (1789999900), chat-1 (1789999900): chat-1
NIP-CW: six messages arrive in general
m1 d14a53f2… at T0+0: ok, depth 0, broadcast false
m2 1c1bfdde… at T0+60: ok, depth 0, broadcast false
m3 31048ee3… at T0+60: ok, depth 0, broadcast false
m4 cc85adb2… at T0+60: ok, depth 0, broadcast false
r1 971b69e8… at T0+90: ok, depth 1, broadcast false
r2 4b46ba5c… at T0+120: ok, depth 1, broadcast true
page 1:
  rows [4b46ba5c,1c1bfdde]
  39006 d general:head
  39006 content {"has_more":true,"next_cursor":{"created_at":1790000060,"id":"1c1bfdde732181ce195077eb6180fdb516a7f62319db107351f4ccb47c7dc8d8"}}
  the client sees a page
page 2:
  rows [31048ee3,cc85adb2]
  39006 d general:1790000060:1c1bfdde732181ce195077eb6180fdb516a7f62319db107351f4ccb47c7dc8d8
  39006 content {"has_more":true,"next_cursor":{"created_at":1790000060,"id":"cc85adb2f0de133f9276e88c8973041ac318faeb73418375bef5ee0bd29f7ff3"}}
  the client sees a page
page 3:
  rows [d14a53f2]
  39006 d general:1790000060:cc85adb2f0de133f9276e88c8973041ac318faeb73418375bef5ee0bd29f7ff3
  39006 content {"has_more":false,"next_cursor":null}
  the client sees a page
thread window under m3: 2 replies, newest first: 4b46ba5c…, 971b69e8…; has_more false
broken inputs
until without before_id: cursor
a before_id of 8 hex digits: cursor
two channels in #h: channels
a client-submitted 39006: relay only
a reader who cannot read general: served false, 0 rows; the client sees no window: fall back to a plain filter
a channel with no messages: served true, 0 rows, {"has_more":false,"next_cursor":null}; the client sees an empty channel
that bounds overlay answering another cursor: the client must discard the response
NIP-DV: Alice hides the DM dm-ab
hide: republish true
the snapshot's tags equal the signed 30622 (c7a50ad6…): true
delivered to Alice true, to Bob false, to Carol false
Bob's filter {kinds:[30622], #p:[Alice]} passes the gate: false
Alice's list: 1 of 2 shown (general); Bob's list: 2 of 2 shown
Carol lists dm-ab false, or names it false; Alice is still a member: true
a client-submitted 30622: allowed false
NIP-WP: the workspace icon
Alice as admin: ok, icon https://example.com/team.png
the same command from a member: role
a javascript: icon: scheme
```

The relay's 39000 tags are byte for byte the tags of the 39000 that Python
signed with the relay's key, and the snapshot's tags those of the signed
30622. The pages are the ones worked out by hand. The broken inputs show
each refusal: half a cursor (`cursor`), an 8-digit `before_id`
(`cursor`), two channels (`channels`), a client-submitted 39006 (`relay
only`), a label with a space (`label`), a type name as a label
(`reserved`), a 39000 signed by Bob (`signer`), a member setting the icon
(`role`) and a `javascript:` icon (`scheme`). The example exits with
status 0.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/channels.coax](../examples/checks/channels.coax),
which loads the example:

- The event ids and pages from
  [channels_vectors.py](../../coax/tests/nostr/channels_vectors.py), which
  signs with [nip59_reference.py](../../coax/tests/nostr/nip59_reference.py)'s
  integer secp256k1 and BIP-340, hashes with `hashlib`, and sorts the
  top-level messages by `(-created_at, id)` in Python. Coax must verify
  each event, recompute its id and return the same three pages and next
  cursors.
- The NIPs' own examples, as printed. Every key and id in them is a
  placeholder (`<creator public key>`, `<64-hex id>`,
  `<viewer-pubkey-hex>`), so they have no event ids to compare with; those
  placeholders are their only wrong values. NIP-CL's 39000 refuses at its
  `P` tag and reads as NIP-CL describes once a real key replaces it;
  NIP-CW's 39006 content refuses at its id; NIP-DV's snapshot refuses at
  its `d` tag.
- The worked example's ids and pages by hand.

<!-- source: nostr/examples/checks/channels.coax check -->
```jai
// From coax/tests/nostr/channels_vectors.py, which signs with
// nip59_reference.py's integer secp256k1 and BIP-340 (no code shared with
// Coax), hashes each id as hashlib.sha256 of json.dumps([0, pubkey,
// created_at, kind, tags, content], separators=(",", ":")), and computes the
// pages by sorting the top-level messages by (-created_at, id) in Python.
PY_PAGES :: [3]string.["4b46ba5c,1c1bfdde","31048ee3,cc85adb2","d14a53f2"];
PY_NEXT :: [3]string.["1790000060:1c1bfdde732181ce195077eb6180fdb516a7f62319db107351f4ccb47c7dc8d8",
                      "1790000060:cc85adb2f0de133f9276e88c8973041ac318faeb73418375bef5ee0bd29f7ff3","null"];

#test "the event ids and the pages are the Python reference's" #steps 1000000000 {
  texts:=[6]string.[CHV_M1,CHV_M2,CHV_M3,CHV_M4,CHV_R1,CHV_R2];
  ids:=[6]string.[CHV_M1_ID,CHV_M2_ID,CHV_M3_ID,CHV_M4_ID,CHV_R1_ID,CHV_R2_ID];
  rows:[8]CwRow;n:[1]int;out:[16384]u8;h:[64]u8;
  for i:0..<6 {
    e:=verified(texts[i],out[..]);#assert e.error==NE_OK && e.verified;
    x:=cw_put_hex(h[..],0,e.computed_id);#assert cast(string)h[..]==ids[i];
    #assert cw_ingest(rows[..],n[..],e)==CW_OK;
  }
  #assert CHV_PAGE1==PY_PAGES[0] && CHV_PAGE2==PY_PAGES[1] && CHV_PAGE3==PY_PAGES[2];
  #assert CHV_PAGE1_NEXT==PY_NEXT[0] && CHV_PAGE2_NEXT==PY_NEXT[1] && CHV_PAGE3_NEXT==PY_NEXT[2];
  q:CwRequest;q.channels=1;q.has_limit=true;q.limit=2;idx:[4]int;
  // The next request's cursor views hex, so hex outlives every page.
  hex:[64]u8;
  for page:0..<3 {
    p:=cw_window(rows[..],n[0],q,true,idx[..]);
    got:[64]u8;g:=0;
    for i:0..<p.n {if i>0 {g=json_raw(got[..],g,",");}g=json_raw(got[..],g,id8(rows[idx[i]].id,h[..]));}
    #assert cast(string)got[0..<g]==PY_PAGES[page];
    next:[96]u8;k:=json_raw(next[..],0,"null");
    if p.has_more {k=cw_put_int(next[..],0,p.next_at);k=json_raw(next[..],k,":");k=cw_put_hex(next[..],k,p.next_id);}
    #assert cast(string)next[0..<k]==PY_NEXT[page];
    x:=cw_put_hex(hex[..],0,p.next_id);
    q.has_until=true;q.until=p.next_at;q.has_before=true;q.before=cast(string)hex[..];
  }
  m:=verified(CHV_META,out[..]);#assert m.verified && cl_read(m,key_of(CHV_RELAY)).created==T0;
  s:=verified(CHV_SNAPSHOT,out[..]);hidden:[2]G29Name;#assert s.verified && dv_hidden(s,key_of(CHV_RELAY),key_of(CHV_ALICE),hidden[..])==1;
}

// The NIPs' own examples are illustrative: every key and id in them is a
// placeholder ("<creator public key>", "<64-hex id>", "<viewer-pubkey-hex>"),
// so none has an event id to compare. Read as printed, each refuses exactly
// at its first placeholder.
NIP_CL_TAGS :: "[[\"d\",\"9b353519-f4fe-4757-aef4-bec6cc0ae54c\"],[\"name\",\"agent-config\"],[\"t\",\"stream\"],[\"t\",\"agent-config\"],[\"P\",\"<creator public key>\"],[\"created_at\",\"1759939200\"]]";
NIP_CW_BOUNDS :: "{\"has_more\":true,\"next_cursor\":{\"created_at\":1751499000,\"id\":\"<64-hex id>\"}}";
NIP_DV_TAGS :: "[[\"d\",\"<viewer-pubkey-hex>\"],[\"p\",\"<viewer-pubkey-hex>\"],[\"h\",\"<hidden-dm-channel-id>\"],[\"h\",\"<hidden-dm-channel-id>\"]]";
#test "the NIPs' published examples, as printed" {
  relay:=g29_k(0xee);
  e:=nt_event(NIP_CL_TAGS);e.kind=39000;e.verified=true;e.public_key=relay;
  #assert cl_read(e,relay).status==CL_SHAPE;
  // With a real key in place of the placeholder, the rest reads as NIP-CL says.
  fixed:[512]u8;n:=json_raw(fixed[..],0,NIP_CL_TAGS[0..<112]);n=json_raw(fixed[..],n,CHV_ALICE);n=json_raw(fixed[..],n,NIP_CL_TAGS[132..<NIP_CL_TAGS.count]);
  e=nt_event(cast(string)fixed[0..<n]);e.kind=39000;e.verified=true;e.public_key=relay;
  c:=cl_read(e,relay);#assert c.status==CL_OK && g29_is(c.type,"stream") && c.labels_n==1 && g29_is(c.labels[0],"agent-config") && c.created==1759939200;
  #assert !cw_read_bounds("c:head",NIP_CW_BOUNDS).ok;
  d:=nt_event(NIP_DV_TAGS);d.kind=30622;d.verified=true;d.public_key=relay;hidden:[2]G29Name;
  #assert dv_hidden(d,relay,key_of(CHV_ALICE),hidden[..])== -DV_SHAPE;
}

// By hand (the chapter's worked example): the top-level rows are r2 at
// T0+120, the three messages at T0+60 by id (1c1b < 3104 < cc85) and m1 at
// T0; r1 is a reply without broadcast. Pages of two cut the T0+60 second
// between page 1 and page 2, and the composite cursor loses nothing there.
#test "the worked example's values, by hand" {
  #assert CHV_R2_ID[0..<8]=="4b46ba5c" && CHV_M2_ID[0..<4]=="1c1b" && CHV_M3_ID[0..<4]=="3104" && CHV_M4_ID[0..<4]=="cc85" && CHV_M1_ID[0..<8]=="d14a53f2";
  #assert PY_PAGES[0]=="4b46ba5c,1c1bfdde" && PY_PAGES[1]=="31048ee3,cc85adb2" && PY_PAGES[2]=="d14a53f2";
}
```

```shell
coax/bin/coax test nostr/examples/checks/channels.coax --run-steps 250000000
```

`coax test` runs 234 checks, including the loaded files' own checks;
all pass.

### The library's own checks

`nipcl.coax` checks every label character class at its edges, the reserved
names, 8 and 9 distinct labels, repeats, clear and mixed sets; creates of
every type and their privacy; edits by an admin and refused edits by a
member; the 39000 written and read back, with unknown, malformed and
oversized tags; the oldest rule with ties and unknown times; and the
listing rule for each type, filter and notice. `nipcw.coax` checks reply
markers, root agreement, broadcast, depth 100 and 101, relay-only kinds,
every request field at its bounds, the cursor's grammar, same-second
order, exact final pages, deletions, kinds, thread windows and their
roots, both aux hops, the overlay's content written and read strictly,
and each client view. `nipdv.coax` checks hide and re-open, members who
left, non-DMs, the snapshot, both gates and every way a snapshot refuses.
`nipwp.coax` checks each scheme, byte and size bound, roles and tag
counts.

```shell
coax/bin/coax test coax/lib/nostr/nipcl.coax
coax/bin/coax mutate coax/lib/nostr/nipcl.coax --root-only
coax/bin/coax test coax/lib/nostr/nipcw.coax
coax/bin/coax mutate coax/lib/nostr/nipcw.coax --root-only
coax/bin/coax test coax/lib/nostr/nipdv.coax
coax/bin/coax mutate coax/lib/nostr/nipdv.coax --root-only
coax/bin/coax test coax/lib/nostr/nipwp.coax
coax/bin/coax mutate coax/lib/nostr/nipwp.coax --root-only
```

On 2026-11-01, each file's checks passed (`coax test` runs the loaded
files' checks too: 217 for `nipcl.coax`, 220 for `nipcw.coax`, 216 for
`nipdv.coax`, 198 for `nipwp.coax`), and root-only `coax mutate` killed
every mutant: 426 of 426 in `nipcl.coax`, 498 of 498 in `nipcw.coax`, 148
of 148 in `nipdv.coax` and 97 of 97 in `nipwp.coax`. None survived and none
is marked equivalent. Earlier runs left survivors where a boundary could
not be observed (a clamp that sets the value it compares with, a byte
compare after a `!=` test, a duplicate check that one target per event
makes dead); the code was changed so each decision is visible (the clamp
reports `clamped`, compares use `<` and `>` separately, the dead check was
removed), and tests were added for the rest. The test builders live in
`coax/tests/nostr/nipcl_fixtures.coax`, `nipcw_fixtures.coax` and
`nipdv_fixtures.coax`, so root-only mutation mutates the library code. The
example matches its transcript in debug and release. The full mutation run
(`COAX_MUTATION_FULL=1`) and the whole-book example run were not run.

## Summary

- `cl_admit` refuses a bad label set or channel type before NIP-29
  decides, and records the creator and creation time from the relay's own
  record; `cl_read` trusts identity tags only from the relay's key and
  keeps a missing tag unknown.
- `cl_oldest` picks the same channel for every reader: lowest
  `created_at` tag, then lowest id, unknown times last.
- `cl_listed` lists only legacy types to a read that does not name a
  channel; system channels need `#t` `system` or `#P`; a private channel,
  such as a DM, never reaches a non-member.
- `cw_window` pages top-level rows in `created_at` descending, `id`
  ascending order with a composite cursor that is exact within one second,
  and a `limit + 1` probe that makes `has_more` exact.
- `cw_view` never shows a response without its one valid bounds overlay as
  an empty channel.
- `dv_query` and `dv_deliver` hand a DM visibility snapshot only to its
  owner; `dv_visible` drops only that viewer's hidden DMs.
- `wp_admit` takes a workspace icon only from an admin or owner, and only
  an image URL.

## Dependencies

- [59. Groups and communities](ch059-groups-and-communities.md)

## Sources

- [Block NIP-CL](../nips/block/NIP-CL.md)
- [Block NIP-DV](../nips/block/NIP-DV.md)
- [Block NIP-CW](../nips/block/NIP-CW.md)
- [Block NIP-WP](../nips/block/NIP-WP.md)
- [NIP-29](../nips/official/29.md) and [NIP-10](../nips/official/10.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
