# 92. Private agent state

An agent keeps a memory between sessions, and its owner wants to watch
what it does while it runs. Neither belongs in public. NIP-AE stores the
memory as signed, encrypted records that only the agent and its owner can
read. NIP-AO streams the live activity as encrypted frames that relays
pass on and never keep. This chapter builds the reader and writer for
NIP-AE records, the relay's check of NIP-AO frames, and one relay entry
that stores the records as ciphertext and never writes a frame to its
snapshot. It runs them on one slug: the agent writes it twice, the owner
reads both versions and refuses a replayed old one, and a canary written
into one record and one frame shows up in what the owner decrypts and
nowhere in the relay's snapshot or audit log.

Part VI, Every NIP. Result: NIP-AE engrams (30174) sealed, opened, chosen
by head selection and protected from rollback; NIP-AO observer frames
(24200) admitted by the relay only between an owned agent and owner, fanned
out in memory, never stored or logged, and read by the client; the
NIP-PMA reservation (30179) refused. NIP-AP personas and NIP-AM turn
metrics are not built yet; see [What is not built](#what-is-not-built).

## What this chapter teaches

- How NIP-AE addresses a record: the `d` tag is an HMAC of the slug under
  the NIP-44 conversation key, so a relay sees an address and learns
  nothing about the slug.
- Who can read and write: the agent and the owner share one conversation
  key, so both decrypt every record, and anyone else fails. Only the agent
  signs records.
- The order of NIP-AE's validity rules, and why the signature comes before
  decryption.
- Head selection: the newest `created_at` wins, and a tie goes to the
  lowest id. Held as a local view, the same rule refuses a rollback.
- Writing at `max(now, T + 1)`, and why a head far in the future is a
  conflict, not a write.
- Tombstones: a record whose value is `null` says the slug is absent.
- NIP-AO's ephemerality contract: a relay checks a frame's direction and
  the agent-owner pair, fans it out in memory and never writes it to
  storage, search or an audit log.
- How a canary checks that no decrypted body reaches a public store.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nipae.coax](../../coax/lib/nostr/nipae.coax) | `ae_seal(slug, body, agent, owner_x, nonce, aux, created_at, w, out)` | Checks a body as a reader will, encrypts it under K_c and signs a kind 30174 |
| [nipae.coax](../../coax/lib/nostr/nipae.coax) | `ae_open(text, secret, agent_x, owner_x, as_owner, w, a)` | NIP-AE's five validity rules, in order, for the agent or the owner |
| [nipae.coax](../../coax/lib/nostr/nipae.coax) | `ae_head(records, d)`, `ae_accept(held, next)` | Head selection, and the same rule as a rollback refusal |
| [nipae.coax](../../coax/lib/nostr/nipae.coax) | `ae_write_at(now, head)` | The next `created_at`, or a conflict |
| [nipao.coax](../../coax/lib/nostr/nipao.coax) | `ao_admit(text, pairs, now, scratch)` | The relay's check of one observer frame |
| [nipao.coax](../../coax/lib/nostr/nipao.coax) | `ps_route(text, pairs, now, scratch, snapshot)` | The relay's entry for private agent state: memory only, refused, or stored |
| [nipao.coax](../../coax/lib/nostr/nipao.coax) | `ps_log(route, out)` | The audit line, which never names a frame |
| [nipao.coax](../../coax/lib/nostr/nipao.coax) | `ao_open(text, secret, me_x, agent_x, w, a)` | The client's check and decryption of a frame |

Both files are new with this chapter, and both are `#authority` files:
`nipae.coax` decides who reads and writes an agent's memory and which
version counts, and `nipao.coax` decides what a relay admits and what it
stores. The checks came first, and a root-only mutation run kills every
mutant (see [How the code is checked](#how-the-code-is-checked)).
`nipae.coax` reuses chapter 20's NIP-44 (`nip44_encrypt_checked`,
`nip44_decrypt_checked`, `nip44_conversation_key_from_keys_checked`),
Bitcoin chapter 8's HMAC-SHA256, chapter 21's event reader and signer, and the
JSON admission adapter that refuses repeated member names. `nipao.coax`
adds `store_admit.coax`'s `ns_admit_one`, which frames one verified
event for the store's snapshot.

## Agent memory: NIP-AE

An engram is an addressable kind 30174 event. The agent signs it. Its
content is NIP-44 v2 ciphertext under the conversation key K_c of the
agent and its owner. Because the NIP-44 conversation key is symmetric,
the agent computes K_c from its secret and the owner's key, and the owner
computes the same K_c from its secret and the agent's key:

```text
K_c = nip44_conversation_key(seckey_a, pubkey_o)
    = nip44_conversation_key(seckey_o, pubkey_a)
```

Anyone else computes a different key, and NIP-44's MAC check fails. That
is the whole access rule for reading. For writing, a record counts only
when the agent signed it.

Every procedure returns one of these codes:

<!-- source: coax/lib/nostr/nipae.coax codes -->
```jai
AE_OK :: cast(u32)0;
AE_INPUT :: cast(u32)1;     // a key, nonce, aux or work view is refused, or the body is over 65,535 bytes
AE_EVENT :: cast(u32)2;     // not a NIP-01 event with its own id
AE_SIGNATURE :: cast(u32)3;
AE_KIND :: cast(u32)4;      // not kind 30174
AE_AUTHOR :: cast(u32)5;    // not signed by the agent
AE_TAGS :: cast(u32)6;      // not exactly one d of 64 hex and one p, or p is not the owner
AE_DECRYPT :: cast(u32)7;   // the content does not decrypt under this reader's K_c
AE_JSON :: cast(u32)8;      // not one JSON object, or a member name repeats
AE_SLUG :: cast(u32)9;      // the slug breaks the grammar
AE_ADDRESS :: cast(u32)10;  // the slug does not derive the d tag
AE_SHAPE :: cast(u32)11;    // slug, value or profile missing or of the wrong type
AE_LIMIT :: cast(u32)12;    // the caller's arena or output is too small
AE_STALE :: cast(u32)13;    // not newer than the head held: a rollback
AE_CONFLICT :: cast(u32)14; // the next created_at runs too far past the clock

AE_KIND_ENGRAM :: 30174;
AE_MAX_BODY :: 65535;       // NIP-44's plaintext limit
AE_MAX_SLUG :: 255;
AE_MAX_SEGMENT :: 64;
AE_MAX_AHEAD :: 600;        // seconds; NIP-AE leaves the threshold to the implementation
AE_DOMAIN :: "agent-memory/v1/d-tag";
AE_REGION :: 131072;
```

The record an open returns has a public part (the event id, `created_at`
and `d`) and a private part (the slug, the type and the value). The
private part is copied into the caller's arena; the decrypted body and
K_c stay in the `AeWork`, which every call clears before it returns:

<!-- source: coax/lib/nostr/nipae.coax record -->
```jai
AeText :: struct {buf:[]u8;at:int;}
// The private workspace: K_c and every decrypted byte live here, and seal
// and open leave it all zero.
AeWork :: struct {pre:[AE_REGION]u8;body:[AE_REGION]u8;cipher:[2*AE_REGION]u8;scratch:[AE_REGION]u8;ck:[32]u8;}
AeRecord :: struct {
  error:u32=AE_INPUT;
  event:string; created_at:int; d:string;   // public: the signed event
  slug:string; core:bool; tombstone:bool;   // private: from the decrypted body
  value:string;                             // a memory's value, or core's profile
}
AeSealed :: struct {error:u32=AE_INPUT; written:int;}
AeWhen :: struct {error:u32; created_at:int;}
```

### Slugs and addresses

A slug names a record. It is `core` (the agent's identity, rules and
goals: one per agent and owner) or a memory slug such as `mem/example`.
NIP-AE gives the grammar:

```text
^mem/[a-z0-9][a-z0-9_-]{0,63}(/[a-z0-9][a-z0-9_-]{0,63})*$
```

with at most 255 bytes in all. Each segment starts with a lowercase
letter or digit and runs at most 64 characters:

<!-- source: coax/lib/nostr/nipae.coax slug -->
```jai
// "core", or ^mem/[a-z0-9][a-z0-9_-]{0,63}(/[a-z0-9][a-z0-9_-]{0,63})*$
// in at most 255 bytes.
ae_slug_ok :: (s:string)->bool {
  if s=="core" {return true;}
  if s.count<5 || s.count>AE_MAX_SLUG || s[0..<4]!="mem/" {return false;}
  run:=0;   // characters in the current segment
  for i:4..<s.count {
    c:=s[i];
    if c=='/' {if run==0 {return false;}run=0;continue;}
    first:=(c>='a' && c<='z') || (c>='0' && c<='9');
    if !first && (run==0 || (c!='_' && c!='-')) {return false;}
    run+=1;if run>AE_MAX_SEGMENT {return false;}
  }
  return run>0;
}
```

The slug never appears on the wire. The `d` tag is derived from it:

```text
d = lower_hex(HMAC-SHA256(K_c, utf8("agent-memory/v1/d-tag") || 0x00 || utf8(slug)))
```

<!-- source: coax/lib/nostr/nipae.coax derive -->
```jai
// d = lower_hex(HMAC-SHA256(K_c, "agent-memory/v1/d-tag" || 0x00 || slug)),
// written as 64 characters to out. The slug must already be valid (at most
// 255 bytes), so the message fits and the HMAC cannot refuse.
ae_derive :: (ck:[]u8,slug:string,out:[]u8)->string {
  m:[22+AE_MAX_SLUG]u8;
  for i:0..<21 {m[i]=AE_DOMAIN[i];}
  for i:0..<slug.count {m[22+i]=slug[i];}
  mac:[32]u8;st:=hmac_sha256_checked(ck,m[0..<22+slug.count],mac[..],false);
  r:=hex_encode_checked(cast(string)mac[..],out,false);return cast(string)out[0..<64];
}
```

### Worked example: the address of mem/example

The keys are NIP-AE's test keys: the agent's secret is the scalar 1 and
the owner's the scalar 2.

1. The agent's public key is the generator's x coordinate,
   `79be667e…16f81798`; the owner's is 2·G's, `c6047f94…5c709ee5`.
2. K_c is the NIP-44 conversation key of the two, as chapter 20 computes
   it: `c41c7753…1ea1412d`. NIP-AE says it matches NIP-44's own vector
   for the secrets 1 and 2.
3. The HMAC message is the 21 bytes of `agent-memory/v1/d-tag`, one zero
   byte, and the 11 bytes of `mem/example`: 33 bytes.
4. HMAC-SHA256 under K_c gives 32 bytes, and their hex is
   `72d4f962…f8a85cba`, NIP-AE's published `d("mem/example")`.

A relay that holds this record sees `72d4f962…`, the agent's key and the
owner's key. It does not see `mem/example`.

### Reading a record

NIP-AE lists five rules for a valid record. `ae_open` applies them in
that order, after the NIP-01 id check:

1. Kind 30174, signed by the agent, exactly one `d` (64 lowercase hex)
   and exactly one `p`, which is the owner. Other tags, such as `alt`, are
   ignored. Tag names and `p` are compared as JSON strings, so escapes are
   allowed; `d` must be written as plain hex, and a `d` with escaped
   characters is refused.
2. The BIP-340 signature verifies. This comes before decryption, so a
   record nobody authorized costs no decryption and reveals nothing.
3. The content decrypts under K_c and is one JSON object, with no member
   name repeated at any depth. Two parsers that keep the first or the last
   of two `value` members would otherwise disagree about the record.
4. The body's slug is in the grammar and derives the event's `d` tag.
5. The body has the shape its slug calls for: a memory has `value`, a
   string or `null`; `core` has `profile`, a string. Unknown fields are
   ignored.

The tag reader:

<!-- source: coax/lib/nostr/nipae.coax tags -->
```jai
// Exactly one ["d", <64 hex>] (copied to d) and exactly one ["p", owner];
// any other tag (alt, say) is ignored. Names and p are compared as JSON
// strings (escapes allowed); d must be written as plain hex.
ae_tags :: (tags:string,owner_hex:string,d:[]u8)->bool {
  j:[1]Json;json_start(j[..],tags);
  t:=json_next(j[..]);ds:=0;ps:=0;
  t=json_next(j[..]);
  while t==JSON_ARRAY {
    k:=0;name:="";v:=json_next(j[..]);
    while v==JSON_STRING {
      raw:=json_text(j[..]);
      if k==0 {name=ifx json_is(raw,"d") then "d" else ifx json_is(raw,"p") then "p" else "-";}
      if k==1 {
        if name=="d" {ds+=1;if !ae_hex64(raw) {return false;}for i:0..<64 {d[i]=raw[i];}}
        if name=="p" {ps+=1;if !json_is(raw,owner_hex) {return false;}}
      }
      k+=1;v=json_next(j[..]);
    }
    t=json_next(j[..]);
  }
  return ds==1 && ps==1;
}
```

The body reader, which does rules 3 to 5:

<!-- source: coax/lib/nostr/nipae.coax body -->
```jai
// One decrypted body: one JSON object with no repeated member name; its
// slug a string in the grammar that derives d under ck; then its shape.
// The slug and value are unescaped into the arena.
AeBody :: struct {error:u32=AE_JSON; slug:string; core:bool; tombstone:bool; value:string;}
AE_ABSENT :: 0; AE_STRING :: 1; AE_NULL :: 2;   // a wrong type reads as absent
ae_state :: (t:int)->int {return ifx t==JSON_STRING then AE_STRING else ifx t==JSON_NULL then AE_NULL else AE_ABSENT;}
AeStr :: struct {ok:bool;s:string;}
ae_string :: (raw:string,a:[]AeText)->AeStr {
  r:AeStr;n:=json_unescape(raw,a[0].buf[a[0].at..<a[0].buf.count]);
  if n<0 {return r;}   // the arena is full
  r.s=cast(string)a[0].buf[a[0].at..<a[0].at+n];a[0].at+=n;r.ok=true;return r;
}
ae_body :: (body:string,ck:[]u8,d:string,a:[]AeText)->AeBody {
  r:AeBody;
  if ja_admit(body,ja_policy(1),0,0)!=JA_OK {return r;}   // valid JSON, no repeated name at any depth
  j:[1]Json;json_start(j[..],body);
  if json_next(j[..])!=JSON_OBJECT {return r;}
  slug:="";value:="";profile:="";vs:=AE_ABSENT;ps:=AE_ABSENT;ss:=AE_ABSENT;
  k:=json_next(j[..]);
  while k==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);raw:=json_text(j[..]);
    if json_is(key,"slug") {ss=ae_state(v);slug=raw;}
    if json_is(key,"value") {vs=ae_state(v);value=raw;}
    if json_is(key,"profile") {ps=ae_state(v);profile=raw;}
    ok:=json_skip(j[..]);   // an unknown field's object or array; ja_admit read it already
    k=json_next(j[..]);
  }
  r.error=AE_SHAPE;if ss!=AE_STRING {return r;}
  sl:=ae_string(slug,a);r.slug=sl.s;
  r.error=AE_LIMIT;if !sl.ok {return r;}
  r.error=AE_SLUG;if !ae_slug_ok(r.slug) {return r;}
  h:[64]u8;r.error=AE_ADDRESS;if ae_derive(ck,r.slug,h[..])!=d {return r;}
  r.core=r.slug=="core";
  r.error=AE_SHAPE;
  if r.core {if ps!=AE_STRING {return r;}value=profile;}
  else {if vs!=AE_STRING && vs!=AE_NULL {return r;}r.tombstone=vs==AE_NULL;}
  if !r.tombstone {vl:=ae_string(value,a);r.value=vl.s;r.error=AE_LIMIT;if !vl.ok {return r;}}
  r.error=AE_OK;return r;
}
```

And the whole open:

<!-- source: coax/lib/nostr/nipae.coax open -->
```jai
ae_done :: (r:AeRecord,error:u32,w:[]AeWork)->AeRecord {ae_clear(w);out:=r;out.error=error;return out;}
// Opens one kind 30174 for the pair (agent_x, owner_x). The reader is the
// owner when as_owner (secret is the owner's), else the agent. Strings of
// the result live in the arena.
ae_open :: (text:string,secret:[]u8,agent_x:[]u8,owner_x:[]u8,as_owner:bool,w:[]AeWork,a:[]AeText)->AeRecord {
  r:AeRecord;
  if secret.count!=32 || agent_x.count!=32 || owner_x.count!=32 || w.count!=1 {return r;}
  e:=nostr_event_read(text,w[0].scratch[..],false);
  if e.error!=NE_OK {return ae_done(r,AE_EVENT,w);}
  id:=e.id;r.event=ae_hex(a,id[..]);r.created_at=e.created_at;
  if r.event=="" {return ae_done(r,AE_LIMIT,w);}
  if e.kind!=AE_KIND_ENGRAM {return ae_done(r,AE_KIND,w);}
  pk:=e.public_key;if cast(string)pk[..]!=cast(string)agent_x {return ae_done(r,AE_AUTHOR,w);}
  oh:[64]u8;dh:[64]u8;hr:=hex_encode_checked(cast(string)owner_x,oh[..],false);
  if !ae_tags(text[e.tags_from..<e.tags_to],cast(string)oh[..],dh[..]) {return ae_done(r,AE_TAGS,w);}
  r.d=ae_keep(a,cast(string)dh[..]);
  if r.d=="" {return ae_done(r,AE_LIMIT,w);}
  if !nostr_fast_verify_vartime(e.id[..],e.public_key[..],e.signature[..]) {return ae_done(r,AE_SIGNATURE,w);}
  ew:[1]EcdhScratch;peer:=ifx as_owner then agent_x else owner_x;
  if nip44_conversation_key_from_keys_checked(secret,peer,ew[..],w[0].ck[..],false)!=NIP44_OK {return ae_done(r,AE_INPUT,w);}
  n:=json_unescape(text[e.content_from..<e.content_to],w[0].scratch[..]);
  dc:=nip44_decrypt_checked(NIP44_COMPAT,w[0].ck[..],cast(string)w[0].scratch[0..<n],w[0].cipher[..],w[0].body[..],false);
  if dc.error!=NIP44_OK {return ae_done(r,AE_DECRYPT,w);}
  b:=ae_body(cast(string)w[0].body[0..<dc.written],w[0].ck[..],r.d,a);
  r.slug=b.slug;r.core=b.core;r.tombstone=b.tombstone;r.value=b.value;
  return ae_done(r,b.error,w);
}
```

The reader passes its own secret and says whether it is the owner. The
owner pairs its secret with the agent's key, and the agent pairs its
secret with the owner's key. A stranger, or the owner's secret in the
agent's role, derives another key, and the open stops at `decrypt`.

### Writing a record

`ae_seal` reads the body exactly as a reader will before it signs, so the
agent never publishes a record its owner would refuse. A body over 65,535
bytes (NIP-44's plaintext limit) refuses as `input`:

<!-- source: coax/lib/nostr/nipae.coax seal -->
```jai
// Seals body at slug from the agent to owner_x and signs it at created_at
// (from ae_write_at). nonce and aux are 32 fresh random bytes each from the
// host. The body is read exactly as a reader will read it first, so the
// agent never publishes a record its owner would refuse.
ae_seal :: (slug:string,body:string,agent:[]u8,owner_x:[]u8,nonce:[]u8,aux:[]u8,created_at:int,w:[]AeWork,out:[]u8)->AeSealed {
  r:AeSealed;
  // A key that is not 32 bytes is refused by the conversation key below.
  if nonce.count!=32 || aux.count!=32 || w.count!=1 || body.count>AE_MAX_BODY {return r;}
  r.error=AE_SLUG;if !ae_slug_ok(slug) {return r;}
  r=ae_seal_in(slug,body,agent,owner_x,nonce,aux,created_at,w,out);
  ae_clear(w);return r;
}
ae_seal_in :: (slug:string,body:string,agent:[]u8,owner_x:[]u8,nonce:[]u8,aux:[]u8,created_at:int,w:[]AeWork,out:[]u8)->AeSealed {
  r:AeSealed;ew:[1]EcdhScratch;
  if nip44_conversation_key_from_keys_checked(agent,owner_x,ew[..],w[0].ck[..],false)!=NIP44_OK {return r;}
  dh:[64]u8;d:=ae_derive(w[0].ck[..],slug,dh[..]);
  a:=[1]AeText.[AeText.{buf=w[0].scratch[..]}];
  b:=ae_body(body,w[0].ck[..],d,a[..]);
  r.error=b.error;if b.error!=AE_OK {return r;}
  for i:0..<body.count {w[0].body[i]=body[i];}
  // Cannot refuse: the key and nonce are 32 bytes, and an admitted body is
  // 2 to 65,535 bytes.
  e:=nip44_encrypt_checked(NIP44_COMPAT,w[0].ck[..],nonce,w[0].body[0..<body.count],w[0].cipher[..],w[0].pre[..],false);
  oh:[64]u8;hr:=hex_encode_checked(cast(string)owner_x,oh[..],false);
  tags:[160]u8;
  n:=json_raw(tags[..],0,"[[\"d\",\"");n=json_raw(tags[..],n,d);n=json_raw(tags[..],n,"\"],[\"p\",\"");
  n=json_raw(tags[..],n,cast(string)oh[..]);n=json_raw(tags[..],n,"\"]]");
  sw:[1]NostrSignWork;k:=EVENT_MORE;
  while k==EVENT_MORE {k=nostr_event_sign_step(agent,aux,created_at,AE_KIND_ENGRAM,cast(string)tags[0..<n],cast(string)w[0].pre[0..<e.written],sw[..],w[0].scratch[..],out,false);}
  r.error=AE_LIMIT;if k==EVENT_REFUSED {return r;}
  r.error=AE_OK;r.written=k;return r;
}
```

### Which version counts

A relay should return only the newest event per `(kind, pubkey, d)`, but
it may return older ones too, and different relays can disagree. NIP-AE's
head selection decides: among valid records with the same `d`, the
greatest `created_at` wins, and a tie goes to the lowest event id. The
same rule, held as the reader's own view, refuses a rollback: a record
that is not newer than the head the reader already holds is `stale`,
whichever relay sends it.

Writing uses the head too. NIP-AE's Writing step 2 sets `created_at :=
max(now, T + 1)`, where T is the head's `created_at` (0 when there is no
head). A write in the same second as the head therefore still lands one
second later, so the NIP-01 tie rule never decides between two writes
whose ids depend on random nonces. When T is far ahead of the clock, the
head is clock-poisoned, and NIP-AE says to report a conflict instead of
publishing. It leaves the threshold to the implementation; this code uses
600 seconds:

<!-- source: coax/lib/nostr/nipae.coax head -->
```jai
// Equal-length lowercase hex compares as bytes.
ae_lower_id :: (a:string,b:string)->bool {
  for i:0..<a.count {if a[i]<b[i] {return true;}if a[i]>b[i] {return false;}}
  return false;
}
// a replaces b: a later created_at, or the same second and a lower id.
ae_newer :: (a:AeRecord,b:AeRecord)->bool {
  return a.created_at>b.created_at || (a.created_at==b.created_at && ae_lower_id(a.event,b.event));
}
// The head of d among opened records: valid ones only. -1 when none.
ae_head :: (rs:[]AeRecord,d:string)->int {
  best:=-1;
  for i:0..<rs.count {
    if rs[i].error==AE_OK && rs[i].d==d && (best<0 || ae_newer(rs[i],rs[best])) {best=i;}
  }
  return best;
}
// The local view: next replaces held only when it is valid, addresses the
// same d and is newer. held.error != AE_OK means nothing is held yet.
ae_accept :: (held:AeRecord,next:AeRecord)->u32 {
  if next.error!=AE_OK {return next.error;}
  if held.error!=AE_OK {return AE_OK;}
  if next.d!=held.d {return AE_ADDRESS;}
  if !ae_newer(next,held) {return AE_STALE;}
  return AE_OK;
}
// Writing step 2: created_at = max(now, T + 1), T the head's created_at or
// 0; a conflict when that is more than AE_MAX_AHEAD seconds after now.
ae_write_at :: (now:int,head:AeRecord)->AeWhen {
  r:AeWhen;t:=ifx head.error==AE_OK then head.created_at else 0;
  r.created_at=now;if t>=now {r.created_at=t+1;}
  if r.created_at-now>AE_MAX_AHEAD {r.error=AE_CONFLICT;}
  return r;
}
```

### Worked example: two versions and a rollback

The agent writes `mem/example` twice at a fixed clock.

1. First write, clock 1700000000. No head, so T = 0 and `created_at` =
   max(1700000000, 0 + 1) = 1700000000. The body is
   `{"slug":"mem/example","value":"hello, agent memory"}`; with NIP-44
   nonce 1 and zero aux, the event is NIP-AE's Event 1, id
   `f4a59417…88a38689`.
2. Second write, clock 1700000002. The head is version 1, T =
   1700000000, and `created_at` = max(1700000002, 1700000001) =
   1700000002. The body is the tombstone `{"slug":"mem/example","value":null}`;
   with nonce 3 the event is NIP-AE's Event 3, id `c8604bef…7b123b63`.
3. A third write in the same second would get max(1700000002,
   1700000002 + 1) = 1700000003.
4. The owner opens both. Both have `d` = `72d4f962…`. 1700000002 >
   1700000000, so version 2 is the head. It is a tombstone, so
   `mem/example` is absent.
5. A relay now sends version 1 again. The owner holds version 2:
   1700000000 is not greater than 1700000002, and the seconds differ, so
   the id tie rule does not apply. Version 1 is `stale` and the slug stays
   absent.
6. If the held head said 1700000002 + 601 = 1700000603, the next write
   would be at 1700000604, 602 seconds past the clock: more than 600, so
   `conflict`.

## Live activity: NIP-AO

An observer frame is a kind 24200 event. Kind 24200 is in NIP-01's
ephemeral range, and NIP-AO goes further: relays must not write it to
durable storage, search indexes or audit logs, and must fan it out only
through in-memory pub/sub. The content is NIP-44 ciphertext between the
sender and the recipient. A frame has exactly one `p`, one `agent` and one
`frame` tag:

- Telemetry (`frame` = `telemetry`) goes from the agent to the owner:
  signed by the agent, `p` is the owner, `agent` is the agent.
- Control (`frame` = `control`) goes from the owner to the agent: signed
  by the owner, `p` is the agent, `agent` is the agent. The only control
  type is `cancel_turn`.

<!-- source: coax/lib/nostr/nipao.coax codes -->
```jai
AO_FANOUT :: cast(u32)0;    // relay: fan out in memory; client: a frame it reads
AO_DROP :: cast(u32)1;      // an unknown frame value: OK to the publisher, delivered to nobody
AO_EVENT :: cast(u32)2;     // not a NIP-01 event with its own id
AO_SIGNATURE :: cast(u32)3;
AO_KIND :: cast(u32)4;      // not kind 24200
AO_TAGS :: cast(u32)5;      // not exactly one p, one agent and one frame, or p or agent not 64 hex
AO_AUTH :: cast(u32)6;      // "AUTH required": wrong direction, or the pair is not in the ownership records
AO_STALE :: cast(u32)7;     // created_at outside the freshness window
AO_RESERVED :: cast(u32)8;  // kind 30179: NIP-PMA's reservation, refused until its gates exist
AO_FULL :: cast(u32)9;      // the snapshot output or the client's arena is too small
AO_DECRYPT :: cast(u32)10;  // client: the content does not decrypt for this reader
AO_SHAPE :: cast(u32)11;    // client: a required payload field missing or of the wrong type
AO_IGNORED :: cast(u32)12;  // client: an unknown telemetry kind or control type

AO_KIND_FRAME :: 24200;
AO_KIND_PMA :: 30179;
AO_WINDOW :: 300;           // seconds either side of the relay's clock
AO_TELEMETRY :: 0; AO_CONTROL :: 1; AO_UNKNOWN :: 2;
AO_FRAMES :: [2]string.["telemetry","control"];
AO_KINDS :: [4]string.["acp_read","acp_write","turn_started","session_resolved"];
PS_STORED :: 0; PS_MEMORY :: 1; PS_DROPPED :: 2; PS_REFUSED :: 3; PS_EPHEMERAL :: 4;
AO_REGION :: 131072;
```

<!-- source: coax/lib/nostr/nipao.coax record -->
```jai
// One ownership record, as 64-hex keys.
AoPair :: struct {agent:string; owner:string;}
AoFrame :: struct {error:u32=AO_EVENT; frame:int; agent:[64]u8; owner:[64]u8;}
PsRoute :: struct {route:int=PS_REFUSED; error:u32=AO_EVENT; kind:int; id:[64]u8; written:int;}
AoText :: struct {buf:[]u8;at:int;}
AoWork :: struct {scratch:[AO_REGION]u8;cipher:[2*AO_REGION]u8;body:[AO_REGION]u8;ck:[32]u8;}
AoOpened :: struct {error:u32=AO_EVENT; frame:int; name:string; seq:int; payload:string;}
```

The relay's check, in NIP-AO's order: the signature, then the tags (an
unknown `frame` value is dropped: the publisher gets OK and nobody gets
the frame), then authorization, then freshness. `#p` alone is not
authorization: the relay must find the agent-owner pair in its own
ownership records. NIP-AO recommends a window of ±5 minutes around the
relay's clock:

<!-- source: coax/lib/nostr/nipao.coax tags -->
```jai
// Exactly one p and one agent (64 hex each, written as plain hex, copied
// out) and one frame. Returns the frame (AO_TELEMETRY, AO_CONTROL or
// AO_UNKNOWN), or -1.
ao_tags :: (tags:string,p:[]u8,agent:[]u8)->int {
  j:[1]Json;json_start(j[..],tags);
  t:=json_next(j[..]);ps:=0;as:=0;fs:=0;fv:="";
  t=json_next(j[..]);
  while t==JSON_ARRAY {
    k:=0;name:="";v:=json_next(j[..]);
    while v==JSON_STRING {
      raw:=json_text(j[..]);
      if k==0 {name=ifx json_is(raw,"p") then "p" else ifx json_is(raw,"agent") then "a" else ifx json_is(raw,"frame") then "f" else "-";}
      if k==1 {
        if name=="p" {ps+=1;if !ao_hex64(raw) {return -1;}for i:0..<64 {p[i]=raw[i];}}
        if name=="a" {as+=1;if !ao_hex64(raw) {return -1;}for i:0..<64 {agent[i]=raw[i];}}
        if name=="f" {fs+=1;fv=raw;}
      }
      k+=1;v=json_next(j[..]);
    }
    t=json_next(j[..]);
  }
  if ps!=1 || as!=1 || fs!=1 {return -1;}
  return ifx json_is(fv,"telemetry") then AO_TELEMETRY else ifx json_is(fv,"control") then AO_CONTROL else AO_UNKNOWN;
}
```

<!-- source: coax/lib/nostr/nipao.coax admit -->
```jai
ao_owns :: (pairs:[]AoPair,agent:string,owner:string)->bool {
  for q:pairs {if q.agent==agent && q.owner==owner {return true;}}
  return false;
}
ao_fail :: (f:AoFrame,error:u32)->AoFrame {r:=f;r.error=error;return r;}
// The relay's check of a frame whose id and signature verified.
ao_check :: (e:NostrEvent,text:string,pairs:[]AoPair,now:int)->AoFrame {
  r:AoFrame;
  if e.kind!=AO_KIND_FRAME {return ao_fail(r,AO_KIND);}
  p:[64]u8;
  r.frame=ao_tags(text[e.tags_from..<e.tags_to],p[..],r.agent[..]);
  if r.frame<0 {return ao_fail(r,AO_TAGS);}
  if r.frame==AO_UNKNOWN {return ao_fail(r,AO_DROP);}
  sh:[64]u8;key:=e.public_key;hr:=hex_encode_checked(cast(string)key[..],sh[..],false);
  agent:=cast(string)r.agent[..];
  // Telemetry: the agent signs, p is the owner. Control: the owner signs,
  // p is the agent.
  if r.frame==AO_TELEMETRY {
    if cast(string)sh[..]!=agent {return ao_fail(r,AO_AUTH);}
    r.owner=p;
  } else {
    if cast(string)p[..]!=agent {return ao_fail(r,AO_AUTH);}
    r.owner=sh;
  }
  if !ao_owns(pairs,agent,cast(string)r.owner[..]) {return ao_fail(r,AO_AUTH);}
  if e.created_at<now-AO_WINDOW || e.created_at>now+AO_WINDOW {return ao_fail(r,AO_STALE);}
  return ao_fail(r,AO_FANOUT);
}
ao_admit :: (text:string,pairs:[]AoPair,now:int,scratch:[]u8)->AoFrame {
  r:AoFrame;
  e:=nostr_event_read(text,scratch,false);
  if e.error!=NE_OK {return r;}
  if !nostr_fast_verify_vartime(e.id[..],e.public_key[..],e.signature[..]) {return ao_fail(r,AO_SIGNATURE);}
  return ao_check(e,text,pairs,now);
}
```

### The relay's single entry

`ps_route` is the one place where the relay takes an event of private
agent state. A frame goes to `ao_admit` and comes out as memory-only,
dropped or refused; there is no path from it to the snapshot writer.
NIP-PMA reserves kind 30179 for an owner's private managed-agent aggregate
and says relays must reject it until its privacy, transactional,
backup, revocation and capability gates are deployed, so `ps_route`
refuses it. Every other event goes to `ns_admit_one`, which verifies it
and frames it for the snapshot, or stores nothing for an ephemeral kind.
An engram is stored as it was signed: its content is ciphertext.

The audit line names a stored event's kind, id and size, and a refusal's
code. It writes nothing for a frame, whatever happened to it:

<!-- source: coax/lib/nostr/nipao.coax route -->
```jai
// The relay's entry for one EVENT. snapshot receives a stored event's
// frame; a 24200 never touches it.
ps_route :: (text:string,pairs:[]AoPair,now:int,scratch:[]u8,snapshot:[]u8)->PsRoute {
  r:PsRoute;
  e:=nostr_event_read(text,scratch,false);
  if e.error!=NE_OK {return r;}
  id:=e.id;hr:=hex_encode_checked(cast(string)id[..],r.id[..],false);r.kind=e.kind;
  if e.kind==AO_KIND_FRAME {
    f:=ao_admit(text,pairs,now,scratch);r.error=f.error;
    r.route=ifx f.error==AO_FANOUT then PS_MEMORY else ifx f.error==AO_DROP then PS_DROPPED else PS_REFUSED;
    return r;
  }
  r.error=AO_RESERVED;if e.kind==AO_KIND_PMA {return r;}
  a:=ns_admit_one(text,scratch,snapshot,false);
  r.error=AO_FANOUT;
  if a.status==NS_ADMIT_OK {r.route=PS_STORED;r.written=a.written;return r;}
  if a.status==NS_ADMIT_EPHEMERAL {r.route=PS_EPHEMERAL;return r;}
  r.error=ifx a.status==NS_ADMIT_INVALID then AO_SIGNATURE else AO_FULL;
  return r;
}
// The audit line for a route, written to out; returns its length. A frame
// (kind 24200) is never logged, whatever happened to it.
ps_log :: (r:PsRoute,out:[]u8)->int {
  if r.kind==AO_KIND_FRAME {return 0;}
  if r.route==PS_STORED {
    n:=json_raw(out,0,"stored kind ");n=json_write_int(out,n,r.kind);n=json_raw(out,n," id ");
    id:=r.id;n=json_raw(out,n,cast(string)id[..]);n=json_raw(out,n," bytes ");n=json_write_int(out,n,r.written);
    return json_raw(out,n,"\n");
  }
  if r.route==PS_REFUSED {n:=json_raw(out,0,"refused ");n=json_write_int(out,n,cast(int)r.error);return json_raw(out,n,"\n");}
  return 0;
}
// The canary check: does text appear anywhere in bytes?
ps_contains :: (bytes:string,text:string)->bool {
  for i:0..<bytes.count-text.count+1 {if bytes[i..<i+text.count]==text {return true;}}
  return false;
}
```

### The client

The owner's client reads telemetry and the agent reads control. Before it
decrypts, `ao_open` checks that the frame is addressed to this client
(`p`) and comes from the agent it trusts (`agent`). It ignores unknown
telemetry kinds and control types, as NIP-AO requires:

<!-- source: coax/lib/nostr/nipao.coax open -->
```jai
ao_done :: (r:AoOpened,error:u32,w:[]AoWork)->AoOpened {
  for i:0..<AO_REGION {w[0].scratch[i]=0;w[0].body[i]=0;w[0].cipher[i]=0;w[0].cipher[AO_REGION+i]=0;}
  for i:0..<32 {w[0].ck[i]=0;}
  out:=r;out.error=error;return out;
}
// The client: me_x is this client's key (the frame's p), agent_x the agent
// it trusts. The owner reads telemetry; the agent reads control.
ao_open :: (text:string,secret:[]u8,me_x:string,agent_x:string,w:[]AoWork,a:[]AoText)->AoOpened {
  r:AoOpened;
  if secret.count!=32 || w.count!=1 {return r;}
  e:=nostr_event_read(text,w[0].scratch[..],false);
  if e.error!=NE_OK {return ao_done(r,AO_EVENT,w);}
  if !nostr_fast_verify_vartime(e.id[..],e.public_key[..],e.signature[..]) {return ao_done(r,AO_SIGNATURE,w);}
  if e.kind!=AO_KIND_FRAME {return ao_done(r,AO_KIND,w);}
  p:[64]u8;ag:[64]u8;
  r.frame=ao_tags(text[e.tags_from..<e.tags_to],p[..],ag[..]);
  if r.frame<0 {return ao_done(r,AO_TAGS,w);}
  if r.frame==AO_UNKNOWN {return ao_done(r,AO_DROP,w);}
  if cast(string)p[..]!=me_x || cast(string)ag[..]!=agent_x {return ao_done(r,AO_AUTH,w);}
  ew:[1]EcdhScratch;peer:=e.public_key;
  if nip44_conversation_key_from_keys_checked(secret,peer[..],ew[..],w[0].ck[..],false)!=NIP44_OK {return ao_done(r,AO_DECRYPT,w);}
  n:=json_unescape(text[e.content_from..<e.content_to],w[0].scratch[..]);
  dc:=nip44_decrypt_checked(NIP44_COMPAT,w[0].ck[..],cast(string)w[0].scratch[0..<n],w[0].cipher[..],w[0].body[..],false);
  if dc.error!=NIP44_OK {return ao_done(r,AO_DECRYPT,w);}
  return ao_read(cast(string)w[0].body[0..<dc.written],r,w,a);
}
// Telemetry needs seq (an integer), timestamp and kind (strings) and
// payload (an object); control needs type (a string). Then the name must
// be known. The payload and the name are returned from the arena.
ao_read :: (body:string,opened:AoOpened,w:[]AoWork,a:[]AoText)->AoOpened {
  r:=opened;
  if ja_admit(body,ja_policy(1),0,0)!=JA_OK {return ao_done(r,AO_SHAPE,w);}
  j:[1]Json;json_start(j[..],body);
  if json_next(j[..])!=JSON_OBJECT {return ao_done(r,AO_SHAPE,w);}
  // name: where the telemetry kind or the control type sits in body.
  seen:=0;nf:int;nt:int;
  k:=json_next(j[..]);
  while k==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);raw:=json_text(j[..]);
    if json_is(key,"seq") && v==JSON_NUMBER {n:=json_int(raw);if n.ok {seen|=1;r.seq=n.value;}}
    if json_is(key,"timestamp") && v==JSON_STRING {seen|=2;}
    if json_is(key,ifx r.frame==AO_TELEMETRY then "kind" else "type") && v==JSON_STRING {seen|=4;nf=j[0].from;nt=j[0].to;}
    if json_is(key,"payload") && v==JSON_OBJECT {seen|=8;}
    ok:=json_skip(j[..]);
    k=json_next(j[..]);
  }
  want:=ifx r.frame==AO_TELEMETRY then 15 else 4;
  if (seen&want)!=want {return ao_done(r,AO_SHAPE,w);}
  name:=body[nf..<nt];known:=false;
  if r.frame==AO_TELEMETRY {for s:AO_KINDS {if json_is(name,s) {known=true;}}}
  else {known=json_is(name,"cancel_turn");}
  if !known {return ao_done(r,AO_IGNORED,w);}
  // Copied out before the work is cleared: the name points into the copy.
  from:=a[0].at;if from+body.count>a[0].buf.count {return ao_done(r,AO_FULL,w);}
  for i:0..<body.count {a[0].buf[from+i]=body[i];}a[0].at+=body.count;
  r.payload=cast(string)a[0].buf[from..<a[0].at];r.name=r.payload[nf..<nt];
  return ao_done(r,AO_FANOUT,w);
}
```

### Worked example: four frames at a fixed clock

The relay's clock is 1777464050, and its ownership records hold one pair:
agent `79be667e…`, owner `c6047f94…`.

1. Telemetry, created 1777464041, signed by `79be667e…`, `agent` =
   `79be667e…`, `p` = `c6047f94…`. The signer is the agent, the pair is in
   the records, and 1777464050 − 1777464041 = 9 ≤ 300: fan out.
2. Control, created 1777464042, signed by `c6047f94…`, `p` = `agent` =
   `79be667e…`: the signer is the owner, the pair is in the records, 8 ≤
   300: fan out.
3. A frame signed by a stranger (`f9308a01…`) that names `79be667e…` as
   the agent: the signer is not the agent, so `AUTH required`.
4. A telemetry frame created 1777463641: 1777464050 − 1777463641 = 409 >
   300, so `stale`.

None of the four reaches the snapshot.

## The canary

Done means, for this chapter, that no decrypted body appears in a public
snapshot or in telemetry. The check is a canary: a string that exists
only in plaintext, `CANARY-5e1f-never-public`. The agent writes it into
an engram (`mem/canary`, value `rotate the deploy key on Friday
CANARY-5e1f-never-public`), and it sends it inside a telemetry frame's
payload. The relay routes every event, and the example then looks for the
canary:

- in the owner's decrypted engram and decrypted frame, where it must be
  (otherwise the check proves nothing);
- in the relay's snapshot bytes and audit log, where it must not be;
- and it looks for the frame's event id in the snapshot and the log,
  where it must not be either.

## What is not built

- NIP-AE: querying the configured relays and taking the union (the NIP-65
  relay list, URL canonicalization), the write-verify step and its
  conflict report, NIP-09 deletion requests, listing across relays and its
  truncation signal, and wiki-link reachability.
- NIP-AO: filling the ownership records (an authenticated lookup; a relay
  can record a pair after it verifies a NIP-OA attestation from
  [chapter 87](ch087-owner-attestation-and-agent-authentication.md)),
  NIP-42 gating of subscriptions, the 100 events per second rate limit,
  the client's subscription filter and ring buffer, NIP-59 wrapping, and
  the meaning of the NIP-29 `h` tag (it is allowed and ignored). The
  example's frames are signed in Python; Coax has no frame sealer yet.
- NIP-AP personas (30175–30178) and their replacement semantics.
- NIP-AM turn metrics (44200): the payload's null-is-not-zero rule, delta
  recomputation and the pricing identity. Its `costUsd` field is an
  estimate in US dollars; this repository settles in Bitcoin only, and a
  reader that sums estimates is a separate decision.
- NIP-PMA beyond the refusal: the aggregate's codec, its compare-and-swap
  generations and every later deployment step.

## The example

[nostr/examples/private-agent-state.coax](../examples/private-agent-state.coax)
runs the worked examples at fixed clocks. The engrams are sealed and
signed by Coax; the frames and the refused engrams were signed by an
independent NIP-44 and BIP-340 in Python
([private_state_vectors.py](../../coax/tests/nostr/private_state_vectors.py)).

The agent's write and the reader's open:

<!-- source: nostr/examples/private-agent-state.coax write -->
```jai
// The agent writes body at slug: created_at from the head it holds and its
// clock, then seal and sign. The signed event goes to out; "" when refused.
WriteResult :: struct {error:u32; created_at:int; text:string;}
seal_at :: (slug:string,body:string,now:int,head:AeRecord,n:int,w:[]AeWork,out:[]u8)->WriteResult {
  r:WriteResult;when:=ae_write_at(now,head);r.created_at=when.created_at;
  r.error=when.error;if when.error!=AE_OK {return r;}
  sk:=key_of(PSV_AGENT_SECRET);ox:=key_of(PSV_OWNER_X);nonce:=nonce_of(n);aux:[32]u8;
  s:=ae_seal(slug,body,sk[..],ox[..],nonce[..],aux[..],when.created_at,w,out);
  r.error=s.error;if s.error==AE_OK {r.text=cast(string)out[0..<s.written];}
  return r;
}
agent_write :: (slug:string,body:string,now:int,head:AeRecord,n:int,w:[]AeWork,out:[]u8)->string {
  r:=seal_at(slug,body,now,head,n,w,out);
  if r.error!=AE_OK {print("write %: %\n",slug,AE_WORDS[r.error]);return "";}
  print("write % at %: event %…\n",slug,r.created_at,r.text[7..<23]);
  return r.text;
}
```

<!-- source: nostr/examples/private-agent-state.coax read -->
```jai
// Opens text as the owner (or the agent, or a stranger by secret).
open_as :: (text:string,secret_hex:string,as_owner:bool,w:[]AeWork,store:[]u8)->AeRecord {
  a:=[1]AeText.[AeText.{buf=store}];
  sk:=key_of(secret_hex);ax:=key_of(PSV_AGENT_X);ox:=key_of(PSV_OWNER_X);
  return ae_open(text,sk[..],ax[..],ox[..],as_owner,w,a[..]);
}
show_record :: (who:string,r:AeRecord) {
  if r.error!=AE_OK {print("%: refused: %\n",who,AE_WORDS[r.error]);}
  else if r.tombstone {print("%: % at %: tombstone (absent)\n",who,r.slug,r.created_at);}
  else {print("%: % at %: \"%\"\n",who,r.slug,r.created_at,r.value);}
}
```

Two versions, then the owner's view and the rollback:

<!-- source: nostr/examples/private-agent-state.coax versions -->
```jai
  e1:=agent_write("mem/example",PSV_BODY1,1700000000,none,1,w[..],e1buf[..]);
  v1:=open_as(e1,PSV_AGENT_SECRET,false,w[..],s1[..]);
  e2:=agent_write("mem/example",PSV_BODY3,1700000002,v1,3,w[..],e2buf[..]);
  print("same second again: created_at %\n",ae_write_at(1700000002,open_as(e2,PSV_AGENT_SECRET,false,w[..],s2[..])).created_at);
  
```

<!-- source: nostr/examples/private-agent-state.coax owner -->
```jai
  r1:=open_as(e1,PSV_OWNER_SECRET,true,w[..],s1[..]);show_record("owner, version 1",r1);
  r2:=open_as(e2,PSV_OWNER_SECRET,true,w[..],s2[..]);show_record("owner, version 2",r2);
  print("d tag: %…\n",r2.d[0..<16]);
  rs:=[2]AeRecord.[r1,r2];h:=ae_head(rs[..],r1.d);
  print("head: version % (created_at %)\n",h+1,rs[h].created_at);
  // A relay replays version 1 to the owner, who holds version 2.
  print("replayed version 1: %\n",AE_WORDS[ae_accept(r2,r1)]);
  
```

The relay routes three engrams, three frames, a stranger's frame and the
NIP-PMA reservation:

<!-- source: nostr/examples/private-agent-state.coax relay -->
```jai
  canary:=agent_write("mem/canary",PSV_CANARY_BODY,1700000004,none,5,w[..],cbuf[..]);
  pairs:=[1]AoPair.[AoPair.{agent=PSV_AGENT_X,owner=PSV_OWNER_X}];
  events:=[8]string.[e1,e2,canary,PSV_TELEMETRY,PSV_CONTROL,PSV_CANARY_FRAME,PSV_AO_STRANGER,PSV_PMA];
  names:=[8]string.["engram v1","engram v2","canary engram","telemetry","control","canary frame","stranger's frame","PMA 30179"];
  snapshot:[16384]u8;at:=0;audit:[2048]u8;logged:=0;scratch:[8192]u8;frames_stored:=0;
  for i:0..<8 {
    r:=ps_route(events[i],pairs[..],RELAY_NOW,scratch[..],snapshot[at..<16384]);
    at+=r.written;logged+=ps_log(r,audit[logged..<2048]);
    if r.kind==AO_KIND_FRAME && r.written>0 {frames_stored+=1;}
    print("%: % (%)\n",names[i],ROUTE_WORDS[r.route],AO_WORDS[r.error]);
  }
  print("snapshot: % bytes, % frames of kind 24200\n",at,frames_stored);
  print("audit log:\n%",cast(string)audit[0..<logged]);
  
```

And the canary:

<!-- source: nostr/examples/private-agent-state.coax canary -->
```jai
  aw:[1]AoWork;fs:[4096]u8;fa:=[1]AoText.[AoText.{buf=fs[..]}];osk:=key_of(PSV_OWNER_SECRET);
  mem:=open_as(canary,PSV_OWNER_SECRET,true,w[..],s4[..]);
  frame:=ao_open(PSV_CANARY_FRAME,osk[..],PSV_OWNER_X,PSV_AGENT_X,aw[..],fa[..]);
  print("owner decrypts the engram: %, canary %\n",AE_WORDS[mem.error],ps_contains(mem.value,PSV_CANARY));
  print("owner decrypts the frame: % seq %, canary %\n",frame.name,frame.seq,ps_contains(frame.payload,PSV_CANARY));
  print("canary in the snapshot: %\n",ps_contains(cast(string)snapshot[0..<at],PSV_CANARY));
  print("canary in the audit log: %\n",ps_contains(cast(string)audit[0..<logged],PSV_CANARY));
  print("canary frame's id in the snapshot or log: %\n",
    ps_contains(cast(string)snapshot[0..<at],PSV_CANARY_FRAME_ID) || ps_contains(cast(string)audit[0..<logged],PSV_CANARY_FRAME_ID));
  
```

```shell
coax/bin/coax run nostr/examples/private-agent-state.coax --run-steps 250000000
```

The transcript, [private-agent-state.out](../fixtures/private-agent-state.out):

```text
-- the agent writes mem/example twice
write mem/example at 1700000000: event f4a594177b7aeea4…
write mem/example at 1700000002: event c8604bef05295856…
same second again: created_at 1700000003
-- the owner reads
owner, version 1: mem/example at 1700000000: "hello, agent memory"
owner, version 2: mem/example at 1700000002: tombstone (absent)
d tag: 72d4f96291064515…
head: version 2 (created_at 1700000002)
replayed version 1: stale
a head 601 s ahead of the clock: conflict
-- refused
stranger reads version 1: refused: decrypt
stranger's engram: refused: author
engram for another owner: refused: tags
d from another slug: refused: address
repeated member name: refused: json
value 7: refused: shape
slug mem/Example: refused: slug
-- the relay routes each event
write mem/canary at 1700000004: event edacb6c761c8361c…
engram v1: stored (fan out)
engram v2: stored (fan out)
canary engram: stored (fan out)
telemetry: memory only (fan out)
control: memory only (fan out)
canary frame: memory only (fan out)
stranger's frame: refused (AUTH required)
PMA 30179: refused (reserved)
snapshot: 2105 bytes, 0 frames of kind 24200
audit log:
stored kind 30174 id f4a594177b7aeea4fe99a09efbf74ae85f0126244f322135682c405888a38689 bytes 687
stored kind 30174 id c8604bef05295856a67a88ec895e07b5b47a2febc23c82934734096a7b123b63 bytes 687
stored kind 30174 id edacb6c761c8361cc35bf0229c1f4c3052ef11a7eda4e509340eb75417a48415 bytes 731
refused 8
-- the canary
owner decrypts the engram: ok, canary true
owner decrypts the frame: acp_read seq 43, canary true
canary in the snapshot: false
canary in the audit log: false
canary frame's id in the snapshot or log: false
```

The first two writes reproduce NIP-AE's Event 1 and Event 3 (`f4a59417…`
and `c8604bef…`). The refusals are the deliberately broken inputs: a
stranger reading version 1 (`decrypt`), an engram signed by the stranger
(`author`), the agent's engram for another owner (`tags`), a body whose
`d` comes from `mem/other` (`address`), a body with `value` twice
(`json`), `value` 7 (`shape`) and the slug `mem/Example` (`slug`). The
relay stores three engrams (687 + 687 + 731 = 2,105 bytes, each a 20-byte
frame header and the signed event), keeps no frame of kind 24200, and
logs three stored engrams and the refused reservation (code 8). The
canary is in both decrypted bodies and in neither the snapshot nor the
log.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/private-agent-state.coax](../examples/checks/private-agent-state.coax),
which loads the example:

- NIP-AE's published reference vectors, copied from the NIP and not from
  code: K_c, the three `d` tags, Event 1 and Event 3's ids and
  signatures, and Event 1's content. The example's two writes must contain them
  byte for byte. Before it writes any vector,
  [private_state_vectors.py](../../coax/tests/nostr/private_state_vectors.py)
  reproduces all four NIP-AE events (content, `sha256(content)`, id and
  signature) with
  [nip59_reference.py](../../coax/tests/nostr/nip59_reference.py)'s
  integer secp256k1, ChaCha20 and BIP-340 and `hashlib`'s HMAC, and stops
  if any differs. NIP-AE's vectors are correct: all four match.
- The canary engram's id as the Python reference signs it.
- The rollback by hand.
- NIP-AO's examples, as printed. The wire events are illustrative: their
  id is `a1b2c3d4...`, their keys are `agent_pubkey_hex` and
  `owner_pubkey_hex`, and their content is `<NIP-44 v2 ciphertext>`, so
  NIP-AO publishes no id or signature to compare with, and a wire event
  read as printed refuses at the event reader. Its decrypted payloads are
  well formed, and the client reads them.

<!-- source: nostr/examples/checks/private-agent-state.coax check -->
```jai
// Copied from NIP-AE's "Reference test vectors" (nostr/nips/block/NIP-AE.md),
// not from any code: K_c, the three d tags, and events 1 and 3's ids and
// signatures. coax/tests/nostr/private_state_vectors.py reproduces every one
// of them with nip59_reference.py's integer secp256k1, ChaCha20 and BIP-340
// and hashlib's HMAC before it writes a vector.
NIP_KC :: "c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d";
NIP_D_CORE :: "bdc233238ffe52e272b44cc233c8f33a2bc510b08be04495b225964283be4a90";
NIP_D_EXAMPLE :: "72d4f9629106451505d7d341ea85bb3ebad4f654fcfd2aad100d5a35f8a85cba";
NIP_D_NOTES :: "31651571a312780cfdc1f0b706b682ac9f3f51a053e8dca76fe57710bae5a4d4";
NIP_E1_ID :: "f4a594177b7aeea4fe99a09efbf74ae85f0126244f322135682c405888a38689";
NIP_E1_SIG :: "0a4582f0bc5995b9a010afda5984f568055988ebbe4552b4e0ec6d11aeb2b303af940f3d84726a7edd1763badb284eb3aa8457664ceba85a90d6252ed4b494cb";
NIP_E3_ID :: "c8604bef05295856a67a88ec895e07b5b47a2febc23c82934734096a7b123b63";
NIP_E3_SIG :: "c8d53859cf08b3a9a20a5b01c61d12fa2f082f462adb635420f05dc6f9bb662a174e729023854bf53e5e35fae8f6f4c9d604e8979a070e298cd77cfb7e6b6468";
NIP_E1_CONTENT :: "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABedgcxyfmpph68LBjCWZsTI5lb0Cbg8dIPVYVe/WVj/l4Yd8HGgzC8awyBi9bn9ClRdtd2IPsmont0jN/cajVSQhahTOwuNNwoJtZIg35aSsUzeCq4tQfd8E+fLoKomdPxjs=";
// The canary engram's id as the Python reference signs it.
PY_CANARY_ID :: "edacb6c761c8361cc35bf0229c1f4c3052ef11a7eda4e509340eb75417a48415";

#test "the example's two versions are NIP-AE's events 1 and 3" #steps 2000000000 {
  w:[1]AeWork;none:AeRecord;b1:[4096]u8;b2:[4096]u8;st:[4096]u8;cb:[4096]u8;
  e1:=seal_at("mem/example",PSV_BODY1,1700000000,none,1,w[..],b1[..]).text;
  v1:=open_as(e1,PSV_AGENT_SECRET,false,w[..],st[..]);
  e2:=seal_at("mem/example",PSV_BODY3,1700000002,v1,3,w[..],b2[..]).text;
  #assert ps_contains(e1,NIP_E1_ID) && ps_contains(e1,NIP_E1_SIG) && ps_contains(e1,NIP_E1_CONTENT);
  #assert ps_contains(e2,NIP_E3_ID) && ps_contains(e2,NIP_E3_SIG) && ps_contains(e2,"\"created_at\":1700000002");
  #assert v1.d==NIP_D_EXAMPLE && PSV_KC==NIP_KC && PSV_D_CORE==NIP_D_CORE && PSV_D_NOTES==NIP_D_NOTES;
  c:=seal_at("mem/canary",PSV_CANARY_BODY,1700000004,none,5,w[..],cb[..]).text;#assert ps_contains(c,PY_CANARY_ID) && c==PSV_CANARY_EVENT;
  // The rollback by hand: version 1 (1700000000) is not newer than version 2 (1700000002).
  #assert ae_accept(open_as(e2,PSV_OWNER_SECRET,true,w[..],st[..]),open_as(e1,PSV_OWNER_SECRET,true,w[..],cb[..]))==AE_STALE;
}

// NIP-AO's examples are illustrative: the wire events carry "a1b2c3d4..."
// for the id, "agent_pubkey_hex" for keys and "<NIP-44 v2 ciphertext>" for
// content, so read as published they refuse before any other check. Their
// decrypted payloads are well formed: they are PSV_TELEMETRY_BODY and
// PSV_CONTROL_BODY, which the Python reference sealed.
NIP_AO_WIRE :: "{\"id\":\"a1b2c3d4...\",\"kind\":24200,\"pubkey\":\"agent_pubkey_hex\",\"created_at\":1777464041,\"content\":\"<NIP-44 v2 ciphertext>\",\"tags\":[[\"p\",\"owner_pubkey_hex\"],[\"agent\",\"agent_pubkey_hex\"],[\"frame\",\"telemetry\"]],\"sig\":\"...\"}";
#test "NIP-AO's examples" #steps 2000000000 {
  s:[8192]u8;p:=[1]AoPair.[AoPair.{agent=PSV_AGENT_X,owner=PSV_OWNER_X}];
  #assert ao_admit(NIP_AO_WIRE,p[..],1777464041,s[..]).error==AO_EVENT;
  w:[1]AoWork;st:[4096]u8;a:=[1]AoText.[AoText.{buf=st[..]}];t:AoOpened;t.frame=AO_TELEMETRY;k:AoOpened;k.frame=AO_CONTROL;
  r:=ao_read(PSV_TELEMETRY_BODY,t,w[..],a[..]);#assert r.error==AO_FANOUT && r.name=="acp_write" && r.seq==42;
  r=ao_read(PSV_CONTROL_BODY,k,w[..],a[..]);#assert r.error==AO_FANOUT && r.name=="cancel_turn";
}
```

```shell
coax/bin/coax test nostr/examples/checks/private-agent-state.coax --run-steps 250000000
```

`coax test` runs 469 checks, including the loaded files' own checks; all pass.

### The library's own checks

`nipae.coax` seals NIP-AE's four events byte for byte, opens them as the
agent and as the owner, and refuses each broken engram at its rule: a
stranger's key, a stranger's signature, another owner, a repeated member
name at the top and nested, a `d` from another slug, a mistyped value, an
upper-case slug, a `core` without `profile`, another kind, a changed
signature and a changed `created_at`. It checks the slug grammar at its
boundaries (64 characters a segment, 255 bytes in all), every tag count,
every body field's type, escaped keys, unknown fields at any depth, every
arena boundary, head selection and its tie rule, the rollback refusal,
and `ae_write_at` at 600 and 601 seconds ahead. It checks that the
work is all zero after every open and seal. `nipao.coax` checks each tag
count, each authorization case (a stranger as agent or owner, the wrong
`p`, an agent tag that is not the signer, an empty or partial ownership
table), the freshness window at ±300 and ±301 seconds, that no frame,
admitted or refused, changes one byte of the snapshot, that the audit
line never names a frame, and every client refusal and payload field.

```shell
coax/bin/coax test coax/lib/nostr/nipae.coax --library --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/nipae.coax --root-only --library --run-steps 250000000
coax/bin/coax test coax/lib/nostr/nipao.coax --library --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/nipao.coax --root-only --library --run-steps 250000000
```

On 2026-11-01, each file's checks passed (`coax test` runs the loaded files' checks too: 429 for `nipae.coax`, 353 for `nipao.coax`), and root-only `coax mutate`, run in four shards each, killed every mutant: 389 of 389 in `nipae.coax` and 318 of 318 in `nipao.coax`. None survived and none is marked equivalent. Earlier runs left survivors where a decision could not be observed: an overflow branch whose result equals an empty value's (tag values are now compared as JSON strings, not unescaped into a fixed buffer), initial values that every path overwrites, a type code no test could tell from another, a JSON policy whose larger limit only a body over 65,536 bytes shows, key-length checks that the conversation key repeats, and an author loop whose first byte was never the only difference. The code was simplified so each decision is made once, and tests were added for the rest. The test builders live in `coax/tests/nostr/nipae_fixtures.coax` and `nipao_fixtures.coax`, so root-only mutation mutates the library code. The example matches its transcript in debug and release. The compiler was the cached release build `~/.cache/coax-release/bcefdc7f…`, not rebuilt in the worktree. The full mutation run (`COAX_MUTATION_FULL=1`) and the whole-book example run were not run.

## Summary

- An engram's `d` tag is an HMAC of its slug under K_c: the relay holds
  an address, not a name.
- Only the agent and its owner decrypt an engram, and only the agent's
  signature makes one count; `ae_open` checks the signature before it
  decrypts.
- Head selection picks the newest record, ties to the lowest id, and the
  same rule refuses a rollback. Writes go at `max(now, T + 1)`, and a
  head too far ahead is a conflict.
- A relay admits an observer frame only in the right direction between a
  pair in its own ownership records, within five minutes, and fans it out
  in memory. `ps_route` has no path from a frame to the snapshot, and
  `ps_log` never names one.
- A canary in plaintext reaches the owner and neither the snapshot nor
  the audit log.

## Dependencies

- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [87. Owner attestation and agent
  authentication](ch087-owner-attestation-and-agent-authentication.md)

## Sources

- [Block NIP-AE](../nips/block/NIP-AE.md)
- [Block NIP-AP](../nips/block/NIP-AP.md)
- [Block NIP-AO](../nips/block/NIP-AO.md)
- [Block NIP-AM](../nips/block/NIP-AM.md)
- [Block NIP-PMA](../nips/block/NIP-PMA.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
