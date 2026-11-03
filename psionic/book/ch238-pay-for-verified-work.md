# 238. Pay for verified work

> **Status: partly implemented.** The buyer's side runs in process: a quote held in escrow per job, released only after the checker agrees (or, by policy, for unsampled work), withheld on disagreement, held for the owner when unverifiable or above the payout cap, one payment intent per receipt digest, a recount from the event log, and a wallet boundary that states Lightning and Spark amounts without paying them. Executing a payment is an owner action. Worker self-tests, a separately paid validator and trace-machine work remain; see [What remains](#what-remains).

[Chapter 195](ch195-check-a-peers-work.md) built a checker. It admits a
peer's receipt, samples some of the work, runs it again and says whether the
peer's output agrees. Its ledger says what is owed. This chapter adds the
step between that verdict and a wallet: who is owed what, when, and exactly
once.

A buyer that pays for work it has not checked can be cheated. A buyer that
pays twice for one piece of work loses money without being cheated. A
buyer that promises more than it has cannot pay at all. This chapter's
ledger addresses all three:

- **Escrow before work.** Before a job runs, the buyer holds the job's quote
  ([chapter 180](ch180-observe-meter-and-price.md)) against a cap the owner
  set for the period. A job whose hold does not fit is refused before any
  work runs.
- **Release after a verdict.** A job's hold is released only when the
  checker's comparison agrees, or when the job was not sampled and the
  policy trusts unsampled work. A disagreement withholds the payment. An
  unverifiable result, unsampled work under a cautious policy, and any
  single payment above the payout cap wait for the owner.
- **Exactly once.** Each release adds one payment intent, keyed by the
  receipt digest. A receipt cannot be held twice, and a decided job cannot
  be decided again, so no receipt is paid twice.
- **A recount.** The buyer keeps a log of every accepted hold, verdict and
  owner decision. Anyone with the log can recount the ledger and get the
  same totals.
- **A wallet boundary.** An intent becomes the amount a Bitcoin wallet would
  be asked to pay: millisatoshis for a Lightning invoice, whole sats for a
  Spark transfer. Nothing here pays it.

One `#authority` file holds every decision:
[`peer/pay.coax`](../lib/peer/pay.coax). It loads chapter 195's
[`peer/audit.coax`](../lib/peer/audit.coax) for the verdicts and, through
it, chapter 180's `bl_admit` for the hold and `BL_MSAT_MOST` (21 million
bitcoin in millisatoshis) for every bound. It holds no key, opens no
socket and calls no wallet. Its shape follows the wallet's own payment
rules in [`verse/wallet.coax`](../../verse/wallet.coax): plain integers in,
a decision out, with the wallet SDK outside.

```shell
coax/bin/coax run psionic/examples/verified-pay.coax
coax/bin/coax test psionic/lib/peer/pay.coax
coax/bin/coax mutate psionic/lib/peer/pay.coax --root-only
```

## The ledger

A job is named by its receipt digest: the SHA-256 of chapter 195's receipt
text, 64 lowercase hex digits. The ledger keeps at most 16 jobs, the payment
intents released so far, the owner's cap as chapter 180's account, and the
total withheld. It copies each receipt digest and payee into its own
storage. A string argument only borrows the caller's bytes, and a caller
that writes the next digest into the same buffer would otherwise rename
every job already held, along with its payment intent and the wallet's key.

<!-- source: psionic/lib/peer/pay.coax types -->
```jai
VP_JOBS :: 16;
VpState :: enum u8 {none;held;paid;withheld;review;}
// The ledger keeps its own copy of every receipt digest and payee. A string
// argument only borrows the caller's bytes, and a caller may reuse them for
// the next job; a stored borrow would then rename a held job, its intent and
// the wallet's idempotency key. VP_TEXT bytes hold a 64-digit digest or a
// hex public key, packed eight to a word, the first byte lowest.
VP_TEXT :: 64;
VpText :: struct {n:int;w:[VP_TEXT/8]u64;}
// One job: the receipt digest that names it, who is paid, the quote held
// for it and, once decided, the amount owed.
VpJob :: struct {receipt:VpText;payee:VpText;quote:int;amount:int;state:VpState;}
// What a wallet is asked to pay: at is the paid job's index, whose receipt
// digest is the key and whose payee is paid.
VpIntent :: struct {at:int;msat:int;}
// trust_unsampled: work the checker did not sample is paid without review.
// payout_cap: a single payment above it waits for the owner.
VpPolicy :: struct {trust_unsampled:bool;payout_cap:int;}
// account.budget is the owner's cap for the period; account.held is escrow;
// account.spent is what intents ask for. withheld counts what was refused.
VpLedger :: struct {account:BlAccount;n:int;jobs:[VP_JOBS]VpJob;m:int;intents:[VP_JOBS]VpIntent;withheld:int;}
VpRefuse :: enum u8 {none;receipt;payee;unit;quote;twice;full;budget;policy;unknown;settled;amount;verdict;total;rail;fraction;}
VpStep :: struct {refuse:VpRefuse;state:VpState;ledger:VpLedger;}
```

The account's three numbers carry the whole money state. `budget` is the
owner's cap for the period. `held` is the escrow: the quotes of jobs not yet
paid or withheld. `spent` is what the intents ask the wallet to pay.
Chapter 180's `bl_admit` keeps `held + spent <= budget` on every hold.

## Hold before the work runs

A hold names the receipt digest, the payee, the quote and its unit. The
payee is 1 to 64 bytes, room for a hex public key. The unit is `msat` or `sat`, and a sat is 1000 msat. Any other unit, such as
`usdt`, is not a Bitcoin amount and is refused. The checks run in a fixed
order, and the first one that fails names the refusal.

<!-- source: psionic/lib/peer/pay.coax hold -->
```jai
// Copy at most VP_TEXT bytes; callers refuse anything longer first.
vp_text :: (s:string)->VpText {
    t:VpText;
    for i:0..<s.count {t.w[i/8]=t.w[i/8]|(cast(u64)s[i]<<((i%8)*8));}
    t.n=s.count;
    return t;
}
vp_byte :: (t:VpText,i:int)->u8 {return cast(u8)((t.w[i/8]>>((i%8)*8))&255);}
vp_same :: (a:VpText,b:VpText)->bool {
    if a.n!=b.n {return false;}
    for j:0..<VP_TEXT/8 {if a.w[j]!=b.w[j] {return false;}}
    return true;
}
vp_is :: (t:VpText,s:string)->bool {return s.count<=VP_TEXT && vp_same(t,vp_text(s));}
// Write the kept bytes to out for a caller to read: the count, or -1 when
// out is too small.
vp_bytes :: (t:VpText,out:[]u8)->int {
    if out.count<t.n {return -1;}
    for i:0..<t.n {out[i]=vp_byte(t,i);}
    return t.n;
}
// A receipt digest is 64 lowercase hex digits, as audit-run.coax writes it.
vp_digest_ok :: (s:string)->bool {
    if s.count!=64 {return false;}
    for i:0..<64 {
        c:=s[i];
        if !((c>='0' && c<='9') || (c>='a' && c<='f')) {return false;}
    }
    return true;
}
vp_find :: (l:VpLedger,receipt:string)->int {
    if receipt.count>VP_TEXT {return -1;}
    r:=vp_text(receipt);
    for i:0..<l.n {if vp_same(l.jobs[i].receipt,r) {return i;}}
    return -1;
}
// Hold a quote before the work runs. unit is "msat" or "sat"; anything else
// is not a Bitcoin amount and is refused. The hold must fit the owner's cap.
vp_hold :: (l:VpLedger,receipt:string,payee:string,quote:int,unit:string)->VpStep #export_c {
    r:=VpStep.{refuse=VpRefuse.receipt,ledger=l};
    if !vp_digest_ok(receipt) {return r;}
    r.refuse=VpRefuse.payee;
    if payee.count==0 || payee.count>VP_TEXT {return r;}
    r.refuse=VpRefuse.unit;
    if unit!="msat" && unit!="sat" {return r;}
    most:=BL_MSAT_MOST;scale:=1;
    if unit=="sat" {most=BL_MSAT_MOST/1000;scale=1000;}
    r.refuse=VpRefuse.quote;
    if quote<1 || quote>most {return r;}
    q:=quote*scale;
    r.refuse=VpRefuse.twice;
    if vp_find(l,receipt)>=0 {return r;}
    r.refuse=VpRefuse.full;
    if l.n==VP_JOBS {return r;}
    a:=bl_admit(l.account,BlPrice.{msat=q});
    r.refuse=VpRefuse.budget;
    if a.cause!=BlCause.none {return r;}
    n:=l;n.jobs[n.n]=VpJob.{receipt=vp_text(receipt),payee=vp_text(payee),quote=q,state=VpState.held};n.n+=1;n.account=a.account;
    return VpStep.{state=VpState.held,ledger=n};
}
```

The quote's upper bound is per unit: `BL_MSAT_MOST` for msat and
`BL_MSAT_MOST/1000` for sat, so the product `quote*1000` never leaves
`int`.

## Decide after the check

The checker's verdict is chapter 195's `PkCompare`. The amount is the work's
actual price under chapter 180's schedule, at least 1 msat and never more
than the quote that was held. As in `pk_settle`, a sampled job needs a
verdict and an unsampled job must not have one.

<!-- source: psionic/lib/peer/pay.coax decide -->
```jai
vp_policy_valid :: (p:VpPolicy)->bool {return bl_in(p.payout_cap,BL_MSAT_MOST);}
// Move job k to its end state. Review keeps the quote in escrow. Paid and
// withheld release it; paid adds one intent, withheld adds to the refused
// total, which never passes 21 million bitcoin.
vp_end :: (l:VpLedger,k:int,amount:int,to:VpState)->VpStep {
    n:=l;n.jobs[k].amount=amount;n.jobs[k].state=to;
    if to==VpState.review {return VpStep.{state=to,ledger=n};}
    n.account.held-=l.jobs[k].quote;
    if to==VpState.withheld {
        if !bl_in(amount,BL_MSAT_MOST-l.withheld) {return VpStep.{refuse=VpRefuse.total,ledger=l};}
        n.withheld+=amount;return VpStep.{state=to,ledger=n};
    }
    n.account.spent+=amount;
    n.intents[n.m]=VpIntent.{at=k,msat=amount};n.m+=1;
    return VpStep.{state=to,ledger=n};
}
// The checker's verdict on a held job. amount is the work's actual price,
// at least 1 msat and at most the quote. A sampled job needs a verdict and
// an unsampled one must not have one, as in pk_settle. Disagree withholds.
// Unverifiable work, untrusted unsampled work and a payment above the cap
// wait for the owner. Everything else is paid.
vp_decide :: (l:VpLedger,p:VpPolicy,receipt:string,amount:int,sampled:bool,c:PkCompare)->VpStep #export_c {
    r:=VpStep.{refuse=VpRefuse.policy,ledger=l};
    if !vp_policy_valid(p) {return r;}
    k:=vp_find(l,receipt);
    r.refuse=VpRefuse.unknown;
    if k<0 {return r;}
    r.refuse=VpRefuse.settled;
    if l.jobs[k].state!=VpState.held {return r;}
    r.refuse=VpRefuse.amount;
    if amount<1 || amount>l.jobs[k].quote {return r;}
    r.refuse=VpRefuse.verdict;
    if sampled==(c.verdict==PkVerdict.none) {return r;}
    if c.verdict==PkVerdict.disagree {return vp_end(l,k,amount,VpState.withheld);}
    if c.verdict==PkVerdict.unverifiable || (!sampled && !p.trust_unsampled) || amount>p.payout_cap {return vp_end(l,k,amount,VpState.review);}
    return vp_end(l,k,amount,VpState.paid);
}
// The owner's decision on a job held for review: pay it or withhold it.
vp_review :: (l:VpLedger,receipt:string,pay:bool)->VpStep #export_c {
    k:=vp_find(l,receipt);
    r:=VpStep.{refuse=VpRefuse.unknown,ledger=l};
    if k<0 {return r;}
    r.refuse=VpRefuse.settled;
    if l.jobs[k].state!=VpState.review {return r;}
    return vp_end(l,k,l.jobs[k].amount,ifx pay then VpState.paid else VpState.withheld);
}
```

Paid and withheld release the job's whole quote from escrow. Paid adds the
amount to `spent` and appends one intent. Withheld adds the amount to the
withheld total, which is refused past 21 million bitcoin. Review moves
nothing: the quote stays in escrow until the owner decides with
`vp_review`, and the owner's decision is final whatever the payout cap.

## Work it by hand

The example uses chapter 195's schedule: 1000 msat a request, 2000 msat per
1000 input tokens, 8000 per 1000 output tokens, and 50000 msat per
device-second. The buyer quotes each job as 1 input token, no output and a
1 ms deadline:

- quote: 1000 + ceil((1 x 2000 x 1000 + 1000 x 50000) / 1000000)
  = 1000 + ceil(52000000 / 1000000) = **1052 msat**.

Request k uses 200 + 10k microseconds, so the actual prices are:

- request 0: 1000 + ceil((2000000 + 200 x 50000) / 1000000)
  = 1000 + 12 = **1012 msat**;
- request 1: 1000 + ceil(12500000 / 1000000) = 1000 + 13 = **1013 msat**;
- requests 2, 3 and 4: 1013, 1014 and 1014 msat.

The owner's cap is 6000 msat for the period, the payout cap is 1100 msat,
and unsampled work is not trusted. Six jobs run. Each hold adds 1052 msat
to the escrow; each decision releases it.

| Job | Peer, profile | Verdict | Outcome | Escrow | Owed | Withheld |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | alpha, seq | agree, exact | paid 1012 | 0 | 1012 | 0 |
| 2 | beta, pair | agree, within | paid 1013 | 0 | 2025 | 0 |
| 3 | gamma, seq, half its input | disagree, bits | withheld 1013 | 0 | 2025 | 1013 |
| 4 | delta, gpu | unverifiable, profile | review | 1052 | 2025 | 1013 |
| 5 | alpha, seq | not sampled | review | 2104 | 2025 | 1013 |
| 6 | beta, pair | not decided | held | 3156 | 2025 | 1013 |

Then the owner withholds job 4 and pays job 5. Escrow falls to 1052 (job
6's quote), owed rises to 2025 + 1014 = **3039 msat** in three intents, and
withheld becomes 1013 + 1014 = **2027 msat**. The cap left for the period is
6000 - 1052 - 3039 = **1909 msat**, so a hold of 1910 msat is refused.

Every intent is a Lightning amount. None is a whole number of sats, so a
Spark transfer of any of them is refused rather than rounded: 1012 msat is
1.012 sat, and rounding down would underpay by 12 msat while rounding up
would overpay.

The first check walks a smaller ledger the same way, with chapter 180's
2202 msat quote and a 1732 msat price:

<!-- source: psionic/lib/peer/pay.coax check-trace -->
```jai
#test "hold, verify, pay or withhold, worked by hand" {
    // Chapter 180's quote for 37 input tokens, 16 output and 20 ms: 2202 msat.
    // Each job's actual price is 1732 msat. The owner's cap is 10000 msat.
    p:=VpPolicy.{trust_unsampled=true,payout_cap=5000};
    l:=VpLedger.{account=BlAccount.{budget=10000}};
    for d:[3]string.[VPT_A,VPT_B,VPT_C] {t:=vp_hold(l,d,"alpha",2202,"msat");#assert t.refuse==VpRefuse.none && t.state==VpState.held;l=t.ledger;}
    #assert l.account.held==6606 && l.account.spent==0 && l.n==3;
    // A fourth 2202 msat hold needs 8808 <= 10000: it fits; a fifth would not.
    // The checker agrees with A: 2202 leaves escrow, 1732 is owed, one intent.
    t:=vp_decide(l,p,VPT_A,1732,true,vpt_agree());
    #assert t.refuse==VpRefuse.none && t.state==VpState.paid && t.ledger.account.held==4404 && t.ledger.account.spent==1732;
    #assert t.ledger.m==1 && t.ledger.intents[0].at==0 && t.ledger.intents[0].msat==1732;
    // B disagrees: its hold is released and nothing is owed.
    t=vp_decide(t.ledger,p,VPT_B,1732,true,vpt_verdict(PkVerdict.disagree));
    #assert t.state==VpState.withheld && t.ledger.account.held==2202 && t.ledger.account.spent==1732 && t.ledger.withheld==1732 && t.ledger.m==1;
    // C could not be verified: it stays in escrow for the owner.
    t=vp_decide(t.ledger,p,VPT_C,1732,true,vpt_verdict(PkVerdict.unverifiable));
    #assert t.state==VpState.review && t.ledger.account.held==2202 && t.ledger.jobs[2].amount==1732 && t.ledger.m==1;
    // The owner pays C.
    t=vp_review(t.ledger,VPT_C,true);
    #assert t.state==VpState.paid && t.ledger.account.held==0 && t.ledger.account.spent==3464 && t.ledger.m==2 && t.ledger.intents[1].at==2;
    // The wallet boundary: 1732 msat is a Lightning amount, not a whole sat.
    i:=vp_invoice(t.ledger,VPT_A,"lightning");
    #assert i.refuse==VpRefuse.none && i.rail==VpRail.lightning && i.msat==1732 && i.sat==0 && vp_is(i.key,VPT_A) && vp_is(i.payee,"alpha");
    #assert vp_invoice(t.ledger,VPT_A,"spark").refuse==VpRefuse.fraction;
}
```

## Run it

The [example](../examples/verified-pay.coax) builds each job's receipt with chapter 195's
`audit-run.coax`, so the receipt digests are real. Job 1's receipt is the
one chapter 195 pinned against an independent Python computation, and its
digest begins `87519f1a334a`. The checker re-executes the sampled jobs with
the `seq` backend and compares under chapter 195's profiles.

```shell
coax/bin/coax run psionic/examples/verified-pay.coax
```

```text
cap 6000 msat for the period, payout cap 1100 msat, unsampled work trusted: false
quote per job: 1052 msat (1 token in, none out, 1 ms deadline)
job 1 (alpha, request 0, stage-f32-seq-v1): receipt 87519f1a334a..., price 1012 msat
  hold: held; escrow 1052 msat, owed 0 msat
  checked: agree exact
  verdict: paid; escrow 0 msat, owed 1012 msat
job 2 (beta, request 1, stage-f32-pair-v1): receipt 863b0ede4c0a..., price 1013 msat
  hold: held; escrow 1052 msat, owed 1012 msat
  checked: agree within
  verdict: paid; escrow 0 msat, owed 2025 msat
job 3 (gamma, request 2, stage-f32-seq-v1): receipt c782a5b45845..., price 1013 msat
  hold: held; escrow 1052 msat, owed 2025 msat
  checked: disagree bits
  verdict: withheld; escrow 0 msat, owed 2025 msat
job 4 (delta, request 3, stage-f32-gpu-v1): receipt 113017998b58..., price 1014 msat
  hold: held; escrow 1052 msat, owed 2025 msat
  checked: unverifiable profile
  verdict: review; escrow 1052 msat, owed 2025 msat
job 5 (alpha, request 4, stage-f32-seq-v1): receipt 217cbdd6b712..., price 1014 msat
  hold: held; escrow 2104 msat, owed 2025 msat
  not sampled
  verdict: review; escrow 2104 msat, owed 2025 msat
job 6 (beta, request 5, stage-f32-pair-v1): receipt de4d0148665e..., price 1015 msat
  hold: held; escrow 3156 msat, owed 2025 msat
owner review:
  job 4, unverifiable, withhold: withheld; escrow 2104 msat, owed 2025 msat
  job 5, not sampled, pay: paid; escrow 1052 msat, owed 3039 msat
ledger: 6 jobs, escrow 1052 msat, owed 3039 msat, withheld 2027 msat, cap left 1909 msat
payment intents (not executed):
  87519f1a334a 1012 msat to alpha: lightning invoice for 1012 msat; spark fraction
  863b0ede4c0a 1013 msat to beta: lightning invoice for 1013 msat; spark fraction
  217cbdd6b712 1014 msat to alpha: lightning invoice for 1014 msat; spark fraction
refusals:
  a verdict for a receipt never held: unknown
  job 1 released again: settled
  job 6 for 1053 msat on a 1052 msat quote: amount
  a hold in usdt: unit
  a hold past the cap: budget
  job 1's intent on an EVM rail: rail
ledger after the refusals: owed 3039 msat, 3 intents
recount of 13 events: none, escrow 1052 msat, owed 3039 msat, withheld 2027 msat, 3 intents
a log that releases job 1 twice: settled, at event 13
```

The transcript matches the hand trace line for line. The example's
transcript is compared exactly in debug and release builds.

## What it refuses

A refusal returns the ledger unchanged. The example's last lines show each
kind: a verdict for a receipt that was never held is `unknown`; job 1
released again is `settled`; 1053 msat on a 1052 msat quote is `amount`; a
hold in `usdt` is `unit`; a hold past the cap is `budget`; and a rail that is
not Lightning or Spark is `rail`. The check below asserts the same
refusals and that the ledger did not change:

<!-- source: psionic/lib/peer/pay.coax check-refuse -->
```jai
#test "refusals change nothing" {
    p:=VpPolicy.{trust_unsampled=true,payout_cap=5000};
    l:=vp_hold(VpLedger.{account=BlAccount.{budget=10000}},VPT_A,"alpha",2202,"msat").ledger;
    t:=vp_decide(l,p,VPT_B,1732,true,vpt_agree());#assert t.refuse==VpRefuse.unknown && vpt_same(t.ledger,l);      // never held
    t=vp_decide(l,p,VPT_A,2203,true,vpt_agree());#assert t.refuse==VpRefuse.amount && vpt_same(t.ledger,l);        // above the quote
    t=vp_hold(l,VPT_B,"alpha",2202,"usdt");#assert t.refuse==VpRefuse.unit && vpt_same(t.ledger,l);                 // not Bitcoin
    paid:=vp_decide(l,p,VPT_A,1732,true,vpt_agree()).ledger;
    t=vp_decide(paid,p,VPT_A,1732,true,vpt_agree());#assert t.refuse==VpRefuse.settled && vpt_same(t.ledger,paid);  // released twice
    t=vp_review(paid,VPT_A,true);#assert t.refuse==VpRefuse.settled && vpt_same(t.ledger,paid);                     // paid, not in review
    t=vp_hold(paid,VPT_A,"alpha",2202,"msat");#assert t.refuse==VpRefuse.twice && vpt_same(t.ledger,paid);          // the same receipt again
    #assert vp_invoice(paid,VPT_A,"usdc").refuse==VpRefuse.rail && vp_invoice(paid,VPT_B,"lightning").refuse==VpRefuse.unknown;
}
```

A recount stops at the first event the ledger refuses and reports its
index. The example's tampered log repeats job 1's release as event 13, and
the recount refuses it there with `settled`.

## The wallet boundary

<!-- source: psionic/lib/peer/pay.coax invoice -->
```jai
// The wallet boundary. A Lightning invoice names its amount in msat; a Spark
// transfer moves whole sats, so an amount with a fraction of a sat is
// refused for Spark rather than rounded. Any other rail is refused. key is
// the idempotency key the wallet records, so a retried intent pays once.
VpRail :: enum u8 {lightning;spark;}
VpInvoice :: struct {refuse:VpRefuse;rail:VpRail;msat:int;sat:int;key:VpText;payee:VpText;}

vp_invoice :: (l:VpLedger,key:string,rail:string)->VpInvoice #export_c {
    r:=VpInvoice.{refuse=VpRefuse.unknown};
    if key.count>VP_TEXT {return r;}
    want:=vp_text(key);k:=-1;
    for i:0..<l.m {if vp_same(l.jobs[l.intents[i].at].receipt,want) {k=i;}}
    if k<0 {return r;}
    t:=l.intents[k];j:=l.jobs[t.at];
    if rail=="lightning" {return VpInvoice.{rail=VpRail.lightning,msat=t.msat,key=j.receipt,payee=j.payee};}
    r.refuse=VpRefuse.rail;
    if rail!="spark" {return r;}
    r.refuse=VpRefuse.fraction;
    if t.msat%1000!=0 {return r;}
    return VpInvoice.{rail=VpRail.spark,msat=t.msat,sat=t.msat/1000,key=j.receipt,payee=j.payee};
}
```

The key the wallet receives is the receipt digest. A wallet that records
the keys it has paid can retry an intent after a crash without paying it
twice. That bookkeeping, like the payment itself, happens in the wallet
([docs/lightning/](../../docs/lightning/README.md)), not here.

## Recount from the log

<!-- source: psionic/lib/peer/pay.coax recount -->
```jai
// The log a buyer publishes: every accepted hold, verdict and owner
// decision, in order. msat is the quote for a hold and the amount for a
// verdict.
VpKind :: enum u8 {hold;decide;review;}
VpEvent :: struct {kind:VpKind;receipt:string;payee:string;msat:int;sampled:bool;verdict:PkVerdict;pay:bool;}
VpRecount :: struct {refuse:VpRefuse;at:int=-1;ledger:VpLedger;}

vp_apply :: (l:VpLedger,p:VpPolicy,e:VpEvent)->VpStep {
    if e.kind==VpKind.hold {return vp_hold(l,e.receipt,e.payee,e.msat,"msat");}
    if e.kind==VpKind.decide {return vp_decide(l,p,e.receipt,e.msat,e.sampled,PkCompare.{verdict=e.verdict});}
    return vp_review(l,e.receipt,e.pay);
}
vp_recount :: (budget:int,p:VpPolicy,events:[]VpEvent)->VpRecount #export_c {
    l:=VpLedger.{account=BlAccount.{budget=budget}};
    for i:0..<events.count {
        t:=vp_apply(l,p,events[i]);
        if t.refuse!=VpRefuse.none {return VpRecount.{refuse=t.refuse,at=i,ledger=l};}
        l=t.ledger;
    }
    return VpRecount.{ledger=l};
}
```

The recount starts from an empty ledger with the same cap and policy, and
applies the logged events in order. It ends with the ledger the buyer
holds, or with the index of the first event that ledger would refuse.

## Checks

The independent check is a brute-force ledger. It does not call the
ledger's own rules: it tracks each of two receipts' states and the three sums
by hand, then compares them with the ledger after every operation. Over
every sequence of three operations from fourteen choices, and both trust
policies (5488 cases), it checks the refusal, the end state, conservation
(escrow is the sum of quotes still held or in review; `spent` is the sum
of the intents; `held + spent` never passes the cap), at most one intent
per receipt, an unchanged ledger after a refusal, and a recount of the
accepted events that reproduces the ledger exactly.

<!-- source: psionic/lib/peer/pay.coax check-oracle -->
```jai
// A brute-force ledger: two receipts, A quoted 300 msat and owed 200, B
// quoted 500 and owed 500, under a 750 msat cap and a 400 msat payout cap.
// Three operations from seven kinds on either receipt: hold, agree,
// disagree, unverifiable, unsampled, owner pays, owner withholds. The
// oracle tracks each receipt's state and three sums by hand; the ledger
// must match it after every step, conserve the cap, never pay a receipt
// twice, leave itself unchanged on a refusal, and recount to the same sums.
#forall "the ledger against a brute-force oracle" trust: bool, o1: 0..<14, o2: 0..<14, o3: 0..<14 #steps 200000000 {
    quote:=[2]int.[300,500];owe:=[2]int.[200,500];key:=[2]string.[VPT_A,VPT_B];
    p:=VpPolicy.{trust_unsampled=trust,payout_cap=400};
    l:=VpLedger.{account=BlAccount.{budget=750}};
    st:=[2]VpState.[VpState.none,VpState.none];held:=0;spent:=0;withheld:=0;
    ev:[3]VpEvent;ne:=0;
    for op:[3]int.[o1,o2,o3] {
        r:=op%2;kind:=op/2;
        e:=VpEvent.{kind=VpKind.decide,receipt=key[r],msat=owe[r],sampled=true};
        want:=VpRefuse.none;to:=st[r];
        if kind==0 {
            e=VpEvent.{kind=VpKind.hold,receipt=key[r],payee="p",msat=quote[r]};
            if st[r]!=VpState.none {want=VpRefuse.twice;}
            else if held+spent+quote[r]>750 {want=VpRefuse.budget;}
            else {to=VpState.held;}
        } else if kind<=4 {
            v:=[5]PkVerdict.[PkVerdict.none,PkVerdict.agree,PkVerdict.disagree,PkVerdict.unverifiable,PkVerdict.none];
            e.verdict=v[kind];e.sampled=kind<4;
            if st[r]==VpState.none {want=VpRefuse.unknown;}
            else if st[r]!=VpState.held {want=VpRefuse.settled;}
            else if kind==2 {to=VpState.withheld;}
            else if kind==3 || (kind==4 && !trust) || owe[r]>400 {to=VpState.review;}
            else {to=VpState.paid;}
        } else {
            e=VpEvent.{kind=VpKind.review,receipt=key[r],pay=kind==5};
            if st[r]==VpState.none {want=VpRefuse.unknown;}
            else if st[r]!=VpState.review {want=VpRefuse.settled;}
            else {to=ifx kind==5 then VpState.paid else VpState.withheld;}
        }
        t:=vp_apply(l,p,e);
        #assert t.refuse==want;
        if want!=VpRefuse.none {#assert vpt_same(t.ledger,l);continue;}
        if to==VpState.held {held+=quote[r];}
        if to==VpState.paid {held-=quote[r];spent+=owe[r];}
        if to==VpState.withheld {held-=quote[r];withheld+=owe[r];}
        st[r]=to;l=t.ledger;ev[ne]=e;ne+=1;
        #assert t.state==to;
    }
    #assert l.account.held==held && l.account.spent==spent && l.withheld==withheld && held+spent<=750;
    // Conservation: escrow is the quotes of jobs held or in review; spending
    // is the sum of the intents; each receipt has at most one intent.
    esc:=0;sum:=0;
    for j:0..<l.n {if l.jobs[j].state==VpState.held || l.jobs[j].state==VpState.review {esc+=l.jobs[j].quote;}}
    for i:0..<l.m {sum+=l.intents[i].msat;for k:0..<i {#assert l.intents[k].at!=l.intents[i].at;}}
    #assert esc==held && sum==spent;
    for r:0..<2 {k:=vp_find(l,key[r]);#assert (k<0)==(st[r]==VpState.none) && (k<0 || l.jobs[k].state==st[r]);}
    c:=vp_recount(750,p,ev[0..<ne]);#assert c.refuse==VpRefuse.none && vpt_same(c.ledger,l);
}
```

Further checks pin every hold refusal in order, the digest format at each
character boundary, the ledger's own copy of each receipt and payee when
the caller writes the next job into the same buffers, the 64-byte payee
bound, the verdict, policy and payout-cap rules, amounts at 21
million bitcoin in both units, the withheld total's bound, the Spark
whole-sat rule and the recount's refusals.

`peer/pay.coax` is an `#authority` file. `coax mutate` changes each of the
536 mutation sites its checks reach, in this file and the ones it loads, in
turn, and the checks kill all 536. Run it
with the other Psionic units:

```shell
psionic/check.sh --only peer/pay
```

## What remains

- **Executing payments.** An intent says what to pay; it pays nothing. Paying
  through the Breez Spark wallet, and setting the cap and payout cap for a
  period, are owner actions recorded in `NEEDS_OWNER.md`. A ledger total
  here is owed money, never a settled payment, and no simulated payment is
  counted as paid.
- **Worker admission.** A worker declaring a capability only after a
  digest-checked self-test is not built.
- **A paid validator.** Here the buyer runs the checker in process. A
  validator on a separate device, paid for its own replay, needs its own
  receipts and intents.
- **Trace-machine work.** The verified work is chapter 195's binary32 stage.
  Paying for traces that [chapter 234](ch234-exact-replay-as-a-verification-class.md)
  replays exactly, on programs beyond a fixture, is not wired in.
- **Signed receipts and a schema.** Receipts are chapter 195's MACed text,
  and the event log is this chapter's in-process layout. Receipts a third
  party can verify need signatures and a published schema in `proto/`.
- **Periods.** The cap covers one period; resetting it each day is not
  built.

## Sources

- [Chapter plan](PLAN.md), Part XXIII.
- [Chapter 195](ch195-check-a-peers-work.md) and
  [chapter 180](ch180-observe-meter-and-price.md).
- [The Breez Spark wallet plan](../../docs/lightning/README.md).
- [Exact computation notes](../docs/exact-computation.md).
