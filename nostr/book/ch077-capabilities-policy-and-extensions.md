# 77. Capabilities, policy and extensions

A host advertises what it can run and decides what it will run. Four
OpenAgents NIPs make both inspectable. NIP-CAP separates a capability's
description from the host binding that runs it and the grant that lets a
principal use that binding. NIP-POL makes host decisions such as approvals
inspectable. NIP-EXT ships components as signed, immutable releases that
their publisher can revoke. NIP-REG lets a curator list releases in a
registry. This chapter builds the host's dispatch decision, single-use
approvals, and the client's decision to install a release, and runs them
against signed release, revocation and checkpoint events.

Part X, Every NIP. Result: NIP-CAP dispatch (definition, binding,
presence, grant, effects, ceilings and an enforcement plan), NIP-POL
approval consumption, and NIP-EXT installation (namespace, manifest
agreement, unknown features and component kinds, equivocation, revocation,
dependencies and checkpoint freshness). Probes, preferences, instruction
sets, routing, listings, migration and NIP-REG are not built yet; see
[What is not built](#what-is-not-built).

## What this chapter teaches

- The four separate CAP objects: a definition describes, a binding runs, a
  grant permits, presence observes. Dispatch needs all four.
- Why a binding names one exact definition digest and profile, so that a
  republished definition never moves an installed binding.
- Effects granted one at a time, and the intersection of grant and
  definition that the host enforces.
- Bounds as ceilings, the smallest one winning, and an enforcement plan
  that refuses a bound nobody enforces.
- POL approvals: one exact action, one recognized approver, consumed once.
- EXT releases: the root key's namespace, equivocation, monotone
  revocation, and why "no revocations found" proves nothing without a
  fresh checkpoint.
- How these NIPs treat extensions: an unknown required feature or component
  kind refuses; only `meta` is ignored.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [dispatch.coax](../../coax/lib/nostr/cap/dispatch.coax) | `cap_dispatch(defs, bindings, grants, policy, q, now)` | The host's decision on one capability request, and the ceiling it records |
| [approve.coax](../../coax/lib/nostr/pol/approve.coax) | `pol_admit(q, recognized, decisions, consumed, now)` | Whether an approval request has an approval to consume now |
| [install.coax](../../coax/lib/nostr/ext/install.coax) | `ext_install(r, releases, revocations, c, have, p, now)` | Whether a client may install one release |
| [install.coax](../../coax/lib/nostr/ext/install.coax) | `ext_checkpoint_accept(held, have, c)` | Whether a client keeps a newer revocation checkpoint |

All three files are new with this chapter. Each decides admission or
policy, so each is an `#authority` file: the checks came first, and a
mutation run kills every mutant. Like chapter 75's `admit.coax`, none of
them loads the crypto. They take verified signers, event ids and the host's
own record as plain fields, so their checks run in milliseconds. The
example reads the signed events with chapter 21's reader and chapter 11's
BIP-340 verification before it calls them.

## Capabilities: description, binding, grant, presence

NIP-CAP keeps four objects apart:

| Object | What it is | Who decides it |
| --- | --- | --- |
| Definition | A portable interface: `id`, `profile`, input and output schemas, declared `effects`, `minimum` bounds | Its publisher. It grants nothing |
| Binding | The host's native operation, executable or remote worker that satisfies one definition | The host's own configuration, never an untrusted checkout |
| Grant | Permission for one principal to use one binding, within effects, bounds and an expiry | The host's operator, never the component |
| Presence | Whether the binding is usable here: `present`, `absent`, `unavailable`, `unprobed` or `unknown` | Local observation, never publication |

A definition's `profile` says what kind of binding can satisfy it:
`native`, `executor`, `plugin`, `adapter` or `service`. A plugin cannot
take an executor's place, and an MCP listing cannot create a binding.

A binding records the digest of the definition it satisfies. When the
publisher republishes a definition, its discovery head (kind 30180) points
to a new digest, but the binding still names the old one. NIP-CAP says
replacement "updates discovery, never an installed or admitted execution".
So the old binding no longer matches, and the host must bind the new
definition on purpose.

Effects use the shared contracts' six names: `reads`, `writes`,
`network`, `process`, `delegates` and `spend`. Each is granted separately.
A grant's effects must also be among the definition's, so the host enforces
the intersection. In the NIP, `reads`, `writes` and `network` are lists of
logical scopes; this chapter grants each class whole, as one bit:

| Effect | Bit |
| --- | --- |
| `reads` | 1 |
| `writes` | 2 |
| `network` | 4 |
| `process` | 8 |
| `delegates` | 16 |
| `spend` | 32 |

The twelve bounds (`wall_ms`, `memory_bytes`, ..., `spend_microunits`) are
bits 1 to 2048 the same way. A request lists the bounds it requires to be
enforced.

The host's record:

<!-- source: coax/lib/nostr/cap/dispatch.coax record -->
```jai
CapDefinition :: struct {id:string;digest:string;profile:int;effects:int;minimum_wall_ms:int;}
CapBinding :: struct {binding:string;definition:string;profile:int;invoke:bool;invoke_writing:bool;presence:int;enforced:int;host:int;}
CapGrant :: struct {grant:string;issuer:string;principal:string;binding:string;effects:int;wall_ms:int;purpose:int;run:string;expires_at:int;revoked:bool;}
CapPolicy :: struct {owner:string;deny:[]string;assurance:int;wall_ms:int;}
CapRequest :: struct {principal:string;definition:string;effects:int;required:int;wall_ms:int;use:int;run:string;}
// The decision and what the host records before dispatch.
CapDispatch :: struct {error:u32;definition:int=-1;binding:int=-1;grant:int=-1;wall_ms:int;limit:int;}
```

`CapPolicy.owner` is the operator's key: only grants it issued count.
`deny` lists definition IDs the operator refuses. `assurance` is
`host_enforced` (only the host's own mechanisms count) or
`trusted_contract` (the binding's declared `enforced` bounds count too).

### The dispatch order

NIP-CAP says to record effective limits and grants before dispatch, and
to recheck revocation and binding identity at dispatch. The outcomes, in
the order the host checks them:

<!-- source: coax/lib/nostr/cap/dispatch.coax codes -->
```jai
CAP_OK :: cast(u32)0;             // dispatch to the binding
CAP_MALFORMED :: cast(u32)1;      // an effect or bound outside the vocabulary
CAP_DENIED :: cast(u32)2;         // the operator's deny list names the definition
CAP_NO_DEFINITION :: cast(u32)3;  // the host holds no definition with that id
CAP_NO_BINDING :: cast(u32)4;     // no host binding for the definition's digest and profile
CAP_NO_INVOKE :: cast(u32)5;      // the binding has no invocation contract (for writing)
CAP_UNAVAILABLE :: cast(u32)6;    // the binding's presence is not `present`
CAP_NOT_ADMITTED :: cast(u32)7;   // no operator grant for this principal and binding,
                                  // an effect outside grant ∩ definition, or an
                                  // evaluation grant used outside its run
CAP_REVOKED :: cast(u32)8;        // the grant is revoked
CAP_EXPIRED :: cast(u32)9;        // the grant has expired by the host's clock
CAP_LIMIT :: cast(u32)10;         // the definition's minimum is above the effective ceiling
CAP_CANNOT_ENFORCE :: cast(u32)11; // a required bound has no mechanism at the policy's assurance
```

<!-- source: coax/lib/nostr/cap/dispatch.coax dispatch -->
```jai
cap_dispatch :: (defs:[]CapDefinition,bindings:[]CapBinding,grants:[]CapGrant,policy:CapPolicy,q:CapRequest,now:int)->CapDispatch {
  r:CapDispatch;
  // 1. The request names only known effects and bounds.
  if q.effects&CAP_EFFECTS!=q.effects || q.required&CAP_BOUNDS!=q.required {return cap_fail(r,CAP_MALFORMED);}
  // 2. The operator's deny list refuses by definition id; preferences grant nothing.
  for id:policy.deny {if id==q.definition {return cap_fail(r,CAP_DENIED);}}
  // 3. The definition, then a host binding to its exact digest and profile.
  for i:0..<defs.count {if r.definition<0 && defs[i].id==q.definition {r.definition=i;}}
  if r.definition<0 {return cap_fail(r,CAP_NO_DEFINITION);}
  d:=defs[r.definition];
  for i:0..<bindings.count {
    if r.binding<0 && bindings[i].definition==d.digest && bindings[i].profile==d.profile {r.binding=i;}
  }
  if r.binding<0 {return cap_fail(r,CAP_NO_BINDING);}
  b:=bindings[r.binding];
  if !b.invoke || (q.effects&CAP_WRITES!=0 && b.profile==CAP_EXECUTOR && !b.invoke_writing) {return cap_fail(r,CAP_NO_INVOKE);}
  if b.presence!=CAP_PRESENT {return cap_fail(r,CAP_UNAVAILABLE);}
  // 4. The grant: from the operator, to this principal, for this binding.
  for i:0..<grants.count {
    g:=grants[i];
    if r.grant<0 && g.issuer==policy.owner && g.principal==q.principal && g.binding==b.binding {r.grant=i;}
  }
  if r.grant<0 {return cap_fail(r,CAP_NOT_ADMITTED);}
  g:=grants[r.grant];
  if g.revoked {return cap_fail(r,CAP_REVOKED);}
  if now>=g.expires_at {return cap_fail(r,CAP_EXPIRED);}
  if q.effects&g.effects&d.effects!=q.effects {return cap_fail(r,CAP_NOT_ADMITTED);}
  if g.purpose==CAP_EVALUATION && (q.use!=CAP_RUN || q.run!=g.run) {return cap_fail(r,CAP_NOT_ADMITTED);}
  // 5. The effective ceiling is the smallest; the definition's minimum must fit.
  r.wall_ms=q.wall_ms;
  if g.wall_ms<r.wall_ms {r.wall_ms=g.wall_ms;r.limit=CAP_BY_GRANT;}
  if policy.wall_ms<r.wall_ms {r.wall_ms=policy.wall_ms;r.limit=CAP_BY_OPERATOR;}
  if r.wall_ms<=0 || d.minimum_wall_ms>r.wall_ms {return cap_fail(r,CAP_LIMIT);}
  // 6. Every required bound has a mechanism: the host's, or the binding's
  // enforced contract when the operator accepts that assurance.
  covered:=b.host;
  if policy.assurance==CAP_TRUSTED_CONTRACT {covered|=b.enforced;}
  if q.required&covered!=q.required {return cap_fail(r,CAP_CANNOT_ENFORCE);}
  return cap_fail(r,CAP_OK);
}
```

1. A request that names an effect or bound outside the vocabulary is
   `malformed`. An unknown effect is not read-only.
2. The deny list refuses first. Preferences only break ties among
   admissible bindings, so they grant nothing.
3. The host must hold the definition, then a binding for its exact digest
   and profile. A binding without an invocation contract "may be inspected
   but MUST NOT receive work". A writing request on an executor needs
   `invoke_writing`. Only `present` can become a route.
4. The grant must come from the operator, name this principal and this
   binding, be unrevoked and unexpired by the host's clock. Every requested
   effect must be in both the grant and the definition. An `evaluation`
   grant authorizes only the run it names: no training, export or
   deployment.
5. The effective ceiling is the smallest of the request's, the grant's and
   the operator's. The host records which one decided. The definition's
   `minimum` must fit under it.
6. Every required bound needs a mechanism. An executor's own claim counts
   only when the operator accepts `trusted_contract`; a signed claim does
   not prove enforcement.

### Worked example: three requests against one policy, by hand

The host holds two definitions, two bindings and one grant:

| Record | Fields |
| --- | --- |
| definition `op:docs.search` | digest `sha256:5e1f`, executor, effects reads + writes + network = 7, minimum 1000 ms |
| definition `op:repo.patch` | digest `sha256:9c02`, executor, effects reads + writes = 3, minimum 5000 ms |
| binding `sha256:b5e1` | definition `sha256:5e1f`, executor, invocation contract, present, host enforces `wall_ms` (1), the executor claims `memory_bytes` (2) |
| binding `sha256:b77a` | definition `sha256:77aa` (the old `repo.patch`), otherwise the same |
| grant `g-docs` | issuer the operator, principal the agent (key 3), binding `sha256:b5e1`, effects reads + network = 5, 60,000 ms, until T0 + 3600 |
| policy | owner the operator (key 1), deny `op:shell.exec`, `host_enforced`, 120,000 ms |

<!-- source: nostr/examples/capabilities-policy.coax world -->
```jai
// The host's record: two definitions, one binding, one grant, one policy.
host_definitions :: (d:[]CapDefinition) {
  d[0].id="op:docs.search";d[0].digest="sha256:5e1f";d[0].profile=CAP_EXECUTOR;
  d[0].effects=CAP_READS|CAP_WRITES|CAP_NETWORK;d[0].minimum_wall_ms=1000;
  // repo.patch was republished: its digest is now 9c02, the binding still names 77aa.
  d[1].id="op:repo.patch";d[1].digest="sha256:9c02";d[1].profile=CAP_EXECUTOR;
  d[1].effects=CAP_READS|CAP_WRITES;d[1].minimum_wall_ms=5000;
}
host_bindings :: (b:[]CapBinding) {
  b[0].binding="sha256:b5e1";b[0].definition="sha256:5e1f";b[0].profile=CAP_EXECUTOR;b[0].invoke=true;
  b[0].invoke_writing=true;b[0].presence=CAP_PRESENT;b[0].host=CAP_WALL_MS;b[0].enforced=CAP_MEMORY_BYTES;
  b[1]=b[0];b[1].binding="sha256:b77a";b[1].definition="sha256:77aa";
}
host_grants :: (g:[]CapGrant) {
  g[0].grant="g-docs";g[0].issuer=OPERATOR;g[0].principal=AGENT;g[0].binding="sha256:b5e1";
  g[0].effects=CAP_READS|CAP_NETWORK;g[0].wall_ms=60000;g[0].expires_at=T0+3600;
}
ask_for :: (definition:string,effects:int,wall_ms:int)->CapRequest {
  q:CapRequest;q.principal=AGENT;q.definition=definition;q.effects=effects;q.required=CAP_WALL_MS;q.wall_ms=wall_ms;return q;
}
```

Each request requires `wall_ms` and runs at T0 + 100:

| Request | Step that decides | Result |
| --- | --- | --- |
| A: `docs.search`, reads (1), 30,000 ms | 1 is known; not denied; definition found; binding `b5e1` names `5e1f`, executor = executor; invocation, present; grant `g-docs` from the operator to the agent for `b5e1`; unrevoked; 100 < 3600; 1 & 5 & 7 = 1; operational. Ceiling min(30000, 60000, 120000) = 30000 ≥ 1000, set by the request. Required bit 1 & host 1 = 1 | dispatch to `b5e1`, 30,000 ms |
| B: `docs.search`, writes (2), 30,000 ms | everything as A up to the effects: 5 & 7 = 5, and 2 & 5 = 0 ≠ 2 | not_admitted |
| C: `repo.patch`, reads, 30,000 ms | definition found, digest `9c02`; `b5e1` names `5e1f` and `b77a` names `77aa`: neither matches | no binding |

Request C is the first "Done means" line: the capability exists and the
agent might even hold a grant for the old binding, but no binding matches
the definition it asks for, so dispatch refuses it. Binding `b77a` to
`9c02` would make it dispatchable.

## Approvals

NIP-POL's action artifact (`openagents.action.v1`) binds everything one
action will do: operation, binding, input, effects, bounds, recipient and a
nonce. Its exact digest is what an approver approves. An approval request
names that digest, the requester, one approver and an expiry; a decision is
`approve` or `deny` with its own expiry, never later than the request's.

<!-- source: coax/lib/nostr/pol/approve.coax record -->
```jai
PolRequest :: struct {request:string;action:string;requester:string;approver:string;expires_at:int;}
PolDecision :: struct {request:string;action:string;approver:string;approve:bool;expires_at:int;}
PolConsumed :: struct {request:string;action:string;}
```

<!-- source: coax/lib/nostr/pol/approve.coax codes -->
```jai
POL_OK :: cast(u32)0;           // approved: consume it now, atomically, then dispatch
POL_REPLAY :: cast(u32)1;       // already consumed: the same receipt, no new dispatch
POL_PENDING :: cast(u32)2;      // no counted decision yet
POL_DENIED :: cast(u32)3;       // a counted deny, alone or against an approve
POL_EXPIRED :: cast(u32)4;      // the request has expired
POL_NOT_ADMITTED :: cast(u32)5; // the host does not recognize the approver for this request
```

<!-- source: coax/lib/nostr/pol/approve.coax approve -->
```jai
pol_admit :: (q:PolRequest,recognized:[]string,decisions:[]PolDecision,consumed:[]PolConsumed,now:int)->u32 {
  known:=false;for a:recognized {if a==q.approver {known=true;}}
  if !known {return POL_NOT_ADMITTED;}
  for c:consumed {if c.request==q.request && c.action==q.action {return POL_REPLAY;}}
  if now>=q.expires_at {return POL_EXPIRED;}
  approved:=false;denied:=false;
  for d:decisions {
    if d.request==q.request && d.action==q.action && d.approver==q.approver && d.expires_at<=q.expires_at && now<d.expires_at {
      if d.approve {approved=true;} else {denied=true;}
    }
  }
  if denied {return POL_DENIED;}
  if approved {return POL_OK;}
  return POL_PENDING;
}
```

Three rules from the NIP decide the order:

- The host must already recognize the approver for this request. Neither
  the request nor the decision can nominate a stranger, and NIP-CTRL
  pairing does not appoint one.
- Approvals are single-use. An exact replay may retrieve the same
  admission receipt but must not dispatch again, so a consumed approval is
  a `replay`, even after the request has expired.
- Conflicting decisions deny pending use, so one counted deny refuses even
  beside an approve.

An approval never replaces dispatch: the host still runs `cap_dispatch`
immediately before it acts, so an approval cannot bypass a hard limit or a
revoked grant.

## Extensions: releases and revocation

A package's ID is `<root-pubkey>:<slug>`. The root key owns the namespace.
A release is a regular kind 3184 event signed by the root, whose body names
the `package`, a `version` label and the `manifest` (an ArtifactRef: the
SHA-256 digest, size and media type of the manifest's exact bytes). The
manifest repeats the package and version, and lists components, files and
dependencies.

<!-- source: coax/lib/nostr/ext/install.coax record -->
```jai
ExtRelease :: struct {event:string;signer:string;kind:int;package:string;version:string;manifest:string;
  manifest_package:string;manifest_version:string;features:[]string;kinds:[]string;dependencies:[]string;}
ExtRevocation :: struct {event:string;signer:string;package:string;release:string;effective_at:int;created_at:int;}
ExtCheckpoint :: struct {signer:string;package:string;revision:int;as_of:int;valid_until:int;revocations:[]string;}
ExtPolicy :: struct {strict_freshness:bool;max_age:int;skew:int;features:[]string;}
```

A release is identified by its event ID and manifest digest, not its
version label. Two releases that bind the same package and version to
different manifests are equivocation. Clients must report a conflict and
must not choose by timestamp.

A revocation is a kind 3185 event signed by the root, with an `e` tag
naming the release and a body with `package`, `release`, `reason` and
`effective_at`, which must not exceed the event's `created_at`. It applies
immediately and forever to that exact release: republishing a listing
cannot undo it.

<!-- source: coax/lib/nostr/ext/install.coax revoked -->
```jai
// A verified revocation of release by its root.
ext_revokes :: (v:ExtRevocation,release:string,root:string,package:string)->bool {
  return v.release==release && v.signer==root && v.package==package && v.effective_at<=v.created_at;
}
ext_revoked :: (revocations:[]ExtRevocation,r:ExtRelease)->bool {
  for v:revocations {if ext_revokes(v,r.event,r.signer,r.package) {return true;}}
  return false;
}
```

A client that asked relays for revocations and got none has learned
nothing: a relay can withhold. So the root also publishes a checkpoint,
addressable kind 30185, with a monotone `revision`, `as_of`, `valid_until`
and the complete sorted list of revocation event IDs. Under strict policy
the client refuses to install unless it holds a current checkpoint.

<!-- source: coax/lib/nostr/ext/install.coax codes -->
```jai
EXT_OK :: cast(u32)0;          // install: stage the closure and commit the lock
EXT_MALFORMED :: cast(u32)1;   // not a 3184, a package id that is not root:slug, or a manifest that disagrees
EXT_NAMESPACE :: cast(u32)2;   // the signer is not the package's root key
EXT_UNSUPPORTED :: cast(u32)3; // a required feature or a component kind this client does not know
EXT_CONFLICT :: cast(u32)4;    // equivocation, or a checkpoint that omits a known revocation
EXT_REVOKED :: cast(u32)5;     // a verified revocation of the release or a dependency
EXT_UNRESOLVED :: cast(u32)6;  // a dependency or a listed revocation the client does not hold
EXT_STALE :: cast(u32)7;       // strict policy: no current checkpoint, or a rolled-back one
```

<!-- source: coax/lib/nostr/ext/install.coax install -->
```jai
ext_install :: (r:ExtRelease,releases:[]ExtRelease,revocations:[]ExtRevocation,c:ExtCheckpoint,have:bool,p:ExtPolicy,now:int)->u32 {
  // 1. A release, its namespace, and a manifest that agrees with it.
  root:=ext_root(r.package);
  if r.kind!=EXT_RELEASE_KIND || root=="" {return EXT_MALFORMED;}
  if r.signer!=root {return EXT_NAMESPACE;}
  if r.manifest_package!=r.package || r.manifest_version!=r.version {return EXT_MALFORMED;}
  // 2. Unknown required features and component kinds refuse; nothing is skipped.
  for f:r.features {if !ext_has(p.features,f) {return EXT_UNSUPPORTED;}}
  known:=EXT_KINDS;
  for k:r.kinds {if !ext_has(known[..],k) {return EXT_UNSUPPORTED;}}
  // 3. Equivocation: never choose between two manifests by timestamp.
  for o:releases {
    if o.signer==root && o.package==r.package && o.version==r.version && o.manifest!=r.manifest {return EXT_CONFLICT;}
  }
  // 4. Revocation of the release, then of each dependency.
  if ext_revoked(revocations,r) {return EXT_REVOKED;}
  for dep:r.dependencies {
    k:=-1;
    for i:0..<releases.count {if releases[i].event==dep {k=i;}}
    if k<0 {return EXT_UNRESOLVED;}
    if ext_revoked(revocations,releases[k]) {return EXT_REVOKED;}
  }
  // 5. The checkpoint: the root's, for this package, complete and resolvable.
  ok:=have && c.signer==root && c.package==r.package;
  if ok {
    for id:c.revocations {
      found:=false;for v:revocations {if v.event==id {found=true;}}
      if !found {return EXT_UNRESOLVED;}
    }
    for v:revocations {
      if v.signer==root && v.package==r.package && v.effective_at<=v.created_at && !ext_has(c.revocations,v.event) {return EXT_CONFLICT;}
    }
  }
  // 6. Strict freshness: an answer with no revocations proves nothing.
  if p.strict_freshness {
    if !ok || c.as_of>now+p.skew || c.valid_until<=c.as_of || now-c.as_of>p.max_age || now>=c.valid_until {return EXT_STALE;}
  }
  return EXT_OK;
}
```

The checks run in this order:

1. The event is a 3184, the package ID is `root:slug`, and the signer is the
   root. A familiar display name or a relay login does not confer a
   namespace.
2. The manifest's package and version equal the release's.
3. Every feature in `requires` is one the client supports, and every
   component kind is one of NIP-EXT's thirteen. This is how these NIPs
   negotiate extensions: there is no optional unknown. The shared contracts
   say an unknown feature, field, enum or bound refuses as
   `unsupported_feature`; only keys inside `meta` may be ignored, and they
   must not change execution.
4. No equivocation among the releases the client has seen.
5. No verified revocation of this release, then of each dependency. A
   held revocation refuses under any policy, with or without a checkpoint.
6. The root's checkpoint for this package must name only revocations the
   client holds (`unresolved` otherwise) and must list every verified
   revocation the client holds (`conflict` otherwise).
7. Under strict freshness: a checkpoint is required, `as_of` may be at most
   `skew` seconds in the future, `valid_until` must follow `as_of`, the age
   is at most `max_age`, and `now` is before `valid_until`. NIP-EXT sets no
   universal duration; this chapter's policy uses 3600 s and 60 s.

A client keeps the newest checkpoint per package. Revisions never go back,
one revision has one content, and a later list keeps every revocation an
earlier one had:

<!-- source: coax/lib/nostr/ext/install.coax checkpoint -->
```jai
// Whether to keep checkpoint c over the one held (have says whether there
// is one) for the same package. Revisions never go back; one revision has
// one content; a later list keeps every revocation an earlier one had.
ext_checkpoint_accept :: (held:ExtCheckpoint,have:bool,c:ExtCheckpoint)->u32 {
  if !have {return EXT_OK;}
  if c.revision<held.revision {return EXT_STALE;}
  for id:held.revocations {if !ext_has(c.revocations,id) {return EXT_CONFLICT;}}
  if c.revision==held.revision && (c.as_of!=held.as_of || c.valid_until!=held.valid_until || c.revocations.count!=held.revocations.count) {return EXT_CONFLICT;}
  return EXT_OK;
}
```

### Worked example: package hello, by hand

The vectors are signed by the root key (scalar 1, `79be667e…`) except one
release signed by scalar 2. T0 = 1790000000.

| Event | Kind | created_at | Body |
| --- | --- | --- | --- |
| release 1, `4cae89f1…` | 3184 | T0 | `79be667e…:hello`, 1.0.0, manifest `sha256:fe89c429…` |
| release 2, `f79cabc2…` | 3184 | T0 + 100 | 1.0.1, manifest `sha256:c10def46…` |
| second 1.0.0, `88f641f2…` | 3184 | T0 + 50 | 1.0.0, manifest `sha256:c10def46…` |
| foreign, `25b7504f…` | 3184 | T0 + 60 | 1.0.0, signed by `c6047f94…` |
| revocation, `3bf81c39…` | 3185 | T0 + 1000 | `e` = release 1, effective_at T0 + 1000 |
| checkpoint, `49fb97cf…` | 30185 | T0 + 1000 | revision 1, as_of T0 + 1000, valid_until T0 + 87400, lists `3bf81c39…` |

At T0 + 1200, with strict freshness (3600 s, 60 s):

| Install | Step that decides | Result |
| --- | --- | --- |
| release 2 | root signs; manifest agrees; no features, kinds `skill` and `guidance`; no other 1.0.1; no revocation names it; the checkpoint lists exactly the held revocation; 1000 ≤ 1200 + 60, 87400 > 1000, 1200 − 1000 = 200 ≤ 3600, 1200 < 87400 | install |
| release 1 | revocation `e` = release 1, signer root, package hello, T0 + 1000 ≤ T0 + 1000 | revoked |
| foreign | the package's first 64 characters are `79be667e…`, the signer is `c6047f94…` | namespace |
| release 1, the second 1.0.0 also seen | 1.0.0 with manifest `fe89…` and 1.0.0 with `c10d…` | conflict |
| release 2 at T0 + 4601 | 4601 − 1000 = 3601 > 3600 | stale |

Release 1 is the second "Done means" line: its revocation is verified, so
it does not install. The order puts equivocation before revocation, so once
the second 1.0.0 is seen, release 1 reports the conflict instead.

## What is not built

- **CAP discovery and probes.** No reader for 30180 heads or 30181
  preference documents, no `t` tag checks, no probe approvals or
  supervisor, and no `prefer` ordering. Presence is given, not observed.
- **Effect scopes and other numeric bounds.** `reads`, `writes` and
  `network` are scope lists in the NIP; here each class is one bit. Only
  `wall_ms` is compared as a number; the other eleven bounds are checked
  for enforcement only. No reservations shared with children, no
  attenuation chain and no effective-authority closure (`reach`).
- **Remote and service bindings.** No `remote` worker or relay hints, no
  decision-service lanes, doors or discovery freshness, and no NIP-X402
  descriptors.
- **POL beyond approvals.** No recipients and disclosure, instruction sets,
  learned preferences, routing receipts or optimization adoption; no
  threshold approvals (NIP-SOV guardians) and no reviews.
- **EXT bodies and packages.** `install.coax` takes fields; the example
  reads them with a small body reader. No manifest file list, path rules or
  size limits, no dependency closure past one level, no listings (30184),
  no namespace migration (3186), no lock commit, updates, rollback or
  uninstall, and no component-set records.
- **NIP-REG.** A registry is an EXT package owned by a curator, with a
  catalog document, Nostr publication and an HTTPS or GitHub mirror. None
  of it is built.
- **Payouts.** A release may carry `fee_msat` and a Spark, Lightning
  address or node-key `payout`; this chapter does not read them.

## The example

[nostr/examples/capabilities-policy.coax](../examples/capabilities-policy.coax)
runs the three parts of the chapter at a fixed clock. The host dispatches
the worked example's requests and more refusals, then consumes one
approval. The client reads the six signed events, recomputes each id,
verifies each BIP-340 signature, reads their bodies, decides which release
installs and which checkpoint it keeps, and finally reads two broken
copies of the revocation.

The events are read with this helper. `as_release` builds the record
`install.coax` takes from the event and the manifest bytes:

<!-- source: nostr/examples/capabilities-policy.coax open -->
```jai
// Reads an event, recomputes its id, verifies its BIP-340 signature, and
// unescapes its content (the body) into the arena.
Opened :: struct {error:int;e:NostrEvent;id:string;signer:string;body:string;}
open_event :: (text:string,pre:[]u8,a:[]Arena)->Opened {
  o:Opened;o.e=nostr_event_read(text,pre,false);o.error=o.e.error;
  if o.error!=NE_OK {return o;}
  if !nostr_fast_verify_vartime(o.e.id[..],o.e.public_key[..],o.e.signature[..]) {o.error=NE_SIGNATURE;return o;}
  o.id=hex_of(a,o.e.id[..]);o.signer=hex_of(a,o.e.public_key[..]);
  n:=json_unescape(text[o.e.content_from..<o.e.content_to],a[0].buf[a[0].at..<a[0].buf.count]);
  o.body=cast(string)a[0].buf[a[0].at..<a[0].at+n];a[0].at+=n;return o;
}
// A 3184 release as install.coax takes it: identity from the event, fields
// from its body and from the manifest bytes it names.
as_release :: (o:Opened,manifest:string,a:[]Arena)->ExtRelease {
  r:ExtRelease;r.event=o.id;r.signer=o.signer;r.kind=o.e.kind;
  r.package=body_get(o.body,"package","",a);r.version=body_get(o.body,"version","",a);
  r.manifest=body_get(o.body,"manifest","digest",a);
  r.manifest_package=body_get(manifest,"package","",a);r.manifest_version=body_get(manifest,"version","",a);
  return r;
}
```

<!-- source: nostr/examples/capabilities-policy.coax main -->
```jai
main :: () {
  print("CAP: one policy, three requests, now = T0 + 100\n");
  defs:[2]CapDefinition;host_definitions(defs[..]);binds:[2]CapBinding;host_bindings(binds[..]);
  grants:[1]CapGrant;host_grants(grants[..]);
  deny:=[1]string.["op:shell.exec"];p:CapPolicy;p.owner=OPERATOR;p.deny=deny[..];p.assurance=CAP_HOST_ENFORCED;p.wall_ms=120000;
  show_cap("A docs.search, reads, 30 s",defs[..],binds[..],grants[..],p,ask_for("op:docs.search",CAP_READS,30000),T0+100);
  show_cap("B docs.search, writes",defs[..],binds[..],grants[..],p,ask_for("op:docs.search",CAP_WRITES,30000),T0+100);
  show_cap("C repo.patch, reads",defs[..],binds[..],grants[..],p,ask_for("op:repo.patch",CAP_READS,30000),T0+100);
  print("more refusals\n");
  show_cap("docs.search, reads, 300 s",defs[..],binds[..],grants[..],p,ask_for("op:docs.search",CAP_READS,300000),T0+100);
  show_cap("docs.search, reads, 0.5 s",defs[..],binds[..],grants[..],p,ask_for("op:docs.search",CAP_READS,500),T0+100);
  show_cap("shell.exec",defs[..],binds[..],grants[..],p,ask_for("op:shell.exec",CAP_PROCESS,30000),T0+100);
  show_cap("web.fetch",defs[..],binds[..],grants[..],p,ask_for("op:web.fetch",CAP_NETWORK,30000),T0+100);
  mem:=ask_for("op:docs.search",CAP_READS,30000);mem.required=CAP_WALL_MS|CAP_MEMORY_BYTES;
  show_cap("docs.search, memory_bytes required",defs[..],binds[..],grants[..],p,mem,T0+100);
  q:=p;q.assurance=CAP_TRUSTED_CONTRACT;
  show_cap("the same, operator trusts the executor's contract",defs[..],binds[..],grants[..],q,mem,T0+100);
  stranger:=ask_for("op:docs.search",CAP_READS,30000);stranger.principal=APPROVER;
  show_cap("docs.search from another key",defs[..],binds[..],grants[..],p,stranger,T0+100);
  show_cap("docs.search at T0 + 3600",defs[..],binds[..],grants[..],p,ask_for("op:docs.search",CAP_READS,30000),T0+3600);
  unprobed:=binds;unprobed[0].presence=CAP_UNPROBED;
  show_cap("docs.search, binding unprobed",defs[..],unprobed[..],grants[..],p,ask_for("op:docs.search",CAP_READS,30000),T0+100);
  eval:=grants;eval[0].purpose=CAP_EVALUATION;eval[0].run="run-7";
  train:=ask_for("op:docs.search",CAP_READS,30000);train.use=CAP_TRAIN;train.run="run-7";
  show_cap("docs.search to train, under an evaluation grant",defs[..],binds[..],eval[..],p,train,T0+100);
  train.use=CAP_RUN;show_cap("the same inside run-7",defs[..],binds[..],eval[..],p,train,T0+100);

  print("POL: approving one action, now = T0 + 100\n");
  ask:PolRequest;ask.request="q-1";ask.action="sha256:ac71";ask.requester=AGENT;ask.approver=APPROVER;ask.expires_at=T0+600;
  who:=[1]string.[APPROVER];yes:[2]PolDecision;
  yes[0].request="q-1";yes[0].action="sha256:ac71";yes[0].approver=APPROVER;yes[0].approve=true;yes[0].expires_at=T0+600;
  spent:[1]PolConsumed;
  print("no decision yet: %\n",POL_WORDS[pol_admit(ask,who[..],yes[0..<0],spent[0..<0],T0+100)]);
  print("approved: %\n",POL_WORDS[pol_admit(ask,who[..],yes[0..<1],spent[0..<0],T0+100)]);
  spent[0].request="q-1";spent[0].action="sha256:ac71";
  print("the same approval again: %\n",POL_WORDS[pol_admit(ask,who[..],yes[0..<1],spent[..],T0+110)]);
  yes[1]=yes[0];yes[1].approve=false;
  print("an approve and a deny, unconsumed: %\n",POL_WORDS[pol_admit(ask,who[..],yes[..],spent[0..<0],T0+100)]);
  print("approved, but the host does not recognize the approver: %\n",POL_WORDS[pol_admit(ask,who[0..<0],yes[0..<1],spent[0..<0],T0+100)]);

  print("EXT: the client reads signed events\n");
  store:[65536]u8;arena:[1]Arena;arena[0].buf=store[..];a:=arena[..];
  pre:[6][8192]u8;
  r1:=open_event(EXT_RELEASE_1,pre[0][..],a);r2:=open_event(EXT_RELEASE_2,pre[1][..],a);
  eq:=open_event(EXT_EQUIVOCATION,pre[2][..],a);fk:=open_event(EXT_FOREIGN,pre[3][..],a);
  rv:=open_event(EXT_REVOCATION,pre[4][..],a);cp:=open_event(EXT_CHECKPOINT,pre[5][..],a);
  for o:[6]Opened.[r1,r2,eq,fk,rv,cp] {print("kind %: %, id %…, signer %…\n",o.e.kind,EVENT_WORDS[o.error],o.id[0..<16],o.signer[0..<16]);}
  rel1:=as_release(r1,EXT_MANIFEST_1,a);rel2:=as_release(r2,EXT_MANIFEST_2,a);
  rele:=as_release(eq,EXT_MANIFEST_2,a);relf:=as_release(fk,EXT_MANIFEST_1,a);
  print("release 1: %…%, %, manifest %\n",rel1.package[0..<8],rel1.package[64..<70],rel1.version,rel1.manifest);
  print("release 2: %…%, %, manifest %\n",rel2.package[0..<8],rel2.package[64..<70],rel2.version,rel2.manifest);
  print("equivocation: %…%, %, manifest %\n",rele.package[0..<8],rele.package[64..<70],rele.version,rele.manifest);
  e:=nostr_tag(rv.e,0);
  v:ExtRevocation;v.event=rv.id;v.signer=rv.signer;v.package=body_get(rv.body,"package","",a);
  v.release=hex_of(a,e.id[..]);v.effective_at=number(body_get(rv.body,"effective_at","",a));v.created_at=rv.e.created_at;
  print("revocation: e tag % (release 1: %), body names %, effective_at T0 + %, reason \"%\"\n",
    v.release[0..<16],v.release==rel1.event,body_get(rv.body,"release","id",a)==v.release,v.effective_at-T0,body_get(rv.body,"reason","",a));
  ids:[4]string;n:=body_ids(cp.body,"revocations",ids[..],a);
  c:ExtCheckpoint;c.signer=cp.signer;c.package=body_get(cp.body,"package","",a);c.revision=number(body_get(cp.body,"revision","",a));
  c.as_of=number(body_get(cp.body,"as_of","",a));c.valid_until=number(body_get(cp.body,"valid_until","",a));c.revocations=ids[0..<n];
  print("checkpoint: revision %, as_of T0 + %, valid_until T0 + %, % revocation(s), lists the revocation: %\n",
    c.revision,c.as_of-T0,c.valid_until-T0,n,n==1 && ids[0]==v.event);

  print("EXT: installing, strict freshness (max age 3600 s, skew 60 s), now = T0 + 1200\n");
  known:=[2]ExtRelease.[rel1,rel2];seen:=[3]ExtRelease.[rel1,rel2,rele];vs:=[1]ExtRevocation.[v];
  fresh:ExtPolicy;fresh.strict_freshness=true;fresh.max_age=3600;fresh.skew=60;
  print("1.0.1: %\n",EXT_WORDS[ext_install(rel2,known[..],vs[..],c,true,fresh,T0+1200)]);
  print("1.0.0: %\n",EXT_WORDS[ext_install(rel1,known[..],vs[..],c,true,fresh,T0+1200)]);
  offline:ExtPolicy;
  print("1.0.0, offline policy, no checkpoint: %\n",EXT_WORDS[ext_install(rel1,known[..],vs[..],c,false,offline,T0+1200)]);
  print("1.0.0 signed by key 2: %\n",EXT_WORDS[ext_install(relf,known[..],vs[..],c,true,fresh,T0+1200)]);
  print("1.0.1 once the second 1.0.0 is seen: %\n",EXT_WORDS[ext_install(rel2,seen[..],vs[..],c,true,fresh,T0+1200)]);
  print("1.0.0 once the second 1.0.0 is seen: %\n",EXT_WORDS[ext_install(rel1,seen[..],vs[..],c,true,fresh,T0+1200)]);
  print("1.0.1, no checkpoint: %\n",EXT_WORDS[ext_install(rel2,known[..],vs[..],c,false,fresh,T0+1200)]);
  print("1.0.1 at T0 + 4601, the checkpoint 3601 s old: %\n",EXT_WORDS[ext_install(rel2,known[..],vs[..],c,true,fresh,T0+4601)]);
  bare:=c;bare.revocations=ids[0..<0];
  print("1.0.1, a checkpoint that omits the revocation: %\n",EXT_WORDS[ext_install(rel2,known[..],vs[..],bare,true,fresh,T0+1200)]);
  print("1.0.1, the revocation not fetched: %\n",EXT_WORDS[ext_install(rel2,known[..],vs[0..<0],c,true,fresh,T0+1200)]);
  x402:=rel2;need:=[1]string.["oa-x402-v1"];x402.features=need[..];
  print("1.0.1 requiring oa-x402-v1: %\n",EXT_WORDS[ext_install(x402,known[..],vs[..],c,true,fresh,T0+1200)]);
  old:=c;old.revision=0;old.revocations=ids[0..<0];
  print("checkpoint revision 0 after revision 1: %\n",EXT_WORDS[ext_checkpoint_accept(c,true,old)]);
  print("revision 1 again without its revocation: %\n",EXT_WORDS[ext_checkpoint_accept(c,true,bare)]);

  print("broken inputs\n");
  t:[4096]u8;s:=EXT_REVOCATION;for i:0..<s.count {t[i]=s[i];}
  // The reason's "1.0.0" becomes "2.0.0": the id no longer matches.
  at:=-1;for i:0..<s.count-5 {if at<0 && s[i..<i+5]=="1.0.0" {at=i;}}
  t[at]='2';bad:=open_event(cast(string)t[0..<s.count],pre[4][..],a);
  print("revocation with its reason changed: %\n",EVENT_WORDS[bad.error]);
  t[at]='1';sig:=0;for i:0..<s.count-6 {if sig==0 && s[i..<i+6]=="\"sig\":" {sig=i+7;}}
  t[sig]=ifx t[sig]=='0' then '1' else '0';bad=open_event(cast(string)t[0..<s.count],pre[4][..],a);
  print("revocation with its first signature digit changed: %\n",EVENT_WORDS[bad.error]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/capabilities-policy.coax --run-steps 250000000
```

Its output, in debug and release builds alike
([nostr/fixtures/capabilities-policy.out](../fixtures/capabilities-policy.out)):

```text
CAP: one policy, three requests, now = T0 + 100
A docs.search, reads, 30 s: dispatch to sha256:b5e1, wall_ms ceiling 30000 set by the request
B docs.search, writes: not_admitted
C repo.patch, reads: no binding
more refusals
docs.search, reads, 300 s: dispatch to sha256:b5e1, wall_ms ceiling 60000 set by the grant
docs.search, reads, 0.5 s: limit_exceeded
shell.exec: denied
web.fetch: no definition
docs.search, memory_bytes required: cannot_enforce
the same, operator trusts the executor's contract: dispatch to sha256:b5e1, wall_ms ceiling 30000 set by the request
docs.search from another key: not_admitted
docs.search at T0 + 3600: expired
docs.search, binding unprobed: unavailable
docs.search to train, under an evaluation grant: not_admitted
the same inside run-7: dispatch to sha256:b5e1, wall_ms ceiling 30000 set by the request
POL: approving one action, now = T0 + 100
no decision yet: pending
approved: approved: consume and dispatch
the same approval again: replay: the same receipt, no dispatch
an approve and a deny, unconsumed: denied
approved, but the host does not recognize the approver: not_admitted
EXT: the client reads signed events
kind 3184: ok, id 4cae89f1451bea38…, signer 79be667ef9dcbbac…
kind 3184: ok, id f79cabc261058058…, signer 79be667ef9dcbbac…
kind 3184: ok, id 88f641f2baaf52df…, signer 79be667ef9dcbbac…
kind 3184: ok, id 25b7504f8695cdfe…, signer c6047f9441ed7d6d…
kind 3185: ok, id 3bf81c39678d1ad6…, signer 79be667ef9dcbbac…
kind 30185: ok, id 49fb97cf337e966a…, signer 79be667ef9dcbbac…
release 1: 79be667e…:hello, 1.0.0, manifest sha256:fe89c4297f4e3d05c4da9b2b104157a90f0cdf16c9a1d64d5cd577f9a254abb6
release 2: 79be667e…:hello, 1.0.1, manifest sha256:c10def463db501098f48b564d19c0453676ea981ba97724e41d0a39c1191c865
equivocation: 79be667e…:hello, 1.0.0, manifest sha256:c10def463db501098f48b564d19c0453676ea981ba97724e41d0a39c1191c865
revocation: e tag 4cae89f1451bea38 (release 1: true), body names true, effective_at T0 + 1000, reason "1.0.0 writes the API key to its log"
checkpoint: revision 1, as_of T0 + 1000, valid_until T0 + 87400, 1 revocation(s), lists the revocation: true
EXT: installing, strict freshness (max age 3600 s, skew 60 s), now = T0 + 1200
1.0.1: install
1.0.0: revoked
1.0.0, offline policy, no checkpoint: revoked
1.0.0 signed by key 2: namespace
1.0.1 once the second 1.0.0 is seen: install
1.0.0 once the second 1.0.0 is seen: conflict
1.0.1, no checkpoint: stale
1.0.1 at T0 + 4601, the checkpoint 3601 s old: stale
1.0.1, a checkpoint that omits the revocation: conflict
1.0.1, the revocation not fetched: unresolved
1.0.1 requiring oa-x402-v1: unsupported
checkpoint revision 0 after revision 1: stale
revision 1 again without its revocation: conflict
broken inputs
revocation with its reason changed: id
revocation with its first signature digit changed: signature
```

Requests A, B and C and the installs match the two worked examples. The
broken inputs are the deliberately wrong cases: changing "1.0.0" to "2.0.0"
in the revocation's reason changes the bytes the id covers, so the reader
refuses it as `id` before any signature check; changing the signature's
first digit leaves the id intact and fails BIP-340 verification as
`signature`. Neither reaches `ext_install`.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/capabilities-policy.coax](../examples/checks/capabilities-policy.coax),
which loads the example. They compare the code with values that share no
code with it:

- The six event ids and the two manifest digests from
  [ext_release_vectors.py](../../coax/tests/nostr/ext_release_vectors.py),
  which signs with
  [nip59_reference.py](../../coax/tests/nostr/nip59_reference.py)'s
  integer secp256k1 and BIP-340 and hashes with `hashlib`. The Coax reader's
  recomputed ids must equal them, every signature must verify, and each
  release must name the SHA-256 of its manifest's exact bytes.
- The worked examples' arithmetic by hand.
- PLAN.md's "Done means": a capability without a matching binding is
  refused at dispatch (and rebinding to the current digest makes it
  dispatchable), and a revoked extension release does not install, under
  offline and strict policy alike.

NIP-CAP, NIP-POL and NIP-EXT publish no event examples or vectors, so the
Python events are the independent check for the wire.

<!-- source: nostr/examples/checks/capabilities-policy.coax check -->
```jai
// From coax/tests/nostr/ext_release_vectors.py, which signs with
// nip59_reference.py's integer secp256k1 and BIP-340 (no code shared with
// Coax): each event's id as hashlib.sha256 of json.dumps([0, pubkey,
// created_at, kind, tags, content], separators=(",", ":")), and each
// manifest's hashlib.sha256 of its exact bytes.
PY_ID :: [6]string.[
  "4cae89f1451bea38a91f5c7cd2561e136118e30a532b072c11f4aae71614e64f",  // release 1.0.0
  "f79cabc2610580581fb63ec7e6404e2f4d392c6477f5fd688cfc060b6b2580f1",  // release 1.0.1
  "88f641f2baaf52df62ade963b16f85b534a35caae11f6f6e0778d059857280d6",  // the second 1.0.0
  "25b7504f8695cdfe6cddb4108b781f5cfc7f0c618b2271eec1706744614e25ee",  // 1.0.0 by key 2
  "3bf81c39678d1ad60686f1cba6813318aa70e645755f5fc1b515c2b291f56310",  // the revocation
  "49fb97cf337e966a2a05450e27d63fc3754ff73687763654c23ae99faeb887be"]; // the checkpoint
PY_MANIFEST_1 :: "sha256:fe89c4297f4e3d05c4da9b2b104157a90f0cdf16c9a1d64d5cd577f9a254abb6";
PY_MANIFEST_2 :: "sha256:c10def463db501098f48b564d19c0453676ea981ba97724e41d0a39c1191c865";

#test "every event's id is the Python reference's, and every signature verifies" {
  texts:=[6]string.[EXT_RELEASE_1,EXT_RELEASE_2,EXT_EQUIVOCATION,EXT_FOREIGN,EXT_REVOCATION,EXT_CHECKPOINT];
  store:[16384]u8;arena:[1]Arena;arena[0].buf=store[..];a:=arena[..];pre:[8192]u8;
  for k:0..<6 {
    o:=open_event(texts[k],pre[..],a);
    #assert o.error==NE_OK && o.id==PY_ID[k] && hex_of(a,o.e.computed_id[..])==PY_ID[k];
  }
}
#test "each release names the digest of its manifest's exact bytes" {
  store:[16384]u8;arena:[1]Arena;arena[0].buf=store[..];a:=arena[..];pre:[8192]u8;
  ms:=[2]string.[EXT_MANIFEST_1,EXT_MANIFEST_2];want:=[2]string.[PY_MANIFEST_1,PY_MANIFEST_2];
  rs:=[2]string.[EXT_RELEASE_1,EXT_RELEASE_2];
  for k:0..<2 {
    m:=ms[k];b:[2048]u8;for i:0..<m.count {b[i]=m[i];}
    d:[32]u8;#assert sha256_hash_checked(b[0..<m.count],d[..],false)==SHA_OK;
    h:=hex_of(a,d[..]);#assert want[k][7..<71]==h;
    r:=as_release(open_event(rs[k],pre[..],a),m,a);
    #assert r.manifest==want[k] && r.manifest_package==r.package && r.manifest_version==r.version;
  }
}

// The worked example by hand, from NIP-CAP's rules: grant g-docs gives the
// agent reads and network on binding b5e1, 60 s, until T0 + 3600; the
// operator's ceiling is 120 s and it accepts only host mechanisms.
#test "the three requests of the worked example" {
  defs:[2]CapDefinition;host_definitions(defs[..]);binds:[2]CapBinding;host_bindings(binds[..]);
  grants:[1]CapGrant;host_grants(grants[..]);
  deny:=[1]string.["op:shell.exec"];p:CapPolicy;p.owner=OPERATOR;p.deny=deny[..];p.wall_ms=120000;
  // A: reads ⊆ {reads, network} ∩ {reads, writes, network}; min(30000, 60000, 120000) = 30000 ≥ 1000;
  // wall_ms is required and the host's supervisor enforces it.
  a:=cap_dispatch(defs[..],binds[..],grants[..],p,ask_for("op:docs.search",CAP_READS,30000),T0+100);
  #assert a.error==CAP_OK && a.binding==0 && a.grant==0 && a.wall_ms==30000 && a.limit==CAP_BY_REQUEST;
  // B: writes = 2; (reads|network) & (reads|writes|network) = 5; 2 & 5 = 0 ≠ 2.
  #assert (CAP_READS|CAP_NETWORK)&(CAP_READS|CAP_WRITES|CAP_NETWORK)==5 && CAP_WRITES&5==0;
  #assert cap_dispatch(defs[..],binds[..],grants[..],p,ask_for("op:docs.search",CAP_WRITES,30000),T0+100).error==CAP_NOT_ADMITTED;
  // C: repo.patch is digest 9c02; the only other binding names 77aa.
  #assert cap_dispatch(defs[..],binds[..],grants[..],p,ask_for("op:repo.patch",CAP_READS,30000),T0+100).error==CAP_NO_BINDING;
}
#test "Done means: a capability without a matching binding is refused at dispatch" {
  defs:[2]CapDefinition;host_definitions(defs[..]);binds:[2]CapBinding;host_bindings(binds[..]);
  grants:[2]CapGrant;host_grants(grants[..]);
  // Even a grant for the stale binding does not help: the binding never matches.
  grants[1]=grants[0];grants[1].binding="sha256:b77a";grants[1].effects=CAP_READS|CAP_WRITES;
  none:[1]string;p:CapPolicy;p.owner=OPERATOR;p.deny=none[0..<0];p.wall_ms=120000;
  #assert cap_dispatch(defs[..],binds[..],grants[..],p,ask_for("op:repo.patch",CAP_WRITES,30000),T0+100).error==CAP_NO_BINDING;
  // Rebinding to the current digest makes it dispatchable.
  binds[1].definition="sha256:9c02";
  r:=cap_dispatch(defs[..],binds[..],grants[..],p,ask_for("op:repo.patch",CAP_WRITES,30000),T0+100);
  #assert r.error==CAP_OK && r.binding==1 && r.grant==1;
}
#test "Done means: a revoked extension release does not install" {
  store:[16384]u8;arena:[1]Arena;arena[0].buf=store[..];a:=arena[..];pre:[8192]u8;
  rel1:=as_release(open_event(EXT_RELEASE_1,pre[..],a),EXT_MANIFEST_1,a);
  rel2:=as_release(open_event(EXT_RELEASE_2,pre[..],a),EXT_MANIFEST_2,a);
  rv:=open_event(EXT_REVOCATION,pre[..],a);
  v:ExtRevocation;v.event=rv.id;v.signer=rv.signer;v.package=body_get(rv.body,"package","",a);
  tag:=nostr_tag(rv.e,0);v.release=hex_of(a,tag.id[..]);v.effective_at=number(body_get(rv.body,"effective_at","",a));v.created_at=rv.e.created_at;
  #assert v.release==PY_ID[0] && v.effective_at==T0+1000 && v.created_at==T0+1000;
  known:=[2]ExtRelease.[rel1,rel2];vs:=[1]ExtRevocation.[v];c:ExtCheckpoint;offline:ExtPolicy;
  #assert ext_install(rel1,known[..],vs[..],c,false,offline,T0+1200)==EXT_REVOKED;
  #assert ext_install(rel1,known[..],vs[0..<0],c,false,offline,T0+1200)==EXT_OK;
  #assert ext_install(rel2,known[..],vs[..],c,false,offline,T0+1200)==EXT_OK;
  // A checkpoint by hand: as_of T0 + 1000, so at T0 + 1200 its age is 200 ≤ 3600;
  // at T0 + 4601 it is 3601 > 3600.
  ids:=[1]string.[PY_ID[4]];c.signer=rv.signer;c.package=v.package;c.revision=1;c.as_of=T0+1000;c.valid_until=T0+87400;c.revocations=ids[..];
  fresh:ExtPolicy;fresh.strict_freshness=true;fresh.max_age=3600;fresh.skew=60;
  #assert ext_install(rel1,known[..],vs[..],c,true,fresh,T0+1200)==EXT_REVOKED;
  #assert ext_install(rel2,known[..],vs[..],c,true,fresh,T0+1200)==EXT_OK && ext_install(rel2,known[..],vs[..],c,true,fresh,T0+4601)==EXT_STALE;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/capabilities-policy.coax --steps 2000000000 --run-steps 250000000
```

`coax test` runs 259 checks, including the loaded files' own checks; all
pass.

### The library's own checks

`dispatch.coax` checks each outcome at its boundary: the six effects and
twelve bounds, the deny list, a binding to an old digest or another
profile, a binding without an invocation contract or `invoke_writing`,
every presence other than `present`, the grant's issuer, principal and
binding, revocation before expiry, expiry at its exact second, the
effects intersection, an evaluation grant outside its run, each ceiling
being the smallest (and the first of equals recorded), and enforcement at
both assurances. `approve.coax` checks recognition, replay after expiry,
each field a decision must match, decision and request expiry, and
conflicting decisions. `install.coax` checks the package ID's shape, the
namespace, manifest agreement, unknown features and kinds, equivocation
and what is not equivocation, which revocations are verified, dependencies,
each checkpoint rule at its boundary second, and checkpoint rollback and
conflict.

```shell
coax/bin/coax test coax/lib/nostr/cap/dispatch.coax --library
coax/bin/coax mutate coax/lib/nostr/cap/dispatch.coax --library
coax/bin/coax test coax/lib/nostr/pol/approve.coax --library
coax/bin/coax mutate coax/lib/nostr/pol/approve.coax --library
coax/bin/coax test coax/lib/nostr/ext/install.coax --library
coax/bin/coax mutate coax/lib/nostr/ext/install.coax --library
```

On 2026-11-01, `dispatch.coax` passed its 11 checks and `coax mutate`
killed all 140 mutants; `approve.coax` passed its 4 checks and killed all
43; `install.coax` passed its 8 checks and killed all 109. None survived
and none is marked equivalent. The full mutation run
(`COAX_MUTATION_FULL=1`) was not run.

## Summary

- A definition describes, a binding runs, a grant permits and presence
  observes. `cap_dispatch` needs all four, checked again at dispatch.
- A binding names one definition digest and profile, so a capability with
  no matching binding is refused, and republishing never moves a binding.
- Effects are granted one at a time and enforced as the intersection of
  grant and definition; the smallest ceiling wins and is recorded; every
  required bound needs a mechanism at the operator's assurance.
- `pol_admit` consumes one approval for one exact action from a recognized
  approver; a replay never dispatches again and any deny refuses.
- `ext_install` refuses a release outside its root's namespace, with an
  unknown feature or kind, under equivocation, or with a verified
  revocation; under strict policy it also needs a current, complete
  checkpoint. A revoked release never installs.

## Dependencies

- [75. Hosts and reachability](ch075-hosts-and-reachability.md)

## Sources

- [OpenAgents NIP-CAP](../nips/openagents/NIP-CAP.md)
- [OpenAgents NIP-POL](../nips/openagents/NIP-POL.md)
- [OpenAgents NIP-EXT](../nips/openagents/NIP-EXT.md)
- [OpenAgents NIP-REG](../nips/openagents/NIP-REG.md)
- [OpenAgents shared contracts](../nips/openagents/contracts.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
