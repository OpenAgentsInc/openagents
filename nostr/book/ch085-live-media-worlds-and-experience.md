# 85. Live media, worlds and experience

The Verse uses Nostr for presence in 3D worlds and for experience points.
This chapter builds the two parts of that a client must get right before it
draws anything or credits anyone: which pose frames a receiver admits, and
in what order, and which awards a reader counts. It describes the
live-media session NIP that binds voice, camera and screen to a task, and
builds none of it.

Part VI, Every NIP. Result: NIP-MV world definitions (33300), entity states
(33301) and pose frames (23300) read and admitted, a receiver's ordering
and presence rules under a fixed clock, and the NIP-XP quests (30193),
awards (3193) and revocations (3194) folded into a derived ledger. NIP-LIVE
is described, not built.

## What this chapter teaches

- MV: world identifiers, the right-handed meters frame, cells, the world
  definition's bounds, entity state and its address, and pose frames.
- MV: why a pose outside its world's bounds refuses, why a receiver orders
  frames by session and sequence rather than by `created_at`, and when a
  visitor counts as offline.
- MV: the object state's id, the first 20 hex digits of a SHA-256.
- XP: quests as frozen versions, awards bound to the exact quest event,
  revocations, and how a reader derives a ledger it can re-derive.
- XP: why two live awards on one key count for no one, and how a
  revocation frees the key.
- LIVE: session plans, consent, participants, floors, lifecycle and
  transcripts, in outline.

## Where the code is

- [coax/lib/nostr/nipmv.coax](../../coax/lib/nostr/nipmv.coax):
  `mv_world_read`, `mv_frame_read`, `mv_state_read`, `mv_order`,
  `mv_online`, `mv_cell` and `mv_object_id`. It is `#authority`: it
  decides which poses a receiver admits.
- [coax/lib/nostr/xp_ledger.coax](../../coax/lib/nostr/xp_ledger.coax):
  `xp_quest_read`, `xp_award_read`, `xp_revocation_read` and `xp_ledger`.
  It is `#authority`: it decides who is credited. It reuses `xk_tags` and
  `xk_hex64` from [xp_keys.coax](../../coax/lib/nostr/xp_keys.coax), which
  reads the trainer profiles and key links.
- Both read tags with `nt_tag` from
  [nip34.coax](../../coax/lib/nostr/nip34.coax) and JSON with
  [json.coax](../../coax/lib/nostr/json.coax). `nipmv.coax` reads
  coordinates with [json-f64.coax](../../coax/lib/nostr/json-f64.coax) and
  hashes with [sha256.coax](../../coax/lib/crypto/sha256.coax).
- The example, [nostr/examples/worlds-and-xp.coax](../examples/worlds-and-xp.coax),
  its checks, and the vectors
  [coax/tests/nostr/xp_mv_vectors.py](../../coax/tests/nostr/xp_mv_vectors.py)
  writes.

The Verse realms in `verse/` simulate and draw worlds (`player.coax`,
`space.coax`, `throw.coax`); none of them reads or writes NIP-MV events
yet, so this chapter shares no code with them. The voice floors and media
epochs NIP-LIVE describes have a local Coax form in
`verse/voice-floor.coax`, which is not a Nostr reader either.

## Three NIPs, one place

NIP-MV says where everyone stands. NIP-XP says what accepted work each key
has done. NIP-LIVE says who may hear, see and record a task's session.
They share one stance: a signature proves who published an event, never
that its content is honest or that it grants anything. A pose does not
authorize combat, an award is not a balance, and a voice in a room is not
an approver. Each reader decides what it admits from rules it can run
itself.

## MV: worlds, coordinates and cells

Every NIP-MV event carries exactly one `w` tag naming its world: either the
address of a world definition, `33300:<pubkey>:<d>`, or an opaque string of
at most 128 bytes agreed out of band. Two different identifiers are two
worlds.

Units are meters. The frame is right-handed with `+Y` up, so the ground is
the `XZ` plane. A position is `[x, y, z]`, an orientation a unit quaternion
`[x, y, z, w]`. Numbers must be finite; a receiver normalizes each
quaternion and discards one whose length is near zero.

A cell is a square of side `cell` meters on the ground. The cell of a
position is `[floor(x / cell), floor(z / cell)]`, written `"<cx>,<cz>"`.
Frames and states carry a `c` tag per occupied cell, so a client can
subscribe to the cells around it with `#c` rather than to the whole world.
`floor` rounds toward minus infinity: `x = -0.5` is in cell `-1`, not `0`.

The world definition, kind 33300, is addressable by `d`, and its `w` tag
names its own address. Its content has `v`, an optional `cell` (default
64) and optional `bounds`, `min` and `max` positions. NIP-MV's own example
is the plaza:

```text
{"v":1,"cell":64,"bounds":{"min":[-264,0,-264],"max":[264,200,264]},"spawn":{"center":[0,0,0],"radius":28}}
```

`mv_world_read` refuses another kind, a missing or repeated `d`, a `w` that
isn't the definition's own address, `v` other than 1, a cell that isn't a
positive finite number, and bounds that aren't exactly `min` and `max`
with `min <= max` on every axis. It skips `spawn` and `scene`: NIP-MV lets
a presence-only reader ignore the scene profile, as long as it doesn't
claim to implement it.

## MV: pose frames

A pose frame, kind 23300, is ephemeral: relays forward it and don't store
it. Its content has a session `s` (1 to 16 bytes, random per client
session), a sequence `n`, a publisher time `t` in milliseconds, and `e`, 1
to 16 entity poses. Each entry has an `id` (1 to 64 bytes of
`[a-z0-9_-]`), a `role`, `p` and `q`, and may have a velocity `v`,
`follows` and an animation `a`. An entry with role `body` reports a shared
body and must carry its stamp `k = [epoch, rev]`; no other role may carry
one.

When the world states bounds, a position outside them refuses. NIP-MV says
a receiver must discard shared-body reports outside the world's bounds;
this reader holds every entity to the bounds its world states, which is
also what lets a renderer trust that every admitted pose is somewhere it
can draw. Every coordinate is also held to `|x| < 10^9` meters before use,
this reader's own bound, since NIP-MV requires receivers to validate every
number.

<!-- source: coax/lib/nostr/nipmv.coax entry -->
```jai
// One entry, its opening brace just read.
MvEntry :: struct {code:MvCode;pose:MvPose;}
mv_entry :: (j:[]Json,w:MvWorld)->MvEntry {
  r:MvEntry;seen:=0;spare:[3]f64;
  t:=json_next(j);
  while t!=JSON_OBJECT_END {
    if t!=JSON_STRING {return MvEntry.{code=MvCode.body};}
    key:=json_text(j);v:=json_next(j);bit:=0;
    if key=="id" {bit=1;} else if key=="role" {bit=2;} else if key=="p" {bit=4;} else if key=="q" {bit=8;}
    else if key=="v" {bit=16;} else if key=="follows" {bit=32;} else if key=="w" {bit=64;} else if key=="k" {bit=128;}
    if bit==0 {if !json_skip(j) {return MvEntry.{code=MvCode.body};}}
    else {
      if (seen&bit)!=0 {return MvEntry.{code=MvCode.field};}
      seen|=bit;
      if bit==1 {if v!=JSON_STRING || !mv_id_ok(json_text(j)) {return MvEntry.{code=MvCode.id};}r.pose.id=json_text(j);}
      if bit==2 {if v!=JSON_STRING || json_text(j).count==0 {return MvEntry.{code=MvCode.role};}r.pose.role=json_text(j);}
      if bit==4 {if !mv_vec(j,v,r.pose.p[..]) {return MvEntry.{code=MvCode.number};}}
      if bit==8 {if !mv_vec(j,v,r.pose.q[..]) {return MvEntry.{code=MvCode.number};}}
      if bit==16 || bit==64 {if !mv_vec(j,v,spare[..]) {return MvEntry.{code=MvCode.number};}}
      if bit==32 {if v!=JSON_STRING || !mv_id_ok(json_text(j)) {return MvEntry.{code=MvCode.id};}}
      if bit==128 {
        if v!=JSON_ARRAY {return MvEntry.{code=MvCode.stamp};}
        r.pose.epoch=mv_count(j,json_next(j));r.pose.rev=mv_count(j,json_next(j));
        if r.pose.epoch<0 || r.pose.rev<0 || json_next(j)!=JSON_ARRAY_END {return MvEntry.{code=MvCode.stamp};}
      }
    }
    t=json_next(j);
  }
  if (seen&15)!=15 {return MvEntry.{code=MvCode.field};}
  r.pose.body=r.pose.role=="body";
  if r.pose.body!=((seen&128)!=0) {return MvEntry.{code=MvCode.stamp};}
  if !mv_unit(r.pose.q[..]) {return MvEntry.{code=MvCode.quat};}
  if !mv_inside(w,r.pose.p) {return MvEntry.{code=MvCode.bounds};}
  r.pose.cx=mv_cell(r.pose.p[0],w.cell);r.pose.cz=mv_cell(r.pose.p[2],w.cell);
  return r;
}
```

Coordinates are read with `json-f64.coax`, which is within a few ulps of
the decimal rather than correctly rounded. A coordinate within that
distance of a bound may land on either side. The bounds and coordinates in
NIP-MV's examples, integers and short decimals, read exactly.

### Ordering and presence

`created_at` has one-second resolution and a client sends about ten frames
a second, so a receiver can't order frames by it. NIP-MV orders them by
`(s, n)`: a receiver drops a frame whose `n` is not greater than the last
one seen for that publisher and session. A replayed frame, or one that
arrives after a newer one, is dropped. A new session (a reloaded page)
starts its own sequence. The table of sessions is bounded, as NIP-MV
requires, and a full table refuses.

A receiver treats an entity as offline when no frame has arrived for about
10 seconds, and falls back to its entity state. Both rules take the clock
as an argument, so the same frames and the same clock always give the same
answer.

<!-- source: coax/lib/nostr/nipmv.coax order -->
```jai
mv_order :: (seen:[]MvSeen,count:int,pubkey:string,s:string,n:int,ms:int)->MvOrder {
  for i:0..<count {
    if seen[i].pubkey==pubkey && seen[i].s==s {
      if n<=seen[i].n {return MvOrder.{code=MvCode.old,count=count};}
      seen[i].n=n;seen[i].ms=ms;
      return MvOrder.{count=count};
    }
  }
  if count>=seen.count {return MvOrder.{code=MvCode.full,count=count};}
  seen[count]=MvSeen.{pubkey=pubkey,s=s,n=n,ms=ms};
  return MvOrder.{count=count+1};
}
mv_online :: (last_ms:int,now_ms:int)->bool {return now_ms-last_ms<MV_OFFLINE_MS;}
```

### Entity state and object ids

An entity state, kind 33301, is the durable record a later visitor reads.
Its `d` is the world identifier, a slash and the entity id, so a relay keeps
one per publisher and entity. The content repeats the id and role, and
carries `p`, `q`, `t` in milliseconds and `online`. A publisher writes one
when an entity joins, comes to rest and leaves (`online: false`), never at
frame rate.

`mv_state_read` checks `d` against the world and the content's id, the
`role` tag against the content's role, `p` against the bounds, and an
avatar's optional `status`: at most 64 bytes with no control character.
A world authority publishes a stateful object (a lamp, a door) as role
`object` with its world-tree `node`. Since a node ID has slashes, the
entity id is `obj-` and the first 20 hex digits of SHA-256(node), and a
reader refuses an object whose id isn't its node's: for `plaza/lamp-0`
that is `obj-558157d13a69707d2163`.

## XP: quests, awards and revocations

XP is evidence of verified accepted work. It is never a balance: it can't
be spent, transferred, sold or turned into sats or permissions, and it is
awarded once per accepted outcome. A quest purse that pays sats settles
through the payment contracts, separately; paying never creates XP.

Every XP body is a JSON object with `v: 1`, `requires: []` and a `type`,
and every event carries exactly one `t` marker `oa:xp:<type>:v1`. A body
whose type, marker and kind disagree is refused, and so is any unknown key.

A quest, kind 30193, is a frozen version at its own address: `d` is
`<id>@<version>`, such as `tb4.fix-git.beat-fable-low@1`. Its body names a
season window, a title and objective, the acceptance rule and its
parameters, the fixed XP per role (`author` and `runner` under
`kb-transfer`, `claimant` and `reproducer` under `reproduce`, summing to 1
to 1,000), and the uniqueness policy. Two quest events at one address are
a rewrite of a frozen version; a reader reports it and counts no award
under it.

An award, kind 3193, is the referee's signed acceptance of one completion.
It pins the exact quest event by id, its `key` is the quest version's
coordinate `30193:<referee>:<address>` under `completions: first`, and it
lists the awardees with their roles and XP. Its signer must be the quest's
signer: a referee awards only its own quests.

A revocation, kind 3194, ends one award irreversibly. Only the award's
referee can sign one; another key's disagreement is not a revocation but a
reason for a reader to change whom it trusts. A revocation frees the key:
the referee may then award the version to a correct completion.

The readers refuse each of those mismatches with its own code. This file
reads only the `kb-transfer` and `reproduce` roles and the `first` policy:
a `playtest`, `eval-check`, `eval-adopt` or `pylon-check` quest refuses as
`rule` and `per-awardee` as `policy`, rather than being counted wrongly.

## XP: deriving a ledger

A reader derives XP from signed events every time; it never stores a
balance it can't re-derive. For its trusted referees it merges events by
id, drops revoked awards, binds each award to its exact quest event, checks
the key, the season and the XP table, groups the survivors by referee and
key, and credits each awardee of each award that is alone on its key. Two
live awards on one key are a conflict, and neither counts: the reader never
picks one by timestamp.

NIP-XP's step 4 also runs the quest's acceptance rule over the exact entry
and evidence events the award names. That needs NIP-KB entries and
NIP-EVAL run evidence, which this chapter doesn't build, so `xp_ledger`
takes the rule's verdict per award as `ruled` from the caller and counts
nothing whose rule wasn't run and passed.

Each credit keeps its own copy of the awardee's key, a `Kept(8)` of up to
64 bytes ([`coax/lib/kept.coax`](../../coax/lib/kept.coax)). A string only
borrows the caller's bytes. Without the copy, a
caller that reuses the buffer holding the keys would move each credit's XP
to whatever key the buffer held next. An award that names a longer key is
not counted.

<!-- source: coax/lib/nostr/xp_ledger.coax ledger -->
```jai
xp_trusted :: (trusted:[]string,key:string)->bool {for k:trusted {if k==key {return true;}}return false;}
// Whether key is 30193:<referee>:<address>.
xp_coordinate :: (key:string,referee:string,address:string)->bool {
  if key.count!=71+address.count {return false;}
  return key[0..<6]=="30193:" && key[6..<70]==referee && key[70]==':' && key[71..<key.count]==address;
}
xp_ledger :: (trusted:[]string,quests:[]XpQuest,awards:[]XpAward,revs:[]XpRevocation,ruled:[]bool,why:[]XpWhy,out:[]XpCredit)->XpLedger {
  l:XpLedger;
  for i:0..<awards.count {
    a:=awards[i];why[i]=XpWhy.counted;
    dup:=false;for k:0..<i {if awards[k].event==a.event {dup=true;}}
    if dup {why[i]=XpWhy.duplicate;continue;}
    if a.code!=XpCode.ok {why[i]=XpWhy.refused;continue;}
    if !xp_trusted(trusted,a.referee) {why[i]=XpWhy.untrusted;continue;}
    revoked:=false;
    for r:revs {if r.code==XpCode.ok && r.award==a.event && r.referee==a.referee && r.key==a.key {revoked=true;}}
    if revoked {why[i]=XpWhy.revoked;l.revoked+=1;continue;}
    at:=-1;
    for k:0..<quests.count {if quests[k].code==XpCode.ok && quests[k].event==a.quest && quests[k].referee==a.referee {at=k;}}
    if at<0 {why[i]=XpWhy.no_quest;continue;}
    q:=quests[at];rewritten:=false;
    for o:quests {if o.code==XpCode.ok && o.referee==q.referee && o.address==q.address && o.event!=q.event {rewritten=true;}}
    if rewritten {why[i]=XpWhy.rewritten;continue;}
    if !xp_coordinate(a.key,q.referee,q.address) {why[i]=XpWhy.key;continue;}
    if a.accepted<q.opens || a.accepted>q.closes {why[i]=XpWhy.time;continue;}
    if a.n!=2 || a.keys[0].count>XP_TEXT || a.keys[1].count>XP_TEXT || a.roles[0]!=q.roles[0] || a.roles[1]!=q.roles[1] || a.xp[0]!=q.xp[0] || a.xp[1]!=q.xp[1] {why[i]=XpWhy.xp;continue;}
    if !ruled[i] {why[i]=XpWhy.rule;continue;}
  }
  // Two live awards from one referee on one key: a conflict, and neither counts.
  for i:0..<awards.count {
    for k:0..<awards.count {
      live:=(why[k]==XpWhy.counted || why[k]==XpWhy.conflict) && (why[i]==XpWhy.counted || why[i]==XpWhy.conflict);
      if k!=i && live && awards[k].referee==awards[i].referee && awards[k].key==awards[i].key {why[i]=XpWhy.conflict;}
    }
  }
  for i:0..<awards.count {
    if why[i]==XpWhy.conflict {l.conflicts+=1;}
    if why[i]!=XpWhy.counted {continue;}
    l.counted+=1;
    for k:0..<awards[i].n {
      if awards[i].xp[k]==0 || l.credits<0 {continue;}
      at:=-1;for c:0..<l.credits {if kept_is(out[c].pubkey,awards[i].keys[k]) {at=c;}}
      if at<0 {
        if l.credits==out.count {l.credits= -1;continue;}
        out[l.credits]=XpCredit.{pubkey=kept64(awards[i].keys[k]),xp=0};at=l.credits;l.credits+=1;
      }
      out[at].xp+=awards[i].xp[k];
    }
  }
  return l;
}
```

## LIVE: sessions, in outline

NIP-LIVE defines no event kinds. It binds voice, camera, screen, browser
and device input to a task through artifacts exchanged over the shared
envelopes, and a host exposes five operation roles: admit a session, admit
or revoke a participant, control the lifecycle, capture bounded evidence
and apply device input.

- **Plan and consent.** A plan (`openagents.live-plan.v1`) names the task,
  the controller generation, a media epoch that rises on every reconnect,
  the transport profile, at most 32 tracks with explicit recipients,
  retention and processing, and the consent and disclosure policies.
  Joining a room is not consent to recording.
- **Participants.** Admission is per participant and per right (`publish`,
  `subscribe`, `transcribe`, `record`, `playback`); no right implies
  another, and transport credentials never appear in events or logs.
- **Floors.** At most one input floor feeds a model per destination, and at
  most one agent floor plays per track; voice recognition never decides
  who holds one.
- **Lifecycle.** `admitted -> active -> paused -> active`, ending in
  `closed`, which requires observed cleanup; a stop request is not an
  observed stop.
- **Captures and transcripts.** An observation names its exact track,
  geometry or interval; a transcript is versioned, and interim text is
  never silently rewritten as if it were the original.

None of this is built here: it rests on the capability, policy and run
contracts of later chapters, and on a media transport that isn't Nostr.

## Worked example: four frames and three ledgers, by hand

The plaza's bounds are `-264..264` in `x` and `z` and `0..200` in `y`,
with cells of 64 meters. Key 2's client sends four frames in session
`a41c`, each with one avatar:

| Frame | n | p | Cell | Bounds | Order |
| --- | --- | --- | --- | --- | --- |
| F1 | 812 | (3.2, 0, -10.5) | floor(3.2/64), floor(-10.5/64) = 0,-1 | inside | first in its session: admitted |
| F2 | 812 | (3.2, 0, -10.5) | 0,-1 | inside | 812 is not greater than 812: dropped |
| F3 | 813 | (300, 0, -10.5) | | 300 > 264: refused | never reaches ordering |
| F4 | 814 | (-0.5, 0, -10.5) | floor(-0.0078) = -1, so -1,-1 | inside | 814 > 812: admitted |

F2 is F1 sent twice: the same bytes, so the same event id, and a relay
would dedupe it; a receiver hearing it from a second relay drops it by
sequence. The quaternion `[0, 0.38, 0, 0.92]` has squared length 0.1444 +
0.8464 = 0.9908, so it normalizes to about `[0, 0.38176, 0, 0.92427]`.
F4's clock reading is 1790000001200 ms, so the avatar is online until
1790000011199 and offline from 1790000011200, when the reader falls back
to its entity state: `online: false` at (-0.5, 0, -10.5).

Key 1 referees quest `tb4.fix-git.beat-fable-low@1`: author 6 XP, runner 4,
a sum of 10, season 1790000000 to 1798000000. Award A1 (author key 2,
runner key 4) is accepted at 1790500000, inside the season, with key
`30193:<key 1>:tb4.fix-git.beat-fable-low@1`. Then:

1. A1 alone: key 2 has 6 XP, key 4 has 4.
2. A1 and the replacement A2 (author key 7, runner key 4), both live: two
   awards from key 1 on one key, a conflict. No one is credited.
3. Key 1's revocation R1 names A1 by id and key: A1 is revoked and removed;
   A2 is now alone on the key. Key 7 has 6 XP, key 4 has 4. Key 2's own
   "revocation" of A1 is refused on reading, since its signer is not the
   award's referee, and removes nothing.

## What is not built

- **MV**: shared bodies' stamps and authority beyond reading `k`, body-set
  snapshots (`set`, `b`), resets, gestures (23301), zone commands (23302),
  world chat and Gym notes, the scene manifest profile, NIP-OA owned
  players, interpolation, writers and subscriptions.
- **XP**: the acceptance rules themselves (`kb-transfer`, `reproduce`,
  `playtest`, `eval-check`, `eval-adopt`, `pylon-check`), the
  `per-awardee` policy and `max_awards`, achievements (NIP-32 labels),
  trainer cards (30194), quest and award writers, and the parts of an
  award's `entry`, `entry_version` and `evidence` a rule reads (they are
  checked only for shape). Trainer profiles and key links are in
  `xp_keys.coax`.
- **LIVE**: all of it.

## The example

[nostr/examples/worlds-and-xp.coax](../examples/worlds-and-xp.coax) runs
the worked example on the events
[coax/tests/nostr/xp_mv_vectors.py](../../coax/tests/nostr/xp_mv_vectors.py)
wrote.

<!-- source: nostr/examples/worlds-and-xp.coax example -->
```jai
// Chapter 85: a world, four pose frames from one visitor admitted and
// ordered under a fixed clock, the visitor's last entity state, and then a
// quest's awards, one revoked, folded into a ledger. Keys are the x-only
// public keys of the scalars 1, 2, 4 and 7: TEST ONLY. Every event text and
// id comes from coax/tests/nostr/xp_mv_vectors.py.
#load "../../coax/lib/nostr/nipmv.coax";
#load "../../coax/lib/nostr/xp_ledger.coax";
#import "Basic";

MV_WORDS :: [24]string.["admitted","another kind","tags","address","not this world","malformed","version",
  "field","cell","bounds","session","sequence","time","entities","entity id","role","number",
  "near-zero quaternion","outside the world's bounds","stamp","status","object id","dropped: n not newer","table full"];
WHY :: [12]string.["counted","refused","untrusted referee","duplicate","revoked","no quest","rewritten quest",
  "key","outside the season","xp table","rule not run","conflict"];
XP_WORDS :: [21]string.["ok","kind","tags","marker","body","version","requires","type","field","id","season",
  "text","rule","award","policy","quest","signer","key","awardees","mismatch","reason"];

// One frame: read for the plaza, then ordered against what was seen.
admit_frame :: (name:string,tags:string,body:string,w:MvWorld,seen:[]MvSeen,count:int,now:int)->int {
  poses:[MV_ENTITIES]MvPose;
  f:=mv_frame_read(MV_FRAME,tags,body,MV_PLAZA,w,poses[..]);
  if f.code!=MvCode.ok {print("  %: refused, %, entry %\n",name,MV_WORDS[cast(int)f.code],f.at);return count;}
  o:=mv_order(seen,count,F1_PK,f.s,f.n,now);
  if o.code!=MvCode.ok {print("  %: s % n %: %\n",name,f.s,f.n,MV_WORDS[cast(int)o.code]);return o.count;}
  p:=poses[0];
  print("  %: s % n %: admitted, % at (%, %, %) in cell %,%\n",name,f.s,f.n,p.id,p.p[0],p.p[1],p.p[2],p.cx,p.cz);
  return o.count;
}
ledger :: (what:string,awards:[]XpAward,revs:[]XpRevocation) {
  trusted:=[1]string.[QUEST_PK];quests:=[1]XpQuest.[xp_quest_read(XP_QUEST,QUEST_ID,QUEST_PK,QUEST_TAGS,QUEST_BODY)];
  ruled:=[2]bool.[true,true];why:[2]XpWhy;out:[4]XpCredit;
  l:=xp_ledger(trusted[..],quests[..],awards,revs,ruled[0..<awards.count],why[..],out[..]);
  print("%\n",what);
  for i:0..<awards.count {print("  award %...: %\n",awards[i].event[0..<8],WHY[cast(int)why[i]]);}
  for i:0..<l.credits {k:[XP_TEXT]u8;print("  credit %...: % XP\n",kept_view(out[i].pubkey,k[..])[0..<8],out[i].xp);}
  if l.credits==0 {print("  no credit\n");}
}
```

<!-- source: nostr/examples/worlds-and-xp.coax main -->
```jai
main :: () {
  addr:[256]u8;
  w:=mv_world_read(WORLD_KIND,WORLD_PK,WORLD_TAGS,WORLD_BODY,addr[..]);
  print("world %\n  %: cell %, bounds (%, %, %) to (%, %, %)\n",cast(string)addr[0..<w.address],MV_WORDS[cast(int)w.code],
    w.cell,w.lo[0],w.lo[1],w.lo[2],w.hi[0],w.hi[1],w.hi[2]);
  print("frames from %... (clock in ms)\n",F1_PK[0..<8]);
  seen:[4]MvSeen;count:=0;
  count=admit_frame("F1",F1_TAGS,F1_BODY,w,seen[..],count,1790000001000);
  count=admit_frame("F2",F2_TAGS,F2_BODY,w,seen[..],count,1790000001050);
  count=admit_frame("F3",F3_TAGS,F3_BODY,w,seen[..],count,1790000001100);
  count=admit_frame("F4",F4_TAGS,F4_BODY,w,seen[..],count,1790000001200);
  last:=seen[0].ms;
  print("presence, last frame at %\n",last);
  print("  at 1790000011199: %\n",ifx mv_online(last,1790000011199) then "online" else "offline");
  print("  at 1790000011200: %\n",ifx mv_online(last,1790000011200) then "online" else "offline");
  s:=mv_state_read(STATE_KIND,STATE_TAGS,STATE_BODY,MV_PLAZA,w);
  print("  entity state: %, % at (%, %, %), online %\n",MV_WORDS[cast(int)s.code],s.id,s.p[0],s.p[1],s.p[2],s.online);
  o:[24]u8;
  print("object id of node %: %\n",LAMP_NODE,mv_object_id(LAMP_NODE,o[..]));

  q:=xp_quest_read(XP_QUEST,QUEST_ID,QUEST_PK,QUEST_TAGS,QUEST_BODY);
  print("quest %: %, season % to %, % % XP, % % XP\n",q.address,XP_WORDS[cast(int)q.code],q.opens,q.closes,q.roles[0],q.xp[0],q.roles[1],q.xp[1]);
  a1:=xp_award_read(XP_AWARD,AWARD1_ID,AWARD1_PK,AWARD1_TAGS,AWARD1_BODY);
  a2:=xp_award_read(XP_AWARD,AWARD2_ID,AWARD2_PK,AWARD2_TAGS,AWARD2_BODY);
  r1:=xp_revocation_read(XP_REVOCATION,REVOKE1_ID,REVOKE1_PK,REVOKE1_TAGS,REVOKE1_BODY);
  forged:=xp_revocation_read(XP_REVOCATION,FORGED_ID,FORGED_PK,FORGED_TAGS,FORGED_BODY);
  print("award %...: %, accepted %\naward %...: %, accepted %\n",a1.event[0..<8],XP_WORDS[cast(int)a1.code],a1.accepted,a2.event[0..<8],XP_WORDS[cast(int)a2.code],a2.accepted);
  print("revocation %... by %...: %\nrevocation %... by %...: %\n",r1.event[0..<8],REVOKE1_PK[0..<8],XP_WORDS[cast(int)r1.code],
    FORGED_ID[0..<8],FORGED_PK[0..<8],XP_WORDS[cast(int)forged.code]);
  one:=[1]XpAward.[a1];both:=[2]XpAward.[a1,a2];revs:=[2]XpRevocation.[r1,forged];
  ledger("ledger: the first award alone",one[..],revs[0..<0]);
  ledger("ledger: both awards, nothing revoked",both[..],revs[0..<0]);
  ledger("ledger: both awards, the first revoked (the forged revocation read and refused)",both[..],revs[..]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/worlds-and-xp.coax
```

Its output, in debug and release builds alike
([nostr/fixtures/worlds-and-xp.out](../fixtures/worlds-and-xp.out)):

```text
world 33300:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:plaza
  admitted: cell 64.0, bounds (-264.0, 0.0, -264.0) to (264.0, 200.0, 264.0)
frames from c6047f94... (clock in ms)
  F1: s a41c n 812: admitted, avatar at (3.2, 0.0, -10.5) in cell 0,-1
  F2: s a41c n 812: dropped: n not newer
  F3: refused, outside the world's bounds, entry 0
  F4: s a41c n 814: admitted, avatar at (-0.5, 0.0, -10.5) in cell -1,-1
presence, last frame at 1790000001200
  at 1790000011199: online
  at 1790000011200: offline
  entity state: admitted, avatar at (-0.5, 0.0, -10.5), online false
object id of node plaza/lamp-0: obj-558157d13a69707d2163
quest tb4.fix-git.beat-fable-low@1: ok, season 1790000000 to 1798000000, author 6 XP, runner 4 XP
award 5ff0138e...: ok, accepted 1790500000
award ca7d4532...: ok, accepted 1790700000
revocation 52393c28... by 79be667e...: ok
revocation 51267968... by c6047f94...: signer
ledger: the first award alone
  award 5ff0138e...: counted
  credit c6047f94...: 6 XP
  credit e493dbf1...: 4 XP
ledger: both awards, nothing revoked
  award 5ff0138e...: conflict
  award ca7d4532...: conflict
  no credit
ledger: both awards, the first revoked (the forged revocation read and refused)
  award 5ff0138e...: revoked
  award ca7d4532...: counted
  credit 5cbdf064...: 6 XP
  credit e493dbf1...: 4 XP
```

The broken inputs and their exact refusals are in that output: a replayed
frame dropped by sequence, a pose 36 meters past the plaza's edge refused,
two live awards on one key counted for no one, and a revocation signed by
a key that isn't the award's referee refused.

## How the code is checked

### Checks in the example

<!-- source: nostr/examples/checks/worlds-and-xp.coax check -->
```jai
// The event ids, from coax/tests/nostr/xp_mv_vectors.py: Python's json
// (compact separators) and hashlib.sha256 over NIP-01's
// [0,pubkey,created_at,kind,tags,content]. Coax serializes and hashes each
// whole event with event.coax and sha256.coax, which share no code with
// Python, and must reach the same 32 bytes.
#test "Coax computes each Python event id" #steps 400000000 {
  texts:=[11]string.[WORLD_JSON,F1_JSON,F2_JSON,F3_JSON,F4_JSON,STATE_JSON,QUEST_JSON,AWARD1_JSON,REVOKE1_JSON,AWARD2_JSON,FORGED_JSON];
  ids:=[11]string.[WORLD_ID,F1_ID,F2_ID,F3_ID,F4_ID,STATE_ID,QUEST_ID,AWARD1_ID,REVOKE1_ID,AWARD2_ID,FORGED_ID];
  scratch:[8192]u8;digits:="0123456789abcdef";
  for i:0..<texts.count {
    e:=nostr_event_read(texts[i],scratch[..],false);
    #assert e.error==NE_OK;
    for b:0..<32 {#assert ids[i][2*b]==digits[e.computed_id[b]>>4] && ids[i][2*b+1]==digits[e.computed_id[b]&15];}
  }
  // A replayed frame is the same event: one id, which a relay dedupes; the
  // ordering rule drops it even when it arrives on another connection.
  #assert F1_ID==F2_ID && QUEST_ID=="bd832fc966e9bf383e5d339fbe2f5ad53a57de1758be88aebae743dc7be71951";
  // The object id: Python's hashlib.sha256(b"plaza/lamp-0").hexdigest()[:20].
  o:[24]u8;#assert mv_object_id(LAMP_NODE,o[..])==LAMP_ID && LAMP_ID=="obj-558157d13a69707d2163";
}

// NIP-MV's own example contents, copied from the pinned NIP: the plaza
// world, the two-entity pose frame and the avatar's entity state. The NIP's
// examples carry no ids, pubkeys or signatures (placeholders), so there are
// no published ids to compare; their contents read as the NIP says.
#test "NIP-MV's example contents read as the NIP describes them" {
  a:[256]u8;w:=mv_world_read(MV_WORLD,MV_PK,MV_PLAZA_TAGS,MV_PLAZA_BODY,a[..]);
  #assert w.code==MvCode.ok && w.cell==64.0 && w.hi[1]==200.0;
  poses:[MV_ENTITIES]MvPose;
  f:=mv_frame_read(MV_FRAME,MV_FRAME_TAGS,MV_FRAME_BODY,MV_PLAZA,w,poses[..]);
  // The NIP's frame carries ["c","0,-1"]: both its entities are in that cell.
  #assert f.code==MvCode.ok && f.count==2 && poses[0].cx==0 && poses[0].cz== -1 && poses[1].cx==0 && poses[1].cz== -1;
  s:=mv_state_read(MV_STATE,MV_STATE_TAGS,MV_STATE_BODY,MV_PLAZA,w);
  #assert s.code==MvCode.ok && s.online && s.cx==0 && s.cz== -1;
}

// The worked example's values, by hand: F3's x of 300 is past the plaza's
// max of 264; F4's x of -0.5 floors to cell -1; the revocation leaves the
// replacement alone on its key, so key 7 has 6 XP and key 4 has 4.
#test "the hand-worked values" {
  #assert mv_cell(300.0,64.0)==4 && mv_cell(-0.5,64.0)== -1 && mv_cell(-10.5,64.0)== -1 && mv_cell(3.2,64.0)==0;
  trusted:=[1]string.[QUEST_PK];quests:=[1]XpQuest.[xp_quest_read(XP_QUEST,QUEST_ID,QUEST_PK,QUEST_TAGS,QUEST_BODY)];
  both:=[2]XpAward.[xp_award_read(XP_AWARD,AWARD1_ID,AWARD1_PK,AWARD1_TAGS,AWARD1_BODY),xp_award_read(XP_AWARD,AWARD2_ID,AWARD2_PK,AWARD2_TAGS,AWARD2_BODY)];
  revs:=[1]XpRevocation.[xp_revocation_read(XP_REVOCATION,REVOKE1_ID,REVOKE1_PK,REVOKE1_TAGS,REVOKE1_BODY)];
  ruled:=[2]bool.[true,true];why:[2]XpWhy;out:[4]XpCredit;
  l:=xp_ledger(trusted[..],quests[..],both[..],revs[..],ruled[..],why[..],out[..]);
  #assert l.counted==1 && l.revoked==1 && l.credits==2;
  #assert kept_is(out[0].pubkey,"5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc") && out[0].xp==6;
  #assert kept_is(out[1].pubkey,"e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13") && out[1].xp==4;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/worlds-and-xp.coax
```

`coax test` runs 234 checks, including the loaded files' own checks; all
pass.

The independent checks are two. Python's `json` and `hashlib` compute
every event's NIP-01 id and the object id, sharing no code with
`event.coax` or `sha256.coax`, and Coax reaches the same bytes. And
NIP-MV's own example contents (the plaza, the two-entity frame and the
avatar's state) read as the NIP describes them, including the `0,-1` cell
its frame's `c` tag names. NIP-MV and NIP-XP publish no signed examples
with event ids (their examples use placeholders), so no published id could
be compared or found wrong.

To regenerate the vectors:

```shell
python3 coax/tests/nostr/xp_mv_vectors.py > coax/tests/nostr/xp_mv_vectors.coax
```

### The library's own checks

`nipmv.coax` checks the NIP's three example contents, each world, frame,
entry and state refusal, each bound on each axis and side (on a bound is
inside), coordinates at `10^9`, quaternion lengths at exactly `2^-20`,
cells either side of zero, 16 and 17 entries, session ids of 16 and 17
bytes, statuses of 64 and 65 bytes, the ordering table's sessions and its
full case, and object ids. `xp_ledger.coax` checks every quest, award and
revocation refusal, the ID and slug grammars at their edges, titles and
objectives of 200/201 and 4,000/4,001 characters, the award sum at 1 and
1,000, and every ledger outcome: untrusted, refused, duplicate, revoked,
no quest, rewritten, key, season edges, XP table, rule not run, conflicts
and sums across awards.

```shell
coax/bin/coax test coax/lib/nostr/nipmv.coax
coax/bin/coax mutate coax/lib/nostr/nipmv.coax --root-only
coax/bin/coax test coax/lib/nostr/xp_ledger.coax
coax/bin/coax mutate coax/lib/nostr/xp_ledger.coax --root-only
```

On 2026-11-01, `nipmv.coax` passed 92 checks (with its loaded files') and
`coax mutate --root-only` killed all 694 mutants; `xp_ledger.coax` passed
60 and killed all 933. None survived and none is marked equivalent. The
full mutation run (`COAX_MUTATION_FULL=1`) and cross-target runs were not
done. On 2026-11-03, after each credit began keeping its own copy of the
key (#12692), `xp_ledger.coax` passed 61 checks and `coax mutate
--root-only` killed all 977 mutants, none marked equivalent. When the copy
moved to the standard library's `Kept(8)` (#12696), `xp_ledger.coax` passed
67 checks (61 and `kept.coax`'s 6) and `coax mutate --root-only` killed all
941 mutants, none marked equivalent; `kept.coax`'s own run killed all 173.

## Summary

- A NIP-MV world names its own address, cell size and bounds; every event
  in it carries exactly one `w` tag.
- `mv_frame_read` admits a pose frame only for its world, with finite
  numbers, a non-zero quaternion, a stamp exactly on bodies, and every
  position inside the world's bounds.
- A receiver orders frames by session and sequence, never by
  `created_at`, and decides presence against a clock it is given.
- An object's entity id is fixed by the SHA-256 of its world-tree node.
- An XP award counts only when it binds to its exact, unrewritten quest,
  matches its key, season and XP table, passed its rule, and is alone on
  its key; a revoked award is removed and frees the key.

## Dependencies

- [61. Live activities](ch061-live-activities.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)

## Sources

- [OpenAgents NIP-LIVE](../nips/openagents/NIP-LIVE.md)
- [OpenAgents NIP-MV](../nips/openagents/NIP-MV.md)
- [OpenAgents NIP-XP](../nips/openagents/NIP-XP.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
