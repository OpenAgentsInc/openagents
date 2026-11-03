# 76. Work, coordination and control

Work outlives a conversation. Four OpenAgents NIPs describe it. NIP-WORK
tracks the objective and who is accountable for it. NIP-COORD decides which
worker may change which resource, and for how long. NIP-CTRL lets another
client, such as a phone, steer or cancel a task without copying the
owner's authority. NIP-CTX records the exact task frame and context that a
run saw. None of them adds an event kind: their artifacts travel inside
kind 3188 private envelopes or CJ execution jobs, and the host's own record
decides. This chapter builds the two decisions that must never be wrong: a
coordinator's claim, with the fencing token that stops an old worker's
writes, and a controller's admission of a client's command.

Part VI, Every NIP. Result: NIP-COORD atomic claims, lease renewal, release
and cancel, and the dispatcher's epoch and token fence; NIP-CTRL pairing,
revocation and ordered command admission with the SHA-256 JCS command
fingerprint. NIP-WORK items and revisions and NIP-CTX frames and context
receipts are described here and not built; see
[What is not built](#what-is-not-built).

## What this chapter teaches

- What each of the four NIPs owns, and why a relay timestamp, an
  addressable head or a model's claim never decides any of it.
- A COORD claim: compare the expected revision, check the lease against the
  coordinator's own clock, grant every resource or none, and take the next
  fencing token.
- Which claims share a resource: reads share, a read of an immutable
  snapshot shares even with a write, live reads and writes do not.
- Why a lease's expiry stops new dispatch but frees nothing, and how a
  token fences the old worker after someone else takes over.
- CTRL pairing: a challenge-bound response from the invited key, and the
  owner's admission, consume one invitation and create one grant.
- CTRL command admission in its order, the idempotency key and its
  fingerprint, and why a duplicate is checked before the frame revision.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [claim.coax](../../coax/lib/nostr/coord/claim.coax) | `coord_claim(s, claims, holds, q, want, now)` | The coordinator's atomic claim: revision compare, lease bounds, all resources or none, the next token |
| [claim.coax](../../coax/lib/nostr/coord/claim.coax) | `coord_control(s, claims, holds, token, action, actor, expected_revision, requested_until, now)` | Renew, release or cancel one claim |
| [claim.coax](../../coax/lib/nostr/coord/claim.coax) | `coord_fence(s, claims, holds, worker, resource, epoch, token, now)` | The dispatcher's check before one write |
| [command.coax](../../coax/lib/nostr/ctrl/command.coax) | `ctrl_pair(invitations, grants, count, p, current, owner_admitted, grant, until, now)` | Consumes an invitation and creates a grant |
| [command.coax](../../coax/lib/nostr/ctrl/command.coax) | `ctrl_revoke(grants, grant, actor, owner)` | Moves a grant's epoch from 0 to 1 |
| [command.coax](../../coax/lib/nostr/ctrl/command.coax) | `ctrl_command_admit(grants, retained, c, current, revision, now)` | The controller's decision on one steer or cancel command |
| [fingerprint.coax](../../coax/lib/nostr/ctrl/fingerprint.coax) | `ctrl_fingerprint(text, canon, out)` | The `sha256:` Digest of a command's JCS form |

All three files are new with this chapter. `claim.coax` and `command.coax`
decide admission, so both are `#authority` files: their checks came first,
and a mutation run kills every mutant. They take the verified signer, the
resolved resource IDs and the host's record as plain fields and load no
cryptography, so their checks run in milliseconds. `fingerprint.coax`
reuses `jcs.coax` for the canonical form and `envelope.coax`'s `ae_digest`,
the procedure that already names an artifact's bytes, for the Digest.

## Four NIPs, four owners

The NIPs split one piece of work among four records. Each one owns a part
and refuses to stand in for the others.

| NIP | Owns | Does not own |
| --- | --- | --- |
| [NIP-WORK](../nips/openagents/NIP-WORK.md) | The tracked objective, its accountable principal, planning relations, evidence links and the disposition (accepted, rejected, cancelled) | Execution, resource locks, payment |
| [NIP-COORD](../nips/openagents/NIP-COORD.md) | Execution tasks, dependencies, resource claims, fencing tokens and leases, findings and proposed integration | Who is accountable, what the user asked |
| [NIP-CTRL](../nips/openagents/NIP-CTRL.md) | Which extra clients may observe, steer or cancel one task, and the receipts of their commands | Approval, spending, a resource claim |
| [NIP-CTX](../nips/openagents/NIP-CTX.md) | The task frame (objective, constraints, corrections), source snapshots and context-selection receipts | The run's journal (that is NIP-RUN) |

Three rules hold across all four:

- **The authority decides by its own record.** One admitted coordinator
  owns a COORD scope; one controller owns a task for CTRL; one host owns a
  WORK scope. A signed artifact is a proposal until that authority admits
  it.
- **Revisions and clocks are the authority's.** A claim's lease, a grant's
  expiry and a command's window are checked against the authority's clock.
  Published `created_at` values never extend anything.
- **Idempotency by key and fingerprint.** A retried request with the same
  key and the same bytes gets the retained result and causes no second
  effect. The same key with different bytes refuses. The fingerprint is
  SHA-256 of the JCS form of the entire request.

## Claims and fencing

NIP-COORD's coordinator keeps a durable record: a scope revision that
moves on every accepted change, an epoch for the controller generation,
and a claim-token counter that only grows. Restart cannot reset any of
them. The record in `claim.coax`:

<!-- source: coax/lib/nostr/coord/claim.coax record -->
```jai
// The record keeps its own copy of each claim's proposal and worker and each
// hold's resource and snapshot. A string argument only borrows the caller's
// bytes, and a coordinator may read the next request into the same buffer;
// a stored borrow would then move a hold to another resource or a claim to
// another worker after the claim was checked. CO_TEXT bytes hold a 64-hex
// key, a "sha256:" digest or a resource ID, packed eight to a word, the
// first byte lowest; a longer string refuses as malformed.
CO_TEXT :: 128;
CoKept :: struct {n:int;w:[CO_TEXT/8]u64;}
CoordState :: struct {owner:string;epoch:int;token:int;revision:int;max_lease:int;claims:int;holds:int;}
CoordClaim :: struct {token:int;proposal:CoKept;worker:CoKept;expires_at:int;state:int;}
CoordHold :: struct {token:int;resource:CoKept;write:bool;snapshot:CoKept;released:bool;}
CoordWant :: struct {resource:string;write:bool;snapshot:string;}
CoordRequest :: struct {proposal:string;worker:string;expected_revision:int;requested_until:int;}
```

A request arrives as strings. The claims and holds that outlive it keep
their own copies, `CoKept`, of up to 128 bytes. A string only borrows the
caller's bytes: without the copy, a coordinator that reads each request into
one buffer would move an existing hold onto whatever resource the next
request names, and free the one it was granted. A longer proposal, worker,
resource or snapshot refuses as `CO_MALFORMED` before anything is written.

<!-- source: coax/lib/nostr/coord/claim.coax kept -->
```jai
// Copy at most CO_TEXT bytes; coord_claim refuses anything longer first.
co_kept :: (s:string)->CoKept {
  t:CoKept;
  for i:0..<s.count {t.w[i/8]=t.w[i/8]|(cast(u64)s[i]<<((i%8)*8));}
  t.n=s.count;
  return t;
}
// A kept text is never longer than CO_TEXT, so a longer s differs by count.
co_is :: (t:CoKept,s:string)->bool {
  if t.n!=s.count {return false;}
  u:=co_kept(s);
  for j:0..<CO_TEXT/8 {if t.w[j]!=u.w[j] {return false;}}
  return true;
}
// Write the kept bytes to out for a caller to read: the count, or -1 when
// out is too small.
co_bytes :: (t:CoKept,out:[]u8)->int {
  if out.count<t.n {return -1;}
  for i:0..<t.n {out[i]=cast(u8)((t.w[i/8]>>((i%8)*8))&255);}
  return t.n;
}
```

A claim request (`openagents.claim-request.v1`) names a proposal, the
worker's key, its resources as `{id, mode, snapshot}`, the revision it
expects and the lease end it wants. The host namespace resolves aliases
first (symlinks, case, account aliases and collection-wide locks), so two
names for one resource arrive as one ID. A worker's claim that its writes
are independent cannot override overlap.

The outcomes, and the NIP refusal code each one reports:

<!-- source: coax/lib/nostr/coord/claim.coax codes -->
```jai
CO_OK :: cast(u32)0;           // accepted
CO_MALFORMED :: cast(u32)1;    // no resources, a resource named twice, or a lease that ends by now
CO_LIMIT :: cast(u32)2;        // a lease past the maximum, or no room left in the record
CO_STALE :: cast(u32)3;        // expected revision is not current; or an old epoch or token at the fence
CO_CONFLICT :: cast(u32)4;     // a resource is held by a claim that has not been released
CO_NOT_ADMITTED :: cast(u32)5; // no such claim, not its worker (or the owner), no write hold
CO_EXPIRED :: cast(u32)6;      // the lease has ended by the coordinator's clock
CO_CLOSED :: cast(u32)7;       // the claim was released or cancelled

CO_ACTIVE :: 0; CO_CANCELLED :: 1; CO_RELEASED :: 2;  // a claim's state
CO_RENEW :: 0; CO_RELEASE :: 1; CO_CANCEL :: 2;       // control actions
CO_MAX_RESOURCES :: 64;                               // the schema's array bound
```

`CO_LIMIT` reports `limit_exceeded`, `CO_STALE` and `CO_EXPIRED` report
`stale`, and `CO_CONFLICT` reports `conflict`. The CJ result that carries
any of them is still `completed`: the operation answered, the task did not
succeed.

### Which modes share

NIP-COORD gives three rules: immutable snapshot reads can share,
unsnapshotted live reads conflict with writes, and multiple writes to one
resource conflict. Written out for one resource:

| Held \ asked | live read | snapshot read | write |
| --- | --- | --- | --- |
| live read | share | share | conflict |
| snapshot read | share | share | share |
| write | conflict | share | conflict |

<!-- source: coax/lib/nostr/coord/claim.coax conflict -->
```jai
// Two entries on one resource conflict unless both read, or the reader holds
// an immutable snapshot.
coord_overlap :: (resource:string,write:bool,snapshot:string,h:CoordHold)->bool {
  if !co_is(h.resource,resource) {return false;}
  if write && h.write {return true;}
  if write {return h.snapshot.n==0;}
  return h.write && snapshot=="";
}
```

### The atomic claim

The coordinator "atomically compares expected revision ... and grants all
resources or none". `coord_claim` checks the request, the revision and
every resource before it writes anything:

<!-- source: coax/lib/nostr/coord/claim.coax claim -->
```jai
coord_claim :: (s:[]CoordState,claims:[]CoordClaim,holds:[]CoordHold,q:CoordRequest,want:[]CoordWant,now:int)->u32 {
  st:=s[0];
  // 1. The request: some resources, each once, and a lease that ends after now.
  if want.count==0 {return CO_MALFORMED;}
  if want.count>CO_MAX_RESOURCES {return CO_LIMIT;}
  for i:0..<want.count {for j:i+1..<want.count {if want[i].resource==want[j].resource {return CO_MALFORMED;}}}
  if q.proposal.count>CO_TEXT || q.worker.count>CO_TEXT {return CO_MALFORMED;}
  for w:want {if w.resource.count>CO_TEXT || w.snapshot.count>CO_TEXT {return CO_MALFORMED;}}
  if q.requested_until<=now {return CO_MALFORMED;}
  if q.requested_until>now+st.max_lease {return CO_LIMIT;}
  // 2. The compare: the revision the worker saw is still current.
  if q.expected_revision!=st.revision {return CO_STALE;}
  // 3. Every resource free of a claim not yet released, expired or not.
  for w:want {
    for k:0..<st.holds {
      if !holds[k].released && coord_overlap(w.resource,w.write,w.snapshot,holds[k]) {return CO_CONFLICT;}
    }
  }
  if st.claims>=claims.count || st.holds+want.count>holds.count {return CO_LIMIT;}
  // 4. Grant all of it at once: the next token, the next revision.
  t:=st.token+1;
  claims[st.claims]=CoordClaim.{token=t,proposal=co_kept(q.proposal),worker=co_kept(q.worker),expires_at=q.requested_until,state=CO_ACTIVE};
  for i:0..<want.count {
    holds[st.holds+i]=CoordHold.{token=t,resource=co_kept(want[i].resource),write=want[i].write,snapshot=co_kept(want[i].snapshot)};
  }
  s[0].token=t;s[0].revision=st.revision+1;s[0].claims=st.claims+1;s[0].holds=st.holds+want.count;
  return CO_OK;
}
```

Two workers that both read revision 10 and both ask for the same write
cannot both win. The first accepted claim moves the revision to 11, so the
second is `stale` before its resources are even looked at. If the second
worker reads revision 11 and tries again, the held resource makes it a
`conflict`. Either check alone would stop it; together they also stop a
worker that claims a different resource set based on old state.

### Leases: renew, release and cancel

A control (`openagents.coordination-control.v1`) names the claim, an
action and the expected revision. NIP-COORD fixes who may send each one:
the named worker or the owner may renew or release, and only the owner (or
a separately admitted controller) may cancel.

<!-- source: coax/lib/nostr/coord/claim.coax control -->
```jai
// Renew moves the lease end to requested_until; release frees the holds
// (the caller has fenced the old dispatchers); cancel, by the owner only,
// stops dispatch and keeps the holds until a release.
coord_control :: (s:[]CoordState,claims:[]CoordClaim,holds:[]CoordHold,token:int,action:int,actor:string,expected_revision:int,requested_until:int,now:int)->u32 {
  st:=s[0];
  k:=coord_find(claims,st.claims,token);
  if k<0 {return CO_NOT_ADMITTED;}
  c:=claims[k];
  if action==CO_CANCEL {if actor!=st.owner {return CO_NOT_ADMITTED;}}
  else if action==CO_RENEW || action==CO_RELEASE {if !co_is(c.worker,actor) && actor!=st.owner {return CO_NOT_ADMITTED;}}
  else {return CO_MALFORMED;}
  if expected_revision!=st.revision {return CO_STALE;}
  if c.state==CO_RELEASED || (action!=CO_RELEASE && c.state==CO_CANCELLED) {return CO_CLOSED;}
  if action==CO_RENEW {
    if now>=c.expires_at {return CO_EXPIRED;}
    if requested_until<=now {return CO_MALFORMED;}
    if requested_until>now+st.max_lease {return CO_LIMIT;}
    claims[k].expires_at=requested_until;
  } else if action==CO_RELEASE {
    claims[k].state=CO_RELEASED;
    for i:0..<st.holds {if holds[i].token==token {holds[i].released=true;}}
  }
  else {claims[k].state=CO_CANCELLED;}
  s[0].revision=st.revision+1;
  return CO_OK;
}
```

The important line is the one that is missing: nothing frees a resource
when its lease ends. NIP-COORD says lease expiry "stops new dispatch but
does not prove old code stopped; it never automatically frees unknown
reservations". A worker whose lease ran out may still be running, on a
machine the coordinator cannot reach. So an expired claim keeps
conflicting until a release, which acknowledges that the old dispatchers
are fenced and earlier effects are reconciled. A renewal that arrives after
the lease ended is `expired`: a lost renewal does not come back.

### The fence

The coordinator can grant a claim; only the dispatcher that performs the
write can refuse an old one. NIP-COORD: "A dispatcher accepts work only
from the named worker, for the exact task/resource grant, and under the
latest token registered in its trusted coordinator connection."

<!-- source: coax/lib/nostr/coord/claim.coax fence -->
```jai
// The dispatcher's check before one write on resource under (epoch, token).
coord_fence :: (s:[]CoordState,claims:[]CoordClaim,holds:[]CoordHold,worker:string,resource:string,epoch:int,token:int,now:int)->u32 {
  st:=s[0];
  k:=coord_find(claims,st.claims,token);
  if k<0 || !co_is(claims[k].worker,worker) {return CO_NOT_ADMITTED;}
  held:=false;newer:=false;
  for i:0..<st.holds {
    if co_is(holds[i].resource,resource) && holds[i].write {
      if holds[i].token==token {held=true;}
      if holds[i].token>token {newer=true;}
    }
  }
  if !held {return CO_NOT_ADMITTED;}
  if epoch!=st.epoch || newer {return CO_STALE;}
  if claims[k].state!=CO_ACTIVE {return CO_CLOSED;}
  if now>=claims[k].expires_at {return CO_EXPIRED;}
  return CO_OK;
}
```

A forwarded signed claim is not transferable authority, so the fence first
asks whether this worker holds this token. Then a newer write token on the
same resource fences the old one as `stale`, whatever its own lease says.
The order matters: a worker whose claim was released and replaced learns
that it is behind (`stale`), not just that its claim is closed.

### Worked example: one task's lease, by hand

The coordinator is at epoch 2 and revision 10, with a 600-second longest
lease. T0 = 1790000000. Worker 3 asks for `doc` (write) and `spec` (a read
of snapshot `5555…`); worker 7 asks for `doc` (write).

| Now | Request | Step that decides | Result | Record after |
| --- | --- | --- | --- | --- |
| T0 | worker 3 claims, expects 10, until T0 + 300 | 300 > 0, 300 ≤ 0 + 600; 10 = 10; no holds | accepted | token 1, revision 11 |
| T0 | worker 7 claims `doc`, expects 10 | 10 ≠ 11 | stale | unchanged |
| T0 | worker 7 claims `doc`, expects 11 | token 1 holds `doc` for writing | conflict | unchanged |
| T0 + 100 | worker 3 writes `doc`, epoch 2, token 1 | its claim, its write hold, epoch 2, no newer token, active, 100 < 300 | accepted | |
| T0 + 100 | worker 7 writes `doc`, token 1 | token 1 is worker 3's | not_admitted | |
| T0 + 250 | worker 3 renews to T0 + 700, expects 11 | 250 < 300; 700 ≤ 250 + 600 = 850 | accepted | revision 12 |
| T0 + 250 | worker 3 renews to T0 + 1000, expects 12 | 1000 > 850 | limit_exceeded | |
| T0 + 700 | worker 3 writes `doc`, token 1 | 700 ≥ 700, the lease end | expired | |
| T0 + 710 | worker 7 claims `doc`, expects 12 | token 1 is not released | conflict | |
| T0 + 710 | worker 3 renews | 710 ≥ 700 | expired | |
| T0 + 715 | worker 7 cancels token 1 | cancel is the owner's | not_admitted | |
| T0 + 720 | the owner releases token 1, expects 12 | the owner; 12 = 12; active | accepted | revision 13 |
| T0 + 720 | worker 7 claims `doc`, expects 13, until T0 + 1020 | 1020 ≤ 720 + 600; 13 = 13; token 1's holds released | accepted | token 2, revision 14 |
| T0 + 730 | worker 3 writes `doc`, token 1 | token 2 > 1 holds `doc` for writing | stale | |
| T0 + 730 | worker 7 writes `doc`, token 2 | all pass; 730 < 1020 | accepted | |
| T0 + 900 | worker 7 releases token 2, expects 14 | its own claim | accepted | revision 15 |

The record ends with two claims, three holds (`doc` and `spec` for token
1, `doc` for token 2), last token 2 and revision 15: five accepted changes.
The write at T0 + 730 is the one fencing exists for. Worker 3's process
woke up after a pause, its own clock said nothing was wrong, and its token
1 was refused because token 2 now owns the document.

## Task control

NIP-CTRL lets a terminal, phone or browser control one task without
becoming its owner. Three keys take part: the **owner**, whose authority
establishes the task; the **authority**, the current controller that
serializes task state (here the host, key 1); and a **client**, a distinct
key admitted to some rights. The rights are exactly `observe`, `steer` and
`cancel`, and none implies another. There is no general `approve` right:
approving a spend or a publication is NIP-POL's, even for a client with all
three.

Every grant is scoped to `{task, controller, generation}`. When a task is
handed to another controller generation, old grants stop working: grant
bytes do not follow a task to a new machine.

<!-- source: coax/lib/nostr/ctrl/command.coax record -->
```jai
CtrlScope :: struct {task:string;controller:string;generation:int;}
CtrlInvitation :: struct {invitation:string;challenge:string;client:string;scope:CtrlScope;rights:int;expires_at:int;pairing:string;}
CtrlPairing :: struct {invitation:string;signer:string;client:string;challenge:string;rights:int;accepted:bool;fingerprint:string;}
CtrlGrant :: struct {grant:string;client:string;scope:CtrlScope;rights:int;epoch:int;issued_at:int;expires_at:int;}
CtrlCommand :: struct {command:string;fingerprint:string;signer:string;grant:string;epoch:int;scope:CtrlScope;expected_revision:int;issued_at:int;expires_at:int;action:int;}
CtrlRetained :: struct {grant:string;command:string;fingerprint:string;}
```

<!-- source: coax/lib/nostr/ctrl/command.coax codes -->
```jai
CT_ACCEPTED :: cast(u32)0;
CT_DUPLICATE :: cast(u32)1;     // an identical repeat: the retained result, no new effect
CT_NOT_ADMITTED :: cast(u32)2;  // no grant for this key (unpaired), a right it lacks, no owner admission
CT_REVOKED :: cast(u32)3;
CT_EXPIRED :: cast(u32)4;       // the grant, invitation or command has expired by the authority's clock
CT_STALE :: cast(u32)5;         // a scope or epoch that is not the grant's or the current one
CT_MALFORMED :: cast(u32)6;     // a bad window or rights set
CT_CONFLICT :: cast(u32)7;      // the expected frame revision is not current; or a second pairing
CT_IDEMPOTENCY :: cast(u32)8;   // different bytes under a retained command ID
CT_CLOSED :: cast(u32)9;        // the client declined: the invitation is consumed, no grant

CT_OBSERVE :: 0; CT_STEER :: 1; CT_CANCEL :: 2;  // rights as bits
CT_ALL_RIGHTS :: 7;
CT_WINDOW :: 300;  // this host's longest command window, seconds
CT_SKEW :: 60;     // how far ahead of the authority's clock a command may be dated
```

NIP-CTRL leaves the command window to "host limits". This host allows 300
seconds and 60 seconds of skew; another host may choose smaller values.

### Pairing

Pairing starts with an independently authenticated introduction, such as
the owner approving a displayed client key at a trusted desktop. The
authority then seals an invitation (`openagents.control-invitation.v1`) to
that exact client key with a random challenge and the proposed rights. The
client answers with a signed pairing response that repeats the challenge
and names a nonempty subset of the rights.

Proof that the client holds its key is necessary but not enough: the host
also needs the owner's admission of this pairing. Then it consumes the
invitation and persists the grant atomically, before it answers.

<!-- source: coax/lib/nostr/ctrl/command.coax pair -->
```jai
// Consumes the invitation a pairing response answers. On acceptance the new
// grant goes to grants[count] and count grows by one.
ctrl_pair :: (invitations:[]CtrlInvitation,grants:[]CtrlGrant,count:[]int,p:CtrlPairing,current:CtrlScope,owner_admitted:bool,grant:string,until:int,now:int)->u32 {
  k:=-1;
  for i:0..<invitations.count {if invitations[i].invitation==p.invitation {k=i;}}
  if k<0 {return CT_NOT_ADMITTED;}
  v:=invitations[k];
  // Used: the same response gets the same answer; another one gets nothing.
  if v.pairing!="" {return ifx v.pairing==p.fingerprint then CT_DUPLICATE else CT_CONFLICT;}
  if p.signer!=p.client || p.client!=v.client || p.challenge!=v.challenge {return CT_NOT_ADMITTED;}
  if now>=v.expires_at {return CT_EXPIRED;}
  if !ctrl_same(v.scope,current) {return CT_STALE;}
  if !p.accepted {invitations[k].pairing=p.fingerprint;return CT_CLOSED;}
  if p.rights==0 || (p.rights|v.rights)!=v.rights {return CT_MALFORMED;}
  if !owner_admitted {return CT_NOT_ADMITTED;}
  if until<=now || count[0]>=grants.count {return CT_MALFORMED;}
  grants[count[0]]=CtrlGrant.{grant=grant,client=p.client,scope=v.scope,rights=p.rights,epoch=0,issued_at=now,expires_at=until};
  count[0]+=1;
  invitations[k].pairing=p.fingerprint;
  return CT_ACCEPTED;
}
```

A relayed or copied response cannot substitute another key: the signer
must be the client the response names, and that must be the client the
invitation names. `accepted: false` closes the invitation without a grant.
A second, different response to a used invitation is a `conflict` and
creates nothing.

### Revocation

Only the owner (or a separately admitted access administrator, not built
here) may revoke. Revocation moves the grant's epoch from 0 to 1, and 1 is
terminal for that grant ID. Re-enrolling creates a new grant ID.

<!-- source: coax/lib/nostr/ctrl/command.coax revoke -->
```jai
ctrl_revoke :: (grants:[]CtrlGrant,grant:string,actor:string,owner:string)->u32 {
  if actor!=owner {return CT_NOT_ADMITTED;}
  for i:0..<grants.count {
    if grants[i].grant==grant {
      if grants[i].epoch==1 {return CT_DUPLICATE;}
      grants[i].epoch=1;return CT_ACCEPTED;
    }
  }
  return CT_NOT_ADMITTED;
}
```

### Commands

A command (`openagents.task-command.v1`) names its own random ID, the
grant and its epoch, the scope, the CTX frame revision it expects, a short
window and the action, `steer` or `cancel`. The authority checks it in
this order:

1. The grant exists and its client is the command's signer. A key that was
   never paired stops here.
2. The grant is not revoked, not expired, and the command's epoch and
   scope equal the grant's, which equal the controller's current scope.
3. The action is `steer` or `cancel`, and the grant holds that right.
4. The command's window is at most 300 seconds, inside the grant, not
   dated more than 60 seconds ahead, and not over.
5. Idempotency: the key is `(authority, grant ID, command ID)`, and the
   fingerprint is SHA-256 of the JCS form of the entire command. The same
   key with the same fingerprint is a duplicate; with another fingerprint
   it is `idempotency_conflict`.
6. The expected frame revision equals the current one. A stale revision is
   a `conflict` and changes nothing. No last-writer-wins rule applies.

<!-- source: coax/lib/nostr/ctrl/command.coax admit -->
```jai
ctrl_command_admit :: (grants:[]CtrlGrant,retained:[]CtrlRetained,c:CtrlCommand,current:CtrlScope,revision:int,now:int)->u32 {
  // 1. The grant, and the key it was paired with.
  k:=-1;
  for i:0..<grants.count {if grants[i].grant==c.grant {k=i;}}
  if k<0 || grants[k].client!=c.signer {return CT_NOT_ADMITTED;}
  g:=grants[k];
  // 2. Revocation, expiry, then the epoch and scope of grant, command and controller.
  if g.epoch==1 {return CT_REVOKED;}
  if now>=g.expires_at {return CT_EXPIRED;}
  if c.epoch!=g.epoch || !ctrl_same(c.scope,g.scope) || !ctrl_same(g.scope,current) {return CT_STALE;}
  // 3. The right for this action: steer or cancel.
  if c.action!=CT_STEER && c.action!=CT_CANCEL {return CT_MALFORMED;}
  if (g.rights>>c.action)&1==0 {return CT_NOT_ADMITTED;}
  // 4. The command's window, inside the grant.
  if c.expires_at<=c.issued_at || c.expires_at-c.issued_at>CT_WINDOW {return CT_MALFORMED;}
  if c.issued_at<g.issued_at || c.expires_at>g.expires_at || c.issued_at>now+CT_SKEW {return CT_MALFORMED;}
  if now>c.expires_at {return CT_EXPIRED;}
  // 5. Idempotency by (grant, command ID), then the frame revision.
  for r:retained {
    if r.grant==c.grant && r.command==c.command {return ifx r.fingerprint==c.fingerprint then CT_DUPLICATE else CT_IDEMPOTENCY;}
  }
  if c.expected_revision!=revision {return CT_CONFLICT;}
  return CT_ACCEPTED;
}
```

Idempotency comes before the revision for a reason. An accepted steer
creates the next frame, so the frame revision moves. If the client loses
the acknowledgement and retransmits, its command still expects the old
revision. Checked in the other order, the retry would be a `conflict` and
the client could not tell whether its command was applied. Checked in this
order, it is a duplicate and gets the retained receipt. Revocation still
comes first: a retransmission after revocation is refused, and the
retained result is never read.

An accepted command means the authority recorded `correction_recorded` or
`cancel_requested`. It does not mean the task changed course or stopped.
That needs RUN evidence.

### The fingerprint

<!-- source: coax/lib/nostr/ctrl/fingerprint.coax print -->
```jai
CF_OK :: cast(u32)0;
CF_JSON :: cast(u32)1;  // not canonical JSON: jcs.coax refused it
CF_ROOM :: cast(u32)2;  // out is not exactly 71 bytes
CF_DIGEST :: 71;        // "sha256:" + 64 hex digits

CtrlPrint :: struct {error:u32;canonical:int;}

// Writes the Digest of text's JCS form to out, 71 bytes; canon is the work area.
ctrl_fingerprint :: (text:string,canon:[]u8,out:[]u8)->CtrlPrint {
  j:=jcs_canonicalize_checked(text,canon,false);
  if j.error!=JCS_OK {return CtrlPrint.{error=CF_JSON};}
  if ae_digest(canon[0..<j.written],out)!=AE_OK {return CtrlPrint.{error=CF_ROOM};}
  return CtrlPrint.{error=CF_OK,canonical=j.written};
}
```

JCS (RFC 8785) sorts object keys and removes insignificant whitespace, so
a client that serializes keys in another order sends the same command.
`jcs.coax` refuses duplicate keys: a command with two `command` members
has no fingerprint and no admission.

### Worked example: pairing and commands, by hand

The task is `4444…`, the controller is key 1 at generation 3, and the
frame is at revision 4. The invitation `1111…` with challenge `2222…`
offers client 3 `observe` and `cancel` (bits 0 and 2, so 5) until T0 + 600.

| Now | Input | Step that decides | Result |
| --- | --- | --- | --- |
| T0 + 100 | client 7 sends a cancel command naming grant `3333…` | no grant `3333…` exists | not_admitted |
| T0 + 60 | client 3's response, signed by client 7 | signer 7 ≠ client 3 | not_admitted |
| T0 + 60 | client 3's response with challenge `1111…` | ≠ `2222…` | not_admitted |
| T0 + 60 | client 3 asks for cancel and steer (6) | 6 \| 5 = 7 ≠ 5 | malformed |
| T0 + 60 | client 3 asks for cancel (4), no owner admission | admission missing | not_admitted |
| T0 + 60 | the same, admitted | 4 \| 5 = 5; admitted; T0 + 86400 > T0 + 60 | accepted: grant `3333…`, rights 4, epoch 0 |
| T0 + 70 | the same response again | invitation used by this fingerprint | duplicate |

The cancel command `aaaa…` expects revision 4 and runs from T0 + 100 to
T0 + 400. Its JCS form is 612 bytes; the wire form has a space after each
of 19 colons and 15 commas, so 646 bytes. Its fingerprint is
`sha256:71fbe0a6…2c4636`.

| Now | Input | Step that decides | Result |
| --- | --- | --- | --- |
| T0 + 120 | client 7 sends it | grant's client is 3 | not_admitted |
| T0 + 120 | client 3 sends it | grant found, epoch 0, 120 < 86400, scope equal; bit 2 of 4 is 1; 400 − 100 = 300 ≤ 300, 100 ≥ 60, 400 ≤ 86400, 100 ≤ 120 + 60, 120 ≤ 400; nothing retained; 4 = 4 | accepted |
| T0 + 130 | the same command, frame now 5 | retained, same fingerprint (before 4 ≠ 5 is checked) | duplicate |
| T0 + 130 | same ID, reason "stop the run now" | retained, fingerprint `be403270…` | idempotency_conflict |
| T0 + 130 | a steer command `bbbb…` | bit 1 of 4 is 0 | not_admitted |
| T0 + 130 | a new cancel `bbbb…` expecting 4 | 4 ≠ 5 | conflict |
| T0 + 431 | `bbbb…` dated T0 + 130 | 431 > 430 | expired |
| T0 + 140 | after a handoff to generation 4 | grant's generation 3 ≠ 4 | stale |
| | client 3 revokes its own grant | not the owner | not_admitted |
| | the owner revokes it | epoch 0 → 1 | accepted |
| T0 + 150 | the first command again | epoch 1 | revoked |

## WORK and CTX

These two NIPs are described here so that the claim and the command have
something to refer to. Neither is built in Coax yet.

**NIP-WORK** tracks an item as a chain of authoritative revisions. A
command (`openagents.work-command.v1`) is `create` (a never-used item ID,
state `open`, no disposition) or `replace` (with the exact current
revision as `expected`). The host signs `openagents.work-revision.v1`
records: revision 1 for create, then previous + 1, each binding the
command and the accepted snapshot. Two signed successors of one revision
are a fork; readers keep both and stop automatic changes. The item states
are `open`, `in_progress`, `blocked`, `resolved`, `cancelled`,
`superseded` and `archived`, and the terminal ones need a disposition:
`resolved` needs `accepted` or `rejected` (resolved alone never means
success), `cancelled` needs `cancelled`, `superseded` needs a successor,
`archived` needs an already terminal item, and leaving a terminal state
needs `reopened`. The idempotency ledger has the same shape as CTRL's:
`(scope, issuer, command_id)` and SHA-256 of JCS.

**NIP-CTX** records the task frame (`openagents.task-frame.v1`): revision
0 with no previous, then each revision pointing at revision − 1, with the
objective, its origin (`user`, `program` or `inferred`), constraints,
acceptance, a snapshot of sources and the authenticated corrections. An
accepted CTRL steer creates the next frame; that is the revision a
command's `expected_revision` names. A context request and its selection
receipt record which evidence a recipient was shown, with one choice per
candidate; a failed decision is `unknown`, not irrelevant.

How the four fit together for one task: a WORK item pins a CTX frame; COORD
proposes an execution task for that frame and grants a claim to one worker;
CTRL clients steer the frame or request cancellation; the result comes back
as WORK evidence, and only a disposer's `accepted` resolves the item.

## What is not built

- **NIP-WORK.** No scope descriptors, items, commands, revision chains,
  fork detection, delegation, evidence links, dispositions, imports or
  projections. The state and disposition rules above are the NIP's, not
  code.
- **NIP-CTX.** No task frames, snapshots, representations and anchors,
  evidence graphs, context requests, selection receipts, history indexes or
  expansion.
- **COORD beyond the claim.** No proposals, dependency graphs and cycle
  checks, the exact reuse key, duplicate suggestions, reservations and
  shared budgets, findings, integration, background plans or OPT studies.
  Cancel by a separately admitted controller and `status` reads are not
  built; `coord_control` admits the owner only for cancel. The coordinator
  descriptor and its admission, epoch advance on handoff, and RUN records
  are not built: the epoch is a number the caller keeps.
- **CTRL beyond admission.** The invitation, grant, revocation, receipt and
  result artifacts are not encoded or sealed: the procedures take their
  fields. Task reads, views, projections and cursors, host-wide device
  commands (`send`, `queue`, `interrupt`, `answer`) and controller handoff
  are not built. Retention of command receipts is the caller's array.
- **The wire.** No artifact here is built or opened as JSON from a 3188
  envelope or a CJ job; chapter 75's `grant.coax` shows that step for
  NIP-HOST, and the same envelope reader applies.
- **Published vectors.** The four NIPs give no wire examples with real
  identifiers (NIP-CTRL's worked flow uses placeholders such as
  `<client-key>`), so there are no published ids to compare or to find
  wrong. The independent check is the Python fingerprint below.

## The example

[nostr/examples/work-control.coax](../examples/work-control.coax) runs the
two worked examples at a fixed clock. The coordinator part makes the claims,
writes, renewals, the release and the second claim of the first table. The
control part fingerprints the cancel command three ways (as sent, in JCS
form, and with a changed reason) and one malformed command, then pairs
client 3 and admits the commands of the second table.

<!-- source: nostr/examples/work-control.coax example -->
```jai
// Chapter 76: one task's lease from claim to release, and a client's
// control from pairing to revocation, at a fixed clock.
// The coordinator (owner key 2) grants claims and fences writes; workers 3
// and 7 race for one document. The controller (key 1) pairs client 3 and
// admits its commands; client 7 was never paired. Keys are the x-only
// public keys of the scalars 1, 2, 3 and 7: TEST ONLY.
#load "../../coax/lib/nostr/coord/claim.coax";
#load "../../coax/lib/nostr/ctrl/command.coax";
#load "../../coax/lib/nostr/ctrl/fingerprint.coax";
#import "Basic";

T0 :: 1790000000;
CONTROLLER :: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
OWNER :: "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
KEY_3 :: "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
KEY_7 :: "5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc";
INVITATION :: "1111111111111111111111111111111111111111111111111111111111111111";
CHALLENGE :: "2222222222222222222222222222222222222222222222222222222222222222";
GRANT :: "3333333333333333333333333333333333333333333333333333333333333333";
TASK :: "4444444444444444444444444444444444444444444444444444444444444444";
COMMAND :: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
COMMAND_2 :: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
SNAPSHOT :: "sha256:5555555555555555555555555555555555555555555555555555555555555555";
PAIRING_PRINT :: "sha256:6666666666666666666666666666666666666666666666666666666666666666";

// The cancel command three ways: as sent (CF_WIRE), in JCS form, and with
// one changed value under the same command ID.
CANONICAL :: "{\"action\":\"cancel\",\"command\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"epoch\":0,\"expected_revision\":4,\"expires_at\":1790000400,\"grant\":{\"digest\":\"sha256:3333333333333333333333333333333333333333333333333333333333333333\",\"media_type\":\"application/json\",\"schema\":\"openagents.control-grant.v1\",\"size\":512},\"issued_at\":1790000100,\"payload\":{\"reason\":\"stop the run\"},\"requires\":[],\"scope\":{\"controller\":\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"generation\":3,\"task\":\"4444444444444444444444444444444444444444444444444444444444444444\"},\"v\":\"openagents.task-command.v1\"}";
CHANGED :: "{\"action\":\"cancel\",\"command\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"epoch\":0,\"expected_revision\":4,\"expires_at\":1790000400,\"grant\":{\"digest\":\"sha256:3333333333333333333333333333333333333333333333333333333333333333\",\"media_type\":\"application/json\",\"schema\":\"openagents.control-grant.v1\",\"size\":512},\"issued_at\":1790000100,\"payload\":{\"reason\":\"stop the run now\"},\"requires\":[],\"scope\":{\"controller\":\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"generation\":3,\"task\":\"4444444444444444444444444444444444444444444444444444444444444444\"},\"v\":\"openagents.task-command.v1\"}";

CO_WORDS :: [8]string.["accepted","malformed","limit_exceeded","stale","conflict","not_admitted","expired","closed"];
CT_WORDS :: [10]string.["accepted","duplicate: the retained result, no new effect","not_admitted","revoked","expired",
  "stale","malformed","conflict","idempotency_conflict","closed: declined, no grant"];

worker_name :: (key:string)->string {return ifx key==KEY_3 then "worker 3" else "worker 7";}
claim :: (s:[]CoordState,c:[]CoordClaim,h:[]CoordHold,worker:string,expected:int,until:int,want:[]CoordWant,now:int) {
  q:CoordRequest;q.proposal="task-1";q.worker=worker;q.expected_revision=expected;q.requested_until=until;
  r:=coord_claim(s,c,h,q,want,now);
  print("T0 + %: % claims at revision %, until T0 + %: %",now-T0,worker_name(worker),expected,until-T0,CO_WORDS[r]);
  if r==CO_OK {print(", token %, revision %",s[0].token,s[0].revision);}
  print("\n");
}
control :: (s:[]CoordState,c:[]CoordClaim,h:[]CoordHold,token:int,action:int,actor:string,expected:int,until:int,now:int) {
  names:=[3]string.["renews","releases","cancels"];who:=ifx actor==OWNER then "the owner" else worker_name(actor);
  r:=coord_control(s,c,h,token,action,actor,expected,until,now);
  print("T0 + %: % % token %: %",now-T0,who,names[action],token,CO_WORDS[r]);
  if r==CO_OK {print(", revision %",s[0].revision);}
  print("\n");
}
dispatch :: (s:[]CoordState,c:[]CoordClaim,h:[]CoordHold,worker:string,token:int,now:int) {
  print("T0 + %: % writes doc under epoch 2, token %: %\n",now-T0,worker_name(worker),token,CO_WORDS[coord_fence(s,c,h,worker,"doc",2,token,now)]);
}
make_command :: (id:string,signer:string,print_:string,revision:int,action:int,at:int)->CtrlCommand {
  c:CtrlCommand;c.command=id;c.fingerprint=print_;c.signer=signer;c.grant=GRANT;
  c.scope=CtrlScope.{task=TASK,controller=CONTROLLER,generation=3};
  c.expected_revision=revision;c.issued_at=at;c.expires_at=at+300;c.action=action;return c;
}
```

<!-- source: nostr/examples/work-control.coax main -->
```jai
main :: () {
  print("COORD: one document, two workers; the coordinator's epoch is 2, revision 10, longest lease 600 s\n");
  s:=[1]CoordState.[CoordState.{owner=OWNER,epoch=2,revision=10,max_lease=600}];c:[4]CoordClaim;h:[8]CoordHold;
  first:=[2]CoordWant.[CoordWant.{resource="doc",write=true},CoordWant.{resource="spec",snapshot=SNAPSHOT}];
  doc:=[1]CoordWant.[CoordWant.{resource="doc",write=true}];
  claim(s[..],c[..],h[..],KEY_3,10,T0+300,first[..],T0);
  claim(s[..],c[..],h[..],KEY_7,10,T0+300,doc[..],T0);
  claim(s[..],c[..],h[..],KEY_7,11,T0+300,doc[..],T0);
  dispatch(s[..],c[..],h[..],KEY_3,1,T0+100);
  dispatch(s[..],c[..],h[..],KEY_7,1,T0+100);
  control(s[..],c[..],h[..],1,CO_RENEW,KEY_3,11,T0+700,T0+250);
  control(s[..],c[..],h[..],1,CO_RENEW,KEY_3,12,T0+1000,T0+250);
  print("worker 3 stops answering; its lease ends at T0 + 700\n");
  dispatch(s[..],c[..],h[..],KEY_3,1,T0+700);
  claim(s[..],c[..],h[..],KEY_7,12,T0+1010,doc[..],T0+710);
  control(s[..],c[..],h[..],1,CO_RENEW,KEY_3,12,T0+1000,T0+710);
  control(s[..],c[..],h[..],1,CO_CANCEL,KEY_7,12,0,T0+715);
  print("the owner fences worker 3's dispatcher, then releases its claim\n");
  control(s[..],c[..],h[..],1,CO_RELEASE,OWNER,12,0,T0+720);
  claim(s[..],c[..],h[..],KEY_7,13,T0+1020,doc[..],T0+720);
  dispatch(s[..],c[..],h[..],KEY_3,1,T0+730);
  dispatch(s[..],c[..],h[..],KEY_7,2,T0+730);
  control(s[..],c[..],h[..],2,CO_RELEASE,KEY_7,14,0,T0+900);
  print("record: % claims, % holds, last token %, revision %\n",s[0].claims,s[0].holds,s[0].token,s[0].revision);

  print("CTRL: task 4444..., controller key 1, generation 3, frame revision 4\n");
  scope:=CtrlScope.{task=TASK,controller=CONTROLLER,generation=3};
  inv:=[1]CtrlInvitation.[CtrlInvitation.{invitation=INVITATION,challenge=CHALLENGE,client=KEY_3,scope=scope,rights=(1<<CT_OBSERVE)|(1<<CT_CANCEL),expires_at=T0+600}];
  grants:[2]CtrlGrant;n:[1]int;kept:[1]CtrlRetained;none:=kept[0..<0];
  canon:[1024]u8;digest:[CF_DIGEST]u8;first_digest:[CF_DIGEST]u8;changed_digest:[CF_DIGEST]u8;
  p:=ctrl_fingerprint(CF_WIRE,canon[..],first_digest[..]);
  wire_print:=cast(string)first_digest[..];
  print("fingerprint of the cancel command as sent (% bytes, % in JCS form):\n  %\n",CF_WIRE.count,p.canonical,wire_print);
  p=ctrl_fingerprint(CANONICAL,canon[..],digest[..]);
  print("the same command in JCS form: %\n",ifx cast(string)digest[..]==wire_print then "the same fingerprint" else "a different fingerprint");
  p=ctrl_fingerprint(CHANGED,canon[..],changed_digest[..]);
  changed_print:=cast(string)changed_digest[..];
  print("reason \"stop the run now\" instead: %\n",changed_print);
  p=ctrl_fingerprint("{\"command\":1,\"command\":2}",canon[..],digest[..]);
  print("a command with a duplicate key: %\n",ifx p.error==CF_JSON then "refused, no fingerprint" else "fingerprinted");
  cancel:=make_command(COMMAND,KEY_3,wire_print,4,CT_CANCEL,T0+100);

  print("before pairing\n");
  stray:=cancel;stray.signer=KEY_7;
  print("client 7 sends the cancel command: %\n",CT_WORDS[ctrl_command_admit(grants[0..<n[0]],none,stray,scope,4,T0+100)]);
  print("client 3 sends it: %\n",CT_WORDS[ctrl_command_admit(grants[0..<n[0]],none,cancel,scope,4,T0+100)]);
  print("pairing: client 3 accepts cancel only\n");
  reply:=CtrlPairing.{invitation=INVITATION,signer=KEY_3,client=KEY_3,challenge=CHALLENGE,rights=1<<CT_CANCEL,accepted=true,fingerprint=PAIRING_PRINT};
  relayed:=reply;relayed.signer=KEY_7;
  print("client 7 relays client 3's response: %\n",CT_WORDS[ctrl_pair(inv[..],grants[..],n[..],relayed,scope,true,GRANT,T0+86400,T0+60)]);
  guessed:=reply;guessed.challenge=INVITATION;
  print("the wrong challenge: %\n",CT_WORDS[ctrl_pair(inv[..],grants[..],n[..],guessed,scope,true,GRANT,T0+86400,T0+60)]);
  wider:=reply;wider.rights=(1<<CT_CANCEL)|(1<<CT_STEER);
  print("asking for steer too: %\n",CT_WORDS[ctrl_pair(inv[..],grants[..],n[..],wider,scope,true,GRANT,T0+86400,T0+60)]);
  print("the owner has not admitted it: %\n",CT_WORDS[ctrl_pair(inv[..],grants[..],n[..],reply,scope,false,GRANT,T0+86400,T0+60)]);
  print("the owner admits it: %",CT_WORDS[ctrl_pair(inv[..],grants[..],n[..],reply,scope,true,GRANT,T0+86400,T0+60)]);
  print(", grant 3333..., rights %, epoch %, T0 + % to T0 + %\n",grants[0].rights,grants[0].epoch,grants[0].issued_at-T0,grants[0].expires_at-T0);
  print("the same response again: %\n",CT_WORDS[ctrl_pair(inv[..],grants[..],n[..],reply,scope,true,GRANT,T0+86400,T0+70)]);

  print("commands, now = T0 + 120\n");
  g:=grants[0..<n[0]];
  print("client 7 sends the cancel command: %\n",CT_WORDS[ctrl_command_admit(g,none,stray,scope,4,T0+120)]);
  print("client 3 sends it: %\n",CT_WORDS[ctrl_command_admit(g,none,cancel,scope,4,T0+120)]);
  kept[0]=CtrlRetained.{grant=GRANT,command=COMMAND,fingerprint=wire_print};
  print("the authority records cancel_requested; the frame is now revision 5\n");
  print("the same command again: %\n",CT_WORDS[ctrl_command_admit(g,kept[..],cancel,scope,5,T0+130)]);
  edited:=cancel;edited.fingerprint=changed_print;
  print("the changed reason under the same command ID: %\n",CT_WORDS[ctrl_command_admit(g,kept[..],edited,scope,5,T0+130)]);
  steer:=make_command(COMMAND_2,KEY_3,PAIRING_PRINT,5,CT_STEER,T0+130);
  print("a steer command: %\n",CT_WORDS[ctrl_command_admit(g,kept[..],steer,scope,5,T0+130)]);
  old:=make_command(COMMAND_2,KEY_3,PAIRING_PRINT,4,CT_CANCEL,T0+130);
  print("a new cancel expecting revision 4: %\n",CT_WORDS[ctrl_command_admit(g,kept[..],old,scope,5,T0+130)]);
  late:=make_command(COMMAND_2,KEY_3,PAIRING_PRINT,5,CT_CANCEL,T0+130);
  print("the same, dated T0 + 130, at T0 + 431: %\n",CT_WORDS[ctrl_command_admit(g,kept[..],late,scope,5,T0+431)]);
  moved:=scope;moved.generation=4;
  print("after a handoff to generation 4: %\n",CT_WORDS[ctrl_command_admit(g,kept[..],late,moved,5,T0+140)]);
  print("client 3 tries to revoke its own grant: %\n",CT_WORDS[ctrl_revoke(g,GRANT,KEY_3,OWNER)]);
  print("the owner revokes it: %, epoch %\n",CT_WORDS[ctrl_revoke(g,GRANT,OWNER,OWNER)],g[0].epoch);
  print("the first command again: %\n",CT_WORDS[ctrl_command_admit(g,kept[..],cancel,scope,5,T0+150)]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/work-control.coax
```

Its output, in debug and release builds alike
([nostr/fixtures/work-control.out](../fixtures/work-control.out)):

```text
COORD: one document, two workers; the coordinator's epoch is 2, revision 10, longest lease 600 s
T0 + 0: worker 3 claims at revision 10, until T0 + 300: accepted, token 1, revision 11
T0 + 0: worker 7 claims at revision 10, until T0 + 300: stale
T0 + 0: worker 7 claims at revision 11, until T0 + 300: conflict
T0 + 100: worker 3 writes doc under epoch 2, token 1: accepted
T0 + 100: worker 7 writes doc under epoch 2, token 1: not_admitted
T0 + 250: worker 3 renews token 1: accepted, revision 12
T0 + 250: worker 3 renews token 1: limit_exceeded
worker 3 stops answering; its lease ends at T0 + 700
T0 + 700: worker 3 writes doc under epoch 2, token 1: expired
T0 + 710: worker 7 claims at revision 12, until T0 + 1010: conflict
T0 + 710: worker 3 renews token 1: expired
T0 + 715: worker 7 cancels token 1: not_admitted
the owner fences worker 3's dispatcher, then releases its claim
T0 + 720: the owner releases token 1: accepted, revision 13
T0 + 720: worker 7 claims at revision 13, until T0 + 1020: accepted, token 2, revision 14
T0 + 730: worker 3 writes doc under epoch 2, token 1: stale
T0 + 730: worker 7 writes doc under epoch 2, token 2: accepted
T0 + 900: worker 7 releases token 2: accepted, revision 15
record: 2 claims, 3 holds, last token 2, revision 15
CTRL: task 4444..., controller key 1, generation 3, frame revision 4
fingerprint of the cancel command as sent (646 bytes, 612 in JCS form):
  sha256:71fbe0a66f7f7fc085b5d2d08c8fa76cc7e5fe711941e7e80d76aad9f52c4636
the same command in JCS form: the same fingerprint
reason "stop the run now" instead: sha256:be4032700c6f319d5d5b632e88a612386e4dc245eb882a02c164a46f43876a9a
a command with a duplicate key: refused, no fingerprint
before pairing
client 7 sends the cancel command: not_admitted
client 3 sends it: not_admitted
pairing: client 3 accepts cancel only
client 7 relays client 3's response: not_admitted
the wrong challenge: not_admitted
asking for steer too: malformed
the owner has not admitted it: not_admitted
the owner admits it: accepted, grant 3333..., rights 4, epoch 0, T0 + 60 to T0 + 86400
the same response again: duplicate: the retained result, no new effect
commands, now = T0 + 120
client 7 sends the cancel command: not_admitted
client 3 sends it: accepted
the authority records cancel_requested; the frame is now revision 5
the same command again: duplicate: the retained result, no new effect
the changed reason under the same command ID: idempotency_conflict
a steer command: not_admitted
a new cancel expecting revision 4: conflict
the same, dated T0 + 130, at T0 + 431: expired
after a handoff to generation 4: stale
client 3 tries to revoke its own grant: not_admitted
the owner revokes it: accepted, epoch 1
the first command again: revoked
```

Every line matches the worked examples. The refusals each stop at a
different step: the loser's first claim at the revision compare
(`stale`), its second at the held resource (`conflict`); worker 3's late
write at the lease end (`expired`) and, after worker 7 took over, at the
newer token (`stale`); the unpaired client 7 at the grant lookup
(`not_admitted`), both before and after client 3 was paired; and the
command with a duplicate key before any fingerprint exists.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/work-control.coax](../examples/checks/work-control.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The fingerprints from Python's `json.dumps(sort_keys=True,
  separators=(",", ":"), ensure_ascii=False)` and `hashlib.sha256` over
  the same command objects: `jcs.coax`'s canonical bytes must equal
  Python's 612 bytes exactly, and both digests must match.
- PLAN.md's "Done means", in both orders of arrival: two workers claim one
  task, exactly one wins, the loser's writes refuse under every token it
  might name, and after a release and its own claim, the first winner's
  writes are the ones fenced. A command from a key with no grant refuses,
  for client 7, the owner and the controller key alike.
- The arithmetic of the worked examples.

<!-- source: nostr/examples/checks/work-control.coax check -->
```jai
// The fingerprints, from Python's json.dumps(sort_keys=True,
// separators=(",", ":"), ensure_ascii=False) and hashlib.sha256 over the
// same command objects. They share no code with jcs.coax or sha256.coax.
// Python's compact sorted output is also byte for byte the CANONICAL string.
PY_PRINT :: "sha256:71fbe0a66f7f7fc085b5d2d08c8fa76cc7e5fe711941e7e80d76aad9f52c4636";
PY_CHANGED :: "sha256:be4032700c6f319d5d5b632e88a612386e4dc245eb882a02c164a46f43876a9a";
PY_JCS_BYTES :: 612;

#test "the fingerprints match the Python reference, and JCS gives Python's bytes" {
  canon:[1024]u8;d:[CF_DIGEST]u8;
  p:=ctrl_fingerprint(CF_WIRE,canon[..],d[..]);
  #assert p.error==CF_OK && p.canonical==PY_JCS_BYTES && cast(string)d[..]==PY_PRINT;
  #assert cast(string)canon[0..<p.canonical]==CANONICAL && CANONICAL.count==PY_JCS_BYTES;
  p=ctrl_fingerprint(CHANGED,canon[..],d[..]);
  #assert p.error==CF_OK && p.canonical==PY_JCS_BYTES+4 && cast(string)d[..]==PY_CHANGED;
}

// PLAN.md's Done means, worked by hand from NIP-COORD's rules: epoch 2,
// revision 10, a 600 s longest lease; both workers saw revision 10.
#test "Done means: two workers claim one task, exactly one wins, the other's writes are fenced" {
  for order:0..<2 {
    s:=[1]CoordState.[CoordState.{owner=OWNER,epoch=2,revision=10,max_lease=600}];c:[4]CoordClaim;h:[4]CoordHold;
    doc:=[1]CoordWant.[CoordWant.{resource="doc",write=true}];
    a:=ifx order==0 then KEY_3 else KEY_7;b:=ifx order==0 then KEY_7 else KEY_3;
    qa:=CoordRequest.{proposal="task-1",worker=a,expected_revision=10,requested_until=T0+300};
    qb:=qa;qb.worker=b;
    ra:=coord_claim(s[..],c[..],h[..],qa,doc[..],T0);rb:=coord_claim(s[..],c[..],h[..],qb,doc[..],T0);
    #assert ra==CO_OK && rb==CO_STALE && s[0].token==1 && s[0].claims==1;
    qb.expected_revision=11;#assert coord_claim(s[..],c[..],h[..],qb,doc[..],T0)==CO_CONFLICT;
    // The loser's writes: under the winner's token, and under any token it might guess.
    #assert coord_fence(s[..],c[..],h[..],a,"doc",2,1,T0+1)==CO_OK;
    for t:0..<4 {#assert coord_fence(s[..],c[..],h[..],b,"doc",2,t,T0+1)==CO_NOT_ADMITTED;}
    // After a release and the loser's own claim, the first winner is the one fenced.
    #assert coord_control(s[..],c[..],h[..],1,CO_RELEASE,OWNER,11,0,T0+2)==CO_OK;
    qb.expected_revision=12;#assert coord_claim(s[..],c[..],h[..],qb,doc[..],T0+2)==CO_OK && s[0].token==2;
    #assert coord_fence(s[..],c[..],h[..],a,"doc",2,1,T0+3)==CO_STALE && coord_fence(s[..],c[..],h[..],b,"doc",2,2,T0+3)==CO_OK;
  }
}
#test "Done means: a command from an unpaired client refuses" {
  scope:=CtrlScope.{task=TASK,controller=CONTROLLER,generation=3};kept:[1]CtrlRetained;none:=kept[0..<0];
  cancel:=make_command(COMMAND,KEY_3,PY_PRINT,4,CT_CANCEL,T0+100);
  // No grants at all, and a grant table that names only client 3.
  empty:[1]CtrlGrant;
  #assert ctrl_command_admit(empty[0..<0],none,cancel,scope,4,T0+100)==CT_NOT_ADMITTED;
  g:=[1]CtrlGrant.[CtrlGrant.{grant=GRANT,client=KEY_3,scope=scope,rights=1<<CT_CANCEL,issued_at=T0,expires_at=T0+86400}];
  #assert ctrl_command_admit(g[..],none,cancel,scope,4,T0+100)==CT_ACCEPTED;
  for key:[3]string.[KEY_7,OWNER,CONTROLLER] {
    stray:=cancel;stray.signer=key;#assert ctrl_command_admit(g[..],none,stray,scope,4,T0+100)==CT_NOT_ADMITTED;
  }
}
// The lifecycle values in the transcript, by hand.
#test "the transcript's numbers" {
  // Renewal at T0 + 250: the longest lease ends at 250 + 600 = 850, so 700 fits and 1000 does not.
  #assert 250+600==850 && 700<=850 && 1000>850;
  // Revisions: claim 11, renew 12, release 13, claim 14, release 15.
  #assert 10+5==15;
  // The command window: issued T0 + 130, 300 s, so T0 + 430 is the last second.
  #assert 130+300==430;
  // The wire form has 34 more bytes than the JCS form: a space after each of 19 colons and 15 commas.
  #assert CF_WIRE.count-CANONICAL.count==34;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/work-control.coax
```

`coax test` runs 123 checks, including the loaded files' own checks; all
pass.

### The library's own checks

`claim.coax` checks the race (stale, then conflict, with the counters moved
once), the lease bounds at their edges (a lease ending now, at the maximum
and one second past it), duplicate and too many resources, all-or-nothing
grants and a full record, the sharing table in all 16 combinations,
expiry that frees nothing, who may renew, release and cancel, closed
claims, each step of the fence, and with `#forall` that each token is one
more than the last. One check reads a request into buffers it then reuses
for the next request, and asserts that the hold stays on its resource and
the claim on its worker. `command.coax` checks pairing (relayed responses, the
wrong challenge, expiry at the exact second, rights wider than the
invitation, a moved scope, a missing owner admission, duplicate,
conflicting and declined responses), an unpaired key, the order of
revocation, expiry and scope, each right alone, the command window at
each edge, idempotency before the revision, and revocation by the owner
only. `fingerprint.coax` checks the example command's digest against
Python's and its refusals.

```shell
coax/bin/coax test coax/lib/nostr/coord/claim.coax --library
coax/bin/coax mutate coax/lib/nostr/coord/claim.coax --library --root-only
coax/bin/coax test coax/lib/nostr/ctrl/command.coax --library
coax/bin/coax mutate coax/lib/nostr/ctrl/command.coax --library --root-only
coax/bin/coax test coax/lib/nostr/ctrl/fingerprint.coax --library
coax/bin/coax mutate coax/lib/nostr/ctrl/fingerprint.coax --library --root-only
```

On 2026-11-01, `claim.coax` passed its 12 checks and `coax mutate
--root-only` killed all 152 mutants; `command.coax` passed its 10 checks
and killed all 136; `fingerprint.coax` passed its checks (97 with
`jcs.coax`, `envelope.coax` and SHA-256's own) and killed all 6. None
survived and none is marked equivalent. The full mutation run
(`COAX_MUTATION_FULL=1`) and cross-target runs were not done. On
2026-11-03, after the record began keeping its own copies (#12692),
`claim.coax` passed 13 checks and `coax mutate --root-only` killed all 205
mutants, none marked equivalent.

## Summary

- WORK tracks the objective, COORD the claims, CTRL the extra clients and
  CTX the frame. Each authority decides by its own record and clock.
- `coord_claim` compares the expected revision, bounds the lease by the
  coordinator's clock, grants all resources or none, and issues the next
  token. Snapshot reads share with writes; live reads and writes do not.
- A lease's expiry stops dispatch and frees nothing; only a release does.
- `coord_fence` refuses a write from another worker, an old epoch or an
  older token, a closed claim and an expired lease.
- `ctrl_pair` turns one invitation, the invited key's challenge response
  and the owner's admission into one grant; `ctrl_revoke` ends it.
- `ctrl_command_admit` refuses an unpaired key first, checks the right and
  window, treats an identical retry as a duplicate before it compares the
  frame revision, and refuses changed bytes under a used command ID.

## Dependencies

- [75. Hosts and reachability](ch075-hosts-and-reachability.md)

## Sources

- [OpenAgents NIP-WORK](../nips/openagents/NIP-WORK.md)
- [OpenAgents NIP-COORD](../nips/openagents/NIP-COORD.md)
- [OpenAgents NIP-CTRL](../nips/openagents/NIP-CTRL.md)
- [OpenAgents NIP-CTX](../nips/openagents/NIP-CTX.md)
- [OpenAgents shared contracts](../nips/openagents/contracts.md)
- [RFC 8785, JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
