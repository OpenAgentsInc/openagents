# 65. Data vending machines

NIP-90 lets a customer ask for computation over Nostr and pay for it in
Lightning. The customer publishes a job request, service providers answer
with feedback and a result, and the result carries the request it answers.
The pinned text now says the NIP "got totally out of control" and prefers
use-case-specific microstandards. OpenAgents' own job NIPs (chapter 80)
follow that advice. This chapter builds NIP-90 for interoperability: it
reads and writes the three event types, decides whether a result really
answers a request, decides whether a customer pays the amount asked, and
runs the customer's side of a job against a fixed clock.

Part X, Every NIP. Result: NIP-90 job requests (5000–5999), results
(6000–6999) and feedback (7000), job chaining, and why the pinned text marks
it `unrecommended`.

## What this chapter teaches

- Requests: `i` inputs of type `url`, `event`, `job` or `text`, `param`,
  `output`, `bid` in millisats, `relays` and `p`.
- Results: kind = request kind + 1000, a `request` tag with the request's
  full JSON, `e` and `p` tags, and `amount` with an optional bolt11.
- Feedback: kind 7000 with `status` `payment-required`, `processing`,
  `error`, `success` or `partial`.
- Matching: why a result's kind, `e`, `p` and embedded request must all
  agree with the request the customer published.
- Paying: the amount tag, the invoice's own amount, and the bid as the
  ceiling, all in millisats and all Bitcoin.
- The customer's state machine over feedback and results, with a deadline
  and a clock the caller supplies.
- Encrypted requests: NIP-90 specifies NIP-04; this stack refuses NIP-04
  and opens NIP-44 only.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_tag(e, i)` | Reads tag `i`: its name and first two values (chapter 55) |
| [nip53.coax](../../coax/lib/nostr/nip53.coax) | `n53_value(e, i, k, out)` | Reads value `k` of tag `i` into a buffer of any size (chapter 61) |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag(e, i)` | Reads an `e` or `p` tag (chapter 22) |
| [nip44.coax](../../coax/lib/nostr/nip44.coax) | `nip44_decrypt_checked` | NIP-44 v2 decryption (chapter 20) |
| [nip90.coax](../../coax/lib/nostr/nip90.coax) | `n90_request(e)`, `n90_private(e, r, key, work, out)` | Reads a job request; opens an encrypted one |
| [nip90.coax](../../coax/lib/nostr/nip90.coax) | `n90_reply(e)`, `n90_match(request, reply, work)` | Reads a result or feedback; decides whether it answers the request |
| [nip90.coax](../../coax/lib/nostr/nip90.coax) | `n90_msats(s)`, `n90_invoice(bolt11)`, `n90_price(bid, amount, bolt11)` | Millisat amounts, an invoice's stated amount, the payment decision |
| [nip90.coax](../../coax/lib/nostr/nip90.coax) | `n90_job`, `n90_tick`, `n90_step` | The customer's job: start, deadline, one event at a time |
| [nip90.coax](../../coax/lib/nostr/nip90.coax) | `n90_request_tags`, `n90_reply_tags` | Write a request's tags; write a result's or feedback's tags |

`nip90.coax` is new with this chapter. It is an `#authority` file,
because `n90_price` and `n90_step` decide whether a customer pays, and
`n90_match` decides which result a customer accepts. Every procedure runs
in a check, and `coax mutate` leaves no survivors.
`coax/tests/nostr/nip90_fixtures.coax` holds the checks' test events, so
that mutation changes the library and not the checks. Nothing in
`nip90.coax` fetches an event, verifies a signature, decodes an invoice's
signature or pays. The caller verifies events (chapter 21) before passing
them in, and its wallet pays.

## Job requests

A job request is a kind between 5000 and 5999. The kind names the job:
5050 is text generation, 5000 speech to text, and so on, in a list kept
outside the NIP. Every tag is optional:

- `["i", <data>, <type>, <relay>, <marker>]`: one input. The type is
  `url` (fetch the data), `event` (a Nostr event id), `job` (the output of
  an earlier job, by its request's id) or `text` (the data itself). The
  relay says where an `event` or `job` input was published, and the marker
  says how the job uses the input.
- `["param", <key>, <value>]`: a parameter, defined by the job kind.
- `["output", <mime-type>]`: the format the customer wants.
- `["bid", <msats>]`: the most the customer will pay, in millisats.
- `["relays", <url>, ...]`: where providers should publish replies.
- `["p", <pubkey>]`: the providers the customer prefers. Others may still
  answer.

<!-- source: coax/lib/nostr/nip90.coax record -->
```jai
N90Input :: struct {
  type:int=-1; data:N32Text; relay:N32Text; marker:N32Text;
  id:[32]u8;                          // event and job: the event id
}
N90Param :: struct {key:N32Text; value:N32Text;}
N90Request :: struct {
  error:int=N90_EVENT; at:int=-1;     // the refusing tag
  kind:int;
  inputs:[N90_MAX_I]N90Input; ni:int;
  params:[N90_MAX_PARAM]N90Param; np:int;
  output:N32Text;
  bid:int=-1;                         // millisats, -1 when absent
  providers:[N90_MAX_P][32]u8; nproviders:int;
  relays:int;                         // relay URLs in relays tags
  encrypted:bool;
}
// A result (kind 6000-6999) or feedback (kind 7000).
N90Reply :: struct {
  error:int=N90_EVENT; at:int=-1;
  kind:int; status:int=-1; extra:N32Text;
  request:[32]u8;                     // the e tag
  customer:[32]u8;                    // the p tag
  amount:int=-1;                      // millisats, -1 when absent
  bolt11:[N90_MAX_BOLT11]u8; bolt11_n:int;
  request_tag:int=-1;                 // the request tag's index
  encrypted:bool;
}
N90Price :: struct {error:int; msats:int;}
// The customer's view of one request.
N90Job :: struct {
  state:int=N90_SENT; error:int;      // error: the last step's refusal
  sent:int; deadline:int; bid:int=-1;
  has_provider:bool; provider:[32]u8; // the first provider that answered
  updated:int;                        // created_at of the last feedback taken
  status:int=-1; owed:int=-1;         // the last status; the amount to pay
  result:[32]u8;
}
// What the writers take.
N90In :: struct {data:string; type:int; relay:string; marker:string;}
N90Pair :: struct {key:string; value:string;}
```

`n90_request` reads every tag once. It refuses an input whose type isn't
one of the four, an `event` or `job` input whose data isn't 64 lowercase
hex digits, an input relay that isn't `ws://` or `wss://`, a `param`
without a key or a value, an empty or repeated `output`, a `bid` that
isn't a millisat amount or comes twice, and a `p` that isn't a public key.
It keeps at most 8 inputs, 8 parameters and 4 providers, and a value read
by `n32_tag` holds at most 256 bytes, so a longer text input is refused as
`N90_LIMIT`. Two bids are refused, not resolved: a customer's ceiling
must be one number.

<!-- source: coax/lib/nostr/nip90.coax request -->
```jai
// The i tag at index: its type and data, and its relay and marker (the
// third and fourth values). An event or job input names a 64-hex event id;
// a relay, when given, is a ws:// or wss:// URL. type -1 when it isn't.
n90_input :: (e:NostrEvent,index:int,t:N32Tag)->N90Input {
  x:N90Input;bad:N90Input;
  for k:0..<4 {if n32_is(t.v[2],N90_INPUT_NAMES[k]) {x.type=k;}}
  x.data=t.v[1];
  if x.type==N90_EVENT_ID || x.type==N90_JOB {
    d:=t.v[1];if !ne_lower_hex(cast(string)d.b[0..<d.n],x.id[..]) {return bad;}
  }
  rn:=n53_value(e,index,3,x.relay.b[..]);mn:=n53_value(e,index,4,x.marker.b[..]);
  if rn<0 || mn<0 {return bad;}
  x.relay.n=rn;x.marker.n=mn;
  if rn>0 && !nt_relay_ok(cast(string)x.relay.b[0..<rn]) {return bad;}
  return x;
}
// Whether the content is a NIP-04 payload: NIP-04's ?iv= suffix. NIP-44's
// base64 never holds a ?.
n90_nip04 :: (e:NostrEvent)->bool {
  for i:e.content_from..<e.content_to {if e.original[i]=='?' {return true;}}
  return false;
}
// A job request (kind 5000-5999). An encrypted one names one provider and
// carries no clear i or param tags.
n90_request :: (e:NostrEvent)->N90Request {
  r:N90Request;if e.error!=NE_OK {return r;}
  if e.kind<N90_REQUEST_MIN || e.kind>N90_REQUEST_MAX {return n90_req_fail(r,N90_KIND,-1);}
  r.kind=e.kind;i:=0;has_output:=false;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n90_req_fail(r,N90_LIMIT,i);}
    if t.error!=NT_OK {return n90_req_fail(r,N90_EVENT,i);}
    if n32_is(t.v[0],"i") {
      x:=n90_input(e,i,t);
      if x.type<0 {return n90_req_fail(r,N90_INPUT,i);}
      if r.ni==N90_MAX_I {return n90_req_fail(r,N90_LIMIT,i);}
      r.inputs[r.ni]=x;r.ni+=1;
    } else if n32_is(t.v[0],"param") {
      if t.values<3 || t.v[1].n==0 {return n90_req_fail(r,N90_PARAM,i);}
      if r.np==N90_MAX_PARAM {return n90_req_fail(r,N90_LIMIT,i);}
      r.params[r.np].key=t.v[1];r.params[r.np].value=t.v[2];r.np+=1;
    } else if n32_is(t.v[0],"output") {
      if t.v[1].n==0 || has_output {return n90_req_fail(r,N90_TAG,i);}
      r.output=t.v[1];has_output=true;
    } else if n32_is(t.v[0],"bid") {
      c:=t.v[1];b:=n90_msats(cast(string)c.b[0..<c.n]);
      if b<0 || r.bid>=0 {return n90_req_fail(r,N90_BID,i);}
      r.bid=b;
    } else if n32_is(t.v[0],"relays") {
      r.relays+=t.values-1;
    } else if n32_is(t.v[0],"p") {
      p:=nostr_tag(e,i);
      if p.error!=NT_OK {return n90_req_fail(r,N90_TAG,i);}
      if r.nproviders==N90_MAX_P {return n90_req_fail(r,N90_LIMIT,i);}
      r.providers[r.nproviders]=p.pubkey;r.nproviders+=1;
    } else if n32_is(t.v[0],"encrypted") {
      r.encrypted=true;
    }
    i+=1;
  }
  if r.encrypted {
    if r.nproviders!=1 || r.ni>0 || r.np>0 {return n90_req_fail(r,N90_ENCRYPTED,-1);}
    if n90_nip04(e) {return n90_req_fail(r,N90_NIP04,-1);}
  }
  return n90_req_fail(r,N90_OK,-1);
}
```

## Encrypted requests: NIP-44, not NIP-04

A customer who wants to keep the inputs secret moves the `i` and `param`
tags into the content, encrypted to one provider, and adds an `encrypted`
tag and that provider's `p` tag. The pinned NIP-90 says to encrypt with
NIP-04.

This stack doesn't. Chapter 19 keeps NIP-04 as a decrypt-only reader for
old direct messages: it has no authentication, so a relay can change the
ciphertext without the reader noticing, and it uses the shared x as the
AES key with no key derivation. A job request is a new message, not an old
one to migrate, so `n90_request` refuses an encrypted request whose
content has NIP-04's `?iv=` suffix as `N90_NIP04`. NIP-44 base64 never
contains `?`, so the test is exact. The request is refused, not opened,
because a reader that accepted NIP-04 here would let a provider answer
inputs that anyone on the path could have changed.

An encrypted request must name exactly one provider and carry no clear
`i` or `param` tags, or it is refused as `N90_ENCRYPTED`. The provider
opens it with `n90_private`: it decrypts the content as NIP-44 v2 with the
conversation key (chapter 20), reads the plaintext as a tag array, and puts
its inputs and parameters in place of the request's. The MAC is checked
before anything is decrypted, so a wrong key or a changed byte gives
`N90_DECRYPT` and no plaintext.

<!-- source: coax/lib/nostr/nip90.coax private -->
```jai
// An encrypted request opened by its provider: the content decrypted as
// NIP-44 v2 with the conversation key into out, read as a tag array, and
// its i and param tags put in place of the request's. work holds the
// decoded payload (nip44_decrypt_work_size).
n90_private :: (e:NostrEvent,r:N90Request,conversation_key:[]u8,work:[]u8,out:[]u8)->N90Request {
  if r.error!=N90_OK || !r.encrypted {return n90_req_fail(r,N90_ENCRYPTED,-1);}
  d:=nip44_decrypt_checked(NIP44_COMPAT,conversation_key,e.original[e.content_from..<e.content_to],work,out,false);
  if d.error!=NIP44_OK {return n90_req_fail(r,N90_DECRYPT,-1);}
  inner:=nt_event(cast(string)out[0..<d.written]);inner.kind=e.kind;
  q:=n90_request(inner);
  if q.error!=N90_OK {return q;}
  if q.encrypted {return n90_req_fail(r,N90_ENCRYPTED,-1);}
  o:=r;o.inputs=q.inputs;o.ni=q.ni;o.params=q.params;o.np=q.np;
  return o;
}
```

This interoperates only with NIP-90 clients that also use NIP-44. A
provider that follows the pinned text and encrypts with NIP-04 can't serve
this customer, and this reader refuses its customers' encrypted requests.
That is the intended result.

## Results and feedback

A provider answers with two kinds of events:

- A result, of kind request kind + 1000: a 5050 request gets a 6050
  result. Its content is the output. Its tags are `["request", <the
  request's JSON>]`, `["e", <request id>, <relay>]`, the request's `i`
  tags, `["p", <customer>]` and optionally `["amount", <msats>,
  <bolt11>]`. A result for an encrypted request carries `encrypted` and no
  clear `i` tags.
- Feedback, kind 7000, with `["status", <status>, <extra>]`, the same `e`
  and `p` tags and optionally an `amount`. Its content is empty or a
  sample of a partial result.

| Status | Meaning |
| --- | --- |
| `payment-required` | The provider won't continue until it is paid |
| `processing` | The provider is working on the job |
| `error` | The provider couldn't do the job |
| `success` | The provider finished the job |
| `partial` | The provider did part of the job; the content may hold a sample |

A `request` tag holds a whole event, and an invoice is often longer than
256 bytes, so both are longer than `n32_tag` reads. `n90_reply` reads the
name of such a tag from its raw first value (`n90_named`) and reads its
values with `n53_value` into larger buffers: up to 1,024 bytes for the
invoice, and the caller's work buffer for the request. It refuses a reply
without exactly one `e` and one `p` tag, an amount that isn't millisats or
comes twice, an unknown or repeated status, a status on a result, feedback
without a status and a result without a `request` tag.

<!-- source: coax/lib/nostr/nip90.coax reply -->
```jai
// Whether tag index is named name, read from its raw first value.
n90_named :: (e:NostrEvent,index:int,name:string)->bool {
  j:[1]Json;started:=nt_start(j[..],e);
  tok:=json_next(j[..]);
  for i:0..<index {skipped:=json_skip(j[..]);tok=json_next(j[..]);}
  v:=json_next(j[..]);
  return json_is(json_text(j[..]),name);
}
// A job result (kind 6000-6999) or feedback (kind 7000): one e tag naming
// the request and one p tag naming the customer; a result has a request
// tag and no status, feedback one known status.
n90_reply :: (e:NostrEvent)->N90Reply {
  r:N90Reply;if e.error!=NE_OK {return r;}
  result:=e.kind>=N90_REQUEST_MIN+N90_RESULT_OFFSET && e.kind<=N90_REQUEST_MAX+N90_RESULT_OFFSET;
  if !result && e.kind!=N90_FEEDBACK {return n90_reply_fail(r,N90_KIND,-1);}
  r.kind=e.kind;i:=0;has_e:=false;has_p:=false;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    // A request tag holds a whole event and an amount tag an invoice:
    // longer than n32_tag reads. Their values are read below.
    long:=t.error==NT_LIMIT && (n90_named(e,i,"request") || n90_named(e,i,"amount"));
    if t.error==NT_LIMIT && !long {return n90_reply_fail(r,N90_LIMIT,i);}
    if t.error!=NT_OK && !long {return n90_reply_fail(r,N90_EVENT,i);}
    if n32_is(t.v[0],"e") {
      x:=nostr_tag(e,i);
      if x.error!=NT_OK || has_e {return n90_reply_fail(r,N90_TAG,i);}
      has_e=true;r.request=x.id;
    } else if n32_is(t.v[0],"p") {
      x:=nostr_tag(e,i);
      if x.error!=NT_OK || has_p {return n90_reply_fail(r,N90_TAG,i);}
      has_p=true;r.customer=x.pubkey;
    } else if n32_is(t.v[0],"amount") {
      c:=t.v[1];a:=n90_msats(cast(string)c.b[0..<c.n]);
      if a<0 || r.amount>=0 {return n90_reply_fail(r,N90_AMOUNT,i);}
      bn:=n53_value(e,i,2,r.bolt11[..]);
      if bn<0 {return n90_reply_fail(r,N90_LIMIT,i);}
      r.amount=a;r.bolt11_n=bn;
    } else if n32_is(t.v[0],"status") {
      s:=-1;
      for k:0..<5 {if n32_is(t.v[1],N90_STATUS_NAMES[k]) {s=k;}}
      if s<0 || r.status>=0 || result {return n90_reply_fail(r,N90_STATUS,i);}
      r.status=s;r.extra=t.v[2];
    } else if n32_is(t.v[0],"request") {
      if r.request_tag>=0 {return n90_reply_fail(r,N90_REQUEST,i);}
      r.request_tag=i;
    } else if n32_is(t.v[0],"encrypted") {
      r.encrypted=true;
    }
    i+=1;
  }
  if !has_e {return n90_reply_fail(r,N90_NO_E,-1);}
  if !has_p {return n90_reply_fail(r,N90_P,-1);}
  if !result && r.status<0 {return n90_reply_fail(r,N90_STATUS,-1);}
  if result && r.request_tag<0 {return n90_reply_fail(r,N90_REQUEST,-1);}
  return n90_reply_fail(r,N90_OK,-1);
}
```

## Matching a reply to its request

A customer listens for kinds 6000–6999 and 7000 that tag it, so it sees
replies to all of its requests and anything else anyone tags it with.
`n90_match` decides whether one reply answers one request:

1. The reply reads (`n90_reply`).
2. The request's kind is between 5000 and 5999.
3. A result's kind is the request's kind + 1000.
4. The `e` tag names the request's id.
5. The `p` tag names the request's author.
6. For a result: the `request` tag's value reads as an event whose id
   checks (its hash matches its fields) and equals the request's id.

Step 6 is the one the NIP's own field list leaves to the reader. Steps 4
and 6 check different things. The `e` tag says which request the provider
claims to answer. The embedded request says which inputs the provider
actually worked on. A result could name the right id and embed a
different request with other inputs, or embed a request the customer
never sent. Because the embedded event's id is the SHA-256 of its
serialized fields, an equal id means the provider saw the same author,
time, kind, tags and content as the customer signed. The embedded
signature isn't checked again: the customer already holds and verified
the request with that id.

<!-- source: coax/lib/nostr/nip90.coax match -->
```jai
// Whether reply answers request: N90_OK or the refusal. A result's kind is
// the request's + 1000 and the request it embeds reads back, through the
// id check, as an event with the request's id. work holds that embedded
// request twice: decoded, then serialized for its hash; N90_LIMIT when it
// doesn't fit.
n90_match :: (request:NostrEvent,reply:NostrEvent,work:[]u8)->int {
  r:=n90_reply(reply);
  if r.error!=N90_OK {return r.error;}
  if request.kind<N90_REQUEST_MIN || request.kind>N90_REQUEST_MAX {return N90_KIND;}
  if r.kind!=N90_FEEDBACK && r.kind!=request.kind+N90_RESULT_OFFSET {return N90_KIND;}
  if !ns_equal32(r.request,request.id) {return N90_E;}
  if !ns_equal32(r.customer,request.public_key) {return N90_P;}
  if r.kind==N90_FEEDBACK {return N90_OK;}
  half:=work.count/2;
  n:=n53_value(reply,r.request_tag,1,work[0..<half]);
  if n<0 {return N90_LIMIT;}
  embedded:=nostr_event_read(cast(string)work[0..<n],work[half..<work.count],false);
  if embedded.error!=NE_OK || !ns_equal32(embedded.id,request.id) {return N90_REQUEST;}
  return N90_OK;
}
```

A result whose `request` tag doesn't match the request's id is refused as
`N90_REQUEST`. When the embedded request is longer than half the work
buffer, the answer is `N90_LIMIT`, not a match.

## Paying: amount, invoice and bid

Every amount in NIP-90 is in millisats. A provider asks for payment with
an `amount` tag, optionally with a bolt11 invoice. The customer pays the
invoice, or zaps the reply (chapter 71). This chapter decides whether to
pay; it doesn't pay.

`n90_msats` reads a decimal amount: digits only, no sign, no leading zero,
at most 21 million bitcoin (2,100,000,000,000,000,000 millisats).

`n90_invoice` reads the amount a BOLT11 invoice states in its
human-readable part, the text before the last `1`: `ln`, a currency, the
amount digits and an optional multiplier. The currencies are Bitcoin
only: `bc` (mainnet), `tb` (testnet), `tbs` (signet) and `bcrt`
(regtest). The multipliers are:

| Multiplier | Bitcoin | Millisats per unit |
| --- | --- | --- |
| none | 1 | 100,000,000,000 |
| `m` | 0.001 | 100,000,000 |
| `u` | 0.000001 | 100,000 |
| `n` | 0.000000001 | 100 |
| `p` | 0.000000000001 | 0.1 (the amount must be a multiple of 10) |

It refuses each problem with its own code, so that the checks can tell the
cases apart: -1 no separator, -2 no amount, -3 not a Bitcoin currency, -4 a
leading zero, -5 part of a millisat, -6 more than 21 million bitcoin. An
invoice without an amount is refused, because the customer would then
choose what to pay. The invoice's checksum, its signature and its payment
hash are the wallet's to check (chapter 72, and the Breez Spark plan in
[docs/lightning](../../docs/lightning/README.md)).

<!-- source: coax/lib/nostr/nip90.coax msats -->
```jai
// A decimal amount of millisats: digits, no sign, no leading zero, at most
// 21 million bitcoin. -1 when it isn't one.
n90_msats :: (s:string)->int {
  if s.count==0 || (s[0]=='0' && s.count>1) {return -1;}
  v:=0;
  for c:s {
    if c<'0' || c>'9' {return -1;}
    d:=cast(int)(c-'0');
    if v>(N90_MAX_MSATS-d)/10 {return -1;}
    v=v*10+d;
  }
  return v;
}
// The amount a BOLT11 invoice's human-readable part states, in millisats:
// ln, a Bitcoin currency, digits without a leading zero and an optional
// multiplier m, u, n or p, at most 21 million bitcoin (a p amount must be
// whole millisats). The part ends at the last 1. Or the refusal: -1 no 1
// after the first byte, -2 no amount, -3 not a Bitcoin currency, -4 a
// leading zero, -5 part of a millisat, -6 too large. The checksum and the
// signature are the wallet's to check.
n90_invoice :: (bolt11:string)->int {
  sep:=-1;
  for i:1..<bolt11.count {if bolt11[i]=='1' {sep=i;}}
  if sep== -1 {return -1;}
  h:=bolt11[0..<sep];
  // Each multiplier's scale to millisats and the most it may count.
  scale:=100000000000;most:=21000000;cut:=0;c:=h[h.count-1];
  if c=='m' {scale=100000000;most=21000000000;cut=1;}
  else if c=='u' {scale=100000;most=21000000000000;cut=1;}
  else if c=='n' {scale=100;most=21000000000000000;cut=1;}
  else if c=='p' {scale=0;most=N90_MAX_MSATS;cut=1;}
  end:=h.count-cut;start:=end;
  while start>0 && h[start-1]>='0' && h[start-1]<='9' {start-=1;}
  if start==end {return -2;}
  known:=false;
  for x:N90_CURRENCIES {if h[0..<start]==x {known=true;}}
  if !known {return -3;}
  if h[start]=='0' {return -4;}
  v:=0;
  for i:start..<end {
    d:=cast(int)(h[i]-'0');
    if v>(most-d)/10 {return -6;}
    v=v*10+d;
  }
  if scale==0 {
    if v%10!=0 {return -5;}
    return v/10;
  }
  return v*scale;
}
```

`n90_price` makes the decision:

1. The amount must be millisats (`N90_AMOUNT`).
2. If an invoice is given, its amount must equal the amount tag
   (`N90_INVOICE`). Otherwise a provider could ask for 5,000 millisats in
   the tag and 10,000 in the invoice the wallet pays.
3. An amount of 0 owes nothing.
4. Without a bid, nothing is paid automatically (`N90_NO_BID`): the
   customer set no ceiling, so a person must decide.
5. An amount over the bid is refused (`N90_OVER_BID`). An amount equal to
   the bid is paid.

<!-- source: coax/lib/nostr/nip90.coax price -->
```jai
// Whether the customer pays amount millisats: an invoice, when given,
// states the same amount; nothing is owed for 0; otherwise the request's
// bid is the ceiling, and without a bid nothing is paid automatically.
n90_price :: (bid:int,amount:int,bolt11:string)->N90Price {
  p:N90Price;p.msats=amount;
  if amount<0 {p.error=N90_AMOUNT;return p;}
  if bolt11.count>0 && n90_invoice(bolt11)!=amount {p.error=N90_INVOICE;return p;}
  if amount==0 {return p;}
  if bid<0 {p.error=N90_NO_BID;return p;}
  if amount>bid {p.error=N90_OVER_BID;}
  return p;
}
```

## The customer's job

`n90_job` starts a job from the request the customer published, with a
deadline `wait` seconds after the request's `created_at`. `n90_step` takes
one event and the current time, and returns the new state. The caller
supplies the time, so the checks and the example run against a fixed
clock.

| State | Meaning | Final |
| --- | --- | --- |
| sent | No reply taken yet | no |
| payment required | The provider asked for an amount within the bid; `owed` holds it | no |
| working | `processing`, `success` or `partial` feedback | no |
| done | A result was taken; `result` holds its id | yes |
| failed | `error` feedback, or the request didn't read | yes |
| too dear | An amount over the bid, or with no bid | yes |
| expired | The deadline passed first | yes |

The rules, in the order `n90_step` applies them:

1. An open job past its deadline expires (`n90_tick`). A final job takes
   nothing more (`N90_CLOSED`).
2. An event made before the request, or after `now`, is refused
   (`N90_TIME`).
3. Once a provider has answered, events from any other provider are
   refused (`N90_PROVIDER`). The customer follows one provider per job.
4. The event must match the request (`n90_match`).
5. Feedback made before the last feedback taken is stale (`N90_STALE`):
   relays deliver events out of order, and an old `processing` must not
   undo a newer `payment-required`. A result is final whenever it was made.
6. `payment-required` without an amount is refused (`N90_AMOUNT`).
7. An amount goes through `n90_price`. Over the bid or with no bid, the job
   is too dear. An invoice for another amount is refused and the job stays
   open.

A refused event leaves the state as it was and sets `error`.

<!-- source: coax/lib/nostr/nip90.coax job -->
```jai
// A new job for a request the customer published: it waits for answers
// until wait seconds after the request's created_at.
n90_job :: (request:NostrEvent,wait:int)->N90Job {
  j:N90Job;r:=n90_request(request);
  if r.error!=N90_OK {j.state=N90_FAILED;j.error=r.error;return j;}
  j.sent=request.created_at;j.deadline=request.created_at+wait;j.bid=r.bid;j.updated=request.created_at;
  return j;
}
// The job at time now: an open job past its deadline has expired.
n90_tick :: (j:N90Job,now:int)->N90Job {
  s:=j;if s.state<N90_DONE && now>s.deadline {s.state=N90_EXPIRED;}
  return s;
}
// One event (feedback or result) taken at time now. A refused event leaves
// the state as it was and sets error. Feedback older than the last one
// taken is stale; a result is final whenever it was made. An amount over
// the bid, or with no bid, closes the job as too dear.
n90_step :: (j:N90Job,request:NostrEvent,ev:NostrEvent,now:int,work:[]u8)->N90Job {
  s:=n90_job_fail(n90_tick(j,now),N90_OK);
  if s.state>=N90_DONE {return n90_job_fail(s,N90_CLOSED);}
  if ev.created_at<s.sent || ev.created_at>now {return n90_job_fail(s,N90_TIME);}
  if s.has_provider && !ns_equal32(ev.public_key,s.provider) {return n90_job_fail(s,N90_PROVIDER);}
  m:=n90_match(request,ev,work);
  if m!=N90_OK {return n90_job_fail(s,m);}
  r:=n90_reply(ev);feedback:=r.kind==N90_FEEDBACK;
  if feedback && ev.created_at<s.updated {return n90_job_fail(s,N90_STALE);}
  if r.status==N90_PAYMENT_REQUIRED && r.amount<0 {return n90_job_fail(s,N90_AMOUNT);}
  o:=s;
  if r.amount>=0 {
    p:=n90_price(s.bid,r.amount,cast(string)r.bolt11[0..<r.bolt11_n]);
    if p.error==N90_OVER_BID || p.error==N90_NO_BID {o.state=N90_TOO_DEAR;return n90_job_fail(o,p.error);}
    if p.error!=N90_OK {return n90_job_fail(s,p.error);}
    o.owed=p.msats;
  }
  o.has_provider=true;o.provider=ev.public_key;
  if !feedback {o.state=N90_DONE;o.result=ev.id;return o;}
  o.updated=ev.created_at;o.status=r.status;
  o.state=ifx r.status==N90_PAYMENT_REQUIRED then N90_PAY else ifx r.status==N90_ERROR then N90_FAILED else N90_WORKING;
  return o;
}
```

### Worked example: a question, a price and an answer

Alice (secret key 1) asks for text generation. Bob (secret key 2) is the
provider. Alice signs this kind 5050 request at T0 = 1762000000:

```jai
[["i","What is the capital of France?","text"],["param","model","LLaMA-2"],
 ["output","text/plain"],["bid","10000"],["relays","wss://relay.example.com"],
 ["p","c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"]]
```

Its id is `ad0f9c45…a3a48a` (the example prints it in full). Bob reads one
`text` input, one parameter, an output of `text/plain`, a bid of 10,000
millisats and himself as the one provider.

At T0 + 5 Bob asks for payment with kind 7000 feedback:

```jai
[["status","payment-required"],["e","ad0f9c45…a3a48a"],
 ["p","79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"],
 ["amount","5000","lnbc50n1pexample"]]
```

By hand, the invoice's human-readable part is `lnbc50n`, the text before
the last `1`. `ln` and `bc` say Lightning on Bitcoin mainnet. `50n` is 50
nano-bitcoin: 50 × 10⁻⁹ BTC × 10¹¹ millisats per bitcoin = 50 × 100 = 5,000
millisats. The invoice states 5,000 and the tag says 5,000, so they agree.
5,000 ≤ 10,000, so Alice may pay. The data part `pexample` is a
placeholder: this is not a payable invoice, and nothing here pays.

At T0 + 10 Bob publishes the kind 6050 result with content `Paris`. Its
tags are `["request", <R's full JSON, 568 bytes>]`, `["e", <R's id>,
"wss://relay.example.com"]`, R's `i` tag and `["p", <Alice>]`. Matching it
by hand:

1. The kind: 6050 = 5050 + 1000.
2. The `e` tag: R's id.
3. The `p` tag: Alice's key, R's author.
4. The `request` tag: R's JSON, whose recomputed id is R's id.

Alice's job with `wait` = 600 runs:

| Step | Time | Event | State | Owed |
| --- | --- | --- | --- | --- |
| start | T0 | | sent | |
| 1 | T0 + 5 | F, `payment-required` 5000 | payment required | 5,000 |
| 2 | T0 + 12 | S, the result | done | 5,000 |
| 3 | T0 + 13 | S again | done, `N90_CLOSED` | 5,000 |

The deadline is T0 + 600. A result taken at T0 + 601 would find the job
expired.

## Writing the tags

`n90_request_tags` writes a request's tags in the order input, parameter,
output, bid, relays, provider and `encrypted`, leaving out what is empty.
It refuses an input whose type isn't one of the four or whose `event` or
`job` data isn't an event id, a relay that isn't `ws://` or `wss://`, a
parameter without a key and a bid outside 0 to 21 million bitcoin.
`n90_reply_tags` writes a result (status -1) or feedback: for a result,
the `request` tag with the request's JSON as it was signed, the `e` tag,
the request's clear inputs, the customer and the amount. It refuses an
invoice whose amount differs from the amount tag, so the provider's side
can't write what the customer's side would refuse.

<!-- source: coax/lib/nostr/nip90.coax write -->
```jai
// ,[ after the array so far, or [ when it is still empty.
n90_next :: (out:[]u8,n:int)->int {
  if n==1 {return json_raw(out,n,"[");}
  return json_raw(out,n,",[");
}
n90_write_input :: (out:[]u8,at:int,data:string,type:int,relay:string,marker:string)->int {
  n:=n90_next(out,at);n=json_raw(out,n,"\"i\",");n=json_write_string(out,n,data);
  n=json_raw(out,n,",\"");n=json_raw(out,n,N90_INPUT_NAMES[type]);n=json_raw(out,n,"\"");
  if relay.count+marker.count>0 {n=json_raw(out,n,",");n=json_write_string(out,n,relay);}
  if marker.count>0 {n=json_raw(out,n,",");n=json_write_string(out,n,marker);}
  return json_raw(out,n,"]");
}
n90_write_amount :: (out:[]u8,at:int,amount:int,bolt11:string)->int {
  if amount<0 {return at;}
  n:=n90_next(out,at);n=json_raw(out,n,"\"amount\",\"");n=json_write_int(out,n,amount);n=json_raw(out,n,"\"");
  if bolt11.count!=0 {n=json_raw(out,n,",");n=json_write_string(out,n,bolt11);}
  return json_raw(out,n,"]");
}
// A job request's tags as a JSON array into out: its length, or minus the
// refusal (N90_INPUT, N90_PARAM, N90_BID, N90_OUTPUT). bid -1 writes none.
n90_request_tags :: (inputs:[]N90In,params:[]N90Pair,output:string,bid:int,relays:[]string,providers:[][32]u8,encrypted:bool,out:[]u8)->int {
  n:=json_raw(out,0,"[");
  for x:inputs {
    id:[32]u8;
    if x.type<0 || x.type>N90_TEXT {return -N90_INPUT;}
    if (x.type==N90_EVENT_ID || x.type==N90_JOB) && !ne_lower_hex(x.data,id[..]) {return -N90_INPUT;}
    if x.relay.count>0 && !nt_relay_ok(x.relay) {return -N90_INPUT;}
    n=n90_write_input(out,n,x.data,x.type,x.relay,x.marker);
  }
  for p:params {
    if p.key.count==0 {return -N90_PARAM;}
    n=n90_next(out,n);n=json_raw(out,n,"\"param\",");n=json_write_string(out,n,p.key);
    n=json_raw(out,n,",");n=json_write_string(out,n,p.value);n=json_raw(out,n,"]");
  }
  if output.count>0 {n=n90_next(out,n);n=json_raw(out,n,"\"output\",");n=json_write_string(out,n,output);n=json_raw(out,n,"]");}
  if bid< -1 || bid>N90_MAX_MSATS {return -N90_BID;}
  if bid>=0 {n=n90_next(out,n);n=json_raw(out,n,"\"bid\",\"");n=json_write_int(out,n,bid);n=json_raw(out,n,"\"]");}
  if relays.count>0 {
    n=n90_next(out,n);n=json_raw(out,n,"\"relays\"");
    for r:relays {n=json_raw(out,n,",");n=json_write_string(out,n,r);}
    n=json_raw(out,n,"]");
  }
  for p:providers {n=n90_next(out,n);n=json_raw(out,n,"\"p\",");n=n10_hex(out,n,p);n=json_raw(out,n,"]");}
  if encrypted {n=n90_next(out,n);n=json_raw(out,n,"\"encrypted\"]");}
  n=json_raw(out,n,"]");
  if n== -1 {return -N90_OUTPUT;}
  return n;
}
// The tags of a reply to request as a JSON array into out: a result when
// status is -1 (the request, its e tag, its clear inputs, the customer,
// the amount, and encrypted for an encrypted request), else feedback with
// that status. Its length, or minus the refusal: the request's own, or
// N90_STATUS, N90_TAG (a relay that isn't ws:// or wss://), N90_AMOUNT,
// N90_INVOICE (an invoice whose amount differs) or N90_OUTPUT.
n90_reply_tags :: (request:NostrEvent,status:int,extra:string,relay:string,amount:int,bolt11:string,out:[]u8)->int {
  q:=n90_request(request);
  if q.error!=N90_OK {return -q.error;}
  if status< -1 || status>N90_PARTIAL {return -N90_STATUS;}
  if relay.count>0 && !nt_relay_ok(relay) {return -N90_TAG;}
  if amount< -1 || amount>N90_MAX_MSATS {return -N90_AMOUNT;}
  if bolt11.count>0 && n90_invoice(bolt11)!=amount {return -N90_INVOICE;}
  n:=json_raw(out,0,"[");
  if status>=0 {
    n=n90_next(out,n);n=json_raw(out,n,"\"status\",\"");n=json_raw(out,n,N90_STATUS_NAMES[status]);n=json_raw(out,n,"\"");
    if extra.count>0 {n=json_raw(out,n,",");n=json_write_string(out,n,extra);}
    n=json_raw(out,n,"]");
  } else {
    n=n90_next(out,n);n=json_raw(out,n,"\"request\",");
    n=json_write_string(out,n,request.original);n=json_raw(out,n,"]");
  }
  n=n90_next(out,n);n=json_raw(out,n,"\"e\",");n=n10_hex(out,n,request.id);
  if relay.count!=0 {n=json_raw(out,n,",");n=json_write_string(out,n,relay);}
  n=json_raw(out,n,"]");
  if status<0 {
    for k:0..<q.ni {
      x:=q.inputs[k];
      n=n90_write_input(out,n,cast(string)x.data.b[0..<x.data.n],x.type,cast(string)x.relay.b[0..<x.relay.n],cast(string)x.marker.b[0..<x.marker.n]);
    }
  }
  n=n90_next(out,n);n=json_raw(out,n,"\"p\",");n=n10_hex(out,n,request.public_key);n=json_raw(out,n,"]");
  n=n90_write_amount(out,n,amount,bolt11);
  if status<0 && q.encrypted {n=n90_next(out,n);n=json_raw(out,n,"\"encrypted\"]");}
  n=json_raw(out,n,"]");
  if n== -1 {return -N90_OUTPUT;}
  return n;
}
```

## Job chaining

An input of type `job` names an earlier request's id: the new job runs on
that job's output. `n90_request` reads such an input and its id, like an
`event` input. Which result of the earlier job to use, and whether to wait
for it to be paid or zapped, is left to the provider by the NIP. This
chapter decides nothing there. A provider that chains jobs would look up
the earlier request's results and pass each through `n90_match` before
using its content.

## Not built

- Payment itself. Paying the invoice or zapping the reply belongs to the
  wallet (chapter 72) and zaps (chapter 71).
- Invoice decoding past the human-readable part: checksum, signature,
  payment hash, expiry.
- Cancellation by a kind 5 deletion of the request (chapter 39 reads
  deletions).
- The job kind list kept outside the NIP, and the provider's own decisions
  about price and order of work.
- NIP-89 discovery of providers is chapter 64's `n89_resolve`, with
  `k` tags such as `5050`; this chapter doesn't call it.

## The example

The example is
[nostr/examples/data-vending.coax](../examples/data-vending.coax). It signs
five events with the secret keys 1 and 2 as Alice and Bob: the request R,
the feedback F and the result S of the worked example, a second request R2
at T0 + 1 with the same tags, and a result X for R that embeds R2. It
reads them back, matches them, runs Alice's job, seals a request with
NIP-44 and opens it as Bob and as Carol (secret key 3), and then shows each
refusal, using unsigned events by Carol where no signer here would write
one.

<!-- source: nostr/examples/data-vending.coax example -->
```jai
#load "../../coax/lib/nostr/nip90.coax";
#load "../../coax/lib/nostr/nip44-key.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

E90 :: [25] string.["ok", "event", "kind", "bad input", "bad param", "bad bid", "bad tag", "bad encrypted request",
  "NIP-04 content refused", "can't decrypt", "limit", "no e tag", "e names another request", "p names another customer",
  "request tag does not match the request", "bad status", "bad amount", "invoice amount differs", "over the bid", "no bid",
  "outside the clock", "stale feedback", "another provider", "job closed", "output"];
STATE :: [7] string.["sent", "payment required", "working", "done", "failed", "too dear", "expired"];
// The public keys of the secret keys 1 to 3: published test values.
// Never sign with a real key in an example.
PEOPLE :: [3] string.["Alice", "Bob", "Carol"];
PUB :: [3] string.[
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"];
ALICE :: 0; BOB :: 1; CAROL :: 2;
T0 :: 1762000000;
WAIT :: 600;                      // Alice waits ten minutes for an answer
RELAY :: "wss://relay.example.com";
QUESTION :: "What is the capital of France?";
// An invoice's human-readable part with a placeholder data part: 50n is
// 50 nano-bitcoin. Not a payable invoice; nothing here pays.
INVOICE :: "lnbc50n1pexample";

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

// Sign an event with secret key who+1 and read it back with its ID and
// signature checked.
sign :: (signer: int, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  key: [32] u8; key[31] = cast(u8) (signer + 1);
  aux: [32] u8; work: [1] NostrSignWork; scratch: [16384] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [16384] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified && ns_equal32(e.public_key, key_of(signer));
  return e;
}
// The conversation key of secret key me+1 and peer's public key.
conversation :: (me: int, peer: int) -> [32] u8 {
  secret: [32] u8; secret[31] = cast(u8) (me + 1); pub := key_of(peer);
  work: [1] EcdhScratch; ck: [32] u8;
  #assert nip44_conversation_key_from_keys_checked(secret[..], pub[..], work[..], ck[..], false) == NIP44_OK;
  return ck;
}

// The request's tags, the replies' tags, and a hand-written result that
// embeds another request.
request_tags :: (out: [] u8) -> string {
  in := [1] N90In.[N90In.{data = QUESTION, type = N90_TEXT}];
  params := [1] N90Pair.[N90Pair.{key = "model", value = "LLaMA-2"}];
  relays := [1] string.[RELAY]; bob := [1][32] u8.[key_of(BOB)];
  n := n90_request_tags(in[..], params[..], "text/plain", 10000, relays[..], bob[..], false, out);
  #assert n > 0;
  return cast(string) out[0..<n];
}
reply_tags :: (request: NostrEvent, status: int, amount: int, bolt11: string, out: [] u8) -> string {
  n := n90_reply_tags(request, status, "", ifx status < 0 then RELAY else "", amount, bolt11, out);
  #assert n > 0;
  return cast(string) out[0..<n];
}
swapped_tags :: (request: NostrEvent, embedded: NostrEvent, out: [] u8) -> string {
  hex: [64] u8;
  n := json_raw(out, 0, "[[\"request\","); n = json_write_string(out, n, embedded.original);
  n = json_raw(out, n, "],[\"e\",\""); n = json_raw(out, n, hex32(request.id, hex[..]));
  n = json_raw(out, n, "\"],[\"p\",\""); n = json_raw(out, n, PUB[ALICE]); n = json_raw(out, n, "\"]]");
  #assert n > 0;
  return cast(string) out[0..<n];
}
// An unsigned event by Carol, parsed but not verified. Used for the
// refusals, which no signer here would write.
unsigned :: (kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"");
  n = json_raw(out, n, PUB[CAROL]); n = json_raw(out, n, "\",\"created_at\":1762000000,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, content);
  n = json_raw(out, n, ",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

show_step :: (what: string, s: N90Job) {
  print("%: %, %", what, STATE[s.state], E90[s.error]);
  if s.owed >= 0 { print(", owes % msats", s.owed); }
  print("\n");
}

// The signed events: R the request, F the feedback, S the result, R2 a
// second request and X a result for R that embeds R2.
EVENTS :: 5;
NAME :: [EVENTS] string.["R", "F", "S", "R2", "X"];

main :: () {
  ev: [EVENTS] NostrEvent; buf: [EVENTS][8192] u8; tags: [8192] u8;
  ev[0] = sign(ALICE, T0, 5050, request_tags(tags[..]), "", buf[0][..]);
  ev[1] = sign(BOB, T0 + 5, 7000, reply_tags(ev[0], N90_PAYMENT_REQUIRED, 5000, INVOICE, tags[..]), "", buf[1][..]);
  ev[2] = sign(BOB, T0 + 10, 6050, reply_tags(ev[0], -1, -1, "", tags[..]), "Paris", buf[2][..]);
  ev[3] = sign(ALICE, T0 + 1, 5050, request_tags(tags[..]), "", buf[3][..]);
  ev[4] = sign(BOB, T0 + 10, 6050, swapped_tags(ev[0], ev[3], tags[..]), "Paris", buf[4][..]);
  req := ev[0]; fb := ev[1]; res := ev[2];

  print("== 1. The events (T0 = %)\n", T0);
  for k: 0..<EVENTS { hex: [64] u8; print("%: kind %, by %, id %\n", NAME[k], ev[k].kind, PEOPLE[ifx k == 0 || k == 3 then ALICE else BOB], hex32(ev[k].id, hex[..])); }
  print("R's tags: %\n", request_tags(tags[..]));
  print("F's tags: %\n", reply_tags(req, N90_PAYMENT_REQUIRED, 5000, INVOICE, tags[..]));

  print("\n== 2. Bob reads the request\n");
  r := n90_request(req);
  print("R: %, kind %, % input, % param, bid % msats, % relay, % provider\n", E90[r.error], r.kind, r.ni, r.np, r.bid, r.relays, r.nproviders);
  print("  input: "); say(r.inputs[0].data); print(" (%)\n", N90_INPUT_NAMES[r.inputs[0].type]);
  print("  param: "); say(r.params[0].key); print(" = "); say(r.params[0].value); print("; output "); say(r.output); print("\n");

  print("\n== 3. Feedback and result, matched to the request\n");
  work: [16384] u8;
  f := n90_reply(fb);
  print("F: %, status %, amount % msats, invoice %\n", E90[f.error], N90_STATUS_NAMES[f.status], f.amount, cast(string) f.bolt11[0..<f.bolt11_n]);
  print("  invoice states % msats; match: %\n", n90_invoice(INVOICE), E90[n90_match(req, fb, work[..])]);
  s := n90_reply(res);
  print("S: %, kind % = % + 1000, request tag at %, match: %\n", E90[s.error], s.kind, req.kind, s.request_tag, E90[n90_match(req, res, work[..])]);
  p := n90_price(r.bid, f.amount, cast(string) f.bolt11[0..<f.bolt11_n]);
  print("pay % msats against a bid of %: %\n", p.msats, r.bid, E90[p.error]);

  print("\n== 4. Alice's job, deadline T0+%\n", WAIT);
  j := n90_job(req, WAIT);
  show_step("start", j);
  j = n90_step(j, req, fb, T0 + 5, work[..]); show_step("F at T0+5", j);
  j = n90_step(j, req, res, T0 + 12, work[..]); show_step("S at T0+12", j);
  j = n90_step(j, req, res, T0 + 13, work[..]); show_step("S again", j);

  print("\n== 5. A request sealed with NIP-44\n");
  plain := "[[\"i\",\"What is the capital of France?\",\"text\"],[\"param\",\"model\",\"LLaMA-2\"]]";
  ck := conversation(ALICE, BOB);
  // A fixed nonce for a reproducible example only: a client takes 32 fresh
  // random bytes for every message.
  nonce: [32] u8; for i: 0..<32 { nonce[i] = 7; }
  pt: [256] u8; for i: 0..<plain.count { pt[i] = plain[i]; }
  sealed: [1024] u8; nw: [1024] u8;
  sr := nip44_encrypt_checked(NIP44_COMPAT, ck[..], nonce[..], pt[0..<plain.count], nw[0..<nip44_encrypt_work_size(NIP44_COMPAT, plain.count)], sealed[..], false);
  #assert sr.error == NIP44_OK;
  bob := [1][32] u8.[key_of(BOB)]; n0: [1] N90In; p0: [1] N90Pair; r0: [1] string;
  et: [256] u8; en := n90_request_tags(n0[0..<0], p0[0..<0], "", 10000, r0[0..<0], bob[..], true, et[..]);
  eb: [4096] u8; enc := sign(ALICE, T0 + 20, 5050, cast(string) et[0..<en], cast(string) sealed[0..<sr.written], eb[..]);
  outer := n90_request(enc);
  print("E's tags: %\n", cast(string) et[0..<en]);
  print("E as anyone reads it: %, encrypted %, % inputs\n", E90[outer.error], outer.encrypted, outer.ni);
  bk := conversation(BOB, ALICE); dw: [4096] u8; dout: [1024] u8;
  o := n90_private(enc, outer, bk[..], dw[..], dout[..]);
  print("E opened by Bob: %, % input, % param, input ", E90[o.error], o.ni, o.np); say(o.inputs[0].data); print("\n");
  ck2 := conversation(CAROL, ALICE);
  print("E opened by Carol: %\n", E90[n90_private(enc, outer, ck2[..], dw[..], dout[..]).error]);

  print("\n== 6. Refusals\n");
  bad: [8192] u8; bt: [8192] u8;
  print("X, a result for R that embeds R2: %\n", E90[n90_match(req, ev[4], work[..])]);
  print("S checked against R2: %\n", E90[n90_match(ev[3], res, work[..])]);
  w := unsigned(6051, reply_tags(req, -1, -1, "", bt[..]), "Paris", bad[..]);
  print("S's tags under kind 6051: %\n", E90[n90_match(req, w, work[..])]);
  dear := unsigned(7000, reply_tags(req, N90_PAYMENT_REQUIRED, 20000, "", bt[..]), "", bad[..]);
  d := n90_step(n90_job(req, WAIT), req, dear, T0 + 5, work[..]); show_step("payment-required for 20000 msats", d);
  hex: [64] u8;
  m := json_raw(bt[..], 0, "[[\"status\",\"payment-required\"],[\"amount\",\"5000\",\"lnbc100n1pexample\"],[\"e\",\"");
  m = json_raw(bt[..], m, hex32(req.id, hex[..])); m = json_raw(bt[..], m, "\"],[\"p\",\""); m = json_raw(bt[..], m, PUB[ALICE]); m = json_raw(bt[..], m, "\"]]");
  mis := unsigned(7000, cast(string) bt[0..<m], "", bad[..]);
  d = n90_step(n90_job(req, WAIT), req, mis, T0 + 5, work[..]); show_step("5000 msats with a 100n invoice", d);
  d = n90_step(n90_job(req, WAIT), req, res, T0 + WAIT + 1, work[..]); show_step("S taken after the deadline", d);
  d = n90_step(n90_job(req, WAIT), req, res, T0 + 9, work[..]); show_step("S taken at T0+9, before it was made", d);
  j = n90_step(n90_job(req, WAIT), req, fb, T0 + 5, work[..]);
  carol := unsigned(7000, reply_tags(req, N90_PROCESSING, -1, "", bt[..]), "", bad[..]);
  d = n90_step(j, req, carol, T0 + 6, work[..]); show_step("Carol's feedback after Bob's", d);
  nip04 := unsigned(5050, "[[\"p\",\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\"],[\"encrypted\"]]", "zooMmtdlrlTCiIbfLqTqYrpCbTWrcVkP7RdcqCuM8TQ=?iv=obLD1OX2BxgpOktcbX6PkA==", bad[..]);
  print("encrypted request with NIP-04 content: %\n", E90[n90_request(nip04).error]);
  q := n90_request(unsigned(5050, "[[\"i\",\"/etc/passwd\",\"file\"]]", "", bad[..]));
  print("input of type file: % at tag %\n", E90[q.error], q.at);
  q = n90_request(unsigned(5050, "[[\"bid\",\"-5\"]]", "", bad[..]));
  print("bid -5: % at tag %\n", E90[q.error], q.at);
}
```

Run it with:

```shell
coax/bin/coax run nostr/examples/data-vending.coax --run-steps 250000000
```

It prints this transcript, which `nostr/fixtures/data-vending.out` holds
and the book's checks compare in debug and release. It exits with status
0.

```text
== 1. The events (T0 = 1762000000)
R: kind 5050, by Alice, id ad0f9c450d87e512250122ef83b2f6ae0e5da9658770951880e7d65156a3a48a
F: kind 7000, by Bob, id 6888825cb061c1395442807221b151a9b5619a93125190fea2a6ccf1f738d542
S: kind 6050, by Bob, id 8a46f3ab994ead2718da9209b070c2c1f3160394342af93adc255944cb5131f0
R2: kind 5050, by Alice, id bcfc219c49f14e84bd56ec8eecd146f124a44a3fcbcf74cad6af27127e35060c
X: kind 6050, by Bob, id e70694af0f43d813426ae5a4d7dfe404680015cfc4bc52def1cc102a42bf38a9
R's tags: [["i","What is the capital of France?","text"],["param","model","LLaMA-2"],["output","text/plain"],["bid","10000"],["relays","wss://relay.example.com"],["p","c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"]]
F's tags: [["status","payment-required"],["e","ad0f9c450d87e512250122ef83b2f6ae0e5da9658770951880e7d65156a3a48a"],["p","79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"],["amount","5000","lnbc50n1pexample"]]

== 2. Bob reads the request
R: ok, kind 5050, 1 input, 1 param, bid 10000 msats, 1 relay, 1 provider
  input: What is the capital of France? (text)
  param: model = LLaMA-2; output text/plain

== 3. Feedback and result, matched to the request
F: ok, status payment-required, amount 5000 msats, invoice lnbc50n1pexample
  invoice states 5000 msats; match: ok
S: ok, kind 6050 = 5050 + 1000, request tag at 0, match: ok
pay 5000 msats against a bid of 10000: ok

== 4. Alice's job, deadline T0+600
start: sent, ok
F at T0+5: payment required, ok, owes 5000 msats
S at T0+12: done, ok, owes 5000 msats
S again: done, job closed, owes 5000 msats

== 5. A request sealed with NIP-44
E's tags: [["bid","10000"],["p","c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"],["encrypted"]]
E as anyone reads it: ok, encrypted true, 0 inputs
E opened by Bob: ok, 1 input, 1 param, input What is the capital of France?
E opened by Carol: can't decrypt

== 6. Refusals
X, a result for R that embeds R2: request tag does not match the request
S checked against R2: e names another request
S's tags under kind 6051: kind
payment-required for 20000 msats: too dear, over the bid
5000 msats with a 100n invoice: sent, invoice amount differs
S taken after the deadline: expired, job closed
S taken at T0+9, before it was made: sent, outside the clock
Carol's feedback after Bob's: payment required, another provider, owes 5000 msats
encrypted request with NIP-04 content: NIP-04 content refused
input of type file: bad input at tag 0
bid -5: bad bid at tag 0
```

Section 4 is the worked example's trace. Section 5 shows that only Bob's
conversation key opens the sealed request. Section 6 shows the refusals:

- `X, a result for R that embeds R2`: the deliberately broken result. Its
  kind, `e` and `p` are right; the request it embeds is R2, whose id isn't
  R's, so it is refused with `request tag does not match the request`.
- `S checked against R2`: S's `e` tag names R, not R2.
- `S's tags under kind 6051`: 6051 isn't 5050 + 1000.
- `payment-required for 20000 msats`: over Alice's bid of 10,000, so the
  job is too dear and closed.
- `5000 msats with a 100n invoice`: the invoice states 10,000 millisats,
  the tag 5,000; refused, and the job stays open.
- `S taken after the deadline` and `S taken at T0+9, before it was made`:
  the clock.
- `Carol's feedback after Bob's`: one provider per job.
- `encrypted request with NIP-04 content`, `input of type file` and `bid
  -5`: requests that don't read.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/data-vending.coax](../examples/checks/data-vending.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it, from a Python
script written from NIP-01, NIP-90 and BIP-340:

- The ids of all five events (`hashlib.sha256` over NIP-01's
  serialization), with each event's tags built in Python from NIP-90's
  field lists. R's and R2's BIP-340 signatures come from the script's own
  integer secp256k1 signer with 32 zero bytes of aux, and the checks verify
  them. S's and X's ids cover the full JSON of R and R2 inside their
  `request` tags, so they also fix the bytes `n90_reply_tags` writes there.
- The hand values of the worked example: 50n is 5,000 millisats, the
  result kind is 6050, the job trace, and X refused as `N90_REQUEST`.
- NIP-90's encrypted parameters example (one input and six parameters),
  every status of its feedback table, and its encrypted request's `p` and
  `encrypted` tags.

<!-- source: nostr/examples/checks/data-vending.coax check -->
```jai
// From a Python script written from NIP-01, NIP-90 and BIP-340 (it shares
// no code with Coax): the public keys of secret keys 1 and 2 by integer
// secp256k1, R's and R2's BIP-340 signatures with 32 zero bytes of aux, each event's
// tags built from NIP-90's field lists, and each id as hashlib.sha256 of
// json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False). S's and X's request tags hold
// R's and R2's full event JSON, so their ids also fix those bytes.
PY_ID :: [EVENTS] string.[
  "ad0f9c450d87e512250122ef83b2f6ae0e5da9658770951880e7d65156a3a48a",
  "6888825cb061c1395442807221b151a9b5619a93125190fea2a6ccf1f738d542",
  "8a46f3ab994ead2718da9209b070c2c1f3160394342af93adc255944cb5131f0",
  "bcfc219c49f14e84bd56ec8eecd146f124a44a3fcbcf74cad6af27127e35060c",
  "e70694af0f43d813426ae5a4d7dfe404680015cfc4bc52def1cc102a42bf38a9"];
PY_R_SIG :: "32f4c1651903b4d77753ddd8fe6c9eba14ab0ef6f4d8c59e291c9e2c701ba84c9946f11faf9be8264a71dd1b26affeb0286893b38f0ea4a10ab00b18e82907f0";
PY_R2_SIG :: "1323e35789644aaeab550cebcde01c8bf33c9c36c7bfa23ec12daf3304ce3beb0f111a0834f522979e839cc34e4161b70c3a1370ceeef12f6bfb06ace9937bfa";
PY_R_LENGTH :: 568;
ZERO_SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
WHO :: [EVENTS] int.[ALICE, BOB, BOB, ALICE, BOB];
AT :: [EVENTS] int.[0, 5, 10, 1, 10];
KIND :: [EVENTS] int.[5050, 7000, 6050, 5050, 6050];
CONTENT :: [EVENTS] string.["", "", "Paris", "", "Paris"];

// An event with Python's id and the given signature, read with its id
// recomputed; the signature is not checked here.
py :: (k: int, tags: string, sig: string, out: [] u8, scratch: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, PY_ID[k]);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB[WHO[k]]);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, T0 + AT[k]);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, KIND[k]);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, CONTENT[k]);
  n = json_raw(out, n, ",\"sig\":\""); n = json_raw(out, n, sig); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}
// The five events, with R and R2 carrying signatures so that the requests
// the results embed are the signed events.
World :: struct { ev: [EVENTS] NostrEvent; buf: [EVENTS][8192] u8; scratch: [EVENTS][8192] u8; }
world :: (w: [] World) {
  tags: [8192] u8;
  w[0].ev[0] = py(0, request_tags(tags[..]), PY_R_SIG, w[0].buf[0][..], w[0].scratch[0][..]);
  w[0].ev[1] = py(1, reply_tags(w[0].ev[0], N90_PAYMENT_REQUIRED, 5000, INVOICE, tags[..]), ZERO_SIG, w[0].buf[1][..], w[0].scratch[1][..]);
  w[0].ev[2] = py(2, reply_tags(w[0].ev[0], -1, -1, "", tags[..]), ZERO_SIG, w[0].buf[2][..], w[0].scratch[2][..]);
  w[0].ev[3] = py(3, request_tags(tags[..]), PY_R2_SIG, w[0].buf[3][..], w[0].scratch[3][..]);
  w[0].ev[4] = py(4, swapped_tags(w[0].ev[0], w[0].ev[3], tags[..]), ZERO_SIG, w[0].buf[4][..], w[0].scratch[4][..]);
}

#test "the ids are Python's, and R's and R2's signatures verify" #steps 400000000 {
  w: [1] World; world(w[..]);
  for k: 0..<EVENTS { #assert w[0].ev[k].error == NE_OK && ns_equal32(w[0].ev[k].computed_id, w[0].ev[k].id); }
  r := w[0].ev[0];
  r2 := w[0].ev[3];
  #assert r.original.count == PY_R_LENGTH && nostr_verify_vartime(r.id[..], r.public_key[..], r.signature[..]);
  #assert nostr_verify_vartime(r2.id[..], r2.public_key[..], r2.signature[..]);
}
// By hand, from the chapter: 50n is 50 x 10^-9 bitcoin, 50 x 100 = 5000
// millisats; a result for kind 5050 is kind 6050; the trace is sent, then
// payment required owing 5000, then done.
#test "the hand values: the invoice, the kinds and the trace" #steps 400000000 {
  #assert n90_invoice(INVOICE) == 50 * 100 && 5050 + 1000 == 6050;
  w: [1] World; world(w[..]); work: [16384] u8;
  r := w[0].ev[0]; f := w[0].ev[1]; s := w[0].ev[2];
  #assert n90_match(r, f, work[..]) == N90_OK && n90_match(r, s, work[..]) == N90_OK && n90_reply(s).kind == r.kind + 1000;
  j := n90_job(r, WAIT); #assert j.state == N90_SENT && j.deadline == T0 + 600;
  j = n90_step(j, r, f, T0 + 5, work[..]); #assert j.state == N90_PAY && j.owed == 5000;
  j = n90_step(j, r, s, T0 + 12, work[..]); #assert j.state == N90_DONE && j.owed == 5000 && ns_equal32(j.result, s.id);
  // The chapter's refusal: X names R in its e tag but embeds R2.
  #assert n90_match(r, w[0].ev[4], work[..]) == N90_REQUEST && n90_match(w[0].ev[3], s, work[..]) == N90_E;
}
// NIP-90's own examples: the encrypted params tag list, and the five
// feedback statuses of its table.
#test "NIP-90's examples read" {
  plain := "[[\"i\",\"what is the capital of France? \",\"text\"],[\"param\",\"model\",\"LLaMA-2\"],[\"param\",\"max_tokens\",\"512\"],[\"param\",\"temperature\",\"0.5\"],[\"param\",\"top-k\",\"50\"],[\"param\",\"top-p\",\"0.7\"],[\"param\",\"frequency_penalty\",\"1\"]]";
  r := n90_request(n32_event(5050, plain));
  #assert r.error == N90_OK && r.ni == 1 && r.inputs[0].type == N90_TEXT && r.np == 6 && n32_is(r.params[2].key, "temperature") && n32_is(r.params[2].value, "0.5");
  names := [5] string.["payment-required", "processing", "error", "success", "partial"];
  for k: 0..<5 {
    t: [512] u8; n := json_raw(t[..], 0, "[[\"status\",\""); n = json_raw(t[..], n, names[k]);
    n = json_raw(t[..], n, "\"],[\"e\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"],[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\"]]");
    #assert n90_reply(n32_event(7000, cast(string) t[0..<n])).status == k;
  }
  // The pinned text's encrypted request content is NIP-04 shaped only in
  // name; this stack refuses a ?iv= payload under the encrypted tag.
  e := n90_request(n32_event(5050, "[[\"p\",\"04f74530a6ede6b24731b976b8e78fb449ea61f40ff10e3d869a3030c4edc91f\"],[\"encrypted\"]]"));
  #assert e.error == N90_OK && e.encrypted && e.providers[0][0] == 0x04;
}
```

```shell
coax/bin/coax test nostr/examples/checks/data-vending.coax --run-steps 250000000
```

`coax test` runs 491 checks, including the loaded files' own checks; all
pass.

### The library's own checks

`nip90.coax` has its own `#test` blocks: the millisat and invoice edges
(each multiplier's largest amount and one more, every refusal code), every
request field and refusal with the tag it reports, the encrypted rules and
NIP-90's encrypted parameters sealed and opened with NIP-44 (wrong key,
nested `encrypted`, inner refusals), every reply field and refusal
including a 1,024-byte invoice, matching at the kind edges 4999, 5000,
5999 and 6000, a swapped and a changed embedded request, the price rules,
the job's states and clock edges, and both writers' exact output.

```shell
coax/bin/coax test coax/lib/nostr/nip90.coax
coax/bin/coax mutate coax/lib/nostr/nip90.coax --root-only
```

On 2026-11-01, `coax test` passed all 325 checks (with the loaded files').
`coax mutate --root-only` killed all 793 mutants of `nip90.coax`, with none
surviving and none marked equivalent. The full mutation run
(`COAX_MUTATION_FULL=1`) and a mutation run of the example were not run.

### The cloud record

The plan named the record
[gptoss-cloud-nip90-2026-10-30.md](../../docs/verification/gptoss-cloud-nip90-2026-10-30.md)
to be reproduced or retired. It is retired as evidence for this chapter.
It qualifies a different code path, `psionic/lib/pylon/text.coax` and
`text-probe-policy.coax`, serving NIP-90 kinds 5050, 6050 and 7000 through
a live relay with a cloud GPU. Reproducing it needs that machine and relay,
which a book check doesn't use. Its two frozen sources still have the
SHA-256 values the record gives on 2026-11-01, so the record still
describes the code it names. This chapter's checks stand on their own.

## Summary

- `n90_request` reads a job request, and refuses an encrypted one with
  NIP-04 content; `n90_private` opens a NIP-44 one.
- `n90_reply` reads a result or feedback, and `n90_match` accepts a result
  only when its kind, `e`, `p` and embedded request all agree with the
  request.
- `n90_price` pays only an amount whose invoice states the same amount and
  that the bid covers.
- `n90_step` runs the customer's job against a caller's clock: one
  provider, no stale feedback, a deadline, and a closed job takes nothing
  more.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)

## Sources

- [NIP-90](../nips/official/90.md)
- [NIP-89](../nips/official/89.md)
- [NIP-04](../nips/official/04.md)
- [NIP-44](../nips/official/44.md)
- [BOLT 11](https://github.com/lightning/bolts/blob/master/11-payment-encoding.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
