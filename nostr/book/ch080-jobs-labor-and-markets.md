# 80. Jobs, labor and markets

OpenAgents sells work over Nostr. Three OpenAgents NIPs replace NIP-90's
general job machine ([chapter 65](ch065-data-vending-machines.md)) with
specific contracts. NIP-CJ carries a job between a caller and one named
worker: encrypted conversation, typed-decision and execution jobs. NIP-MKT
lets a buyer and a provider agree on an exact service, price and set of
deadlines, and pays a fixed Bitcoin price after acceptance. NIP-LAB is
MKT's first domain profile, agent labor: it binds the agreed order to a CJ
execution, names the worker and the reviewer, and says what counts as
delivery and acceptance. This chapter builds the decisions where a mistake
costs money: who may answer a job, which quote an order may name, when an
order is accepted, and what proof settles the payment.

Part X, Every NIP. Result: NIP-CJ response admission (the signer must be
the worker the request named); NIP-MKT negotiation (RFQ, quote, order and
confirmation) and Lightning settlement (one obligation, an attempt
persisted before the wallet, a preimage that hashes to the payment hash);
NIP-LAB's worker submission, reviewer verdict and buyer acceptance at the
exact price. Rework, disputes, resolvers, cancellation, refunds and the
record encodings are described here and not built; see
[What is not built](#what-is-not-built).

## What this chapter teaches

- What CJ, MKT and LAB each own, and why a signature, a relay `OK` or a
  status message never stands in for acceptance or payment.
- CJ: why a response is admitted only from the worker the request named,
  with one `p` naming the caller and one `e` naming the request.
- MKT negotiation: an RFQ against a fresh offering, a quote within the
  RFQ's price limit with deadlines in order, and an order (the award) only
  for a quote that exists, has not expired and has not been used.
- LAB delivery: a submission only from the worker in the terms, a verdict
  only from the reviewer, and buyer acceptance only of a passed, timely
  submission, which makes exactly the quoted price due.
- MKT settlement: one earned-price obligation per order, an invoice for
  exactly the amount due, one payment attempt at a time, and a confirmed
  payment only with a Lightning preimage that hashes to the payment hash.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [result.coax](../../coax/lib/nostr/cj/result.coax) | `cj_answer(request, response)` | Whether a verified CJ response answers a verified request |
| [order.coax](../../coax/lib/nostr/mkt/order.coax) | `mkt_rfq(m, o, signer, price_limit, response_due_at, retain_until, now)` | The buyer's request for quotes against an offering |
| [order.coax](../../coax/lib/nostr/mkt/order.coax) | `mkt_quote(m, q, t, signer, now)` | The provider's quote with its exact terms |
| [order.coax](../../coax/lib/nostr/mkt/order.coax) | `mkt_order(m, q, signer, quote_id, terms_digest, order_id, now)` | The buyer's order of one quote: the award |
| [order.coax](../../coax/lib/nostr/mkt/order.coax) | `mkt_ack(m, q, signer, confirm, now)` | The provider's confirmation or refusal |
| [order.coax](../../coax/lib/nostr/mkt/order.coax) | `mkt_submit(m, q, signer, now)` | LAB submission zero from the worker |
| [order.coax](../../coax/lib/nostr/mkt/order.coax) | `mkt_verify(m, q, signer, verdict)` | The reviewer's verdict |
| [order.coax](../../coax/lib/nostr/mkt/order.coax) | `mkt_accept(m, q, signer, received_at, now)` | The buyer's acceptance with its delivery receipt |
| [order.coax](../../coax/lib/nostr/mkt/order.coax) | `mkt_instruct(m, q, signer, obligation, amount_msat, invoice, payment_hash, expires_at, now)` | The provider's payment instruction |
| [order.coax](../../coax/lib/nostr/mkt/order.coax) | `mkt_attempt(m, q, signer, n, fee_limit_msat, now)` | The buyer's payment attempt, persisted before the wallet call |
| [order.coax](../../coax/lib/nostr/mkt/order.coax) | `mkt_result(m, signer, p)` | The buyer's wallet observation of that attempt |

Both files are new with this chapter, and both are `#authority` files:
`result.coax` decides who may answer a job and `order.coax` decides money.
Their checks came first, and `coax mutate --root-only` kills every mutant.
`result.coax` reuses `tags.coax`'s `nostr_tag` to read `p` and `e` tags.
`order.coax` reuses two procedures from chapter 86's
[settle.coax](../../coax/lib/x402/settle.coax): `x402_bolt11_msat`, which
reads the amount in an invoice's human-readable part, and
`x402_preimage_matches`, which compares SHA-256 of a preimage with a payment
hash. The checks' helpers live in
[mkt_order_fixtures.coax](../../coax/tests/nostr/mkt_order_fixtures.coax),
so that mutation changes the library and not the checks. The CJ events in
the checks and the example come from
[cj_result_vectors.py](../../coax/tests/nostr/cj_result_vectors.py), which
signs and encrypts them without any Coax code.

## Three NIPs, three owners

The three NIPs split one purchase into separate facts. Each fact has its own
record and its own signer, and none substitutes for another.

| NIP | Owns | Signed by |
| --- | --- | --- |
| CJ | The job: one request to one worker, its admission, progress and result | The caller (request) and the worker (responses) |
| MKT | The agreement and the money: offering, RFQ, quote, order, confirmation, payment instruction, attempts and results | The buyer and the provider; the payee and the payer |
| LAB | The deliverable: linkage of the order to a CJ execution, submissions, verification, review and acceptance | The buyer, the worker and the reviewer |

The **buyer** and the **provider** are the Nostr keys that sign their
commercial records. The **worker** is the execution key that the terms
name; it may differ from the provider. The **reviewer** is a third key,
distinct from the provider and the worker, that signs verification. A
wallet key, a relay identity and an agent's owner are all distinct from
these roles, and none of them grants a role.

MKT's private records travel in kind 3188 envelopes (NIP-44 v2 to one
recipient) and add no public routing tags: prices, invoices and order IDs
never appear in a tag. The only new public kinds are the offering (3192,
regular) and its current head (30192, addressable), both draft
assignments. CJ's kinds are ephemeral: relays fan them out and do not keep
them, so a worker's own durable state, not a relay, records a job.

The NIPs keep these facts apart on purpose:

- Execution completion is not delivery. A CJ result says the worker ran.
- Delivery is not verification. A submission says the worker handed over
  bytes.
- Verification is not acceptance. A reviewer's `passed` says the agreed
  checker passed; the buyer still decides.
- Acceptance is not payment. It makes the price due; the buyer's wallet
  still has to pay, and the provider bears the risk that it doesn't.
- A payment receipt is not proof of quality, and a NIP-57 zap receipt is
  not proof of payment: it is the recipient wallet's own claim.

## CJ: who may answer a job

Every CJ request names exactly one worker in a `p` tag. Every response has
exactly one `p` naming the caller and one `e` naming the exact request it
answers, and its content is NIP-44 v2 encrypted to the caller. The NIP
lists the order of checks before any payload is accepted: the event's id
and signature, the kind, the signer, the recipient and the request binding.

Three families share the transport. Each has its own request, result and
feedback kind, and a handler must refuse another family's kind rather than
reinterpret it:

| Family | Request | Result | Feedback |
| --- | --- | --- | --- |
| Conversation | 25900 | 26900 | 27000 |
| Decision | 25910 | 26910 | 27010 |
| Execution | 25920 | 26920 | 27020 |

<!-- source: coax/lib/nostr/cj/result.coax codes -->
```jai
CJ_OK :: 0;        // the response answers the request
CJ_EVENT :: 1;     // an event that did not verify (id or signature)
CJ_KIND :: 2;      // not a request kind, or not a result or feedback kind
CJ_FAMILY :: 3;    // the response belongs to another family than the request
CJ_TAGS :: 4;      // not exactly one well-formed p (request), or p and e (response)
CJ_SIGNER :: 5;    // signed by a key other than the worker the request named
CJ_CALLER :: 6;    // the response's p is not the request's author
CJ_REQUEST :: 7;   // the response's e is not the request's id

CJ_CONVERSATION :: 0; CJ_DECISION :: 1; CJ_EXECUTION :: 2;   // families
CJ_REQUEST_KIND :: 25900; CJ_RESULT_KIND :: 26900; CJ_FEEDBACK_KIND :: 27000;
```

`cj_family` maps a kind to its family for one role:

<!-- source: coax/lib/nostr/cj/result.coax family -->
```jai
// The family of a kind of the given role (request 25900, result 26900 or
// feedback 27000 as base): 0 conversation, 1 decision, 2 execution, -1 none.
cj_family :: (kind:int,base:int)->int {
  d:=kind-base;
  if d==0 || d==10 || d==20 {return d/10;}
  return -1;
}
```

`cj_tags` counts the `p` and `e` tags and keeps the last of each. A
malformed one, such as a `p` that is not 64 lowercase hex digits, makes
the whole reading fail:

<!-- source: coax/lib/nostr/cj/result.coax tags -->
```jai
// The event's p and e tags: np and ne count them, p and e hold the last of
// each, and a malformed one makes ok false. Other tags are not read.
CjTags :: struct {ok:bool=true;np:int;ne:int;p:[32]u8;e:[32]u8;}
cj_tags :: (ev:NostrEvent)->CjTags {
  r:CjTags;i:=0;
  while true {
    t:=nostr_tag(ev,i);
    if t.error==NT_INDEX {break;}
    if t.type==NT_P || t.type==NT_E {
      if t.error!=NT_OK {r.ok=false;}
      if t.type==NT_P {r.np+=1;r.p=t.pubkey;} else {r.ne+=1;r.e=t.id;}
    }
    i+=1;
  }
  return r;
}
cj_same :: (a:[32]u8,b:[32]u8)->bool {for i:0..<32 {if a[i]!=b[i] {return false;}}return true;}
```

`cj_answer` applies the checks in a fixed order. The signer check comes
before the caller and request checks, so a response from another key
refuses as `CJ_SIGNER` whatever its tags say:

<!-- source: coax/lib/nostr/cj/result.coax answer -->
```jai
// Whether response answers request: CJ_OK or the first refusal, in this order.
cj_answer :: (request:NostrEvent,response:NostrEvent)->int {
  if request.error!=NE_OK || !request.verified || response.error!=NE_OK || !response.verified {return CJ_EVENT;}
  family:=cj_family(request.kind,CJ_REQUEST_KIND);
  answer:=cj_family(response.kind,CJ_RESULT_KIND);
  if answer<0 {answer=cj_family(response.kind,CJ_FEEDBACK_KIND);}
  if family<0 || answer<0 {return CJ_KIND;}
  if family!=answer {return CJ_FAMILY;}
  q:=cj_tags(request);
  if !q.ok || q.np!=1 {return CJ_TAGS;}
  r:=cj_tags(response);
  if !r.ok || r.np!=1 || r.ne!=1 {return CJ_TAGS;}
  if !cj_same(response.public_key,q.p) {return CJ_SIGNER;}
  if !cj_same(r.p,request.public_key) {return CJ_CALLER;}
  if !cj_same(r.e,request.id) {return CJ_REQUEST;}
  return CJ_OK;
}
```

The check uses the verified signer of the event, `response.public_key`,
not a key the payload claims. NIP-42 relay authentication does not change
this: it authenticates a connection, not a forwarded event. `cj_answer`
does not open the encrypted content. Reading the body's request, attempt
and run identities, freshness windows and deduplication by event id are
the caller's.

## MKT: the negotiation

One MKT negotiation has one buyer, one provider and a buyer-generated
market ID. `order.coax` keeps it in one record. Each step takes the
verified signer of the record's envelope and the host's own receipt time,
and changes the record only when it returns `MKT_OK`.

<!-- source: coax/lib/nostr/mkt/order.coax codes -->
```jai
MKT_OK :: 0;
MKT_MALFORMED :: 1;            // bad deadlines, amounts or fields
MKT_NOT_ADMITTED :: 2;         // the signer does not hold this record's role
MKT_IDENTITY :: 3;             // identity_mismatch: another provider, terms digest or quote
MKT_STALE :: 4;                // an expired offering, a late quote, an expired invoice
MKT_CONFLICT :: 5;             // the step is not open in this state
MKT_LIMIT :: 6;                // no room for another quote
MKT_UNSUPPORTED :: 7;          // unsupported_profile: payment profile, network or rail
MKT_QUOTE_EXPIRED :: 8;
MKT_QUOTE_CONSUMED :: 9;
MKT_CONFIRMATION_EXPIRED :: 10;
MKT_PRICE_EXCEEDED :: 11;
MKT_UNAVAILABLE :: 12;         // content_unavailable: no such quote
MKT_IDEMPOTENCY :: 13;         // idempotency_conflict: same ID, other bytes
MKT_NOT_ACCEPTED :: 14;        // no valid acceptance (or instruction) to pay against
MKT_AMOUNT :: 15;              // not exactly the accepted price
MKT_PROOF :: 16;               // the preimage does not hash to the payment hash
MKT_PAID :: 17;                // the obligation is already paid
MKT_PENDING :: 18;             // an earlier attempt is pending or unknown
MKT_LATE :: 19;                // delivery received after delivery_due_at
MKT_UNVERIFIED :: 20;          // the reviewer's verdict is not passed
```

<!-- source: coax/lib/nostr/mkt/order.coax states -->
```jai
MKT_OPEN :: 0; MKT_ASKED :: 1; MKT_ORDERED :: 2; MKT_CONFIRMED :: 3;   // negotiation
MKT_FREE :: 1; MKT_LIGHTNING :: 2;               // payment profiles, as bits
MKT_BITCOIN :: 1; MKT_TESTNET :: 2; MKT_REGTEST :: 4;   // networks, as bits; 0 is null
MKT_PASSED :: 0; MKT_FAILED :: 1; MKT_UNVERIFIABLE :: 2; MKT_NOT_RUN :: 3;  // verdicts
MKT_NONE :: 0; MKT_PAY_PENDING :: 1; MKT_PAY_UNKNOWN :: 2; MKT_PAY_FAILED :: 3; MKT_PAY_CONFIRMED :: 4;
MKT_RAIL_LIGHTNING :: 0; MKT_RAIL_ONCHAIN :: 1; MKT_RAIL_ZAP :: 2;   // any other value is another rail
```

<!-- source: coax/lib/nostr/mkt/order.coax record -->
```jai
MktOffering :: struct {provider:string;valid_until:int;profiles:int;networks:int;}
MktTerms :: struct {
  quote_id:string;digest:string;worker:string;reviewer:string;
  price_msat:int;fee_limit_msat:int;profile:int;network:int;
  quote_expires_at:int;order_confirm_by:int;delivery_due_at:int;
  review_due_at:int;payment_due_at:int;retain_until:int;
  consumed:bool;                     // a buyer's order has named it
}
MktMarket :: struct {
  market:string;buyer:string;provider:string;offering:MktOffering;
  stage:int;price_limit:int;response_due_at:int;
  quotes:int;order:int=-1;order_id:string;     // order indexes the quotes
  submitted:bool;submitted_at:int;verdict:int=-1;
  accepted:bool;amount_due:int=-1;
  obligation:string;amount_msat:int;payment_hash:string;expires_at:int;
  attempts:int;payment:int;paid_msat:int;
}
MktProof :: struct {rail:int;state:int;amount_msat:int;preimage:string;}
```

A quote's terms are MKT's `openagents.market-terms.v1` artifact. Prices
and fees are nonnegative integer millisatoshis, never fiat amounts, and its
deadlines must satisfy:

```jai
quote_expires_at <= order_confirm_by < delivery_due_at < review_due_at < payment_due_at < retain_until
```

Two payment profiles exist. `free-v1` has price 0, fee limit 0 and a null
network, and never produces a payment instruction.
`lightning-bolt11-fixed-postacceptance-v1` has a positive price, a network
of `bitcoin`, `testnet` or `regtest`, and pays after acceptance. The NIP's
list is closed: a quote with any other profile refuses as
`unsupported_profile`. The NIP says not to add another rail, x402
included, without specifying its obligation, deadline, cancellation and
nondelivery rules.

The four negotiation steps:

1. **RFQ** (buyer). The offering must come from this negotiation's provider
   and be unexpired. The response deadline is in the future and the
   retention time after it.
2. **Quote** (provider). Sent by the response deadline, with deadlines in
   order, a reviewer distinct from the provider and the worker, a payment
   profile and network the offering supports, and a price within the RFQ's
   limit (`price_exceeded` otherwise). A quote ID sent again with the same
   terms digest is a retransmission; with another digest it is an
   `idempotency_conflict`.
3. **Order** (buyer), the award. It names one quote and that quote's exact
   terms digest. The quote must exist (`content_unavailable`), match the
   digest (`identity_mismatch`), not already be used by an order
   (`quote_consumed`) and not have expired (`quote_expired`). A quote can be
   used by at most one order.
4. **Acknowledgment** (provider). A confirmation must arrive by
   `order_confirm_by` (`confirmation_expired`). It excludes every other
   order in the negotiation. A refusal opens the negotiation for another
   order and leaves the refused quote used.

<!-- source: coax/lib/nostr/mkt/order.coax negotiate -->
```jai
mkt_bit :: (x:int)->bool {return x==1 || x==2 || x==4;}

mkt_rfq :: (m:[]MktMarket,o:MktOffering,signer:string,price_limit:int,response_due_at:int,retain_until:int,now:int)->int {
  if signer!=m[0].buyer {return MKT_NOT_ADMITTED;}
  if m[0].stage!=MKT_OPEN {return MKT_CONFLICT;}
  if o.provider!=m[0].provider {return MKT_IDENTITY;}
  if now>=o.valid_until {return MKT_STALE;}
  if price_limit<0 || response_due_at<=now || retain_until<=response_due_at {return MKT_MALFORMED;}
  m[0].offering=o;m[0].price_limit=price_limit;m[0].response_due_at=response_due_at;m[0].stage=MKT_ASKED;
  return MKT_OK;
}

mkt_quote :: (m:[]MktMarket,q:[]MktTerms,t:MktTerms,signer:string,now:int)->int {
  if signer!=m[0].provider {return MKT_NOT_ADMITTED;}
  if m[0].stage!=MKT_ASKED {return MKT_CONFLICT;}
  if now>m[0].response_due_at {return MKT_STALE;}
  for i:0..<m[0].quotes {
    if q[i].quote_id==t.quote_id {return ifx q[i].digest==t.digest then MKT_OK else MKT_IDEMPOTENCY;}
  }
  if t.quote_expires_at<=now || t.order_confirm_by<t.quote_expires_at || t.delivery_due_at<=t.order_confirm_by
     || t.review_due_at<=t.delivery_due_at || t.payment_due_at<=t.review_due_at || t.retain_until<=t.payment_due_at {return MKT_MALFORMED;}
  if t.reviewer==m[0].provider || t.reviewer==t.worker {return MKT_MALFORMED;}
  if (t.profile!=MKT_FREE && t.profile!=MKT_LIGHTNING) || (m[0].offering.profiles&t.profile)==0 {return MKT_UNSUPPORTED;}
  if t.profile==MKT_FREE {
    if t.price_msat!=0 || t.fee_limit_msat!=0 || t.network!=0 {return MKT_MALFORMED;}
  } else {
    if t.price_msat<=0 || t.fee_limit_msat<0 {return MKT_MALFORMED;}
    if !mkt_bit(t.network) || (m[0].offering.networks&t.network)==0 {return MKT_UNSUPPORTED;}
  }
  if t.price_msat>m[0].price_limit {return MKT_PRICE_EXCEEDED;}
  if m[0].quotes>=q.count {return MKT_LIMIT;}
  q[m[0].quotes]=t;q[m[0].quotes].consumed=false;m[0].quotes+=1;
  return MKT_OK;
}

// The award: the buyer orders exactly one quote's terms.
mkt_order :: (m:[]MktMarket,q:[]MktTerms,signer:string,quote_id:string,terms_digest:string,order_id:string,now:int)->int {
  if signer!=m[0].buyer {return MKT_NOT_ADMITTED;}
  if m[0].stage!=MKT_ASKED {return MKT_CONFLICT;}
  for i:0..<m[0].quotes {
    if q[i].quote_id==quote_id {
      if q[i].digest!=terms_digest {return MKT_IDENTITY;}
      if q[i].consumed {return MKT_QUOTE_CONSUMED;}
      if now>q[i].quote_expires_at {return MKT_QUOTE_EXPIRED;}
      q[i].consumed=true;m[0].order=i;m[0].order_id=order_id;m[0].stage=MKT_ORDERED;
      return MKT_OK;
    }
  }
  return MKT_UNAVAILABLE;
}

// The provider confirms (or refuses) the order. A refusal frees the
// negotiation for another order; the refused quote stays consumed.
mkt_ack :: (m:[]MktMarket,q:[]MktTerms,signer:string,confirm:bool,now:int)->int {
  if signer!=m[0].provider {return MKT_NOT_ADMITTED;}
  if m[0].stage!=MKT_ORDERED {return MKT_CONFLICT;}
  if !confirm {m[0].stage=MKT_ASKED;m[0].order=-1;m[0].order_id="";return MKT_OK;}
  if now>q[m[0].order].order_confirm_by {return MKT_CONFIRMATION_EXPIRED;}
  m[0].stage=MKT_CONFIRMED;
  return MKT_OK;
}
```

All of a step's checks run before its first write, so a refusal changes
nothing. `mkt_quote` copies the terms into the next free slot and clears
`consumed`, so a quote cannot arrive already marked as used.

## LAB: delivery, verification and acceptance

LAB names the worker and the reviewer in the quoted terms. The worker
signs submissions, the reviewer signs verification, and the buyer signs the
review that accepts. A submission from any other key refuses as
`not_admitted`, the provider's included: the provider is a commercial role,
and the worker is the execution role.

<!-- source: coax/lib/nostr/mkt/order.coax deliver -->
```jai
// Submission zero from the worker the confirmed terms name; any other key
// refuses, the provider's included.
mkt_submit :: (m:[]MktMarket,q:[]MktTerms,signer:string,now:int)->int {
  if m[0].stage!=MKT_CONFIRMED {return MKT_CONFLICT;}
  if signer!=q[m[0].order].worker {return MKT_NOT_ADMITTED;}
  if m[0].submitted {return MKT_CONFLICT;}
  m[0].submitted=true;m[0].submitted_at=now;
  return MKT_OK;
}

mkt_verify :: (m:[]MktMarket,q:[]MktTerms,signer:string,verdict:int)->int {
  if m[0].stage!=MKT_CONFIRMED || !m[0].submitted {return MKT_CONFLICT;}
  if signer!=q[m[0].order].reviewer {return MKT_NOT_ADMITTED;}
  if verdict<MKT_PASSED || verdict>MKT_NOT_RUN {return MKT_MALFORMED;}
  if m[0].verdict!=-1 {return MKT_CONFLICT;}
  m[0].verdict=verdict;
  return MKT_OK;
}

// The buyer's acceptance with its delivery receipt: received_at is when it
// fetched the submission. Accepted means the amount due is the price.
mkt_accept :: (m:[]MktMarket,q:[]MktTerms,signer:string,received_at:int,now:int)->int {
  if signer!=m[0].buyer {return MKT_NOT_ADMITTED;}
  if m[0].stage!=MKT_CONFIRMED || !m[0].submitted || m[0].accepted {return MKT_CONFLICT;}
  if received_at<m[0].submitted_at || received_at>now {return MKT_MALFORMED;}
  if m[0].verdict!=MKT_PASSED {return MKT_UNVERIFIED;}
  if received_at>q[m[0].order].delivery_due_at {return MKT_LATE;}
  m[0].accepted=true;m[0].amount_due=q[m[0].order].price_msat;
  return MKT_OK;
}
```

The buyer's acceptance carries its delivery receipt: `received_at` is when
it fetched and verified the submission. LAB's `buyer_acceptance` form
requires three things:

- A passed verification. `failed`, `unverifiable` and `not_run` refuse as
  `MKT_UNVERIFIED`.
- A receipt no later than `delivery_due_at`. A later one refuses as
  `MKT_LATE`; acceptance cannot silently waive lateness.
- A receipt no earlier than the submission and no later than now.

The accepted outcome's `amount_due_msat` is exactly the quoted
`price_msat`. LAB allows only the full price or zero, and zero needs a
supported rejection, which this chapter does not build.

## MKT: settlement

The payee (the provider, for an earned price) issues a payment instruction
with a BOLT11 invoice. The payer (the buyer) records each attempt before it
calls its wallet, then records what the wallet observed.

<!-- source: coax/lib/nostr/mkt/order.coax settle -->
```jai
// The provider's payment instruction for the earned price.
mkt_instruct :: (m:[]MktMarket,q:[]MktTerms,signer:string,obligation:string,amount_msat:int,invoice:string,payment_hash:string,expires_at:int,now:int)->int {
  if signer!=m[0].provider {return MKT_NOT_ADMITTED;}
  if m[0].stage!=MKT_CONFIRMED {return MKT_CONFLICT;}
  t:=q[m[0].order];
  if t.profile!=MKT_LIGHTNING || t.network!=MKT_BITCOIN {return MKT_UNSUPPORTED;}
  if !m[0].accepted {return MKT_NOT_ACCEPTED;}
  if m[0].obligation!="" {return MKT_CONFLICT;}   // one earned-price obligation per order
  if amount_msat!=m[0].amount_due || x402_bolt11_msat(invoice)!=amount_msat {return MKT_AMOUNT;}
  if !x402_is_hex64(payment_hash) || obligation=="" {return MKT_MALFORMED;}
  if expires_at<=now {return MKT_STALE;}
  m[0].obligation=obligation;m[0].amount_msat=amount_msat;m[0].payment_hash=payment_hash;m[0].expires_at=expires_at;
  return MKT_OK;
}

// The buyer persists attempt number n before it calls its wallet.
mkt_attempt :: (m:[]MktMarket,q:[]MktTerms,signer:string,n:int,fee_limit_msat:int,now:int)->int {
  if signer!=m[0].buyer {return MKT_NOT_ADMITTED;}
  if m[0].obligation=="" {return MKT_NOT_ACCEPTED;}
  if m[0].payment==MKT_PAY_CONFIRMED {return MKT_PAID;}
  if m[0].payment==MKT_PAY_PENDING || m[0].payment==MKT_PAY_UNKNOWN {return MKT_PENDING;}
  if n!=m[0].attempts+1 || fee_limit_msat!=q[m[0].order].fee_limit_msat {return MKT_MALFORMED;}
  if now>=m[0].expires_at {return MKT_STALE;}
  m[0].attempts=n;m[0].payment=MKT_PAY_PENDING;
  return MKT_OK;
}

// The buyer's wallet observation of the pending attempt. Only a Lightning
// proof settles: the exact amount and a preimage of the payment hash.
mkt_result :: (m:[]MktMarket,signer:string,p:MktProof)->int {
  if signer!=m[0].buyer {return MKT_NOT_ADMITTED;}
  if m[0].payment==MKT_PAY_CONFIRMED {return MKT_PAID;}
  if m[0].payment!=MKT_PAY_PENDING && m[0].payment!=MKT_PAY_UNKNOWN {return MKT_CONFLICT;}
  if p.rail!=MKT_RAIL_LIGHTNING {return MKT_UNSUPPORTED;}
  if p.state==MKT_PAY_UNKNOWN || p.state==MKT_PAY_FAILED {m[0].payment=p.state;return MKT_OK;}
  if p.state!=MKT_PAY_CONFIRMED {return MKT_MALFORMED;}
  if p.amount_msat!=m[0].amount_msat {return MKT_AMOUNT;}
  if !x402_preimage_matches(p.preimage,m[0].payment_hash) {return MKT_PROOF;}
  m[0].payment=MKT_PAY_CONFIRMED;m[0].paid_msat=p.amount_msat;
  return MKT_OK;
}
```

The money rules, each with the step that enforces it:

- Payment only after acceptance: `mkt_instruct` refuses
  `MKT_NOT_ACCEPTED` before acceptance, and `mkt_attempt` refuses it
  before an instruction exists.
- Never above (or below) the accepted price: the instruction's
  `amount_msat` must equal `amount_due`, and so must the invoice's own
  amount. A confirmed result must state the same amount.
- One earned-price obligation per order: a second instruction refuses as
  `conflict`, whatever its obligation ID.
- One attempt at a time: while an attempt is pending or `unknown`, another
  refuses as `MKT_PENDING`. An unknown result keeps the reservation; only a
  `failed` result, which needs evidence that the payment cannot still
  settle, lets attempt `n + 1` start. The fee limit must equal the terms'.
- No double payment: after a confirmed result, an attempt or a result
  refuses as `MKT_PAID`.
- Only a Lightning proof settles: the result's rail must be Lightning, and
  the preimage must hash, by SHA-256, to the payment hash.

The NIP defines no on-chain profile. PLAN.md's "Done means" names
Lightning or on-chain proof, but the pinned NIP-MKT closes its list of
payment profiles to `free-v1` and the Lightning profile, so `mkt_result`
refuses an on-chain proof as `unsupported_profile`, together with a NIP-57
zap receipt and any other rail. An on-chain profile needs its own NIP text
first.

## Worked example: one order, by hand

The clock starts at T0 = 1790000000. The buyer is key 1, the provider key
2, the worker key 3 and the reviewer key 4 (the x-only public keys of
those scalars). Key 7 holds no role.

The offering is valid until T0 + 86400 and supports the Lightning profile on
`bitcoin`.

| Time | Signer | Record | Check | Result |
| --- | --- | --- | --- | --- |
| T0 | buyer | RFQ, limit 60,000 msat, answers due T0 + 300 | offering fresh: T0 < T0 + 86400 | accepted |
| T0 + 100 | provider | quote A, 50,000 msat, fee limit 1,000 msat | 600 <= 900 < 3600 < 7200 < 86400 < 2592000 (offsets); 50,000 <= 60,000 | accepted |
| T0 + 120 | provider | quote B, 70,000 msat | 70,000 > 60,000 | `price_exceeded` |
| T0 + 150 | key 7 | quote C | key 7 is not the provider | `not_admitted` |
| T0 + 300 | buyer | order of quote Z | no quote Z | `content_unavailable` |
| T0 + 300 | buyer | order of A naming terms `bbbb...` | A's digest is `aaaa...` | `identity_mismatch` |
| T0 + 300 | buyer | order of A naming `aaaa...` | 300 <= 600, A unused | accepted |
| T0 + 301 | buyer | order of A again | the negotiation is ordered | `conflict` |
| T0 + 350 | provider | confirmation | 350 <= 900 | accepted |

The buyer then sends the CJ execute request, kind 25920, id `2be5278b...`,
with `p` = worker key 3 (`f9308a01...`). Six answers arrive:

| Answer | Signer | Tags | Result |
| --- | --- | --- | --- |
| 27020 acceptance | `f9308a01...` = the request's `p` | `p` = buyer, `e` = `2be5278b...` | answers |
| 26920 result | `f9308a01...` | the same | answers |
| 26920 result | `5cbdf064...` (key 7) ≠ `f9308a01...` | the same | `CJ_SIGNER` |
| 26900 result | `f9308a01...` | the same | `CJ_FAMILY`: conversation, not execution |
| 26920 result | `f9308a01...` | `e` = `63fadca9...` | `CJ_REQUEST` |
| 26920 result | `f9308a01...` | `p` = key 7 | `CJ_CALLER` |

Delivery, acceptance and payment:

| Time | Signer | Record | Check | Result |
| --- | --- | --- | --- | --- |
| T0 + 950 | key 7, then provider | submission | neither is worker key 3 | `not_admitted` twice |
| T0 + 950 | worker | submission 0 | worker key 3 | accepted |
| T0 + 1000 | buyer | acceptance | no verdict yet | `MKT_UNVERIFIED` |
| T0 + 1200 | reviewer | verdict `passed` | reviewer key 4 | accepted |
| T0 + 1300 | buyer | acceptance, received at T0 + 1000 | 950 <= 1000 <= 1300; 1000 <= 3600; passed | accepted, 50,000 msat due |
| T0 + 1350 | provider | instruction, 60,000 msat | 60,000 ≠ 50,000 | `MKT_AMOUNT` |
| T0 + 1400 | provider | instruction, 50,000 msat, invoice `lnbc500n1...` | see below | accepted |
| T0 + 1410 | provider | a second obligation | one per order | `conflict` |
| T0 + 1500 | buyer | attempt 1, fee limit 1,000 msat | the terms' fee limit; 1500 < 5000 | accepted |
| T0 + 1505 | buyer | attempt 2 | attempt 1 is pending | `MKT_PENDING` |
| T0 + 1510 | buyer | zap receipt, then on-chain transaction | not Lightning | `unsupported_profile` twice |
| T0 + 1510 | buyer | preimage `00...00` | SHA-256 is `66687aad...` ≠ `ae216c2e...` | `MKT_PROOF` |
| T0 + 1510 | buyer | 50,000 msat, preimage `0102...20` | SHA-256 is `ae216c2e...` | accepted, paid |
| T0 + 1600 | buyer | attempt 2 | already paid | `MKT_PAID` |

The invoice's amount, by hand: the human-readable part `lnbc500n` is the
prefix `lnbc` (Bitcoin mainnet), the amount 500 and the multiplier `n`
(10^-9). One bitcoin is 10^11 msat, so 500 × 10^-9 × 10^11 = 500 × 100 =
50,000 msat, the amount due. The buyer's most expensive outcome is the
price plus the fee limit: 50,000 + 1,000 = 51,000 msat.

## What is not built

- **CJ bodies.** `cj_answer` reads tags only. Decrypting the NIP-44
  content, the closed conversation, decision and execution body schemas,
  request freshness, deduplication, controls (status, replay, cancel) and
  caller-paid model calls are not built.
- **Record encodings.** The JCS JSON of offerings, heads, MKT records,
  terms, LAB terms, submissions, verifications, reviews and acceptances,
  their schema checks and their ArtifactRef digests are not read or written
  here. The steps take typed fields and the verified signer. The 3188
  envelope itself is [chapter 75](ch075-hosts-and-reachability.md)'s.
- **Record chains.** MKT's per-issuer `seq` and `prev`, retransmission by
  digest across records, and equivocation (different bytes for one
  sequence number) are not tracked.
- **LAB linkage and rework.** The buyer-signed linkage of an order to a CJ
  execute event, rework requests and numbered replacement submissions,
  rejections, disputes, the resolver and its resolutions, cancellation, an
  unavailable buyer, rights and supersession are not built. One
  submission, number zero, is accepted.
- **Settlement beyond one payment.** Decoding the BOLT11 invoice (its
  signature, tagged fields, payee binding, network and payment hash),
  replacement invoices, the payee's `payment_received`, refunds and their
  reservations, close and close_ack are not built. The instruction's
  payment hash and expiry are taken as decoded by the payment host, and
  only `bitcoin` (mainnet) invoices are read.
- **Rails.** No on-chain profile exists in the pinned NIP; Cashu, NIP-60
  and NIP-61 are not Bitcoin settlement for this profile and refuse.
- **Key agency.** NIP-KEYS terms as an ongoing MKT service are
  [chapter 104](ch104-agent-treasuries-and-a-network-of-keys.md)'s.
- **NIP examples.** NIP-CJ, NIP-LAB and NIP-MKT publish no wire events
  with real ids (LAB's worked coding example is prose), so there are no
  published ids to compare or to find wrong. The independent check is the
  Python signer's ids below.

## The example

[nostr/examples/labor-market.coax](../examples/labor-market.coax) runs the
worked example at the fixed clock. It verifies every CJ event's id and
BIP-340 signature at run time with `nostr_event_verify_vartime`, then
changes one byte of worker 3's encrypted result to show the event
refusing before `cj_answer` reads any tag.

<!-- source: nostr/examples/labor-market.coax example -->
```jai
// Chapter 80: one labor order from request for quote to paid, at a fixed
// clock, amounts in millisatoshis. The buyer is key 1, the provider key 2,
// the worker key 3, the reviewer key 4; key 7 belongs to none of them. Keys
// are the x-only public keys of those scalars: TEST ONLY. The CJ events were
// signed and encrypted by coax/tests/nostr/cj_result_vectors.py.
#load "../../coax/lib/nostr/cj/result.coax";
#load "../../coax/lib/nostr/mkt/order.coax";
#import "Basic";

T0 :: 1790000000;
BUYER :: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
PROVIDER :: "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
WORKER :: "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
REVIEWER :: "e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13";
KEY_7 :: "5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc";
TERMS_A :: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
TERMS_B :: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
// A 50,000 msat (500n) invoice whose payment hash is SHA-256 of the
// preimage 01 02 ... 20: x402's fixture (coax/lib/x402/settle.coax).
INVOICE :: "lnbc500n1p4wfve5pp54cskcth4y3ar0qkpxhh6y7drunxuvyy5yu8462l9333qfdaxztys";
HASH :: "ae216c2ef5247a3782c135efa279a3e4cdc61094270f5d2be58c6204b7a612c9";
PREIMAGE :: "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
ZERO :: "0000000000000000000000000000000000000000000000000000000000000000";

MKT_WORDS :: [21]string.["accepted","malformed","not_admitted","identity_mismatch","stale","conflict",
  "limit_exceeded","unsupported_profile","quote_expired","quote_consumed","confirmation_expired",
  "price_exceeded","content_unavailable","idempotency_conflict","not accepted: nothing to pay against",
  "amount: not the accepted price","no proof: the preimage does not hash to the payment hash",
  "already paid","pending: an earlier attempt is unresolved","late","not passed by the reviewer"];
CJ_WORDS :: [8]string.["answers the request","refused: the event does not verify","refused: kind",
  "refused: another family","refused: tags","refused: not the worker the request named",
  "refused: p is not the caller","refused: e is not the request"];

say :: (at:int,what:string,r:int) {print("T0 + %: %: %\n",at,what,MKT_WORDS[r]);}
quote_terms :: (id:string,digest:string,price:int)->MktTerms {
  t:MktTerms;t.quote_id=id;t.digest=digest;t.worker=WORKER;t.reviewer=REVIEWER;
  t.price_msat=price;t.fee_limit_msat=1000;t.profile=MKT_LIGHTNING;t.network=MKT_BITCOIN;
  t.quote_expires_at=T0+600;t.order_confirm_by=T0+900;t.delivery_due_at=T0+3600;
  t.review_due_at=T0+7200;t.payment_due_at=T0+86400;t.retain_until=T0+30*86400;return t;
}
proof :: (rail:int,state:int,amount:int,preimage:string)->MktProof {
  p:MktProof;p.rail=rail;p.state=state;p.amount_msat=amount;p.preimage=preimage;return p;
}
verify :: (text:string,scratch:[]u8)->NostrEvent {return nostr_event_verify_vartime(text,scratch,false);}
show_answer :: (request:NostrEvent,what:string,text:string) {
  scratch:[4096]u8;e:=verify(text,scratch[..]);
  print("  %: %\n",what,CJ_WORDS[cj_answer(request,e)]);
}
```

<!-- source: nostr/examples/labor-market.coax main -->
```jai
main :: () {
  print("offering: provider key 2, Lightning on bitcoin, valid until T0 + 86400\n");
  o:=MktOffering.{provider=PROVIDER,valid_until=T0+86400,profiles=MKT_LIGHTNING,networks=MKT_BITCOIN};
  m:=[1]MktMarket.[MktMarket.{market="1111",buyer=BUYER,provider=PROVIDER}];q:[4]MktTerms;

  print("negotiation\n");
  say(0,"buyer asks for quotes, limit 60000 msat, answers due T0 + 300",mkt_rfq(m[..],o,BUYER,60000,T0+300,T0+40*86400,T0));
  say(100,"provider quotes A: 50000 msat, fee limit 1000 msat, worker key 3, reviewer key 4",mkt_quote(m[..],q[..],quote_terms("A",TERMS_A,50000),PROVIDER,T0+100));
  say(120,"provider quotes B: 70000 msat",mkt_quote(m[..],q[..],quote_terms("B",TERMS_B,70000),PROVIDER,T0+120));
  say(150,"key 7 quotes C",mkt_quote(m[..],q[..],quote_terms("C",TERMS_B,40000),KEY_7,T0+150));
  say(300,"buyer orders quote Z",mkt_order(m[..],q[..],BUYER,"Z",TERMS_A,"order-1",T0+300));
  say(300,"buyer orders A naming terms bbbb...",mkt_order(m[..],q[..],BUYER,"A",TERMS_B,"order-1",T0+300));
  say(300,"buyer orders A naming terms aaaa...",mkt_order(m[..],q[..],BUYER,"A",TERMS_A,"order-1",T0+300));
  say(301,"buyer orders A again",mkt_order(m[..],q[..],BUYER,"A",TERMS_A,"order-2",T0+301));
  say(350,"provider confirms the order",mkt_ack(m[..],q[..],PROVIDER,true,T0+350));

  print("execution: the buyer's request %... (kind 25920, p = worker key 3)\n",CJV_REQUEST_ID[0..<8]);
  scratch:[4096]u8;request:=verify(CJV_REQUEST,scratch[..]);
  print("  the request verifies: %\n",ifx request.error==NE_OK && request.verified then "yes" else "no");
  show_answer(request,"27020 from worker 3",CJV_ACCEPTED);
  show_answer(request,"26920 from worker 3",CJV_RESULT);
  show_answer(request,"26920 from key 7",CJV_IMPOSTOR);
  show_answer(request,"26900 from worker 3",CJV_FAMILY);
  show_answer(request,"26920 from worker 3, e = 63fadca9...",CJV_WRONG_E);
  show_answer(request,"26920 from worker 3, p = key 7",CJV_WRONG_P);
  // One character of worker 3's encrypted result changed.
  forged:[2048]u8;n:=CJV_RESULT.count;for i:0..<n {forged[i]=CJV_RESULT[i];}
  at:=0;while cast(string)forged[at..<at+11]!="\"content\":\"" {at+=1;}
  at+=11+20;forged[at]=ifx forged[at]=='Q' then 'R' else 'Q';
  bad:=verify(cast(string)forged[0..<n],scratch[..]);
  why:=ifx bad.error==NE_ID then "NE_ID: the id is not the hash of the fields" else "another event error";
  print("  26920 from worker 3, content byte % changed: %, %\n",at,CJ_WORDS[cj_answer(request,bad)],why);

  print("delivery and acceptance\n");
  say(950,"key 7 submits",mkt_submit(m[..],q[..],KEY_7,T0+950));
  say(950,"provider key 2 submits",mkt_submit(m[..],q[..],PROVIDER,T0+950));
  say(950,"worker 3 submits submission 0",mkt_submit(m[..],q[..],WORKER,T0+950));
  say(1000,"buyer accepts before any verification",mkt_accept(m[..],q[..],BUYER,T0+1000,T0+1000));
  say(1200,"reviewer key 4 records passed",mkt_verify(m[..],q[..],REVIEWER,MKT_PASSED));
  say(1300,"buyer accepts, received at T0 + 1000",mkt_accept(m[..],q[..],BUYER,T0+1000,T0+1300));
  print("amount due: % msat\n",m[0].amount_due);

  print("settlement\n");
  say(1350,"provider asks 60000 msat",mkt_instruct(m[..],q[..],PROVIDER,"ob-1",60000,INVOICE,HASH,T0+5000,T0+1350));
  say(1400,"provider asks 50000 msat with lnbc500n1p4wfve5...",mkt_instruct(m[..],q[..],PROVIDER,"ob-1",50000,INVOICE,HASH,T0+5000,T0+1400));
  say(1410,"provider opens a second obligation",mkt_instruct(m[..],q[..],PROVIDER,"ob-2",50000,INVOICE,HASH,T0+5000,T0+1410));
  say(1500,"buyer records attempt 1, fee limit 1000 msat",mkt_attempt(m[..],q[..],BUYER,1,1000,T0+1500));
  say(1505,"buyer records attempt 2",mkt_attempt(m[..],q[..],BUYER,2,1000,T0+1505));
  say(1510,"a zap receipt as proof",mkt_result(m[..],BUYER,proof(MKT_RAIL_ZAP,MKT_PAY_CONFIRMED,50000,PREIMAGE)));
  say(1510,"an on-chain transaction as proof",mkt_result(m[..],BUYER,proof(MKT_RAIL_ONCHAIN,MKT_PAY_CONFIRMED,50000,PREIMAGE)));
  say(1510,"confirmed with preimage 0000...",mkt_result(m[..],BUYER,proof(MKT_RAIL_LIGHTNING,MKT_PAY_CONFIRMED,50000,ZERO)));
  say(1510,"confirmed 50000 msat with preimage 0102...20",mkt_result(m[..],BUYER,proof(MKT_RAIL_LIGHTNING,MKT_PAY_CONFIRMED,50000,PREIMAGE)));
  say(1600,"buyer records attempt 2",mkt_attempt(m[..],q[..],BUYER,2,1000,T0+1600));
  print("paid: % msat in % attempt\n",m[0].paid_msat,m[0].attempts);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/labor-market.coax
```

Its output, in debug and release builds alike
([nostr/fixtures/labor-market.out](../fixtures/labor-market.out)):

```text
offering: provider key 2, Lightning on bitcoin, valid until T0 + 86400
negotiation
T0 + 0: buyer asks for quotes, limit 60000 msat, answers due T0 + 300: accepted
T0 + 100: provider quotes A: 50000 msat, fee limit 1000 msat, worker key 3, reviewer key 4: accepted
T0 + 120: provider quotes B: 70000 msat: price_exceeded
T0 + 150: key 7 quotes C: not_admitted
T0 + 300: buyer orders quote Z: content_unavailable
T0 + 300: buyer orders A naming terms bbbb...: identity_mismatch
T0 + 300: buyer orders A naming terms aaaa...: accepted
T0 + 301: buyer orders A again: conflict
T0 + 350: provider confirms the order: accepted
execution: the buyer's request 2be5278b... (kind 25920, p = worker key 3)
  the request verifies: yes
  27020 from worker 3: answers the request
  26920 from worker 3: answers the request
  26920 from key 7: refused: not the worker the request named
  26900 from worker 3: refused: another family
  26920 from worker 3, e = 63fadca9...: refused: e is not the request
  26920 from worker 3, p = key 7: refused: p is not the caller
  26920 from worker 3, content byte 372 changed: refused: the event does not verify, NE_ID: the id is not the hash of the fields
delivery and acceptance
T0 + 950: key 7 submits: not_admitted
T0 + 950: provider key 2 submits: not_admitted
T0 + 950: worker 3 submits submission 0: accepted
T0 + 1000: buyer accepts before any verification: not passed by the reviewer
T0 + 1200: reviewer key 4 records passed: accepted
T0 + 1300: buyer accepts, received at T0 + 1000: accepted
amount due: 50000 msat
settlement
T0 + 1350: provider asks 60000 msat: amount: not the accepted price
T0 + 1400: provider asks 50000 msat with lnbc500n1p4wfve5...: accepted
T0 + 1410: provider opens a second obligation: conflict
T0 + 1500: buyer records attempt 1, fee limit 1000 msat: accepted
T0 + 1505: buyer records attempt 2: pending: an earlier attempt is unresolved
T0 + 1510: a zap receipt as proof: unsupported_profile
T0 + 1510: an on-chain transaction as proof: unsupported_profile
T0 + 1510: confirmed with preimage 0000...: no proof: the preimage does not hash to the payment hash
T0 + 1510: confirmed 50000 msat with preimage 0102...20: accepted
T0 + 1600: buyer records attempt 2: already paid
paid: 50000 msat in 1 attempt
```

The deliberately broken input is the changed content byte: the event's id
no longer equals SHA-256 of its fields, `nostr_event_read` returns `NE_ID`
(7), and `cj_answer` refuses it as `CJ_EVENT`. Every other refusal in the
transcript comes from a different check: the price limit, the signer of a
quote, an unknown quote, another terms digest, a second order, the CJ
signer, family, request and caller, two keys that are not the worker, a
missing verdict, a wrong amount, a second obligation, a pending attempt,
two rails that are not Lightning, a wrong preimage and a paid obligation.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/labor-market.coax](../examples/checks/labor-market.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The event ids from
  [cj_result_vectors.py](../../coax/tests/nostr/cj_result_vectors.py),
  which computes NIP-01 ids with Python's `json.dumps` and `hashlib`, signs
  with the integer BIP-340 of `nip59_reference.py` and encrypts with its
  NIP-44 v2. Coax recomputes all seven ids. The example verifies all seven
  signatures at run time.
- The invoice's amount and the preimage's hash, worked by hand above.
- PLAN.md's "Done means": a result from another key refuses, at the CJ
  event and at the LAB submission, and an order settles only with a
  Lightning proof.

<!-- source: nostr/examples/checks/labor-market.coax check -->
```jai
// The ids, from coax/tests/nostr/cj_result_vectors.py: Python's json.dumps
// and hashlib.sha256 over [0, pubkey, created_at, kind, tags, content], with
// BIP-340 signing and NIP-44 encryption from nip59_reference.py. They share
// no code with event.coax, sha256.coax or nip44.coax. NIP-CJ, NIP-LAB and
// NIP-MKT publish no wire examples with real ids, so these are the
// reference.
// Whether Coax's recomputed id of text, in hex, is id.
lm_id :: (text:string,id:string)->bool {
  scratch:[4096]u8;hex:[64]u8;
  e:=nostr_event_read(text,scratch[..],false);
  #assert e.error==NE_OK;
  r:=hex_encode_checked(cast(string)e.computed_id[..],hex[..],false);
  #assert r.error==NC_OK && r.written==64;
  return cast(string)hex[..]==id;
}
#test "Coax recomputes each Python id" {
  #assert lm_id(CJV_REQUEST,CJV_REQUEST_ID) && CJV_REQUEST_ID=="2be5278b186ccba2c8a9174fb3475a52c014c0710be6689a197093c7e954f2a8";
  #assert lm_id(CJV_RESULT,CJV_RESULT_ID) && CJV_RESULT_ID=="511dc19e759af8920b4ebf82a9eba1dac307fa0505dabf66ed4cccba37a24f3c";
  #assert lm_id(CJV_ACCEPTED,CJV_ACCEPTED_ID);
  #assert lm_id(CJV_IMPOSTOR,CJV_IMPOSTOR_ID) && CJV_IMPOSTOR_ID=="06cff4c26cd3a6c2f79ba38e2ad1cadedea1b45bcfe5e39cbc805f101c0367c9";
  #assert lm_id(CJV_FAMILY,CJV_FAMILY_ID) && lm_id(CJV_WRONG_E,CJV_WRONG_E_ID) && lm_id(CJV_WRONG_P,CJV_WRONG_P_ID);
}

// The money, worked by hand: 500n is 500 * 10^-9 BTC = 500 * 100 msat =
// 50,000 msat (1 BTC = 10^11 msat), the price of quote A; SHA-256 of the
// bytes 01 .. 20 is ae216c2e... (Python hashlib).
#test "the invoice's amount and the preimage, by hand" {
  #assert x402_bolt11_msat(INVOICE)==500*100 && 500*100==50000;
  #assert x402_preimage_matches(PREIMAGE,HASH) && !x402_preimage_matches(ZERO,HASH);
}

// PLAN.md's Done means.
lm_read :: (text:string)->NostrEvent {scratch:[4096]u8;e:=nostr_event_read(text,scratch[..],false);e.verified=e.error==NE_OK;return e;}
#test "Done means: a job result from a key other than the admitted worker refuses" {
  q:=lm_read(CJV_REQUEST);
  #assert cj_answer(q,lm_read(CJV_RESULT))==CJ_OK && cj_answer(q,lm_read(CJV_IMPOSTOR))==CJ_SIGNER;
  m:=[1]MktMarket.[MktMarket.{buyer=BUYER,provider=PROVIDER}];t:[2]MktTerms;
  o:=MktOffering.{provider=PROVIDER,valid_until=T0+86400,profiles=MKT_LIGHTNING,networks=MKT_BITCOIN};
  #assert mkt_rfq(m[..],o,BUYER,60000,T0+300,T0+400,T0)==MKT_OK;
  #assert mkt_quote(m[..],t[..],quote_terms("A",TERMS_A,50000),PROVIDER,T0+100)==MKT_OK;
  #assert mkt_order(m[..],t[..],BUYER,"A",TERMS_A,"o",T0+300)==MKT_OK && mkt_ack(m[..],t[..],PROVIDER,true,T0+350)==MKT_OK;
  for k:[3]string.[KEY_7,PROVIDER,BUYER] {#assert mkt_submit(m[..],t[..],k,T0+950)==MKT_NOT_ADMITTED;}
  #assert mkt_submit(m[..],t[..],WORKER,T0+950)==MKT_OK;
}
#test "Done means: an order settles only with a Lightning proof" {
  m:=[1]MktMarket.[MktMarket.{buyer=BUYER,provider=PROVIDER}];t:[2]MktTerms;
  o:=MktOffering.{provider=PROVIDER,valid_until=T0+86400,profiles=MKT_LIGHTNING,networks=MKT_BITCOIN};
  #assert mkt_rfq(m[..],o,BUYER,60000,T0+300,T0+400,T0)==MKT_OK;
  #assert mkt_quote(m[..],t[..],quote_terms("A",TERMS_A,50000),PROVIDER,T0+100)==MKT_OK;
  #assert mkt_order(m[..],t[..],BUYER,"A",TERMS_A,"o",T0+300)==MKT_OK && mkt_ack(m[..],t[..],PROVIDER,true,T0+350)==MKT_OK;
  #assert mkt_submit(m[..],t[..],WORKER,T0+950)==MKT_OK && mkt_verify(m[..],t[..],REVIEWER,MKT_PASSED)==MKT_OK;
  #assert mkt_instruct(m[..],t[..],PROVIDER,"ob",50000,INVOICE,HASH,T0+5000,T0+1000)==MKT_NOT_ACCEPTED;
  #assert mkt_accept(m[..],t[..],BUYER,T0+1000,T0+1300)==MKT_OK;
  #assert mkt_instruct(m[..],t[..],PROVIDER,"ob",50000,INVOICE,HASH,T0+5000,T0+1400)==MKT_OK;
  #assert mkt_attempt(m[..],t[..],BUYER,1,1000,T0+1500)==MKT_OK;
  for r:[3]int.[MKT_RAIL_ONCHAIN,MKT_RAIL_ZAP,3] {
    #assert mkt_result(m[..],BUYER,proof(r,MKT_PAY_CONFIRMED,50000,PREIMAGE))==MKT_UNSUPPORTED;
  }
  #assert mkt_result(m[..],BUYER,proof(MKT_RAIL_LIGHTNING,MKT_PAY_CONFIRMED,50000,ZERO))==MKT_PROOF;
  #assert m[0].payment==MKT_PAY_PENDING && m[0].paid_msat==0;
  #assert mkt_result(m[..],BUYER,proof(MKT_RAIL_LIGHTNING,MKT_PAY_CONFIRMED,50000,PREIMAGE))==MKT_OK && m[0].paid_msat==50000;
  #assert mkt_attempt(m[..],t[..],BUYER,2,1000,T0+1600)==MKT_PAID;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/labor-market.coax
```

`coax test` runs 233 checks, including the loaded files' own checks; all
pass.

To regenerate the vectors:

```shell
python3 coax/tests/nostr/cj_result_vectors.py > coax/tests/nostr/cj_result_vectors.coax
```

### The library's own checks

`result.coax` checks the family of every kind near the three bases with
`#forall`, the worker's result and acceptance, the impostor's result, the
other family, request and caller, unverified events on either side, kinds
outside the families, the conversation and decision families, and every
tag shape (no `p`, two `p`, a malformed `p` or `e`, an extra tag, ids that
differ only in their first byte). Its checks recompute the vectors' ids
and mark them verified; BIP-340 verification is `event.coax`'s and is too
slow to repeat for every mutant. `order.coax` checks the whole order from
RFQ to paid, then each step's refusals at their boundaries: the offering's
expiry, each deadline one second either side, every payment profile and
network, a full quote table, a used and an expired quote, a refused
confirmation, the worker, reviewer and buyer roles, each verdict, late and
impossible receipts, each wrong amount, a second obligation, attempt
numbering and fee limits, unknown and failed results, every rail and a
wrong preimage.

```shell
coax/bin/coax test coax/lib/nostr/cj/result.coax --library
coax/bin/coax mutate coax/lib/nostr/cj/result.coax --root-only
coax/bin/coax test coax/lib/nostr/mkt/order.coax --library
coax/bin/coax mutate coax/lib/nostr/mkt/order.coax --root-only
```

On 2026-11-01, `result.coax` passed its checks (204 with `tags.coax`,
`event.coax` and their own) and `coax mutate --root-only` killed all 105
mutants; `order.coax` passed its checks (84 with `settle.coax`'s and
SHA-256's) and killed all 391. None survived and none is marked
equivalent. The full mutation run (`COAX_MUTATION_FULL=1`) and cross-target
runs were not done.

## Summary

- CJ carries the job, MKT the agreement and the money, LAB the deliverable;
  completion, delivery, verification, acceptance and payment stay separate
  facts.
- `cj_answer` admits a response only from the worker the request named,
  with one `p` naming the caller and one `e` naming the request.
- `mkt_quote` keeps quotes within the RFQ's limit with deadlines in order;
  `mkt_order`, the award, names one quote that exists, matches its digest,
  is unused and has not expired.
- `mkt_submit` takes a submission only from the worker in the terms;
  `mkt_accept` needs a passed verdict and a timely receipt, and makes
  exactly the quoted price due.
- `mkt_instruct`, `mkt_attempt` and `mkt_result` allow one obligation at
  the exact price, one attempt at a time, and settlement only with a
  Lightning preimage of the payment hash; a paid obligation refuses
  every further attempt.

## Dependencies

- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [76. Work, coordination and control](ch076-work-coordination-and-control.md)

## Sources

- [OpenAgents NIP-CJ](../nips/openagents/NIP-CJ.md)
- [OpenAgents NIP-LAB](../nips/openagents/NIP-LAB.md)
- [OpenAgents NIP-MKT](../nips/openagents/NIP-MKT.md)
- [OpenAgents shared contracts](../nips/openagents/contracts.md)
- [BOLT 11, invoice protocol for Lightning payments](https://github.com/lightning/bolts/blob/master/11-payment-encoding.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
