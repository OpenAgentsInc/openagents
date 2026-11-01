# 90. Artifacts, archives and reminders

Channels (chapter 89) carry messages. Three of Block's NIPs add state that
outlives a message: an editable record with a home channel (NIP-AR), a
relay's statement that a pubkey is retired there (NIP-IA), and a private
reminder that becomes due at a time (NIP-ER). This chapter builds the
relay's and the client's decisions for all three in Coax.

Part X, Every NIP. Result: NIP-AR channel artifacts (45010) with
compare-and-swap revisions, NIP-IA identity archival with consent, and
NIP-ER event reminders (30300). What is not built is listed under
[What is not built](#what-is-not-built).

## What this chapter teaches

- AR: an artifact's identity, the envelope tags, why the relay accepts a
  revision only when it names the current one, access by channel, moving
  and deleting, and what current state means.
- IA: how a relay decides an archive or unarchive request (form,
  freshness, consent path), and why archival is not revocation: nothing is
  banned or deleted.
- ER: what a reminder hides and what it shows, which version of an address
  counts, and how a client notifies once, at the reminder's time.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip29.coax](../../coax/lib/nostr/nip29.coax) | `g29_tag` | The tag reader all three files reuse (chapter 59) |
| [nipar.coax](../../coax/lib/nostr/nipar.coax) | `ar_envelope(e)` | Reads and checks the envelope tags of a 45010 |
| [nipar.coax](../../coax/lib/nostr/nipar.coax) | `ar_submit(arts, n, e, writable)` | The relay's compare-and-swap: create, update, move, delete, restore |
| [nipar.coax](../../coax/lib/nostr/nipar.coax) | `ar_current`, `ar_lookup`, `ar_revision_visible` | The read rules |
| [nipia.coax](../../coax/lib/nostr/nipia.coax) | `ia_read(e)` | The form of a 9035 or 9036 request |
| [nipia.coax](../../coax/lib/nostr/nipia.coax) | `ia_auth`, `ia_profile` | The two owner-of-agent proofs, reusing NIP-OA (`nipoa.coax`) |
| [nipia.coax](../../coax/lib/nostr/nipia.coax) | `ia_decide(e, now, admins, banned, profile)` | Freshness and the consent path |
| [nipia.coax](../../coax/lib/nostr/nipia.coax) | `ia_apply`, `ia_delta`, `ia_snapshot` | The archive set and the tags of the 8002, 8003 and 13535 |
| [nipia.coax](../../coax/lib/nostr/nipia.coax) | `ia_list(e, relay, out)` | A client's reading of the relay's 13535 |
| [niper.coax](../../coax/lib/nostr/niper.coax) | `er_number`, `er_read(e)` | The `not_before` rule and the outer event |
| [niper.coax](../../coax/lib/nostr/niper.coax) | `er_store`, `er_wins` | One head per reminder address |
| [niper.coax](../../coax/lib/nostr/niper.coax) | `er_visible`, `er_gate`, `er_relay_due` | Author-only reads and the relay's due push |
| [niper.coax](../../coax/lib/nostr/niper.coax) | `er_open`, `er_content` | Decrypting with the author's own key, and the plaintext rules |
| [niper.coax](../../coax/lib/nostr/niper.coax) | `er_fire`, `er_deliver`, `er_tick` | The client's check at its clock and the notification path |

All three library files start with `#authority;`: NIP-AR decides who may
write and read an artifact, NIP-IA decides which requests change relay
state, and NIP-ER decides who may read a reminder and when it notifies.

## Channel artifacts

An artifact is a kind 45010 event. Each event is a complete snapshot of
the record, and the relay, not the author's clock, decides which snapshot
is current. Clients define what the record means (`buzz.task`,
`buzz.project`); the relay only keeps identity, access and revisions.

### The envelope

| Tag | Rule |
| --- | --- |
| `ar` | `1`; anything else is an unsupported version |
| `d` | The artifact's UUID: lowercase, hyphenated, not the nil UUID. Never changes |
| `h` | The home channel's UUID |
| `type` | Dot-separated lowercase components, each a letter then letters, digits, `_` or `-`, at most 128 bytes. Never changes |
| `title` | Nonblank, at most 512 bytes; absent on `delete` |
| `op` | `create`, `update`, `move`, `delete` or `restore` |
| `root` | Optional conversation anchor, 64 lowercase hex |
| `prev` | The current accepted revision's id; absent on `create`, required otherwise |

Each envelope tag has exactly two elements and occurs once. More than 256
tags refuse. A delete carries empty content and only envelope and `auth`
tags. Other tags are the client's, for filtering; they never override the
envelope or grant access.

<!-- source: coax/lib/nostr/nipar.coax read -->
```jai
ar_envelope :: (e:NostrEvent)->ArEnvelope {
  r:ArEnvelope;seen:[8]int;k:=0;others:=false;
  while true {
    t:=g29_tag(e,k);
    if t.error==NT_INDEX {break;}
    k+=1;
    if k>AR_MAX_TAGS {return ar_fail(r,AR_TAGS);}
    if t.error==NT_LIMIT {return ar_fail(r,AR_LIMIT);}
    if t.error!=NT_OK {return ar_fail(r,AR_SHAPE);}
    w:=-1;for i:0..<8 {if g29_tag_is(t,0,AR_NAMES[i]) {w=i;}}
    if w<0 {if !g29_tag_is(t,0,"auth") {others=true;}continue;}
    seen[w]+=1;
    if seen[w]>1 || t.values!=2 {return ar_fail(r,AR_SHAPE);}
    v:=t.v[1];ok:=true;
    if w==0 {if !n32_is(v,"1") {return ar_fail(r,AR_VERSION);}}
    else if w==1 {ok=ar_uuid(v);r.d=ar_text(v);}
    else if w==2 {ok=ar_uuid(v);r.h=ar_text(v);}
    else if w==3 {ok=ar_type(v);r.type=ar_text(v);}
    else if w==4 {ok=!ar_blank(v);}
    else if w==5 {for i:0..<5 {if n32_is(v,AR_OPS[i]) {r.op=i+1;}}ok=r.op!=0;}
    else if w==6 {r.has_root=true;ok=g29_key(v,r.root[..]);}
    else {r.has_prev=true;ok=g29_key(v,r.prev[..]);}
    if !ok {return ar_fail(r,AR_SHAPE);}
  }
  if seen[0]!=1 || seen[1]!=1 || seen[2]!=1 || seen[3]!=1 || seen[5]!=1 {return ar_fail(r,AR_SHAPE);}
  del:=r.op==AR_DELETE;
  if (seen[4]==1)==del || r.has_prev==(r.op==AR_CREATE) {return ar_fail(r,AR_SHAPE);}
  if del && (others || e.content_to!=e.content_from) {return ar_fail(r,AR_SHAPE);}
  return r;
}
```

The tag reader decodes values of up to 256 bytes (`NT_MAX_VALUE`), so a
title of 257 to 512 bytes refuses here as `AR_LIMIT`, which is stricter
than NIP-AR's 512.

### Identity and edits

An artifact is identified by its `d`. A `create` needs an unused `d`.
Every later revision names the current accepted revision in `prev`, and
the relay advances the current revision in the same step as it checks
it. Two edits made from the same revision therefore cannot both succeed:
the first to arrive becomes current, and the second names a revision that
is no longer current, which is `AR_CONFLICT`. Timestamps never choose the
winner. A client that sees a conflict fetches the current revision and
reconciles; resubmitting with only `prev` replaced would overwrite the
other edit.

An event the relay already accepted succeeds again without being applied
(`AR_AGAIN`), even after later revisions, so a client that lost an
acknowledgement never sees a false conflict.

<!-- source: coax/lib/nostr/nipar.coax submit -->
```jai
ar_submit :: (arts:[]ArArtifact,n:[]int,e:NostrEvent,writable:[]ArText)->ArResult {
  if e.error!=NE_OK || !e.verified {return ar_result(AR_EVENT,-1);}
  if e.kind!=AR_KIND_ARTIFACT {return ar_result(AR_KIND,-1);}
  env:=ar_envelope(e);
  if env.status!=AR_OK {return ar_result(env.status,-1);}
  i:=-1;for k:0..<n[0] {if ar_same(arts[k].d,env.d) {i=k;}}
  if i>=0 {for k:0..<arts[i].ids_n {if ns_equal32(arts[i].ids[k],e.id) {return ar_result(AR_AGAIN,i);}}}
  if env.op==AR_CREATE {
    if i>=0 {return ar_result(AR_TAKEN,-1);}
    if !ar_can(writable,env.h) {return ar_result(AR_DENIED,-1);}
    if n[0]==arts.count {return ar_result(AR_FULL,-1);}
    i=n[0];n[0]+=1;
    arts[i]=ArArtifact.{d=env.d,type=env.type};
  } else {
    if i<0 {return ar_result(AR_UNKNOWN,-1);}
    a:=arts[i];
    if !ar_can(writable,a.h) || (env.op==AR_MOVE && !ar_can(writable,env.h)) {return ar_result(AR_DENIED,i);}
    if !ar_same(a.type,env.type) {return ar_result(AR_TYPE,i);}
    if a.deleted!=(env.op==AR_RESTORE) {return ar_result(AR_STATE,i);}
    if !ns_equal32(a.current,env.prev) {return ar_result(AR_CONFLICT,i);}
    if ar_same(a.h,env.h)==(env.op==AR_MOVE) {return ar_result(AR_HOME,i);}
    if env.op==AR_DELETE && (a.has_root!=env.has_root || !ns_equal32(a.root,env.root)) {return ar_result(AR_ROOT,i);}
    if a.ids_n==AR_MAX_REVISIONS {return ar_result(AR_FULL,i);}
  }
  r:=ar_result(AR_OK,i);
  if env.op==AR_MOVE {r.moved=true;r.from=arts[i].h;r.replaced=arts[i].current;}
  arts[i].h=env.h;arts[i].has_root=env.has_root;arts[i].root=env.root;
  arts[i].current=e.id;arts[i].deleted=env.op==AR_DELETE;
  arts[i].ids[arts[i].ids_n]=e.id;arts[i].homes[arts[i].ids_n]=env.h;arts[i].ids_n+=1;
  return r;
}
```

The order of the checks matters for what a refusal reveals. Permission in
the current home comes before the type, state, `prev` and home checks, so
a sender who can't write there learns only `AR_DENIED`. A create whose
`d` is taken says only `AR_TAKEN`.

### Access, moves and deletes

Writing needs write permission in the home channel, the same check as
posting a kind-9 message; the caller passes the channels the sender may
write. A move needs write permission in both channels. An `update`,
`delete` or `restore` keeps `h`; a `move` changes it. A move returns the
source channel and the replaced revision, for the relay's removal notice
there.

A delete is a soft delete: the artifact leaves current-state queries, its
`d` stays reserved, and only `restore` is accepted after it. A delete
keeps the artifact's `root`.

<!-- source: coax/lib/nostr/nipar.coax access -->
```jai
// In current-state queries: not deleted, and the reader can read its home.
ar_current :: (a:ArArtifact,readable:[]ArText)->bool {return !a.deleted && ar_can(readable,a.h);}
// A lookup by d: the current revision, a deletion included.
ar_lookup :: (a:ArArtifact,readable:[]ArText)->bool {return ar_can(readable,a.h);}
// The k'th accepted revision, under the access of the channel it was in.
ar_revision_visible :: (a:ArArtifact,k:int,readable:[]ArText)->bool {return k>=0 && k<a.ids_n && ar_can(readable,a.homes[k]);}
```

Every read needs read permission in the channel the revision belongs to.
After a move the destination reads the current revision, but not the
revisions accepted in the source.

## Identity archival

A relay accumulates stale pubkeys: rotated keys, retired bots, agents from
old worktrees. NIP-IA lets the relay say "this pubkey is retired here":
clients hide it from member lists and autocomplete on that relay, and its
history stays. Archival is not revocation. It does not ban, remove or
delete anything; relay access is NIP-43's (`nip43.coax`), and archive state
from one relay says nothing about another.

| Kind | Signer | What it is |
| --- | --- | --- |
| 9035 | user | Archive request |
| 9036 | user | Unarchive request |
| 8002 | relay | Archived identity delta |
| 8003 | relay | Unarchived identity delta |
| 13535 | relay | The current archive set (replaceable) |

### Deciding a request

`ia_read` does the form checks of the relay processing algorithm: a
verified 9035 or 9036, exactly one NIP-70 `["-"]` tag, exactly one `p` tag
naming the target in lowercase hex, and a `replaced-by`, when present,
that is lowercase hex and not the target. `ia_decide` adds freshness (120
seconds either way of the relay's clock) and picks the consent path:

1. Self: the actor is the target. An `auth` tag is then ignored. A target
   that access control bans can't self-unarchive; any other target always
   can, which is NIP-IA's protection against silent hiding.
2. Admin: the relay's admin policy lists the actor.
3. Owner: the actor proves owner-of-agent authority with NIP-OA. On the
   request itself, the `auth` tag's owner must be the actor, the signature
   must be over the target's preimage `nostr:agent-auth:<target>:<conditions>`,
   and the request's `created_at` must meet the time clauses; `kind=`
   clauses never deny. In the target's latest kind 0, no clause is
   evaluated, because the profile is a standing declaration.

Anything else is `IA_DENIED`.

<!-- source: coax/lib/nostr/nipia.coax decide -->
```jai
// A request at the relay's clock now: its form, freshness, and the consent
// path. admins is the relay's admin policy; banned says the target is barred
// by access control; profile holds the target's latest kind 0, if any.
ia_decide :: (e:NostrEvent,now:int,admins:[][32]u8,banned:bool,profile:[]NostrEvent)->IaRequest {
  r:=ia_read(e);
  if r.status!=IA_OK {return r;}
  if e.created_at<now-IA_FRESH || e.created_at>now+IA_FRESH {return ia_fail(r,IA_STALE);}
  if ns_equal32(r.actor,r.target) {
    if !r.archive && banned {return ia_fail(r,IA_BANNED);}
    r.consent=IA_SELF;r.by=r.target;return r;
  }
  r.by=r.actor;
  for a:admins {if ns_equal32(a,r.actor) {r.consent=IA_ADMIN;return r;}}
  if ia_auth(e,r.target) {r.consent=IA_OWNER;return r;}
  if profile.count==1 && ia_profile(profile[0],r.target,r.actor) {r.consent=IA_OWNER;r.proof=true;r.proof_id=profile[0].id;return r;}
  return ia_fail(r,IA_DENIED);
}
```

`nipoa.coax`'s `noa_read` refuses an `auth` tag whose owner is the event's
author (`NOA_SELF`) and hashes the author's preimage. On the request path
NIP-IA expects the owner to be the author and the target to be in the
preimage, so `ia_auth` reads the tag itself and reuses `noa_conditions`,
`noa_hash` and the BIP-340 verifier. The profile path is an ordinary
NIP-OA tag on the agent's own event, so `ia_profile` calls `noa_read` and
`noa_signed` directly.

### State, deltas and the snapshot

`ia_apply` is idempotent: archiving an archived key or unarchiving a key
that isn't archived returns `IA_SAME`, and the relay then publishes no
delta and no snapshot. When the set changes, `ia_delta` writes the 8002 or
8003 tags (the marker, the target, the consent path and its key, the
request id, then the reason and replacement) and `ia_snapshot` writes the
13535 tags. A client counts only a 13535 signed by the relay's NIP-11
`self` key with exactly one `["-"]`, and skips an invalid `p` entry.

## Event reminders

A reminder is a kind 30300 addressable event. The relay sees the author,
the `d` tag, `created_at`, the ciphertext's length and one public tag,
`not_before`: when the reminder is due. The target, the note and the
status are NIP-44 encrypted to the author's own key.

### The outer event

`not_before` is a decimal Unix time: ASCII digits only, no sign and no
leading zero except `0`, at most 9007199254740991 (2^53 − 1), read without
floating point. An event has exactly one nonempty `d`, at most one
`not_before`, and an `expiration`, when both are present, later than
`not_before`. A malformed `not_before` is the relay's
`invalid: malformed not_before`. A `not_before` in the future is never a
reason to refuse.

<!-- source: coax/lib/nostr/niper.coax number -->
```jai
// A decimal Unix time: its value, or -1.
er_number :: (s:N32Text)->int {
  c:=s;if c.n==0 || (c.b[0]=='0' && c.n>1) {return -1;}
  v:=0;
  for i:0..<c.n {
    d:=c.b[i];
    if d<'0' || d>'9' {return -1;}
    v=v*10+cast(int)(d-'0');
    if v>ER_MAX_SAFE {return -1;}
  }
  return v;
}
```

### Heads and access

The relay keeps only the latest version of each `(author, 30300, d)`
address: the highest `created_at`, ties to the lowest id. An older version
is dropped, and any due delivery for it with it. The relay can't read the
status, so it decides only by this order.

<!-- source: coax/lib/nostr/niper.coax store -->
```jai
er_same :: (a:ErReminder,b:ErReminder)->bool {x:=b;return ns_equal32(a.author,b.author) && n32_is(a.d,cast(string)x.d.b[0..<x.d.n]);}
er_lower :: (a:[32]u8,b:[32]u8)->bool {
  for i:0..<32 {if a[i]<b[i] {return true;}if a[i]>b[i] {return false;}}
  return false;
}
// Does a replace b at the same address? Later created_at, then lower id.
er_wins :: (a:ErReminder,b:ErReminder)->bool {
  if a.created_at>b.created_at {return true;}
  if a.created_at<b.created_at {return false;}
  return er_lower(a.id,b.id);
}
// The relay keeps one head per address: ER_OK when r is stored (new, or it
// replaced the head), ER_OLDER when the head stays.
er_store :: (heads:[]ErReminder,n:[]int,r:ErReminder)->int {
  if r.status!=ER_OK {return r.status;}
  for i:0..<n[0] {
    if !er_same(heads[i],r) {continue;}
    if !er_wins(r,heads[i]) {return ER_OLDER;}
    heads[i]=r;return ER_OK;
  }
  if n[0]==heads.count {return ER_FULL;}
  heads[n[0]]=r;n[0]+=1;return ER_OK;
}
```

Only the authenticated author reads a reminder. A filter for 30300 alone
from someone who hasn't authenticated closes with `auth-required:`; from
another key, `restricted:`. A filter that mixes kinds goes on with the
30300 matches omitted.

### The plaintext

`er_open` decrypts the content with the author's NIP-44 conversation key
with themselves (`nl_self`, chapter 51). The payload is base64, so the
JSON string is used as written. `er_content` then requires one JSON
object with no duplicate member names, a `status` of `pending`, `done` or
`cancelled`, a `target.id` of 64 lowercase hex, a `target.a` that is a
valid address, `relays` an array, `preview` and `note` strings, and a
pending reminder with a target reference or a nonempty note. Unknown
members are skipped. `target.id` and `target.a` are compared as written,
so an escape in either refuses.

### Firing once, at the time

A client notifies only for a head whose plaintext is pending and whose
`not_before` is at or before its own clock. A done or cancelled head never
notifies, even if a faulty client left `not_before` on it.

<!-- source: coax/lib/nostr/niper.coax fire -->
```jai
// The client's local check for the head r with plaintext c at its clock.
er_fire :: (r:ErReminder,c:ErContent,now:int)->int {
  if r.status!=ER_OK || c.status!=ER_OK {return ER_SHAPE;}
  if c.state!=ER_PENDING {return ER_TERMINAL;}
  if !r.has_nb {return ER_BOOKMARK;}
  return ifx r.nb<=now then ER_FIRE else ER_WAIT;
}
// The notification path: what is held per address, and the ids notified.
ErNotifier :: struct {held:[ER_MAX_HELD]ErReminder; held_n:int; fired:[ER_MAX_FIRED][32]u8; fired_n:int;}
er_deliver :: (nt:[]ErNotifier,r:ErReminder,c:ErContent,now:int)->int {
  if r.status!=ER_OK {return r.status;}
  k:=0;
  for i:0..<nt[0].held_n {if !er_same(nt[0].held[i],r) {nt[0].held[k]=nt[0].held[i];k+=1;}}
  nt[0].held_n=k;
  f:=er_fire(r,c,now);
  if f!=ER_FIRE && f!=ER_WAIT {return f;}
  if r.nb>now+ER_WINDOW {return ER_DISCARD;}
  if k==ER_MAX_HELD {return ER_FULL;}
  nt[0].held[k]=r;nt[0].held_n+=1;return ER_HELD;
}
// At now, the held reminders that are due: their ids into out, each id once
// ever; they leave the held set.
er_tick :: (nt:[]ErNotifier,now:int,out:[][32]u8)->int {
  m:=0;k:=0;
  for i:0..<nt[0].held_n {
    h:=nt[0].held[i];
    if h.nb>now {nt[0].held[k]=h;k+=1;continue;}
    seen:=false;for f:0..<nt[0].fired_n {if ns_equal32(nt[0].fired[f],h.id) {seen=true;}}
    if seen || m==out.count || nt[0].fired_n==ER_MAX_FIRED {continue;}
    out[m]=h.id;m+=1;nt[0].fired[nt[0].fired_n]=h.id;nt[0].fired_n+=1;
  }
  nt[0].held_n=k;return m;
}
```

The notification path follows NIP-ER's stateless profile. Every delivery
replaces what is held for its address, including deliveries that are
themselves discarded or terminal. A delivery more than 60 seconds early
(`ER_WINDOW`) is discarded, because a push-mode relay sends it again when
it is due. A held reminder notifies at `not_before`, and an event id
notifies once: the relay may send the same event again, and the path
remembers the ids it has notified.

## Worked example: three reminders by hand

Alice has three reminders. The clock is 1770000000.

| Address | Versions | Head |
| --- | --- | --- |
| A, `d` a3f8c2e1… | created 1769990000, `not_before` 1770000000, pending "Submit travel receipt" | that one |
| B, `d` 5c1e0f3a… | B1 created 1769990100, `not_before` 1769999990; B2 (a snooze) created 1769999995, `not_before` 1770086400, both pending | B2: 1769999995 > 1769990100 |
| C, `d` e7b2d4c1… | C1 created 1769990200, `not_before` 1769999000, pending; C2 created 1769999100, cancelled, no `not_before`, `expiration` 1777542730 | C2: 1769999100 > 1769990200 |

At the clock:

1. A: pending, and 1770000000 ≤ 1770000000. It fires. On the notification
   path it is not early (0 ≤ 60 seconds), so it is held and notifies now.
2. B2: pending, and 1770086400 > 1770000000. It waits. It is 86400
   seconds early, more than 60, so the path discards it. B1 was due at
   1769999990, but B1 is no longer the head, so it never fires.
3. C2: cancelled. It never fires. C1 was due at 1769999000, but it was
   replaced.

So exactly one reminder, A, notifies at 1770000000. When the relay sends A
again, the path holds it but doesn't notify, because its id has already
notified. At 1770086400 the relay's push (`er_relay_due`: B2's
`not_before` ≤ the clock) delivers B2, the path holds it and B2 notifies
once. The due set at 1770086400 is A and B2; A has already notified.

The artifact runs the same way: create `d9c279bf…`; Alice's update
`2eb4cede…` names it and becomes current; Bob's update also names
`d9c279bf…`, which is no longer current, so it conflicts; the move, delete
and restore each name the revision before; Alice's update sent again is
accepted before; Bob's create of the same `d` is taken. The current
revision is the restore, `30a81354…`.

## What is not built

- NIP-AR: the `root` anchor's existence and channel checks (the relay's
  message store), relationship-tag queries over current state and their
  advertised limits, the removal event's tags for a move, NIP-29 kind-9005
  redaction and retention, rejecting kind-5 deletions of revisions,
  mention and unread suppression, and titles of 257 to 512 bytes.
- NIP-IA: the relay-initiated `relay` consent path, rejected-request audit
  storage, snapshot rollback tracking (the highest accepted `created_at`
  per relay), applying 8002 and 8003 deltas on the client, the NIP-11
  `self` lookup, and snapshots larger than `IA_MAX` (32) keys.
- NIP-ER: building and encrypting reminders (the example's events come
  from Python), NIP-09 hard deletion, NIP-11 `supported_extensions` and
  `limitation` fields, horizon limits, the relay's timers, filter parsing
  for the read gate, `target.relays` URL checks, and recovery pagination.

## The example

[nostr/examples/artifacts.coax](../examples/artifacts.coax) runs the
worked example. The NIP-AR and NIP-ER events were signed and encrypted by
an independent BIP-340 and NIP-44 in Python
([artifacts_vectors.py](../../coax/tests/nostr/artifacts_vectors.py)); the
NIP-IA events are NIP-IA's own test vectors. The example verifies every
event before using it.

<!-- source: nostr/examples/artifacts.coax keys -->
```jai
key_of :: (hex:string)->[32]u8 {k:[32]u8;ok:=ne_lower_hex(hex,k[..]);return k;}
// A signed event, read and verified (chapter 21).
verified :: (text:string,out:[]u8)->NostrEvent {return nostr_event_verify_vartime(text,out,false);}
id8 :: (id:[32]u8,out:[]u8)->string {n:=g29_put_key(out,0,id);return cast(string)out[1..<9];}
uuid :: (s:string)->ArText {t:ArText;for i:0..<s.count {t.b[i]=s[i];}t.n=s.count;return t;}
```

The artifact's eight submissions:

<!-- source: nostr/examples/artifacts.coax artifacts -->
```jai
part_ar :: () {
  print("NIP-AR: a task in channel 9b353519…\n");
  h1:=uuid("9b353519-f4fe-4757-aef4-bec6cc0ae54c");h2:=uuid("17cfe3ca-b3d0-4a76-a2a4-4eaf0d3a4109");
  alice:=[2]ArText.[h1,h2];bob:=[1]ArText.[h1];
  arts:[2]ArArtifact;n:[1]int;out:[16384]u8;h:[80]u8;
  names:=[8]string.["Alice creates it","Alice updates from the create","Bob updates from the create","Alice moves it to 17cfe3ca…","Alice deletes it","Alice restores it","Alice resends her update","Bob creates the same d"];
  texts:=[8]string.[ARV_CREATE,ARV_UPDATE_ALICE,ARV_UPDATE_BOB,ARV_MOVE,ARV_DELETE,ARV_RESTORE,ARV_UPDATE_ALICE,ARV_CREATE_AGAIN];
  who:=[8]bool.[true,true,false,true,true,true,true,false];
  for i:0..<8 {
    e:=verified(texts[i],out[..]);
    r:=ar_submit(arts[..],n[..],e,ifx who[i] then alice[..] else bob[..]);
    print("%: %",names[i],AR_WORDS[r.status]);
    if r.status==AR_OK {print(", current %…",id8(arts[0].current,h[..]));}
    if r.moved {print("; a removal notice goes to %… for %…",cast(string)r.from.b[0..<8],id8(r.replaced,h[..]));}
    print("\n");
  }
  a:=arts[0];
  print("current revision %…, % accepted revisions, deleted %\n",id8(a.current,h[..]),a.ids_n,a.deleted);
  print("a reader of 9b353519… sees it in current state %; of 17cfe3ca… %\n",ar_current(a,bob[..]),ar_current(a,alice[1..<2]));
  print("history: revision 1 (in 9b353519…) readable from 17cfe3ca… %, from 9b353519… %\n",ar_revision_visible(a,1,alice[1..<2]),ar_revision_visible(a,1,bob[..]));
}
```

The archive and unarchive chain from NIP-IA's vectors:

<!-- source: nostr/examples/artifacts.coax archive -->
```jai
show_tags :: (label:string,mine:string,published:string) {
  print("%: equal to the published tags %\n",label,mine==published);
}
part_ia :: (relay:[32]u8) {
  print("NIP-IA: the owner archives a zombie agent\n");
  out:[16384]u8;tags:[1024]u8;nk:[1][32]u8;none:=nk[0..<0];np:[1]NostrEvent;
  s:[1]IaState;
  e:=verified(IAV_1,out[..]);
  r:=ia_decide(e,1713956401,none,false,np[0..<0]);
  print("vector 1, a 9035 from the owner: %, consent %\n",IA_WORDS[r.status],IA_PATHS[r.consent]);
  print("apply: %; again: %\n",IA_WORDS[ia_apply(s[..],r)],IA_WORDS[ia_apply(s[..],r)]);
  m:=ia_delta(r,tags[..]);show_tags("the relay's 8002 tags",cast(string)tags[0..<m],IAV_2_TAGS);
  m=ia_snapshot(s[0],tags[..]);show_tags("the relay's 13535 tags",cast(string)tags[0..<m],IAV_3_TAGS);
  keys:[4][32]u8;h:[80]u8;
  k:=ia_list(verified(IAV_3,out[..]),relay,keys[..]);
  print("a client reads vector 3: % archived, %…\n",k,id8(keys[0],h[..]));
  print("NIP-IA: the agent unarchives itself\n");
  e=verified(IAV_4,out[..]);
  r=ia_decide(e,1713956501,none,false,np[0..<0]);
  print("vector 4, a 9036 from the agent: %, consent %; apply: %\n",IA_WORDS[r.status],IA_PATHS[r.consent],IA_WORDS[ia_apply(s[..],r)]);
  m=ia_delta(r,tags[..]);show_tags("the relay's 8003 tags",cast(string)tags[0..<m],IAV_5_TAGS);
  m=ia_snapshot(s[0],tags[..]);print("the new 13535 tags: %\n",cast(string)tags[0..<m]);
  print("broken inputs\n");
  print("vector 1 replayed 1000 s later: %\n",IA_WORDS[ia_decide(verified(IAV_1,out[..]),1713957400,none,false,np[0..<0]).status]);
  print("vector 4 from a banned agent: %\n",IA_WORDS[ia_decide(verified(IAV_4,out[..]),1713956501,none,true,np[0..<0]).status]);
  print("vector 3 checked against Alice's key as the relay: %\n",IA_WORDS[-ia_list(verified(IAV_3,out[..]),key_of(ARV_ALICE),keys[..])]);
}
```

The three reminders at the clock:

<!-- source: nostr/examples/artifacts.coax reminders -->
```jai
ER_WORD :: (s:int)->string {
  words:=[6]string.["fire","wait","terminal","bookmark","held","discard"];
  if s>=ER_FIRE && s<=ER_DISCARD {return words[s-ER_FIRE];}
  if s==ER_OLDER {return "older: dropped";}
  if s==ER_NOT_BEFORE {return "invalid: malformed not_before";}
  if s==ER_AUTH_REQUIRED {return "auth-required:";}
  if s==ER_RESTRICTED {return "restricted:";}
  return ifx s==ER_OK then "stored" else "refused";
}
part_er :: () {
  print("NIP-ER: Alice's three reminders at the clock %\n",NOW);
  alice:=key_of(ARV_ALICE);secret:[32]u8;secret[31]=1;
  me:[1]NlSelf;ec:[1]EcdhScratch;s:=nl_self(secret[..],ec[..],me[..]);
  heads:[4]ErReminder;n:[1]int;out:[16384]u8;
  names:=[5]string.["A","B1","B2","C1","C2"];texts:=[5]string.[ERV_A,ERV_B1,ERV_B2,ERV_C1,ERV_C2];
  for i:0..<5 {
    r:=er_read(verified(texts[i],out[..]));
    print("relay stores %: %\n",names[i],ER_WORD(er_store(heads[..],n[..],r)));
  }
  print("B1 arrives again: %; % addresses held\n",ER_WORD(er_store(heads[..],n[..],er_read(verified(ERV_B1,out[..])))),n[0]);
  // The relay's heads, in the order it stored them: A, B, C.
  heads_text:=[3]string.[ERV_A,ERV_B2,ERV_C2];head_names:=[3]string.["A","B2","C2"];
  plain:[512]u8;work:[1]NlWork;contents:[3]ErContent;
  nt:[1]ErNotifier;fired:[4][32]u8;h:[80]u8;
  for i:0..<3 {
    e:=verified(heads_text[i],out[..]);
    m:=er_open(e,me[0],work[..],plain[..]);
    contents[i]=er_content(cast(string)plain[0..<m]);
    print("% opens to %\n  local check: %; notification path: %\n",head_names[i],cast(string)plain[0..<m],
          ER_WORD(er_fire(heads[i],contents[i],NOW)),ER_WORD(er_deliver(nt[..],heads[i],contents[i],NOW)));
  }
  k:=er_tick(nt[..],NOW,fired[..]);
  print("at %: % notification, %…\n",NOW,k,id8(fired[0],h[..]));
  print("the relay sends A again: %; notifications: %\n",ER_WORD(er_deliver(nt[..],heads[0],contents[0],NOW+5)),er_tick(nt[..],NOW+5,fired[..]));
  print("at %: the relay pushes B2: %; client: %; notifications: %",LATER,er_relay_due(heads[1],LATER),
        ER_WORD(er_deliver(nt[..],heads[1],contents[1],LATER)),er_tick(nt[..],LATER,fired[..]));
  print(", %…\n",id8(fired[0],h[..]));
  print("broken inputs\n");
  print("a not_before of \"01770000000\": %\n",ER_WORD(er_read(verified(ERV_BAD_NB,out[..])).status));
  bob:=key_of(ARV_BOB);mine:=[1][32]u8.[alice];
  print("Bob asks for Alice's reminders: %; an unauthenticated reader: %\n",ER_WORD(er_gate(true,bob,mine[..],true)),ER_WORD(er_gate(false,bob,mine[..],true)));
  print("Bob opens A with his own keys: %\n",ifx er_open(verified(ERV_A,out[..]),NlSelf.{ok=true},work[..],plain[..])== -ER_NOT_OWNER then "not the owner" else "opened");
}
```

The output:

```shell
coax/bin/coax run nostr/examples/artifacts.coax
```

Output:

```text
NIP-AR: a task in channel 9b353519…
Alice creates it: ok, current d9c279bf…
Alice updates from the create: ok, current 2eb4cede…
Bob updates from the create: conflict
Alice moves it to 17cfe3ca…: ok, current b7ea4baa…; a removal notice goes to 9b353519… for 2eb4cede…
Alice deletes it: ok, current 3137d815…
Alice restores it: ok, current 30a81354…
Alice resends her update: accepted before: success, not applied
Bob creates the same d: taken
current revision 30a81354…, 5 accepted revisions, deleted false
a reader of 9b353519… sees it in current state false; of 17cfe3ca… true
history: revision 1 (in 9b353519…) readable from 17cfe3ca… false, from 9b353519… true
NIP-IA: the owner archives a zombie agent
vector 1, a 9035 from the owner: ok, consent owner
apply: ok; again: same
the relay's 8002 tags: equal to the published tags true
the relay's 13535 tags: equal to the published tags true
a client reads vector 3: 1 archived, c6047f94…
NIP-IA: the agent unarchives itself
vector 4, a 9036 from the agent: ok, consent self; apply: ok
the relay's 8003 tags: equal to the published tags true
the new 13535 tags: [["-"]]
broken inputs
vector 1 replayed 1000 s later: stale
vector 4 from a banned agent: banned
vector 3 checked against Alice's key as the relay: signer
NIP-ER: Alice's three reminders at the clock 1770000000
relay stores A: stored
relay stores B1: stored
relay stores B2: stored
relay stores C1: stored
relay stores C2: stored
B1 arrives again: older: dropped; 3 addresses held
A opens to {"status":"pending","note":"Submit travel receipt"}
  local check: fire; notification path: held
B2 opens to {"target":{"a":"30023:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:proposal","id":"7b4f3c2a1e9d8c7061524334aabbccddeeff00112233445566778899aabbccdd","relays":["wss://relay.example"],"preview":"Can you review this before Friday?"},"status":"pending","note":"Follow up before planning"}
  local check: wait; notification path: discard
C2 opens to {"status":"cancelled","note":"Call the bank"}
  local check: terminal; notification path: terminal
at 1770000000: 1 notification, 870b47fb…
the relay sends A again: held; notifications: 0
at 1770086400: the relay pushes B2: true; client: held; notifications: 1, fc26031f…
broken inputs
a not_before of "01770000000": invalid: malformed not_before
Bob asks for Alice's reminders: restricted:; an unauthenticated reader: auth-required:
Bob opens A with his own keys: not the owner
```

The broken inputs and their refusals are in the output: Bob's update from
a stale revision (`conflict`), Bob's create of a taken `d` (`taken`),
vector 1 replayed 1000 seconds later (`stale`), vector 4 from a banned
agent (`banned`), the 13535 checked against the wrong relay key
(`signer`), a `not_before` with a leading zero
(`invalid: malformed not_before`), Bob's read of Alice's reminders
(`restricted:`), an unauthenticated read (`auth-required:`) and Bob's
attempt to open Alice's reminder (`not the owner`).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/artifacts.coax](../examples/checks/artifacts.coax),
which loads the example:

- The artifact's ids and outcomes from
  [artifacts_vectors.py](../../coax/tests/nostr/artifacts_vectors.py),
  which signs with
  [nip59_reference.py](../../coax/tests/nostr/nip59_reference.py)'s
  integer secp256k1, BIP-340 and NIP-44, hashes ids with `hashlib`, and
  applies NIP-AR's `prev` rule in Python. Coax must verify each event,
  recompute its id and give the same eight outcomes and current revision.
- The reminders due at 1770000000 (`A`) and 1770086400 (`A,B2`), computed
  in Python from the heads and the decrypted statuses; Coax decrypts the
  heads with Alice's key and must agree.
- NIP-IA's five published test vectors. Their ids and signatures are
  correct: Coax verifies each event and recomputes the published id, and
  the relay's own 8002, 13535 and 8003 tags equal the published tags byte
  for byte.
- The NIP-AR and NIP-ER examples, as printed. They carry no keys, ids or
  ciphertexts (`<author-pubkey>`, `<event-id>`, `<nip44-ciphertext>`), so
  they have no ids to compare. Their tags and plaintexts read as the NIPs
  say, and NIP-ER's create, snooze and complete replace each other in
  order.

<!-- source: nostr/examples/checks/artifacts.coax check -->
```jai
// From coax/tests/nostr/artifacts_vectors.py, which signs with
// nip59_reference.py's integer secp256k1, BIP-340 and NIP-44 (no code
// shared with Coax), hashes each id as hashlib.sha256 of json.dumps([0,
// pubkey, created_at, kind, tags, content], separators=(",", ":")), applies
// NIP-AR's prev rule to the submissions in order, and picks the reminder
// heads (highest created_at, then lowest id) and the pending ones due at a
// clock.
PY_OUTCOMES :: "ok,ok,conflict,ok,ok,ok,again,taken";

#test "the artifact's ids and outcomes are the Python reference's" #steps 2000000000 {
  texts:=[8]string.[ARV_CREATE,ARV_UPDATE_ALICE,ARV_UPDATE_BOB,ARV_MOVE,ARV_DELETE,ARV_RESTORE,ARV_UPDATE_ALICE,ARV_CREATE_AGAIN];
  ids:=[8]string.[ARV_CREATE_ID,ARV_UPDATE_ALICE_ID,ARV_UPDATE_BOB_ID,ARV_MOVE_ID,ARV_DELETE_ID,ARV_RESTORE_ID,ARV_UPDATE_ALICE_ID,ARV_CREATE_AGAIN_ID];
  words:=[17]string.["ok","again","","","","","","","taken","","","","","conflict","","",""];
  all:=[2]ArText.[uuid("9b353519-f4fe-4757-aef4-bec6cc0ae54c"),uuid("17cfe3ca-b3d0-4a76-a2a4-4eaf0d3a4109")];
  arts:[2]ArArtifact;n:[1]int;out:[16384]u8;h:[80]u8;got:[128]u8;g:=0;
  for i:0..<8 {
    e:=verified(texts[i],out[..]);#assert e.error==NE_OK && e.verified;
    k:=g29_put_key(h[..],0,e.computed_id);#assert cast(string)h[1..<65]==ids[i];
    if i>0 {g=json_raw(got[..],g,",");}
    g=json_raw(got[..],g,words[ar_submit(arts[..],n[..],e,all[..]).status]);
  }
  #assert cast(string)got[0..<g]==PY_OUTCOMES && PY_OUTCOMES==ARV_OUTCOMES;
  k:=g29_put_key(h[..],0,arts[0].current);#assert cast(string)h[1..<65]==ARV_CURRENT;
}

#test "the reminders due at each clock are the Python reference's" #steps 2000000000 {
  texts:=[5]string.[ERV_A,ERV_B1,ERV_B2,ERV_C1,ERV_C2];ids:=[5]string.[ERV_A_ID,ERV_B1_ID,ERV_B2_ID,ERV_C1_ID,ERV_C2_ID];
  heads:[4]ErReminder;n:[1]int;out:[16384]u8;h:[80]u8;
  for i:0..<5 {
    e:=verified(texts[i],out[..]);#assert e.verified;
    k:=g29_put_key(h[..],0,e.computed_id);#assert cast(string)h[1..<65]==ids[i];
    #assert er_store(heads[..],n[..],er_read(e))==ER_OK;
  }
  secret:[32]u8;secret[31]=1;me:[1]NlSelf;ec:[1]EcdhScratch;#assert nl_self(secret[..],ec[..],me[..])==NL_OK;
  heads_text:=[3]string.[ERV_A,ERV_B2,ERV_C2];names:=[3]string.["A","B2","C2"];
  plain:[512]u8;work:[1]NlWork;
  for clock:0..<2 {
    now:=ifx clock==0 then NOW else LATER;due:[16]u8;d:=0;
    for i:0..<3 {
      m:=er_open(verified(heads_text[i],out[..]),me[0],work[..],plain[..]);
      if er_fire(heads[i],er_content(cast(string)plain[0..<m]),now)==ER_FIRE {if d>0 {d=json_raw(due[..],d,",");}d=json_raw(due[..],d,names[i]);}
    }
    #assert cast(string)due[0..<d]==(ifx clock==0 then ERV_DUE_NOW else ERV_DUE_LATER);
  }
}

// NIP-IA publishes five real test vectors. Their ids and signatures are
// correct: Coax verifies each event and finds the published id, and the
// relay's own 8002, 13535 and 8003 tags equal the published ones byte for
// byte.
#test "NIP-IA's published vectors" #steps 2000000000 {
  texts:=[5]string.[IAV_1,IAV_2,IAV_3,IAV_4,IAV_5];ids:=[5]string.[IAV_1_ID,IAV_2_ID,IAV_3_ID,IAV_4_ID,IAV_5_ID];
  out:[16384]u8;h:[80]u8;
  for i:0..<5 {
    e:=verified(texts[i],out[..]);#assert e.error==NE_OK && e.verified;
    k:=g29_put_key(h[..],0,e.computed_id);#assert cast(string)h[1..<65]==ids[i];
  }
  nk:[1][32]u8;none:=nk[0..<0];np:[1]NostrEvent;s:[1]IaState;tags:[1024]u8;
  r:=ia_decide(verified(IAV_1,out[..]),1713956401,none,false,np[0..<0]);#assert r.consent==IA_OWNER && ia_apply(s[..],r)==IA_OK;
  m:=ia_delta(r,tags[..]);#assert cast(string)tags[0..<m]==IAV_2_TAGS;
  m=ia_snapshot(s[0],tags[..]);#assert cast(string)tags[0..<m]==IAV_3_TAGS;
  r=ia_decide(verified(IAV_4,out[..]),1713956501,none,false,np[0..<0]);#assert r.consent==IA_SELF && ia_apply(s[..],r)==IA_OK;
  m=ia_delta(r,tags[..]);#assert cast(string)tags[0..<m]==IAV_5_TAGS;
}

// NIP-AR's and NIP-ER's examples are illustrative: NIP-AR's task has no
// pubkey, id or signature, and NIP-ER's events carry "<author-pubkey>",
// "<event-id>" and "<nip44-ciphertext>" placeholders, so neither has an id
// to compare. Their tags read as the NIPs say.
NIP_AR_TAGS :: "[[\"ar\",\"1\"],[\"d\",\"04737c81-e5e8-4412-bb47-f446813cfeba\"],[\"h\",\"9b353519-f4fe-4757-aef4-bec6cc0ae54c\"],[\"type\",\"buzz.task\"],[\"title\",\"Fix the payment timeout\"],[\"op\",\"create\"],[\"assignee\",\"<assignee-pubkey>\"],[\"project\",\"17cfe3ca-b3d0-4a76-a2a4-4eaf0d3a4109\"]]";
NIP_ER_CREATE :: "[[\"d\",\"a3f8c2e1b4d79600e5d2f1a8c3b6094d\"],[\"not_before\",\"1770000000\"],[\"alt\",\"Encrypted reminder\"]]";
NIP_ER_SNOOZE :: "[[\"d\",\"a3f8c2e1b4d79600e5d2f1a8c3b6094d\"],[\"not_before\",\"1770086400\"],[\"alt\",\"Encrypted reminder\"]]";
NIP_ER_DONE :: "[[\"d\",\"a3f8c2e1b4d79600e5d2f1a8c3b6094d\"],[\"alt\",\"Encrypted reminder\"],[\"expiration\",\"1777542730\"]]";
NIP_ER_PLAIN :: "{\"target\":{\"a\":\"30023:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:proposal\",\"id\":\"7b4f3c2a1e9d8c7061524334aabbccddeeff00112233445566778899aabbccdd\",\"relays\":[\"wss://relay.example\"],\"preview\":\"Can you review this before Friday?\"},\"status\":\"pending\",\"note\":\"Follow up before planning\"}";
#test "the NIPs' published examples, as printed" {
  e:=nt_event(NIP_AR_TAGS);e.kind=45010;e.verified=true;
  env:=ar_envelope(e);#assert env.status==AR_OK && env.op==AR_CREATE && !env.has_prev;
  c:=nt_event(NIP_ER_CREATE);c.kind=30300;c.verified=true;c.created_at=1769990000;
  s:=nt_event(NIP_ER_SNOOZE);s.kind=30300;s.verified=true;s.created_at=1770000100;
  d:=nt_event(NIP_ER_DONE);d.kind=30300;d.verified=true;d.created_at=1770086410;
  heads:[1]ErReminder;n:[1]int;
  #assert er_store(heads[..],n[..],er_read(c))==ER_OK && heads[0].nb==1770000000;
  #assert er_store(heads[..],n[..],er_read(s))==ER_OK && heads[0].nb==1770086400;
  #assert er_store(heads[..],n[..],er_read(d))==ER_OK && !heads[0].has_nb && heads[0].exp==1777542730 && n[0]==1;
  p:=er_content(NIP_ER_PLAIN);#assert p.status==ER_OK && p.has_a && p.has_id && p.state==ER_PENDING;
  #assert er_content("{\"status\":\"pending\",\"note\":\"Submit travel receipt\"}").status==ER_OK;
}

// By hand (the chapter's worked example): at 1770000000 the heads are A
// (pending, not_before 1770000000: due, not_before <= now), B2 (pending,
// not_before 1770086400: 86400 s early, more than the 60 s window) and C2
// (cancelled, no not_before). Only A notifies; at 1770086400 B2 does.
#test "the worked example's values, by hand" {
  #assert NOW==1770000000 && LATER-NOW==86400 && ERV_DUE_NOW=="A" && ERV_DUE_LATER=="A,B2";
  #assert ARV_CURRENT==ARV_RESTORE_ID && ARV_OUTCOMES=="ok,ok,conflict,ok,ok,ok,again,taken";
}
```

### The library's own checks

Each library file's `#test` checks were written before the code. Their
helpers are in
[nipar_fixtures.coax](../../coax/tests/nostr/nipar_fixtures.coax),
[nipia_fixtures.coax](../../coax/tests/nostr/nipia_fixtures.coax) and
[niper_fixtures.coax](../../coax/tests/nostr/niper_fixtures.coax). The
owner-of-agent checks use NIP-IA's NIP-OA tag and three more credentials
signed in Python: one whose window closed, one whose conditions don't read
and one for a different agent.

Run on 2026-11-01 with the compiler from
`~/.cache/coax-release/bcefdc7f…`: `coax test` passes for each file, and
`coax mutate --root-only` killed every mutant: `nipar.coax` 348 of 348,
`nipia.coax` 297 of 297 and `niper.coax` 421 of 421, none marked
equivalent. The full mutation run and the whole-book example run were not
run.

## Summary

- `ar_submit` accepts a revision only when its `prev` is the current
  revision, so of two edits from one revision the second is `AR_CONFLICT`;
  an already accepted event is `AR_AGAIN`, never a conflict.
- Writing needs write permission in the home, and a move in both
  channels; reads follow the channel each revision was accepted in.
- `ia_decide` picks self, admin or owner consent, checks freshness, and
  never lets a ban-free target lose self-unarchive; `ia_apply` publishes
  nothing when nothing changed. Archival never bans or deletes.
- `er_store` keeps one head per address by `created_at`, then id;
  `er_fire` fires only a pending head at or after its `not_before`; the
  notification path notifies each event id once.

## Dependencies

- [89. Channels](ch089-channels.md)

## Sources

- [Block NIP-AR](../nips/block/NIP-AR.md)
- [Block NIP-IA](../nips/block/NIP-IA.md)
- [Block NIP-ER](../nips/block/NIP-ER.md)
- [Block NIP-OA](../nips/block/NIP-OA.md), [NIP-44](../nips/official/44.md)
  and [NIP-01](../nips/official/01.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
