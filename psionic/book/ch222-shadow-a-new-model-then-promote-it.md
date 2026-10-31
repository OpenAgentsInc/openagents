# 222. Shadow a new model, then promote it

A router that picks an action for every request is replaced, sooner or later,
by a different model: a distilled student, a cheaper one, a newer one. Its
scores on a fixed suite are not enough to switch. The real traffic may not
look like the suite, and a model that scores the same can still answer
differently.

So the candidate first runs in shadow:
- **Both models answer every request.** Only the active model's answer is
  used. Both answers are logged, with no request content, and each record is
  joined to its outcome when the outcome arrives.
- **A report decides.** The candidate is promoted only if the two models
  agree often enough, and its errors on joined outcomes are not shown to be
  worse.
- **Activation is one compare-and-swap.** Candidates are immutable, named by
  the SHA-256 of their bytes. Switching needs the approved digest, a
  promoting report and the version the caller expects. On any refusal the
  active model stays as it was.

[Chapter 177](PLAN.md) will roll out new builds of one model behind a canary.
This chapter rolls out a different model.

```shell
coax/bin/coax run psionic/examples/shadow.coax
coax/bin/coax test psionic/lib/shadow.coax
```

```jai
active: router-v3 (d011bf535170), version 1; 4 models registered
shadowed beside router-v3 on 4000 requests:
  router-v4 (aa8225ec8987): disagrees on 309 of 4000 requests, 0.0773 + 2 SE = 0.0857 (limit 0.12)
    errors on 3193 joined: active 202, candidate 121; extra error -0.0254 + 2 SE = -0.0165 (margin 0.005) -> promote
  router-small (8b12cc2371a5): disagrees on 406 of 4000 requests, 0.1015 + 2 SE = 0.111 (limit 0.12)
    errors on 3193 joined: active 202, candidate 284; extra error 0.0257 + 2 SE = 0.0352 (margin 0.005) -> hold: regression
  router-other (991c711383f3): disagrees on 474 of 4000 requests, 0.1185 + 2 SE = 0.1287 (limit 0.12)
    errors on 3193 joined: active 202, candidate 204; extra error 0.0006 + 2 SE = 0.0123 (margin 0.005) -> hold: disagreement
  router-other after its first 1000 requests: 801 joined -> wait
approval: router-v4 aa8225ec8987 with report 421ab852e405
activate router-v4 expecting version 1: active is now router-v4 (aa8225ec8987), version 2
refused, each leaving the active model unchanged:
  router-small, whose report holds, with an approval of it: approval
  router-v4 again, by a caller who still expects version 1: version
  router-v4's bytes with one bit changed after approval: digest
  router-v4's report again, now that router-v4 itself is active: approval
  registering router-v4 again with other bytes: immutable
```

The [example](../examples/shadow.coax) shadows three candidates beside the
active `router-v3`:
- `router-v4` is better and is promoted.
- `router-small` agrees often enough, but its errors are clearly worse, so it
  is held.
- `router-other` makes as many errors as the active model, but makes them
  on different requests. Its disagreement is over the limit, so it is held.

Before 1,000 outcomes have joined, the report says `wait`. The last lines
try to switch anyway, in four wrong ways, and register a changed model under
an existing name.

## A simulated decision

<!-- source: psionic/lib/shadow.coax world -->
```jai
// SplitMix64 draws. Request i has its true answer (one of three), a
// difficulty, and whether its outcome ever arrives; a model's own draws come
// from its seed.
sh_bits :: (seed:u64,counter:u64)->u64 {
    z:=add_wrap(seed,mul_wrap(SH_GOLDEN,add_wrap(counter,1)));
    z=mul_wrap(z^(z>>30),0xbf58476d1ce4e5b9);z=mul_wrap(z^(z>>27),0x94d049bb133111eb);
    return z^(z>>31);
}
sh_uniform :: (seed:u64,counter:u64)->f64 {return f64(sh_bits(seed,counter)>>11)/9007199254740992.0;}
sh_truth :: (world:u64,i:int)->u8 {return cast(u8)cast(int)(3*sh_uniform(world,cast(u64)(3*i)));}
// A draw u reaches a probability p when u + p >= 1.
sh_joined :: (world:u64,i:int)->bool {return cast(int)(sh_uniform(world,cast(u64)(3*i+2))+SH_JOIN)>0;}
sh_answer :: (m:ShModel,world:u64,i:int)->u8 {
    truth:=sh_truth(world,i);
    score:=(1-m.weight)*sh_uniform(world,cast(u64)(3*i+1))+m.weight*sh_uniform(m.seed,cast(u64)i);
    if cast(int)(1+m.skill-score)>0 {return truth;}
    return cast(u8)((cast(int)truth+1+cast(int)(2*sh_uniform(m.seed,SH_WRONG+cast(u64)i)))%3);
}
```

Each request has a true answer (one of three), a difficulty, and a draw that
decides whether its outcome ever arrives (80 in 100 do).

A model is correct when its score is within its skill. The score is a mix of
the request's difficulty and the model's own draw. Two models with the same
skill but different mixes make the same number of errors on different
requests. That is the case a suite score can't see and a disagreement limit
can.

## Identity

<!-- source: psionic/lib/shadow.coax identity -->
```jai
// A model's bytes: its name, then skill, weight and seed as little-endian
// words. The digest of those bytes is the model's identity.
sh_blob :: (m:ShModel,out:[]u8)->int {
    if m.name.count==0 || m.name.count>SH_NAME_MAX || out.count<SH_BLOB {return 0;}
    words:=[3]u64.[f64_bits(m.skill),f64_bits(m.weight),m.seed];
    for w,k:words {for b:0..<8 {out[SH_NAME_MAX+8*k+b]=cast,trunc(u8)(w>>cast(u64)(8*b));}}
    for i:0..<SH_NAME_MAX {out[i]=ifx i<m.name.count then m.name[i] else 0;}
    return SH_BLOB;
}
sh_digest :: (bytes:[]u8)->[32]u8 {out:[32]u8;#assert sha256_hash_checked(bytes,out[..],false)==SHA_OK;return out;}
sh_same :: (a:[32]u8,b:[32]u8)->bool {for i:0..<32 {if a[i]!=b[i] {return false;}} return true;}
// Registering a name again with the same bytes changes nothing; with other
// bytes it is refused. A model is never edited in place.
sh_register :: (r:ShRegistry,blob:[]u8)->ShRegistry {
    out:=r;out.status=ShStatus.ok;
    if blob.count!=SH_BLOB {out.status=ShStatus.shape;return out;}
    digest:=sh_digest(blob);length:=0;
    for i:0..<SH_NAME_MAX {if blob[i]!=0 {length=i+1;}}
    if length==0 {out.status=ShStatus.shape;return out;}
    entries:=r.entries;
    for e:entries[0..<r.count] {
        if e.length==length && sh_name(e,blob) {
            if !sh_same(e.digest,digest) {out.status=ShStatus.immutable;}
            return out;
        }
    }
    if r.count==SH_MODELS_MAX {out.status=ShStatus.capacity;return out;}
    e:=ShEntry.{length=length,digest=digest};
    for i:0..<length {e.name[i]=blob[i];}
    out.entries[r.count]=e;out.count+=1;
    return out;
}
sh_name :: (e:ShEntry,blob:[]u8)->bool {for i:0..<e.length {if e.name[i]!=blob[i] {return false;}} return true;}
```

A model's bytes are its name, then its parameters as little-endian words.
The SHA-256 of those bytes is its identity.

Registering the same bytes again changes nothing. Different bytes under a
registered name are refused as `immutable`: a changed model needs a new name
and its own shadow run.

## The shadow log

<!-- source: psionic/lib/shadow.coax shadow -->
```jai
// Both models answer every request; only the active answer is acted on. A
// record holds the two answers and, once it arrives, the outcome (-1 while
// it is pending). No request content is logged.
sh_log :: (world:u64,active:ShModel,candidate:ShModel,records:[]ShRecord)->ShStatus {
    if records.count==0 {return ShStatus.shape;}
    for i:0..<records.count {
        r:=ShRecord.{request=i,active=sh_answer(active,world,i),candidate=sh_answer(candidate,world,i),outcome=-1};
        if sh_joined(world,i) {r.outcome=cast(int)sh_truth(world,i);}
        records[i]=r;
    }
    return ShStatus.ok;
}
```

## The report

<!-- source: psionic/lib/shadow.coax report -->
```jai
// Disagreement is measured over every request, errors only over joined ones,
// paired request by request. The candidate is promoted when enough outcomes
// have joined, the disagreement rate plus two standard errors is within the
// limit, and the mean extra error (candidate minus active) plus two standard
// errors is within the margin.
sh_report :: (active:[32]u8,candidate:[32]u8,records:[]ShRecord,limits:ShLimits)->ShReport {
    r:=ShReport.{active=active,candidate=candidate,requests=records.count};
    if records.count==0 || limits.joined<2 {r.status=ShStatus.shape;return r;}
    for x:records {
        if x.active!=x.candidate {r.disagree+=1;}
        if x.outcome==-1 {continue;}
        r.joined+=1;
        if cast(int)x.active!=x.outcome {r.active_wrong+=1;}
        if cast(int)x.candidate!=x.outcome {r.candidate_wrong+=1;}
    }
    n:=f64(records.count);r.rate=f64(r.disagree)/n;r.rate_bound=r.rate+2*sqrt(r.rate*(1-r.rate)/n);
    if r.joined<limits.joined {return r;}
    // d is -1, 0 or 1 per joined request.
    j:=f64(r.joined);sum:=0.0;squares:=0.0;
    for x:records {
        if x.outcome==-1 {continue;}
        d:=f64(ifx cast(int)x.candidate!=x.outcome then 1 else 0)-f64(ifx cast(int)x.active!=x.outcome then 1 else 0);
        sum+=d;squares+=d*d;
    }
    r.difference=sum/j;
    r.difference_bound=r.difference+2*sqrt((squares-sum*sum/j)/(j-1)/j);
    if r.rate_bound>limits.disagreement {r.verdict=ShVerdict.disagreement;}
    else if r.difference_bound>limits.margin {r.verdict=ShVerdict.regression;}
    else {r.verdict=ShVerdict.promote;}
    return r;
}
// The report's digest: both identities, the counts and the verdict.
sh_report_digest :: (r:ShReport)->[32]u8 {
    bytes:[105]u8;
    for i:0..<32 {bytes[i]=r.active[i];bytes[32+i]=r.candidate[i];}
    counts:=[5]int.[r.requests,r.joined,r.disagree,r.active_wrong,r.candidate_wrong];
    for c,k:counts {for b:0..<8 {bytes[64+8*k+b]=cast,trunc(u8)(cast(u64)c>>cast(u64)(8*b));}}
    bytes[104]=cast(u8)r.verdict;
    return sh_digest(bytes[..]);
}
```

The report uses two measures:
- **Disagreement** is counted over every request, joined or not. It needs no
  outcome.
- **Errors** are compared only on joined requests, paired request by request.
  Each request's extra error (candidate minus active) is −1, 0 or 1.

Both are judged by their value plus two standard errors, so the gates read
as "can't be shown to exceed". A candidate that is better by chance alone
will not promote on a short log.

The report's digest covers both identities, the counts and the verdict. An
approval names that digest, so it can't be moved to a different run.

## Activation

<!-- source: psionic/lib/shadow.coax activate -->
```jai
// One compare-and-swap. The caller names the version it expects (checked
// first), the bytes it wants active, and an approval; the bytes must hash to the approved
// digest, be registered, and have a promoting report, shadowed against the
// model active now, whose digest the approval names. On any refusal the
// active record is returned unchanged.
sh_activate :: (now:ShActive,r:ShRegistry,expected:int,blob:[]u8,report:ShReport,approval:ShApproval)->ShActive {
    out:=now;
    if expected!=now.version {out.status=ShStatus.version;return out;}
    digest:=sh_digest(blob);entry:=-1;
    entries:=r.entries;
    for e,k:entries[0..<r.count] {if sh_same(e.digest,digest) {entry=k;}}
    if entry==-1 || !sh_same(digest,approval.candidate) {out.status=ShStatus.digest;return out;}
    if report.verdict!=ShVerdict.promote || !sh_same(report.candidate,digest)
        || !sh_same(report.active,now.digest) || !sh_same(sh_report_digest(report),approval.report) {out.status=ShStatus.approval;return out;}
    return ShActive.{digest=digest,version=now.version+1,entry=entry};
}
```

The checks run in a fixed order:
1. The version comes first, so a caller acting on a stale view is refused
   before anything else is looked at.
2. The bytes must hash to the approved digest, and be registered.
3. The report must promote this candidate against the model active now, and
   its digest must be the one approved.

`router-v4`'s own report, used again once `router-v4` is active, fails the
third check: it was shadowed against `router-v3`.

## How it is checked

- **6 Coax checks:**
  - draws, answers and a log against the oracle;
  - a model's bytes and digest from `hashlib`;
  - the registry's refusals, at 16 models;
  - a report counted by hand, at each gate's exact edge, with digests for
    small counts and for counts past one byte;
  - each refusal in activation, each returning the record unchanged.
- **[`shadow_oracle.py`](../tests/shadow_oracle.py)** rebuilds the example's
  output line for line, with `hashlib` for every digest.
- **Mutation:** 1,061 mutants, all killed.
