# 87. Owner attestation and agent authentication

An agent runs with its own key, but someone owns it. This chapter builds the
two Block NIPs that say so: NIP-OA, a tag by which an owner key authorizes
an agent key, and NIP-AA, which lets a relay admit that agent because its
owner is a member.

Part X, Every NIP. Result: Block's NIP-OA `auth` tag by which an owner key
authorizes an agent key, and NIP-AA agent authentication to relays with
virtual member privileges and revocation.

## What this chapter teaches

- NIP-OA: the tag `["auth", owner, conditions, sig]`, the signed preimage,
  the conditions grammar, and why the conditions string is hashed exactly
  as the tag carries it.
- The agent stays the author. A valid tag adds "authorized by", never a
  change of author. This is the difference from NIP-26 (chapter 70).
- NIP-AA: the relay's six steps on a NIP-42 AUTH event, which of them
  answer `invalid:` and which `restricted:`, and why `kind=` clauses are not
  evaluated at admission.
- Virtual membership: what it allows (relay-level reads and writes), what
  it never allows (administration, membership changes), and how it ends.
- Key possession is separate from membership. NIP-42 proves the agent holds
  its key; only the owner's active membership lets it in.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nipoa.coax](../../coax/lib/nostr/nipoa.coax) | `noa_conditions(text, facts)` | Reads the conditions and says which kinds of clause hold (`#authority`) |
| | `noa_preimage`, `noa_hash`, `noa_write_tag` | The owner's side: the signed message and the tag |
| | `noa_read(e)`, `noa_decide`, `noa_signed` | The tag read up to the signature, then decided with the signature's verdict |
| | `noa_check(e)`, `noa_attribute(e)` | The whole NIP-OA check on one verified event, and the author with an optional "authorized by" |
| [nipaa.coax](../../coax/lib/nostr/nipaa.coax) | `naa_auth(conn, m, n, gen, now, text, scratch)` | One AUTH event's JSON through NIP-AA's six steps (`#authority`) |
| | `naa_auth_fields`, `naa_reply` | The steps on a verified event and a signature verdict; the `OK` message |
| | `naa_add`, `naa_remove`, `naa_member` | The relay's active member list |
| | `naa_event`, `naa_read`, `naa_admin`, `naa_end` | Virtual member privileges, and ending sessions by owner |
| [nip42.coax](../../coax/lib/nostr/nip42.coax) | `nip42_admit_fields` | Chapter 38's check, which step 1 calls unchanged |

The helpers and data for the checks are in
`coax/tests/nostr/nipoa_fixtures.coax` and `nipaa_fixtures.coax`, so that
mutation changes the library and not the checks.

## The owner's attestation (NIP-OA)

The owner signs one string: the domain `nostr:agent-auth:`, the agent's
public key as 64 lowercase hex digits, a colon, and the conditions. The
signature is BIP-340 over the SHA-256 of that string, with the owner's key.
The agent then puts the tag on its own events:

```jai
["auth", "<owner-pubkey-hex>", "<conditions>", "<sig-hex>"]
```

The tag is a reusable capability. The same tag can sit on many events by
the same agent, as long as each event meets the conditions.

### The record

`noa_read` fills a claim: whom the tag names, the claimed signature, the
conditions as signed, and the hash the signature must cover. `noa_attribute`
returns the event's author, which is always `event.pubkey`, and the owner
only when the tag verifies.

<!-- source: coax/lib/nostr/nipoa.coax record -->
```jai
NoaClaim :: struct {
  status:int=NOA_EVENT;
  owner:[32]u8;                            // whom the tag names
  sig:[64]u8;                              // the owner's claimed signature
  conditions:[NOA_MAX_CONDITIONS]u8; n:int;  // exactly as signed
  hash:[32]u8;                             // sha256 of the preimage
}
// The author is always the agent. owner is set only when the tag verifies.
NoaAttribution :: struct {author:[32]u8; authorized:bool; owner:[32]u8; status:int;}
// The event fields clauses are evaluated against, and what held.
NoaFacts :: struct {kind:int; created_at:int;}
NoaHeld :: struct {read:bool; kinds:bool; times:bool;}
```

### Conditions

The conditions string is empty, or clauses joined by `&`. A clause is
`kind=N` with N from 0 to 65535, `created_at<T` or `created_at>T` with T
from 0 to 4294967295. Numbers are canonical decimal: no sign and no leading
zero except `0` itself. An empty clause, whitespace, a capital letter,
another field or operator, or a number out of range refuses the whole tag.

Every clause must hold. So `kind=1&kind=7` authorizes no event: no event has
two kinds. This is not NIP-26's rule, where kind clauses are alternatives.

`noa_conditions` reads every clause and reports separately whether the
kind clauses held and whether the time clauses held. The caller decides
which count. NIP-OA evaluates both; NIP-AA's admission evaluates only the
time clauses.

<!-- source: coax/lib/nostr/nipoa.coax conditions -->
```jai
// s as a canonical decimal number from 0 to max, or -1.
noa_number :: (s:string,max:int)->int {
  if s.count==0 || (s[0]=='0' && s.count>1) {return -1;}
  v:=0;
  for c:s {
    if c<'0' || c>'9' {return -1;}
    v=v*10+cast(int)(c-'0');
    if v>max {return -1;}
  }
  return v;
}
// Whether the conditions read, and whether an event of facts' kind and
// created_at meets every kind= clause and every created_at clause. Every
// clause is read; the caller decides which kinds of clause count.
noa_conditions :: (text:string,facts:NoaFacts)->NoaHeld {
  h:=NoaHeld.{read=true,kinds=true,times=true};bad:NoaHeld;
  if text.count==0 {return h;}
  from:=0;
  while from<=text.count {
    i:=from;while i<text.count && text[i]!='&' {i+=1;}
    c:=text[from..<i];from=i+1;
    // The name ends at the first operator.
    p:=0;while p<c.count && c[p]!='=' && c[p]!='<' && c[p]!='>' {p+=1;}
    if p==c.count {return bad;}
    name:=c[0..<p];op:=c[p];max:=NOA_MAX_TIME;
    if name=="kind" && op=='=' {max=NOA_MAX_KIND;}
    else if name!="created_at" || op=='=' {return bad;}
    v:=noa_number(c[p+1..<c.count],max);
    if v<0 {return bad;}
    if op=='=' {if facts.kind!=v {h.kinds=false;}}
    else if op=='<' {if facts.created_at>=v {h.times=false;}}
    else if facts.created_at<=v {h.times=false;}
  }
  return h;
}
```

The number reader stops as soon as the value passes its maximum, so a long
string of digits can never overflow into a small value.

### The preimage

<!-- source: coax/lib/nostr/nipoa.coax preimage -->
```jai
// "nostr:agent-auth:<agent hex>:<conditions>" into out: its length, or -1.
noa_preimage :: (agent:[32]u8,conditions:string,out:[]u8)->int {
  if out.count<NOA_PREFIX.count+65+conditions.count {return -1;}
  key:=agent;hex:[64]u8;h:=hex_encode_checked(cast(string)key[..],hex[..],false);
  for i:0..<NOA_PREFIX.count {out[i]=NOA_PREFIX[i];}
  n:=NOA_PREFIX.count;
  for i:0..<64 {out[n+i]=hex[i];}
  n+=64;out[n]=':';n+=1;
  for i:0..<conditions.count {out[n+i]=conditions[i];}
  return n+conditions.count;
}
// The message the owner signs: sha256 of the preimage, into out (32
// bytes). False when the conditions are longer than NOA_MAX_CONDITIONS.
noa_hash :: (agent:[32]u8,conditions:string,out:[]u8)->bool {
  if conditions.count>NOA_MAX_CONDITIONS {return false;}
  text:[NOA_TEXT_MAX]u8;
  n:=noa_preimage(agent,conditions,text[..]);
  return sha256_hash_checked(text[0..<n],out,false)==SHA_OK;
}
```

The conditions go into the preimage byte for byte. NIP-OA forbids
reordering, deduplicating or normalizing them, and the code has no way to:
it hashes the unescaped tag value as it is. Reordering two clauses gives
another hash, so the owner's signature no longer verifies.

### Reading and deciding

`noa_read` checks everything up to the signature, in NIP-OA's order:

1. The event is verified (event.coax checked its id and signature). A tag on
   an invalid event establishes nothing.
2. Exactly one `auth` tag. Two tags mean no valid tag.
3. Exactly four elements.
4. The owner key and the signature are lowercase hex of 32 and 64 bytes.
   The values are JSON-unescaped first, so `7` reads as `7`.
5. The owner is not the event's own key: no self-attestation.
6. The conditions read, in at most 256 bytes.

<!-- source: coax/lib/nostr/nipoa.coax read -->
```jai
// A tag value unescaped and read as lowercase hex of out.count bytes.
noa_hex :: (raw:string,out:[]u8)->bool {
  buf:[128]u8;n:=json_unescape(raw,buf[..]);
  return n!=-1 && ne_lower_hex(cast(string)buf[0..<n],out);
}
noa_status :: (c:NoaClaim,status:int)->NoaClaim {r:=c;r.status=status;return r;}
// The event's auth tag read and checked up to the signature: NOA_UNCHECKED
// with the owner, the signature, the conditions and the hash, or the first
// refusal.
noa_read :: (e:NostrEvent)->NoaClaim {
  c:NoaClaim;
  if e.error!=NE_OK || !e.verified {return c;}
  ev:=nt_event_of(e);
  count:=nt_tags(ev,"auth");
  if count<0 {return c;}
  if count==0 {return noa_status(c,NOA_NONE);}
  if count>1 {return noa_status(c,NOA_REPEATED);}
  t:=nt_tag(ev,"auth",0);
  if t.values!=3 {return noa_status(c,NOA_SHAPE);}
  if !noa_hex(nt_raw(ev,t.v[0]),c.owner[..]) || !noa_hex(nt_raw(ev,t.v[2]),c.sig[..]) {return noa_status(c,NOA_HEX);}
  if ns_equal32(c.owner,e.public_key) {return noa_status(c,NOA_SELF);}
  c.n=json_unescape(nt_raw(ev,t.v[1]),c.conditions[..]);
  if c.n<0 {c.n=0;return noa_status(c,NOA_CONDITIONS);}
  conditions:=cast(string)c.conditions[0..<c.n];
  if !noa_conditions(conditions,NoaFacts.{}).read {return noa_status(c,NOA_CONDITIONS);}
  if !noa_hash(e.public_key,conditions,c.hash[..]) {return c;}
  return noa_status(c,NOA_UNCHECKED);
}
```

`noa_decide` takes the read claim and the signature's verdict, so the
expensive Schnorr verification is done once by `noa_signed` and the
decision stays cheap to check:

<!-- source: coax/lib/nostr/nipoa.coax decide -->
```jai
// A read claim decided with the signature's verdict (signed) against an
// event of this kind and time; kinds and times say which clauses count.
noa_decide :: (c:NoaClaim,signed:bool,kind:int,created_at:int,kinds:bool,times:bool)->int {
  if c.status!=NOA_UNCHECKED {return c.status;}
  if !signed {return NOA_SIGNATURE;}
  d:=c;h:=noa_conditions(cast(string)d.conditions[0..<d.n],NoaFacts.{kind=kind,created_at=created_at});
  if (kinds && !h.kinds) || (times && !h.times) {return NOA_OUTSIDE;}
  return NOA_OK;
}
// Whether the owner signed the claim's hash. Only a claim that read.
noa_signed :: (c:NoaClaim)->bool {
  d:=c;return d.status==NOA_UNCHECKED && nostr_verify_vartime(d.hash[..],d.owner[..],d.sig[..]);
}
// The whole NIP-OA check on one verified event: every clause evaluated.
noa_check :: (e:NostrEvent)->NoaClaim {
  c:=noa_read(e);
  return noa_status(c,noa_decide(c,noa_signed(c),e.kind,e.created_at,true,true));
}
// The event's author is its signer. The owner is reported only as the key
// that authorized it, and only when the tag verifies.
noa_attribute :: (e:NostrEvent)->NoaAttribution {
  c:=noa_check(e);a:=NoaAttribution.{author=e.public_key,status=c.status};
  if c.status==NOA_OK {a.authorized=true;a.owner=c.owner;}
  return a;
}
```

Nothing here reads a clock. NIP-OA says verification must not depend on
the verifier's clock: a `created_at<T` clause constrains the event's own
`created_at`, which the agent chooses. The owner revokes future authority
only by not issuing new tags, or through a relay that enforces freshness,
which is what NIP-AA adds.

### The owner's side

The owner writes the tag after signing `noa_hash`'s message. The writer
refuses conditions that don't read, so an owner can't publish a tag every
verifier must reject:

<!-- source: coax/lib/nostr/nipoa.coax write -->
```jai
// ["auth",owner,conditions,sig] into out at n, for an owner who signed
// noa_hash's message: the new length, or -1 when the conditions don't read
// or out is small.
noa_write_tag :: (owner:[32]u8,conditions:string,sig:[64]u8,out:[]u8,at:int)->int {
  if !noa_conditions(conditions,NoaFacts.{}).read {return -1;}
  o:=owner;s:=sig;ohex:[64]u8;shex:[128]u8;
  h:=hex_encode_checked(cast(string)o[..],ohex[..],false);
  h=hex_encode_checked(cast(string)s[..],shex[..],false);
  n:=json_raw(out,at,"[\"auth\",\"");n=json_raw(out,n,cast(string)ohex[..]);n=json_raw(out,n,"\",");
  n=json_write_string(out,n,conditions);
  n=json_raw(out,n,",\"");n=json_raw(out,n,cast(string)shex[..]);return json_raw(out,n,"\"]");
}
```

## Agent authentication (NIP-AA)

A relay that keeps a member list (NIP-43, chapter 67) admits only members.
Without NIP-AA, an operator who adds a person must also enroll every agent
that person runs, and remove them again when the person leaves. With
NIP-AA, the agent answers the relay's NIP-42 challenge with its owner's
`auth` tag in the kind 22242 event, and the relay checks the owner instead.

### The state

The member list is the relay's own authoritative state, not a 13534 event.
Each connection holds the keys authenticated on it. A virtual member's
entry keeps the owner and the conditions of the tag that admitted it, for
ending sessions by owner and for optional kind enforcement. Nothing about
the agent outlives the connection.

<!-- source: coax/lib/nostr/nipaa.coax record -->
```jai
// The relay's active members: its authoritative state.
NaaMembers :: struct {keys:[NAA_MAX_MEMBERS][32]u8; n:int;}
// One authenticated key on a connection: a member, or a virtual member with
// the owner and the conditions of the tag that admitted it.
NaaKey :: struct {key:[32]u8; derived:bool; owner:[32]u8; conditions:[NOA_MAX_CONDITIONS]u8; n:int;}
NaaConnection :: struct {k:[NAA_MAX_KEYS]NaaKey; n:int;}
// One AUTH's answer: the result, the step's detail, and the keys.
NaaAuth :: struct {code:int; detail:int; agent:[32]u8; owner:[32]u8;}
```

<!-- source: coax/lib/nostr/nipaa.coax members -->
```jai
naa_find_member :: (m:NaaMembers,key:[32]u8)->int {
  c:=m;for i:0..<c.n {if ns_equal32(c.keys[i],key) {return i;}}
  return -1;
}
naa_member :: (m:NaaMembers,key:[32]u8)->bool {return naa_find_member(m,key)>=0;}
// Adds key once; false when the list is full.
naa_add :: (m:[]NaaMembers,key:[32]u8)->bool {
  if naa_member(m[0],key) {return true;}
  if m[0].n==NAA_MAX_MEMBERS {return false;}
  m[0].keys[m[0].n]=key;m[0].n+=1;return true;
}
// Revokes key's membership; false when it wasn't a member.
naa_remove :: (m:[]NaaMembers,key:[32]u8)->bool {
  i:=naa_find_member(m[0],key);
  if i<0 {return false;}
  m[0].n-=1;m[0].keys[i]=m[0].keys[m[0].n];return true;
}
```

### The six steps

`naa_auth_fields` runs NIP-AA's steps in order. The first failure answers,
and a failed AUTH changes nothing on the connection.

1. **NIP-42.** Chapter 38's `nip42_admit_fields`: the event is verified,
   kind 22242, with this relay's `relay` tag and this connection's
   challenge, within NIP-42's 600 seconds. Then NIP-AA's stricter window:
   `created_at` within 120 seconds of the relay's clock. Failures answer
   `invalid:`.
2. **Direct membership.** A member is admitted as itself. Its tags are not
   read.
3. **One `auth` tag.** None, or more than one, refuses: `restricted:`.
4. **The credential.** `noa_read`'s checks, the owner's signature, and the
   AUTH event's `created_at` against the time clauses. `kind=` clauses are
   read but not evaluated: the AUTH event is kind 22242, and NIP-AA admits
   at the connection level.
5. **The owner.** The owner must be an active member: `restricted:`.
6. **Virtual membership** for the agent's key on this connection. A new
   AUTH by the same key replaces its entry; it never merges two
   credentials.

<!-- source: coax/lib/nostr/nipaa.coax auth -->
```jai
naa_find :: (c:NaaConnection,key:[32]u8)->int {
  d:=c;for i:0..<d.n {if ns_equal32(d.k[i].key,key) {return i;}}
  return -1;
}
// Step 6 (and step 2's record): the key's entry replaced, or added.
naa_grant :: (conn:[]NaaConnection,a:NaaAuth,entry:NaaKey,code:int)->NaaAuth {
  r:=a;i:=naa_find(conn[0],entry.key);
  if i<0 {
    if conn[0].n==NAA_MAX_KEYS {r.code=NAA_FULL;return r;}
    i=conn[0].n;conn[0].n+=1;
  }
  conn[0].k[i]=entry;r.code=code;return r;
}
// One AUTH event on a connection: e is the event (verified by event.coax),
// c is noa_read(e), and signed is noa_signed(c). naa_auth passes them.
naa_auth_fields :: (conn:[]NaaConnection,m:NaaMembers,n:[]Nip42,gen:int,now:int,e:NostrEvent,c:NoaClaim,signed:bool)->NaaAuth {
  a:NaaAuth;a.code=NAA_NIP42;key:=e.public_key;
  x:=nip42_admit_fields(n,gen,now,e.error==NE_OK && e.verified,e.kind,e.created_at,e.original[e.tags_from..<e.tags_to],key[..]);
  a.detail=x.error;
  if x.error!=N42_OK {return a;}
  a.code=NAA_STALE;
  if e.created_at<now-NAA_WINDOW || e.created_at>now+NAA_WINDOW {return a;}
  a.agent=key;entry:NaaKey;entry.key=key;
  if naa_member(m,key) {return naa_grant(conn,a,entry,NAA_MEMBER);}
  a.code=NAA_NO_AUTH;
  if c.status==NOA_NONE {return a;}
  a.code=NAA_REPEATED;
  if c.status==NOA_REPEATED {return a;}
  a.code=NAA_CREDENTIAL;a.detail=noa_decide(c,signed,e.kind,e.created_at,false,true);
  if a.detail!=NOA_OK {return a;}
  a.code=NAA_OWNER;a.owner=c.owner;
  if !naa_member(m,c.owner) {return a;}
  entry.derived=true;entry.owner=c.owner;entry.conditions=c.conditions;entry.n=c.n;
  return naa_grant(conn,a,entry,NAA_VIRTUAL);
}
// The relay side for an AUTH event's JSON: event.coax checks its id and
// signature, nipoa.coax reads its auth tag and checks the owner's
// signature, then naa_auth_fields decides. scratch holds the id preimage.
naa_auth :: (conn:[]NaaConnection,m:NaaMembers,n:[]Nip42,gen:int,now:int,text:string,scratch:[]u8)->NaaAuth {
  e:=nostr_event_verify_vartime(text,scratch,false);c:=noa_read(e);
  return naa_auth_fields(conn,m,n,gen,now,e,c,noa_signed(c));
}
// ["OK",<id>,true,""] or ["OK",<id>,false,<reason>] into out: its length,
// or -1 when out is small.
naa_reply :: (a:NaaAuth,id_hex:string,out:[]u8)->int {
  k:=json_raw(out,0,"[\"OK\",");k=json_write_string(out,k,id_hex);
  k=json_raw(out,k,ifx a.code<=NAA_VIRTUAL then ",true," else ",false,");
  k=json_write_string(out,k,NAA_REASONS[a.code]);return json_raw(out,k,"]");
}
```

The NIP-42 freshness window is what makes `created_at<T` an expiry. The
agent chooses `created_at`, but the relay refuses an AUTH event more than
120 seconds from its clock. So a tag with `created_at<T` can admit the agent
only while the relay's clock is before `T + 120`.

### What a virtual member may do

<!-- source: coax/lib/nostr/nipaa.coax privileges -->
```jai
// An EVENT whose pubkey is key, of this kind: the key must be authenticated
// on the connection (as a member or a virtual member). With enforce, a
// virtual member's event must meet every kind= clause of its credential;
// created_at clauses were evaluated at admission and are not again.
naa_event :: (c:NaaConnection,key:[32]u8,kind:int,enforce:bool)->int {
  i:=naa_find(c,key);
  if i<0 {return NAA_E_UNAUTHENTICATED;}
  d:=c;k:=d.k[i];
  if enforce && !noa_conditions(cast(string)k.conditions[0..<k.n],NoaFacts.{kind=kind}).kinds {return NAA_E_KIND;}
  return NAA_E_OK;
}
// REQ, COUNT and other reads: at least one key on the connection holds
// active or virtual membership.
naa_read :: (c:NaaConnection)->bool {return c.n>0;}
// Relay administration and membership changes: never by a virtual member.
// NAA_E_OK only for an authenticated member key; whether that member is an
// admin is the relay's own list.
naa_admin :: (c:NaaConnection,key:[32]u8)->int {
  i:=naa_find(c,key);
  if i<0 {return NAA_E_UNAUTHENTICATED;}
  d:=c;return ifx d.k[i].derived then NAA_E_VIRTUAL else NAA_E_OK;
}
// Ends every entry on the connection that owner's membership admitted: its
// own, and its agents'. The count removed. The relay calls it on each open
// connection when it revokes owner and wants sessions to end now.
naa_end :: (conn:[]NaaConnection,owner:[32]u8)->int {
  removed:=0;i:=0;
  while i<conn[0].n {
    k:=conn[0].k[i];
    if ns_equal32(k.key,owner) || (k.derived && ns_equal32(k.owner,owner)) {
      conn[0].n-=1;conn[0].k[i]=conn[0].k[conn[0].n];removed+=1;
    } else {i+=1;}
  }
  return removed;
}
```

- An `EVENT` must come from a key authenticated on the connection, as a
  member or a virtual member.
- With the relay's optional kind enforcement on, every `kind=` clause of the
  credential must hold for each event. The time clauses were evaluated at
  admission and are not evaluated again.
- Reads pass when at least one key on the connection holds membership.
- Relay administration and membership changes refuse a virtual member.
  Channel, group and role checks use the agent's own key, never the
  owner's; they belong to the chapters that build them.

### Revocation

Virtual membership is checked on each AUTH, never cached across
connections. When the operator removes the owner, every agent of that owner
fails step 5 on its next AUTH. An open session continues until the
connection closes, as NIP-AA says; a relay that wants it to end now calls
`naa_end(conn, owner)` on each open connection, which removes the owner's
own entry and every entry it admitted.

## Worked example: one attestation chain by hand

The keys are NIP-OA's test keys: the owner's secret is 1 and the agent's
is 2. Never sign with a real key in an example.

| Key | x-only public key |
| --- | --- |
| Owner (secret 1) | `79be667e...16f81798` (the generator's x) |
| Agent (secret 2) | `c6047f94...5c709ee5` (x of 2·G) |

### Step 1: the preimage

The conditions are `kind=1&created_at<1713957000`: `kind=1` (6 bytes), `&`
(1) and `created_at<1713957000` (11 + 10 = 21), 28 bytes.

| Part | Bytes |
| --- | ---: |
| `nostr:agent-auth:` | 17 |
| The agent's key in hex | 64 |
| `:` | 1 |
| The conditions | 28 |
| Total | 110 |

Its SHA-256 is `08cdecd5...197069a6`, NIP-OA's published value. The owner
signs that hash. The tag is `["auth","` (9 bytes), the owner key (64),
`","` (3), the conditions (28), `","` (3), the signature (128) and `"]`
(2): 237 bytes, NIP-OA's tag line.

### Step 2: the agent's kind 1 event

NIP-OA's signed event example is kind 1, `created_at = 1713956400`,
signed by the agent. Its id and signature verify. It has one `auth` tag of
four elements; the owner is not the agent; both clauses read. The owner's
signature verifies over `08cdecd5...`. `kind=1` holds, and
`1713956400 < 1713957000` holds. The event is authored by `c6047f94...` and
authorized by `79be667e...`.

### Step 3: the AUTH event

The relay `wss://relay.example.com` sends `["AUTH","challenge-87"]`. The
agent signs a kind 22242 event at `created_at = 1713956400` with the tags
`relay`, `challenge` and the same `auth` tag. The relay's clock reads
`now = 1713956430`, and the owner is its only member.

1. NIP-42: kind 22242, the relay and challenge match,
   `|1713956400 − 1713956430| = 30 ≤ 600`. Freshness: `30 ≤ 120`.
2. The agent is not a member: continue.
3. One `auth` tag.
4. The tag reads and the owner's signature verifies. `kind=1` is not
   evaluated. `1713956400 < 1713957000` holds.
5. The owner is a member.
6. The agent gets virtual membership, with owner `79be667e...` kept.

The answer is `["OK","<id>",true,""]`. NIP-OA's own check of the same AUTH
event refuses as outside the conditions, because it evaluates `kind=1`
against kind 22242. This is the one place the two NIPs deliberately
differ.

### Step 4: where it stops

- At `created_at = 1713957001` with a matching clock, step 1 passes and
  step 4 refuses: `1713957001 < 1713957000` is false.
- An AUTH event 121 seconds old refuses at step 1 as `invalid:`.
- After the operator removes the owner, the same agent's next AUTH on a new
  connection passes steps 1 to 4 and refuses at step 5.

## The example

The example is
[nostr/examples/agent-auth.coax](../examples/agent-auth.coax). It computes
the worked values, verifies NIP-OA's published signature and signed event,
and signs every other event itself: the refusals, the AUTH events and the
revocation. Each refused event is signed by the agent, so its own id and
signature verify and only the `auth` tag is wrong.

<!-- source: nostr/examples/agent-auth.coax example -->
```jai
#load "../../coax/lib/nostr/nipaa.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
hex_of :: (bytes: [] u8, out: [] u8) -> string {
  r := hex_encode_checked(cast(string) bytes, out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<r.written];
}
// A secret key of 31 zero bytes and then b: NIP-OA's test keys are 1 and 2.
// Never sign with a real key in an example.
test_key :: (b: u8) -> [32] u8 { k: [32] u8; k[31] = b; return k; }
// The x-only public key of a secret: x(k·G).
public_x :: (secret: [] u8) -> [32] u8 {
  k := sc_reduce_bytes(secret).value;
  a := sg_to_affine(smc_multiply(k, sg_generator()).value).value;
  b: [32] u8;
  #assert sf_store(a.x, b[..]);
  return b;
}
// BIP-340 signing of a 32-byte message with chapter 13's signer.
sign_hash :: (key: [] u8, aux: [] u8, message: [] u8, out: [] u8) {
  work: [1] SignWork; status := SIGN_MORE;
  while status == SIGN_MORE { status = schnorr_sign_step(key, aux, message, work[..], out, false); }
  #assert status == SIGN_DONE;
}
// A signed event with these fields, into ev.
sign_event :: (key: [] u8, aux: [] u8, created_at: int, kind: int, tags: string, content: string, ev: [] u8) -> string {
  work: [1] NostrSignWork; scratch: [2048] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key, aux, created_at, kind, tags, content, work[..], scratch[..], ev, false);
  }
  #assert n > 0;
  return cast(string) ev[0..<n];
}
// Append text to out at n; the new length.
put :: (out: [] u8, at: int, text: string) -> int {
  n := at;
  for c: text { out[n] = c; n += 1; }
  return n;
}
// [["relay",RELAY],["challenge",challenge]] and then the extra tags.
auth_tags :: (challenge: string, extra: string, out: [] u8) -> string {
  n := put(out, 0, "[[\"relay\",\""); n = put(out, n, RELAY); n = put(out, n, "\"],[\"challenge\",\"");
  n = put(out, n, challenge); n = put(out, n, "\"]"); n = put(out, n, extra); n = put(out, n, "]");
  return cast(string) out[0..<n];
}
NOA_WORDS :: [11] string.["ok", "not a verified event", "no auth tag", "more than one auth tag", "not four elements",
                          "not lowercase hex", "the owner is the event's own key", "the conditions don't read",
                          "not the owner's signature", "outside the conditions", "unchecked"];
// The NIP-OA check on a signed event's JSON.
check :: (label: string, text: string) {
  scratch: [2048] u8;
  e := nostr_event_verify_vartime(text, scratch[..], false);
  print("%: %\n", label, NOA_WORDS[noa_check(e).status]);
}
// The agent answers a challenge on a connection; the relay decides.
login :: (label: string, conn: [] NaaConnection, m: NaaMembers, challenge: string, created_at: int, now: int, extra: string) -> NaaAuth {
  agent := test_key(2); aux: [32] u8;
  n: [1] Nip42; frame: [64] u8;
  #assert nip42_init(n[..], RELAY) == N42_OK && nip42_issue(n[..], 1, challenge, frame[..]).error == N42_OK;
  tb: [1024] u8; ev: [2048] u8; scratch: [2048] u8; out: [256] u8;
  text := sign_event(agent[..], aux[..], created_at, 22242, auth_tags(challenge, extra, tb[..]), "", ev[..]);
  a := naa_auth(conn, m, n[..], 1, now, text, scratch[..]);
  e := nostr_event_read(text, scratch[..], false); hex: [64] u8;
  k := naa_reply(a, hex_of(e.id[..], hex[..]), out[..]);
  print("%: %\n", label, cast(string) out[0..<k]);
  if a.code == NAA_CREDENTIAL { print("  the tag: %\n", NOA_WORDS[a.detail]); }
  return a;
}
E_WORDS :: [4] string.["accepted", "restricted: not authenticated", "restricted: kind not in the credential", "restricted: virtual member"];

RELAY :: "wss://relay.example.com";
CONDS :: "kind=1&created_at<1713957000";
T :: 1713956400;
// NIP-OA's published signature and signed event example.
NIP_SIG :: "8b7df2575caf0a108374f8471722b233c53f9ff827a8b0f91861966c3b9dd5cb2e189eae9f49d72187674c2f5bd244145e10ff86c9f257ffe65a1ee5f108b369";
NIP_EVENT :: "{\"id\":\"d892a65e7677e0554ebb70ee16deeb6a0727dba46450fb4bc001291d7bff971b\",\"pubkey\":\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\",\"created_at\":1713956400,\"kind\":1,\"tags\":[[\"auth\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"kind=1&created_at<1713957000\",\"8b7df2575caf0a108374f8471722b233c53f9ff827a8b0f91861966c3b9dd5cb2e189eae9f49d72187674c2f5bd244145e10ff86c9f257ffe65a1ee5f108b369\"]],\"content\":\"owner-attested agent event\",\"sig\":\"7fd38992b70b5e9e113644e51b4c8ee2227f3bdd402b1855f8786c0600394ab3ec2621742a7bad0b0000b93d4d1ae6e39525f286a3c1029f43f46c3359a6c76f\"}";

main :: () {
  owner_key := test_key(1); agent_key := test_key(2); other_key := test_key(3); aux: [32] u8;
  owner := public_x(owner_key[..]); agent := public_x(agent_key[..]);
  other := public_x(other_key[..]);
  h1: [64] u8; h2: [64] u8; h3: [64] u8;
  print("NIP-OA: the owner's attestation\n");
  print("owner (secret 1): %\n", hex_of(owner[..], h1[..]));
  print("agent (secret 2): %\n", hex_of(agent[..], h2[..]));
  pre: [NOA_TEXT_MAX] u8; n := noa_preimage(agent, CONDS, pre[..]);
  print("preimage (% bytes): %\n", n, cast(string) pre[0..<n]);
  hash: [32] u8; #assert noa_hash(agent, CONDS, hash[..]);
  print("sha256(preimage): %\n", hex_of(hash[..], h3[..]));
  nip_sig: [64] u8; decode_hex(NIP_SIG, nip_sig[..]);
  print("NIP-OA's auth_sig verifies under the owner: %\n", nostr_verify_vartime(hash[..], owner[..], nip_sig[..]));
  mine: [64] u8; sign_hash(owner_key[..], aux[..], hash[..], mine[..]); s: [128] u8;
  print("the owner signs again, aux 0: %\n", hex_of(mine[..], s[..]));
  print("that signature verifies too: %\n", nostr_verify_vartime(hash[..], owner[..], mine[..]));
  tag: [512] u8; t := noa_write_tag(owner, CONDS, nip_sig, tag[..], 0);
  print("tag (% bytes): %\n", t, cast(string) tag[0..<t]);

  print("NIP-OA's signed event example\n");
  scratch: [2048] u8; e := nostr_event_verify_vartime(NIP_EVENT, scratch[..], false);
  print("id and sig verify: %\n", e.verified);
  a := noa_attribute(e);
  print("auth tag: %\n", NOA_WORDS[a.status]);
  print("author: %\n", hex_of(a.author[..], h1[..]));
  print("authorized by: %\n", hex_of(a.owner[..], h1[..]));

  print("refusals, each event signed by the agent\n");
  ev: [2048] u8; tb: [1024] u8; k := 0;
  k = put(tb[..], 0, "[[\"auth\",\""); k = put(tb[..], k, hex_of(owner[..], h1[..]));
  k = put(tb[..], k, "\",\"kind=1&created_at<1713957001\",\""); k = put(tb[..], k, NIP_SIG); k = put(tb[..], k, "\"]]");
  check("conditions changed to created_at<1713957001", sign_event(agent_key[..], aux[..], T, 1, cast(string) tb[0..<k], "x", ev[..]));
  k = put(tb[..], 0, "[[\"auth\",\""); k = put(tb[..], k, hex_of(owner[..], h1[..]));
  k = put(tb[..], k, "\",\"created_at<1713957000&kind=1\",\""); k = put(tb[..], k, NIP_SIG); k = put(tb[..], k, "\"]]");
  check("the same clauses reordered", sign_event(agent_key[..], aux[..], T, 1, cast(string) tb[0..<k], "x", ev[..]));
  k = put(tb[..], 0, "["); k = put(tb[..], k, cast(string) tag[0..<t]); k = put(tb[..], k, "]");
  good := cast(string) tb[0..<k];
  check("the tag on a kind 7 event", sign_event(agent_key[..], aux[..], T, 7, good, "x", ev[..]));
  check("the tag on a kind 1 event at 1713957000", sign_event(agent_key[..], aux[..], 1713957000, 1, good, "x", ev[..]));
  two: [1024] u8; k = put(two[..], 0, "["); k = put(two[..], k, cast(string) tag[0..<t]); k = put(two[..], k, ",");
  k = put(two[..], k, cast(string) tag[0..<t]); k = put(two[..], k, "]");
  check("two auth tags", sign_event(agent_key[..], aux[..], T, 1, cast(string) two[0..<k], "x", ev[..]));
  check("three elements", sign_event(agent_key[..], aux[..], T, 1, "[[\"auth\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"kind=1\"]]", "x", ev[..]));
  check("conditions kind=1&", sign_event(agent_key[..], aux[..], T, 1, "[[\"auth\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"kind=1&\",\"8b7df2575caf0a108374f8471722b233c53f9ff827a8b0f91861966c3b9dd5cb2e189eae9f49d72187674c2f5bd244145e10ff86c9f257ffe65a1ee5f108b369\"]]", "x", ev[..]));
  check("conditions kind=01", sign_event(agent_key[..], aux[..], T, 1, "[[\"auth\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"kind=01\",\"8b7df2575caf0a108374f8471722b233c53f9ff827a8b0f91861966c3b9dd5cb2e189eae9f49d72187674c2f5bd244145e10ff86c9f257ffe65a1ee5f108b369\"]]", "x", ev[..]));
  check("the owner's tag on an event the owner signs", sign_event(owner_key[..], aux[..], T, 1, good, "x", ev[..]));
  changed: [1024] u8; for i: 0..<NIP_EVENT.count { changed[i] = NIP_EVENT[i]; }
  changed[NIP_EVENT.count - 150] = 'O';   // "owner-attested" becomes "Owner-attested"
  check("NIP-OA's example with one content byte changed", cast(string) changed[0..<NIP_EVENT.count]);

  print("NIP-AA: the agent logs in to %, the owner a member\n", RELAY);
  m: [1] NaaMembers; #assert naa_add(m[..], owner);
  conn: [1] NaaConnection; extra: [512] u8; k = put(extra[..], 0, ","); k = put(extra[..], k, cast(string) tag[0..<t]);
  with_tag := cast(string) extra[0..<k];
  tb2: [1024] u8; text := sign_event(agent_key[..], aux[..], T, 22242, auth_tags("challenge-87", with_tag, tb2[..]), "", ev[..]);
  print("AUTH event (% bytes): %\n", text.count, text);
  print("NIP-OA's own check of that event: %\n", NOA_WORDS[noa_check(nostr_event_verify_vartime(text, scratch[..], false)).status]);
  g := login("relay clock 1713956430", conn[..], m[0], "challenge-87", T, T + 30, with_tag);
  print("the agent's kind 1 event: %\n", E_WORDS[naa_event(conn[0], agent, 1, false)]);
  print("its kind 7 event: %\n", E_WORDS[naa_event(conn[0], agent, 7, false)]);
  print("its kind 7 event, kind clauses enforced: %\n", E_WORDS[naa_event(conn[0], agent, 7, true)]);
  print("its membership change request: %\n", E_WORDS[naa_admin(conn[0], agent)]);

  print("refusals\n");
  r: [1] NaaConnection;
  g = login("an AUTH event 121 s old", r[..], m[0], "challenge-87", T, T + 121, with_tag);
  g = login("created_at 1713957001, against created_at<1713957000", r[..], m[0], "challenge-87", 1713957001, 1713957001, with_tag);
  g = login("no auth tag", r[..], m[0], "challenge-87", T, T + 30, "");
  k = put(extra[..], 0, ","); k = put(extra[..], k, cast(string) tag[0..<t]); k = put(extra[..], k, ","); k = put(extra[..], k, cast(string) tag[0..<t]);
  g = login("two auth tags", r[..], m[0], "challenge-87", T, T + 30, cast(string) extra[0..<k]);
  osig: [64] u8; sign_hash(other_key[..], aux[..], hash[..], osig[..]);
  ot: [512] u8; k = noa_write_tag(other, CONDS, osig, ot[..], 1); ot[0] = ',';
  g = login("signed by an owner who is not a member", r[..], m[0], "challenge-87", T, T + 30, cast(string) ot[0..<k]);
  print("keys admitted on that connection: %\n", r[0].n);

  print("revocation\n");
  #assert naa_remove(m[..], owner);
  print("the operator removes the owner: % members\n", m[0].n);
  fresh: [1] NaaConnection;
  g = login("the agent's next AUTH, new connection", fresh[..], m[0], "challenge-88", T + 60, T + 70, with_tag);
  print("the old connection still holds % key; its kind 1 event: %\n", conn[0].n, E_WORDS[naa_event(conn[0], agent, 1, false)]);
  print("sessions ended by owner: %; its kind 1 event: %\n", naa_end(conn[..], owner), E_WORDS[naa_event(conn[0], agent, 1, false)]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/agent-auth.coax --run-steps 250000000
```

The output matches the worked values: a 110-byte preimage with hash
`08cdecd5...`, the 237-byte tag, the published event verified and
attributed to the agent, and the AUTH event answered `true`.

```text
NIP-OA: the owner's attestation
owner (secret 1): 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
agent (secret 2): c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5
preimage (110 bytes): nostr:agent-auth:c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5:kind=1&created_at<1713957000
sha256(preimage): 08cdecd55af4c28d3801fd69615dcf5cc04fab3bc134b38a840bf157197069a6
NIP-OA's auth_sig verifies under the owner: true
the owner signs again, aux 0: 4fd007535ff75b35b6fa4696d4b1e04144d512b9ff74baca09dd9a1cac4c759d63d9c3645e1b8d86ab9198d21813d1f405d17327f0c99970df4bb9cded69fd76
that signature verifies too: true
tag (237 bytes): ["auth","79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","kind=1&created_at<1713957000","8b7df2575caf0a108374f8471722b233c53f9ff827a8b0f91861966c3b9dd5cb2e189eae9f49d72187674c2f5bd244145e10ff86c9f257ffe65a1ee5f108b369"]
NIP-OA's signed event example
id and sig verify: true
auth tag: ok
author: c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5
authorized by: 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
refusals, each event signed by the agent
conditions changed to created_at<1713957001: not the owner's signature
the same clauses reordered: not the owner's signature
the tag on a kind 7 event: outside the conditions
the tag on a kind 1 event at 1713957000: outside the conditions
two auth tags: more than one auth tag
three elements: not four elements
conditions kind=1&: the conditions don't read
conditions kind=01: the conditions don't read
the owner's tag on an event the owner signs: the owner is the event's own key
NIP-OA's example with one content byte changed: not a verified event
NIP-AA: the agent logs in to wss://relay.example.com, the owner a member
AUTH event (648 bytes): {"id":"0f4f3fd0ac4d12ad62601fc9b832633be644c9ff10f07aea706c1351bbdce189","pubkey":"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5","created_at":1713956400,"kind":22242,"tags":[["relay","wss://relay.example.com"],["challenge","challenge-87"],["auth","79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","kind=1&created_at<1713957000","8b7df2575caf0a108374f8471722b233c53f9ff827a8b0f91861966c3b9dd5cb2e189eae9f49d72187674c2f5bd244145e10ff86c9f257ffe65a1ee5f108b369"]],"content":"","sig":"9a9d2adcb322e80713fa0a040a43b5c86fef224aff3303439e7144b3991d90934c38c272b904ce59ba52f62de899db363bfb65c2e21495db6968a958470730dc"}
NIP-OA's own check of that event: outside the conditions
relay clock 1713956430: ["OK","0f4f3fd0ac4d12ad62601fc9b832633be644c9ff10f07aea706c1351bbdce189",true,""]
the agent's kind 1 event: accepted
its kind 7 event: accepted
its kind 7 event, kind clauses enforced: restricted: kind not in the credential
its membership change request: restricted: virtual member
refusals
an AUTH event 121 s old: ["OK","0f4f3fd0ac4d12ad62601fc9b832633be644c9ff10f07aea706c1351bbdce189",false,"invalid: created_at is not within 120 seconds"]
created_at 1713957001, against created_at<1713957000: ["OK","501bbed96f7a12eb0ce0ebc4644b936edbdb11699e13d6ce7c218966f89cd97c",false,"restricted: the auth tag is not valid for this AUTH event"]
  the tag: outside the conditions
no auth tag: ["OK","b9566f955a3597cdb82d73a3b1229aa342e00fc5594616776349d563fcbaffca",false,"restricted: no auth tag and not a member"]
two auth tags: ["OK","e529311374cc64ffbe63d8c94d77f6c98976a124551b19c59d9131afb26a8afa",false,"restricted: more than one auth tag"]
signed by an owner who is not a member: ["OK","4d1a8c63abcaf88ed6f926de99c7f3fc3dcefec709f4df7ab97160a6c23c21cc",false,"restricted: the owner is not a member"]
keys admitted on that connection: 0
revocation
the operator removes the owner: 0 members
the agent's next AUTH, new connection: ["OK","aadd4a545395ed8aa8bad122e58616a2130592833e752afbfb1669747da25e1e",false,"restricted: the owner is not a member"]
the old connection still holds 1 key; its kind 1 event: accepted
sessions ended by owner: 1; its kind 1 event: restricted: not authenticated
```

The refusals, one per check:

- Conditions changed to `created_at<1713957001`: the agent tries to stretch
  its window by one second. The owner never signed that string, so the
  signature refuses. This is the first "Done means" line.
- The same two clauses reordered also refuse at the signature: the
  conditions are hashed as written.
- The valid tag on a kind 7 event, and on a kind 1 event at `1713957000`,
  refuse as outside the conditions.
- Two `auth` tags, three elements, `kind=1&`, `kind=01` and a tag on an
  event the owner signs are NIP-OA's invalid vectors.
- NIP-OA's example with one content byte changed is not a verified event,
  so its valid tag establishes nothing.
- At the relay: an AUTH event 121 seconds old is `invalid:`; the others are
  `restricted:`, and none of them adds a key to the connection.
- After the owner is removed, the agent's next AUTH refuses at step 5. This
  is the second "Done means" line. The old connection keeps its entry until
  `naa_end` ends it.

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/agent-auth.out](../fixtures/agent-auth.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/agent-auth.coax](../examples/checks/agent-auth.coax),
which loads the example. The independent values come from
[nipoa_reference.py](../../coax/tests/nostr/nipoa_reference.py), which uses
the pure-Python BIP-340 signer and NIP-01 serializer of
`nip59_reference.py` and shares no code with Coax.

- The preimage's length and hash.
- Both owners' signatures with 32 zero aux bytes, byte for byte.
- The agent's AUTH event, byte for byte (648 bytes, id `0f4f3fd0...`).
- NIP-OA's signed event: verified, `auth` tag valid, the agent the author.
- The relay on the Python-signed AUTH event: virtual membership, then a
  refusal at step 5 after the owner is removed.

<!-- source: nostr/examples/checks/agent-auth.coax check -->
```jai
#load "../agent-auth.coax";

// Values from coax/tests/nostr/nipoa_reference.py, a pure-Python BIP-340
// signer and NIP-01 serializer that shares no code with Coax. It also
// confirms NIP-OA's published hash, auth_sig, tag bytes, event id and event
// sig: all six are correct.
PY_HASH :: "08cdecd55af4c28d3801fd69615dcf5cc04fab3bc134b38a840bf157197069a6";
// sign(owner secret 1, sha256(preimage), 32 zero aux bytes).
PY_OWNER_SIG :: "4fd007535ff75b35b6fa4696d4b1e04144d512b9ff74baca09dd9a1cac4c759d63d9c3645e1b8d86ab9198d21813d1f405d17327f0c99970df4bb9cded69fd76";
// The same for the second owner (secret 3).
PY_OTHER_SIG :: "c3e7657e3886b13a00b61f97549aeff0fee7df532fab8ed9d3f6769e9f1defd934dc59f1ca12565782ef1cd0725fbc18c866d8007b358a355a69497332c1b8b8";

#test "the preimage and its hash are NIP-OA's and Python's" {
  two := test_key(2); agent := public_x(two[..]); out: [NOA_TEXT_MAX] u8; h: [32] u8; hex: [64] u8;
  #assert noa_preimage(agent, CONDS, out[..]) == 17 + 64 + 1 + 28;
  #assert noa_hash(agent, CONDS, h[..]) && hex_of(h[..], hex[..]) == PY_HASH;
}
#test "the owners' signatures are Python's, byte for byte" #steps 2000000000 {
  two := test_key(2); agent := public_x(two[..]); h: [32] u8; #assert noa_hash(agent, CONDS, h[..]);
  aux: [32] u8; sig: [64] u8; hex: [128] u8;
  one := test_key(1); sign_hash(one[..], aux[..], h[..], sig[..]);
  #assert hex_of(sig[..], hex[..]) == PY_OWNER_SIG;
  three := test_key(3); sign_hash(three[..], aux[..], h[..], sig[..]);
  #assert hex_of(sig[..], hex[..]) == PY_OTHER_SIG;
}
#test "the agent's AUTH event is Python's, byte for byte" #steps 2000000000 {
  key := test_key(2); aux: [32] u8; ev: [2048] u8;
  text := sign_event(key[..], aux[..], T, 22242, "[[\"relay\",\"wss://relay.example.com\"],[\"challenge\",\"challenge-87\"],[\"auth\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"kind=1&created_at<1713957000\",\"8b7df2575caf0a108374f8471722b233c53f9ff827a8b0f91861966c3b9dd5cb2e189eae9f49d72187674c2f5bd244145e10ff86c9f257ffe65a1ee5f108b369\"]]", "", ev[..]);
  #assert text == NAA_T_AUTH && text.count == 648;
}
#test "NIP-OA's signed event: id, sig and tag verify; the agent is the author" #steps 2000000000 {
  scratch: [2048] u8; e := nostr_event_verify_vartime(NIP_EVENT, scratch[..], false);
  #assert e.verified;
  a := noa_attribute(e); hex: [64] u8;
  #assert a.status == NOA_OK && a.authorized && hex_of(a.author[..], hex[..]) == NOA_T_AGENT;
  #assert hex_of(a.owner[..], hex[..]) == NOA_T_OWNER;
}
#test "the relay on Python's AUTH event: virtual, then refused once the owner is revoked" #steps 2000000000 {
  m: [1] NaaMembers; #assert naa_add(m[..], noa_k(NOA_T_OWNER));
  n: [1] Nip42; n[0] = naa_n42(); scratch: [2048] u8;
  conn: [1] NaaConnection;
  #assert naa_auth(conn[..], m[0], n[..], 1, NAA_T_NOW, NAA_T_AUTH, scratch[..]).code == NAA_VIRTUAL;
  #assert naa_remove(m[..], noa_k(NOA_T_OWNER));
  fresh: [1] NaaConnection;
  a := naa_auth(fresh[..], m[0], n[..], 1, NAA_T_NOW, NAA_T_AUTH, scratch[..]);
  #assert a.code == NAA_OWNER && fresh[0].n == 0 && conn[0].n == 1;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/agent-auth.coax --run-steps 250000000
```

`coax test` runs 436 checks, including the loaded files' own checks.

### NIP-OA's published values

`nipoa_reference.py` recomputes all six published values in NIP-OA: the
110-byte preimage, its hash, the `auth_sig` (it verifies under the owner),
the tag bytes, the signed event's id and its signature (it verifies under
the agent). All six are correct; none is recorded as wrong. Both published
signatures were made with an aux value other than 32 zero bytes, so a
signer cannot reproduce them, only verify them; the checks therefore
compare signatures made with zero aux. NIP-AA publishes no complete AUTH
event, so its examples are checked as decisions: every row of its reject
table and its kind enforcement table is a check in `nipaa.coax`.

### Checks in the library

- `nipoa.coax`: the conditions grammar on 28 malformed strings, the bounds
  of both ranges, conjunctive kind clauses, 256 and 257 bytes; the preimage
  and hash against NIP-OA's vector; the tag writer against NIP-OA's tag
  line; every refusal of `noa_read` and its order; escaped spellings; and,
  with real signatures, NIP-OA's tag verifying, altered conditions
  refusing, and events outside the clauses refusing.
- `nipaa.coax`: the member list; NIP-AA's accept example; every step's
  refusal, in order, with the 120-second bounds both ways and NIP-42's
  600; replacement on re-AUTH; a full connection; revocation on a new
  connection and ending sessions by owner; the privileges; the `OK`
  messages; and one Python-signed AUTH event verified end to end.

On 2026-11-01, `coax mutate --root-only` killed every mutant with none
surviving: 210 of 210 in `nipoa.coax` and 155 of 155 in `nipaa.coax`. The first
runs' 17 and 3 survivors were killed by new checks (`kind=1:`, the time
maximum, values longer than the hex buffer, an end that skips no entry) or
by removing code that could not change a result (a length test that the
range test made redundant, constant arguments to a reader whose
evaluation was ignored). Full mutation, and mutation of the example and
its checks, were not run. Chapter 38's records for `nip42.coax` were not
rerun.

### Not built

- Rate limits and quotas aggregated by owner across that owner's agents.
- Audit logs and membership introspection that mark virtual members.
- Closing the WebSocket on an AUTH too malformed to give an event id; that
  belongs to the relay's socket code (chapter 34).
- Wiring `naa_auth` into the relay's AUTH path and `naa_event` into its
  `EVENT` path, and reading the member list from NIP-43 events.
- Displaying provenance in a client ("authorized by"); `noa_attribute`
  gives a client what it needs.

## Summary

- An owner authorizes an agent by signing
  `nostr:agent-auth:<agent>:<conditions>`. The agent stays the only author.
- The conditions are hashed exactly as the tag carries them, so changing or
  reordering a clause breaks the owner's signature.
- Every clause must hold. Nothing in NIP-OA reads a clock.
- A relay admits an agent when the AUTH event passes NIP-42, is within 120
  seconds, carries one valid tag, and the owner is an active member.
- Virtual membership lives only on the connection, never administers the
  relay, and fails on the next AUTH once the owner is removed.

## Dependencies

- [38. NIP-42 authentication](ch038-nip-42-authentication.md)

## Sources

- [Block NIP-OA](../nips/block/NIP-OA.md)
- [Block NIP-AA](../nips/block/NIP-AA.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
