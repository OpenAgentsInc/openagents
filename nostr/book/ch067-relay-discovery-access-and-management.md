# 67. Relay discovery, access and management

Relays describe themselves, control who may join, and accept management calls.
This chapter builds the decisions behind all three: the NIP-86 management
endpoint that a relay's operators call over HTTP, the NIP-66 records that
monitors publish about relays, and the NIP-43 member lists that a relay
signs with its own key.

Part X, Every NIP. Result: the NIP-86 JSON-RPC management API over HTTP with
NIP-98 auth, applied to a ban and allow policy that relay admission
consults; NIP-66 relay discovery (30166) and monitor announcements (10166),
with a median round-trip time over trusted monitors; and NIP-43 membership
lists (13534), add and remove events (8000, 8001) and join and leave
requests (28934, 28936).

## What this chapter teaches

- NIP-86: a `POST` with `Content-Type: application/nostr+json+rpc` and the
  body `{"method": ..., "params": [...]}`. The `Authorization` header is a
  NIP-98 event (chapter 49) whose `payload` tag is required, and the
  signer must be one of the relay's admin keys.
- The order of the relay's refusals, and why the body is read last.
- The methods that change the policy (`banpubkey`, `allowpubkey`,
  `banevent`, `allowkind`, and others) and the replies they write.
- NIP-66: kind 30166 with `d` the relay URL and tags `rtt-open`,
  `rtt-read`, `rtt-write`, `n`, `N` and `R`; kind 10166 with `frequency`,
  `timeout` and `c`. A median over several monitors, because one monitor
  can be wrong.
- NIP-43: a kind 13534 member list, and 8000 and 8001 events, count only
  when the key in the relay's NIP-11 `self` field signed them. Join
  requests (28934) carry a `claim`; join and leave requests (28936) must be
  recent and carry NIP-70's `-` tag.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip86.coax](../../coax/lib/nostr/nip86.coax) | `n86_serve(p, http_method, content_type, authorization, url, body, now, out)` | The whole management endpoint for one request (`#authority`): HTTP status, reply body, and the policy changed only on success |
| | `n86_request`, `n86_dispatch` | Read the body as one request; apply it to the policy and write the reply |
| | `n86_admit(p, pubkey, id, kind)` | What relay admission asks of the policy for one event |
| [nip66.coax](../../coax/lib/nostr/nip66.coax) | `n66_read_relay`, `n66_read_monitor`, `n66_write_relay`, `n66_write_monitor` | Read and write kind 30166 and 10166 tags |
| | `n66_median(events, trusted, url, which)` | The median round-trip time for one relay over the newest event of each trusted monitor |
| [nip43.coax](../../coax/lib/nostr/nip43.coax) | `n43_self`, `n43_members`, `n43_change`, `n43_request` | The relay's key from its NIP-11 document; member lists and add and remove events signed by that key; join and leave requests (`#authority`) |
| [nip98.coax](../../coax/web/api/nip98.coax) | `nip98_check_coax` | Chapter 49's NIP-98 check, which `n86_serve` calls unchanged |

The helpers and data for each file's checks are in
`coax/tests/nostr/nip86_fixtures.coax`, `nip66_fixtures.coax` and
`nip43_fixtures.coax`, so that mutation changes the library and not the
checks. `coax/relay/relay.coax` serves NIP-86 with `--admin` (see
[The relay's endpoint](#the-relays-endpoint)); NIP-66 and NIP-43 are not
wired into it.

## The management endpoint (NIP-86)

NIP-86 puts a management API on the relay's own URL. An operator's tool
sends an HTTP request instead of a WebSocket upgrade. The relay reads the
body as one method call, applies it, and answers with a JSON object that
holds a `result` and, on failure, an `error`.

### The policy

The state is small and fixed in size. Each list holds at most eight
entries, so a full list is a refusal and never an overwrite.

<!-- source: coax/lib/nostr/nip86.coax policy -->
```jai
N86Text :: struct {b:[N86_MAX_TEXT]u8; n:int;}
N86Entry :: struct {key:[32]u8; reason:N86Text;}
N86List :: struct {e:[N86_MAX_LIST]N86Entry; n:int;}
N86Kinds :: struct {k:[N86_MAX_LIST]int; n:int;}
// The relay's management state: who may call, the lists admission
// consults, the relay's name, and the NIP-98 event IDs already used.
N86Policy :: struct {
  admins:[N86_MAX_ADMINS][32]u8; admins_n:int;
  banned:N86List; allowed:N86List; banned_events:N86List;
  allowed_kinds:N86Kinds; disallowed_kinds:N86Kinds;
  name:N86Text;
  seen:[N86_SEEN][64]u8; seen_n:int;
}
```

The methods built here, and the shape of the params each takes:

<!-- source: coax/lib/nostr/nip86.coax methods -->
```jai
N86_METHODS :: [15]string.["supportedmethods","banpubkey","unbanpubkey","listbannedpubkeys",
  "allowpubkey","unallowpubkey","listallowedpubkeys","banevent","unbanevent","listbannedevents",
  "allowkind","disallowkind","listallowedkinds","listdisallowedkinds","changerelayname"];
// The params each method takes: 0 none, 1 a 64-digit hex key or id and an
// optional reason, 2 one kind number from 0 to 65535, 3 one string.
N86_SHAPES :: [15]int.[0,1,1,0,1,1,0,1,1,0,2,2,0,0,3];
```

NIP-86 also lists roles, claims, IP blocks, event allow lists and a
moderation queue. Those are not built, and a call to them refuses as an
unknown method.

### The order of the checks

`n86_serve` decides in this order. The first failure answers.

1. The `Content-Type` is `application/nostr+json+rpc`, with ASCII letters
   compared without case. A type with parameters such as `; charset=utf-8`
   refuses: 415.
2. The HTTP method is `POST`: 405.
3. `nip98_check_coax` accepts the `Authorization` value for this method,
   URL, body and clock: base64, the event, its ID and signature, kind
   27235, `created_at` within 60 seconds, the `u` tag equal to the relay
   URL byte for byte, the `method` tag, and the `payload` tag when present.
   Any failure is 401 and names the check.
4. The event has a `payload` tag. NIP-98 leaves it optional, and without
   it the body is not bound: anyone who saw the header could send another
   body. NIP-86 requires it: 401.
5. The signer is one of the relay's admin keys: 403.
6. The event ID has not been used: 401. When the list of used IDs is full,
   the answer is 503, and no ID is forgotten to make room.

Only then is the body read. A body that is not one request object is 400,
as are an unknown method and params of the wrong shape. The event ID is
used up even when the body then refuses, because the admin did sign that
request.

<!-- source: coax/lib/nostr/nip86.coax serve -->
```jai
// The whole management endpoint for one HTTP request.
n86_serve :: (p:[]N86Policy,http_method:string,content_type:string,authorization:string,
              url:string,body:[]u8,now:int,out:[]u8)->N86Reply {
  if !n86_same_fold(content_type,N86_TYPE) {return n86_fail(N86_CONTENT_TYPE,out);}
  if http_method!="POST" {return n86_fail(N86_HTTP_METHOD,out);}
  work:[N98_WORK]u8;a:[N98_OUT]u8;
  v:=nip98_check_coax(authorization,"POST",url,body,now,work[..],a[..]);
  if v.error!=N98_OK {return n86_unauthorized(v.error,out);}
  if v.payload==0 {return n86_fail(N86_NO_PAYLOAD,out);}
  // a holds the event id and the signer's pubkey as lowercase hex.
  id:=cast(string)a[0..<64];signer:=cast(string)a[64..<128];
  admin:=false;hex:[64]u8;
  for i:0..<p[0].admins_n {if n86_hex(p[0].admins[i],hex[..])==signer {admin=true;}}
  if !admin {return n86_fail(N86_NOT_ADMIN,out);}
  for i:0..<p[0].seen_n {if cast(string)p[0].seen[i][..]==id {return n86_fail(N86_REPLAYED,out);}}
  if p[0].seen_n==N86_SEEN {return n86_fail(N86_SEEN_FULL,out);}
  for i:0..<64 {p[0].seen[p[0].seen_n][i]=a[i];}
  p[0].seen_n+=1;
  return n86_dispatch(p,n86_request(cast(string)body),out);
}
```

### Reading the request

The reader takes one JSON object with a `method` string and a `params`
array, each once. Other keys, such as a JSON-RPC `id`, are skipped. Each
param is a string of at most 64 bytes or an integer.

<!-- source: coax/lib/nostr/nip86.coax request -->
```jai
// The body as one request: the method's index and its params, checked
// against the method's shape. Other keys are skipped.
n86_request :: (body:string)->N86Request {
  r:N86Request;j:[1]Json;json_start(j[..],body);has_method:=false;has_params:=false;
  if json_next(j[..])!=JSON_OBJECT {return r;}
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);raw:=json_text(j[..]);
    if json_is(key,"method") {
      if has_method || v!=JSON_STRING {return r;}
      has_method=true;
      for k:0..<N86_METHODS.count {if json_is(raw,N86_METHODS[k]) {r.method=k;}}
    } else if json_is(key,"params") {
      if has_params || v!=JSON_ARRAY {return r;}
      has_params=true;u:=json_next(j[..]);
      while u!=JSON_ARRAY_END {
        if u==JSON_ERROR {return r;}
        if r.n==N86_MOST_PARAMS {return n86_rfail(r,N86_PARAMS);}
        if u==JSON_STRING {
          m:=json_unescape(json_text(j[..]),r.p[r.n].text.b[..]);
          if m<0 {return n86_rfail(r,N86_PARAMS);}
          r.p[r.n].text.n=m;r.p[r.n].is_text=true;
        } else if u==JSON_NUMBER {
          x:=json_int(json_text(j[..]));
          if !x.ok {return n86_rfail(r,N86_PARAMS);}
          r.p[r.n].number=x.value;
        } else {return n86_rfail(r,N86_PARAMS);}
        r.n+=1;u=json_next(j[..]);
      }
    } else if !json_skip(j[..]) {return r;}
    t=json_next(j[..]);
  }
  if t!=JSON_OBJECT_END || json_next(j[..])!=JSON_END || !has_method || !has_params {return r;}
  if r.method<0 {return n86_rfail(r,N86_UNKNOWN);}
  if !n86_shape_ok(r) {return n86_rfail(r,N86_PARAMS);}
  return n86_rfail(r,N86_OK);
}
```

### Applying it

`n86_dispatch` changes a copy of the policy and stores it only when the
call succeeds, so a refused call changes nothing. Banning a key takes it
off the allow list, and allowing takes it off the ban list, as NIP-86
says. `unbanpubkey` does not allow, and `unallowpubkey` does not ban. For
kinds, `allowkind` and `disallowkind` move a kind between the two lists in
the same way; NIP-86 doesn't say, so that is this relay's choice.

<!-- source: coax/lib/nostr/nip86.coax dispatch -->
```jai
// Apply one read request to the policy and write its reply. A refused
// request changes nothing.
n86_dispatch :: (p:[]N86Policy,r:N86Request,out:[]u8)->N86Reply {
  if r.error!=N86_OK {return n86_fail(r.error,out);}
  m:=r.method;key:[32]u8;
  if N86_SHAPES[m]==1 {decoded:=n86_key(r.p[0].text,key[..]);}
  reason:N86Text;if r.n==2 {reason=r.p[1].text;}
  kind:=r.p[0].number;
  n:=json_raw(out,0,"{\"result\":");
  if m==0 {
    n=json_raw(out,n,"[");
    for k:1..<N86_METHODS.count {if k>1 {n=json_raw(out,n,",");}n=json_write_string(out,n,N86_METHODS[k]);}
    n=json_raw(out,n,"]");
  }
  else if m==3 {n=n86_write_list(out,n,p[0].banned,"pubkey");}
  else if m==6 {n=n86_write_list(out,n,p[0].allowed,"pubkey");}
  else if m==9 {n=n86_write_list(out,n,p[0].banned_events,"id");}
  else if m==12 {n=n86_write_kinds(out,n,p[0].allowed_kinds);}
  else if m==13 {n=n86_write_kinds(out,n,p[0].disallowed_kinds);}
  else {
    full:=false;q:=p[0];
    if m==1 {full=n86_full(q.banned,key);q.banned=n86_add(q.banned,key,reason);q.allowed=n86_remove(q.allowed,key);}
    else if m==2 {q.banned=n86_remove(q.banned,key);}
    else if m==4 {full=n86_full(q.allowed,key);q.allowed=n86_add(q.allowed,key,reason);q.banned=n86_remove(q.banned,key);}
    else if m==5 {q.allowed=n86_remove(q.allowed,key);}
    else if m==7 {full=n86_full(q.banned_events,key);q.banned_events=n86_add(q.banned_events,key,reason);}
    else if m==8 {q.banned_events=n86_remove(q.banned_events,key);}
    else if m==10 {full=n86_kind_full(q.allowed_kinds,kind);q.allowed_kinds=n86_kind_add(q.allowed_kinds,kind);q.disallowed_kinds=n86_kind_remove(q.disallowed_kinds,kind);}
    else if m==11 {full=n86_kind_full(q.disallowed_kinds,kind);q.disallowed_kinds=n86_kind_add(q.disallowed_kinds,kind);q.allowed_kinds=n86_kind_remove(q.allowed_kinds,kind);}
    else {q.name=r.p[0].text;}
    // A full list refuses before q is stored, so nothing changes.
    if full {return n86_fail(N86_LIST_FULL,out);}
    p[0]=q;n=json_raw(out,n,"true");
  }
  return N86Reply.{http=200,code=N86_OK,n=json_raw(out,n,"}")};
}
```

### What admission asks

The relay's `EVENT` path asks the policy one question per event. A banned
key refuses first. A non-empty allow list admits only its keys, and a
non-empty allowed-kinds list admits only its kinds.

<!-- source: coax/lib/nostr/nip86.coax admit -->
```jai
// What relay admission asks of the policy for one event: N86_ADMIT_OK or
// the first rule that refuses it. A non-empty allow list admits only its
// keys; a non-empty allowed-kinds list admits only its kinds.
N86_ADMIT_OK :: 0; N86_ADMIT_BANNED :: 1; N86_ADMIT_NOT_ALLOWED :: 2;
N86_ADMIT_EVENT :: 3; N86_ADMIT_KIND :: 4; N86_ADMIT_KIND_NOT_ALLOWED :: 5;
n86_admit :: (p:N86Policy,pubkey:[32]u8,id:[32]u8,kind:int)->int {
  if n86_find(p.banned,pubkey)>=0 {return N86_ADMIT_BANNED;}
  if p.allowed.n>0 && n86_find(p.allowed,pubkey)<0 {return N86_ADMIT_NOT_ALLOWED;}
  if n86_find(p.banned_events,id)>=0 {return N86_ADMIT_EVENT;}
  if n86_kind_at(p.disallowed_kinds,kind)>=0 {return N86_ADMIT_KIND;}
  if p.allowed_kinds.n>0 && n86_kind_at(p.allowed_kinds,kind)<0 {return N86_ADMIT_KIND_NOT_ALLOWED;}
  return N86_ADMIT_OK;
}
```

## Worked example: one banpubkey call by hand

The admin bans Mallory's key with the reason `spam`. The admin's key is
BIP-340 test vector 1's published secret key, as in chapter 49, so the
pubkey is `dff1d77f...502ba659`. Mallory's key is the secret key 3, with
pubkey `f9308a01...bce036f9`. Never sign with a real key in an example. The
relay URL is `wss://relay.example.com/`, 24 bytes: `wss://` (6),
`relay.example.com` (17) and `/` (1).

### Step 1: the body

```jai
{"method":"banpubkey","params":["f9308a01...bce036f9","spam"]}
```

| Part | Bytes |
| --- | ---: |
| `{"method":"banpubkey",` | 11 + 9 + 2 = 22 |
| `"params":["` | 11 |
| Mallory's key | 64 |
| `","spam"]}` | 10 |
| Total | 107 |

Its SHA-256 is `d731279e...8880bb94`, the value of the `payload` tag.

### Step 2: the tags and the ID

| Part | Bytes |
| --- | ---: |
| `[["u","` + URL + `"],` | 7 + 24 + 3 = 34 |
| `["method","POST"],` | 18 |
| `["payload","` + hash + `"]` | 12 + 64 + 2 = 78 |
| `]` | 1 |
| Total | 131 |

The NIP-01 preimage `[0,"<pubkey>",1790000000,27235,<tags>,""]` is
4 + 64 + 2 + 10 + 1 + 5 + 1 + 131 + 4 = 222 bytes. Its SHA-256 is the ID,
`343f4fd7...ce8fba24`.

### Step 3: the event and the header

The event is chapter 49's table with these tags: 73 + 76 + 24 + 13 +
(7 + 131 + 1) + 13 + 137 = 475 bytes. `475 = 3·158 + 1`, so base64 writes
158 full groups and one group of 1 byte with two `=`: `159·4 = 636`
characters. With `Nostr ` (6), the header value is 642 bytes.

### Step 4: what the relay decides

The relay has one admin key, `dff1d77f...`, and its clock reads
`now = 1790000030`.

1. The `Content-Type` and the method match.
2. NIP-98: `1789999970 ≤ 1790000000 ≤ 1790000090` holds; `u`, `method`
   and `payload` match; the signature verifies.
3. The `payload` tag is present, and the signer is the admin.
4. The ID `343f4fd7...` is new, so it is recorded.
5. The body reads as `banpubkey` with a 64-digit key and a reason. The ban
   list gets `{f9308a01..., "spam"}`, and the key is removed from the allow
   list, where it was not.

The answer is 200 with `{"result":true}`. Sent again, the same header
passes every NIP-98 check, but its ID is recorded, so the answer is 401.
After the ban, `n86_admit` refuses any event by Mallory's key.

## Relay discovery (NIP-66)

A monitor connects to relays, measures them, and publishes one addressable
kind 30166 event per relay. The `d` tag is the relay's normalized URL. The
record keeps the tags this chapter reads:

<!-- source: coax/lib/nostr/nip66.coax relay -->
```jai
N66Relay :: struct {
  status:int=N66_EVENT; url:N32Text; rtt:[3]int; network:N32Text;
  nips:[N66_MAX_LIST]int; nips_n:int; reqs:[N66_MAX_LIST]N32Text; reqs_n:int;
}
N66Timeout :: struct {check:N32Text; ms:int;}
N66Monitor :: struct {
  status:int=N66_EVENT; frequency:int=-1;
  timeouts:[N66_MAX_LIST]N66Timeout; timeouts_n:int; checks:[N66_MAX_LIST]N32Text; checks_n:int;
}
```

Each round-trip time is 1 to 7 decimal digits with no leading zero, in
milliseconds. A repeated `d`, `rtt-*` or `frequency` tag refuses, as does a
number in another form. The `t`, `k`, `g`, `l` and `T` tags are not read.

<!-- source: coax/lib/nostr/nip66.coax read -->
```jai
// A verified kind 30166 event as one relay's record. Absent round-trip
// times are -1.
n66_read_relay :: (e:NostrEvent)->N66Relay {
  r:N66Relay;r.rtt[0]=-1;r.rtt[1]=-1;r.rtt[2]=-1;
  if e.error!=NE_OK || !e.verified {return r;}
  if e.kind!=N66_DISCOVERY {return n66_refuse(r,N66_KIND);}
  has_d:=false;k:=0;
  while true {
    t:=n32_tag(e,k);k+=1;
    if t.error==NT_INDEX {break;}
    if t.error!=NT_OK {return n66_refuse(r,n66_tag_status(t));}
    if t.values<2 {continue;}
    if n32_is(t.v[0],"d") {
      if has_d {return n66_refuse(r,N66_REPEAT);}
      has_d=true;r.url=t.v[1];
    } else if n32_is(t.v[0],"n") {r.network=t.v[1];}
    else if n32_is(t.v[0],"N") {
      if r.nips_n==N66_MAX_LIST {return n66_refuse(r,N66_LIMIT);}
      v:=n66_number(t.v[1]);if v<0 {return n66_refuse(r,N66_NUMBER);}
      r.nips[r.nips_n]=v;r.nips_n+=1;
    } else if n32_is(t.v[0],"R") {
      if r.reqs_n==N66_MAX_LIST {return n66_refuse(r,N66_LIMIT);}
      r.reqs[r.reqs_n]=t.v[1];r.reqs_n+=1;
    } else {
      for w:0..<3 {
        if n32_is(t.v[0],N66_RTT[w]) {
          if r.rtt[w]>=0 {return n66_refuse(r,N66_REPEAT);}
          r.rtt[w]=n66_number(t.v[1]);if r.rtt[w]<0 {return n66_refuse(r,N66_NUMBER);}
        }
      }
    }
  }
  if !has_d || r.url.n==0 {return n66_refuse(r,N66_NO_D);}
  return n66_refuse(r,N66_OK);
}
```

A monitor writes the same record back as tags. The writer puts `d` first,
then `n`, each `N`, each `R`, and the round-trip times that are present:

<!-- source: coax/lib/nostr/nip66.coax write -->
```jai
// A relay's record as kind 30166 tags: d, n, each N, each R, then the
// round-trip times that are present. The length, or -1 when out is small.
n66_write_relay :: (r:N66Relay,out:[]u8)->int {
  c:=r;s:[NT_MAX_VALUE]u8;d:[24]u8;
  n:=json_raw(out,0,"[");n=n66_pair(out,n,true,"d",n66_str(c.url,s[..]));
  if c.network.n>0 {n=n66_pair(out,n,false,"n",n66_str(c.network,s[..]));}
  for i:0..<c.nips_n {n=n66_pair(out,n,false,"N",n66_digits(c.nips[i],d[..]));}
  for i:0..<c.reqs_n {n=n66_pair(out,n,false,"R",n66_str(c.reqs[i],s[..]));}
  for w:0..<3 {if c.rtt[w]>=0 {n=n66_pair(out,n,false,N66_RTT[w],n66_digits(c.rtt[w],d[..]));}}
  return json_raw(out,n,"]");
}
```

A monitor announces itself with kind 10166: `frequency` in seconds (the
reader requires it), `timeout` tags and `c` tags for the checks it runs.
NIP-66's prose puts a timeout's milliseconds at index 1 and the check at
index 2, but its example puts the check first. The reader takes whichever
value is a number as the milliseconds, and the writer follows the example.

### Trusting more than one monitor

NIP-66 warns that a monitor can publish wrong values, by mistake or on
purpose, and that clients should not trust one source. `n66_median` takes
30166 events from many monitors and the keys the caller trusts. It skips
events that don't read, are signed by other keys, describe another relay
or lack the time asked for. It keeps each monitor's newest event, and
returns the median of their times.

<!-- source: coax/lib/nostr/nip66.coax median -->
```jai
// The median round-trip time (which: N66_OPEN, N66_READ or N66_WRITE) for
// the relay at url, over the newest event of each trusted monitor that
// reports it. Events that do not read, are signed by other keys, describe
// another relay or lack that time are skipped. With an even count the
// median is the mean of the middle two, rounded down.
n66_median :: (events:[]NostrEvent,trusted:[][32]u8,url:string,which:int)->N66Median {
  m:N66Median;keys:[N66_MAX_MONITORS][32]u8;at:[N66_MAX_MONITORS]int;rtt:[N66_MAX_MONITORS]int;
  for e:events {
    r:=n66_read_relay(e);
    if r.status!=N66_OK || !n32_trusted(trusted,e.public_key) || !n32_is(r.url,url) || r.rtt[which]<0 {continue;}
    i:=0;while i<m.monitors && !ns_equal32(keys[i],e.public_key) {i+=1;}
    if i==m.monitors {
      if i==N66_MAX_MONITORS {m.status=N66_LIMIT;return m;}
      m.monitors+=1;keys[i]=e.public_key;at[i]=e.created_at;rtt[i]=r.rtt[which];
    } else if e.created_at>at[i] {at[i]=e.created_at;rtt[i]=r.rtt[which];}
  }
  if m.monitors==0 {return m;}
  v:=rtt[0..<m.monitors];h:=m.monitors/2;
  m.rtt=ifx m.monitors%2==1 then n66_kth(v,h) else (n66_kth(v,h-1)+n66_kth(v,h))/2;
  m.status=N66_OK;return m;
}
```

By hand, for `wss://relay.example.com/` and `rtt-open`, with trusted
monitors A, B and C:

| Event | Signer | `created_at` | `rtt-open` | Used |
| --- | --- | ---: | ---: | --- |
| 1 | A | 100 | 120 | yes |
| 2 | B | 100 | 300 | yes |
| 3 | C | 100 | 200 | yes |
| 4 | A | 40 | 900 | no: A's event 1 is newer |
| 5 | an untrusted key | 100 | 5 | no |

The three values sorted are 120, 200 and 300, so the median is 200 ms.
Trusting only A and B leaves 120 and 300; with an even count the median is
the mean of the middle two, rounded down: 210 ms. An untrusted monitor that
reports 5 ms moves neither.

## Membership (NIP-43)

A relay that restricts access may publish who its members are. The list is
the relay's word, so it counts only when the relay's own key signed it.
That key is the `self` field of the relay's NIP-11 document (chapter 37):

<!-- source: coax/lib/nostr/nip43.coax self -->
```jai
// The self field of a NIP-11 document: the relay's key, 64 lowercase hex
// digits. False when the document has none or it does not read.
n43_self :: (nip11:string,out:[]u8)->bool {
  j:[1]Json;json_start(j[..],nip11);found:=false;
  if json_next(j[..])!=JSON_OBJECT {return false;}
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);
    if json_is(key,"self") {
      if found || v!=JSON_STRING {return false;}
      buf:[64]u8;n:=json_unescape(json_text(j[..]),buf[..]);
      if n!=64 || !ne_lower_hex(cast(string)buf[0..<64],out) {return false;}
      found=true;
    } else if !json_skip(j[..]) {return false;}
    t=json_next(j[..]);
  }
  return found && t==JSON_OBJECT_END && json_next(j[..])==JSON_END;
}
```

A kind 13534 list must also carry NIP-70's `-` tag, so that no one else
can republish it. Each `member` tag holds a 64-digit hex key and,
optionally, a role.

<!-- source: coax/lib/nostr/nip43.coax members -->
```jai
// The relay's member list, when self signed it: each member tag's key and
// its first role (empty when none).
n43_members :: (self:[32]u8,e:NostrEvent)->N43Members {
  m:N43Members;m.status=n43_relay_event(self,e,N43_MEMBERS_KIND,N43_MEMBERS_KIND);
  if m.status!=N43_OK {return m;}
  k:=0;
  while true {
    t:=n32_tag(e,k);k+=1;
    if t.error==NT_INDEX {break;}
    if t.error!=NT_OK {m.status=n43_status(t);return m;}
    if !n32_is(t.v[0],"member") {continue;}
    if t.values<2 {m.status=N43_SHAPE;return m;}
    if m.n==N43_MAX_MEMBERS {m.status=N43_LIMIT;return m;}
    c:=t.v[1];
    if !ne_lower_hex(cast(string)c.b[0..<c.n],m.keys[m.n][..]) {m.status=N43_HEX;return m;}
    m.roles[m.n]=t.v[2];   // n32_tag leaves a missing value empty
    m.n+=1;
  }
  return m;
}
```

A list signed by any other key refuses as `N43_NOT_SELF`, and a client
ignores it. NIP-43 also says the list is neither exhaustive nor
authoritative: a client should read the member's own kind 10010 list too.
That second read is not built here. Kind 8000 and 8001 events, which
announce one member added or removed, pass the same checks in
`n43_change`.

A user asks to join with kind 28934 and to leave with kind 28936. Both
carry the `-` tag and must be signed within a few minutes of the relay's
clock; this relay allows 300 seconds either way. A join request must carry
a non-empty `claim`, an invite code.

<!-- source: coax/lib/nostr/nip43.coax request -->
```jai
// A user's join (28934, with a claim) or leave (28936) request.
n43_request :: (e:NostrEvent,now:int)->N43Request {
  r:N43Request;
  if e.error!=NE_OK || !e.verified {return r;}
  r.status=N43_KIND;
  if e.kind!=N43_JOIN && e.kind!=N43_LEAVE {return r;}
  r.status=N43_TIME;
  if e.created_at<now-N43_WINDOW || e.created_at>now+N43_WINDOW {return r;}
  r.status=n43_protected(e);
  if r.status!=N43_OK {return r;}
  r.join=e.kind==N43_JOIN;
  if !r.join {return r;}
  k:=0;
  while true {
    t:=n32_tag(e,k);k+=1;
    if t.error==NT_INDEX {r.status=N43_NO_CLAIM;return r;}
    if t.error!=NT_OK {r.status=n43_status(t);return r;}
    if t.values>=2 && n32_is(t.v[0],"claim") && t.v[1].n>0 {r.claim=t.v[1];return r;}
  }
  return r;
}
```

Checking the claim against the relay's invite codes, NIP-86's
`createclaim` and `listclaims`, role definitions (33534) and invites
(28935) are not built.

## The example

The example is
[nostr/examples/relay-management.coax](../examples/relay-management.coax).
It builds the worked example's header, serves it twice, lists the bans and
asks admission about Mallory. It then sends one request per refusal. Two
headers in it, Mallory's and the one without a `payload` tag, are the
Python-signed ones from `nip86_fixtures.coax`. Last, it runs the NIP-66
median and the NIP-43 checks.

The NIP-66 and NIP-43 events are built already verified, with keys of 32
equal bytes, as the library's checks build them. Signing and verifying
them is chapter 21's and 22's work; this chapter's code decides what a
verified event means.

<!-- source: nostr/examples/relay-management.coax example -->
```jai
#load "../../coax/lib/nostr/nip86.coax";
#load "../../coax/lib/nostr/nip66.coax";
#load "../../coax/lib/nostr/nip43.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#load "../../coax/lib/nostr/base64.coax";
#import "Basic";

// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
// Append text to out at n; the new length.
put :: (out: [] u8, at: int, text: string) -> int {
  n := at;
  for c: text { out[n] = c; n += 1; }
  return n;
}

// The client: the body's SHA-256 as hex, into hex (64 bytes).
body_hash :: (body: string, hex: [] u8) {
  h: [32] u8; b: [256] u8;
  for i: 0..<body.count { b[i] = body[i]; }
  #assert sha256_hash_checked(b[0..<body.count], h[..], false) == SHA_OK;
  r := hex_encode_checked(cast(string) h[..], hex, false);
  #assert r.written == 64;
}
// The client: the NIP-98 tags for a POST of body to url, payload included.
auth_tags :: (url: string, body: string, out: [] u8) -> string {
  hex: [64] u8; body_hash(body, hex[..]);
  n := put(out, 0, "[[\"u\",\""); n = put(out, n, url);
  n = put(out, n, "\"],[\"method\",\"POST\"],[\"payload\",\""); n = put(out, n, cast(string) hex[..]);
  n = put(out, n, "\"]]");
  return cast(string) out[0..<n];
}
// The client: sign a kind 27235 event with empty content into ev.
sign :: (key: [] u8, aux: [] u8, created_at: int, tags: string, ev: [] u8) -> string {
  work: [1] NostrSignWork; scratch: [1024] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key, aux, created_at, 27235, tags, "", work[..], scratch[..], ev, false);
  }
  #assert n > 0;
  return cast(string) ev[0..<n];
}
// The client: "Nostr <base64 of event>" into header.
encode_header :: (event: string, header: [] u8) -> string {
  h := put(header, 0, "Nostr ");
  e := base64_encode_checked(event, header[h..<header.count], false);
  #assert e.error == NC_OK;
  return cast(string) header[0..<h + e.written];
}

// The relay: serve one request and print its status and reply.
serve :: (label: string, p: [] N86Policy, content_type: string, header: string, body: string, now: int) {
  b: [256] u8; out: [N86_OUT] u8;
  for i: 0..<body.count { b[i] = body[i]; }
  r := n86_serve(p, "POST", content_type, header, RELAY, b[0..<body.count], now, out[..]);
  print("%: % %\n", label, r.http, cast(string) out[0..<r.n]);
}
ADMIT :: [6] string.["admitted", "banned key", "key not on the allow list", "banned event",
                     "kind disallowed", "kind not on the allowed list"];
N66_WORDS :: [10] string.["ok", "not a verified event", "wrong kind", "no d tag", "not a number",
                          "repeated tag", "a tag does not read", "too many tags", "no frequency", "none"];
N43_WORDS :: [11] string.["ok", "not a verified event", "wrong kind", "not signed by self", "no - tag",
                          "a tag does not read", "too many members", "bad hex", "outside 300 s", "no claim", "no p tag"];

// BIP-340 vector 1's published test key and aux, as in chapter 49. Never
// sign with a real key in an example.
KEY_HEX :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
AUX_HEX :: "0000000000000000000000000000000000000000000000000000000000000001";
RELAY :: "wss://relay.example.com/";
CREATED :: 1790000000;
BAN :: "{\"method\":\"banpubkey\",\"params\":[\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"spam\"]}";
DELETEROLE :: "{\"method\":\"deleterole\",\"params\":[\"king\"]}";

main :: () {
  key: [32] u8; aux: [32] u8;
  decode_hex(KEY_HEX, key[..]); decode_hex(AUX_HEX, aux[..]);

  print("NIP-86: one banpubkey call, the client\n");
  hex: [64] u8; body_hash(BAN, hex[..]);
  print("body (% bytes): %\n", BAN.count, BAN);
  print("sha256(body): %\n", cast(string) hex[..]);
  tb: [256] u8; tags := auth_tags(RELAY, BAN, tb[..]);
  print("tags (% bytes): %\n", tags.count, tags);
  ev: [1024] u8; hb: [2048] u8;
  event := sign(key[..], aux[..], CREATED, tags, ev[..]);
  header := encode_header(event, hb[..]);
  print("event (% bytes): %\n", event.count, event);
  print("header value: % bytes, %...%\n", header.count, header[0..<24], header[header.count - 8..<header.count]);

  print("the relay, now = created_at + 30, one admin key\n");
  p: [1] N86Policy; decode_hex(N86_T_ADMIN, p[0].admins[0][..]); p[0].admins_n = 1;
  now := CREATED + 30;
  serve("banpubkey by the admin", p[..], N86_TYPE, header, BAN, now);
  serve("the same request again", p[..], N86_TYPE, header, BAN, now);
  serve("listbannedpubkeys by the admin", p[..], N86_TYPE, N86_T_LIST, N86_T_LIST_BODY, now);
  mallory: [32] u8; decode_hex(N86_T_MALLORY, mallory[..]);
  id: [32] u8;
  print("admission of a kind 1 event by mallory: %\n", ADMIT[n86_admit(p[0], mallory, id, 1)]);
  print("admission of a kind 1 event by the admin: %\n", ADMIT[n86_admit(p[0], p[0].admins[0], id, 1)]);

  print("refusals\n");
  serve("unbanpubkey signed by mallory", p[..], N86_TYPE, N86_T_MAL, N86_T_MAL_BODY, now);
  serve("the ban header with another body", p[..], N86_TYPE, header, N86_T_LIST_BODY, now);
  serve("a ban event without a payload tag", p[..], N86_TYPE, N86_T_NOPAY, BAN, now);
  serve("Content-Type text/plain", p[..], "text/plain", header, BAN, now);
  db: [256] u8; de: [1024] u8; dh: [2048] u8;
  serve("deleterole by the admin", p[..], N86_TYPE,
        encode_header(sign(key[..], aux[..], CREATED + 1, auth_tags(RELAY, DELETEROLE, db[..]), de[..]), dh[..]),
        DELETEROLE, now);
  print("banned keys: %, used event ids: %\n", p[0].banned.n, p[0].seen_n);

  print("NIP-66: relay discovery\n");
  r := n66_read_relay(n66_ev(30166, 1, 0, N66_T_RELAY));
  print("NIP-66's example: d %, rtt-open %, % N tags, % R tags\n",
        cast(string) r.url.b[0..<r.url.n], r.rtt[N66_OPEN], r.nips_n, r.reqs_n);
  trusted := [3][32]u8.[n66_k(0xa1), n66_k(0xa2), n66_k(0xa3)];
  reports := [5]NostrEvent.[
    n66_ev(30166, 0xa1, 100, "[[\"d\",\"wss://relay.example.com/\"],[\"rtt-open\",\"120\"]]"),
    n66_ev(30166, 0xa2, 100, "[[\"d\",\"wss://relay.example.com/\"],[\"rtt-open\",\"300\"]]"),
    n66_ev(30166, 0xa3, 100, "[[\"d\",\"wss://relay.example.com/\"],[\"rtt-open\",\"200\"]]"),
    n66_ev(30166, 0xa1, 40, "[[\"d\",\"wss://relay.example.com/\"],[\"rtt-open\",\"900\"]]"),
    n66_ev(30166, 0xb0, 100, "[[\"d\",\"wss://relay.example.com/\"],[\"rtt-open\",\"5\"]]")];
  m := n66_median(reports[..], trusted[..], RELAY, N66_OPEN);
  print("median rtt-open over % trusted monitors: % ms\n", m.monitors, m.rtt);
  m = n66_median(reports[..], trusted[0..<2], RELAY, N66_OPEN);
  print("over the first two only: % monitors, % ms\n", m.monitors, m.rtt);
  out: [512] u8; mine: N66Relay; mine.url = n66_text(RELAY); mine.network = n66_text("clearnet");
  mine.nips[0] = 11; mine.nips[1] = 86; mine.nips_n = 2; mine.rtt[N66_OPEN] = 200; mine.rtt[N66_READ] = -1; mine.rtt[N66_WRITE] = -1;
  n := n66_write_relay(mine, out[..]);
  print("30166 tags: %\n", cast(string) out[0..<n]);
  print("NIP-66's 10166 example, as printed: %\n", N66_WORDS[n66_read_monitor(n66_ev(10166, 0xa1, 0, N66_T_MONITOR)).status]);
  print("with the missing comma added: frequency % s, % timeouts, % checks\n", n66_read_monitor(n66_ev(10166, 0xa1, 0, N66_T_MONITOR_FIXED)).frequency,
        n66_read_monitor(n66_ev(10166, 0xa1, 0, N66_T_MONITOR_FIXED)).timeouts_n, n66_read_monitor(n66_ev(10166, 0xa1, 0, N66_T_MONITOR_FIXED)).checks_n);

  print("NIP-43: membership\n");
  self: [32] u8;
  print("self from NIP-11: %\n", n43_self(N43_T_NIP11, self[..]));
  list := n43_members(self, n43_ev(13534, 0xee, 0, N43_T_LIST));
  print("13534 signed by self: %, % members, second role %\n", N43_WORDS[list.status], list.n,
        cast(string) list.roles[1].b[0..<list.roles[1].n]);
  forged := n43_members(self, n43_ev(13534, 0xaa, 0, N43_T_LIST));
  print("the same list signed by another key: % (ignored), % members\n", N43_WORDS[forged.status], forged.n);
  j := n43_request(n43_ev(28934, 0x33, 1000, "[[\"-\"],[\"claim\",\"pizza-42\"]]"), 1000);
  print("join request: %, claim %\n", N43_WORDS[j.status], cast(string) j.claim.b[0..<j.claim.n]);
  print("the same request 301 s later: %\n", N43_WORDS[n43_request(n43_ev(28934, 0x33, 1000, "[[\"-\"],[\"claim\",\"pizza-42\"]]"), 1301).status]);
  print("a leave request without the - tag: %\n", N43_WORDS[n43_request(n43_ev(28936, 0x33, 1000, "[]"), 1000).status]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/relay-management.coax --run-steps 250000000
```

The output matches the worked values: a 107-byte body, 131 bytes of tags,
a 475-byte event with ID `343f4fd7...`, a 642-byte header that ends in
`==`, and the medians 200 and 210.

```text
NIP-86: one banpubkey call, the client
body (107 bytes): {"method":"banpubkey","params":["f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","spam"]}
sha256(body): d731279ebacd6e5bf9edb3d0355adfd839f78b7a5c08f375712eca6c8880bb94
tags (131 bytes): [["u","wss://relay.example.com/"],["method","POST"],["payload","d731279ebacd6e5bf9edb3d0355adfd839f78b7a5c08f375712eca6c8880bb94"]]
event (475 bytes): {"id":"343f4fd7b7b52d6ee4c3b10f0adbe3453b90183616b75e24b16c5d5ece8fba24","pubkey":"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659","created_at":1790000000,"kind":27235,"tags":[["u","wss://relay.example.com/"],["method","POST"],["payload","d731279ebacd6e5bf9edb3d0355adfd839f78b7a5c08f375712eca6c8880bb94"]],"content":"","sig":"8e83d819ac7498df4256089bac800667927e0972b75017abbfeb2d6732a4833b97391ca0617c195b36ca8ea3f7c8e859139bda6d9299e02e9bd42e5e054c829b"}
header value: 642 bytes, Nostr eyJpZCI6IjM0M2Y0Zm...OWIifQ==
the relay, now = created_at + 30, one admin key
banpubkey by the admin: 200 {"result":true}
the same request again: 401 {"result":null,"error":"event id already used"}
listbannedpubkeys by the admin: 200 {"result":[{"pubkey":"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","reason":"spam"}]}
admission of a kind 1 event by mallory: banned key
admission of a kind 1 event by the admin: admitted
refusals
unbanpubkey signed by mallory: 403 {"result":null,"error":"not an admin key"}
the ban header with another body: 401 {"result":null,"error":"unauthorized: the payload tag is not this body"}
a ban event without a payload tag: 401 {"result":null,"error":"the NIP-98 event has no payload tag"}
Content-Type text/plain: 415 {"result":null,"error":"Content-Type is not application/nostr+json+rpc"}
deleterole by the admin: 400 {"result":null,"error":"unknown method"}
banned keys: 1, used event ids: 3
NIP-66: relay discovery
NIP-66's example: d wss://some.relay/, rtt-open 234, 2 N tags, 2 R tags
median rtt-open over 3 trusted monitors: 200 ms
over the first two only: 2 monitors, 210 ms
30166 tags: [["d","wss://relay.example.com/"],["n","clearnet"],["N","11"],["N","86"],["rtt-open","200"]]
NIP-66's 10166 example, as printed: a tag does not read
with the missing comma added: frequency 3600 s, 4 timeouts, 5 checks
NIP-43: membership
self from NIP-11: true
13534 signed by self: ok, 2 members, second role 28b7e50f
the same list signed by another key: not signed by self (ignored), 0 members
join request: ok, claim pizza-42
the same request 301 s later: outside 300 s
a leave request without the - tag: no - tag
```

The refusals, one per check:

- `unbanpubkey` signed by Mallory passes NIP-98, so the signature, time,
  URL and payload were all right, and refuses as not an admin: 403. Her
  request does not use up an event ID.
- The ban header with another body refuses as the payload: 401. The
  replay check comes after NIP-98, so this header's recorded ID does not
  hide the real reason.
- A correctly signed admin event without a `payload` tag refuses: 401.
  Chapter 49's check alone would accept it with any body.
- `text/plain` refuses before the header is read: 415.
- `deleterole` passes every check up to the body and refuses as an
  unknown method: 400. Its event ID is used up, so the relay ends with 3
  used IDs: the ban, the list and this one.
- NIP-66's own 10166 example refuses as printed: it is missing the comma
  between its `["c","geo"]` and `["g",...]` tags, so its tags are not
  JSON. With the comma added it reads as frequency 3600 with 4 timeouts
  and 5 checks.
- The NIP-43 list signed by another key is ignored, a join request 301
  seconds old refuses, and a leave request without the `-` tag refuses.

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/relay-management.out](../fixtures/relay-management.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/relay-management.coax](../examples/checks/relay-management.coax),
which loads the example. The independent values come from
[nip59_reference.py](../../coax/tests/nostr/nip59_reference.py), a
pure-Python BIP-340 signer and NIP-01 serializer that shares no code with
Coax, from Python's `hashlib` and `base64`, and from Python's
`statistics.median`.

- The worked example's event, byte for byte, and its header equal to the
  Python-encoded `N86_T_BAN`.
- The relay's decisions on that header: accepted once, then a replay; and
  each refusal on a fresh relay leaves no used ID and no ban.
- The medians 200 and 210 from Python.

<!-- source: nostr/examples/checks/relay-management.coax check -->
```jai
#load "../relay-management.coax";

// The worked example's request signed by coax/tests/nostr/nip59_reference.py,
// a pure-Python BIP-340 signer and NIP-01 serializer that shares no code
// with Coax: signed(key, aux, 1790000000, 27235, tags, "") with BIP-340
// vector 1's key and aux; the body hash from Python's hashlib and the header
// from Python's base64.b64encode (N86_T_BAN in nip86_fixtures.coax).
PY_BODY_SHA256 :: "d731279ebacd6e5bf9edb3d0355adfd839f78b7a5c08f375712eca6c8880bb94";
PY_EVENT :: "{\"id\":\"343f4fd7b7b52d6ee4c3b10f0adbe3453b90183616b75e24b16c5d5ece8fba24\",\"pubkey\":\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\",\"created_at\":1790000000,\"kind\":27235,\"tags\":[[\"u\",\"wss://relay.example.com/\"],[\"method\",\"POST\"],[\"payload\",\"d731279ebacd6e5bf9edb3d0355adfd839f78b7a5c08f375712eca6c8880bb94\"]],\"content\":\"\",\"sig\":\"8e83d819ac7498df4256089bac800667927e0972b75017abbfeb2d6732a4833b97391ca0617c195b36ca8ea3f7c8e859139bda6d9299e02e9bd42e5e054c829b\"}";
// Python's statistics.median of the trusted monitors' times, and of the
// first two: 200 and 210.0.
PY_MEDIAN_3 :: 200; PY_MEDIAN_2 :: 210;

#test "the client builds Python's event and header, byte for byte" #steps 2000000000 {
  hex: [64] u8; body_hash(BAN, hex[..]);
  #assert cast(string) hex[..] == PY_BODY_SHA256;
  key: [32] u8; aux: [32] u8; decode_hex(KEY_HEX, key[..]); decode_hex(AUX_HEX, aux[..]);
  tb: [256] u8; ev: [1024] u8; hb: [2048] u8;
  tags := auth_tags(RELAY, BAN, tb[..]);
  #assert tags.count == 131;
  event := sign(key[..], aux[..], CREATED, tags, ev[..]);
  #assert event == PY_EVENT && event.count == 475;
  header := encode_header(event, hb[..]);
  #assert header == N86_T_BAN && header.count == 6 + (475 + 2) / 3 * 4;
}
#test "the relay's decisions on the worked example" #steps 2000000000 {
  p: [1] N86Policy; decode_hex(N86_T_ADMIN, p[0].admins[0][..]); p[0].admins_n = 1;
  out: [N86_OUT] u8; b: [256] u8;
  for i: 0..<BAN.count { b[i] = BAN[i]; }
  body := b[0..<BAN.count];
  #assert n86_serve(p[..], "POST", N86_TYPE, N86_T_BAN, RELAY, body, CREATED + 30, out[..]).http == 200;
  #assert n86_serve(p[..], "POST", N86_TYPE, N86_T_BAN, RELAY, body, CREATED + 30, out[..]).code == N86_REPLAYED;
  // Each refusal named in the chapter, on a fresh relay.
  q: [1] N86Policy; decode_hex(N86_T_ADMIN, q[0].admins[0][..]); q[0].admins_n = 1;
  #assert n86_serve(q[..], "POST", N86_TYPE, N86_T_MAL, RELAY, body, CREATED, out[..]).detail == N98_PAYLOAD;
  #assert n86_serve(q[..], "POST", N86_TYPE, N86_T_NOPAY, RELAY, body, CREATED, out[..]).code == N86_NO_PAYLOAD;
  #assert n86_serve(q[..], "POST", "text/plain", N86_T_BAN, RELAY, body, CREATED, out[..]).http == 415;
  #assert n86_serve(q[..], "PUT", N86_TYPE, N86_T_BAN, RELAY, body, CREATED, out[..]).http == 405;
  #assert n86_serve(q[..], "POST", N86_TYPE, N86_T_BAN, RELAY, body, CREATED - 61, out[..]).detail == N98_TIME;
  #assert q[0].seen_n == 0 && q[0].banned.n == 0;
}
#test "the median matches Python's statistics.median" {
  trusted := [3][32]u8.[n66_k(0xa1), n66_k(0xa2), n66_k(0xa3)];
  reports := [3]NostrEvent.[
    n66_ev(30166, 0xa1, 100, "[[\"d\",\"wss://relay.example.com/\"],[\"rtt-open\",\"120\"]]"),
    n66_ev(30166, 0xa2, 100, "[[\"d\",\"wss://relay.example.com/\"],[\"rtt-open\",\"300\"]]"),
    n66_ev(30166, 0xa3, 100, "[[\"d\",\"wss://relay.example.com/\"],[\"rtt-open\",\"200\"]]")];
  #assert n66_median(reports[..], trusted[..], RELAY, N66_OPEN).rtt == PY_MEDIAN_3;
  #assert n66_median(reports[..], trusted[0..<2], RELAY, N66_OPEN).rtt == PY_MEDIAN_2;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/relay-management.coax --run-steps 250000000
```

`coax test` runs 398 checks, including the loaded files' own checks.

### Checks in the library

- `nip86.coax`: the request reader on valid and malformed bodies and on
  each wrong params shape; every method's reply text; full lists refusing
  without a change; the admission rules in order; and the endpoint on the
  four Python-signed headers in `nip86_fixtures.coax`: each refusal code,
  the 60-second bounds, the replay and the full list of used IDs.
- `nip66.coax`: NIP-66's 30166 example tags verbatim, read and written
  back; its 10166 example refusing as printed and reading with the comma
  added; every refusal; and the median over ties, an even count, an older
  event, untrusted keys and nine monitors (one past the limit).
- `nip43.coax`: NIP-43's 13534 example tags verbatim, signed by the
  relay's key and by another; the `self` field read from NIP-11 documents;
  8000 and 8001 events; and the 300-second window's inclusive bounds.

The NIPs' example events use placeholders such as `<eventid>` and
`<signature>` for their IDs and signatures, so there are no published IDs
to compare; the checks use the examples' tags only. NIP-66's example also
names its signature field `signature` instead of NIP-01's `sig`.

On 2026-11-01, `coax mutate --root-only` killed every mutant with none
surviving: 412 of 412 in `nip86.coax`, 319 of 319 in `nip66.coax` and 205
of 205 in `nip43.coax`. The first runs' survivors were killed by new checks
or by removing a condition that could not change the result. Full
mutation, and mutation of the example and its checks, were not run.
Chapter 49's mutation and differential records for `nip98.coax` were not
rerun.

### The relay's endpoint

Since chapter 103's wiring, `coax/relay/relay.coax` started with
`--admin PUBKEY` (64 hex digits; up to 4 keys) serves this endpoint on its
own URL:

- `relay_route.coax`'s `rr_post` (`#authority`) reads the request: the
  line `POST TARGET HTTP/1.1`, header lines ending in CRLF within 8,192
  bytes, exactly one `Content-Length` of at most 4,096 bytes, and at most
  one `Content-Type` and one `Authorization`. A request it refuses is a
  `400`, or a `413` when too large.
- `n86_serve` answers with the relay URL (the one NIP-42 answers name) as
  the `u` the NIP-98 event must carry, and the reply goes back as
  `application/nostr+json+rpc`; then the connection closes.
- Every `EVENT` goes through `n86_admit` after its signature and the rate
  limit: a banned key is `blocked: this key is banned on this relay`, a
  key off a non-empty allow list `restricted: this key is not on this
  relay's allow list`, and likewise for a banned event, a disallowed kind
  and a kind off the allowed-kinds list.
- NIP-11 lists 86 only with `--admin` (`rr_nip11_base`).
- `n86_serve` remembers 8 used NIP-98 IDs. An event is good for 60 seconds
  either side of its `created_at`, so one accepted at time t is dead by
  t + 120: the relay forgets them all once 121 seconds pass after the last
  one was added (`rr_admin_forget`). Until then a ninth call is a `503`.
- Without `--admin`, a `POST` is a `400`, as before.
- With `--store FILE` the policy survives a restart. A call that changes
  it (answered `N86_OK` by a method that takes params, `N86_SHAPES` above
  0) has its body appended to `FILE.nip86` and synced before the change
  is kept and answered: `ry_admin` runs `n86_serve` on a copy of the
  policy and keeps the copy only once the frame is on disk, and
  `relay_route.coax`'s `rr_admin_saves` (`#authority`) says which calls
  are saved. The frames are chapter 35's: a 4-byte big-endian length,
  then the bytes. A failed append keeps only the used NIP-98 IDs and
  answers `500`. A listing is never saved, nor is a body holding a zero
  byte. The file is created, mode 0600, at the first change.
- On start, `ry_journal_replay` applies each frame in order with
  `n86_dispatch`, without NIP-98: each call was authorized when it was
  made. `store_recover.coax` decides each frame's fate, as it does for
  the store file: a crash's partial last append is cut, and any other
  damage stops the relay before it listens, leaving the file as it was.
  A whole frame continues only when it is one the relay would have saved,
  so a frame holding an unknown method also refuses to start.

Root-only mutation of `relay_route.coax` on 2026-11-01: 824 of 824
killed, and 914 of 914 after `rr_admin_saves` and `rr_group_keep`
(chapter 59) were added. `relay_session.py` (ASan and UBSan) checks each refusal, a
replayed event, `banpubkey` and its effect on a connected publisher,
`listbannedpubkeys`, `disallowkind`, `unbanpubkey` and `allowpubkey`
against a running relay. Its restart check bans a key and disallows a
kind with `--store`, restarts, and finds both still refused and the ban
listed; it then appends half a frame to `FILE.nip86`, and the next start
cuts it back to the last whole frame with the ban intact; a whole frame
with an unknown method refuses to start and leaves the file unchanged.

### Not built

- Without `--store` the relay keeps the policy in memory: a restart
  starts with empty lists. `FILE.nip86` only grows: every change stays in
  it, and nothing compacts it. Not trusted freshness: an older copy of
  the file replays as an older policy, and the journal is not signed, so
  anyone who can write it can change the policy. `changerelayname` changes the policy's name, not the served NIP-11
  document. Banning a key or event doesn't remove what is already stored.
- NIP-86: `allowevent`, `unallowevent`, `listallowedevents`,
  `listeventsneedingmoderation`, roles, claims, IP blocks,
  `changerelaydescription` and `changerelayicon`.
- NIP-66: the `t`, `k`, `g`, `l` and `T` tags, and the NIP-11 document in
  the content.
- NIP-43: role definitions (33534), invites (28935), checking claims, and
  the member's own kind 10010 list.

## Summary

- A NIP-86 call is accepted only with the right media type, a `POST`, a
  valid NIP-98 event with a `payload` tag, an admin signer and a new event
  ID. The body is read last.
- The policy changes only when a call succeeds; a full list refuses.
- One monitor can be wrong, so the NIP-66 median uses several trusted ones
  and each monitor's newest event.
- A NIP-43 member list counts only when the relay's NIP-11 `self` key
  signed it.

## Dependencies

- [37. NIP-11 relay information](ch037-nip-11-relay-information.md)
- [38. NIP-42 authentication](ch038-nip-42-authentication.md)
- [49. HTTP auth](ch049-http-auth.md)

## Sources

- [NIP-66](../nips/official/66.md)
- [NIP-43](../nips/official/43.md)
- [NIP-86](../nips/official/86.md)
- [NIP-98](../nips/official/98.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
