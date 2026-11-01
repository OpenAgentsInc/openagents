# 61. Live activities

Live streams and meeting spaces announce themselves as addressable events.
This chapter reads and writes them, decides at a given clock time whether a
stream is still live, and binds live chat to the stream it belongs to.

Part X, Every NIP. Result: NIP-53 kind 30311 live events with `streaming`,
`status` and participant roles, kind 1311 live chat, and kind 30312/30313
meeting spaces and rooms with kind 10312 presence.

## What this chapter teaches

- Kind 30311: `d`, `title`, `streaming`, `recording`, `starts`, `ends`,
  `status` `planned`/`live`/`ended`, and `p` tags that carry a role.
- When a `live` event counts as ended: NIP-53 lets a client treat a live
  event with no update for an hour as ended. The decision here takes the
  clock as an argument, so the same event and clock give the same answer.
- Kind 1311: chat that names its activity with an `a` tag, and why a chat
  message whose `a` names another activity is not shown with this one.
- Kinds 30312 and 30313 for meeting spaces and the meetings in them, and
  kind 10312 presence.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag(e, i)` | Reads tag `i`: a `p` tag's key and relay, an `a` tag's address (chapter 22) |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_tag(e, i)` | Reads tag `i` of any name: the name and its first two values (chapter 55) |
| [nip23.coax](../../coax/lib/nostr/nip23.coax) | `nip23_address`, `nip23_current` | Writes an `a` coordinate; picks the current version of an addressable event (chapter 53) |
| [nip53.coax](../../coax/lib/nostr/nip53.coax) | `n53_read(e)` | A kind 30311, 30312 or 30313 activity: its fields, status and participants |
| [nip53.coax](../../coax/lib/nostr/nip53.coax) | `n53_state(a, updated, now)` | The status to show at time `now`: a live event older than an hour is ended |
| [nip53.coax](../../coax/lib/nostr/nip53.coax) | `n53_chat(e)`, `n53_chat_in(chat, activity)` | A kind 1311 message's activity and parent; whether it belongs to a given activity |
| [nip53.coax](../../coax/lib/nostr/nip53.coax) | `n53_presence(e)`, `n53_fresh` | A kind 10312 presence's room and raised hand; whether it is recent enough to show |
| [nip53.coax](../../coax/lib/nostr/nip53.coax) | `n53_tags`, `n53_chat_tags` | Write a kind 30311's tags and a kind 1311's tags |

`nip53.coax` is new with this chapter. It is an `#authority` file: it
decides which status a client shows and which chat messages belong to a
stream, so every procedure runs in a check and `coax mutate` leaves no
survivors. It verifies no signature and fetches nothing. The caller
verifies the events (chapter 21) before it passes them in.

## The records

An activity is read into one record for all three addressable kinds. The
fields that a kind doesn't have stay empty: a space has a room and a
service where a stream has a title and a streaming URL, so the reader puts
them in the same two fields.

<!-- source: coax/lib/nostr/nip53.coax record -->
```jai
N53Participant :: struct {
  pubkey:[32]u8;
  role:N32Text;                       // the fourth entry; empty when absent
  has_proof:bool; proof:[64]u8;       // the fifth entry, checked for shape only
}
N53Activity :: struct {
  error:int=N53_EVENT; at:int=-1;     // the refusing tag
  kind:int;
  d:N32Text;
  title:N32Text;                      // title; a space's room
  url:N32Text;                        // streaming; a space's service
  recording:N32Text;                  // a live stream's recording
  status:int;                         // N53_NONE when absent
  has_starts:bool; starts:int; has_ends:bool; ends:int;
  has_space:bool; space_author:[32]u8; space_d:N32Text;   // a meeting's space
  p:[N53_MAX_P]N53Participant; n:int;
  hosts:int;                          // participants whose role is Host
}
N53Chat :: struct {
  error:int=N53_EVENT; at:int=-1;
  author:[32]u8; d:N32Text;           // the activity: 30311:author:d
  has_parent:bool; parent:[32]u8;     // the e tag: the message replied to
}
N53Presence :: struct {
  error:int=N53_EVENT; at:int=-1;
  room_kind:int; author:[32]u8; d:N32Text;   // the room: 30312 or 30313
  hand:bool;                          // ["hand", "1"]
}
// What the writer puts in a kind 30311's tags. Empty strings, negative
// times and N53_NONE are left out; d is required.
N53Meta :: struct {d:string; title:string; streaming:string; recording:string; status:int; starts:int=-1; ends:int=-1;}
N53Role :: struct {pubkey:[32]u8; relay:string; role:string;}
```

## Live streams

A kind 30311 event describes one stream. The `d` tag names it: kind 30311,
the host's key and `d` form its address, and a newer event with the same
address replaces the older one. The host keeps publishing new versions as
the stream goes from planned to live to ended, and as people join and
leave. NIP-53 says every tag but `d` is optional.

### Status values

`status` takes one of a closed set of values. A stream or a meeting is
`planned`, `live` or `ended`. A space is `open`, `private` or `closed`.
The values are case-sensitive, so `Live` is refused. `starts` and `ends`
are decimal unix seconds.

<!-- source: coax/lib/nostr/nip53.coax status -->
```jai
N53_STATUS_NAMES :: [7] string.["none", "planned", "live", "ended", "open", "private", "closed"];
// The status a string names for this kind, or N53_NONE: planned, live and
// ended for a live stream or a meeting; open, private and closed for a space.
n53_status :: (kind:int,s:N32Text)->int {
  first:=ifx kind==N53_SPACE then N53_OPEN else N53_PLANNED;
  for k:first..<first+3 {if n32_is(s,N53_STATUS_NAMES[k]) {return k;}}
  return N53_NONE;
}
n53_status_name :: (status:int)->string {
  if status<N53_NONE || status>N53_CLOSED {return "unknown";}
  return N53_STATUS_NAMES[status];
}
// Decimal unix seconds, or -1.
n53_time :: (s:N32Text)->int {
  t:=s;v:=json_int(cast(string)t.b[0..<t.n]);
  if !v.ok || v.value<0 {return -1;}
  return v.value;
}
```

### Reading an activity

`n53_read` walks the tags once with `n32_tag`. For each field it keeps the
first tag with that name, as chapter 53 does for articles. It reads `p`
and `a` tags through chapter 22's `nostr_tag`, so their keys, relays and
addresses get the same checks as everywhere else.

<!-- source: coax/lib/nostr/nip53.coax read -->
```jai
// A kind 30311, 30312 or 30313 activity: its fields and participants.
n53_read :: (e:NostrEvent)->N53Activity {
  a:N53Activity;if e.error!=NE_OK {return a;}
  if e.kind!=N53_LIVE_EVENT && e.kind!=N53_SPACE && e.kind!=N53_MEETING {return n53_refuse(a,N53_KIND,-1);}
  a.kind=e.kind;seen:[8]bool;i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n53_refuse(a,N53_LIMIT,i);}
    if t.error!=NT_OK {return n53_refuse(a,N53_EVENT,i);}
    f:=n53_field(e.kind,t.v[0]);v:=t.v[1];
    if f>=0 && !seen[f] {
      seen[f]=true;
      if f==0 {a.d=v;} else if f==1 {a.title=v;} else if f==2 {a.url=v;} else if f==3 {a.recording=v;}
      else if f==4 {a.status=n53_status(e.kind,v);if a.status==N53_NONE {return n53_refuse(a,N53_STATUS,i);}}
      else if f==5 {a.starts=n53_time(v);a.has_starts=true;if a.starts<0 {return n53_refuse(a,N53_TIME,i);}}
      else if f==6 {a.ends=n53_time(v);a.has_ends=true;if a.ends<0 {return n53_refuse(a,N53_TIME,i);}}
      else {
        s:=nostr_tag(e,i);
        if s.error!=NT_OK {return n53_refuse(a,N53_TAG,i);}
        if s.address_kind!=N53_SPACE {return n53_refuse(a,N53_ADDRESS,i);}
        a.has_space=true;a.space_author=s.pubkey;a.space_d=n53_copy(s.d,s.d_n);
      }
    }
    if n32_is(t.v[0],"p") {
      s:=nostr_tag(e,i);
      if s.error!=NT_OK {return n53_refuse(a,N53_TAG,i);}
      if a.n==N53_MAX_P {return n53_refuse(a,N53_LIMIT,i);}
      p:N53Participant;p.pubkey=s.pubkey;
      p.role.n=n53_value(e,i,3,p.role.b[..]);
      proof:N32Text;proof.n=n53_value(e,i,4,proof.b[..]);
      if proof.n>0 {
        if !ne_lower_hex(cast(string)proof.b[0..<proof.n],p.proof[..]) {return n53_refuse(a,N53_TAG,i);}
        p.has_proof=true;
      }
      if n32_is(p.role,"Host") {a.hosts+=1;}
      a.p[a.n]=p;a.n+=1;
    }
    i+=1;
  }
  if a.d.n==0 {return n53_refuse(a,N53_NO_D,-1);}
  if a.has_starts && a.has_ends && a.ends<a.starts {return n53_refuse(a,N53_TIME,-1);}
  if e.kind==N53_SPACE && (a.title.n==0 || a.status==N53_NONE || a.url.n==0 || a.n==0) {return n53_refuse(a,N53_MISSING,-1);}
  if e.kind==N53_MEETING {
    if !a.has_space {return n53_refuse(a,N53_NO_A,-1);}
    if a.title.n==0 || !a.has_starts || a.status==N53_NONE {return n53_refuse(a,N53_MISSING,-1);}
  }
  a.error=N53_OK;return a;
}
```

The reader refuses:

- Any kind other than 30311, 30312 and 30313: `N53_KIND`.
- An activity with no `d` tag or an empty one: `N53_NO_D`. Without one,
  every activity of a host would have the same address and replace the
  others.
- A `status` outside the kind's set: `N53_STATUS`, at the tag's index.
- A `starts` or `ends` that is not decimal unix seconds, or an `ends`
  before `starts`: `N53_TIME`.
- A `p` or `a` tag whose key or address doesn't read, or a proof that is
  not 128 lowercase hex digits: `N53_TAG`.
- A space without `room`, `status`, `service` or a `p` tag, or a meeting
  without `title`, `starts` or `status`: `N53_MISSING`. NIP-53 marks these
  fields as required.
- A meeting without an `a` tag: `N53_NO_A`. A meeting whose `a` tag names
  anything but a kind 30312 space: `N53_ADDRESS`.
- More than eight participants: `N53_LIMIT`. NIP-53 asks providers to keep
  the list small and says that clients should not expect a complete list.

A stream without a `status` tag reads, with status `none`. NIP-53 makes
the tag optional, so the reader doesn't refuse it, and `n53_state` shows
it as `none`, neither live nor ended.

## Participants and roles

Every `p` tag names one participant:

```text
["p", <pubkey>, <relay, may be empty>, <role>, <proof>]
```

The role is a name to display, such as `Host`, `Speaker` or `Participant`.
The reader keeps it as written and counts the participants whose role is
exactly `Host` in `hosts`.

The proof is a signature, by the participant's key, over the SHA-256 of
the activity's `a` tag. It shows that the participant agreed to be named.
NIP-53 says a client may show a participant without a proof as invited.
The reader checks that a proof is 64 bytes of lowercase hex and records
`has_proof`. It does not verify the signature; see Not built.

NIP-53 says a space MUST have a provider with the `Host` role, but the
NIP's own space example names an `Owner` and a `Moderator`. The reader
follows the example: it requires at least one `p` tag on a space and
reports `hosts`, and it doesn't refuse a space with no `Host`.

## Is it live?

NIP-53 asks the host to keep updating a live event, and lets a client
treat a `live` event with no update for an hour as `ended`. A host whose
client crashed never publishes `ended`, so without this rule the stream
would stay live forever. `n53_state` applies the rule against a clock
that the caller passes:

<!-- source: coax/lib/nostr/nip53.coax state -->
```jai
// The status to show at time now for an activity last updated at updated:
// a live one with no update for more than N53_STALE seconds is ended.
n53_state :: (a:N53Activity,updated:int,now:int)->int {
  if a.status==N53_LIVE && now-updated>N53_STALE {return N53_ENDED;}
  return a.status;
}
// Whether an event created at created is within window seconds of now, as
// a client filters kind 10312 presence.
n53_fresh :: (created:int,now:int,window:int)->bool {return now-created<=window;}
```

"After an hour" is read as strictly more than 3,600 seconds: an event
exactly 3,600 seconds old is still live. An event created a little after
`now`, which happens when clocks differ, has a negative age and stays
live. The rule changes only `live`: a planned stream stays planned however
old it is, and an ended one stays ended.

To decide what to show at a clock time, a client first picks the current
version of the stream among the versions it has seen, then applies the
rule to that version. A version created after `now` hasn't been seen yet.

### Worked example: three clock times

Alice hosts a stream with `d` `coax-weekly`. Let T0 be 1762000000. She
publishes three versions:

| Version | `created_at` | `status` | Other tags |
| --- | --- | --- | --- |
| V1 | T0 | `planned` | `starts` T0+3600 |
| V2 | T0+3600 | `live` | `starts` T0+3600, `streaming` |
| V3 | T0+9000 | `ended` | `starts` T0+3600, `ends` T0+7000, `recording` |

Decide by hand what a client shows at three clock times:

1. `now` = T0+1800. Only V1 has `created_at` ≤ `now`, so V1 is current.
   Its status is `planned`, and the hour rule applies only to `live`. The
   client shows **planned**: not live.
2. `now` = T0+7200. V1 and V2 have been seen. V2 is newer (T0+3600 > T0),
   so V2 is current. Its status is `live`. Its age is 7200 − 3600 = 3,600
   seconds, which is not more than 3,600. The client shows **live**.
3. `now` = T0+7201. V2 is still current. Its age is 7201 − 3600 = 3,601
   seconds, more than 3,600. The client shows **ended**, although V2 says
   `live`.

Then V3 arrives. At T0+9000 it is current (T0+9000 > T0+3600) and says
`ended`, so the client shows **ended** from V3's own status, and V3 adds
the recording.

The example prints the same four lines, and the checks compare them with
these hand values.

## Live chat

A kind 1311 message names its stream with an `a` tag, `30311:<host's
key>:<d>`, and NIP-53 says clients MUST include it. An `e` tag names the
message it replies to.

<!-- source: coax/lib/nostr/nip53.coax chat -->
```jai
// The first a tag of an event, which must name one of two kinds (the
// second may be the first): its kind, author and d, or the refusal and
// the refusing tag.
N53Room :: struct {error:int=N53_NO_A; at:int=-1; kind:int; author:[32]u8; d:N32Text;}
n53_room :: (e:NostrEvent,kind:int,other:int)->N53Room {
  r:N53Room;i:=0;t:=n32_tag(e,0);
  while t.error!=NT_INDEX {
    r.at=i;
    if t.error==NT_LIMIT {r.error=N53_LIMIT;return r;}
    if t.error!=NT_OK {r.error=N53_EVENT;return r;}
    if n32_is(t.v[0],"a") {
      s:=nostr_tag(e,i);
      if s.error!=NT_OK {r.error=N53_TAG;return r;}
      if s.address_kind!=kind && s.address_kind!=other {r.error=N53_ADDRESS;return r;}
      r.kind=s.address_kind;r.author=s.pubkey;r.d=n53_copy(s.d,s.d_n);r.error=N53_OK;return r;
    }
    i+=1;t=n32_tag(e,i);
  }
  r.at= -1;return r;
}
// A kind 1311 chat message: the activity its a tag names, and the message
// its first e tag replies to.
n53_chat :: (e:NostrEvent)->N53Chat {
  c:N53Chat;if e.error!=NE_OK {return c;}
  if e.kind!=N53_CHAT {return n53_chat_fail(c,N53_KIND,-1);}
  r:=n53_room(e,N53_LIVE_EVENT,N53_LIVE_EVENT);
  if r.error!=N53_OK {return n53_chat_fail(c,r.error,r.at);}
  c.author=r.author;c.d=r.d;
  i:=0;
  while !c.has_parent {
    t:=n32_tag(e,i);
    if t.error!=NT_OK {break;}
    if n32_is(t.v[0],"e") {
      s:=nostr_tag(e,i);
      if s.error!=NT_OK {return n53_chat_fail(c,N53_TAG,i);}
      c.has_parent=true;c.parent=s.id;
    }
    i+=1;
  }
  return n53_chat_fail(c,N53_OK,-1);
}
// Whether chat belongs to activity, a kind 30311 event: N53_OK when the
// chat's a tag names the activity's author and d, else the refusal.
n53_chat_in :: (chat:NostrEvent,activity:NostrEvent)->int {
  a:=n53_read(activity);if a.error!=N53_OK {return a.error;}
  if a.kind!=N53_LIVE_EVENT {return N53_KIND;}
  c:=n53_chat(chat);if c.error!=N53_OK {return c.error;}
  if !ns_equal32(c.author,activity.public_key) || n32_text_order(c.d,a.d)!=0 {return N53_MISMATCH;}
  return N53_OK;
}
```

`n53_chat` reads the first `a` tag. It refuses chat with no `a` tag
(`N53_NO_A`) and chat whose first `a` tag names any kind but 30311
(`N53_ADDRESS`). Later `a` tags may cite other events and are not read.

`n53_chat_in` decides whether a message belongs to a stream: the chat's
`a` must name the stream's own author and `d`. A message whose `a` names
another `d`, or the same `d` under another key, is refused with
`N53_MISMATCH`. A client that shows chat under a stream checks each
message this way, so a message can't be placed under a stream it doesn't
name. Every version of a stream has the same address, so a message stays
bound to the stream as it is updated.

## Meeting spaces, meetings and presence

A kind 30312 space is a room that persists: a name in `room`, a status of
`open`, `private` or `closed`, the URL of the service that runs it, and
its providers in `p` tags. A kind 30313 meeting is one scheduled or
ongoing meeting in a space. It names its space with an `a` tag, and has a
title, a start time and a `planned`/`live`/`ended` status, like a stream.
NIP-53 says a meeting without updates for an hour may be considered ended,
so `n53_state` applies to meetings too.

A kind 10312 event says that a listener is in a room. It is replaceable,
so a user is present in one room at a time. Its `a` tag names a space or a
meeting, and `["hand", "1"]` raises a hand.

<!-- source: coax/lib/nostr/nip53.coax presence -->
```jai
// A kind 10312 presence: the room its first a tag names (a space or a
// meeting) and whether the first hand tag is "1".
n53_presence :: (e:NostrEvent)->N53Presence {
  p:N53Presence;if e.error!=NE_OK {return p;}
  if e.kind!=N53_PRESENCE {return n53_presence_fail(p,N53_KIND,-1);}
  r:=n53_room(e,N53_SPACE,N53_MEETING);
  if r.error!=N53_OK {return n53_presence_fail(p,r.error,r.at);}
  p.room_kind=r.kind;p.author=r.author;p.d=r.d;
  b:[NT_MAX_VALUE]u8;h:=n10_text(e,"hand",b[..]);
  p.hand=h.n==1 && b[0]=='1';      // n is 0 when absent
  return n53_presence_fail(p,N53_OK,-1);
}
```

NIP-53 says clients should ignore presence events older than a time
window, without giving the window. `n53_fresh` takes the window from the
caller.

## Writing

`n53_tags` writes a kind 30311's tags in a fixed order: `d`, `title`,
`streaming`, `recording`, `starts`, `ends`, `status`, then one `p` tag per
role. Empty strings and negative times are left out. It refuses an empty
`d`, a status that a stream can't have, and `ends` before `starts`, so the
writer never produces an event that the reader refuses. `n53_chat_tags`
writes a chat message's `a` tag from the host's key and `d`, with an
optional `e` tag for the parent.

<!-- source: coax/lib/nostr/nip53.coax write -->
```jai
// A kind 30311's tags as a JSON array into out: its length, or minus the
// refusal (N53_NO_D, N53_STATUS, N53_TIME, N53_OUTPUT).
n53_tags :: (m:N53Meta,roles:[]N53Role,out:[]u8)->int {
  if m.d.count==0 {return -N53_NO_D;}
  if m.status<N53_NONE || m.status>N53_ENDED {return -N53_STATUS;}
  if m.ends>=0 && m.ends<m.starts {return -N53_TIME;}
  n:=json_raw(out,0,"[[\"d\",");n=json_write_string(out,n,m.d);n=json_raw(out,n,"]");
  n=n23_pair(out,n,"title",m.title);n=n23_pair(out,n,"streaming",m.streaming);n=n23_pair(out,n,"recording",m.recording);
  num:[24]u8;
  if m.starts>=0 {k:=json_write_int(num[..],0,m.starts);n=n23_pair(out,n,"starts",cast(string)num[0..<k]);}
  if m.ends>=0 {k:=json_write_int(num[..],0,m.ends);n=n23_pair(out,n,"ends",cast(string)num[0..<k]);}
  if m.status!=N53_NONE {n=n23_pair(out,n,"status",N53_STATUS_NAMES[m.status]);}
  for r:roles {
    n=json_raw(out,n,",[\"p\",");n=n10_hex(out,n,r.pubkey);n=json_raw(out,n,",");
    n=json_write_string(out,n,r.relay);n=json_raw(out,n,",");n=json_write_string(out,n,r.role);n=json_raw(out,n,"]");
  }
  n=json_raw(out,n,"]");
  if n== -1 {return -N53_OUTPUT;}
  return n;
}
// A kind 1311's tags into out: the a tag of the activity 30311:author:d
// with a relay hint (may be empty), then an e tag for the parent when
// has_parent. Its length, or -1 for an empty d or a short out.
n53_chat_tags :: (author:[32]u8,d:string,relay:string,has_parent:bool,parent:[32]u8,out:[]u8)->int {
  if d.count==0 {return -1;}
  addr:[NT_MAX_VALUE+80]u8;an:=nip23_address(N53_LIVE_EVENT,author,d,addr[..]);
  if an== -1 {return -1;}
  n:=json_raw(out,0,"[[\"a\",");n=json_write_string(out,n,cast(string)addr[0..<an]);
  n=json_raw(out,n,",");n=json_write_string(out,n,relay);n=json_raw(out,n,"]");
  if has_parent {n=json_raw(out,n,",[\"e\",");n=n10_hex(out,n,parent);n=json_raw(out,n,"]");}
  return json_raw(out,n,"]");
}
```

## Not built

- **The media itself.** This chapter reads the URLs in `streaming`,
  `recording` and `service`. It doesn't fetch, play or serve the stream
  (HLS, WebRTC or anything else) and doesn't run a meeting service.
- **Proof verification.** The reader checks a proof's shape but doesn't
  verify the signature. Verifying it needs the SHA-256 of the `a` tag and
  chapter 11's Schnorr verification; until it is built, every participant
  shows as invited unless the caller verifies the proof itself.
- **The other tags.** `summary`, `image`, `t`, `current_participants`,
  `total_participants`, `relays`, `pinned`, a space's `endpoint` and
  chat's `q` tags are kept in the signed tags and not read.
- **Following streams.** Subscribing to kind 30311 events by follow list
  or status, the `naddr` link (chapter 16's NIP-19 code encodes it), zaps
  to a stream and deleting a stream after it ends belong to other
  chapters.

## The example

The example is
[nostr/examples/live-activities.coax](../examples/live-activities.coax).
It signs eight events with the secret keys 1, 2 and 3 as Alice, Bob and
Carol: Alice's three versions of `coax-weekly`, written by `n53_tags`;
Bob's chat message and Carol's reply to it, written by `n53_chat_tags`;
and Alice's space `coax-hall`, a meeting in it and Carol's presence there.
It reads every event back, decides the stream's state at the worked
example's clock times, binds the chat to V2, and then shows each refusal
on an unsigned event, because no signer here would write one.

<!-- source: nostr/examples/live-activities.coax example -->
```jai
#load "../../coax/lib/nostr/nip53.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

ERROR :: [13] string.["ok", "event", "kind", "no d", "bad status", "bad time", "bad tag", "missing field",
  "no a tag", "a names the wrong kind", "a names another activity", "limit", "output"];
// The public keys of the secret keys 1, 2 and 3: published test values.
// Never sign with a real key in an example.
PEOPLE :: [3] string.["Alice", "Bob", "Carol"];
PUB :: [3] string.[
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"];
ALICE :: 0; BOB :: 1; CAROL :: 2;
T0 :: 1762000000;

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
// Seconds after T0: the chapter writes clock times as T0+seconds.
after :: (t: int) -> int { return t - T0; }

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
  #assert e.error == NE_OK && e.verified;
  return e;
}
// The tags with each @ replaced by C1's id.
with_parent :: (template: string, parent: NostrEvent, out: [] u8) -> string {
  n := 0; i := 0; hex: [64] u8;
  while i < template.count {
    if template[i] == '@' { n = json_raw(out, n, hex32(parent.id, hex[..])); i += 1; continue; }
    n = json_raw(out, n, template[i..<i + 1]); i += 1;
  }
  #assert n > 0;
  return cast(string) out[0..<n];
}
// An unsigned event by Carol, parsed but not verified. Used for the
// refusals, which no signer here would write.
unsigned :: (kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"");
  n = json_raw(out, n, PUB[CAROL]); n = json_raw(out, n, "\",\"created_at\":1762000000,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":\"\",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

show_activity :: (name: string, e: NostrEvent) {
  a := n53_read(e);
  print("%: %", name, ERROR[a.error]);
  if a.at >= 0 { print(" at tag %", a.at); }
  if a.error != N53_OK { print("\n"); return; }
  print(", kind %, d ", a.kind); say(a.d); print(", \""); say(a.title); print("\", status %", n53_status_name(a.status));
  if a.has_starts { print(", starts T0+%", after(a.starts)); }
  if a.has_ends { print(", ends T0+%", after(a.ends)); }
  print("\n");
  if a.url.n > 0 { print("  % ", ifx a.kind == N53_SPACE then "service" else "streaming"); say(a.url); print("\n"); }
  if a.recording.n > 0 { print("  recording "); say(a.recording); print("\n"); }
  if a.has_space { print("  in space 30312:%:", person(a.space_author)); say(a.space_d); print("\n"); }
  for k: 0..<a.n { print("  % ", person(a.p[k].pubkey)); say(a.p[k].role); print("%\n", ifx a.p[k].has_proof then ", proof" else ", no proof (invited)"); }
  if a.n > 0 { print("  hosts %\n", a.hosts); }
}

// The events: who signs, when (after T0), the kind, the tags (@ for C1's
// id) and the content.
EVENTS :: 8;
NAME :: [EVENTS] string.["V1", "V2", "C1", "C2", "V3", "S", "M", "P"];
WHO :: [EVENTS] int.[ALICE, ALICE, BOB, CAROL, ALICE, ALICE, ALICE, CAROL];
AT :: [EVENTS] int.[0, 3600, 3700, 3760, 9000, 0, 60, 120];
KIND :: [EVENTS] int.[30311, 30311, 1311, 1311, 30311, 30312, 30313, 10312];
CONTENT :: [EVENTS] string.["", "", "Hello from the stage.", "Hi Bob!", "", "", "", ""];
// V1, V2 and V3 are written by n53_tags and C1 and C2 by n53_chat_tags;
// S, M and P are written out here.
OTHER_TAGS :: [3] string.[
  "[[\"d\",\"coax-hall\"],[\"room\",\"Coax hall\"],[\"status\",\"open\"],[\"service\",\"https://meet.example.com/coax-hall\"],[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"\",\"Host\"],[\"p\",\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\",\"\",\"Moderator\"]]",
  "[[\"d\",\"office-hours\"],[\"a\",\"30312:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:coax-hall\",\"\"],[\"title\",\"Office hours\"],[\"starts\",\"1762086400\"],[\"status\",\"planned\"],[\"p\",\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\",\"\",\"Speaker\"]]",
  "[[\"a\",\"30312:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:coax-hall\",\"\",\"root\"],[\"hand\",\"1\"]]"];

// The tags of event k.
tags_of :: (k: int, c1: NostrEvent, out: [] u8) -> string {
  roles: [2] N53Role; roles[0].pubkey = key_of(ALICE); roles[0].role = "Host"; roles[1].pubkey = key_of(BOB); roles[1].role = "Speaker";
  m: N53Meta; m.d = "coax-weekly"; m.title = "Coax weekly"; m.starts = T0 + 3600;
  n := 0;
  if k == 0 { m.status = N53_PLANNED; n = n53_tags(m, roles[..], out); }
  if k == 1 { m.status = N53_LIVE; m.streaming = "https://stream.example.com/coax-weekly.m3u8"; n = n53_tags(m, roles[..], out); }
  if k == 4 { m.status = N53_ENDED; m.ends = T0 + 7000; m.recording = "https://stream.example.com/coax-weekly.mp4"; n = n53_tags(m, roles[..], out); }
  none: [32] u8;
  if k == 2 { n = n53_chat_tags(key_of(ALICE), "coax-weekly", "wss://relay.example.com", false, none, out); }
  if k == 3 { n = n53_chat_tags(key_of(ALICE), "coax-weekly", "wss://relay.example.com", true, c1.id, out); }
  if k >= 5 { n = json_raw(out, 0, OTHER_TAGS[k - 5]); }
  #assert n > 0;
  return cast(string) out[0..<n];
}

// What a client shows at clock time now: of the versions of
// 30311:Alice:coax-weekly it has seen (created at or before now), the
// current one, and its state at now.
show_at :: (versions: [] NostrEvent, names: [] string, now: int) {
  seen: [3] NostrEvent; seen_names: [3] string; n := 0;
  for k: 0..<versions.count { if versions[k].created_at <= now { seen[n] = versions[k]; seen_names[n] = names[k]; n += 1; } }
  cur := nip23_current(seen[0..<n], N53_LIVE_EVENT, key_of(ALICE), "coax-weekly");
  e := seen[cur]; a := n53_read(e);
  print("now T0+%: current %, status %, updated T0+%, age % s, shown %\n", after(now), seen_names[cur], n53_status_name(a.status),
        after(e.created_at), now - e.created_at, n53_status_name(n53_state(a, e.created_at, now)));
}

main :: () {
  all: [EVENTS] NostrEvent; buf: [EVENTS][4096] u8; tags: [2048] u8;
  for k: 0..<EVENTS { all[k] = sign(WHO[k], T0 + AT[k], KIND[k], tags_of(k, all[2], tags[..]), CONTENT[k], buf[k][..]); }
  print("== 1. Each event's id (T0 = %)\n", T0);
  for k: 0..<EVENTS { hex: [64] u8; print("%: kind %, by %, at T0+%, id %, verified %\n", NAME[k], KIND[k], PEOPLE[WHO[k]], AT[k], hex32(all[k].id, hex[..]), all[k].verified); }
  print("V2's tags: %\n", tags_of(1, all[2], tags[..]));

  print("\n== 2. The live stream's versions\n");
  show_activity("V1", all[0]); show_activity("V2", all[1]); show_activity("V3", all[4]);
  ab: [160] u8; an := nip23_address(N53_LIVE_EVENT, key_of(ALICE), "coax-weekly", ab[..]);
  print("a coordinate %\n", cast(string) ab[0..<an]);

  print("\n== 3. Is it live? Three clock times, then V3\n");
  versions: [2] NostrEvent; versions[0] = all[0]; versions[1] = all[1];
  names := [3] string.["V1", "V2", "V3"];
  show_at(versions[..], names[..], T0 + 1800);
  show_at(versions[..], names[..], T0 + 7200);
  show_at(versions[..], names[..], T0 + 7201);
  three: [3] NostrEvent; three[0] = all[0]; three[1] = all[1]; three[2] = all[4];
  show_at(three[..], names[..], T0 + 9000);

  print("\n== 4. Live chat\n");
  for k: 2..<4 {
    c := n53_chat(all[k]);
    print("%: %, by %, \"%\", activity 30311:%:", NAME[k], ERROR[c.error], PEOPLE[WHO[k]], CONTENT[k], person(c.author)); say(c.d);
    if c.has_parent { print(", replies to %", ifx ns_equal32(c.parent, all[2].id) then "C1" else "another message"); }
    print(", in V2: %\n", ERROR[n53_chat_in(all[k], all[1])]);
  }

  print("\n== 5. A meeting space, a meeting and presence\n");
  show_activity("S", all[5]); show_activity("M", all[6]);
  p := n53_presence(all[7]);
  print("P: %, % in room %:%:", ERROR[p.error], PEOPLE[WHO[7]], p.room_kind, person(p.author)); say(p.d); print(", hand raised %\n", p.hand);
  print("P fresh at T0+420 within 300 s: %; at T0+421: %\n", n53_fresh(all[7].created_at, T0 + 420, 300), n53_fresh(all[7].created_at, T0 + 421, 300));

  print("\n== 6. Refusals\n");
  bad: [2048] u8;
  show_activity("status paused", unsigned(30311, "[[\"d\",\"coax-weekly\"],[\"status\",\"paused\"]]", bad[..]));
  show_activity("starts soon", unsigned(30311, "[[\"d\",\"coax-weekly\"],[\"starts\",\"soon\"]]", bad[..]));
  show_activity("ends before starts", unsigned(30311, "[[\"d\",\"coax-weekly\"],[\"starts\",\"1762003600\"],[\"ends\",\"1762003599\"]]", bad[..]));
  show_activity("no d", unsigned(30311, "[[\"title\",\"Coax weekly\"],[\"status\",\"live\"]]", bad[..]));
  show_activity("space without service", unsigned(30312, "[[\"d\",\"coax-hall\"],[\"room\",\"Coax hall\"],[\"status\",\"open\"],[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"\",\"Host\"]]", bad[..]));
  show_activity("meeting in a live stream", unsigned(30313, "[[\"d\",\"office-hours\"],[\"a\",\"30311:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:coax-weekly\"]]", bad[..]));
  print("chat without a: %\n", ERROR[n53_chat_in(unsigned(1311, "[]", bad[..]), all[1])]);
  print("chat for coax-monthly: %\n", ERROR[n53_chat_in(unsigned(1311, "[[\"a\",\"30311:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:coax-monthly\"]]", bad[..]), all[1])]);
  print("chat for Bob's coax-weekly: %\n", ERROR[n53_chat_in(unsigned(1311, "[[\"a\",\"30311:c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5:coax-weekly\"]]", bad[..]), all[1])]);
  c := n53_chat(unsigned(1311, "[[\"a\",\"30023:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:coax-weekly\"]]", bad[..]));
  print("chat whose a names an article: % at tag %\n", ERROR[c.error], c.at);
  m: N53Meta; m.d = "coax-weekly"; m.status = N53_OPEN; roles: [1] N53Role;
  print("writing a live stream with status open: %\n", ERROR[-n53_tags(m, roles[0..<0], tags[..])]);
}
```

Run it with:

```shell
coax/bin/coax run nostr/examples/live-activities.coax --run-steps 250000000
```

It prints this transcript, which `nostr/fixtures/live-activities.out`
holds and the book's checks compare in debug and release. It exits with
status 0.

```text
== 1. Each event's id (T0 = 1762000000)
V1: kind 30311, by Alice, at T0+0, id e1af03500154058ed778fd33d004780f59c03eeab94108c2d3a6bee4e67e30aa, verified true
V2: kind 30311, by Alice, at T0+3600, id a9f2856553be753efc52035c6088e31c9d3a96cc4f904c765f872cc25e45c153, verified true
C1: kind 1311, by Bob, at T0+3700, id 496c551b4a6f29ef369ade6fe1daf77bc42b73fe7120355a24dda1042d18cbf0, verified true
C2: kind 1311, by Carol, at T0+3760, id 06b4bc87fc3e58591987c499b554cb916f0f34905c8a6dcd69802e606c78feb7, verified true
V3: kind 30311, by Alice, at T0+9000, id cefe1ee87b27bd857fc3f13839ce717108650d49d24947bbbba8ef5fd5589e83, verified true
S: kind 30312, by Alice, at T0+0, id 1c700d03a524345263f38c1c6d75c667d6d4e7db89d5c1cb74bd5081e0b2af3e, verified true
M: kind 30313, by Alice, at T0+60, id 14d4be7f88ae8f1ccd95e190dcdf324a66a47c7dda4dadde61e9d21a8e135c80, verified true
P: kind 10312, by Carol, at T0+120, id 72f19fb20536b52402c130db00774d839b0628a9a90cdb029189d569c62dc23c, verified true
V2's tags: [["d","coax-weekly"],["title","Coax weekly"],["streaming","https://stream.example.com/coax-weekly.m3u8"],["starts","1762003600"],["status","live"],["p","79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","","Host"],["p","c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5","","Speaker"]]

== 2. The live stream's versions
V1: ok, kind 30311, d coax-weekly, "Coax weekly", status planned, starts T0+3600
  Alice Host, no proof (invited)
  Bob Speaker, no proof (invited)
  hosts 1
V2: ok, kind 30311, d coax-weekly, "Coax weekly", status live, starts T0+3600
  streaming https://stream.example.com/coax-weekly.m3u8
  Alice Host, no proof (invited)
  Bob Speaker, no proof (invited)
  hosts 1
V3: ok, kind 30311, d coax-weekly, "Coax weekly", status ended, starts T0+3600, ends T0+7000
  recording https://stream.example.com/coax-weekly.mp4
  Alice Host, no proof (invited)
  Bob Speaker, no proof (invited)
  hosts 1
a coordinate 30311:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:coax-weekly

== 3. Is it live? Three clock times, then V3
now T0+1800: current V1, status planned, updated T0+0, age 1800 s, shown planned
now T0+7200: current V2, status live, updated T0+3600, age 3600 s, shown live
now T0+7201: current V2, status live, updated T0+3600, age 3601 s, shown ended
now T0+9000: current V3, status ended, updated T0+9000, age 0 s, shown ended

== 4. Live chat
C1: ok, by Bob, "Hello from the stage.", activity 30311:Alice:coax-weekly, in V2: ok
C2: ok, by Carol, "Hi Bob!", activity 30311:Alice:coax-weekly, replies to C1, in V2: ok

== 5. A meeting space, a meeting and presence
S: ok, kind 30312, d coax-hall, "Coax hall", status open
  service https://meet.example.com/coax-hall
  Alice Host, no proof (invited)
  Bob Moderator, no proof (invited)
  hosts 1
M: ok, kind 30313, d office-hours, "Office hours", status planned, starts T0+86400
  in space 30312:Alice:coax-hall
  Bob Speaker, no proof (invited)
  hosts 0
P: ok, Carol in room 30312:Alice:coax-hall, hand raised true
P fresh at T0+420 within 300 s: true; at T0+421: false

== 6. Refusals
status paused: bad status at tag 1
starts soon: bad time at tag 1
ends before starts: bad time
no d: no d
space without service: missing field
meeting in a live stream: a names the wrong kind at tag 1
chat without a: no a tag
chat for coax-monthly: a names another activity
chat for Bob's coax-weekly: a names another activity
chat whose a names an article: a names the wrong kind at tag 0
writing a live stream with status open: bad status
```

Section 3 matches the worked example. The refusals show each broken input
and its exact diagnostic:

- `status paused`: `paused` is not a stream status, at tag 1.
- `starts soon`: `soon` is not unix seconds, at tag 1.
- `ends before starts`: `ends` is one second before `starts`.
- `no d`: a stream with a title and a status but no `d` tag.
- `space without service`: a space with room, status and a Host, but no
  `service`.
- `meeting in a live stream`: a meeting whose `a` names a kind 30311, at
  tag 1.
- `chat without a`: a kind 1311 with no tags.
- `chat for coax-monthly`: the `a` names Alice's `coax-monthly`, not V2's
  `coax-weekly`.
- `chat for Bob's coax-weekly`: the right `d` under Bob's key, not
  Alice's.
- `chat whose a names an article`: the `a` names a kind 30023, at tag 0.
- `writing a live stream with status open`: `n53_tags` refuses a space's
  status on a stream.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/live-activities.coax](../examples/checks/live-activities.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The ids of all eight events, from a Python script written from NIP-01
  and NIP-53 (`hashlib.sha256` over NIP-01's serialization). The checks
  read each event with Python's id and require that the recomputed id
  matches.
- The worked example: the current version and the state at T0+1800,
  T0+7200, T0+7201 and T0+9000, from the hand table above, and the hour
  boundary on both sides.
- The chat binding: both messages belong to every version of the stream,
  C2's parent is C1, and the `a` coordinate is the one written out by
  hand.
- NIP-53's two signed examples. The chat example's id and BIP-340
  signature verify, and its `a` binds it to the live stream example. The
  live stream example's printed id is not the SHA-256 of its fields: the
  Python script computes `6c1338a7…` where the NIP prints `57f28dbc…`, so
  the NIP's fields were changed after the event was signed. The checks
  require that `nostr_event_read` refuses it with `NE_ID` and computes
  Python's id, and then read its fields.

<!-- source: nostr/examples/checks/live-activities.coax check -->
```jai
// The ids from a Python script written from NIP-01 and NIP-53 (it shares no
// code with Coax): hashlib.sha256 of json.dumps([0, pubkey, created_at,
// kind, tags, content], separators=(",", ":"), ensure_ascii=False), over
// the same events.
PY_ID :: [EVENTS] string.[
  "e1af03500154058ed778fd33d004780f59c03eeab94108c2d3a6bee4e67e30aa",
  "a9f2856553be753efc52035c6088e31c9d3a96cc4f904c765f872cc25e45c153",
  "496c551b4a6f29ef369ade6fe1daf77bc42b73fe7120355a24dda1042d18cbf0",
  "06b4bc87fc3e58591987c499b554cb916f0f34905c8a6dcd69802e606c78feb7",
  "cefe1ee87b27bd857fc3f13839ce717108650d49d24947bbbba8ef5fd5589e83",
  "1c700d03a524345263f38c1c6d75c667d6d4e7db89d5c1cb74bd5081e0b2af3e",
  "14d4be7f88ae8f1ccd95e190dcdf324a66a47c7dda4dadde61e9d21a8e135c80",
  "72f19fb20536b52402c130db00774d839b0628a9a90cdb029189d569c62dc23c"];
SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
// NIP-53's two signed examples, copied from nostr/nips/official/53.md.
NIP_LIVE :: "{\"id\":\"57f28dbc264990e2c61e80a883862f7c114019804208b14da0bff81371e484d2\",\"pubkey\":\"1597246ac22f7d1375041054f2a4986bd971d8d196d7997e48973263ac9879ec\",\"created_at\":1687182672,\"kind\":30311,\"tags\":[[\"d\",\"demo-cf-stream\"],[\"title\",\"Adult Swim Metalocalypse\"],[\"summary\",\"Live stream from IPTV-ORG collection\"],[\"streaming\",\"https://adultswim-vodlive.cdn.turner.com/live/metalocalypse/stream.m3u8\"],[\"starts\",\"1687182672\"],[\"status\",\"live\"],[\"t\",\"animation\"],[\"t\",\"iptv\"],[\"image\",\"https://i.imgur.com/CaKq6Mt.png\"]],\"content\":\"\",\"sig\":\"5bc7a60f5688effa5287244a24768cbe0dcd854436090abc3bef172f7f5db1410af4277508dbafc4f70a754a891c90ce3b966a7bc47e7c1eb71ff57640f3d389\"}";
NIP_CHAT :: "{\"id\":\"97aa81798ee6c5637f7b21a411f89e10244e195aa91cb341bf49f718e36c8188\",\"pubkey\":\"3f770d65d3a764a9c5cb503ae123e62ec7598ad035d836e2a810f3877a745b24\",\"created_at\":1687286726,\"kind\":1311,\"tags\":[[\"a\",\"30311:1597246ac22f7d1375041054f2a4986bd971d8d196d7997e48973263ac9879ec:demo-cf-stream\",\"\",\"root\"]],\"content\":\"Zaps to live streams is beautiful.\",\"sig\":\"997f62ddfc0827c121043074d50cfce7a528e978c575722748629a4137c45b75bdbc84170bedc723ef0a5a4c3daebf1fef2e93f5e2ddb98e5d685d022c30b622\"}";
// The id the same Python script computes from NIP_LIVE's fields. It is not
// the id the NIP prints: the example's fields were changed after it was
// signed.
PY_NIP_LIVE :: "6c1338a7dcf76ed1b289a4ac07b7065a2132881b4dc43e96853b671fd156ca12";

same_id :: (id: [32] u8, text: string) -> bool { want: [32] u8; decode_hex(text, want[..]); return ns_equal32(id, want); }

// Event k with Python's id and a zero signature, read with its id
// recomputed; the signature is not checked. C2's tags need C1's id, which
// is Python's too.
py :: (k: int, out: [] u8, scratch: [] u8) -> NostrEvent {
  c1: NostrEvent; decode_hex(PY_ID[2], c1.id[..]);
  tags: [1024] u8;
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, PY_ID[k]);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB[WHO[k]]);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, T0 + AT[k]);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, KIND[k]);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags_of(k, c1, tags[..]));
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, CONTENT[k]);
  n = json_raw(out, n, ",\"sig\":\""); n = json_raw(out, n, SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}
read_all :: (ev: [] NostrEvent, buf: [][2048] u8, scratch: [][4096] u8) {
  for k: 0..<EVENTS { ev[k] = py(k, buf[k][..], scratch[k][..]); }
}

#test "the ids are Python's" {
  buf: [EVENTS][2048] u8; s: [EVENTS][4096] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..], s[..]);
  for k: 0..<EVENTS { #assert ev[k].error == NE_OK && ns_equal32(ev[k].computed_id, ev[k].id); }
}

// By hand, from the chapter. V1 is created at T0, V2 at T0+3600, V3 at
// T0+9000; V2 is live. At T0+1800 only V1 exists: planned. At T0+7200 V2
// is 3600 s old, not more than an hour: live. At T0+7201 it is 3601 s old:
// ended. At T0+9000 V3 says ended.
HAND_NOW :: [4] int.[1800, 7200, 7201, 9000];
HAND_CURRENT :: [4] int.[0, 1, 1, 4];
HAND_STATE :: [4] int.[N53_PLANNED, N53_LIVE, N53_ENDED, N53_ENDED];
#test "the state at the chapter's clock times matches the hand values" {
  buf: [EVENTS][2048] u8; s: [EVENTS][4096] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..], s[..]);
  versions := [3] int.[0, 1, 4];
  for t: 0..<4 {
    now := T0 + HAND_NOW[t]; seen: [3] NostrEvent; index: [3] int; n := 0;
    for v: versions { if ev[v].created_at <= now { seen[n] = ev[v]; index[n] = v; n += 1; } }
    cur := nip23_current(seen[0..<n], N53_LIVE_EVENT, key_of(ALICE), "coax-weekly");
    #assert index[cur] == HAND_CURRENT[t];
    #assert n53_state(n53_read(seen[cur]), seen[cur].created_at, now) == HAND_STATE[t];
  }
  // The boundary on both sides, for each live event.
  a := n53_read(ev[1]);
  #assert n53_state(a, T0 + 3600, T0 + 3600 + N53_STALE) == N53_LIVE && n53_state(a, T0 + 3600, T0 + 3601 + N53_STALE) == N53_ENDED;
}
#test "chat is bound to the activity's a coordinate" {
  buf: [EVENTS][2048] u8; s: [EVENTS][4096] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..], s[..]);
  #assert n53_chat_in(ev[2], ev[1]) == N53_OK && n53_chat_in(ev[3], ev[1]) == N53_OK;
  #assert n53_chat_in(ev[2], ev[0]) == N53_OK && n53_chat_in(ev[2], ev[4]) == N53_OK;   // every version has the same address
  c := n53_chat(ev[3]); #assert c.has_parent && ns_equal32(c.parent, ev[2].id);
  #assert n53_chat_in(ev[2], ev[5]) == N53_KIND;   // a space has no live chat
  // The a coordinate written out by hand: kind, Alice's key, d.
  ab: [160] u8; an := nip23_address(N53_LIVE_EVENT, key_of(ALICE), "coax-weekly", ab[..]);
  #assert cast(string) ab[0..<an] == "30311:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:coax-weekly";
}
#test "NIP-53's signed chat verifies and belongs to its live stream" #steps 1500000000 {
  b1: [8192] u8; b2: [8192] u8;
  chat := nostr_event_verify_vartime(NIP_CHAT, b1[..], false);
  #assert chat.error == NE_OK && chat.verified;
  // The live stream example's printed id is not the hash of its fields.
  live := nostr_event_read(NIP_LIVE, b2[..], false);
  #assert live.error == NE_ID && same_id(live.computed_id, PY_NIP_LIVE);
  // Its fields still read, and the chat names it.
  live.error = NE_OK;
  a := n53_read(live);
  #assert a.error == N53_OK && a.status == N53_LIVE && a.starts == 1687182672 && n32_is(a.d, "demo-cf-stream");
  #assert n53_chat_in(chat, live) == N53_OK;
  // Live at created_at plus an hour, ended a second later.
  #assert n53_state(a, live.created_at, 1687182672 + 3600) == N53_LIVE && n53_state(a, live.created_at, 1687182672 + 3601) == N53_ENDED;
}
#test "the roles in the example's events" {
  buf: [EVENTS][2048] u8; s: [EVENTS][4096] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..], s[..]);
  v := n53_read(ev[1]); #assert v.n == 2 && n32_is(v.p[0].role, "Host") && n32_is(v.p[1].role, "Speaker") && v.hosts == 1;
  sp := n53_read(ev[5]); #assert sp.status == N53_OPEN && n32_is(sp.p[1].role, "Moderator") && sp.hosts == 1;
  m := n53_read(ev[6]); #assert m.has_space && ns_equal32(m.space_author, key_of(ALICE)) && n32_is(m.space_d, "coax-hall") && m.hosts == 0;
  p := n53_presence(ev[7]); #assert p.error == N53_OK && p.room_kind == N53_SPACE && p.hand;
}
```

```shell
coax/bin/coax test nostr/examples/checks/live-activities.coax --run-steps 250000000
```

`coax test` runs 394 checks, including the loaded files' own checks; all
pass.

### The library's own checks

`nip53.coax` has its own `#test` blocks: NIP-53's stream, chat, space and
meeting examples (placeholders filled with real keys), the first-of-each
rule and which tag names belong to which kind, every refusal and the tag
index it reports, roles and proofs, the eight-participant bound at its
edge, forged and oversized tags, the hour boundary, presence and its hand
flag, and both writers, with their output read back and every short
output buffer refused.

```shell
coax/bin/coax test coax/lib/nostr/nip53.coax
coax/bin/coax mutate coax/lib/nostr/nip53.coax --root-only
```

On 2026-11-01, `coax test` passed all 240 checks (with the loaded files').
`coax mutate --root-only` killed all 438 mutants of `nip53.coax`, with
none surviving and none marked equivalent. The full mutation run
(`COAX_MUTATION_FULL=1`) and a mutation run of the example were not run.

## Summary

- `n53_read` reads a stream, a space or a meeting: the first of each
  field, a status from the kind's closed set, times in unix seconds, and
  up to eight participants with their roles.
- `n53_state` shows a `live` stream or meeting as `ended` when its current
  version is more than 3,600 seconds older than the clock the caller
  passes.
- `n53_chat_in` accepts a chat message only when its `a` tag names the
  stream's own author and `d`.
- `n53_tags` and `n53_chat_tags` write only events that the reader
  accepts.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Sources

- [NIP-53](../nips/official/53.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
