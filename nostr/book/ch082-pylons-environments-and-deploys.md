# 82. Pylons, environments and deploys

A Pylon is a machine that serves models to buyers. It says in public that
it is online with a signed beacon. A buyer that used it can say in public
that a job finished with a signed receipt. This chapter reads both records
the way a buyer, a pool and a world read them: it verifies the event,
admits or refuses the body, judges the beacon's freshness by the reader's
own clock, and decides what a beacon adds to a pool and which beacon a
receipt counts for.

Part X, Every NIP. Result: NIP-PYLON beacons (kind 30200) and service
receipts (kind 3201) read and verified, the freshness rule that keeps an
expired beacon out of a pool's capacity, and the rule that ties a receipt to
the beacon it credits. NIP-ENV and NIP-DEPLOY are described, not built; see
[What is not built](#what-is-not-built).

## What this chapter teaches

- The beacon: its content, its `d`, `t`, `x` and `expiration` tags, and the
  reader's refusals.
- Freshness by the reader's clock: future, fresh, stale, rollback and not
  newer, and what each shows.
- Capacity: an expired or offline beacon adds nothing to a pool.
- The receipt: the buyer signs it, and its `p` and `a` tags name the beacon
  it credits.
- Which receipts a reader refuses, and which it admits but never counts for
  a given beacon.

## Where the code is

| File | Procedures | What they decide |
| --- | --- | --- |
| [beacon.coax](../../psionic/lib/pylon/beacon.coax) | `pb_content`, `pb_write`, `pb_read`, `pb_fresh`, `pb_show`, `pb_routable`, `pb_start` to `pb_next` | The beacon's bytes, its admission, its freshness, and the provider's republish schedule. `#authority` |
| [host.coax](../../psionic/lib/pylon/host.coax) | `ph_digest`, `ph_beacon`, `ph_read_beacon` | The same calls with the real SHA-256 of the content |
| [receipt.coax](../../psionic/lib/mesh/receipt.coax) | `mr_body`, `mr_tags`, `mr_read`, `mr_check_tags`, `mr_check_read` | The receipt's bytes and admission, and NIP-32 check labels. `#authority` |
| [totals.coax](../../psionic/lib/mesh/totals.coax) | `mt_recompute` | A pool aggregate's totals from admitted beacons, receipts and labels. `#authority` |
| [event.coax](../../coax/lib/nostr/event.coax) | `nostr_event_verify_vartime` | The event's id and BIP-340 signature (chapters 11 and 21) |

None of the Pylon readers verifies a signature. Each takes the signer as an
argument and expects the caller to have verified the whole event first, as
the example does with `nostr_event_verify_vartime`. Admission is not trust:
a beacon is the provider's claim, and a receipt is the buyer's claim.

## The beacon

A provider publishes a kind 30200 beacon while its pylon takes work, and
again with `status` `offline` when it stops. The content is JCS JSON (keys
sorted, no spaces) with these fields:

| Field | Value in the example | Rule |
| --- | --- | --- |
| `v` | `openagents.pylon-beacon.v1` | Exactly this version. |
| `requires` | `[]` | Empty in v1. |
| `provider` | `f9308a01...36f9` | The signer. |
| `pylon` | `studio-gpu` | A slug of 1 to 64 bytes of `[a-z0-9_-]`, equal to the `d` tag. |
| `label` | `Studio GPU` | Inert text, at most 64 bytes. |
| `status` | `online` | `online`, `draining` or `offline`. |
| `generation` | 7 | Rises when the pylon restarts or updates. |
| `since`, `observed_at`, `valid_until` | 1791380000, 1791400000, 1791400300 | `since <= observed_at <= valid_until <= observed_at + 300`. |
| `class` | `gpu`, `medium`, 16 GB | A coarse hardware class. |
| `slots` | 1 total, 1 free | `0 <= free <= total <= 64`. |
| `services` | one `cj-decision` service | 1 to 16 entries, each a capability, a model, lanes, an offering and a price hint. |
| `settlement`, `pools` | `["free-v1"]`, `[]` | Payment profiles; pools the provider asks to join. |

The tags are exactly one each of `d` (the pylon), `t`
(`oa:pylon-beacon:v1`), `x` (the SHA-256 of the content bytes) and
`expiration` (equal to `valid_until`), plus an optional NIP-OA `auth` tag
from the owner. `pb_read` refuses with one code per rule:

| Code | Refused when |
| --- | --- |
| `kind` | The kind is not 30200. |
| `tags` | A required tag is missing, repeated or the wrong size, or `expiration` is not a whole number. |
| `marker`, `digest` | The `t` marker is not `oa:pylon-beacon:v1`, or `x` is not the content's SHA-256. |
| `auth` | The `auth` tag has the wrong shape. Its signature is carried, not verified. |
| `json`, `field`, `version` | The content is not one object, has an unknown or repeated field, or names another version. |
| `provider` | `provider` is not the signer. |
| `slug` | `pylon` is not a slug, or differs from `d`. |
| `times` | The times break `since <= observed_at <= valid_until <= observed_at + 300`. |
| `expiration` | The `expiration` tag differs from `valid_until`. |
| `class`, `slots`, `services`, `lanes`, `identity`, `settlement`, `pools`, `label`, `status` | That field is out of range. |

### Worked example: the beacon, by hand

The example's beacon is chapter 24's fixture pylon, signed with the test
scalar 3. Check it before the program runs:

1. The content is 657 bytes. Its keys are in JCS order: `class`,
   `generation`, `label`, `observed_at`, `pools`, `provider`, `pylon`,
   `requires`, `services`, `settlement`, `since`, `slots`, `status`, `v`,
   `valid_until`. A reader that re-sorts the keys gets the same bytes.
2. `x` is the SHA-256 of those 657 bytes:
   `a200d2649d0fdabc548e70175b18b0f274674b21f587fae8bc4c5d1be9797664`.
3. `valid_until - observed_at = 1791400300 - 1791400000 = 300`, the most
   NIP-PYLON allows. The `expiration` tag is the string `"1791400300"`,
   equal to `valid_until`.
4. `provider` is `f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9`,
   the x-coordinate of 3·G, which is the event's `pubkey`.
5. `pylon` is `studio-gpu`, equal to `d`. The beacon's NIP-33 address is
   `30200:f9308a01...36f9:studio-gpu`; a receipt names the pylon by it.
6. Uptime is `observed_at - since = 20000` seconds, 5 h 33 min 20 s.
7. The NIP-01 id is the SHA-256 of
   `[0,"f9308a01...36f9",1791400000,30200,[["d","studio-gpu"],...],"<content>"]`:
   `5e8c454542809f9066eb59e6f944b697b6a57008230eefb3b70812bc6a4a057f`.
8. The signature's first 32 bytes are `r = c9c6479c...0e32cb`. BIP-340
   accepts it when `s·G - e·P` has x-coordinate `r` and an even y, with
   `e` the tagged hash `BIP0340/challenge` of `r || P || id` (chapter 11).

### Freshness

A reader judges a beacon by the time it received it, not by the pylon's
clock. `pb_fresh` applies NIP-PYLON's rules in order:

<!-- source: psionic/lib/pylon/beacon.coax fresh -->
```jai
pb_fresh :: (r:PbRead,received:int,held:PbHeld)->PbFresh #export_c {
    if r.observed_at>received+PB_FUTURE {return PbFresh.future;}
    if held.present {
        if r.generation<held.generation {return PbFresh.rollback;}
        if r.generation==held.generation && r.observed_at<=held.observed_at {return PbFresh.not_newer;}
    }
    if received>r.valid_until || received>r.observed_at+PB_WINDOW {return PbFresh.stale;}
    return PbFresh.fresh;
}
pb_show :: (r:PbRead,f:PbFresh)->PbShow {
    if f!=PbFresh.fresh {return PbShow.unknown;}return cast(PbShow)cast(int)r.status;
}
// Whether a buyer may send this pylon a decision job now.
pb_routable :: (r:PbRead,f:PbFresh)->bool #export_c {
    return r.code==PbCode.ok && f==PbFresh.fresh && r.status==PbStatus.online && r.free>0 && r.decision;
}
```

1. A beacon whose `observed_at` is more than 30 seconds after the reader's
   time is `future`.
2. Against a held beacon for the same pylon, a lower generation is
   `rollback`, and the same generation that is not newer is `not_newer`.
3. A beacon received after `valid_until`, or more than 300 seconds after
   `observed_at`, is `stale`.

Anything but `fresh` shows as `unknown`, never as online. A buyer routes a
decision job only to a fresh, online beacon with a free slot and a decision
service (`pb_routable`).

### Worked example: freshness, by hand

The beacon has `observed_at = T0` and `valid_until = T0 + 300`, with
`T0 = 1791400000`.

| Received | Rule | Result |
| --- | --- | --- |
| `T0 - 30` | `observed_at > received + 30` is `T0 > T0`, false | fresh |
| `T0 - 31` | `T0 > T0 - 1`, true | future |
| `T0 + 300` | `received > valid_until` is `T0 + 300 > T0 + 300`, false | fresh |
| `T0 + 301` | `T0 + 301 > T0 + 300`, true | stale |
| `T0 + 10`, generation 8 held | `7 < 8` | rollback |

### Capacity

A pool's capacity is the slots of its pylons that are online now. An
expired beacon contributes no capacity: the pylon may have stopped, and the
reader can't tell. The example's `capacity_at` states the rule for one
beacon at the reader's time:

<!-- source: nostr/examples/pylon-receipts.coax capacity -->
```jai
// What one verified beacon adds to a pool's capacity at the reader's time
// now: its slots while it is fresh and not offline, nothing otherwise. The
// aggregate rule (NIP-PYLON 30201 totals, psionic/lib/mesh/totals.coax's
// mt_recompute) counts the same beacons at window.to.
Capacity :: struct {pylons:int;total:int;free:int;}
capacity_at :: (b:PbRead,now:int)->Capacity {
  c:Capacity;f:=pb_fresh(b,now,PbHeld.{});
  if b.code!=PbCode.ok || f!=PbFresh.fresh || b.status==PbStatus.offline {return c;}
  c.pylons=1;c.total=b.total;c.free=b.free;return c;
}
```

So the beacon adds 1 pylon and 1 of 1 free slots at `T0 + 300`, and nothing
at `T0 + 301`. An offline beacon adds nothing; a draining beacon still
counts its slots but takes no new jobs.

A pool aggregate (kind 30201) applies the same rule at the end of its
window: its `pylons_online`, `slots_total` and `slots_free` come from the
newest fresh online or draining beacon of each counted pylon at
`window.to`. `mt_recompute` in `totals.coax` skips a beacon whose
`valid_until` is before `window.to`. Its check "newer offline or expired
beacons tombstone old capacity; future observations do not" asserts that a
newer beacon with `valid_until = 119` adds nothing to a window ending at
120, and hides the older beacon of the same pylon too. Psionic
[chapter 26](../../psionic/book/ch026-check-the-mesh.md) builds the
aggregate.

## The receipt

A buyer publishes one kind 3201 receipt when a job ends. The buyer signs
it; the pylon never does. NIP-PYLON's reason: a receipt credits the pylon,
so a pylon that could sign its own receipts could invent work. The tags are
exactly `p` (the provider's key), `a` (the beacon's address), `t`
(`oa:pylon-receipt:v2`) and `x` (the content's SHA-256).

`receipt.coax` reads the free decision profile that the Psionic mesh
publishes: 15 fields, lane `cj-decision`, outcome `accepted`, one `jobs`
unit and a null payment. `mr_read` takes the verified signer and refuses
unless every check holds:

<!-- source: psionic/lib/mesh/receipt.coax read -->
```jai
mr_read :: (kind:int,signer:string,created_at:int,tags:string,content:string)->MrReceipt #export_c {
    bad:MrReceipt;if kind!=MR_KIND {return bad;}r:=mr_body_read(content,created_at);if !r.ok || r.buyer!=signer {return bad;}
    p:=pt_tag(tags,"p");a:=pt_tag(tags,"a");t:=pt_tag(tags,"t");x:=pt_tag(tags,"x");
    if mr_array_count(tags)!=4 || p.size!=2 || a.size!=2 || t.size!=2 || x.size!=2 {return bad;}
    hex:[64]u8;hashed:=ph_digest(content,hex[..]);address:[160]u8;expected:=mr_address(r.provider,r.pylon,address[..]);
    if p.v[1]!=r.provider || a.v[1]!=expected || t.v[1]!=MR_MARKER || x.v[1]!=cast(string)hex[..] {return bad;}return r;
}
```

1. The kind is 3201, and the body reads: JCS, at most 4096 bytes, exactly
   the 15 fields, `v` `openagents.pylon-receipt.v2`, the capability
   `<provider>:pylon/decision`, and `started_at <= finished_at`.
2. `buyer` is the signer, and `buyer` is not `provider`.
3. There are exactly four tags, each of size 2: `p` is the provider, `a` is
   `30200:<provider>:<pylon>`, `t` is the marker, and `x` is the SHA-256 of
   the content.

`mr_read` returns only admitted or refused, with no reason code. Paid
receipts, other outcomes and units, `meta`, and attested receipts are
refused, not relabeled free.

### Worked example: the receipt, by hand

The buyer (test scalar 1) reports one decision job on `studio-gpu`:

1. The content is 702 bytes with the 15 fields in JCS order, from `buyer`
   to `v`.
2. `buyer` is `79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798`,
   the x-coordinate of 1·G, which is the event's `pubkey`. `provider` is
   3·G's `f9308a01...36f9`, a different key.
3. `started_at = T0 + 100` and `finished_at = T0 + 102`, so the job took
   2 seconds and `finished_at >= started_at`.
4. `p` is `f9308a01...36f9`, the provider. `a` is `"30200:"` +
   `f9308a01...36f9` + `":"` + `studio-gpu`, the beacon's address from
   step 5 of the beacon.
5. `x` is the SHA-256 of the 702 bytes:
   `e23e991fd9e601d8c0cecdccd5e82e3a5047125114f541d5ef107cabbc06b37b`.
6. The id is
   `d3645062fa1ee20d15cb68853b4adbdb05565e4c94097472d79d35028cabbd88`.
   Changing one digit of `started_at` changes the preimage, so the stated
   id no longer matches: the reader refuses with `id` before it tries the
   signature.

### Which beacon a receipt credits

[PLAN.md](PLAN.md) states this chapter's second check as "a receipt signed
by a key other than the beacon's refuses". NIP-PYLON puts it the other way
round: the receipt must be signed by its buyer, and it credits the pylon
whose key and slug its `p` and `a` tags name. `mr_read` has already bound
`p` and `a` to the body's `provider` and `pylon`, so the example compares
those with the beacon:

<!-- source: nostr/examples/pylon-receipts.coax attach -->
```jai
// A receipt counts for a beacon only when its p and a tags (which mr_read
// has already bound to the body) name the beacon's signer and pylon.
attaches :: (r:MrReceipt,beacon_signer:string,b:PbRead)->bool {
  return r.ok && b.code==PbCode.ok && r.provider==beacon_signer && r.pylon==b.pylon;
}
```

The example checks each case:

| Receipt | `mr_read` | Counts for the beacon |
| --- | --- | --- |
| Signed by its buyer, key 1, for `studio-gpu` of key 3 | admitted | yes |
| The same bytes signed by key 2 | refused: the signer is not the buyer | no |
| The same bytes signed by the pylon's key 3 | refused: the signer is not the buyer | no |
| Signed by key 1 for `studio-gpu` of key 2 | admitted | no: `p` and `a` name another key |

The writer refuses the remaining case: `mr_body` returns -1 for a receipt
whose buyer is its provider, so a pylon can't write a receipt for its own
job.

## What is not built

- **Model-group beacons (v2).** `beacon.coax` reads v1 only; a v2 version or
  marker refuses as `version` or `marker`. Groups, stages and heads are not
  implemented.
- **The owner's `auth` tag.** The NIP-OA signature is carried in `PbRead`,
  not verified, so a reader can't yet refuse a receipt whose buyer is the
  beacon's owner.
- **Paid and other receipts.** Payment acknowledgments, outcomes other than
  `accepted`, units other than one job, and attested receipts are refused.
- **Pool aggregate events.** `totals.coax` recomputes the totals from
  admitted records and `pool-wire.coax` matches a 30201 body against them;
  this chapter doesn't sign or read a 30201 event.
- **NIP-ENV.** Environment definitions, lease admission, expiry and binding
  an execution to a lease before dispatch. NIP-ENV adds no event kinds: its
  requests and results travel as private 3188 artifacts and RUN records.
  Nothing in Coax implements it.
- **NIP-DEPLOY.** Public deploy status: kind 3205 for each phase of a run
  and addressable kind 30205 for each environment's newest phase (not 30440, as
  this chapter's draft said), with `env`, `tier`, `phase`, `run` and optional
  `commit` fields, from one announcer key. The NIP's implementation was in
  the Rust workspace that the reset removed; nothing in Coax implements it.

## The example

[nostr/examples/pylon-receipts.coax](../examples/pylon-receipts.coax) reads
seven signed events with the test keys 1 (the buyer), 2 (another key) and
3 (the pylon). The events were signed once with `event_sign.coax` and aux
0, from what `pb_content` and `mr_body`/`mr_tags` write, and are copied
into the program as a relay would send them. The program first shows that
the writers still produce the signed content. It then verifies every event
(id and BIP-340 signature), reads the beacon and the receipt, walks the
freshness boundaries, asks what the beacon adds to the pool at three times,
and runs the refusals.

<!-- source: nostr/examples/pylon-receipts.coax example -->
```jai
T0 :: 1791400000;   // the beacon's observed_at (chapter 24's fixture)
BUYER :: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";   // scalar 1
OTHER :: "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";   // scalar 2
PYLON :: "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";   // scalar 3, PB_KEY

CODE_WORDS :: [22]string.["ok","kind","tags","marker","digest","json","version","field","provider","slug","label",
  "status","times","expiration","class","slots","services","lanes","identity","settlement","pools","auth"];
FRESH_WORDS :: [5]string.["fresh","future","stale","rollback","not newer"];
SHOW_WORDS :: [4]string.["online","draining","offline","unknown"];
NE_WORDS :: [9]string.["ok","json","shape","limit","hex","output","alias","id","signature"];

to_hex :: (bytes:[]u8,out:[]u8)->string {r:=hex_encode_checked(cast(string)bytes,out,false);return cast(string)out[0..<r.written];}

// What a reader holds after verifying one event: its id, signer, kind,
// time, tags and unescaped content. error is NE_OK only when the id
// recomputes from the fields and the signature verifies under pubkey.
Opened :: struct {error:int;id:string;signer:string;kind:int;created_at:int;tags:string;content:string;}
Bufs :: struct {id:[64]u8;signer:[64]u8;content:[2048]u8;}
open_event :: (event:string,b:[]Bufs)->Opened {
  o:Opened;scratch:[16384]u8;e:=nostr_event_verify_vartime(event,scratch[..],false);o.error=e.error;
  if e.error!=NE_OK {return o;}
  o.id=to_hex(e.id[..],b[0].id[..]);o.signer=to_hex(e.public_key[..],b[0].signer[..]);o.kind=e.kind;o.created_at=e.created_at;
  o.tags=event[e.tags_from..<e.tags_to];
  n:=json_unescape(event[e.content_from..<e.content_to],b[0].content[..]);#assert n>=0;o.content=cast(string)b[0].content[0..<n];
  return o;
}
// The receipt the buyer wrote: one accepted decision job on studio-gpu.
the_receipt :: ()->MrReceipt {
  return MrReceipt.{ok=true,buyer=BUYER,provider=PYLON,pylon="studio-gpu",request=MR_REQUEST,request_digest=MR_REQUEST_DIGEST,
    result_digest=MR_RESULT_DIGEST,started_at=T0+100,finished_at=T0+102,created_at=T0+103};
}
// A copy of text with the byte at i replaced by c.
altered :: (text:string,i:int,c:u8,out:[]u8)->string {for k:0..<text.count {out[k]=text[k];}out[i]=c;return cast(string)out[0..<text.count];}
```

<!-- source: nostr/examples/pylon-receipts.coax main -->
```jai
main :: () {
  print("the pylon's beacon (key 3)\n");
  lanes:[1]string;services:[1]PbService;settlement:[1]string;pb:=pb_fixture(lanes[..],services[..],settlement[..]);
  body:[2048]u8;n:=pb_content(pb,body[..]);content:=cast(string)body[0..<n];
  x:[64]u8;#assert ph_digest(content,x[..]);
  bb:[1]Bufs;ob:=open_event(BEACON,bb[..]);
  print("verify: %, id %, signer %, kind %, created_at T0 + %\n",NE_WORDS[ob.error],ob.id,ob.signer,ob.kind,ob.created_at-T0);
  print("tags %\n",ob.tags);
  print("content (% bytes) is what pb_content writes: %\n",ob.content.count,ob.content==content);
  print("x = sha256(content) = %\n",cast(string)x[..]);
  b:=ph_read_beacon(ob.kind,ob.signer,ob.tags,ob.content);
  print("read: %, %, generation %, slots %/% free, class %/%/% GB, valid_until T0 + %\n",CODE_WORDS[cast(int)b.code],
    SHOW_WORDS[cast(int)b.status],b.generation,b.free,b.total,b.family,b.tier,b.memory_gb,b.valid_until-T0);

  print("freshness by the reader's clock\n");
  none:=PbHeld.{};
  for at:[5]int.[-30,10,300,301,-31] {
    f:=pb_fresh(b,T0+at,none);
    print("received at T0 % % s: %, shown %, routable %\n",ifx at<0 then "-" else "+",ifx at<0 then -at else at,
      FRESH_WORDS[cast(int)f],SHOW_WORDS[cast(int)pb_show(b,f)],pb_routable(b,f));
  }
  held:=PbHeld.{present=true,generation=8,observed_at=T0-500};
  print("generation 8 already held, received at T0 + 10: %\n",FRESH_WORDS[cast(int)pb_fresh(b,T0+10,held)]);

  print("the buyer's receipt (key 1)\n");
  rb:[1]Bufs;orc:=open_event(RECEIPT,rb[..]);
  print("verify: %, id %, signer %, kind %, created_at T0 + %\n",NE_WORDS[orc.error],orc.id,orc.signer,orc.kind,orc.created_at-T0);
  print("tags %\n",orc.tags);
  mine:[4096]u8;m:=mr_body(the_receipt(),mine[..]);
  print("content (% bytes) is what mr_body writes: %\n",orc.content.count,orc.content==cast(string)mine[0..<m]);
  r:=mr_read(orc.kind,orc.signer,orc.created_at,orc.tags,orc.content);
  print("read: %, buyer is the signer: %, finished T0 + %, attaches to the beacon: %\n",ifx r.ok then "admitted" else "refused",
    r.buyer==orc.signer,r.finished_at-T0,attaches(r,ob.signer,b));

  print("what the beacon adds to the pool\n");
  for at:[3]int.[200,300,301] {
    c:=capacity_at(b,T0+at);
    print("at T0 + %: pylons online %, slots total %, free %\n",at,c.pylons,c.total,c.free);
  }

  print("refusals\n");
  b2:[1]Bufs;o:=open_event(BEACON_BY_OTHER,b2[..]);
  print("the beacon's bytes signed by key 2: verify %, read %\n",NE_WORDS[o.error],CODE_WORDS[cast(int)ph_read_beacon(o.kind,o.signer,o.tags,o.content).code]);
  b3:[1]Bufs;o=open_event(BEACON_LATE_EXPIRATION,b3[..]);
  print("expiration tag T0 + 400, valid_until T0 + 300: verify %, read %\n",NE_WORDS[o.error],CODE_WORDS[cast(int)ph_read_beacon(o.kind,o.signer,o.tags,o.content).code]);
  out:[4096]u8;
  forged:=altered(RECEIPT,RECEIPT.count-3,ifx RECEIPT[RECEIPT.count-3]=='0' then cast(u8)'1' else cast(u8)'0',out[..]);
  print("the receipt with one signature digit changed: verify %\n",NE_WORDS[open_event(forged,b3[..]).error]);
  at:=pb_index(RECEIPT,"\\\"started_at\\\":1791400100");#assert at>=0;
  late:=altered(RECEIPT,at+24,'9',out[..]);
  print("the receipt with started_at changed to 1791400109: verify %\n",NE_WORDS[open_event(late,b3[..]).error]);
  for which:0..<3 {
    event:=ifx which==0 then RECEIPT_BY_OTHER else ifx which==1 then RECEIPT_BY_PYLON else RECEIPT_OTHER_PYLON;
    label:=ifx which==0 then "the receipt's bytes signed by key 2" else ifx which==1 then "the receipt's bytes signed by the pylon's key 3" else "a receipt by key 1 for key 2's studio-gpu";
    bo:[1]Bufs;o=open_event(event,bo[..]);v:=mr_read(o.kind,o.signer,o.created_at,o.tags,o.content);
    print("%: verify %, read %, attaches %\n",label,NE_WORDS[o.error],ifx v.ok then "admitted" else "refused",attaches(v,ob.signer,b));
  }
  self:=the_receipt();self.buyer=PYLON;
  print("the pylon writing a receipt for its own job: mr_body %\n",mr_body(self,mine[..]));
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/pylon-receipts.coax --run-steps 250000000
```

It prints this, in debug and release, and exits with status 0:

```text
the pylon's beacon (key 3)
verify: ok, id 5e8c454542809f9066eb59e6f944b697b6a57008230eefb3b70812bc6a4a057f, signer f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9, kind 30200, created_at T0 + 0
tags [["d","studio-gpu"],["t","oa:pylon-beacon:v1"],["x","a200d2649d0fdabc548e70175b18b0f274674b21f587fae8bc4c5d1be9797664"],["expiration","1791400300"]]
content (657 bytes) is what pb_content writes: true
x = sha256(content) = a200d2649d0fdabc548e70175b18b0f274674b21f587fae8bc4c5d1be9797664
read: ok, online, generation 7, slots 1/1 free, class gpu/medium/16 GB, valid_until T0 + 300
freshness by the reader's clock
received at T0 - 30 s: fresh, shown online, routable true
received at T0 + 10 s: fresh, shown online, routable true
received at T0 + 300 s: fresh, shown online, routable true
received at T0 + 301 s: stale, shown unknown, routable false
received at T0 - 31 s: future, shown unknown, routable false
generation 8 already held, received at T0 + 10: rollback
the buyer's receipt (key 1)
verify: ok, id d3645062fa1ee20d15cb68853b4adbdb05565e4c94097472d79d35028cabbd88, signer 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798, kind 3201, created_at T0 + 103
tags [["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["a","30200:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:studio-gpu"],["t","oa:pylon-receipt:v2"],["x","e23e991fd9e601d8c0cecdccd5e82e3a5047125114f541d5ef107cabbc06b37b"]]
content (702 bytes) is what mr_body writes: true
read: admitted, buyer is the signer: true, finished T0 + 102, attaches to the beacon: true
what the beacon adds to the pool
at T0 + 200: pylons online 1, slots total 1, free 1
at T0 + 300: pylons online 1, slots total 1, free 1
at T0 + 301: pylons online 0, slots total 0, free 0
refusals
the beacon's bytes signed by key 2: verify ok, read provider
expiration tag T0 + 400, valid_until T0 + 300: verify ok, read expiration
the receipt with one signature digit changed: verify signature
the receipt with started_at changed to 1791400109: verify id
the receipt's bytes signed by key 2: verify ok, read refused, attaches false
the receipt's bytes signed by the pylon's key 3: verify ok, read refused, attaches false
a receipt by key 1 for key 2's studio-gpu: verify ok, read admitted, attaches false
the pylon writing a receipt for its own job: mr_body -1
```

The refusals:

- The beacon's bytes signed by key 2 verify as an event, and `pb_read`
  refuses them with `provider`: the content names key 3.
- An `expiration` tag of `T0 + 400` beside `valid_until = T0 + 300` is
  refused with `expiration`, although the signature is good.
- One changed signature digit fails BIP-340: `signature`.
- One changed digit of `started_at` fails the id: `id`.
- The receipt signed by key 2 or by the pylon's key 3 is refused, and the
  good receipt for key 2's pylon is admitted but doesn't count for this
  beacon.
- `mr_body` returns -1 for a receipt whose buyer is the pylon.

## How the code is checked

### Checks in the example

[checks/pylon-receipts.coax](../examples/checks/pylon-receipts.coax) holds
the independent values. A Python script written from NIP-01 and BIP-340
(hashlib, and the integer BIP-340 in
[nip59_reference.py](../../coax/tests/nostr/nip59_reference.py), which
shares no code with Coax) computed every event's content digest and id,
reproduced every signature with aux 0, and verified each one. The checks
compare those values with Coax's, and assert the hand values of the worked
examples and both Done-means rules:

<!-- source: nostr/examples/checks/pylon-receipts.coax check -->
```jai
// From a Python script written from NIP-01 and BIP-340 that shares no code
// with Coax (hashlib, plus coax/tests/nostr/nip59_reference.py's integer
// BIP-340): for each event, the SHA-256 of its content (the x tag), the
// SHA-256 of [0,pubkey,created_at,kind,tags,content] (the id), its content
// length, and that the reference signer with aux 0 reproduces every sig and
// the reference verifier accepts it. The signatures are checked there and in
// the example's run; here each id is recomputed without the slow verify.
PY_EVENTS :: [7]string.[BEACON,BEACON_BY_OTHER,BEACON_LATE_EXPIRATION,RECEIPT,RECEIPT_BY_OTHER,RECEIPT_BY_PYLON,RECEIPT_OTHER_PYLON];
PY_ID :: [7]string.[
  "5e8c454542809f9066eb59e6f944b697b6a57008230eefb3b70812bc6a4a057f",
  "a7c49b5162a2e4cef079726a9974bd32df4378241d114fbe8f9bcf80415972f1",
  "35c087ad1ad5c46f31ac5dc83499436b087db7a1ef6ec28c5ccf56c8263650e5",
  "d3645062fa1ee20d15cb68853b4adbdb05565e4c94097472d79d35028cabbd88",
  "5f7de15664129fe06346a10aa21818a5b6b5476a6ea992afbb3884cb4d536fbc",
  "c911c4315e51d70b73bcc814152a0b0df13642e7018160bd6b4cec4c7ff134ca",
  "945c2dc64e1450091493d29a68e7814f4f951d044652e0e0d265cb9c9ea0441e"];
PY_X :: [7]string.[
  "a200d2649d0fdabc548e70175b18b0f274674b21f587fae8bc4c5d1be9797664",
  "a200d2649d0fdabc548e70175b18b0f274674b21f587fae8bc4c5d1be9797664",
  "a200d2649d0fdabc548e70175b18b0f274674b21f587fae8bc4c5d1be9797664",
  "e23e991fd9e601d8c0cecdccd5e82e3a5047125114f541d5ef107cabbc06b37b",
  "e23e991fd9e601d8c0cecdccd5e82e3a5047125114f541d5ef107cabbc06b37b",
  "e23e991fd9e601d8c0cecdccd5e82e3a5047125114f541d5ef107cabbc06b37b",
  "1f9be2db6c2d006f125025a6eb1987894be1454acc7e8bdf2fce9565beed2123"];
PY_LEN :: [7]int.[657,657,657,702,702,702,702];

// Read an event without its signature: the id must recompute.
read_only :: (event:string,b:[]Bufs)->Opened {
  o:Opened;scratch:[16384]u8;e:=nostr_event_read(event,scratch[..],false);o.error=e.error;
  if e.error!=NE_OK {return o;}
  o.id=to_hex(e.id[..],b[0].id[..]);o.signer=to_hex(e.public_key[..],b[0].signer[..]);o.kind=e.kind;o.created_at=e.created_at;
  o.tags=event[e.tags_from..<e.tags_to];
  n:=json_unescape(event[e.content_from..<e.content_to],b[0].content[..]);o.content=cast(string)b[0].content[0..<n];
  return o;
}

#test "every id and content digest matches the Python reference" {
  for i:0..<7 {
    b:[1]Bufs;o:=read_only(PY_EVENTS[i],b[..]);#assert o.error==NE_OK && o.id==PY_ID[i] && o.content.count==PY_LEN[i];
    x:[64]u8;#assert ph_digest(o.content,x[..]) && cast(string)x[..]==PY_X[i];
    #assert pt_tag(o.tags,"x").v[1]==PY_X[i];
  }
}
#test "the beacon by hand: a 300-second window, the expiration tag, the slots" {
  b:[1]Bufs;o:=read_only(BEACON,b[..]);r:=ph_read_beacon(o.kind,o.signer,o.tags,o.content);
  #assert r.code==PbCode.ok && r.valid_until-r.observed_at==300 && PB_WINDOW==300 && r.observed_at==o.created_at;
  #assert pt_tag(o.tags,"expiration").v[1]=="1791400300" && pt_tag(o.tags,"d").v[1]==r.pylon;
  #assert r.since==1791380000 && r.observed_at-r.since==20000;   // uptime 20000 s, 5 h 33 min 20 s
  // Freshness: observed_at may lead the reader by 30 s; valid_until is the last fresh second.
  #assert pb_fresh(r,T0-30,PbHeld.{})==PbFresh.fresh && pb_fresh(r,T0-31,PbHeld.{})==PbFresh.future;
  #assert pb_fresh(r,T0+300,PbHeld.{})==PbFresh.fresh && pb_fresh(r,T0+301,PbHeld.{})==PbFresh.stale;
  // An expired beacon contributes no capacity, an offline one neither.
  #assert capacity_at(r,T0+300).total==1 && capacity_at(r,T0+300).free==1 && capacity_at(r,T0+300).pylons==1;
  #assert capacity_at(r,T0+301).total==0 && capacity_at(r,T0+301).free==0 && capacity_at(r,T0+301).pylons==0;
  off:=r;off.status=PbStatus.offline;#assert capacity_at(off,T0+10).pylons==0;
  drain:=r;drain.status=PbStatus.draining;#assert capacity_at(drain,T0+10).total==1 && !pb_routable(drain,PbFresh.fresh);
  bad:=r;bad.code=PbCode.slots;#assert capacity_at(bad,T0+10).pylons==0;
}
#test "the beacon refusals: another signer, a late expiration tag" {
  b:[1]Bufs;o:=read_only(BEACON_BY_OTHER,b[..]);#assert o.signer==OTHER && ph_read_beacon(o.kind,o.signer,o.tags,o.content).code==PbCode.provider;
  o=read_only(BEACON_LATE_EXPIRATION,b[..]);#assert o.signer==PYLON && ph_read_beacon(o.kind,o.signer,o.tags,o.content).code==PbCode.expiration;
}
#test "the receipt binds the buyer as signer and the beacon by p and a" {
  b:[1]Bufs;o:=read_only(RECEIPT,b[..]);r:=mr_read(o.kind,o.signer,o.created_at,o.tags,o.content);
  #assert r.ok && r.buyer==BUYER && r.provider==PYLON && r.pylon=="studio-gpu" && r.finished_at-r.started_at==2;
  #assert pt_tag(o.tags,"p").v[1]==PYLON && pt_tag(o.tags,"a").v[1]=="30200:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:studio-gpu";
  bb:[1]Bufs;ob:=read_only(BEACON,bb[..]);beacon:=ph_read_beacon(ob.kind,ob.signer,ob.tags,ob.content);
  #assert attaches(r,ob.signer,beacon) && !attaches(r,OTHER,beacon);
  // Signed by a key other than the buyer: by key 2, and by the beacon's own key 3.
  o=read_only(RECEIPT_BY_OTHER,b[..]);#assert o.signer==OTHER && !mr_read(o.kind,o.signer,o.created_at,o.tags,o.content).ok;
  o=read_only(RECEIPT_BY_PYLON,b[..]);#assert o.signer==PYLON && !mr_read(o.kind,o.signer,o.created_at,o.tags,o.content).ok;
  // A good receipt for another pylon reads, but never counts for this beacon.
  o=read_only(RECEIPT_OTHER_PYLON,b[..]);v:=mr_read(o.kind,o.signer,o.created_at,o.tags,o.content);
  #assert v.ok && v.provider==OTHER && !attaches(v,ob.signer,beacon);
  // The writer refuses a receipt whose buyer is its provider.
  self:=the_receipt();self.buyer=PYLON;out:[4096]u8;#assert mr_body(self,out[..])==-1 && mr_body(the_receipt(),out[..])==702;
}
#test "a changed byte breaks the id before any signature is tried" {
  out:[4096]u8;at:=pb_index(RECEIPT,"\\\"started_at\\\":1791400100");#assert at>=0;
  b:[1]Bufs;#assert read_only(altered(RECEIPT,at+24,'9',out[..]),b[..]).error==NE_ID;
}
```

```shell
coax/bin/coax test nostr/examples/checks/pylon-receipts.coax --run-steps 250000000
coax/bin/coax mutate nostr/examples/checks/pylon-receipts.coax --root-only --run-steps 250000000
```

Run on 2026-11-01: the example in debug and release matches its
transcript; the checks file passes 367 of 367 checks (its five and the
loaded units' own); root-only mutation of the checks file kills 16 of 16
mutants; the Python reference reproduces all seven ids, digests and
signatures.

### The library's own checks

- `beacon.coax` passes 44 of 44 checks, `host.coax` 100 of 100 and
  `receipt.coax` 249 of 249, run on 2026-11-01. Their mutation runs were
  not repeated today; the `book:` markers this chapter added are comments.
- [psionic-mesh-receipt-2026-10-30.md](../../docs/verification/psionic-mesh-receipt-2026-10-30.md)
  records `receipt.coax`'s entry-source mutation run: 253 of 253 mutants
  killed, zero survivors.
- [2026-10-30-newland-coax-nostr.md](../../docs/verification/2026-10-30-newland-coax-nostr.md)
  records the homepage verifying beacons in the browser with the same
  reader.

## Summary

- A beacon is the provider's signed, short-lived claim: `provider` is the
  signer, `x` is the content's SHA-256, `expiration` equals `valid_until`,
  and `valid_until` is at most 300 seconds after `observed_at`.
- A reader judges freshness by its own clock: future past 30 seconds ahead,
  stale after `valid_until`, rollback and not newer against a held beacon.
  Only a fresh beacon shows as online.
- An expired or offline beacon adds nothing to a pool's capacity.
- A receipt is the buyer's signed claim. `mr_read` refuses one that its
  buyer didn't sign, and a receipt credits only the beacon its `p` and `a`
  tags name.
- NIP-ENV and NIP-DEPLOY are specified but not built in Coax.

## Dependencies

- [75. Hosts and reachability](ch075-hosts-and-reachability.md), whose
  freshness rule this chapter's beacons follow.
- [80. Jobs, labor and markets](ch080-jobs-labor-and-markets.md), still a
  design draft: the jobs a receipt reports. This chapter doesn't use its
  code.

## Sources

- [OpenAgents NIP-PYLON](../nips/openagents/NIP-PYLON.md)
- [OpenAgents NIP-ENV](../nips/openagents/NIP-ENV.md)
- [OpenAgents NIP-DEPLOY](../nips/openagents/NIP-DEPLOY.md)
- [OpenAgents NIP-REACH](../nips/openagents/NIP-REACH.md), for freshness.
- [NIP-01](../nips/official/01.md) and [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
