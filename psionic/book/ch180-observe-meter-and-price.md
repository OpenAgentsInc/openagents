# 180. Observe, meter and price

> **Status: partly implemented.** Per-request usage records bound to their
> capability evidence, service metrics with a fixed-bucket latency
> histogram, gauges, a content-free audit log, and an `#authority` price
> policy in whole millisatoshis with quotes, budget holds and settlement run
> on the CPU as in-process profiles. Schemas in `proto/` and
> `docs/protocol/`, wiring the meter into the live host so token counts come
> from the engine, and signed receipts are not built yet.

A service that other people depend on has to answer three questions after
every request: what happened, what changed, and what it cost. This chapter
answers them with three things. A **usage record** says what one request
used and under which evidence it ran. An **audit log** says when the
configuration, the build or the price policy changed. A **price policy**
says what a request costs, in whole millisatoshis, and refuses work the
account cannot pay for.

The price policy decides money, so it is an `#authority` file with
zero-survivor mutation checks:
[`serve-price.coax`](../lib/serve-price.coax). The records, metrics and
log are in [`serve-meter.coax`](../lib/serve-meter.coax). Neither moves
money. Settlement stays in the Bitcoin wallet
([docs/lightning/](../../docs/lightning/README.md)), outside Psionic, as it
does for [the voice meter](../../verse/voice-meter.coax).

```shell
coax/bin/coax run psionic/examples/serve-meter.coax
coax/bin/coax test psionic/lib/serve-price.coax
coax/bin/coax test psionic/lib/serve-meter.coax
```

## A price is a total, rounded once

*Inference Engineering* treats cost as a total over what a request
actually used, not as a token price worked backwards from a bill. The
schedule here has four integer rates:

<!-- source: psionic/lib/serve-price.coax types -->
```jai
BlStatus :: enum u8 {none;answered;refused;busy;cancelled;}
// Millisatoshis per answered request, per 1000 input tokens, per 1000 output
// tokens and per device-second.
BlSchedule :: struct {base:int;per_k_in:int;per_k_out:int;per_second:int;}
// What one request used: its outcome, its tokens and its device time.
BlUse :: struct {status:BlStatus;tokens_in:int;tokens_out:int;device_us:int;}
BlCause :: enum u8 {none;schedule;use;account;quote;budget;total;}
BlPrice :: struct {cause:BlCause;msat:int;}
// budget is what the account may spend; held is what admitted requests may
// still charge; spent is what settled requests did charge.
BlAccount :: struct {budget:int;held:int;spent:int;}
// capped says the request ran past its quote and was charged the quote.
BlSettle :: struct {cause:BlCause;charge:int;capped:bool;account:BlAccount;}
```

- `base` is charged once for each answered request.
- `per_k_in` and `per_k_out` are millisatoshis per 1000 input and output
  tokens.
- `per_second` is millisatoshis per device-second, the time the request
  held the device ([chapter 18](ch018-time-a-decision.md)'s timing).

Every rate is at most 1,000,000 msat (1000 sat). Token counts are at most
1,048,576, the bound [chapter 22](ch022-run-it-live.md)'s usage counters
already use, and device time is at most one day.

The rounding rule is the part to get exactly right. Rates per 1000 tokens
and per second do not divide evenly into a whole millisatoshi, so the
price is computed in millionths of a millisatoshi, summed, and rounded
**up to the next whole millisatoshi once per request**:

```text
price = base + ceil((tokens_in * per_k_in * 1000
                   + tokens_out * per_k_out * 1000
                   + device_us * per_second) / 1000000)
```

Work one request by hand before any program runs. The schedule is
`base=1000`, `per_k_in=2000`, `per_k_out=8000`, `per_second=50000`. The
request read 37 tokens, wrote 5 and used 12345 us of device time:

| Part | Millionths of a msat |
| --- | --- |
| input, 37 × 2000 × 1000 | 74,000,000 |
| output, 5 × 8000 × 1000 | 40,000,000 |
| device, 12345 × 50000 | 617,250,000 |
| sum | 731,250,000 |

The sum is 731.25 msat, which rounds up to 732. With the base, the
request costs **1732 msat**. A second request of 3 tokens in, 1 out and
999 us sums to 63,950,000 millionths, 63.95 msat, and costs 1000 + 64 =
**1064 msat**.

Two statuses cost nothing: a request refused at admission and a request
turned away because the slot was busy. A cancelled request pays only for
the device time it used, with no base and no tokens: 12345 us is 617.25
msat, charged 618. These are the checks, written first:

<!-- source: psionic/lib/serve-price.coax check-price -->
```jai
#test "a price is the base plus one ceiling, worked by hand" {
    s:=bl_test_schedule();
    // 37*2000*1000 + 5*8000*1000 + 12345*50000 = 731250000 millionths of a
    // msat: 731.25 msat, rounded up to 732, plus the 1000 msat base.
    p:=bl_price(s,BlUse.{status=BlStatus.answered,tokens_in=37,tokens_out=5,device_us=12345});
    #assert p.cause==BlCause.none && p.msat==1732;
    // 3 in, 1 out, 999 us: 63950000 millionths, 63.95 msat, so 64 + 1000.
    p=bl_price(s,BlUse.{status=BlStatus.answered,tokens_in=3,tokens_out=1,device_us=999});
    #assert p.msat==1064;
    // A cancelled request pays for the device time it used and nothing else:
    // 12345*50000 = 617250000 millionths, 617.25, so 618.
    p=bl_price(s,BlUse.{status=BlStatus.cancelled,tokens_in=37,tokens_out=5,device_us=12345});
    #assert p.msat==618;
    // A refused or busy request costs nothing.
    #assert bl_price(s,BlUse.{status=BlStatus.refused,tokens_in=37,device_us=12345}).msat==0;
    #assert bl_price(s,BlUse.{status=BlStatus.busy,device_us=12345}).msat==0;
}
```

The code follows them. No float appears anywhere in the money path:

<!-- source: psionic/lib/serve-price.coax price -->
```jai
bl_in :: (v:int,most:int)->bool {return v>=0 && v<=most;}
bl_schedule_valid :: (s:BlSchedule)->bool {
    return bl_in(s.base,BL_RATE_MOST) && bl_in(s.per_k_in,BL_RATE_MOST) && bl_in(s.per_k_out,BL_RATE_MOST) && bl_in(s.per_second,BL_RATE_MOST);
}
bl_use_valid :: (u:BlUse)->bool {
    if u.status==BlStatus.none {return false;}
    return bl_in(u.tokens_in,BL_TOKENS_MOST) && bl_in(u.tokens_out,BL_TOKENS_MOST) && bl_in(u.device_us,BL_DEVICE_US_MOST);
}
// Round up to the next whole unit; num is never negative.
bl_ceil :: (num:int,den:int)->int {return (num+den-1)/den;}
// Millionths of a msat: 1000 tokens at r msat is r*1000000/1000 per token.
bl_price :: (s:BlSchedule,u:BlUse)->BlPrice #export_c {
    if !bl_schedule_valid(s) {return BlPrice.{cause=BlCause.schedule};}
    if !bl_use_valid(u) {return BlPrice.{cause=BlCause.use};}
    if u.status==BlStatus.refused || u.status==BlStatus.busy {return BlPrice.{};}
    device:=u.device_us*s.per_second;
    if u.status==BlStatus.cancelled {return BlPrice.{msat=bl_ceil(device,BL_US_PER_SECOND)};}
    tokens:=u.tokens_in*s.per_k_in*1000+u.tokens_out*s.per_k_out*1000;
    return BlPrice.{msat=s.base+bl_ceil(tokens+device,BL_US_PER_SECOND)};
}
// The worst case a request may reach: its input, its output limit, its deadline.
bl_quote :: (s:BlSchedule,tokens_in:int,max_out:int,deadline_us:int)->BlPrice #export_c {
    return bl_price(s,BlUse.{status=BlStatus.answered,tokens_in=tokens_in,tokens_out=max_out,device_us=deadline_us});
}
```

The bounds keep every product inside `int`. The largest numerator is
2 × 1,048,576 × 1,000,000 × 1000 + 86,400,000,000 × 1,000,000, about
8.85 × 10^16 millionths, far below the 9.2 × 10^18 an `int` holds, and the
largest price the schedule can produce is 88,498,152,000 msat. A schedule
or a use outside its bounds is refused with a typed cause, never priced.

The independent check is an oracle that prices the same request a
different way: it splits each component into a whole part and a
remainder, adds the whole parts directly, and rounds the summed
remainders once. A `#forall` compares the two over 500 combinations of
token counts, device times, statuses and schedules, including every rate
at its bound.

## Quote, hold, settle

A request's price is known only after it runs. Admission has to decide
before. So admission prices the **worst case** the request may reach: its
input tokens, its output limit and its deadline. That is the quote. The
price only grows with each part, so any run that stays inside its limits
costs at most its quote.

An account has a budget, what admitted requests hold, and what settled
requests spent. Admission holds the quote if it fits in what is left, and
refuses the request before any model work if it does not. Settlement
releases the hold and charges the smaller of the actual price and the
quote. A request that ran past its deadline is charged its quote, and the
settlement says so with `capped`.

Work the budget by hand. The quote for 37 input tokens, at most 16 output
tokens and a 20 ms deadline is 74,000,000 + 128,000,000 + 1,000,000,000
millionths, 1202 msat, plus the base: 2202 msat. On a 4000 msat budget:

1. The first request is held at 2202. It uses 5 tokens and 12345 us, so
   it is charged 1732; the hold is released. Spent: 1732.
2. A second identical quote fits: 1732 + 2202 = 3934, at most 4000.
3. A third would need 1732 + 2202 + 2202 = 6136 and is refused.

<!-- source: psionic/lib/serve-price.coax check-budget -->
```jai
#test "admission holds the quote; settlement charges at most the quote" {
    s:=bl_test_schedule();
    // The worst case of 37 input tokens, at most 16 output tokens and a 20 ms
    // deadline: 74000000 + 128000000 + 1000000000 millionths, 1202 + 1000.
    q:=bl_quote(s,37,16,20000);#assert q.cause==BlCause.none && q.msat==2202;
    a:=BlAccount.{budget=4000};
    h:=bl_admit(a,q);#assert h.cause==BlCause.none && h.account.held==2202 && h.account.spent==0;
    // It used 5 tokens and 12345 us: 1732 msat, and the hold is released.
    d:=bl_settle(s,h.account,q,BlUse.{status=BlStatus.answered,tokens_in=37,tokens_out=5,device_us=12345});
    #assert d.cause==BlCause.none && d.charge==1732 && d.account.held==0 && d.account.spent==1732;
    // 1732 spent + 2202 held = 3934 fits 4000; a third hold would need 6136.
    h=bl_admit(d.account,q);#assert h.cause==BlCause.none && h.account.held==2202;
    r:=bl_admit(h.account,q);#assert r.cause==BlCause.budget && r.account.held==2202 && r.account.spent==1732;
    // On a 3000 msat budget the second request is refused before any work.
    r=bl_admit(BlAccount.{budget=3000,spent=1732},q);#assert r.cause==BlCause.budget;
}
```

<!-- source: psionic/lib/serve-price.coax admit -->
```jai
bl_account_valid :: (a:BlAccount)->bool {
    return bl_in(a.budget,BL_MSAT_MOST) && bl_in(a.held,a.budget) && bl_in(a.spent,a.budget-a.held);
}
// Hold a quote against what is left; refuse before any work if it does not fit.
bl_admit :: (a:BlAccount,q:BlPrice)->BlSettle #export_c {
    if !bl_account_valid(a) {return BlSettle.{cause=BlCause.account,account=a};}
    if q.cause!=BlCause.none || !bl_in(q.msat,BL_MSAT_MOST) {return BlSettle.{cause=BlCause.quote,account=a};}
    if q.msat>a.budget-a.held-a.spent {return BlSettle.{cause=BlCause.budget,account=a};}
    b:=a;b.held+=q.msat;return BlSettle.{account=b};
}
// Release the hold and charge the smaller of the actual price and the quote.
bl_settle :: (s:BlSchedule,a:BlAccount,q:BlPrice,u:BlUse)->BlSettle #export_c {
    if !bl_account_valid(a) {return BlSettle.{cause=BlCause.account,account=a};}
    if q.cause!=BlCause.none || !bl_in(q.msat,a.held) {return BlSettle.{cause=BlCause.quote,account=a};}
    p:=bl_price(s,u);if p.cause!=BlCause.none {return BlSettle.{cause=p.cause,account=a};}
    capped:=p.msat>q.msat;charge:=ifx capped then q.msat else p.msat;
    b:=a;b.held-=q.msat;b.spent+=charge;return BlSettle.{charge=charge,capped=capped,account=b};
}
```

Holding the quote, not checking it and forgetting it, is what keeps two
requests admitted at the same time from spending the same millisatoshis.
The account invariant `held + spent <= budget` holds after every admit
and every settle; an account that breaks it is refused as `account`.

## An invoice is the sum of the charges

Rounding once per request has a consequence a reader must know about: the
invoice for a period is the sum of the per-request charges, **not** the
schedule applied to the period's totals. Two requests of 1 us each at
400,000 msat per second each cost 0.4 msat, charged 1 apiece: 2 msat.
Pricing the 2 us total once would say 0.8, charged 1. The policy charges
each request what it was quoted against, and the invoice adds them up.

<!-- source: psionic/lib/serve-price.coax check-total -->
```jai
#test "an invoice is the sum of per-request charges, not a re-priced total" {
    s:=BlSchedule.{base=0,per_k_in=0,per_k_out=0,per_second=400000};
    // Each request used 1 us at 400000 msat per second: 0.4 msat, charged 1.
    one:=bl_price(s,BlUse.{status=BlStatus.answered,device_us=1}).msat;#assert one==1;
    charges:=[2]int.[one,one];t:=bl_total(charges[..]);#assert t.cause==BlCause.none && t.msat==2;
    // Pricing the 2 us total once would say 0.8, so 1: one msat less than charged.
    #assert bl_price(s,BlUse.{status=BlStatus.answered,device_us=2}).msat==1;
}
```

<!-- source: psionic/lib/serve-price.coax total -->
```jai
// The invoice: the sum of the charges, refused past 21 million bitcoin.
bl_total :: (charges:[]int)->BlPrice #export_c {
    t:=0;
    for i:0..<charges.count {
        if !bl_in(charges[i],BL_MSAT_MOST-t) {return BlPrice.{cause=BlCause.total,msat=t};}
        t+=charges[i];
    }
    return BlPrice.{msat=t};
}
```

`bl_total` refuses a total past 21 million bitcoin
(2,100,000,000,000,000,000 msat), which also fits inside `int`.

## One record per request

A usage record carries what the request used and the evidence it ran
under: the model, checkpoint digest, backend and profile of the
[chapter 173](ch173-serve-a-model-with-an-honest-capability.md) capability
entry that admitted it, and the chapter-17 record that qualified that
entry. It also carries the quote and the charge. It has no field for the
request's text, its state, its questions or its answer.

<!-- source: psionic/lib/serve-meter.coax types -->
```jai
// What a request ran under, copied from its capability entry (chapter 173).
UmEvidence :: struct {model:string;digest:string;backend:string;profile:string;record:string;}
UmRecord :: struct {seq:int;evidence:UmEvidence;use:BlUse;latency_us:int;quote_msat:int;charge_msat:int;}
UmMetrics :: struct {
    requests:int;status:[5]int;tokens_in:int;tokens_out:int;device_us:int;
    latency:[UM_BUCKETS]int;latency_most:int;slowest:int;quoted:int;charged:int;
}
UmCause :: enum u8 {none;seq;evidence;use;charge;full;kind;digest;order;}
UmResult :: struct {cause:UmCause;metrics:UmMetrics;}
// The current queue depth, its peak and when the peak was first reached.
UmGauges :: struct {replicas:int;queue:int;queue_most:int;queue_most_ms:int;}
UmKind :: enum u8 {none;config;build;policy;}
// One change: what kind, when, and the digests before and after. Nothing else.
UmAudit :: struct {seq:int;at_ms:int;kind:UmKind;before:[64]u8;after:[64]u8;}
UmLog :: struct {count:int;entries:[UM_AUDIT_MAX]UmAudit;}
```

`um_observe` folds one record into the metrics, or refuses it and changes
nothing. Records are numbered from 1, and a record must carry the next
number: a second record for the same request, or a skipped one, is refused
as `seq`. That is what makes "every completed request has exactly one
record" checkable. A request that ran (answered or cancelled) must carry
valid evidence; a refused or busy request ran under no entry and may not
be charged. A charge above its quote is refused.

The metrics hold the volume, a count for each status, total input and
output tokens, total device time, the quoted and charged millisatoshis, and
a latency histogram with 12 fixed buckets. The first 11 have upper bounds of
1, 2, 5, 10, 20, 50, 100, 200 and 500 ms and 1 and 2 s; the 12th holds
everything slower. A percentile is the upper bound of the bucket holding
the observation at rank `ceil(requests × pct / 100)`; in the last bucket
it is the slowest latency seen.

Work three records by hand: the two answered requests above at 13,000 us
and 1,500 us, and one refused request at 40 us. The latencies land in
buckets 4, 1 and 0. The median's rank is ceil(3 × 50 / 100) = 2, so the
walk passes bucket 0 (one record) and stops at bucket 1: **2000 us**. The
99th percentile's rank is 3, which is bucket 4: **20000 us**. The charges
are 1732 + 1064 + 0 = **2796 msat**, the same as the invoice of the three
records.

<!-- source: psionic/lib/serve-meter.coax check-observe -->
```jai
#test "three records fold into totals a reader can recompute by hand" {
    m:=UmMetrics.{};
    r:=[3]UmRecord.[um_test_record(1,BlStatus.answered,37,5,12345,13000,2202,1732),
        um_test_record(2,BlStatus.answered,3,1,999,1500,2202,1064),
        um_test_record(3,BlStatus.refused,0,0,0,40,0,0)];
    for i:0..<3 {o:=um_observe(m,r[i]);#assert o.cause==UmCause.none;m=o.metrics;}
    #assert m.requests==3 && m.status[1]==2 && m.status[2]==1 && m.status[3]==0 && m.status[4]==0;
    #assert m.tokens_in==40 && m.tokens_out==6 && m.device_us==13344;
    // 13000 us is in the (10000, 20000] bucket, 1500 in (1000, 2000], 40 in [0, 1000].
    #assert m.latency[0]==1 && m.latency[1]==1 && m.latency[4]==1 && m.latency_most==13000 && m.slowest==1;
    #assert m.quoted==4404 && m.charged==2796;
    // Conservation: the metrics' charged total is the invoice of the records.
    charges:=[3]int.[r[0].charge_msat,r[1].charge_msat,r[2].charge_msat];#assert bl_total(charges[..]).msat==m.charged;
    // Percentiles: ranks ceil(3*50/100)=2 and ceil(3*99/100)=3.
    #assert um_percentile(m,50)==2000 && um_percentile(m,99)==20000 && um_percentile(m,1)==1000;
}
```

<!-- source: psionic/lib/serve-meter.coax observe -->
```jai
// Fold one record into the metrics, or refuse it and change nothing.
um_observe :: (m:UmMetrics,r:UmRecord)->UmResult {
    if m.requests>=UM_REQUESTS_MOST {return UmResult.{cause=UmCause.full,metrics=m};}
    if r.seq!=m.requests+1 {return UmResult.{cause=UmCause.seq,metrics=m};}
    if !bl_use_valid(r.use) || !bl_in(r.latency_us,BL_DEVICE_US_MOST) {return UmResult.{cause=UmCause.use,metrics=m};}
    ran:=r.use.status==BlStatus.answered || r.use.status==BlStatus.cancelled;
    if ran && !um_evidence_valid(r.evidence) {return UmResult.{cause=UmCause.evidence,metrics=m};}
    if !bl_in(r.quote_msat,UM_QUOTE_MOST) || !bl_in(r.charge_msat,ifx ran then r.quote_msat else 0) {return UmResult.{cause=UmCause.charge,metrics=m};}
    n:=m;n.requests+=1;n.status[cast(int)r.use.status]+=1;
    n.tokens_in+=r.use.tokens_in;n.tokens_out+=r.use.tokens_out;n.device_us+=r.use.device_us;
    n.latency[um_bucket(r.latency_us)]+=1;
    if r.latency_us>n.latency_most || n.slowest==0 {n.latency_most=r.latency_us;n.slowest=r.seq;}
    n.quoted+=r.quote_msat;n.charged+=r.charge_msat;
    return UmResult.{metrics=n};
}
// The upper bound of the bucket holding the observation at rank
// ceil(requests*pct/100); past the last bound, the slowest latency seen.
um_percentile :: (m:UmMetrics,pct:int)->int {
    if m.requests<1 || pct<1 || pct>100 {return -1;}
    rank:=(m.requests*pct+99)/100;seen:=0;
    for i:0..<11 {seen+=m.latency[i];if seen>=rank {return UM_BOUNDS[i];}}
    return m.latency_most;
}
```

The totals stay inside `int` by bounds, not by luck: at most 1,000,000
records, and a quote above 88,498,152,000 msat (the largest price the
schedule can produce) is refused, so a million quotes sum to at most about
8.8 × 10^16.

Beside the metrics, `UmGauges` holds the replica count and the queue depth,
with its peak and when the peak was first reached, and `um_utilization`
reports device time over the time the replicas existed, in basis points:
13,344 device-us over one replica for a 100,000 us window is 1334, or
13.34%. Device time greater than the replicas' time is refused, not
reported as more than 100%.

## Change, not content

The audit log records changes to configuration, builds and policies. An
entry holds a kind, a time in milliseconds and the digests before and
after the change, copied into the log's own storage. The log accepts only
64 lowercase hex digits for each digest, so free text such as a user's
name or a prompt cannot enter it. Times must not go backwards, a change to
the same digest is refused, and the log holds at most 64 entries.

<!-- source: psionic/lib/serve-meter.coax check-audit -->
```jai
#test "the audit log keeps kinds, times and digests, in order" {
    trail:[1]UmLog;
    #assert um_audit(trail[..],100,UmKind.config,UM_D1,UM_D2)==UmCause.none;
    #assert um_audit(trail[..],100,UmKind.policy,UM_D2,UM_D1)==UmCause.none;
    #assert trail[0].count==2 && trail[0].entries[1].seq==2 && trail[0].entries[1].kind==UmKind.policy;
    #assert cast(string)trail[0].entries[0].before[..]==UM_D1 && cast(string)trail[0].entries[0].after[..]==UM_D2;
    // Free text cannot enter: only 64 lowercase hex digits are accepted.
    #assert um_audit(trail[..],101,UmKind.build,UM_D1,"set temperature to 0.7 for alice")==UmCause.digest;
    #assert um_audit(trail[..],99,UmKind.build,UM_D1,UM_D2)==UmCause.order;
    #assert um_audit(trail[..],101,UmKind.none,UM_D1,UM_D2)==UmCause.kind;
    #assert trail[0].count==2;
```

<!-- source: psionic/lib/serve-meter.coax audit -->
```jai
um_audit :: (trail:[]UmLog,at_ms:int,kind:UmKind,before:string,after:string)->UmCause {
    n:=trail[0].count;
    if kind==UmKind.none {return UmCause.kind;}
    if !um_hex64(before) || !um_hex64(after) || before==after {return UmCause.digest;}
    last:=ifx n>0 then trail[0].entries[n-1].at_ms else 0;
    if at_ms<last {return UmCause.order;}
    if n>=UM_AUDIT_MAX {return UmCause.full;}
    e:=UmAudit.{seq=n+1,at_ms=at_ms,kind=kind};
    for i:0..<64 {e.before[i]=before[i];e.after[i]=after[i];}
    trail[0].entries[n]=e;trail[0].count=n+1;return UmCause.none;
}
```

The meter writes plain text lines: one `usage` line per record, a
`metrics` line and one `audit` line per change. The check that matters is
the one with marked content. The request said `MARK-7f3a: my account
number is 12345`; the meter received only its token count:

<!-- source: psionic/lib/serve-meter.coax check-private -->
```jai
#test "a request's marked content leaves no trace in records, metrics or the log" {
    // The request said "MARK-7f3a: my account number is 12345". The engine
    // processed 11 input tokens; nothing else about it reaches the meter.
    r:=um_test_record(1,BlStatus.answered,11,1,800,900,2202,1031);
    m:=um_observe(UmMetrics.{},r).metrics;trail:[1]UmLog;
    #assert um_audit(trail[..],5,UmKind.config,UM_D1,UM_D2)==UmCause.none;
    out:[1024]u8;n:=um_line(r,out[..],0);n=um_metrics_line(m,out[..],n);n=um_audit_line(trail[0].entries[0],out[..],n);
    #assert n>0 && !um_has(out[0..<n],"MARK-7f3a") && !um_has(out[0..<n],"12345 ") && um_has(out[0..<n],"tokens_in=11 ");
}
```

## Run it

The [example](../examples/serve-meter.coax) quotes, holds, runs, settles and records three requests
against a 4000 msat budget. The request texts all carry the marker
`MARK-7f3a`; they never reach the meter.

<!-- source: psionic/examples/serve-meter.coax example -->
```jai
main :: () {
    s:=BlSchedule.{base=1000,per_k_in=2000,per_k_out=8000,per_second=50000};
    // The toy's qualified capability entry from chapter 173.
    toy:=UmEvidence.{model="psionic-toy",digest="b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573",
        backend="cpu",profile="serve-cpu-bf16-v1",record="psionic/fixtures/serve-decision.out"};
    asks:=[3]Ask.[
        Ask.{text="MARK-7f3a: is water wet?",tokens_in=37,max_out=16,deadline_us=20000,tokens_out=5,device_us=12345,latency_us=13000},
        Ask.{text="MARK-7f3a: tea or soup?",tokens_in=3,max_out=16,deadline_us=20000,tokens_out=1,device_us=999,latency_us=1500},
        Ask.{text="MARK-7f3a: one more?",tokens_in=37,max_out=16,deadline_us=20000,latency_us=40}];
    a:=BlAccount.{budget=4000};m:=UmMetrics.{};out:[2048]u8;n:=0;charges:[3]int;
    for i:0..<3 {
        q:=bl_quote(s,asks[i].tokens_in,asks[i].max_out,asks[i].deadline_us);h:=bl_admit(a,q);
        print("request %: quote % msat, budget left % msat: %\n",i+1,q.msat,a.budget-a.held-a.spent,bl_cause_name(h.cause));
        r:=UmRecord.{seq=i+1,evidence=toy,latency_us=asks[i].latency_us};
        if h.cause==BlCause.none {
            r.use=BlUse.{status=BlStatus.answered,tokens_in=asks[i].tokens_in,tokens_out=asks[i].tokens_out,device_us=asks[i].device_us};
            d:=bl_settle(s,h.account,q,r.use);a=d.account;r.quote_msat=q.msat;r.charge_msat=d.charge;
        } else {
            r.use=BlUse.{status=BlStatus.refused,tokens_in=asks[i].tokens_in};
        }
        m=um_observe(m,r).metrics;charges[i]=r.charge_msat;n=um_line(r,out[..],n);
    }
    trail:[1]UmLog;
    changed:=um_audit(trail[..],1000,UmKind.policy,"0f3e4c1b7a9d2e5f60718293a4b5c6d7e8f90a1b2c3d4e5f6a7b8c9d0e1f2a3b","5c2d8e1f4a7b0c3d6e9f2a5b8c1d4e7f0a3b6c9d2e5f8a1b4c7d0e3f6a9b2c5d");#assert changed==UmCause.none;
    n=um_metrics_line(m,out[..],n);n=um_audit_line(trail[0].entries[0],out[..],n);
    print("%",cast(string)out[0..<n]);
    t:=bl_total(charges[..]);
    print("invoice: % msat over % requests; metrics charged % msat; equal: %\n",t.msat,m.requests,m.charged,t.msat==m.charged);
    print("account: spent % msat, held % msat of %\n",a.spent,a.held,a.budget);
    print("utilization: % basis points of one replica over 100000 us\n",um_utilization(m.device_us,1,100000));
    print("request text in the meter's output: %\n",um_has(out[0..<n],"MARK-7f3a"));
}
```

```text
request 1: quote 2202 msat, budget left 4000 msat: none
request 2: quote 2134 msat, budget left 2268 msat: none
request 3: quote 2202 msat, budget left 1204 msat: budget
usage seq=1 model=psionic-toy digest=b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 backend=cpu profile=serve-cpu-bf16-v1 record=psionic/fixtures/serve-decision.out status=answered tokens_in=37 tokens_out=5 device_us=12345 latency_us=13000 quote_msat=2202 charge_msat=1732
usage seq=2 model=psionic-toy digest=b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 backend=cpu profile=serve-cpu-bf16-v1 record=psionic/fixtures/serve-decision.out status=answered tokens_in=3 tokens_out=1 device_us=999 latency_us=1500 quote_msat=2134 charge_msat=1064
usage seq=3 model=psionic-toy digest=b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 backend=cpu profile=serve-cpu-bf16-v1 record=psionic/fixtures/serve-decision.out status=refused tokens_in=37 tokens_out=0 device_us=0 latency_us=40 quote_msat=0 charge_msat=0
metrics requests=3 answered=2 refused=1 busy=0 cancelled=0 tokens_in=77 tokens_out=6 device_us=13344 p50_us=2000 p95_us=20000 p99_us=20000 quoted_msat=4336 charged_msat=2796
audit seq=1 at_ms=1000 kind=policy before=0f3e4c1b7a9d2e5f60718293a4b5c6d7e8f90a1b2c3d4e5f6a7b8c9d0e1f2a3b after=5c2d8e1f4a7b0c3d6e9f2a5b8c1d4e7f0a3b6c9d2e5f8a1b4c7d0e3f6a9b2c5d
invoice: 2796 msat over 3 requests; metrics charged 2796 msat; equal: true
account: spent 2796 msat, held 0 msat of 4000
utilization: 1334 basis points of one replica over 100000 us
request text in the meter's output: false
```

The second request's quote is 6,000,000 + 128,000,000 + 1,000,000,000
millionths, 1134 msat, plus the base: 2134. After two settled requests
2796 msat are spent, so 1204 remain, and the third quote of 2202 is
refused before any work. The invoice equals the metrics' charged total,
and the request text appears in no line.

## What it refuses

Every refusal has a typed cause and changes nothing: no hold, no charge,
no record, no log entry. The [refusal example](../examples/serve-meter-refuse.coax)
shows one of each.

<!-- source: psionic/examples/serve-meter-refuse.coax refuse -->
```jai
main :: () {
    s:=BlSchedule.{base=1000,per_k_in=2000,per_k_out=8000,per_second=50000};
    bad:=s;bad.per_second=-1;
    print("a negative rate: %\n",bl_cause_name(bl_price(bad,BlUse.{status=BlStatus.answered}).cause));
    print("2000000 input tokens: %\n",bl_cause_name(bl_quote(s,2000000,16,20000).cause));
    q:=bl_quote(s,37,16,20000);
    h:=bl_admit(BlAccount.{budget=2201},q);
    print("a 2202 msat quote on a 2201 msat budget: %, held %\n",bl_cause_name(h.cause),h.account.held);
    d:=bl_settle(s,BlAccount.{budget=4000},q,BlUse.{status=BlStatus.answered,tokens_in=37,tokens_out=5,device_us=12345});
    print("settling a quote that was never held: %, spent %\n",bl_cause_name(d.cause),d.account.spent);
    big:=[2]int.[BL_MSAT_MOST,1];t:=bl_total(big[..]);
    print("an invoice past 21 million bitcoin: %, total stops at %\n",bl_cause_name(t.cause),t.msat);
    toy:=UmEvidence.{model="psionic-toy",digest="b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573",
        backend="cpu",profile="serve-cpu-bf16-v1",record="psionic/fixtures/serve-decision.out"};
    r:=UmRecord.{seq=1,evidence=toy,use=BlUse.{status=BlStatus.answered,tokens_in=37,tokens_out=5,device_us=12345},latency_us=13000,quote_msat=2202,charge_msat=1732};
    m:=um_observe(UmMetrics.{},r).metrics;
    print("record 1 again: %\n",um_cause_name(um_observe(m,r).cause));
    x:=r;x.seq=3;print("record 3 after record 1: %\n",um_cause_name(um_observe(m,x).cause));
    x=r;x.seq=2;x.evidence.digest="";print("an answer with no checkpoint digest: %\n",um_cause_name(um_observe(m,x).cause));
    x=r;x.seq=2;x.charge_msat=2203;print("a charge of 2203 msat on a 2202 msat quote: %\n",um_cause_name(um_observe(m,x).cause));
    print("metrics after the refusals: % requests, % msat charged\n",m.requests,m.charged);
    trail:[1]UmLog;D1:="1111111111111111111111111111111111111111111111111111111111111111";
    print("free text in the audit log: %\n",um_cause_name(um_audit(trail[..],5,UmKind.config,D1,"temperature 0.7 for alice")));
    first:=um_audit(trail[..],5,UmKind.config,D1,"2222222222222222222222222222222222222222222222222222222222222222");
    print("a change at 4 ms after one at 5 ms: %\n",um_cause_name(um_audit(trail[..],4,UmKind.config,D1,"3333333333333333333333333333333333333333333333333333333333333333")));
    print("audit entries kept: % (first: %)\n",trail[0].count,um_cause_name(first));
}
```

```text
a negative rate: schedule
2000000 input tokens: use
a 2202 msat quote on a 2201 msat budget: budget, held 0
settling a quote that was never held: quote, spent 0
an invoice past 21 million bitcoin: total, total stops at 2100000000000000000
record 1 again: seq
record 3 after record 1: seq
an answer with no checkpoint digest: evidence
a charge of 2203 msat on a 2202 msat quote: charge
metrics after the refusals: 1 requests, 1732 msat charged
free text in the audit log: digest
a change at 4 ms after one at 5 ms: order
audit entries kept: 1 (first: none)
```

Both examples run in debug and release and their output is compared with
the recorded transcripts. The program exits with status 0 because each
refusal is an expected, typed answer.

## Checks

`coax test psionic/lib/serve-price.coax` runs 11 checks, including the
500-case oracle comparison, and `coax test psionic/lib/serve-meter.coax`
runs 23, in debug and release. `coax mutate psionic/lib/serve-price.coax`
reports 128 mutants, 128 killed, 0 survived. `coax mutate
psionic/lib/serve-meter.coax --root-only` reports 268 mutants, 268 killed,
0 survived; `--root-only` mutates the meter, and the price policy it loads
carries its own gate.

## Limits

- The schedule, records, metrics and log are the in-process profiles
  `psionic-price-v1` and `psionic-usage-v1`. Their schemas are not yet in
  `proto/` or `docs/protocol/`.
- The live host does not call the meter yet. Chapter 22's
  `psionic-decision-metrics-v1` file and its `ls_usage` counters still
  record the live service; the examples here pass token counts and device
  times in as numbers. A check that every record's token counts match what
  the engine processed needs that wiring.
- Records are not signed. Chapter 26's signed receipts and recomputable
  pool totals are not applied to these records yet.
- The audit log's digests in the example stand in for digests of real
  configuration and schedule files.
- Psionic does not move money. The charged total goes to the Breez Spark
  wallet for settlement over Lightning, which this chapter does not call.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [The voice meter](../../verse/voice-meter.coax), the pattern for integer
  millisatoshi pricing
