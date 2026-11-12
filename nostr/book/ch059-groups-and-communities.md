# 59. Groups and communities

A group needs a place that enforces who may write to it. NIP-29 makes a
relay that place: the relay keeps each group's members and their roles,
admits a user's event only when the group's rules allow it, and publishes
the group's state as events signed by its own key. This chapter builds
that relay-side decision in Coax, then the one check a client needs for
NIP-72's older moderated communities: whether a moderator approved a post.

Part VI, Every NIP. Result: the relay side of NIP-29 relay-based groups (the
`h` tag, moderation kinds 9000 to 9009, join and leave requests 9021 and
9022, `previous` references, and the 39000 to 39003 state the relay
publishes), NIP-72's approval check, and the client's readers for NIP-C7
chats (kind 9 with `q` replies) and NIP-7D forum threads (kind 11 with
kind 1111 replies). NIP-28 public chat is listed under
[Not built](#not-built).

## What this chapter teaches

- What a NIP-29 group is: an id that a relay enforces rules for. Every event
  sent to a group carries `["h", "<group-id>"]`.
- The state a relay keeps for a group: members and their roles, the
  `restricted` and `closed` flags, invite codes and the ids of recent
  events.
- How moderation events change that state, and how the relay decides who
  may send each one: roles map to permissions by the relay's own policy.
- How join requests work with invite codes, and why a `closed` group
  refuses a request without one.
- What a `previous` tag proves: that the writer saw events from this relay,
  so a copy replayed on a fork's relay refuses.
- Why kinds 39000 to 39005 are accepted only when the relay's own key
  signs them.
- How a NIP-72 community decides that a post counts: a listed moderator
  must approve it.
- What a NIP-C7 chat message is (kind 9), how a reply quotes its parent in
  a `q` tag, and why a chat view fetches kind 9 only.
- What a NIP-7D thread is (kind 11 with a `title`), and why its replies are
  NIP-22 comments whose root and parent are both the thread.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [event.coax](../../coax/lib/nostr/event.coax) | `nostr_event_verify_vartime` | Reads an event, recomputes its id and verifies its signature (chapter 21) |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag_d` | An addressable event's `d` value (chapter 22) |
| [nip29.coax](../../coax/lib/nostr/nip29.coax) | `g29_read(e)` | Reads an event's NIP-29 tags once: `h`, `p` with roles, `e`, `code`, `previous`, `name` and the flags |
| [nip29.coax](../../coax/lib/nostr/nip29.coax) | `g29_admit(g, e, relay)` | Decides whether one verified event is admitted to the group, and changes the group's state when it is |
| [nip29.coax](../../coax/lib/nostr/nip29.coax) | `g29_can(g, key, relay)` | The permissions a signer holds: everything for the relay's key, else the union of its roles' permissions |
| [nip29.coax](../../coax/lib/nostr/nip29.coax) | `g29_moderate(g, kind, r)` | Applies one allowed moderation event to the state |
| [nip29.coax](../../coax/lib/nostr/nip29.coax) | `g29_write_metadata`, `g29_write_admins`, `g29_write_members`, `g29_write_roles` | The tags of the 39000 to 39003 events the relay signs |
| [nip72.coax](../../coax/lib/nostr/nip72.coax) | `n72_counts(community, post, approval)` | Whether an approval makes a post count in a community |
| [nip72.coax](../../coax/lib/nostr/nip72.coax) | `n72_address(c, out)` | A community's address, `34550:<author>:<d>` |
| [nipc7.coax](../../coax/lib/nostr/nipc7.coax) | `nc7_chat(e)` | A kind 9 chat message: its `q` tags, and the parent it replies to |
| [nipc7.coax](../../coax/lib/nostr/nipc7.coax) | `nc7_cites(e, t)`, `nc7_quotes(e)` | What one `q` tag cites (an event id or an address), and whether every `q` tag cites something |
| [nipc7.coax](../../coax/lib/nostr/nipc7.coax) | `nc7_reply_tags(id, relay, author, out)` | The `q` tag of a reply |
| [nip7d.coax](../../coax/lib/nostr/nip7d.coax) | `n7d_thread(e)`, `n7d_thread_tags(title, out)` | A kind 11 thread and its title |
| [nip7d.coax](../../coax/lib/nostr/nip7d.coax) | `n7d_reply(e)` | A kind 1111 comment as a reply to a thread: its root is a kind 11 event and its parent is that same event |
| [nip7d.coax](../../coax/lib/nostr/nip7d.coax) | `n7d_reply_tags(thread, relay, author, out)` | The six NIP-22 tags of a reply to a thread |

`nip29.coax` and `nip72.coax` start with `#authority;`: `nip29.coax`
decides admission to a group, and `nip72.coax` decides which posts a
community shows. `nipc7.coax` and `nip7d.coax` carry it too. They decide
which events a chat view and a thread view show, and they read events
through chapter 63's tag readers in
[nip34.coax](../../coax/lib/nostr/nip34.coax) (`nt_tag`, `nt_hex`,
`nt_coord`) and chapter 52's comment reader in
[nip22.coax](../../coax/lib/nostr/nip22.coax).

## Groups on a relay

NIP-29 has no event that creates a group in the sense of a global
registry. A group is an id, such as `pizza`, that one relay agrees to
enforce rules for. Another relay may host a group with the same id and
different rules; NIP-29 calls that a fork. So every decision in this
chapter is one relay's decision about its own copy of a group.

A user sends a group event like any other event, with an `h` tag naming
the group. The relay looks the group up by that id and decides. The caller
of `g29_admit` does both of those steps: it verifies the event (chapter
21) and passes the state of the group that the `h` tag names. Sockets,
storage and the `OK` message are the relay's (chapter 34).

### The state

<!-- source: coax/lib/nostr/nip29.coax state -->
```jai
G29Name :: struct {b:[G29_MAX_NAME]u8; n:int;}
G29Member :: struct {key:[32]u8; roles:[G29_MAX_ROLES]G29Name; roles_n:int;}
G29Group :: struct {
  exists:bool; id:G29Name; name:G29Name;
  is_private:bool; restricted:bool; hidden:bool; closed:bool;
  members:[G29_MAX_MEMBERS]G29Member; n:int;
  invites:[G29_MAX_INVITES]G29Name; invites_n:int;
  seen:[G29_SEEN][4]u8; live:[G29_SEEN]bool; seen_n:int; seen_next:int;
}
```

- `members` holds up to 8 keys, each with up to 2 role names.
- `restricted` means only members may post. `closed` means a join
  request needs an invite code. `private` and `hidden` are kept and
  published; they limit who may read, which is the relay's query path,
  not admission.
- `invites` holds up to 4 codes from `create-invite` events.
- `seen` holds the first 4 bytes of the ids of the last 50 events the
  group admitted, with `live` cleared when a `delete-event` removes one.

The limits are small on purpose: they keep every check fast and every
state easy to print. A relay that hosts large groups keeps the same
decisions over a database.

### Reading the tags

`g29_read` reads every tag of an event once, into one record. It refuses a
second `h` tag (an event belongs to one group), a second `p` tag (a
moderation event names one user), a key or id that is not 64 lowercase hex
digits, a `previous` value that is not 8, and any name, code or role over
64 bytes. Tags it does not know, such as `picture`, are ignored.

<!-- source: coax/lib/nostr/nip29.coax read -->
```jai
// One event's NIP-29 tags, read once. A second h or p tag refuses: an event
// is about one group and one user.
G29Read :: struct {
  status:int=G29_OK;
  has_h:bool; h:G29Name;
  has_p:bool; p:[32]u8; roles:[G29_MAX_ROLES]G29Name; roles_n:int;
  has_e:bool; e:[32]u8;
  has_code:bool; code:G29Name;
  previous:[G29_MAX_PREVIOUS][4]u8; previous_n:int;
  name:G29Name; is_private:bool; restricted:bool; hidden:bool; closed:bool;
}
```

### Roles and permissions

NIP-29 leaves roles to the relay: a role is a label, and the relay decides
what each label may do. This relay grants two roles. `admin` may do every
moderation action. `moderator` may only delete events, as in NIP-29's own
example of a relay policy. Any other role name, such as `gardener`, is
kept on the member and published, but grants nothing. The relay's own key
may do everything, member or not.

<!-- source: coax/lib/nostr/nip29.coax policy -->
```jai
// What each action needs, as bits, and the roles this relay grants them to.
G29_CAN_PUT :: 1; G29_CAN_REMOVE :: 2; G29_CAN_EDIT :: 4; G29_CAN_DELETE :: 8;
G29_CAN_DROP :: 16; G29_CAN_INVITE :: 32; G29_CAN_ALL :: 63;
G29_ROLE_NAMES :: [2] string.["admin", "moderator"];
G29_ROLE_CAN :: [2] int.[G29_CAN_ALL, G29_CAN_DELETE];
G29_ROLE_ABOUT :: [2] string.["every moderation action", "delete events"];
// The bit a moderation kind needs; 0 for a kind this relay does not build.
g29_needs :: (kind:int)->int #effects(all) {
  if kind==G29_PUT_USER {return G29_CAN_PUT;}
  if kind==G29_REMOVE_USER {return G29_CAN_REMOVE;}
  if kind==G29_EDIT_METADATA {return G29_CAN_EDIT;}
  if kind==G29_DELETE_EVENT {return G29_CAN_DELETE;}
  if kind==G29_DELETE_GROUP {return G29_CAN_DROP;}
  if kind==G29_CREATE_INVITE {return G29_CAN_INVITE;}
  return 0;
}
```

<!-- source: coax/lib/nostr/nip29.coax can -->
```jai
// What the signer may do: everything for the relay's own key, else the
// union of what the member's roles grant.
g29_can :: (g:G29Group,key:[32]u8,relay:[32]u8)->int #effects(all) {
  if ns_equal32(key,relay) {return G29_CAN_ALL;}
  m:=g29_member(g,key);if m<0 {return 0;}
  can:=0;
  for r:0..<g.members[m].roles_n {
    for k:0..<G29_ROLE_NAMES.count {if g29_is(g.members[m].roles[r],G29_ROLE_NAMES[k]) {can|=G29_ROLE_CAN[k];}}
  }
  return can;
}
```

### Moderation

Each moderation kind changes one part of the state:

| Kind | Name | Tags read | Change |
| --- | --- | --- | --- |
| 9000 | `put-user` | `p` with a key and up to 2 roles | Adds the user, or replaces an existing member's roles |
| 9001 | `remove-user` | `p` with a key | Removes the member; a non-member refuses |
| 9002 | `edit-metadata` | `name`, `private`, `restricted`, `hidden`, `closed` | Replaces all five: a flag not in the event is cleared |
| 9005 | `delete-event` | `e` with an id | Clears the event from `seen`, so later references to it refuse |
| 9007 | `create-group` | none | Creates the group; the signer becomes its first member, as `admin` |
| 9008 | `delete-group` | none | Clears the whole state |
| 9009 | `create-invite` | `code` | Adds the code; the same code twice is kept once |

Kind 9010 (`update-pin-list`) and the unassigned kinds up to 9020 refuse
with `G29_KIND`: this relay does not build them.

<!-- source: coax/lib/nostr/nip29.coax moderate -->
```jai
// A moderation event the signer is allowed to send: its change to the state.
g29_moderate :: (g:[]G29Group,kind:int,r:G29Read)->int #effects(all) {
  if kind==G29_PUT_USER || kind==G29_REMOVE_USER {
    if !r.has_p {return G29_SHAPE;}
    if kind==G29_PUT_USER {return g29_put(g,r.p,r);}
    return g29_remove(g,r.p);
  }
  if kind==G29_EDIT_METADATA {
    g[0].name=r.name;g[0].is_private=r.is_private;g[0].restricted=r.restricted;
    g[0].hidden=r.hidden;g[0].closed=r.closed;return G29_OK;
  }
  if kind==G29_DELETE_EVENT {
    if !r.has_e {return G29_SHAPE;}
    g29_forget(g,r.e);return G29_OK;
  }
  if kind==G29_DELETE_GROUP {fresh:G29Group;g[0]=fresh;return G29_OK;}
  // G29_CREATE_INVITE, the only other kind g29_needs grants.
  if !r.has_code {return G29_SHAPE;}
  if g29_has_invite(g[0],r.code) {return G29_OK;}
  if g[0].invites_n==G29_MAX_INVITES {return G29_LIMIT;}
  g[0].invites[g[0].invites_n]=r.code;g[0].invites_n+=1;return G29_OK;
}
```

### Admitting an event

`g29_admit` runs the checks cheapest first and changes the state only for
an event it admits:

1. The event must have been verified. A copy with a changed signature
   refuses with `G29_EVENT`.
2. Kinds 39000 to 39005 are the relay's own. They are admitted only when
   the relay's key signed them (`G29_SIGNER` otherwise). This is the rule
   that keeps anyone from publishing a group's member list in the relay's
   name.
3. An event without an `h` tag is not a group event: `G29_PASS`, and the
   relay's ordinary policy decides it.
4. `create-group` needs a group that does not exist yet.
5. Every other event needs the group that its `h` tag names, and every
   `previous` reference must name an event in `seen`.
6. Moderation needs a permission; a join needs a valid code or an open
   group; a post to a restricted group needs a member.
7. The admitted event's id goes into `seen`.

<!-- source: coax/lib/nostr/nip29.coax admit -->
```jai
// Admit one verified event to the group g[0] holds, changing the state when
// it is admitted. G29_PASS: the event names no group, so this file has no
// say in it.
g29_admit :: (g:[]G29Group,e:NostrEvent,relay:[32]u8)->int #effects(all) {
  if e.error!=NE_OK || !e.verified {return G29_EVENT;}
  if e.kind>=G29_METADATA && e.kind<=39005 {
    return ifx ns_equal32(e.public_key,relay) then G29_OK else G29_SIGNER;
  }
  r:=g29_read(e);if r.status!=G29_OK {return r.status;}
  if !r.has_h {return G29_PASS;}
  if e.kind==G29_CREATE_GROUP {
    if g[0].exists {return G29_EXISTS;}
    if r.previous_n>0 {return G29_PREVIOUS;}
    g[0].exists=true;g[0].id=r.h;              // a slot without a group is all zero
    admin:G29Read;admin.roles[0]=g29_text("admin");admin.roles_n=1;
    s:=g29_put(g,e.public_key,admin);
    g29_remember(g,e.id);return s;
  }
  if !g[0].exists || !g29_same(g[0].id,r.h) {return G29_NO_GROUP;}
  for i:0..<r.previous_n {if !g29_known(g[0],r.previous[i]) {return G29_PREVIOUS;}}
  member:=g29_member(g[0],e.public_key)>=0;
  s:=G29_OK;
  if e.kind>=G29_PUT_USER && e.kind<=9020 {
    need:=g29_needs(e.kind);if need==0 {return G29_KIND;}
    if (g29_can(g[0],e.public_key,relay)&need)==0 {return G29_NOT_ALLOWED;}
    s=g29_moderate(g,e.kind,r);
    if e.kind==G29_DELETE_GROUP {return s;}
  } else if e.kind==G29_JOIN {
    if member {return G29_DUPLICATE;}
    if r.has_code && !g29_has_invite(g[0],r.code) {return G29_INVITE;}
    if !r.has_code && g[0].closed {return G29_CLOSED;}
    none:G29Read;s=g29_put(g,e.public_key,none);
  } else if e.kind==G29_LEAVE {
    s=g29_remove(g,e.public_key);
  } else if g[0].restricted && !member {
    return G29_NOT_MEMBER;
  }
  if s==G29_OK {g29_remember(g,e.id);}
  return s;
}
```

### Join requests and invite codes

A kind 9021 event asks to join. If it carries a `code` tag, the code must
be one the group holds, or the request refuses with `G29_INVITE`, even in
an open group: a wrong code is a mistake the user should see. Without a
code, a closed group refuses with `G29_CLOSED` and an open group adds the
user at once. A member's request refuses with `G29_DUPLICATE`, which the
relay sends with NIP-29's `duplicate:` prefix. Codes can be used more than
once; NIP-29 does not say, and this relay keeps them until the group is
deleted.

A kind 9022 event leaves: the relay removes the signer. NIP-29 says the
relay then issues a 9001 itself; signing and storing that event is the
relay's.

### Timeline references

A `previous` tag holds the first 8 hex digits of events the writer saw in
the group on this relay, for example
`["previous", "eb96c864", "2db75638", "b5d1065f"]`. The relay refuses an
event that references an event it does not have. A message copied to a
forked group on another relay carries references that relay never saw, so
it refuses there and cannot be shown out of context. NIP-29 recommends
that relays require at least 3 references; this relay requires none, and
checks every one that is present.

### The state the relay publishes

The relay publishes the state as four addressable events with the group id
in the `d` tag, signed by its own key: 39000 metadata, 39001 admins (the
members whose roles grant something, with all their roles), 39002 members
and 39003 the roles it supports. The `g29_write_*` functions write their
tags; the example signs them.

<!-- source: coax/lib/nostr/nip29.coax publish -->
```jai
// 39000: the id, name and flags.
g29_write_metadata :: (g:G29Group,out:[]u8)->int #effects(all) {
  n:=g29_open(out,g);
  if g.name.n>0 {n=json_raw(out,n,",[\"name\",");n=g29_put_name(out,n,g.name);n=json_raw(out,n,"]");}
  if g.is_private {n=json_raw(out,n,",[\"private\"]");}
  if g.restricted {n=json_raw(out,n,",[\"restricted\"]");}
  if g.hidden {n=json_raw(out,n,",[\"hidden\"]");}
  if g.closed {n=json_raw(out,n,",[\"closed\"]");}
  return json_raw(out,n,"]");
}
// 39001: members whose roles grant something, with all their roles.
g29_write_admins :: (g:G29Group,relay:[32]u8,out:[]u8)->int #effects(all) {
  n:=g29_open(out,g);
  for i:0..<g.n {
    if g29_can(g,g.members[i].key,relay)==0 {continue;}
    n=json_raw(out,n,",[\"p\",");n=g29_put_key(out,n,g.members[i].key);
    for r:0..<g.members[i].roles_n {n=json_raw(out,n,",");n=g29_put_name(out,n,g.members[i].roles[r]);}
    n=json_raw(out,n,"]");
  }
  return json_raw(out,n,"]");
}
// 39002: every member.
g29_write_members :: (g:G29Group,out:[]u8)->int #effects(all) {
  n:=g29_open(out,g);
  for i:0..<g.n {n=json_raw(out,n,",[\"p\",");n=g29_put_key(out,n,g.members[i].key);n=json_raw(out,n,"]");}
  return json_raw(out,n,"]");
}
// 39003: the roles this relay grants something to.
g29_write_roles :: (g:G29Group,out:[]u8)->int #effects(all) {
  n:=g29_open(out,g);
  for k:0..<G29_ROLE_NAMES.count {
    n=json_raw(out,n,",[\"role\",");n=json_write_string(out,n,G29_ROLE_NAMES[k]);
    n=json_raw(out,n,",");n=json_write_string(out,n,G29_ROLE_ABOUT[k]);n=json_raw(out,n,"]");
  }
  return json_raw(out,n,"]");
}
```

## Worked example: one group, three posts

The relay's key is the key `2` (public key `c6047f94...`). Alice, Bob,
Carol and Mallory sign with BIP-340's published test keys. Work the group
out by hand before the program runs:

| Event | Signer | Kind | Tags | Members after |
| --- | --- | --- | --- | --- |
| E1 | Alice | 9007 | `h pizza` | Alice (admin) |
| E2 | Alice | 9002 | `name "Pizza Lovers"`, `restricted`, `closed` | Alice (admin) |
| E3 | Alice | 9000 | `p` Bob | Alice (admin), Bob |
| E4 | Alice | 9000 | `p` Carol `moderator` | Alice (admin), Bob, Carol (moderator) |
| E5 | Alice | 9001 | `p` Carol | Alice (admin), Bob |

Each step is allowed: E1 creates a group that did not exist and makes Alice
its admin. E2 to E5 are signed by Alice, whose `admin` role grants put,
remove and edit. After E5, `seen` holds the id prefixes of E1 to E5.

Now three posts, each kind 9 with `h pizza`:

1. P1, by Bob, `previous` E1 and E3 (`1faedc7d`, `62a812cc`). The group is
   restricted; Bob is a member. Both references are in `seen`. Admitted,
   and P1's prefix `f9e7b4a0` joins `seen`.
2. P2, by Carol, `previous` P1. P1 is known, but E5 removed Carol and the
   group is restricted: refused, not a member.
3. P3, by Alice, `previous` P1 and `deadbeef`. Alice is a member and P1 is
   known, but no admitted event starts with `deadbeef`: refused, unknown
   previous event.

Then the refusals the chapter is about:

- M1: Bob sends `remove-user` Alice. Bob holds no role, so `g29_can` gives
  0 and the kind needs `G29_CAN_REMOVE`: refused, not allowed.
- I1: Alice creates the invite `slice-42`. J1: Mallory asks to join with
  `slice-41`: refused, bad invite code. J2: Mallory asks with no code; the
  group is closed: refused. J3: with `slice-42`: admitted, and Mallory is a
  member with no roles. J4: the same again: refused as a duplicate.
- The relay signs 39000 with the tags
  `[["d","pizza"],["name","Pizza Lovers"],["restricted"],["closed"]]`:
  admitted. Mallory signs the same tags: refused, not signed by the
  relay. Alice is the only admin in 39001, and 39002 lists Alice, Bob and
  Mallory.

## Moderated communities (NIP-72)

NIP-72 is marked `unrecommended` in favour of NIP-29, but clients still
meet it. A kind 34550 event defines a community and lists its moderators
as `["p", key, relay, "moderator"]`. Anyone may post with the community's
`a` tag; a post counts in the community only when a moderator approves it
with a kind 4550 event. `n72_counts` decides that one question. All three
events must be verified, the approval's signer must be listed as a
moderator, and the approval must carry the community's `a` tag, the post's
id in an `e` tag and the post's author in a `p` tag. The post itself must
tag the community with `a` or `A`.

<!-- source: coax/lib/nostr/nip72.coax counts -->
```jai
// Does the approval make the post count in the community?
n72_counts :: (community:NostrEvent,post:NostrEvent,approval:NostrEvent)->int #effects(all) {
  all:=[3]NostrEvent.[community,post,approval];
  for e:all {if e.error!=NE_OK || !e.verified {return N72_EVENT;}}
  if community.kind!=N72_COMMUNITY_KIND || approval.kind!=N72_APPROVAL_KIND {return N72_KIND;}
  buf:[400]u8;n:=n72_address(community,buf[..]);if n== -1 {return N72_LIMIT;}
  address:=cast(string)buf[0..<n];
  hex:[64]u8;
  signer:=approval.public_key;
  s:=n72_has(community,"p","p",n72_hex(signer[..],hex[..]),"moderator");
  if s!=N72_OK {return ifx s==N72_LIMIT then s else N72_NOT_MODERATOR;}
  s=n72_has(approval,"a","a",address,"");if s!=N72_OK {return s;}
  id:=post.id;s=n72_has(approval,"e","e",n72_hex(id[..],hex[..]),"");
  if s!=N72_OK {return ifx s==N72_LIMIT then s else N72_POST;}
  author:=post.public_key;s=n72_has(approval,"p","p",n72_hex(author[..],hex[..]),"");
  if s!=N72_OK {return ifx s==N72_LIMIT then s else N72_AUTHOR;}
  s=n72_has(post,"a","A",address,"");
  if s!=N72_OK {return ifx s==N72_LIMIT then s else N72_UNTAGGED;}
  return N72_OK;
}
```

Worked by hand: Alice's community `pizza-fans` lists Bob as its only
moderator. Carol posts kind 1111 with the community's `A` and `a` tags.
Bob's approval names the community, Carol's post and Carol: counts.
Mallory's approval names the same things, but Mallory is not listed: not a
moderator. A second approval by Bob names P1's id instead of Carol's post:
another post.

## Chats and threads (NIP-C7 and NIP-7D)

The posts in the worked group are kind 9 events. NIP-C7 defines that kind
on its own: a chat message is a kind 9 event, and its content is the
message. NIP-7D defines a second shape of conversation, the forum thread.
A client reads both, and no relay has to decide anything, so each has one
small reader. A reader takes a parsed event and returns a record whose
`status` is `N34_OK` or the reason the event refuses. Neither reader
verifies a signature or fetches an event: the caller passes events it has
verified (chapter 21).

### Chats

A reply to a chat message is another kind 9 that quotes its parent in a
`q` tag:

```text
["q", "<event id>", "<relay URL>", "<pubkey>"]
```

The `q` tag is NIP-18's (chapter 52), so a chat message may also quote
other content by its address, `<kind>:<pubkey>:<d>`. `nc7_cites` says
which of the two a tag holds, and `nc7_quotes` refuses the event when any
`q` tag holds neither:

<!-- source: coax/lib/nostr/nipc7.coax quote -->
```jai
// What one q tag cites: an event id, an event address, or nothing this
// file reads. An author, when the tag carries one, must be a pubkey.
nc7_cites :: (e:NtEvent,t:NtTag)->int #effects(all) {
 raw:=nt_raw(e,t.v[0]);author:=nt_raw(e,t.v[2]);
 // A tag without an author has an empty third span.
 if author.count>0 && !nt_hex(author,64) {return NC7_NOTHING;}
 if nt_hex(raw,64) {return NC7_ID;}
 return ifx nt_coord(raw).kind>=0 then NC7_ADDRESS else NC7_NOTHING;
}
// How many q tags the event has, or -1 when one of them cites nothing.
nc7_quotes :: (e:NtEvent)->int #effects(all) {
 n:=nt_tags(e,"q");
 for k:0..<n {if nc7_cites(e,nt_tag(e,"q",k))==NC7_NOTHING {return -1;}}
 return n;
}
```

`nc7_chat` reads the message. The parent is the first `q` tag that holds
an event id. A message with no such tag starts a conversation.

<!-- source: coax/lib/nostr/nipc7.coax chat -->
```jai
// A kind 9 chat message. Every q tag must cite an event or an address; the
// first one that holds an event id is the parent.
nc7_chat :: (e:NtEvent)->NC7Chat #effects(all) {
 c:NC7Chat;c.status=n34_start(e,NC7_KIND_CHAT);if c.status!=N34_OK {return c;}
 c.quotes=nc7_quotes(e);
 if c.quotes<0 {c.status=N34_MALFORMED;return c;}
 for k:0..<c.quotes {
  t:=nt_tag(e,"q",k);
  if nc7_cites(e,t)==NC7_ID {c.parent=t;break;}
 }
 return c;
}
```

NIP-C7 ends with a rule for clients: "Clients that render a 'chat view' as
a stream of ordered events MUST only fetch `kind 9` events". Two things
follow for this reader:

- `nc7_chat` refuses every other kind with `N34_KIND`, so a chat view built
  on it never shows a note, a comment or a thread as a chat message.
- The reader cannot know that the quoted event is a kind 9. The client
  fetches the parent and reads it with `nc7_chat` too.

`nc7_reply_tags` writes the one tag of a reply. It refuses an id or a key
that is not 64 lowercase hex digits.

<!-- source: coax/lib/nostr/nipc7.coax reply -->
```jai
// The tags of a reply to the kind 9 event id, by author, found on relay:
// one q tag. The new length, or -1 when an id isn't 64 lowercase hex digits
// or out is short.
nc7_reply_tags :: (id:string,relay:string,author:string,out:[]u8)->int #effects(all) {
 if !nt_hex(id,64) || !nt_hex(author,64) {return -1;}
 n:=json_raw(out,0,"[[\"q\",\"");n=json_raw(out,n,id);n=json_raw(out,n,"\",");n=json_write_string(out,n,relay);
 n=json_raw(out,n,",\"");n=json_raw(out,n,author);return json_raw(out,n,"\"]]");
}
```

### Threads

A thread is a kind 11 event. Its content opens the discussion, and it
"SHOULD include a `title`". A reply is neither a kind 11 nor a kind 1:
"Replies to `kind 11` MUST use NIP-22 `kind 1111` comments."

A NIP-22 comment (chapter 52) names two things. Its uppercase tags `E`, `K`
and `P` name the root, the item the whole discussion is about. Its
lowercase tags `e`, `k` and `p` name the parent, the item this comment
answers. NIP-7D adds one sentence: "Replies should always be to the root
`kind 11` to avoid arbitrarily nested reply hierarchies." So in a reply to
a thread the root and the parent are the same event.

`n7d_reply` reads the comment with chapter 52's `n22_comment` and then
asks for exactly that:

<!-- source: coax/lib/nostr/nip7d.coax reply -->
```jai
// A kind 1111 comment as a reply to a thread: its root is an event of kind
// 11, and its parent is that same event.
n7d_reply :: (e:NtEvent)->N7DReply #effects(all) {
 r:N7DReply;c:=n22_comment(e);r.status=c.status;if r.status!=N34_OK {return r;}
 if c.root.at!='E' || c.root.kind_value!=N7D_KIND_THREAD {r.status=N7D_ELSEWHERE;return r;}
 r.thread=c.root.ref;r.author=c.root.author;
 if c.parent.at!='e' || c.parent.kind_value!=N7D_KIND_THREAD || nt_raw(e,c.parent.ref.v[0])!=nt_raw(e,c.root.ref.v[0]) {r.status=N7D_NESTED;}
 return r;
}
// The tags of a reply to the thread id, by author, found on relay: NIP-22's
// E, K and P for the root and e, k and p for the parent, which is the same
// event. The new length, or -1 when an id isn't 64 lowercase hex digits or
// out is short.
n7d_reply_tags :: (thread:string,relay:string,author:string,out:[]u8)->int #effects(all) {
 if !nt_hex(thread,64) || !nt_hex(author,64) {return -1;}
 n:=json_raw(out,0,"[");names:="EKPekp";
 for half:0..<2 {
  n=json_raw(out,n,"[\"");n=json_raw(out,n,names[3*half..<3*half+1]);n=json_raw(out,n,"\",\"");n=json_raw(out,n,thread);n=json_raw(out,n,"\",");
  n=json_write_string(out,n,relay);n=json_raw(out,n,",\"");n=json_raw(out,n,author);n=json_raw(out,n,"\"],[\"");
  n=json_raw(out,n,names[3*half+1..<3*half+2]);n=json_raw(out,n,"\",\"11\"],[\"");n=json_raw(out,n,names[3*half+2..<3*half+3]);n=json_raw(out,n,"\",\"");
  n=json_raw(out,n,author);n=json_raw(out,n,ifx half==0 then "\"]," else "\"]]");
 }
 return n;
}
```

It has two refusals of its own:

- `N7D_ELSEWHERE`: the comment's root is not an event of kind 11. The
  comment belongs to an article, an issue or a web page, not to a thread.
- `N7D_NESTED`: the root is a thread, but the parent is something else,
  usually another comment (`k` is `1111`). NIP-22 allows that comment;
  NIP-7D asks clients not to write it. The record still names the thread,
  so a client can show the comment flat under the thread or leave it out.

NIP-7D's own example reply carries only `K` and `E`. NIP-22 requires the
parent's `e` and `k` too, so that example refuses here as `N34_MISSING`.
`n7d_reply_tags` writes all six tags.

### Worked example: two chats and one thread

Alice, Bob and Carol sign with BIP-340's published test keys. Read five
events by hand before the program runs:

| Event | Signer | Kind | Tags | Reads as |
| --- | --- | --- | --- | --- |
| C1 | Alice | 9 | none | A chat message that starts a conversation |
| C2 | Bob | 9 | `q` C1, the relay, Alice | A reply to C1 |
| T1 | Carol | 11 | `title` "Which kettle?" | A thread |
| R1 | Alice | 1111 | `E` T1, `K` 11, `P` Carol; `e` T1, `k` 11, `p` Carol | A reply to T1 |
| R2 | Bob | 1111 | `E` T1, `K` 11, `P` Carol; `e` R1, `k` 1111, `p` Alice | Nested: it answers R1 |

1. C1 has no `q` tag, so `quotes` is 0 and there is no parent.
2. C2 has one `q` tag whose first value is 64 hex digits: an event id. It
   is the parent, and it is C1's id.
3. T1 is kind 11 with one `title` tag that has a value.
4. R1 is a NIP-22 comment. Its root is `E` with `K` 11, so it is about a
   thread. Its parent is `e` with `k` 11 and the same id as the root: a
   reply to the thread.
5. R2's root is the same thread, but its parent's `k` is 1111 and its id
   is R1's. NIP-22's rule holds (a parent of kind 1111 is a comment under
   the root), so `n22_comment` reads it. `n7d_reply` returns `N7D_NESTED`.

A chat view over these five events shows C1 and C2. A thread view shows T1
with R1 under it, and decides for itself what to do with R2.

## The example

The example is
[nostr/examples/groups-communities.coax](../examples/groups-communities.coax).
It signs every event with BIP-340 test keys, verifies each one, runs the
group through `g29_admit` and prints each verdict and the members after
every moderation event. Then it sends a copy of P1 with one changed
signature digit, publishes the relay's state, and checks three NIP-72
approvals. Every approval carries Carol's post as JSON in its content, as
NIP-72 asks.

<!-- source: nostr/examples/groups-communities.coax example -->
```jai
#load "../../coax/lib/nostr/nip72.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

STATUS :: [15] string.["admitted", "refused: event does not verify", "refused: bad tags", "refused: over a limit",
  "not a group event", "refused: no such group", "refused: group exists", "refused: unknown previous event",
  "refused: not a member", "refused: not allowed", "refused: bad invite code", "refused: group is closed",
  "refused: duplicate", "refused: kind not built", "refused: not signed by the relay"];
COUNTS :: [9] string.["counts", "event does not verify", "wrong kind", "approver is not a moderator",
  "approval names another community", "approval names another post", "approval names another author",
  "post does not tag the community", "bad tags"];
// BIP-340 test vectors 0 to 3's keys, and the key 2 for the relay. Never
// sign with a real key in an example.
PEOPLE :: [5] string.["Alice", "Bob", "Carol", "Mallory", "Relay"];
PUB :: [5] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
  "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8",
  "25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517",
  "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"];
SECRET :: [5] string.[
  "0000000000000000000000000000000000000000000000000000000000000003",
  "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef",
  "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9",
  "0b432b2677937381aef05bb02a66ecd012773062cf3fa2549e44f58ed2401710",
  "0000000000000000000000000000000000000000000000000000000000000002"];
ALICE :: 0; BOB :: 1; CAROL :: 2; MALLORY :: 3; RELAY :: 4;
T0 :: 1793491200;                         // 2026-11-01T00:00:00Z

decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
hex32 :: (id: [32] u8, out: [] u8) -> string { own := id; return n72_hex(own[..], out); }
key_of :: (who: int) -> [32] u8 { k: [32] u8; decode_hex(PUB[who], k[..]); return k; }
who_is :: (key: [32] u8) -> string {
  for k: 0..<PEOPLE.count { if ns_equal32(key_of(k), key) { return PEOPLE[k]; } }
  return "someone";
}

// Sign an event into out and read it back with its id and signature checked.
sign :: (who: int, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  key: [32] u8; decode_hex(SECRET[who], key[..]);
  aux: [32] u8; work: [1] NostrSignWork; scratch: [8192] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [8192] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified && ns_equal32(e.public_key, key_of(who));
  return e;
}
// The tags with ^k replaced by person k's key, &k by the first 8 hex digits
// of event k's id (a previous reference) and *k by its whole id.
with_refs :: (template: string, ids: [] [32] u8, out: [] u8) -> string {
  n := 0; i := 0; hex: [64] u8;
  while i < template.count {
    c := template[i];
    if c == '^' || c == '&' || c == '*' {
      k := cast(int) (template[i + 1] - '0'); i += 2;
      if i < template.count && template[i] >= '0' && template[i] <= '9' { k = k * 10 + cast(int) (template[i] - '0'); i += 1; }
      s := ifx c == '^' then PUB[k] else hex32(ids[k], hex[..]);
      if c == '&' { s = s[0..<8]; }
      n = json_raw(out, n, s);
    } else { out[n] = c; n += 1; i += 1; }
  }
  return cast(string) out[0..<n];
}

// book:begin show
show_members :: (g: G29Group) {
  print("    members:");
  if g.n == 0 { print(" none"); }
  for i: 0..<g.n {
    print(" %", who_is(g.members[i].key));
    for r: 0..<g.members[i].roles_n { own := g.members[i].roles[r]; print("%", ifx r == 0 then " (" else ", "); print("%", cast(string) own.b[0..<own.n]); }
    if g.members[i].roles_n > 0 { print(")"); }
  }
  print("\n");
}
// book:end show

// The group's events: a name, who signs, the kind, tags and content.
EVENTS :: 14;
NAME :: [EVENTS] string.["E1", "E2", "E3", "E4", "E5", "P1", "P2", "P3", "M1", "I1", "J1", "J2", "J3", "J4"];
WHAT :: [EVENTS] string.["create-group", "edit-metadata", "put-user Bob", "put-user Carol as moderator",
  "remove-user Carol", "post", "post", "post", "remove-user Alice", "create-invite",
  "join with code slice-41", "join without a code", "join with code slice-42", "join again"];
WHO :: [EVENTS] int.[ALICE, ALICE, ALICE, ALICE, ALICE, BOB, CAROL, ALICE, BOB, ALICE, MALLORY, MALLORY, MALLORY, MALLORY];
KIND :: [EVENTS] int.[9007, 9002, 9000, 9000, 9001, 9, 9, 9, 9001, 9009, 9021, 9021, 9021, 9021];
TAGS :: [EVENTS] string.[
  "[[\"h\",\"pizza\"]]",
  "[[\"h\",\"pizza\"],[\"name\",\"Pizza Lovers\"],[\"restricted\"],[\"closed\"]]",
  "[[\"h\",\"pizza\"],[\"p\",\"^1\"]]",
  "[[\"h\",\"pizza\"],[\"p\",\"^2\",\"moderator\"]]",
  "[[\"h\",\"pizza\"],[\"p\",\"^2\"]]",
  "[[\"h\",\"pizza\"],[\"previous\",\"&0\",\"&2\"]]",
  "[[\"h\",\"pizza\"],[\"previous\",\"&5\"]]",
  "[[\"h\",\"pizza\"],[\"previous\",\"&5\",\"deadbeef\"]]",
  "[[\"h\",\"pizza\"],[\"p\",\"^0\"]]",
  "[[\"h\",\"pizza\"],[\"code\",\"slice-42\"]]",
  "[[\"h\",\"pizza\"],[\"code\",\"slice-41\"]]",
  "[[\"h\",\"pizza\"]]",
  "[[\"h\",\"pizza\"],[\"code\",\"slice-42\"]]",
  "[[\"h\",\"pizza\"],[\"code\",\"slice-42\"]]"];
CONTENT :: [EVENTS] string.["", "", "", "", "", "who wants pizza?", "me!", "Friday?", "", "", "", "", "", ""];

// book:begin publish
// The relay signs its state as 39000 to 39003 and admits its own events.
publish :: (g: [] G29Group, relay: [32] u8) {
  kinds := [4] int.[39000, 39001, 39002, 39003];
  for kind: kinds {
    tags: [2048] u8; n := 0;
    if kind == 39000 { n = g29_write_metadata(g[0], tags[..]); }
    if kind == 39001 { n = g29_write_admins(g[0], relay, tags[..]); }
    if kind == 39002 { n = g29_write_members(g[0], tags[..]); }
    if kind == 39003 { n = g29_write_roles(g[0], tags[..]); }
    #assert n > 0;
    text := cast(string) tags[0..<n];
    out: [4096] u8; e := sign(RELAY, T0 + 900, kind, text, "", out[..]);
    print("  % by Relay: %\n    tags %\n", kind, STATUS[g29_admit(g, e, relay)], text);
    if kind == 39000 {
      forged: [4096] u8; f := sign(MALLORY, T0 + 900, kind, text, "", forged[..]);
      print("  % by Mallory, the same tags: %\n", kind, STATUS[g29_admit(g, f, relay)]);
    }
  }
}
// book:end publish

main :: () {
  relay := key_of(RELAY);
  ids: [EVENTS + 4] [32] u8; buf: [EVENTS + 5] [2048] u8; all: [EVENTS] NostrEvent;
  for k: 0..<EVENTS {
    tags: [2048] u8;
    all[k] = sign(WHO[k], T0 + 60 * k, KIND[k], with_refs(TAGS[k], ids[..], tags[..]), CONTENT[k], buf[k][..]);
    ids[k] = all[k].id;
  }
  print("each event's id\n");
  for k: 0..<EVENTS { hex: [64] u8; print("  %: %\n", NAME[k], hex32(ids[k], hex[..])); }

  print("group pizza on the relay %...\n", PUB[RELAY][0..<8]);
  g: [1] G29Group;
  for k: 0..<EVENTS {
    tags: [2048] u8;
    print("  % % kind % %", NAME[k], PEOPLE[WHO[k]], KIND[k], WHAT[k]);
    if CONTENT[k].count > 0 { print(" \"%\"", CONTENT[k]); }
    p := g29_read(all[k]);
    if p.previous_n > 0 {
      print(", previous");
      for i: 0..<p.previous_n { own := p.previous[i]; hex: [8] u8; print(" %", n72_hex(own[..], hex[..])); }
    }
    print(": %\n", STATUS[g29_admit(g[..], all[k], relay)]);
    if KIND[k] >= 9000 && KIND[k] <= 9022 { show_members(g[0]); }
  }
  // A copy of P1 with one byte of its signature changed.
  bad: [2048] u8; text := cast(string) buf[5][0..<all[5].written];
  for i: 0..<text.count { bad[i] = text[i]; }
  at := text.count - 3; bad[at] = ifx bad[at] == '0' then '1' else '0';
  check: [8192] u8; forged := nostr_event_verify_vartime(cast(string) bad[0..<text.count], check[..], false);
  print("  P1 with one signature digit changed: %\n", STATUS[g29_admit(g[..], forged, relay)]);
  print("relay state\n");
  publish(g[..], relay);

  print("community pizza-fans (NIP-72)\n");
  tags: [2048] u8;
  c := sign(ALICE, T0 + 1000, 34550, with_refs("[[\"d\",\"pizza-fans\"],[\"name\",\"Pizza fans\"],[\"p\",\"^1\",\"\",\"moderator\"]]", ids[..], tags[..]), "", buf[EVENTS][..]);
  q := sign(CAROL, T0 + 1060, 1111, with_refs("[[\"A\",\"34550:^0:pizza-fans\"],[\"a\",\"34550:^0:pizza-fans\"],[\"P\",\"^0\"],[\"p\",\"^0\"],[\"K\",\"34550\"],[\"k\",\"34550\"]]", ids[..], tags[..]), "first slice", buf[EVENTS + 1][..]);
  ids[EVENTS] = q.id; ids[EVENTS + 1] = all[5].id;
  post := cast(string) buf[EVENTS + 1][0..<q.written];
  approvals := [3] int.[BOB, MALLORY, BOB];
  targets := [3] string.["*14", "*14", "*15"];
  labels := [3] string.["Bob approves Carol's post", "Mallory approves Carol's post", "Bob's approval names P1 instead"];
  for i: 0..<3 {
    template: [512] u8; n := json_raw(template[..], 0, "[[\"a\",\"34550:^0:pizza-fans\"],[\"e\",\"");
    n = json_raw(template[..], n, targets[i]); n = json_raw(template[..], n, "\"],[\"p\",\"^2\"],[\"k\",\"1111\"]]");
    a := sign(approvals[i], T0 + 1120 + i, 4550, with_refs(cast(string) template[0..<n], ids[..], tags[..]), post, buf[EVENTS + 2 + i][..]);
    print("  %: %\n", labels[i], COUNTS[n72_counts(c, q, a)]);
  }
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/groups-communities.coax --run-steps 250000000
```

The output matches the hand values: P1 admitted, P2 refused as not a
member, P3 refused for `deadbeef`, M1 not allowed, J1 to J4 as worked out,
the forged 39000 refused, and Bob's approval the only one that counts:

```text
each event's id
  E1: 1faedc7d07730a6d560ea3af069d93f2ded8beb565e8fcc9653d301c214fcdb6
  E2: f98f23a05a149673d140661f847c3ff8fc5c2e1050af59b64e271b3ffd8626d6
  E3: 62a812ccc6e11b9e82139ce1dc69a439b806ab3df0f062024e9ae6ebe92eda39
  E4: 4bff9f37ff627845b7cc7d0087cafbbdd8180ccefe63b8e4a4090ad84a2d8102
  E5: a501a8c65a30d55e443053486237fadd99a32b1751123464a0b325c46426e556
  P1: f9e7b4a040395cdcd2b4aeb20c5ec195b3642913dc54aafa0c36cc94907cfd5e
  P2: 805b8c31397dcaa0bc3a5e60584301e91ce9ca80a29472119a427d6e0b02cf1c
  P3: a081498c3b97ffa6ee5a7845bd69f75781a17aa37d17eed1ada2bd8ecc5a0c53
  M1: 63882d8a2035fee60ff605a59ed63bb44ac8eb31ae249761edaec53b5dcbcc8c
  I1: 9c13a9ebcbf5f3beb95055f36b43d77237ddac086469a9e73dd066f57d78c53f
  J1: 6d531217a74343419f0b23326d27a477a3754366f3ccaa219e6e004537374c65
  J2: e75fb06b540835d52b6d6c3c643086e8fc3a879b2c26d5b3b2ee0d5a59eb8cf1
  J3: c4ecb4fa625fbad31cb35a8bd44b64a47c6993be057e20382f0852d0299e19b3
  J4: 0377621b32a6ffa8a0f6738bfd8f5c67eb44f8aab0bd4124693ff19961d5fb1e
group pizza on the relay c6047f94...
  E1 Alice kind 9007 create-group: admitted
    members: Alice (admin)
  E2 Alice kind 9002 edit-metadata: admitted
    members: Alice (admin)
  E3 Alice kind 9000 put-user Bob: admitted
    members: Alice (admin) Bob
  E4 Alice kind 9000 put-user Carol as moderator: admitted
    members: Alice (admin) Bob Carol (moderator)
  E5 Alice kind 9001 remove-user Carol: admitted
    members: Alice (admin) Bob
  P1 Bob kind 9 post "who wants pizza?", previous 1faedc7d 62a812cc: admitted
  P2 Carol kind 9 post "me!", previous f9e7b4a0: refused: not a member
  P3 Alice kind 9 post "Friday?", previous f9e7b4a0 deadbeef: refused: unknown previous event
  M1 Bob kind 9001 remove-user Alice: refused: not allowed
    members: Alice (admin) Bob
  I1 Alice kind 9009 create-invite: admitted
    members: Alice (admin) Bob
  J1 Mallory kind 9021 join with code slice-41: refused: bad invite code
    members: Alice (admin) Bob
  J2 Mallory kind 9021 join without a code: refused: group is closed
    members: Alice (admin) Bob
  J3 Mallory kind 9021 join with code slice-42: admitted
    members: Alice (admin) Bob Mallory
  J4 Mallory kind 9021 join again: refused: duplicate
    members: Alice (admin) Bob Mallory
  P1 with one signature digit changed: refused: event does not verify
relay state
  39000 by Relay: admitted
    tags [["d","pizza"],["name","Pizza Lovers"],["restricted"],["closed"]]
  39000 by Mallory, the same tags: refused: not signed by the relay
  39001 by Relay: admitted
    tags [["d","pizza"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","admin"]]
  39002 by Relay: admitted
    tags [["d","pizza"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["p","dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"],["p","25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517"]]
  39003 by Relay: admitted
    tags [["d","pizza"],["role","admin","every moderation action"],["role","moderator","delete events"]]
community pizza-fans (NIP-72)
  Bob approves Carol's post: counts
  Mallory approves Carol's post: approver is not a moderator
  Bob's approval names P1 instead: approval names another post
```

The refusals:

- P2: Carol was removed and the group is restricted (`G29_NOT_MEMBER`).
  This is the "non-member's event to a closed group" of this chapter's
  plan: the group is both closed and restricted, and NIP-29 calls the
  posting rule `restricted`.
- P3: `deadbeef` names no admitted event (`G29_PREVIOUS`).
- M1: moderation by a member with no role (`G29_NOT_ALLOWED`).
- J1: a code the group does not hold (`G29_INVITE`); J2: no code for a
  closed group (`G29_CLOSED`); J4: a member again (`G29_DUPLICATE`).
- P1 with one signature digit changed does not verify (`G29_EVENT`).
- 39000 signed by Mallory (`G29_SIGNER`).
- Mallory's approval: not a moderator (`N72_NOT_MODERATOR`); Bob's approval
  of P1: another post (`N72_POST`).

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/groups-communities.out](../fixtures/groups-communities.out).

### The chats and threads example

A second example,
[nostr/examples/chats-threads.coax](../examples/chats-threads.coax), signs
the five events of the worked example with the same test keys, verifies
each one, reads them with `nc7_chat`, `n7d_thread` and `n7d_reply`, and
then reads seven events that refuse.

<!-- source: nostr/examples/chats-threads.coax example -->
```jai
#load "../../coax/lib/nostr/nip34-event.coax";
#load "../../coax/lib/nostr/nipc7.coax";
#load "../../coax/lib/nostr/nip7d.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// The statuses of nip34.coax, then nip22.coax's 6 and nip7d.coax's 7 and 8.
STATUS :: [9] string.["ok", "event", "kind", "missing", "malformed", "output",
  "parent kind differs from root kind", "elsewhere", "nested"];
// BIP-340 test vectors 0, 1 and 2's published secret keys. Never sign with
// a real key in an example.
KEYS :: [3] string.[
  "0000000000000000000000000000000000000000000000000000000000000003",
  "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef",
  "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9"];
PEOPLE :: [3] string.["Alice", "Bob", "Carol"];
ALICE :: 0; BOB :: 1; CAROL :: 2;
RELAY :: "wss://relay.example.com";
T0 :: 1761686400;

// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
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

// An unsigned event with these tags, parsed but not verified: its id and
// sig are zeros. Used for the refusals, which the readers refuse on their
// kind and tags alone.
unsigned :: (kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\",\"created_at\":1761686400,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":\"\",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

// A tag list built from pieces of JSON text.
Tags :: struct { b: [2048] u8; n: int; }
put :: (t: [] Tags, s: string) { t[0].n = json_raw(t[0].b[..], t[0].n, s); #assert t[0].n > 0; }
tags_of :: (t: [] Tags) -> string { return cast(string) t[0].b[0..<t[0].n]; }

// The tags of a comment on a comment: the root is the thread (E, K 11, P),
// the parent is the reply it answers (e, k 1111, p). NIP-22 allows this;
// NIP-7D asks clients to reply to the thread instead.
nested_tags :: (t: [] Tags, thread: string, thread_author: string, reply: string, reply_author: string) {
  put(t, "[[\"E\",\""); put(t, thread); put(t, "\",\""); put(t, RELAY); put(t, "\",\""); put(t, thread_author);
  put(t, "\"],[\"K\",\"11\"],[\"P\",\""); put(t, thread_author); put(t, "\"],[\"e\",\""); put(t, reply); put(t, "\",\""); put(t, RELAY);
  put(t, "\",\""); put(t, reply_author); put(t, "\"],[\"k\",\"1111\"],[\"p\",\""); put(t, reply_author); put(t, "\"]]");
}

// Which person a pubkey in a tag names.
who :: (pk: [] [64] u8, raw: string) -> string {
  for k: 0..<pk.count { if raw == cast(string) pk[k][..] { return PEOPLE[k]; } }
  return "someone else";
}

show_chat :: (label: string, event: NostrEvent, first: string, pk: [] [64] u8) {
  e := nt_event_of(event); c := nc7_chat(e);
  print("%: %", label, STATUS[c.status]);
  if c.status != N34_OK { print("\n"); return; }
  if !c.parent.found { print(", % q tags, not a reply\n", c.quotes); return; }
  print(", % q tag, a reply to % by %, found on %\n", c.quotes, ifx nt_raw(e, c.parent.v[0]) == first then "C1" else "another event",
    who(pk, nt_raw(e, c.parent.v[2])), nt_raw(e, c.parent.v[1]));
}
show_reply :: (label: string, event: NostrEvent, thread: string, pk: [] [64] u8) {
  e := nt_event_of(event); r := n7d_reply(e);
  print("%: %", label, STATUS[r.status]);
  if r.status != N34_OK && r.status != N7D_NESTED { print("\n"); return; }
  print(", thread % by %\n", ifx nt_raw(e, r.thread.v[0]) == thread then "T1" else "another event", who(pk, nt_raw(e, r.author.v[0])));
}

main :: () {
  secret: [3][32] u8; pk: [3][64] u8;
  for k: 0..<3 { decode_hex(KEYS[k], secret[k][..]); }
  buf: [5][8192] u8;
  // Each public key comes from signing: sign an empty note first.
  for k: 0..<3 { pk[k] = n34_hex32(sign(secret[k][..], T0, 1, "[]", "", buf[0][..]).public_key); }
  for k: 0..<3 { print("%: %\n", PEOPLE[k], cast(string) pk[k][..]); }

  print("chats (kind 9)\n");
  c1 := sign(secret[ALICE][..], T0, 9, "[]", "GM", buf[0][..]);
  id1 := n34_hex32(c1.id);
  tags: [1024] u8;
  n := nc7_reply_tags(cast(string) id1[..], RELAY, cast(string) pk[ALICE][..], tags[..]);
  #assert n > 0;
  print("C2 tags: %\n", cast(string) tags[0..<n]);
  c2 := sign(secret[BOB][..], T0 + 30, 9, cast(string) tags[0..<n], "GM, Alice", buf[1][..]);
  show_chat("C1", c1, cast(string) id1[..], pk[..]);
  show_chat("C2", c2, cast(string) id1[..], pk[..]);

  print("a thread (kind 11) and its replies (kind 1111)\n");
  n = n7d_thread_tags("Which kettle?", tags[..]);
  #assert n > 0;
  t1 := sign(secret[CAROL][..], T0 + 120, 11, cast(string) tags[0..<n], "Gooseneck or plain?", buf[2][..]);
  te := nt_event_of(t1); thread := n7d_thread(te);
  print("T1: %, title \"%\"\n", STATUS[thread.status], nt_raw(te, thread.title.v[0]));
  idt := n34_hex32(t1.id);
  n = n7d_reply_tags(cast(string) idt[..], RELAY, cast(string) pk[CAROL][..], tags[..]);
  #assert n > 0;
  print("R1 tags: %\n", cast(string) tags[0..<n]);
  r1 := sign(secret[ALICE][..], T0 + 180, 1111, cast(string) tags[0..<n], "Gooseneck. It pours slower.", buf[3][..]);
  idr := n34_hex32(r1.id);
  t: [1] Tags;
  nested_tags(t[..], cast(string) idt[..], cast(string) pk[CAROL][..], cast(string) idr[..], cast(string) pk[ALICE][..]);
  r2 := sign(secret[BOB][..], T0 + 240, 1111, tags_of(t[..]), "Slower is the point.", buf[4][..]);
  show_reply("R1", r1, cast(string) idt[..], pk[..]);
  show_reply("R2, Bob's answer to R1", r2, cast(string) idt[..], pk[..]);
  print("R2 as a NIP-22 comment: %\n", STATUS[n22_comment(nt_event_of(r2)).status]);

  print("each event's id\n");
  all := [5] NostrEvent.[c1, c2, t1, r1, r2];
  names := [5] string.["C1", "C2", "T1", "R1", "R2"];
  for k: 0..<5 { h := n34_hex32(all[k].id); print("%: %, verified %\n", names[k], cast(string) h[..], all[k].verified); }

  print("refusals\n");
  bad: [8192] u8;
  e := unsigned(9, "[[\"q\",\"5c83da77\"]]", bad[..]);
  print("chat, q of 8 digits: %\n", STATUS[nc7_chat(nt_event_of(e)).status]);
  e = unsigned(9, "[[\"q\"]]", bad[..]);
  print("chat, q without a value: %\n", STATUS[nc7_chat(nt_event_of(e)).status]);
  e = unsigned(1, "[]", bad[..]);
  print("a kind 1 note read as a chat: %\n", STATUS[nc7_chat(nt_event_of(e)).status]);
  e = unsigned(11, "[[\"title\"]]", bad[..]);
  print("thread, title without a value: %\n", STATUS[n7d_thread(nt_event_of(e)).status]);
  e = unsigned(1111, "[[\"K\",\"11\"],[\"E\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\",\"wss://relay.example.com\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"]]", bad[..]);
  print("reply with K and E only, as NIP-7D's example: %\n", STATUS[n7d_reply(nt_event_of(e)).status]);
  e = unsigned(1111, "[[\"E\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\"],[\"K\",\"1\"],[\"e\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\"],[\"k\",\"1\"]]", bad[..]);
  print("comment on a kind 1 note read as a thread reply: %\n", STATUS[n7d_reply(nt_event_of(e)).status]);
  e = unsigned(1111, "[[\"E\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\"],[\"K\",\"11\"],[\"e\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\"],[\"k\",\"1\"]]", bad[..]);
  print("reply whose k is neither 11 nor 1111: %\n", STATUS[n7d_reply(nt_event_of(e)).status]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/chats-threads.coax --run-steps 250000000
```

The output matches the hand values: C2 is a reply to C1, R1 is a reply to
T1, and R2 is nested.

```text
Alice: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
Bob: dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
Carol: dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8
chats (kind 9)
C2 tags: [["q","f9e8ae5406b60f1417b03ce2d7366cf0833d16aabbe09cdd92c616f48fff7680","wss://relay.example.com","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"]]
C1: ok, 0 q tags, not a reply
C2: ok, 1 q tag, a reply to C1 by Alice, found on wss://relay.example.com
a thread (kind 11) and its replies (kind 1111)
T1: ok, title "Which kettle?"
R1 tags: [["E","386c56b8dae39d338b82bcc26fe5a80a6230e167c5f98004adda66118e44efc1","wss://relay.example.com","dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"],["K","11"],["P","dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"],["e","386c56b8dae39d338b82bcc26fe5a80a6230e167c5f98004adda66118e44efc1","wss://relay.example.com","dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"],["k","11"],["p","dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"]]
R1: ok, thread T1 by Carol
R2, Bob's answer to R1: nested, thread T1 by Carol
R2 as a NIP-22 comment: ok
each event's id
C1: f9e8ae5406b60f1417b03ce2d7366cf0833d16aabbe09cdd92c616f48fff7680, verified true
C2: c38a6f009d18565e779fb4163176e8af958e3a08d932a00c3c1adc5d3b705913, verified true
T1: 386c56b8dae39d338b82bcc26fe5a80a6230e167c5f98004adda66118e44efc1, verified true
R1: 4777a42b623e988dc88a81123b3708831b68d6717cbca538a2e3f56dc1f82604, verified true
R2: d0e9dda8ae20ea726b10f3fb8c018c77cb59d465b5bf00774ea56c6438a47d8a, verified true
refusals
chat, q of 8 digits: malformed
chat, q without a value: malformed
a kind 1 note read as a chat: kind
thread, title without a value: malformed
reply with K and E only, as NIP-7D's example: missing
comment on a kind 1 note read as a thread reply: elsewhere
reply whose k is neither 11 nor 1111: parent kind differs from root kind
```

The refusals:

- A `q` tag of 8 digits, or with no value, cites nothing
  (`N34_MALFORMED`).
- A kind 1 note is not a chat message (`N34_KIND`).
- A `title` tag without a value (`N34_MALFORMED`).
- NIP-7D's own short reply, with `K` and `E` only, lacks the parent tags
  that NIP-22 requires (`N34_MISSING`).
- A comment on a kind 1 note is not a reply to a thread
  (`N7D_ELSEWHERE`).
- A comment whose `k` is neither the root's kind nor 1111 breaks NIP-22's
  own rule (`N22_KINDS`), before NIP-7D is asked.

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/chats-threads.out](../fixtures/chats-threads.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/groups-communities.coax](../examples/checks/groups-communities.coax),
which loads the example. They compare the code with values it did not
produce:

- the ids of all 16 events against a Python script written from NIP-01
  alone (`hashlib.sha256` of the serialized array), which also cuts the
  `previous` references from the ids it computed;
- every verdict of the group's 14 events against the hand values above, on
  events rebuilt from Python's ids;
- NIP-29's own examples, verbatim: the `previous` example reads as three
  references and refuses on this group, the 39000 metadata example is
  admitted from the relay's key and refused from Mallory's, and the join
  request reads its `h` and `code`. NIP-29's examples use placeholders
  such as `<pubkey1-as-hex>` where a key belongs, so a `put-user` with
  that `p` tag refuses as a bad tag;
- the three NIP-72 approvals against the hand values, and a fourth by
  Carol, who is not listed.

<!-- source: nostr/examples/checks/groups-communities.coax check -->
```jai
// The ids from a Python script written from NIP-01 (it shares no code with
// Coax): hashlib.sha256 of json.dumps([0, pubkey, created_at, kind, tags,
// content], separators=(",", ":"), ensure_ascii=False), over the same
// events, with the previous references in P1, P2 and P3 cut by the script
// itself from the ids it computed. The last two are the community and
// Carol's post.
PY_ID :: [EVENTS + 2] string.[
  "1faedc7d07730a6d560ea3af069d93f2ded8beb565e8fcc9653d301c214fcdb6",
  "f98f23a05a149673d140661f847c3ff8fc5c2e1050af59b64e271b3ffd8626d6",
  "62a812ccc6e11b9e82139ce1dc69a439b806ab3df0f062024e9ae6ebe92eda39",
  "4bff9f37ff627845b7cc7d0087cafbbdd8180ccefe63b8e4a4090ad84a2d8102",
  "a501a8c65a30d55e443053486237fadd99a32b1751123464a0b325c46426e556",
  "f9e7b4a040395cdcd2b4aeb20c5ec195b3642913dc54aafa0c36cc94907cfd5e",
  "805b8c31397dcaa0bc3a5e60584301e91ce9ca80a29472119a427d6e0b02cf1c",
  "a081498c3b97ffa6ee5a7845bd69f75781a17aa37d17eed1ada2bd8ecc5a0c53",
  "63882d8a2035fee60ff605a59ed63bb44ac8eb31ae249761edaec53b5dcbcc8c",
  "9c13a9ebcbf5f3beb95055f36b43d77237ddac086469a9e73dd066f57d78c53f",
  "6d531217a74343419f0b23326d27a477a3754366f3ccaa219e6e004537374c65",
  "e75fb06b540835d52b6d6c3c643086e8fc3a879b2c26d5b3b2ee0d5a59eb8cf1",
  "c4ecb4fa625fbad31cb35a8bd44b64a47c6993be057e20382f0852d0299e19b3",
  "0377621b32a6ffa8a0f6738bfd8f5c67eb44f8aab0bd4124693ff19961d5fb1e",
  "d11bc0cadd3f5c7b1f6b520af004d3c4e7d19566464a46decdab13fc48614c36",
  "f77d76cda3295ff34e404f71332742f5538c34224b285371e30ebf7f9a41f7d9"];
SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
COMMUNITY_TAGS :: "[[\"d\",\"pizza-fans\"],[\"name\",\"Pizza fans\"],[\"p\",\"^1\",\"\",\"moderator\"]]";
POST_TAGS :: "[[\"A\",\"34550:^0:pizza-fans\"],[\"a\",\"34550:^0:pizza-fans\"],[\"P\",\"^0\"],[\"p\",\"^0\"],[\"K\",\"34550\"],[\"k\",\"34550\"]]";

// An event with this id and a zero signature, read with its id recomputed
// when check_id is set. The signature is not checked here (the example
// checks every one), so the event is marked verified by hand.
py :: (id: string, who: int, at: int, kind: int, tags: string, content: string, check_id: bool, out: [] u8, scratch: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, id);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB[who]);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, at);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, kind);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, content);
  n = json_raw(out, n, ",\"sig\":\""); n = json_raw(out, n, SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  e := ifx check_id then nostr_event_read(cast(string) out[0..<n], scratch, false) else ne_parse(cast(string) out[0..<n]);
  e.verified = e.error == NE_OK;
  return e;
}
Events :: struct { ids: [EVENTS + 2] [32] u8; buf: [EVENTS + 2] [2048] u8; e: [EVENTS + 2] NostrEvent; }
read_all :: (all: [] Events) {
  scratch: [4096] u8; tags: [2048] u8;
  for k: 0..<EVENTS + 2 {
    t := ifx k == EVENTS then COMMUNITY_TAGS else ifx k == EVENTS + 1 then POST_TAGS else TAGS[k];
    who := ifx k == EVENTS then ALICE else ifx k == EVENTS + 1 then CAROL else WHO[k];
    at := ifx k == EVENTS then T0 + 1000 else ifx k == EVENTS + 1 then T0 + 1060 else T0 + 60 * k;
    kind := ifx k == EVENTS then 34550 else ifx k == EVENTS + 1 then 1111 else KIND[k];
    content := ifx k == EVENTS then "" else ifx k == EVENTS + 1 then "first slice" else CONTENT[k];
    all[0].e[k] = py(PY_ID[k], who, at, kind, with_refs(t, all[0].ids[..], tags[..]), content, true, all[0].buf[k][..], scratch[..]);
    all[0].ids[k] = all[0].e[k].id;
  }
}

#test "the ids are Python's" {
  all: [1] Events; read_all(all[..]);
  for k: 0..<EVENTS + 2 { #assert all[0].e[k].error == NE_OK && ns_equal32(all[0].e[k].computed_id, all[0].e[k].id); }
}

// The worked example by hand. E1 makes Alice the admin; E2 makes the group
// restricted and closed; E3 adds Bob; E4 adds Carol as a moderator; E5
// removes her. P1: Bob is a member and E1 and E3 are known: admitted. P2:
// Carol was removed and the group is restricted: not a member. P3: deadbeef
// names no event the group admitted: unknown previous. M1: Bob holds no
// role: not allowed. I1 adds slice-42. J1 has the wrong code, J2 none in a
// closed group, J3 the right one, J4 is Mallory again: duplicate.
HAND :: [EVENTS] int.[G29_OK, G29_OK, G29_OK, G29_OK, G29_OK, G29_OK, G29_NOT_MEMBER, G29_PREVIOUS,
  G29_NOT_ALLOWED, G29_OK, G29_INVITE, G29_CLOSED, G29_OK, G29_DUPLICATE];
#test "every verdict matches the hand values" {
  all: [1] Events; read_all(all[..]); g: [1] G29Group; relay := key_of(RELAY);
  for k: 0..<EVENTS {
    #assert g29_admit(g[..], all[0].e[k], relay) == HAND[k];
    if k == 4 { #assert g[0].n == 2 && g29_member(g[0], key_of(CAROL)) == -1; }
  }
  #assert g[0].n == 3 && g29_member(g[0], key_of(MALLORY)) == 2 && g29_can(g[0], key_of(ALICE), relay) == G29_CAN_ALL;
  #assert g29_can(g[0], key_of(BOB), relay) == 0 && g[0].restricted && g[0].closed && g29_is(g[0].name, "Pizza Lovers");
}

// NIP-29's own examples, verbatim. Their ids and keys are placeholders such
// as "<group-id>" and "<pubkey1-as-hex>", so they read only where the text
// is a value, not a key: the previous example reads as three references,
// the 39000 metadata example is admitted only from the relay's key, and a
// put-user whose p tag is "<pubkey1-as-hex>" refuses as a bad tag.
NIP29_PREVIOUS :: "[[\"h\",\"pizza\"],[\"previous\", \"eb96c864\", \"2db75638\", \"b5d1065f\"]]";
NIP29_METADATA :: "[[\"d\", \"<group-id>\"],[\"name\", \"Pizza Lovers\"],[\"picture\", \"https://pizza.com/pizza.png\"],[\"banner\", \"https://pizza.com/banner.png\"],[\"about\", \"a group for people who love pizza\"],[\"private\"],[\"closed\"],[\"supported_kinds\", \"9\", \"11\"]]";
NIP29_JOIN :: "[[\"h\", \"<group-id>\"],[\"code\", \"<optional-invite-code>\"]]";
NIP29_ADMIN :: "[[\"h\", \"<group-id>\"],[\"p\", \"<pubkey1-as-hex>\", \"ceo\"]]";
#test "NIP-29's examples" {
  all: [1] Events; read_all(all[..]); g: [1] G29Group; relay := key_of(RELAY);
  for k: 0..<10 { #assert g29_admit(g[..], all[0].e[k], relay) == HAND[k]; }
  scratch: [4096] u8; out: [2048] u8; zero := "0000000000000000000000000000000000000000000000000000000000000000";
  e := py(zero, BOB, T0 + 2000, 9, NIP29_PREVIOUS, "", false, out[..], scratch[..]);
  r := g29_read(e);
  #assert r.status == G29_OK && r.previous_n == 3 && r.previous[0][0] == 0xeb && r.previous[1][0] == 0x2d && r.previous[2][3] == 0x5f;
  #assert g29_admit(g[..], e, relay) == G29_PREVIOUS;
  #assert g29_admit(g[..], py(zero, RELAY, T0, 39000, NIP29_METADATA, "", false, out[..], scratch[..]), relay) == G29_OK;
  #assert g29_admit(g[..], py(zero, MALLORY, T0, 39000, NIP29_METADATA, "", false, out[..], scratch[..]), relay) == G29_SIGNER;
  j := g29_read(py(zero, MALLORY, T0, 9021, NIP29_JOIN, "", false, out[..], scratch[..]));
  #assert j.status == G29_OK && g29_is(j.h, "<group-id>") && g29_is(j.code, "<optional-invite-code>");
  #assert g29_admit(g[..], py(zero, MALLORY, T0, 9021, NIP29_JOIN, "", false, out[..], scratch[..]), relay) == G29_NO_GROUP;
  #assert g29_read(py(zero, ALICE, T0, 9000, NIP29_ADMIN, "", false, out[..], scratch[..])).status == G29_SHAPE;
}

// NIP-72 by hand: Bob is the only p tag marked moderator. His approval
// names the community, Carol's post and Carol: counts. Mallory's names the
// same, but she is not listed. Bob's third names P1's id: another post.
approve :: (who: int, post: int, all: [] Events, out: [] u8) -> NostrEvent {
  template: [512] u8; tags: [2048] u8; scratch: [4096] u8;
  n := json_raw(template[..], 0, "[[\"a\",\"34550:^0:pizza-fans\"],[\"e\",\"*");
  n = json_write_int(template[..], n, post); n = json_raw(template[..], n, "\"],[\"p\",\"^2\"],[\"k\",\"1111\"]]");
  zero := "0000000000000000000000000000000000000000000000000000000000000000";
  return py(zero, who, T0 + 1120, 4550, with_refs(cast(string) template[0..<n], all[0].ids[..], tags[..]), "", false, out, scratch[..]);
}
#test "NIP-72 approvals match the hand values" {
  all: [1] Events; read_all(all[..]); out: [2048] u8;
  c := all[0].e[EVENTS]; q := all[0].e[EVENTS + 1];
  #assert n72_counts(c, q, approve(BOB, EVENTS + 1, all[..], out[..])) == N72_OK;
  #assert n72_counts(c, q, approve(MALLORY, EVENTS + 1, all[..], out[..])) == N72_NOT_MODERATOR;
  #assert n72_counts(c, q, approve(BOB, 5, all[..], out[..])) == N72_POST;
  #assert n72_counts(c, q, approve(CAROL, EVENTS + 1, all[..], out[..])) == N72_NOT_MODERATOR;
}
```

### Checks in the library

`nip29.coax` has `#test` checks written with its code: the worked group;
who may moderate (roles, two roles on one member, unknown roles, the
relay's key, kinds 9006, 9010 and 9020); join and leave with codes, closed
and open groups and duplicates; create, exists, another group and delete;
metadata kinds 39000 to 39005 from the relay's key and from anyone else;
every tag that refuses, with 64 and 65 byte names; the 8-member limit and
the 50-event window; and the exact tags of the four published events.
`nip72.coax` checks an approval that counts, each way one does not, kinds,
verification and the community's own tags. The checks' helpers live in
`coax/tests/nostr/nip29_fixtures.coax` and `nip72_fixtures.coax`, so that
mutation changes the library and not the checks.

```shell
coax/bin/coax test coax/lib/nostr/nip72.coax
coax/bin/coax test nostr/examples/checks/groups-communities.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/nip29.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip72.coax --root-only
```

On 2026-11-01, `coax test` passed all 215 checks of `nip72.coax` (which
loads `nip29.coax` and runs its checks too) and all 368 of the
example's checks. `coax mutate --root-only` killed all 527 mutants of
`nip29.coax` and all 104 of `nip72.coax`, with none surviving and none
marked equivalent. The first runs left survivors; each was killed by a new
check (the first slot of the event window, a one-letter name, each kind's
own permission bit) or by removing code that could not change a result (an
unreachable return, a reset of a slot that is already empty). The example
ran in debug and release with the same output. These runs used the cached
release compiler in `~/.cache/coax-release`. The full mutation run
(`COAX_MUTATION_FULL=1`) and a mutation run of the example were not run.

### Checks of chats and threads

The checks are in
[nostr/examples/checks/chats-threads.coax](../examples/checks/chats-threads.coax),
which loads the example. They compare the code with values it did not
produce:

- the ids of the five events, and the tags of C2, T1, R1 and R2, against a
  Python script written from NIP-01, NIP-C7, NIP-7D and NIP-22;
- each reading against the hand values above, on events rebuilt from
  Python's ids;
- NIP-C7's and NIP-7D's own examples, with an id, a relay and a key where
  the NIPs write placeholders.

<!-- source: nostr/examples/checks/chats-threads.coax check -->
```jai
// The ids and tags from a Python script written from NIP-01, NIP-C7, NIP-7D
// and NIP-22 (it shares no code with Coax): the ids are hashlib.sha256 of
// json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False).
PUB :: [3] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
  "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"];
PY_ID :: [5] string.[
  "f9e8ae5406b60f1417b03ce2d7366cf0833d16aabbe09cdd92c616f48fff7680",
  "c38a6f009d18565e779fb4163176e8af958e3a08d932a00c3c1adc5d3b705913",
  "386c56b8dae39d338b82bcc26fe5a80a6230e167c5f98004adda66118e44efc1",
  "4777a42b623e988dc88a81123b3708831b68d6717cbca538a2e3f56dc1f82604",
  "d0e9dda8ae20ea726b10f3fb8c018c77cb59d465b5bf00774ea56c6438a47d8a"];
PY_C2 :: "[[\"q\",\"f9e8ae5406b60f1417b03ce2d7366cf0833d16aabbe09cdd92c616f48fff7680\",\"wss://relay.example.com\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"]]";
PY_T1 :: "[[\"title\",\"Which kettle?\"]]";
PY_R1 :: "[[\"E\",\"386c56b8dae39d338b82bcc26fe5a80a6230e167c5f98004adda66118e44efc1\",\"wss://relay.example.com\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"],[\"K\",\"11\"],[\"P\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"],[\"e\",\"386c56b8dae39d338b82bcc26fe5a80a6230e167c5f98004adda66118e44efc1\",\"wss://relay.example.com\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"],[\"k\",\"11\"],[\"p\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"]]";
PY_R2 :: "[[\"E\",\"386c56b8dae39d338b82bcc26fe5a80a6230e167c5f98004adda66118e44efc1\",\"wss://relay.example.com\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"],[\"K\",\"11\"],[\"P\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"],[\"e\",\"4777a42b623e988dc88a81123b3708831b68d6717cbca538a2e3f56dc1f82604\",\"wss://relay.example.com\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"k\",\"1111\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"]]";
PY_AUTHOR :: [5] int.[0, 1, 2, 0, 1];
PY_AT :: [5] int.[0, 30, 120, 180, 240];
PY_KIND :: [5] int.[9, 9, 11, 1111, 1111];
PY_TAGS :: [5] string.["[]", PY_C2, PY_T1, PY_R1, PY_R2];
PY_CONTENT :: [5] string.["GM", "GM, Alice", "Gooseneck or plain?", "Gooseneck. It pours slower.", "Slower is the point."];

// Event i of the script with a zero signature, read with its id recomputed;
// the signature is not checked.
py :: (i: int, out: [] u8, scratch: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, PY_ID[i]);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB[PY_AUTHOR[i]]);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, T0 + PY_AT[i]);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, PY_KIND[i]);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, PY_TAGS[i]);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, PY_CONTENT[i]);
  n = json_raw(out, n, ",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}

#test "the ids are Python's, and so are the tags the library and the example write" {
  buf: [5][4096] u8; scratch: [4096] u8;
  for i: 0..<5 { e := py(i, buf[i][..], scratch[..]); #assert e.error == NE_OK && n34_same(e.computed_id, e.id); }
  tags: [1024] u8;
  n := nc7_reply_tags(PY_ID[0], RELAY, PUB[ALICE], tags[..]); #assert cast(string) tags[0..<n] == PY_C2;
  n = n7d_thread_tags("Which kettle?", tags[..]); #assert cast(string) tags[0..<n] == PY_T1;
  n = n7d_reply_tags(PY_ID[2], RELAY, PUB[CAROL], tags[..]); #assert cast(string) tags[0..<n] == PY_R1;
  t: [1] Tags; nested_tags(t[..], PY_ID[2], PUB[CAROL], PY_ID[3], PUB[ALICE]); #assert tags_of(t[..]) == PY_R2;
}

// By hand: C1 has no q tag, so it starts a conversation. C2's one q tag
// holds C1's id, so C2 is a reply to C1. T1 is a thread titled "Which
// kettle?". R1's root (E, K 11) and parent (e, k 11) are both T1: a reply
// to the thread. R2's root is T1 but its parent is R1, a comment (k 1111):
// NIP-22 reads it, NIP-7D calls it nested.
#test "Python's events read as worked by hand" {
  buf: [5][4096] u8; scratch: [4096] u8; ev: [5] NtEvent;
  for i: 0..<5 { ev[i] = nt_event_of(py(i, buf[i][..], scratch[..])); }
  c := nc7_chat(ev[0]); #assert c.status == N34_OK && c.quotes == 0 && !c.parent.found;
  c = nc7_chat(ev[1]); #assert c.status == N34_OK && c.quotes == 1 && c.parent.found;
  #assert nt_raw(ev[1], c.parent.v[0]) == PY_ID[0] && nt_raw(ev[1], c.parent.v[1]) == RELAY && nt_raw(ev[1], c.parent.v[2]) == PUB[ALICE];
  t := n7d_thread(ev[2]); #assert t.status == N34_OK && nt_raw(ev[2], t.title.v[0]) == "Which kettle?";
  r := n7d_reply(ev[3]); #assert r.status == N34_OK && nt_raw(ev[3], r.thread.v[0]) == PY_ID[2] && nt_raw(ev[3], r.author.v[0]) == PUB[CAROL];
  r = n7d_reply(ev[4]); #assert r.status == N7D_NESTED && nt_raw(ev[4], r.thread.v[0]) == PY_ID[2];
  #assert n22_comment(ev[4]).status == N34_OK;
  // Each reader refuses the other kinds: a chat view holds kind 9 only.
  #assert nc7_chat(ev[2]).status == N34_KIND && nc7_chat(ev[3]).status == N34_KIND && n7d_thread(ev[0]).status == N34_KIND && n7d_reply(ev[2]).status == N34_KIND;
}

// NIP-C7's and NIP-7D's own examples. Where a NIP writes a placeholder such
// as <event-id>, the check puts Python's id, relay and key in its place.
#test "the NIPs' own examples" {
  // NIP-C7: {"kind": 9, "content": "GM", "tags": []}.
  c := nc7_chat(n34_event("[]", 9)); #assert c.status == N34_OK && !c.parent.found;
  // NIP-C7: a reply, ["q", <event-id>, <relay-url>, <pubkey>].
  e := n34_event(PY_C2, 9); c = nc7_chat(e); #assert c.status == N34_OK && c.parent.found && c.parent.values == 3;
  // NIP-7D: {"kind": 11, "content": "Good morning", "tags": [["title", "GM"]]}.
  e = n34_event("[[\"title\",\"GM\"]]", 11); t := n7d_thread(e); #assert t.status == N34_OK && nt_raw(e, t.title.v[0]) == "GM";
  // NIP-7D's reply has K and E only. NIP-22, which it says replies "MUST
  // use", requires the parent's e and k too: the example refuses as missing.
  buf: [512] u8; n := json_raw(buf[..], 0, "[[\"K\",\"11\"],[\"E\",\""); n = json_raw(buf[..], n, PY_ID[2]);
  n = json_raw(buf[..], n, "\",\"wss://relay.example.com\",\""); n = json_raw(buf[..], n, PUB[CAROL]); n = json_raw(buf[..], n, "\"]]");
  #assert n7d_reply(n34_event(cast(string) buf[0..<n], 1111)).status == N34_MISSING;
}
```

`nipc7.coax` and `nip7d.coax` have `#test` checks written with their code.
`nipc7.coax` checks a message with no tags, a reply with and without a
relay and an author, a quoted article, a profile's kind 0 address, the
first event id among several `q` tags, every `q` tag that refuses, the
kinds on both sides of 9, and the reply tags with bad ids and a short
buffer. `nip7d.coax` checks a thread with and without a title, a reply
written by `n7d_reply_tags` and read back by both `n7d_reply` and
`n22_comment`, four nested replies, five roots that are elsewhere,
NIP-7D's short example, and what NIP-22 refuses.

```shell
coax/bin/coax test coax/lib/nostr/nipc7.coax
coax/bin/coax test coax/lib/nostr/nip7d.coax
coax/bin/coax test nostr/examples/checks/chats-threads.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/nipc7.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip7d.coax --root-only
```

On 2026-11-12, `coax test` passed all 48 checks of `nipc7.coax`, all 56 of
`nip7d.coax` and all 394 of the example's checks (each count includes the
loaded files' checks). `coax mutate --root-only` killed all 47 mutants of
`nipc7.coax` and all 99 of `nip7d.coax`, with none surviving and none
marked equivalent. The first run of `nipc7.coax` left four survivors; each
was killed by a new check (a one-letter author, a kind 0 address) or by
removing a condition that could not change a result. The full mutation
run (`COAX_MUTATION_FULL=1`) was not run.

### The relay's groups

Since chapter 103's wiring, `nostr/relay/relay.coax` started with
`--groups` puts every `EVENT` to `g29_admit` after its signature, the rate
limit and NIP-86's `n86_admit`:

- The relay keeps up to 64 groups. `relay_route.coax`'s `rr_group_slot`
  (`#authority`) picks the slot: the live group whose id is the event's
  `h` tag, or for a 9007 the first free slot; a 9007 with none free is
  `restricted: this relay holds no more groups`.
- `g29_admit` runs on a copy of the group, and the copy replaces the group
  only when the store then takes the event, so a refused or duplicate
  event changes nothing.
- Each refusal is an `OK` false with a NIP-01 prefix (`rr_group_prefix`,
  `rr_group_text`): for example `invalid: no such group`, `duplicate:
  already a member`, `restricted: your roles in this group don't allow
  this`.
- An event without an `h` tag is not the groups' business (`G29_PASS`),
  except kinds 39000 to 39005, which only the relay may sign.
- NIP-11 lists 29 only with `--groups` (`rr_nip11_nip`).
- With `--store FILE` the groups survive a restart the way chapter 39's
  tombstones do: the moderation events are in the store file, so after
  the store's replay `ry_groups_replay` reads the file again in order and
  puts each stored event with an `h` tag through the same step as a live
  `EVENT` (`ry_group`: `rr_group_slot`, then `g29_admit`), keeping the
  state when `relay_route.coax`'s `rr_group_keep` (`#authority`) says the
  group admitted it. Every event in the file was stored, so each counts as
  taken. Nothing new is written: the store file is the record. Without
  `--store` the groups are in memory.

Root-only mutation of `relay_route.coax` on 2026-11-01: 890 of 890
killed, and 914 of 914 after `rr_group_keep` and `rr_admin_saves`
(chapter 67) were added. `relay_session.py` (ASan and UBSan) creates a group, posts to it,
joins it from a second key, and checks the refusals for a second create,
a 39000, a group that doesn't exist, a second join and a member putting a
user. Its restart check creates a group with `--store`, posts to it and
joins it, restarts, and finds the group whole: a second create is
`duplicate: the group already exists`, the member's second join
`duplicate: already a member`, and both post.

### Not built

- In the relay: the relay has no key of its own, so it publishes no 39000
  to 39003 events, and no admin or moderation event can be the relay's.
  Without `--store` the groups are kept in memory and a restart starts
  with none. The rebuild replays every stored group event, so it costs a
  second read of the file and a signature check for each event with an
  `h` tag. A 9005 drops the event from the group's
  `previous` window but not from the store.
- Kind 9010 `update-pin-list` and 39005 pinned events, 39004 LiveKit
  participants and the LiveKit token endpoint, subgroups (`parent` and
  `child` tags), and `supported_kinds`.
- Reading permissions: `private` and `hidden` are kept and published, but
  filtering what non-members may read belongs to the relay's query path.
- Late publication limits, a minimum number of `previous` references, and
  issuing the 9001 that answers a leave request.
- Migration and forks: kind 10009 lists and following a group to a new
  relay.
- The client side of groups: NIP-28 public chat (kinds 40 to 44), and
  showing a group from its 39000 to 39003 events.
- In chats and threads: fetching a reply's parent to see that it is a
  kind 9, checking that a reply's content cites its parent with a
  `nostr:nevent1` link as NIP-C7's example does, ordering a chat view by
  time, and deciding how a thread view shows a nested reply.
- NIP-72 approvals by `a` tag (for every version of a replaceable post),
  and checking that an approval's content is the approved event.

## Summary

- A NIP-29 group is an id that one relay enforces rules for; events join it
  with an `h` tag.
- `g29_admit` admits a verified event only when the group's rules allow
  it, and changes the group's state only for an admitted event.
- Moderation kinds need a permission that the signer's roles grant; this
  relay's `admin` may do everything and `moderator` only delete events.
- A restricted group refuses a non-member's post; a closed group refuses a
  join request without a valid invite code.
- Every `previous` reference must name an event the group admitted, so a
  message replayed on a fork's relay refuses.
- Kinds 39000 to 39005 are accepted only from the relay's own key.
- In a NIP-72 community, a post counts only when a listed moderator's
  approval names the community, the post and its author.
- A NIP-C7 chat message is a kind 9 event. `nc7_chat` reads its `q` tags,
  takes the first one that holds an event id as the parent, and refuses
  every other kind, so a chat view shows kind 9 only.
- A NIP-7D thread is a kind 11 event with a `title`. `n7d_reply` reads a
  kind 1111 comment as a reply only when its root and its parent are the
  same kind 11 event; a comment on another comment is `N7D_NESTED`.

## Dependencies

- [38. NIP-42 authentication](ch038-nip-42-authentication.md)
- [34. Admit an event](ch034-admit-an-event.md)
- [22. Kinds and tags](ch022-kinds-and-tags.md)

The chats and threads sections also use chapter 52's NIP-22 comment reader
([52. Reposts, reactions and comments](ch052-reposts-reactions-and-comments.md))
and chapter 63's tag readers ([63. Git on Nostr](ch063-git-on-nostr.md)).

## Sources

- [NIP-29](../nips/official/29.md)
- [NIP-72](../nips/official/72.md)
- [NIP-7D](../nips/official/7D.md)
- [NIP-C7](../nips/official/C7.md)
- [NIP-28](../nips/official/28.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
