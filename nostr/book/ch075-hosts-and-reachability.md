# 75. Hosts and reachability

An OpenAgents host is the agent host running on one computer. Your phone,
a browser tab or another computer reaches it to start tasks, read sessions
and open terminals. Two OpenAgents NIPs cover this. NIP-HOST says who may
use a host: the host signs a grant to a device key, and checks that grant on
every request. NIP-REACH says how a device finds the host: presence samples,
reachability hints and a direct channel. Neither one turns a route into a
permission. This chapter opens a real grant event, admits and refuses
requests against the host's record, and decides which presence and hints a
device may use.

Part X, Every NIP. Result: a NIP-HOST grant bound to the original
host-signed, device-encrypted 3188 envelope; the host's admission of a
request (the request window, revocation, host-clock expiry, epochs, rights
and retained replies); and NIP-REACH presence freshness, rollback refusal
and the generation check on a hint set. Invitations, enrollment, the owner
directory, hint selection and the direct channel are not built yet; see
[What is not built](#what-is-not-built).

## What this chapter teaches

- A grant (`openagents.host-grant.v1`) and the private artifact that carries
  it: kind 3188, one `p`, one `h` mailbox, `t: oa:artifact:v1`, NIP-44
  content, and an artifact envelope whose inline body matches its digest.
- Why a device checks the original envelope and not a copy of its fields.
- How the host admits a request: the request's 60-second window first, then
  the grant (revoked, expired, epoch), then the right, then the request ID.
- Revocation epochs: revoking a device revokes every grant and advances its
  epoch, so a grant from before is stale.
- REACH freshness by the reader's own clock, generations that never roll
  back, and hints used only for the current generation.
- Network membership, a route or an open channel never grants execution.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [grant.coax](../../coax/lib/nostr/host/grant.coax) | `host_access_bind_checked(wire, secret, x, work, overlap)` | Binds a saved access record's grant to its original signed 3188 envelope for this device |
| [admit.coax](../../coax/lib/nostr/host/admit.coax) | `host_admit(grants, epochs, retained, q, now)` | The host's decision on one request: admitted, an exact retry, or the refusal |
| [admit.coax](../../coax/lib/nostr/host/admit.coax) | `host_revoke(grants, epochs, device)` | Revokes every grant of a device and advances its epoch |
| [fresh.coax](../../coax/lib/nostr/reach/fresh.coax) | `reach_presence_accept(held, have, s)` | Whether a device keeps a presence sample |
| [fresh.coax](../../coax/lib/nostr/reach/fresh.coax) | `reach_presence_fresh(s, now)` | Whether a kept sample is still fresh |
| [fresh.coax](../../coax/lib/nostr/reach/fresh.coax) | `reach_hints_usable(generation, expires_at, presence, have, now)` | Whether a hint set may be tried at all |

`grant.coax` existed before this chapter, with its checks and its
cross-target runner. It gains only two listing markers (comments).
`admit.coax` and `fresh.coax` are new with this chapter. They decide
admission, so both are `#authority` files: their checks came first, and a
mutation run kills every mutant. Neither loads the crypto: they take the
verified signer, the event id and the host's own record as plain fields,
so their checks run in milliseconds.

## The three keys

NIP-HOST has three principals, each a different key:

- The **owner** is the person whose authority the host serves. The host
  learns its owner locally, on that machine, and never from a relay, an
  invitation or a network route. The owner needs no grant.
- The **host** key signs grants and replies. It is not the owner's key.
- A **device** key holds grants: a phone, a browser, another host acting as
  a client.

A grant always names three distinct keys. In this chapter's test vectors
they are the secp256k1 scalars 1 (host), 2 (owner), 3 (device) and 7 (a
second device): the x-coordinates of `1·G`, `2·G`, `3·G` and `7·G` from
chapter 10.

| Key | Scalar | x-only public key |
| --- | --- | --- |
| host | 1 | `79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798` |
| owner | 2 | `c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5` |
| device | 3 | `f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9` |
| second device | 7 | `5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc` |

## Grants

A grant is a JSON object with these fields, all required, and no others:

| Field | Meaning |
| --- | --- |
| `v` | `openagents.host-grant.v1` |
| `requires` | `[]`. A required feature this version does not know refuses |
| `grant` | A random 64-hex ID. It is also the mailbox of the envelope that carries it |
| `host`, `owner`, `device` | Three distinct keys |
| `relay` | The exact relay the host admitted, 1 to 256 bytes |
| `rights` | A nonempty, duplicate-free list in NIP-HOST's order: `observe`, `operate`, `terminal`, `review`, `access_read`, `access_admin`, `world` |
| `epoch` | The device's revocation epoch when the host issued the grant |
| `origin` | `{kind, id, issuer}`: `invitation` or `approval`, its ID, and the key that authorized it |
| `issued_at`, `expires_at` | Unix seconds, at most 30 days (2,592,000 s) apart |

No right implies another. The **standard device grant** is `observe`,
`operate`, `terminal` and `review`. A right is necessary, never
sufficient: a terminal still needs a terminal session, and a file read
still passes the host's disclosure policy.

### The envelope

The host does not publish the grant as plain JSON. It seals it as a private
artifact (the [shared contracts](../nips/openagents/contracts.md)' kind
3188): a regular event signed by the host with

- exactly one `p` tag, the device: only that device can read it;
- exactly one `h` tag, a random 64-hex mailbox. For a grant the mailbox is
  the grant ID;
- the tag `t` = `oa:artifact:v1`;
- content that is a NIP-44 v2 payload (chapter 20) encrypted with the
  conversation key of the host and the device.

Decrypted, the content is an artifact envelope:
`{v: "openagents.artifact-envelope.v1", requires: [], artifact, inline,
issued_at, retain_until}`. `artifact` names the schema
(`openagents.host-grant.v1`), the SHA-256 digest and the size of `inline`'s
canonical JSON bytes (RFC 8785, JCS), and `inline` is the grant itself.

The device saves an access record holding the grant's fields (a protobuf
projection, `openagents.host.v1.Access.grant`) and the original event JSON
(`Access.authorization`). The projection is convenient to read; it is not
evidence. Anyone could write a projection with more rights beside a real
envelope. So `grant.coax` does the whole path again from the original
bytes, and compares every projected field with the decrypted one.

### Worked example: the fixture grant, by hand

The grant in this chapter is NIP-HOST's fixture grant, sealed by an
independent implementation (nostr-effect's NIP-44 and signing, and the
unlock sealer's JCS, in
[host_grant_fixtures.ts](../../coax/tests/nostr/host_grant_fixtures.ts)).
Its signed body, in JCS key order:

```text
{"device":"f9308a01…bce036f9","epoch":0,"expires_at":1790086400,
 "grant":"1111…1111","host":"79be667e…16f81798","issued_at":1790000000,
 "origin":{"id":"2222…2222","issuer":"79be667e…16f81798","kind":"invitation"},
 "owner":"c6047f94…5c709ee5","relay":"wss://relay.example.com/",
 "requires":[],"rights":["observe","operate","terminal","review"],
 "v":"openagents.host-grant.v1"}
```

Check each rule by hand:

- `host`, `owner` and `device` are the keys of scalars 1, 2 and 3: three
  distinct keys.
- The lifetime is 1790086400 − 1790000000 = 86,400 s, one day. That is
  more than 0 and at most 2,592,000.
- The rights are four known names in strictly increasing NIP order
  (positions 0, 1, 2, 3): the standard device grant. As bits, `1 + 2 + 4 + 8`
  = 15, which `admit.coax` calls `HA_STANDARD`.
- `epoch` is 0: the device was never revoked.
- `origin` says an invitation, ID `2222…2222`, authorized by the host key.

The event that carries it:

| Field | Value | Rule |
| --- | --- | --- |
| `kind` | 3188 | a private artifact |
| `pubkey` | `79be667e…16f81798` | the signer; it must equal the grant's `host` |
| `created_at` | 1790000000 | equals the envelope's `issued_at` |
| `p` | `f9308a01…bce036f9` | the device, and only it |
| `h` | `1111…1111` | the mailbox; it must equal the grant's `grant` |
| `t` | `oa:artifact:v1` | |
| `content` | `AukGSqBz…` (base64) | NIP-44 v2: the first byte decodes to `0x02`, the version |
| `id` | `1b9bc616…3d740bf2` | SHA-256 of the NIP-01 serialization (chapter 21) |

The body is 668 bytes of JCS, and the envelope's artifact says
`size: 668` and digest `sha256:84eb6b0a…cdf15644`. The envelope's
`retain_until` is 1790086460: the grant's expiry plus 60 s, the host's
tombstone window below.

`host_access_bind_checked` follows that table top to bottom. Each step has
its own refusal, so a test can tell where a bad record stops:

<!-- source: coax/lib/nostr/host/grant.coax bind -->
```jai
// Binds Access.grant to Access.authorization for the device whose secret
// and x-only public key are given. On HG_OK, original is the exact signed
// envelope (borrowed from access_wire) to retain beside the projection.
// overlap is the host's truthful fact that any two views share memory.
host_access_bind_checked :: (access_wire:[]u8,device_secret:[]u8,device_x:[]u8,work:[]u8,overlap:bool)->HostGrant #export_c {
  r:HostGrant;
  if device_secret.count!=32 || device_x.count!=32 || work.count<HG_WORK || overlap {return r;}
  access:=H_Access_decode(access_wire);
  if access.error!=0 {return hg_done(r,HG_WIRE,work);}
  g:=H_Access_get_grant(access);
  if !g.present || g.error!=0 {return hg_done(r,HG_WIRE,work);}
  // An absent authorization reads as an empty SignedEvent: its json is absent too.
  original:=H_SignedEvent_get_json(H_Access_get_authorization(access).value);
  if original.error!=0 || !original.present {return hg_done(r,HG_WIRE,work);}
  text:=original.value;
  // The regions are each at least twice what their user needs.
  pre:=work[0..<HG_REGION]; // mutant: equivalent (a 1-byte shift of a region bound is never reached)
  e:=nostr_event_read(text,pre,false);
  if e.error!=NE_OK {return hg_done(r,HG_EVENT,work);}
  if !nostr_fast_verify_vartime(e.id[..],e.public_key[..],e.signature[..]) {return hg_done(r,HG_SIGNATURE,work);}
  ck:[32]u8;ew:[1]EcdhScratch;peer:=e.public_key;
  c:=nip44_conversation_key_from_keys_checked(device_secret,peer[..],ew[..],ck[..],false);
  if c!=NIP44_OK {return hg_done(r,ifx c==NIP44_SECRET then HG_KEY else HG_DECRYPT,work);}
  result:=hg_open(text,e,g.value,ck[..],device_x,work);
  hg_zero(ck[..]); // mutant: equivalent (wipes a local no output reads)
  return result;
}
// After the signature and the ECDH conversation key (device secret, signer):
// recipient, decryption, envelope and grant. Split out so checks reach it
// within a check budget (ECDH alone is about one); host_access_bind_checked
// is the only caller outside checks.
hg_open :: (text:string,e:NostrEvent,g:H_Grant,ck:[]u8,device_x:[]u8,work:[]u8)->HostGrant {
  r:HostGrant;
  // 32 bytes always fit 64 without overlap: hex encoding cannot refuse here.
  dev:[64]u8;signer:[64]u8;mail:[64]u8;
  peer:=e.public_key;
  hd:=hex_encode_checked(cast(string)device_x,dev[..],false);hs:=hex_encode_checked(cast(string)peer[..],signer[..],false);
  if e.kind!=3188 || !hg_tags(text[e.tags_from..<e.tags_to],cast(string)dev[..],mail[..]) {return hg_done(r,HG_ARTIFACT,work);}
  // The reader admitted content as JSON of at most 65536 bytes: it unescapes into the 131072-byte scratch.
  scratch:=work[4*HG_REGION..<5*HG_REGION]; // mutant: equivalent (see the region note above)
  n:=json_unescape(text[e.content_from..<e.content_to],scratch);
  body:=work[HG_REGION..<2*HG_REGION];nwork:=work[2*HG_REGION..<4*HG_REGION]; // mutant: equivalent (see the region note above)
  d:=nip44_decrypt_checked(NIP44_COMPAT,ck,cast(string)scratch[0..<n],nwork,body,false);
  if d.error!=NIP44_OK {return hg_done(r,HG_DECRYPT,work);}
  envelope:=cast(string)body[0..<d.written];
  info:=envelope_check_body_checked(envelope,HG_SCHEMA,scratch,false);
  if info.error!=AE_OK {return hg_done(r,HG_ENVELOPE,work);}
  b:=hg_body(envelope[info.inline_from..<info.inline_to],g,cast(string)signer[..],cast(string)dev[..],cast(string)mail[..]);
  if b.error!=HG_OK {return hg_done(r,b.error,work);}
  b.original=text;return hg_done(b,HG_OK,work);
}
```

`HG_OK` says only that the projection is exactly what this host signed for
this device. It says nothing about whether the grant is still current:
offline, a device can't know that the host has not revoked it. Only the
host's record can.

## How the host admits a request

Every operation is a request, `openagents.host-request.v1`, signed by the
device. It names its own `request` ID (stable through retransmission),
the `grant` and `epoch` it uses, `issued_at` and `expires_at` at most 60 s
apart, and an operation that needs one right. A copied grant is not a
bearer credential: the request must be signed by the grant's own device.

The host keeps three records:

- its grants, each with a revoked mark (a tombstone);
- one revocation epoch per device key, starting at 0;
- the signed replies it retains, by request ID, until the request expires.

<!-- source: coax/lib/nostr/host/admit.coax record -->
```jai
HostGrantRecord :: struct {grant:string;device:string;epoch:int;issued_at:int;expires_at:int;rights:int;revoked:bool;}
HostEpoch :: struct {device:string;epoch:int;}
HostRetained :: struct {request:string;event:string;expires_at:int;}
HostRequest :: struct {request:string;event:string;signer:string;grant:string;epoch:int;issued_at:int;expires_at:int;right:int;}
```

The outcomes, in the order the host checks them:

<!-- source: coax/lib/nostr/host/admit.coax codes -->
```jai
HA_OK :: cast(u32)0;           // admit and act
HA_RETRY :: cast(u32)1;        // an exact retry: answer the retained reply, act again never
HA_SILENT :: cast(u32)2;       // the request has expired: no reply, no effect
HA_MALFORMED :: cast(u32)3;    // window over 60 s, empty, or outside the grant
HA_EARLY :: cast(u32)4;        // dated more than 60 s after the host's clock
HA_NOT_ADMITTED :: cast(u32)5; // no such grant, another device's grant, or a right it lacks
HA_REVOKED :: cast(u32)6;
HA_EXPIRED :: cast(u32)7;      // the grant has expired by the host's clock
HA_STALE :: cast(u32)8;        // an epoch differs: request vs grant, or grant vs device
HA_CONFLICT :: cast(u32)9;     // different bytes under a retained request ID

HA_WINDOW :: 60;  // a request's lifetime, and the clock skew the host allows
```

The order matters. The request's own window comes first, so an expired
request earns no reply and no effect (`silent`), whatever it names. The
principal comes next, so an exact retry repeats every check: after a
revocation, a retry refuses as `revoked` instead of reading the reply the
host kept. Only a request that passes all of that reaches the retained
replies, where the same request ID with the same event is a retry (the
host answers the same bytes and does nothing again) and with different
bytes is a `conflict`.

<!-- source: coax/lib/nostr/host/admit.coax admit -->
```jai
host_admit :: (grants:[]HostGrantRecord,epochs:[]HostEpoch,retained:[]HostRetained,q:HostRequest,now:int)->u32 {
  // 1. The request: at most 60 s long, not yet expired, not from the future.
  if q.expires_at<=q.issued_at || q.expires_at-q.issued_at>HA_WINDOW {return HA_MALFORMED;}
  if now>q.expires_at {return HA_SILENT;}
  if q.issued_at>now+HA_WINDOW {return HA_EARLY;}
  // 2. The principal: this device's grant, unrevoked, unexpired, current.
  k:=-1;
  for i:0..<grants.count {if grants[i].grant==q.grant {k=i;}}
  if k<0 || grants[k].device!=q.signer {return HA_NOT_ADMITTED;}
  g:=grants[k];
  if g.revoked {return HA_REVOKED;}
  if now>=g.expires_at {return HA_EXPIRED;}
  if q.epoch!=g.epoch || g.epoch!=host_epoch(epochs,g.device) {return HA_STALE;}
  if q.issued_at<g.issued_at-HA_WINDOW || q.expires_at>g.expires_at {return HA_MALFORMED;}
  // 3. The right. It is necessary, never sufficient.
  if (g.rights>>q.right)&1==0 {return HA_NOT_ADMITTED;}
  // 4. Idempotency: the request ID, while its retained reply lasts.
  for r:retained {
    if r.request==q.request && now<=r.expires_at {return ifx r.event==q.event then HA_RETRY else HA_CONFLICT;}
  }
  return HA_OK;
}
```

Expiry is the host's clock, never the event's `created_at`. A device clock
may run up to 60 s behind, so a request may be dated up to 60 s before its
grant; a request more than 60 s ahead of the host is `early`.

### Revocation and epochs

Revoking a device marks all of its grants revoked and advances its epoch by
one, together, before the host answers. Re-enrolling issues a new grant at
the new epoch. NIP-HOST refuses a request whose epoch differs from its
grant's, or a grant whose epoch differs from the device's current one, as
`stale`. The second rule is what makes the epoch useful: a grant from
before a revocation is refused even when its tombstone is gone.

<!-- source: coax/lib/nostr/host/admit.coax revoke -->
```jai
// Marks every grant of device revoked and advances its epoch, together.
// Returns the new epoch, or -1 when the device has no epoch entry (its
// grants are still revoked; the caller adds the entry at epoch 1).
host_revoke :: (grants:[]HostGrantRecord,epochs:[]HostEpoch,device:string)->int {
  for i:0..<grants.count {if grants[i].device==device {grants[i].revoked=true;}}
  for i:0..<epochs.count {if epochs[i].device==device {epochs[i].epoch+=1;return epochs[i].epoch;}}
  return -1;
}
```

The host keeps tombstones until at least grant expiry plus the 60-second
request window, and epochs for as long as any grant of that device could be
presented. A restart never resets either.

### Worked example: admitting requests, by hand

The host records the fixture grant: grant `1111…`, device 3, epoch 0, from
T0 = 1790000000 to T0 + 86400, rights 15. Device 3's epoch is 0. Request R
(`aaaa…`, event `bbbb…`) asks for `operate` (bit 1) with epoch 0, from
T0 + 100 to T0 + 160.

| Case | Now | Step that decides | Result |
| --- | --- | --- | --- |
| R | T0 + 110 | 160 − 100 = 60 ≤ 60; 110 ≤ 160; grant found, device 3; unrevoked; 110 < 86400; epochs 0 = 0 = 0; 100 ≥ 0 − 60, 160 ≤ 86400; bit 1 of 15 is 1; no retained R | admitted |
| R again, after the host kept its reply | T0 + 120 | every check again; retained R with event `bbbb…` | retry |
| R's ID with event `cccc…` | T0 + 120 | retained R, another event | conflict |
| `access_admin` (bit 5) | T0 + 110 | 15 >> 5 = 0 | not_admitted |
| R signed by device 7 | T0 + 110 | the grant's device is 3 | not_admitted |
| R again | T0 + 161 | 161 > 160 | silent |
| a request 61 s long | T0 + 110 | 61 > 60 | malformed |

Then the owner revokes device 3: the grant is marked revoked and the epoch
becomes 1. R again at T0 + 120 is `revoked`: the window passes, the grant
is found, and its mark decides before the retained reply is reached.
Device 3 enrolls again and receives grant `3333…` at epoch 1, valid from
T0 + 200 to T0 + 86600:

| Case | Now | Step that decides | Result |
| --- | --- | --- | --- |
| grant `3333…` named with epoch 0 | T0 + 210 | request epoch 0 ≠ grant epoch 1 | stale |
| grant `3333…` with epoch 1 | T0 + 210 | all pass | admitted |
| grant `3333…`, request T0 + 86540 to T0 + 86600 | T0 + 86600 | request still in its window (86600 ≤ 86600); 86600 ≥ 86600, the grant's end | expired |

## Reachability

NIP-REACH is about where a host is, never about what a device may do. A
directory entry, a presence sample, a hint and an open channel are all
facts about routes. Each record is another 3188 artifact, signed by its
author and encrypted to one recipient, so a reader first opens it as above
and checks that the signer is the host it expected.

- The **owner host directory** (`openagents.host-directory.v1`) is the
  owner's list of hosts, signed by the owner and encrypted to the owner.
  The highest valid `revision` is current; a host cannot add itself.
- **Presence** (`openagents.host-presence.v1`) is one sample a host sends
  to each enrolled device: its `generation`, a counter that increases
  whenever the host restarts, updates or restores state; its protocol and
  compatibility range; up to 64 capabilities; `observed_at`, the host's
  clock; and coarse telemetry or `null`.
- **Reachability hints** (`openagents.reach-hints.v1`) list up to 16
  routes for one generation: `loopback`, `lan`, `tailnet`, `public` or
  `relay` addresses over TCP, WebSocket or a Nostr relay, each with what
  the host last observed. A hint is a claim, not a proof.
- A **direct channel** is a TCP, WebSocket or iroh connection whose
  handshake proves both Nostr keys and then binds one grant ID, one epoch
  and one host generation. The host checks the grant before every
  operation.

### Freshness

A device judges a presence sample by its own clock, `received_at`, when the
event arrived. With NIP-REACH's reference limits (30 s of skew, 180 s of
age), a sample observed more than 30 s after receipt is from the future,
and one received more than 180 s after it was observed is an old sample
replayed late. The device keeps the newest sample per host: a lower
generation would roll the host back to an earlier process, and within one
generation a sample that is not newer is no news.

<!-- source: coax/lib/nostr/reach/fresh.coax codes -->
```jai
RF_OK :: cast(u32)0;
RF_FUTURE :: cast(u32)1;    // observed after receipt plus the skew
RF_STALE :: cast(u32)2;     // too old, a late replay, or hints of another generation or expired
RF_ROLLBACK :: cast(u32)3;  // a lower generation than the sample held
RF_NOT_NEWER :: cast(u32)4; // the same generation, observed no later

RF_SKEW :: 30;     // NIP-REACH's reference skew, seconds
RF_MAX_AGE :: 180; // NIP-REACH's reference maximum age, seconds
```

<!-- source: coax/lib/nostr/reach/fresh.coax sample -->
```jai
// A presence sample as kept: the host's generation and observed_at, and the
// reader's own clock when the event arrived.
ReachSample :: struct {generation:int;observed_at:int;received_at:int;}
```

<!-- source: coax/lib/nostr/reach/fresh.coax presence -->
```jai
// Whether to keep sample s, given the sample held for that host (have
// says whether there is one).
reach_presence_accept :: (held:ReachSample,have:bool,s:ReachSample)->u32 {
  if s.observed_at>s.received_at+RF_SKEW {return RF_FUTURE;}
  if s.received_at-s.observed_at>RF_MAX_AGE {return RF_STALE;}
  if have && s.generation<held.generation {return RF_ROLLBACK;}
  if have && s.generation==held.generation && s.observed_at<=held.observed_at {return RF_NOT_NEWER;}
  return RF_OK;
}
// Whether a kept sample is still fresh at now, the reader's clock.
reach_presence_fresh :: (s:ReachSample,now:int)->bool {
  return now-s.received_at<=RF_MAX_AGE && s.received_at-s.observed_at<=RF_MAX_AGE;
}
```

### Route generations

Hints describe the routes of one host generation. After a restart the old
ports and addresses may belong to something else, so the first selection
step refuses the whole set as `stale` when it names another generation than
the current presence, or when it has expired. A device without fresh
presence cannot tell which generation is current, so it uses no hints.

<!-- source: coax/lib/nostr/reach/fresh.coax hints -->
```jai
// Whether a hint set of this generation and expiry may be tried at now,
// against the presence sample held for its host.
reach_hints_usable :: (generation:int,expires_at:int,presence:ReachSample,have:bool,now:int)->u32 {
  if !have || !reach_presence_fresh(presence,now) || generation!=presence.generation {return RF_STALE;}
  if now>=expires_at {return RF_STALE;}
  return RF_OK;
}
```

### Worked example: generation 4, by hand

The host restarted into generation 4. Its sample observed at T0 arrives at
T0 + 2: 0 ≤ 2 + 30, and 2 − 0 = 2 ≤ 180, and nothing is held, so the device
keeps it.

| Sample or hint set | Arithmetic | Result |
| --- | --- | --- |
| generation 3, observed T0 + 60, received T0 + 61 | 3 < 4 | rollback |
| generation 4, observed T0, received T0 + 9 | same generation, T0 ≤ T0 | not newer |
| generation 4, observed T0 + 40, received T0 + 5 | 40 > 5 + 30 | future |
| generation 5, observed T0 − 200, received T0 + 10 | 10 − (−200) = 210 > 180 | stale |
| hints of generation 3, now T0 + 60 | presence fresh (58 ≤ 180), 3 ≠ 4 | stale |
| hints of generation 4, now T0 + 60 | fresh, 4 = 4, T0 + 60 < T0 + 3600 | usable |
| hints of generation 4, now T0 + 183 | 183 − 2 = 181 > 180: presence stale | stale |
| hints of generation 4 expiring at T0 + 60, now T0 + 60 | 60 ≥ 60 | stale |

A rejected sample from generation 5 does not move the device forward: the
device still holds generation 4 and keeps waiting for a fresh sample.

## What is not built

- **Issuing grants.** The Coax code opens and checks grants; nothing in
  Coax seals one yet. The fixtures were sealed by the TypeScript generator.
- **Workspace-scoped grants.** A grant with a `scope` field or the
  `openagents.workspace-scope.v1` feature refuses as `HG_SCOPED`.
- **Invitations, connect codes and redemption.** NIP-HOST's invitation
  strings, the camera connect code and `enroll.redeem` have no Coax code.
- **Enrollment over iroh.** The NIP carries a redemption on the iroh ALPN
  `openagents/enroll/1` as length-prefixed JSON messages: the device names
  the invitation, the host answers its key, relay and clock, and the device
  sends its signed redemption. iroh proves only the endpoint, never a
  principal. Not built.
- **Reverse enrollment for a headless host.** The host seals an enrollment
  request to the owner and its administrators and shows an 8-character
  Crockford base32 code; an approver types the code, and the host compares
  only `SHA-256("openagents.host-enrollment-code.v1:" ‖ enrollment ‖ ":" ‖
  code)`, closing the request after 5 mismatches. Not built.
- **Delegation, renewal and the operations.** No `access_admin` chains,
  no renewal on an open channel, no request bodies. `host_admit` takes the
  right an operation needs as a number.
- **The owner directory, compatibility, hint address rules and selection
  order** (direct classes before relay, `reachable` before `unknown`, never
  loopback from another machine), **placement** and **the direct channel**
  with its handshake transcript and session keys. Before the reset these
  were Rust crates; none is ported to Coax yet.

## The example

[nostr/examples/hosts-reach.coax](../examples/hosts-reach.coax) runs the
three parts of the chapter with the test keys and a fixed clock. The device
reads the fixture grant's original event and binds the saved access record
with device 3's key (a real BIP-340 verification, ECDH and NIP-44
decryption), then binds five broken records. The host builds its record
from the bound grant and admits the requests of the worked example, revokes
device 3, and admits the re-enrolled grant. The device then keeps and
refuses presence samples and judges four hint sets.

<!-- source: nostr/examples/hosts-reach.coax example -->
```jai
// Chapter 75: a host grant from envelope to admission, and REACH freshness.
// The device binds the grant the host sealed to it (signature, NIP-44,
// envelope, body); the host admits requests against its own record at a
// fixed clock; the device keeps presence samples and judges a hint set.
// The sealed grants are coax/tests/nostr/host_grant_vectors.coax, made by an
// independent implementation. Keys are the scalars 1 (host), 2 (owner),
// 3 (device) and 7 (another device): TEST ONLY.
#load "../../coax/lib/nostr/host/grant.coax";
#load "../../coax/lib/nostr/host/admit.coax";
#load "../../coax/lib/nostr/reach/fresh.coax";
#import "Basic";

T0 :: 1790000000;
DEVICE :: "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
OTHER :: "5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc";
GRANT_1 :: "1111111111111111111111111111111111111111111111111111111111111111";
GRANT_2 :: "3333333333333333333333333333333333333333333333333333333333333333";
REQUEST :: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
EVENT_1 :: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
EVENT_2 :: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

to_hex :: (bytes:[]u8,out:[]u8)->string {
  r:=hex_encode_checked(cast(string)bytes,out,false);return cast(string)out[0..<r.written];
}
// The device: bind an Access wire with device 3's key (or device 7's).
open_access :: (wire:[]u8,other:bool)->HostGrant {
  secret:=ifx other then HG_OTHER_SECRET else HG_DEVICE_SECRET;x:=ifx other then HG_OTHER_X else HG_DEVICE_X;
  work:[HG_WORK]u8;
  return host_access_bind_checked(wire,secret[..],x[..],work[..],false);
}
GRANT_WORDS :: [13]string.["ok","input","wire","event","signature","artifact","key","decrypt","envelope","scoped","grant","issuer","projection"];
ADMIT_WORDS :: [10]string.["admitted","retry: the retained reply, no new effect","silent: the request expired",
  "malformed","early","not_admitted","revoked","expired","stale","conflict"];
REACH_WORDS :: [5]string.["kept","future","stale","rollback","not newer"];
HINT_WORDS :: [3]string.["usable","future","stale"];

make_request :: (grant:string,epoch:int,at:int,event:string,right:int)->HostRequest {
  q:HostRequest;q.request=REQUEST;q.event=event;q.signer=DEVICE;q.grant=grant;q.epoch=epoch;
  q.issued_at=at;q.expires_at=at+60;q.right=right;return q;
}
show :: (label:string,g:[]HostGrantRecord,e:[]HostEpoch,r:[]HostRetained,q:HostRequest,now:int) {
  print("%: %\n",label,ADMIT_WORDS[host_admit(g,e,r,q,now)]);
}
make_sample :: (generation:int,observed:int,received:int)->ReachSample {
  s:ReachSample;s.generation=generation;s.observed_at=observed;s.received_at=received;return s;
}
```

<!-- source: nostr/examples/hosts-reach.coax main -->
```jai
main :: () {
  print("the device opens its grant\n");
  original:=HG_ORIGINAL_OK;pre:[HG_REGION]u8;e:=nostr_event_read(original,pre[..],false);
  h:[128]u8;
  print("original: kind %, created_at %, % bytes\n",e.kind,e.created_at,original.count);
  print("  id %\n",to_hex(e.id[..],h[..]));
  print("  signer %\n",to_hex(e.public_key[..],h[..]));
  print("  tags %\n",original[e.tags_from..<e.tags_to]);
  wire:=HG_ACCESS_OK;g:=open_access(wire[..],false);
  print("bind: %, epoch %, issued_at %, expires_at %, lifetime % s, % rights, original kept: %\n",
    GRANT_WORDS[g.error],g.epoch,g.issued_at,g.expires_at,g.expires_at-g.issued_at,g.rights,g.original==original);
  print("the signed body: %\n",HG_FIXTURE_GRANT);

  print("refusals at the device\n");
  forged:=HG_ACCESS_FORGED;print("first signature digit changed: %\n",GRANT_WORDS[open_access(forged[..],false).error]);
  sub:=HG_ACCESS_SUBSTITUTED;print("projection names relay wss://other.example.com/: %\n",GRANT_WORDS[open_access(sub[..],false).error]);
  mine:=HG_ACCESS_FOR_OTHER;print("sealed to device 7, opened by device 3: %\n",GRANT_WORDS[open_access(mine[..],false).error]);
  print("the same, opened by device 7: %\n",GRANT_WORDS[open_access(mine[..],true).error]);
  mail:=HG_ACCESS_MAILBOX;print("mailbox is not the grant id: %\n",GRANT_WORDS[open_access(mail[..],false).error]);
  wrong:=HG_ACCESS_NOT_HOST;print("signed by the owner, not the host: %\n",GRANT_WORDS[open_access(wrong[..],false).error]);

  print("the host admits requests, now = T0 + 110\n");
  grants:[2]HostGrantRecord;epochs:[1]HostEpoch;kept:[1]HostRetained;
  grants[0].grant=GRANT_1;grants[0].device=DEVICE;grants[0].epoch=g.epoch;
  grants[0].issued_at=g.issued_at;grants[0].expires_at=g.expires_at;grants[0].rights=HA_STANDARD;
  epochs[0].device=DEVICE;epochs[0].epoch=0;
  one:=grants[0..<1];none:=kept[0..<0];
  q:=make_request(GRANT_1,0,T0+100,EVENT_1,HA_OPERATE);
  show("task.create (operate), epoch 0",one,epochs[..],none,q,T0+110);
  kept[0].request=REQUEST;kept[0].event=EVENT_1;kept[0].expires_at=q.expires_at;
  show("the same event again",one,epochs[..],kept[..],q,T0+120);
  show("new bytes under the same request ID",one,epochs[..],kept[..],make_request(GRANT_1,0,T0+100,EVENT_2,HA_OPERATE),T0+120);
  show("device.revoke (access_admin)",one,epochs[..],none,make_request(GRANT_1,0,T0+100,EVENT_1,HA_ACCESS_ADMIN),T0+110);
  stolen:=q;stolen.signer=OTHER;show("device 7 presents device 3's grant",one,epochs[..],none,stolen,T0+110);
  show("the request again at T0 + 161",one,epochs[..],kept[..],q,T0+161);
  long:=q;long.expires_at=q.issued_at+61;show("a 61-second request",one,epochs[..],none,long,T0+110);

  print("the owner revokes device 3\n");
  print("new epoch: %\n",host_revoke(one,epochs[..],DEVICE));
  show("the same event again",one,epochs[..],kept[..],q,T0+120);
  show("a new request under grant 1",one,epochs[..],none,make_request(GRANT_1,0,T0+130,EVENT_2,HA_OPERATE),T0+130);
  print("device 3 enrolls again: grant 2 at epoch 1\n");
  grants[1]=grants[0];grants[1].grant=GRANT_2;grants[1].epoch=1;grants[1].revoked=false;
  grants[1].issued_at=T0+200;grants[1].expires_at=T0+86600;
  both:=grants[..];
  show("grant 2 named with epoch 0",both,epochs[..],none,make_request(GRANT_2,0,T0+210,EVENT_2,HA_OPERATE),T0+210);
  show("grant 2 at epoch 1",both,epochs[..],none,make_request(GRANT_2,1,T0+210,EVENT_2,HA_OPERATE),T0+210);
  show("grant 2 at epoch 1, now = its expires_at",both,epochs[..],none,make_request(GRANT_2,1,T0+86540,EVENT_2,HA_OPERATE),T0+86600);

  print("REACH: presence and hints, received by the device's clock\n");
  held:ReachSample;have:=false;
  s:=make_sample(4,T0,T0+2);r:=reach_presence_accept(held,have,s);
  print("generation 4, observed T0, received T0 + 2: %\n",REACH_WORDS[r]);
  if r==RF_OK {held=s;have=true;}
  print("generation 3 replayed, observed T0 + 60, received T0 + 61: %\n",REACH_WORDS[reach_presence_accept(held,have,make_sample(3,T0+60,T0+61))]);
  print("generation 4 again, observed T0, received T0 + 9: %\n",REACH_WORDS[reach_presence_accept(held,have,make_sample(4,T0,T0+9))]);
  print("generation 4, observed T0 + 40, received T0 + 5: %\n",REACH_WORDS[reach_presence_accept(held,have,make_sample(4,T0+40,T0+5))]);
  print("generation 5, observed T0 - 200, received T0 + 10: %\n",REACH_WORDS[reach_presence_accept(held,have,make_sample(5,T0-200,T0+10))]);
  print("hints of generation 3 (the old routes), now T0 + 60: %\n",HINT_WORDS[reach_hints_usable(3,T0+3600,held,have,T0+60)]);
  print("hints of generation 4, now T0 + 60: %\n",HINT_WORDS[reach_hints_usable(4,T0+3600,held,have,T0+60)]);
  print("hints of generation 4, now T0 + 183: %\n",HINT_WORDS[reach_hints_usable(4,T0+3600,held,have,T0+183)]);
  print("hints of generation 4 that expired at T0 + 60, now T0 + 60: %\n",HINT_WORDS[reach_hints_usable(4,T0+60,held,have,T0+60)]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/hosts-reach.coax --run-steps 250000000
```

Its output, in debug and release builds alike
([nostr/fixtures/hosts-reach.out](../fixtures/hosts-reach.out)):

```text
the device opens its grant
original: kind 3188, created_at 1790000000, 1969 bytes
  id 1b9bc616c2ce5998bed89393335a5190ca2b715ca18c6e1409c44d4d3d740bf2
  signer 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
  tags [["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["h","1111111111111111111111111111111111111111111111111111111111111111"],["t","oa:artifact:v1"]]
bind: ok, epoch 0, issued_at 1790000000, expires_at 1790086400, lifetime 86400 s, 4 rights, original kept: true
the signed body: {"device":"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","epoch":0,"expires_at":1790086400,"grant":"1111111111111111111111111111111111111111111111111111111111111111","host":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","issued_at":1790000000,"origin":{"id":"2222222222222222222222222222222222222222222222222222222222222222","issuer":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","kind":"invitation"},"owner":"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5","relay":"wss://relay.example.com/","requires":[],"rights":["observe","operate","terminal","review"],"v":"openagents.host-grant.v1"}
refusals at the device
first signature digit changed: signature
projection names relay wss://other.example.com/: projection
sealed to device 7, opened by device 3: artifact
the same, opened by device 7: ok
mailbox is not the grant id: issuer
signed by the owner, not the host: issuer
the host admits requests, now = T0 + 110
task.create (operate), epoch 0: admitted
the same event again: retry: the retained reply, no new effect
new bytes under the same request ID: conflict
device.revoke (access_admin): not_admitted
device 7 presents device 3's grant: not_admitted
the request again at T0 + 161: silent: the request expired
a 61-second request: malformed
the owner revokes device 3
new epoch: 1
the same event again: revoked
a new request under grant 1: revoked
device 3 enrolls again: grant 2 at epoch 1
grant 2 named with epoch 0: stale
grant 2 at epoch 1: admitted
grant 2 at epoch 1, now = its expires_at: expired
REACH: presence and hints, received by the device's clock
generation 4, observed T0, received T0 + 2: kept
generation 3 replayed, observed T0 + 60, received T0 + 61: rollback
generation 4 again, observed T0, received T0 + 9: not newer
generation 4, observed T0 + 40, received T0 + 5: future
generation 5, observed T0 - 200, received T0 + 10: stale
hints of generation 3 (the old routes), now T0 + 60: stale
hints of generation 4, now T0 + 60: usable
hints of generation 4, now T0 + 183: stale
hints of generation 4 that expired at T0 + 60, now T0 + 60: stale
```

The admissions and the REACH lines match the two worked examples. The
refusals at the device each stop at a different step of the bind: the
changed signature digit at `signature`, before any decryption; a
projection that names another relay at `projection`, although the envelope
is genuine; a grant sealed to device 7 at `artifact`, because its `p` tag
is not device 3 (device 7 opens the same record); a mailbox that is not the
grant ID, and a grant signed by the owner instead of the host, at `issuer`.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/hosts-reach.coax](../examples/checks/hosts-reach.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The fixture grant opened by
  [nip59_reference.py](../../coax/tests/nostr/nip59_reference.py), a
  pure-Python NIP-01, BIP-340 and NIP-44 v2 written from the specifications
  with only `hashlib`, `hmac` and integers. It recomputed the event's id,
  verified its signature, derived the conversation key of device 3 and the
  host, decrypted the envelope, and found the inline grant's JCS bytes equal
  to the fixture body, 668 bytes with the digest the envelope names. The
  Coax reader's id, Coax's ECDH conversation key and Coax's SHA-256 of the
  body must give the same values.
- The lifetime and tombstone arithmetic worked by hand above.
- PLAN.md's "Done means": a revoked grant, an expired grant and a replayed
  grant (from before a revocation, at the old epoch) refuse, a replayed
  request earns only its retained reply and nothing after its window, and
  hints of a stale generation are not used.

<!-- source: nostr/examples/checks/hosts-reach.coax check -->
```jai
// HG_ORIGINAL_OK opened by coax/tests/nostr/nip59_reference.py, a pure-Python
// NIP-01, BIP-340 and NIP-44 v2 that shares no code with Coax (or with
// nostr-effect, which sealed it): the NIP-01 id of its fields, BIP-340 verify
// true, the conversation key of device 3 and the signer, and from the
// decrypted envelope the artifact's digest, size and retain_until. The inline
// grant, as JCS bytes, equals HG_FIXTURE_GRANT and hashes to that digest.
PY_ID :: "1b9bc616c2ce5998bed89393335a5190ca2b715ca18c6e1409c44d4d3d740bf2";
PY_CK :: "68ace26dd21fd98a8781d65588d0a7bfb3746974cafdfbf8ea797fb42d251b9d";
PY_DIGEST :: "84eb6b0a96c6f74ad4d8cd327a54eaddd40405ef1acf108f2b593575cdf15644";
PY_SIZE :: 668;
PY_RETAIN_UNTIL :: 1790086460;

hex32 :: (b:[]u8,out:[]u8)->string {r:=hex_encode_checked(cast(string)b,out,false);return cast(string)out[0..<r.written];}

#test "the original's id and conversation key match the Python reference" {
  pre:[HG_REGION]u8;e:=nostr_event_read(HG_ORIGINAL_OK,pre[..],false);h:[64]u8;
  #assert e.error==NE_OK && hex32(e.id[..],h[..])==PY_ID;
  ck:[32]u8;ew:[1]EcdhScratch;dev:=HG_DEVICE_SECRET;peer:=e.public_key;
  #assert nip44_conversation_key_from_keys_checked(dev[..],peer[..],ew[..],ck[..],false)==NIP44_OK;
  #assert hex32(ck[..],h[..])==PY_CK;
}
#test "the signed body hashes to the envelope's digest and size" {
  body:=HG_FIXTURE_GRANT;b:[1024]u8;for i:0..<body.count {b[i]=body[i];}
  d:[32]u8;#assert sha256_hash_checked(b[0..<body.count],d[..],false)==SHA_OK;
  h:[64]u8;#assert hex32(d[..],h[..])==PY_DIGEST && body.count==PY_SIZE;
  // The lifetime by hand: 1790086400 - 1790000000 = 86400 s, a day, under 30 days.
  #assert 1790086400-1790000000==86400 && 86400<=30*24*3600 && 30*24*3600==HG_MAX_LIFETIME;
  // The sealer asked relays to retain it 60 s past expiry: NIP-HOST's tombstone window.
  #assert PY_RETAIN_UNTIL==1790086400+HA_WINDOW;
}
#test "binding gives what the Python reference decrypted" {
  w:=HG_ACCESS_OK;g:=open_access(w[..],false);
  #assert g.error==HG_OK && g.epoch==0 && g.issued_at==1790000000 && g.expires_at==1790086400 && g.rights==4 && g.original==HG_ORIGINAL_OK;
}

// PLAN.md's Done means, worked by hand from NIP-HOST's rules: grant 1 for
// device 3 at epoch 0, T0 to T0 + 86400, standard rights.
grant_one :: (g:[]HostGrantRecord,revoked:bool) {
  g[0].grant=GRANT_1;g[0].device=DEVICE;g[0].issued_at=T0;g[0].expires_at=T0+86400;g[0].rights=HA_STANDARD;g[0].revoked=revoked;
}
#test "Done means: a revoked, expired or replayed grant refuses" {
  ep:[1]HostEpoch;ep[0].device=DEVICE;r1:[1]HostRetained;none:=r1[0..<0];
  q:=make_request(GRANT_1,0,T0+100,EVENT_1,HA_OPERATE);
  live:[1]HostGrantRecord;grant_one(live[..],false);#assert host_admit(live[..],ep[..],none,q,T0+100)==HA_OK;
  dead:[1]HostGrantRecord;grant_one(dead[..],true);#assert host_admit(dead[..],ep[..],none,q,T0+100)==HA_REVOKED;
  late:=make_request(GRANT_1,0,T0+86340,EVENT_1,HA_OPERATE);
  #assert host_admit(live[..],ep[..],none,late,T0+86399)==HA_OK && host_admit(live[..],ep[..],none,late,T0+86400)==HA_EXPIRED;
  // Replayed: the grant from before a revocation, at the old epoch, after re-enrollment.
  ep[0].epoch=1;#assert host_admit(live[..],ep[..],none,q,T0+100)==HA_STALE;
  // Replayed: the request itself, after its 60 s, earns nothing; inside them, only the retained reply.
  ep[0].epoch=0;r1[0].request=REQUEST;r1[0].event=EVENT_1;r1[0].expires_at=T0+160;
  #assert host_admit(live[..],ep[..],r1[..],q,T0+160)==HA_RETRY && host_admit(live[..],ep[..],r1[..],q,T0+161)==HA_SILENT;
}
#test "Done means: a stale route generation is not used" {
  held:=make_sample(4,T0,T0+2);
  #assert reach_hints_usable(3,T0+3600,held,true,T0+60)==RF_STALE;
  #assert reach_hints_usable(4,T0+3600,held,true,T0+60)==RF_OK;
  // The old generation's presence cannot come back either.
  #assert reach_presence_accept(held,true,make_sample(3,T0+60,T0+61))==RF_ROLLBACK;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/hosts-reach.coax --steps 2000000000 --run-steps 250000000
```

`coax test` runs 381 checks, including the loaded files' own checks; all
pass.

### The library's own checks

`admit.coax` checks each outcome at its boundary second (a 60 s request
admits and a 61 s one does not; a grant admits at its last second and is
expired at its end), the order of the checks (revoked before expired, a
retry after revocation refused), a copied grant presented by another
device, a missing right, both epoch rules, retained replies that have
lapsed, and revocation that reaches every grant of one device and no other.
`fresh.coax` checks the skew and age limits at their edges, rollback and
not-newer against a held sample, and each reason a hint set is stale.
`grant.coax`'s checks cover the bind: every body rule, each projected
field changed, tags in shapes the reader would not produce, and the sealed
fixtures for each refusal.

```shell
coax/bin/coax test coax/lib/nostr/host/admit.coax --library
coax/bin/coax mutate coax/lib/nostr/host/admit.coax --library
coax/bin/coax test coax/lib/nostr/reach/fresh.coax --library
coax/bin/coax mutate coax/lib/nostr/reach/fresh.coax --library
coax/bin/coax test coax/lib/nostr/host/grant.coax --run-steps 250000000
coax/tests/nostr/host_grant_targets.sh
```

On 2026-11-01, `admit.coax` passed its 10 checks and `coax mutate` killed
all 73 mutants; `fresh.coax` passed its 5 checks and `coax mutate` killed
all 32; none survived and none is marked equivalent. `grant.coax`'s checks
ran as part of the example's 381. `grant.coax` gained only comment lines
(the listing markers), so its mutation run and the native, wasm32 and iOS
simulator runs of `host_grant_targets.sh` were not repeated; nor was the
full mutation run (`COAX_MUTATION_FULL=1`).

## Summary

- A grant names a host, an owner and a device (three distinct keys), a
  relay, canonical rights, an epoch, its origin and a lifetime of at most
  30 days. The host seals it to the device as a 3188 private artifact whose
  mailbox is the grant ID.
- `host_access_bind_checked` proves that a saved grant is exactly what the
  host signed for this device, and nothing more.
- `host_admit` checks the request's window, then the grant (revoked,
  expired by the host's clock, epochs), then the right, then the request ID:
  an exact retry gets the retained reply, and every check runs again.
- `host_revoke` revokes all of a device's grants and advances its epoch, so
  an older grant is stale.
- `reach_presence_accept` keeps the newest presence sample by the reader's
  clock and never rolls a host back; `reach_hints_usable` uses hints only
  for the generation of fresh presence.

## Dependencies

- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [49. HTTP auth](ch049-http-auth.md)

## Sources

- [OpenAgents NIP-HOST](../nips/openagents/NIP-HOST.md)
- [OpenAgents NIP-REACH](../nips/openagents/NIP-REACH.md)
- [OpenAgents shared contracts](../nips/openagents/contracts.md)
- [NIP-44](../nips/official/44.md)
- [RFC 8785, JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
