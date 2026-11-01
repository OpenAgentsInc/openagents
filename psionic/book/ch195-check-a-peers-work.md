# 195. Check a peer's work

> **Status: partly implemented.** The in-process slice runs: MACed stage receipts admitted against the job, sampling tied to a peer's record and the price, re-execution compared exactly or within a pinned tolerance, verdicts bound to evidence digests, a ledger that pays, withholds or holds in millisatoshis, and a reader's recount. Signed receipts a third party can verify, real backends on different hardware, multi-stage pipelines over chapter 188's links and payment remain; see [What remains](#what-remains).

A model split across machines runs as stages on different peers
([chapter 188](ch188-peers-transport-and-discovery.md)). Each peer reports
the work it did and asks to be paid for it. The buyer cannot watch the
peer's hardware, so it needs a checker: a node it trusts that runs some of
the same work again and pays only for work that passes.

This chapter builds that checker. It answers four questions about each
receipt a peer sends:

- **Is the receipt admitted?** Its MAC matches, it is new, and it names the
  job the buyer actually sent: peer, request, stage, model, profile and
  input. The output it names is the output the buyer received.
- **Is this answer checked?** A draw that the peer cannot predict is
  compared with a threshold. A new peer, and any high-value answer, is
  always checked; a peer with a long run of passed checks is checked less
  often, but never less than a floor.
- **Does the answer agree?** The checker runs the same input itself and
  compares under the profile it pinned: bit for bit when the peer claims the
  checker's own backend profile, within a stated tolerance otherwise.
- **What does the verdict do?** Work that was not checked, or that agrees,
  is paid. Work that disagrees is withheld and suspends the peer. Work that
  could not be verified is held for the owner.

Two files hold it:

- [`peer/audit.coax`](../lib/peer/audit.coax), an `#authority` file: every
  decision above. It does no cryptography and runs no stage; the caller
  passes in whether the MAC matched, the draw byte and both outputs. It
  prices work with [chapter 180](ch180-observe-meter-and-price.md)'s
  `bl_price` and samples with [chapter 26](ch026-check-the-mesh.md)'s
  `ms_selected`, unchanged.
- [`peer/audit-run.coax`](../lib/peer/audit-run.coax): the stage the
  checker re-executes, the digests, the receipt MAC, the draw and the
  record digest.

The profile is `psionic-peer-check-v0`: in-process, one CPU core, a toy
binary32 stage. The receipt and record layouts are this chapter's, not a
published protocol, so nothing in `proto/` or `docs/protocol/` changes.

## The stage

The work is one pipeline stage: a 16-to-8 linear layer in binary32. Its
weights, biases and inputs come from small integer formulas, so anyone can
rebuild them. Two backends run it. `seq` sums each row's products left to
right; `pair` sums them as a pairwise tree. Both add the bias last.
Coax rounds every binary32 operation once and never fuses a multiply and
an add, so each backend gives the same bits on every run. The two backends
give slightly different bits from each other.

<!-- source: psionic/lib/peer/audit-run.coax stage -->
```jai
PJ_IN :: 16;
PJ_OUT :: 8;
PjBackend :: enum u8 {seq;pair;}

// Weights, biases and inputs come from small integer formulas, so anyone can
// rebuild them: every value is an integer over 37, 7 or 19, in binary32.
pj_weight :: (j:int,i:int)->f32 {return f32(((i*31+j*17+5)%97)-48)/f32(37);}
pj_bias :: (j:int)->f32 {return f32((j*13)%11-5)/f32(7);}
pj_input :: (k:int,x:[]f32) {for i:0..<PJ_IN {x[i]=f32(((k*29+i*11)%53)-26)/f32(19);}}

// The pairwise tree: each half summed the same way, then the two added.
pj_pair :: (t:[]f32)->f32 {
    if t.count<2 {return t[0];}
    h:=t.count/2;
    return pj_pair(t[0..<h])+pj_pair(t[h..<t.count]);
}
// out[j] is the row's products summed in the backend's order, plus the bias.
pj_stage :: (x:[]f32,out:[]f32,b:PjBackend) {
    for j:0..<PJ_OUT {
        t:[16]f32;
        for i:0..<PJ_IN {t[i]=pj_weight(j,i)*x[i];}
        s:f32=0;
        if b==PjBackend.seq {for i:0..<PJ_IN {s+=t[i];}}
        else {s=pj_pair(t[..]);}
        out[j]=s+pj_bias(j);
    }
}
```

The profile names say which order a peer used: `stage-f32-seq-v1` or
`stage-f32-pair-v1`. Outputs, inputs and the model are named by SHA-256
digests over each value's four little-endian bytes. The model's digest
covers its weights row by row, then its biases.

<!-- source: psionic/lib/peer/audit-run.coax digests -->
```jai
pj_hex :: (d:[]u8,out:[]u8)->string {
    digits:="0123456789abcdef";
    for i:0..<d.count {out[2*i]=digits[cast(int)(d[i]>>4)];out[2*i+1]=digits[cast(int)(d[i]&15)];}
    return cast(string)out[0..<2*d.count];
}
// SHA-256 over each value's four little-endian bytes, in hex.
pj_values_digest :: (v:[]f32,out:[]u8)->string {
    sha:[112]u8;d:[32]u8;#assert sha256_init(sha[..])==SHA_OK;
    for x:v {
        w:=f32_bits(x);b:[4]u8;
        for i:0..<4 {b[i]=cast(u8)((w>>cast(u32)(8*i))&0xff);}
        #assert sha256_update_checked(sha[..],b[..],false)==SHA_OK;
    }
    #assert sha256_final_checked(sha[..],d[..],false)==SHA_OK;
    return pj_hex(d[..],out);
}
pj_text_digest :: (text:[]u8,out:[]u8)->string {
    d:[32]u8;#assert sha256_hash_checked(text,d[..],false)==SHA_OK;return pj_hex(d[..],out);
}
// The model's digest: its weights row by row, then its biases.
pj_model_digest :: (out:[]u8)->string {
    v:[PJ_OUT*PJ_IN+PJ_OUT]f32;
    for j:0..<PJ_OUT {for i:0..<PJ_IN {v[j*PJ_IN+i]=pj_weight(j,i);}v[PJ_OUT*PJ_IN+j]=pj_bias(j);}
    return pj_values_digest(v[..],out);
}
```

The first check pins these bits against an independent computation: a
short Python script rebuilt the stage with every operation rounded to
binary32 through `struct.pack('<f')`. For request 0, value 0 is
`0xbefcd39a` (-0.4938019) under `seq`; the digests of the inputs and of
both backends' outputs for requests 0 to 2 match the script's.

<!-- source: psionic/lib/peer/audit-run.coax check-stage -->
```jai
#test "the stage matches an independent binary32 emulation" {
    x:[PJ_IN]f32;s:[PJ_OUT]f32;p:[PJ_OUT]f32;h:[64]u8;
    seq:=[3]string.["a494c276196c552b91227ff6a832eeb13916b7c79aca0eee1dc34b8e434e4675","25cee7aed9e4fb52751ab3a76ae57bccfcf3011c34a152dbbcd4b911bc665921","b5d5793a84f639fd1edea707ae539ef01d567709fab2311128356d8f3436531a"];
    pair:=[3]string.["a729aa8a11348421d5be4027958921dca1d56f2f919a9f9388e66042f6612738","dfc9ec1d0ac49fb2f9241bfe97c211daff4cd4bbbfa246aa6f70e51800f90cea","366f25b45b3e3a37362cf1118e76d4d69578de741c4fa509b44ecc6c68f0a21f"];
    inputs:=[3]string.["01aef69987b77c43a34e905203f402d644bc5f65cd9071b821a0ead894df29f3","2f2b9519dacae19a4cb80fa051a16eee3801fc0ef7a6032cd7328db355681627","f6d867d2c1904f01d24a28667b151c024f12c0b84ef255c033831e324788f1df"];
    for k:0..<3 {
        pj_input(k,x[..]);#assert pj_values_digest(x[..],h[..])==inputs[k];
        pj_stage(x[..],s[..],PjBackend.seq);#assert pj_values_digest(s[..],h[..])==seq[k];
        pj_stage(x[..],p[..],PjBackend.pair);#assert pj_values_digest(p[..],h[..])==pair[k];
    }
    pj_input(0,x[..]);pj_stage(x[..],s[..],PjBackend.seq);
    #assert f32_bits(s[0])==0xbefcd39a && f32_bits(s[1])==0x3ec0cb17;
    #assert pj_model_digest(h[..])=="4aa6751881c6f068f48932f6f82503815b312e797cb3c88d2d1b7ac316d56ca0";
    one:=[1]f32.[1.0];
    #assert pj_values_digest(one[0..<0],h[..])=="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    #assert pj_values_digest(one[..],h[..])=="e00e5eb9444182f352323374ef4e08ebcb784725fdd4fd612d7730540b3e0c8c";
}
```

## How far apart may the two backends be?

The checker runs `seq`. When a peer runs `pair`, the outputs differ, and the
checker needs a tolerance that no honest `pair` peer can exceed. The
standard bound for floating-point sums gives one.

Summing n terms left to right is off from the exact sum by at most
γ(n-1) times the sum of the terms' magnitudes, where
γ(m) = m·u/(1-m·u) and u = 2⁻²⁴ is binary32's unit roundoff. A pairwise
tree of 16 terms is 4 levels deep, so it is off by at most γ(4) times the
same sum. Adding the bias rounds once more, by at most u. With

- γ(15) = 8.9407e-7,
- γ(4) = 2.3842e-7,
- 2u(1+γ(15)) = 1.1921e-7,

the two backends differ by at most 1.2517e-6 times the row's magnitude.
Over the stage's whole input range, each weight is at most 48/37 in size,
each input at most 26/19 and each bias at most 5/7, so a row's magnitude is
at most 16·(48/37)·(26/19) + 5/7 = 29.118, after allowing one rounding for
each stored value. The bound is 1.2517e-6 × 29.118 = 3.6447e-5, or 36447
billionths. The profile `stage-f32-pair-v1` therefore gets a tolerance of
40000 ppb (4e-5).

The check computes the bound, confirms it is below the tolerance, and then
runs all 53 distinct inputs: each backend stays inside its own bound around
the exact sum, the two stay inside the shared bound, and they do differ.

<!-- source: psionic/lib/peer/audit-run.coax check-bound -->
```jai
// Summing n terms left to right is off by at most gamma(n-1) times the sum
// of their magnitudes, gamma(m) = m*u/(1-m*u), u = 2^-24; the pairwise tree
// of 16 has depth 4. Adding the bias rounds once more. So the two backends
// differ by at most (gamma(15)+gamma(4)+2u(1+gamma(15))) times the row's
// magnitude, which over the whole input range stays below 40000 ppb.
#test "both backends stay inside the a-priori bound, and it inside the tolerance" {
    u:=1.0/16777216.0;g15:=15.0*u/(1.0-15.0*u);g4:=4.0*u/(1.0-4.0*u);
    row:=16.0*(48.0/37.0)*(26.0/19.0)*(1.0+u)*(1.0+u)*(1.0+u)+(5.0/7.0)*(1.0+u);
    domain:=(g15+g4+2.0*u*(1.0+g15))*row;
    #assert domain<=40000.0/1000000000.0 && domain>36000.0/1000000000.0;
    x:[PJ_IN]f32;s:[PJ_OUT]f32;p:[PJ_OUT]f32;worst:=0.0;
    for k:0..<53 {
        pj_input(k,x[..]);pj_stage(x[..],s[..],PjBackend.seq);pj_stage(x[..],p[..],PjBackend.pair);
        for j:0..<PJ_OUT {
            exact:=f64(pj_bias(j));size:=abs(f64(pj_bias(j)));
            for i:0..<PJ_IN {t:=pj_weight(j,i)*x[i];exact+=f64(t);size+=abs(f64(t));}
            #assert abs(f64(s[j])-exact)<=(g15+u*(1.0+g15))*size && abs(f64(p[j])-exact)<=(g4+u*(1.0+g4))*size;
            d:=abs(f64(s[j])-f64(p[j]));#assert d<=(g15+g4+2.0*u*(1.0+g15))*size && size<=row;
            if d>worst {worst=d;}
        }
    }
    // The orders do differ, by a few units in the last place.
    #assert worst>0 && worst<0.000001;
}
```

The tolerance holds for inputs in this range only. A stage with larger
activations needs a larger tolerance, derived the same way. This is a
different question from [chapter 219](ch219-how-far-apart-may-two-honest-answers-be.md)'s:
there, two backends give decision answers whose distance is measured
empirically. Here, a proof bounds a single sum.

## Admit the receipt

A peer sends a receipt with every answer. Its text is one field a line:
the layout name, the peer, the peer's receipt counter, the request digest,
the stage, the model digest, the profile, the input and output digests, and
the use it bills. The peer MACs that text with HMAC-SHA256 under a key it
shares with the checker, as chapter 188's invitations do. The checker
recomputes the MAC and compares in constant time.

<!-- source: psionic/lib/peer/audit-run.coax receipt -->
```jai
// The receipt's bytes: one field a line, in a fixed order.
pj_receipt_text :: (r:PkReceipt,out:[]u8)->int {
    n:=pj_put(out,0,"psionic-stage-receipt-v0");
    n=pj_put(out,n,r.peer);n=pj_put_int(out,n,r.seq);n=pj_put(out,n,r.request);n=pj_put_int(out,n,r.stage);
    n=pj_put(out,n,r.model);n=pj_put(out,n,r.profile);n=pj_put(out,n,r.input);n=pj_put(out,n,r.output);
    n=pj_put_int(out,n,cast(int)r.use.status);n=pj_put_int(out,n,r.use.tokens_in);n=pj_put_int(out,n,r.use.tokens_out);n=pj_put_int(out,n,r.use.device_us);
    return n;
}
// HMAC-SHA256 of those bytes under the key the peer shares with the checker.
pj_mac :: (key:[]u8,r:PkReceipt,mac:[]u8)->bool {
    text:[1024]u8;n:=pj_receipt_text(r,text[..]);
    return n!=-1 && mac.count==32 && hmac_sha256_checked(key,text[0..<n],mac,false)==SHA_OK;
}
// The checker recomputes the MAC and compares in constant time.
pj_mac_ok :: (key:[]u8,r:PkReceipt,mac:[]u8)->bool {
    want:[32]u8;return mac.count==32 && pj_mac(key,r,want[..]) && cb_ct_equal(want[..],mac)!=0;
}
pj_receipt_digest :: (r:PkReceipt,out:[]u8)->string {
    text:[1024]u8;n:=pj_receipt_text(r,text[..]);return pj_text_digest(text[0..<n],out);
}
// The draw: the first byte of SHA-256(checker secret, receipt digest).
pj_draw :: (secret:[]u8,receipt:string)->int {
    #assert receipt.count==64;
    hex:[64]u8;for i:0..<64 {hex[i]=receipt[i];}
    sha:[112]u8;d:[32]u8;#assert sha256_init(sha[..])==SHA_OK;
    #assert sha256_update_checked(sha[..],secret,false)==SHA_OK && sha256_update_checked(sha[..],hex[..],false)==SHA_OK;
    #assert sha256_final_checked(sha[..],d[..],false)==SHA_OK;
    return cast(int)d[0];
}
```

`pk_admit` refuses a receipt for the first thing wrong with it, in a fixed
order. A forged MAC comes first, because no field of an unauthenticated
receipt can be trusted. A suspended peer is refused next, then a counter no
newer than the last receipt counted. The remaining checks bind the
receipt to the job: the peer, request, stage, model, profile and input must
be what the buyer sent, and the output digest must match the bytes the
buyer received. Last, the use must be within chapter 180's bounds and
`answered`.

<!-- source: psionic/lib/peer/audit.coax admit -->
```jai
// A stage receipt as the checker reads it: who did the work, the peer's
// receipt counter, the request and stage it answers, the model and profile
// it claims, the digests of its input and output, the use it bills, and
// whether its MAC, under the key this peer shares with the checker, matched.
PkReceipt :: struct {peer:string;seq:int;request:string;stage:int;model:string;profile:string;input:string;output:string;use:BlUse;mac_ok:bool;}
// The job the buyer actually sent that peer.
PkJob :: struct {peer:string;request:string;stage:int;model:string;profile:string;input:string;}
// A peer's standing with this checker. last_seq is the newest receipt
// counted; agreed counts checks passed since the last failure; paid,
// withheld and held are millisatoshis.
PkStanding :: struct {last_seq:int;agreed:int;disagreed:int;unverifiable:int;unsampled:int;suspended:bool;paid:int;withheld:int;held:int;}
PkAdmit :: enum u8 {ok;forged;suspended;replayed;peer;request;stage;model;profile;input;output;use;}

// delivered is the digest of the output bytes the buyer actually received.
pk_admit :: (r:PkReceipt,jb:PkJob,delivered:string,st:PkStanding)->PkAdmit #export_c {
    if !r.mac_ok {return PkAdmit.forged;}
    if st.suspended {return PkAdmit.suspended;}
    if r.seq<=st.last_seq {return PkAdmit.replayed;}
    if r.peer!=jb.peer {return PkAdmit.peer;}
    if r.request!=jb.request {return PkAdmit.request;}
    if r.stage!=jb.stage {return PkAdmit.stage;}
    if r.model!=jb.model {return PkAdmit.model;}
    if r.profile!=jb.profile {return PkAdmit.profile;}
    if r.input!=jb.input {return PkAdmit.input;}
    if r.output!=delivered {return PkAdmit.output;}
    if !bl_use_valid(r.use) || r.use.status!=BlStatus.answered {return PkAdmit.use;}
    return PkAdmit.ok;
}
```

The checks name each tamper case and its reason. An answer changed after
its receipt is refused as `output`. A receipt that names another model,
even one the peer MACed itself, is refused as `model`. Any change made in
transit is refused as `forged`, and a receipt sent again is refused as
`replayed`.

<!-- source: psionic/lib/peer/audit.coax check-admit -->
```jai
#test "a receipt is refused for the first thing wrong with it" {
    j:=pk_test_job();st:=PkStanding.{last_seq=4};r:=pk_test_receipt();
    #assert pk_admit(r,j,"o1",st)==PkAdmit.ok;
    #assert pk_admit(r,j,"o2",st)==PkAdmit.output;                    // the answer changed after the receipt
    x:=r;x.model="m2";#assert pk_admit(x,j,"o1",st)==PkAdmit.model;    // another model's digest
    st.last_seq=5;#assert pk_admit(r,j,"o1",st)==PkAdmit.replayed;    // the same receipt again
    x=r;x.mac_ok=false;#assert pk_admit(x,j,"o2",st)==PkAdmit.forged; // altered in transit
    st.last_seq=4;st.suspended=true;#assert pk_admit(r,j,"o1",st)==PkAdmit.suspended;
}
```

A `#forall` runs all 2,048 combinations of the eleven conditions against a
table written separately, so every refusal has its place in the order.

## Decide whether to check

A draw byte below the threshold is checked, chapter 26's rule. The draw is
the first byte of SHA-256 over a secret only the checker holds, followed by
the receipt's digest. The peer has committed to its output before the draw
exists and cannot compute it, so it cannot know which answers will be
checked.

The threshold depends on the peer's record and the price. A peer whose
current run of passed checks (`agreed`) is shorter than the policy's
`probation` is checked every time, and so is a receipt priced at or above
`high_value`. After probation the threshold is 256·probation/(agreed+1),
rounded down. A draw below the policy's `floor` is always checked as well,
so no peer is ever checked less often than floor/256 of the time. A failed
check sets `agreed` back to 0, so probation starts again.

<!-- source: psionic/lib/peer/audit.coax sample -->
```jai
// probation: checks a peer must pass, in a row, before sampling thins out.
// floor: draws below it are always checked, so no peer is ever sampled at
// less than floor/256. high_value: a price in msat at or above which every
// receipt is checked.
PkPolicy :: struct {probation:int;floor:int;high_value:int;}

pk_policy_valid :: (p:PkPolicy)->bool {
    return p.probation>=1 && p.probation<=1024 && p.floor>=1 && p.floor<=256 && bl_in(p.high_value,BL_MSAT_MOST);
}
// The sampling threshold out of 256, or -1 for a policy or price out of
// bounds. A peer on probation and a high-value receipt are always checked;
// after probation the threshold falls as 256*probation/(agreed+1). A
// failure sets agreed back to 0, so probation starts again.
pk_threshold :: (p:PkPolicy,st:PkStanding,price:int)->int #export_c {
    if !pk_policy_valid(p) || !bl_in(price,BL_MSAT_MOST) || st.agreed<0 {return -1;}
    if st.agreed<p.probation || price>=p.high_value {return 256;}
    return 256*p.probation/(st.agreed+1);
}
// Chapter 26's rule: a draw byte below the threshold, or below the floor,
// is checked. The draw comes from a secret only the checker holds and the
// receipt's digest, so a peer cannot know which answers will be checked
// when it commits to them.
pk_sampled :: (p:PkPolicy,st:PkStanding,price:int,draw:int)->bool #export_c {
    t:=pk_threshold(p,st,price);
    if t<0 {return false;}
    return ms_selected(draw,t) || ms_selected(draw,p.floor);
}
```

By hand, with probation 4, floor 32 and high value 50,000 msat:

| agreed | threshold | checked at least |
| --- | --- | --- |
| 0 to 3 | 256 | every time |
| 4 | 1024/5 = 204.8, so 204 | 204/256 |
| 7 | 1024/8 = 128 | half |
| 31 | 1024/32 = 32 | 32/256 |
| 32 | 1024/33 = 31.03, so 31 | 32/256, the floor |
| 1,000,000 | 0 | 32/256, the floor |

<!-- source: psionic/lib/peer/audit.coax check-sample -->
```jai
#test "sampling thresholds worked by hand" {
    p:=pk_test_policy();st:=PkStanding.{};
    for a:0..<4 {st.agreed=a;#assert pk_threshold(p,st,1014)==256;}   // probation: everything
    st.agreed=4;#assert pk_threshold(p,st,1014)==204;                  // 1024/5 = 204.8
    st.agreed=7;#assert pk_threshold(p,st,1014)==128;                  // half
    st.agreed=31;#assert pk_threshold(p,st,1014)==32;
    st.agreed=32;#assert pk_threshold(p,st,1014)==31;                  // below the floor now
    #assert pk_sampled(p,st,1014,31) && !pk_sampled(p,st,1014,32);     // the floor still checks 32 in 256
    st.agreed=1000000;#assert pk_threshold(p,st,1014)==0 && pk_sampled(p,st,1014,0) && pk_sampled(p,st,1014,31) && !pk_sampled(p,st,1014,32);
    st.agreed=7;#assert pk_threshold(p,st,50000)==256 && pk_threshold(p,st,49999)==128;   // high value
    #assert pk_sampled(p,st,1014,127) && !pk_sampled(p,st,1014,128) && pk_sampled(p,st,1014,0);
    #assert !pk_sampled(p,st,1014,-1) && !pk_sampled(p,st,1014,256);
}
```

A second `#forall` compares the threshold with an oracle that counts down
from 256 to the largest t with t·(agreed+1) ≤ 256·probation, for 6
probations, 4 floors, 70 values of `agreed` and 3 prices, and checks seven
draws for each.

## Compare

The checker pins a list of profiles it can check. An exact profile is its
own backend: the peer's output must match its own bit for bit, and the
verdict names the first value that differs. A bounded profile gives a
tolerance in billionths: the largest absolute difference must be at most
that, and every value of the peer's output must be finite.

The verdict is `agree`, `disagree` or `unverifiable`, with a reason. It is
`unverifiable` when the checker has no profile pinned for the name the
receipt claims, or when its own run failed or gave a value that is not
finite. Neither of those is the peer's fault. An output of the wrong length
always disagrees.

<!-- source: psionic/lib/peer/audit.coax compare -->
```jai
// A comparison profile the checker pins. exact: the checker runs the same
// backend profile, so the outputs must match bit for bit. Otherwise the
// largest absolute difference may be at most tolerance_ppb billionths.
PkProfile :: struct {name:string;exact:bool;tolerance_ppb:int;}
PkVerdict :: enum u8 {none;agree;disagree;unverifiable;}
PkReason :: enum u8 {none;exact;within;bits;distance;shape;number;profile;reference;}
// at is the first value that differs (exact) or the largest difference
// (bounded), -1 when no value was compared.
PkCompare :: struct {verdict:PkVerdict;reason:PkReason;worst:f64;at:int=-1;tolerance:int;}
PK_VALUES :: 4096;

pk_finite :: (x:f32)->bool {return (f32_bits(x)&0x7fffffff)<0x7f800000;}
// claimed is the receipt's profile; reference is the checker's own run of
// the same input, ref_ok whether that run finished.
pk_compare :: (profiles:[]PkProfile,claimed:string,peer:[]f32,reference:[]f32,ref_ok:bool)->PkCompare #export_c {
    k:=-1;
    for i:0..<profiles.count {if k<0 && profiles[i].name==claimed {k=i;}}
    if k<0 || !bl_in(profiles[k].tolerance_ppb,1000000000) {return PkCompare.{verdict=PkVerdict.unverifiable,reason=PkReason.profile};}
    p:=profiles[k];r:=PkCompare.{tolerance=p.tolerance_ppb};
    if !ref_ok || reference.count<1 || reference.count>PK_VALUES {r.verdict=PkVerdict.unverifiable;r.reason=PkReason.reference;return r;}
    for i:0..<reference.count {if !pk_finite(reference[i]) {r.verdict=PkVerdict.unverifiable;r.reason=PkReason.reference;return r;}}
    r.verdict=PkVerdict.disagree;
    if peer.count!=reference.count {r.reason=PkReason.shape;return r;}
    if p.exact {
        for i:0..<peer.count {if f32_bits(peer[i])!=f32_bits(reference[i]) {r.reason=PkReason.bits;r.at=i;return r;}}
        r.verdict=PkVerdict.agree;r.reason=PkReason.exact;return r;
    }
    for i:0..<peer.count {
        if !pk_finite(peer[i]) {r.reason=PkReason.number;r.at=i;return r;}
        d:=abs(f64(peer[i])-f64(reference[i]));
        if r.at<0 || d>r.worst {r.worst=d;r.at=i;}
    }
    if r.worst<=f64(p.tolerance_ppb)/1000000000.0 {r.verdict=PkVerdict.agree;r.reason=PkReason.within;}
    else {r.reason=PkReason.distance;}
    return r;
}
```

By hand: with a reference of 1.0, -2.0 and 0.25, the same three values agree
exactly. Changing 0.25 by one unit in the last place, 2⁻²⁵, disagrees on
value 2 under the exact profile. Under a bounded profile of 0.5, an output
of 1.5, -2.25 and 0.25 is exactly at the limit on value 0 and agrees.
Negative zero agrees within any tolerance but disagrees exactly, because
its bits differ.

<!-- source: psionic/lib/peer/audit.coax check-compare -->
```jai
#test "comparisons worked by hand" {
    prof:=[3]PkProfile.[PkProfile.{name="seq",exact=true},PkProfile.{name="pair",tolerance_ppb=500000000},PkProfile.{name="pair",exact=true}];
    ref:=[3]f32.[1.0,-2.0,0.25];
    same:=[3]f32.[1.0,-2.0,0.25];
    c:=pk_compare(prof[..],"seq",same[..],ref[..],true);
    #assert c.verdict==PkVerdict.agree && c.reason==PkReason.exact && c.at==-1 && c.tolerance==0;
    // One unit in the last place of 0.25 is 2^-25: exact refuses it.
    near:=[3]f32.[1.0,-2.0,f32_from_bits(0x3e800001)];
    c=pk_compare(prof[..],"seq",near[..],ref[..],true);
    #assert c.verdict==PkVerdict.disagree && c.reason==PkReason.bits && c.at==2;
    // Bounded by 0.5: 1.5 against 1.0 is exactly at the limit, so it agrees.
    off:=[3]f32.[1.5,-2.25,0.25];
    c=pk_compare(prof[..],"pair",off[..],ref[..],true);
    #assert c.verdict==PkVerdict.agree && c.reason==PkReason.within && c.worst==0.5 && c.at==0 && c.tolerance==500000000;
    off[1]=-2.5000002;c=pk_compare(prof[..],"pair",off[..],ref[..],true);
    #assert c.verdict==PkVerdict.disagree && c.reason==PkReason.distance && c.at==1 && c.worst>0.5;
    // The checker has no tolerance pinned for a profile it does not know.
    c=pk_compare(prof[..],"gpu",same[..],ref[..],true);
    #assert c.verdict==PkVerdict.unverifiable && c.reason==PkReason.profile && c.at==-1;
    // The first profile of a name is the pinned one.
    c=pk_compare(prof[..],"pair",near[..],ref[..],true);#assert c.reason==PkReason.within;
}
```

A `#forall` runs every pair of two-value outputs drawn from five values
(including one ulp above 1.0 and infinity), both kinds of profile and four
tolerances, 5,000 cases, against a rule written out separately.

## Count the verdict

`pk_settle` counts one admitted receipt in the peer's standing. The price is
chapter 180's `bl_price` under the checker's schedule: a base of 1000 msat,
2000 msat per 1000 input tokens and 50,000 msat per device-second. Each
receipt bills one token and its device time.

<!-- source: psionic/lib/peer/audit.coax settle -->
```jai
PkCause :: enum u8 {none;schedule;use;replayed;verdict;total;}
PkSettle :: struct {cause:PkCause;price:int;standing:PkStanding;}

// Count one admitted receipt. Unsampled work and work that agrees is paid;
// work that disagrees is withheld and suspends the peer until its owner
// decides; work that could not be verified is held for the owner. A sampled
// receipt needs a verdict and an unsampled one must not have one.
pk_settle :: (st:PkStanding,s:BlSchedule,seq:int,use:BlUse,sampled:bool,c:PkCompare)->PkSettle #export_c {
    p:=bl_price(s,use);
    if p.cause==BlCause.schedule {return PkSettle.{cause=PkCause.schedule,standing=st};}
    if p.cause!=BlCause.none {return PkSettle.{cause=PkCause.use,standing=st};}
    if seq<=st.last_seq {return PkSettle.{cause=PkCause.replayed,standing=st};}
    if sampled==(c.verdict==PkVerdict.none) {return PkSettle.{cause=PkCause.verdict,standing=st};}
    bucket:=0;
    if c.verdict==PkVerdict.disagree {bucket=1;}
    if c.verdict==PkVerdict.unverifiable {bucket=2;}
    so_far:=[3]int.[st.paid,st.withheld,st.held];
    if !bl_in(p.msat,BL_MSAT_MOST-so_far[bucket]) {return PkSettle.{cause=PkCause.total,standing=st};}
    n:=st;n.last_seq=seq;
    if !sampled {n.unsampled+=1;n.paid+=p.msat;}
    else if bucket==0 {n.agreed+=1;n.paid+=p.msat;}
    else if bucket==1 {n.disagreed+=1;n.withheld+=p.msat;n.suspended=true;n.agreed=0;}
    else {n.unverifiable+=1;n.held+=p.msat;}
    return PkSettle.{price=p.msat,standing=n};
}
```

By hand: one token and 240 µs is 1·2000·1000 + 240·50,000 = 14,000,000
millionths of a millisatoshi, which is 14 msat, plus the 1000 msat base:
1014 msat. A cancelled stage would pay only its device time:
⌈240·50,000/1,000,000⌉ = 12 msat.

<!-- source: psionic/lib/peer/audit.coax check-settle -->
```jai
#test "settlement worked by hand" {
    s:=bl_test_schedule();u:=BlUse.{status=BlStatus.answered,tokens_in=1,device_us=240};
    // 1*2000*1000 + 240*50000 = 14000000 millionths: 14 msat, plus 1000.
    #assert bl_price(s,u).msat==1014 && bl_price(s,pk_test_receipt().use).msat==1014;
    st:=PkStanding.{agreed=7};
    agree:=PkCompare.{verdict=PkVerdict.agree,reason=PkReason.exact};
    t:=pk_settle(st,s,1,u,false,PkCompare.{});
    #assert t.cause==PkCause.none && t.price==1014 && t.standing.paid==1014 && t.standing.unsampled==1 && t.standing.agreed==7 && t.standing.last_seq==1;
    t=pk_settle(t.standing,s,2,u,true,agree);
    #assert t.standing.paid==2028 && t.standing.agreed==8 && t.standing.last_seq==2;
    t=pk_settle(t.standing,s,3,u,true,PkCompare.{verdict=PkVerdict.unverifiable,reason=PkReason.reference});
    #assert t.standing.held==1014 && t.standing.unverifiable==1 && t.standing.agreed==8 && !t.standing.suspended && t.standing.paid==2028;
    t=pk_settle(t.standing,s,4,u,true,PkCompare.{verdict=PkVerdict.disagree,reason=PkReason.bits,at=3});
    #assert t.standing.withheld==1014 && t.standing.disagreed==1 && t.standing.suspended && t.standing.agreed==0 && t.standing.paid==2028 && t.standing.held==1014;
}
```

Every amount is a whole number of millisatoshis, and every sum is refused
before it could pass 21 million bitcoin. The ledger says what is owed. Paying
it is an owner decision made in the wallet, outside Psionic.

## Publish and recount

For every counted receipt, the checker keeps a record: the digests that
bind the verdict to its evidence (the receipt, the peer's output and the
checker's own run), the use, whether it was checked, and the comparison. The
record's own digest is SHA-256 over its text.

<!-- source: psionic/lib/peer/audit-run.coax record -->
```jai
pj_record_text :: (r:PkRecord,out:[]u8)->int {
    n:=pj_put(out,0,"psionic-stage-check-v0");
    n=pj_put(out,n,r.receipt);n=pj_put(out,n,r.output);n=pj_put(out,n,r.reference);n=pj_put_int(out,n,r.seq);
    n=pj_put_int(out,n,cast(int)r.use.status);n=pj_put_int(out,n,r.use.tokens_in);n=pj_put_int(out,n,r.use.tokens_out);n=pj_put_int(out,n,r.use.device_us);
    n=pj_put_int(out,n,ifx r.sampled then 1 else 0);n=pj_put_int(out,n,cast(int)r.compare.verdict);n=pj_put_int(out,n,cast(int)r.compare.reason);
    n=pj_put_int(out,n,r.compare.at);n=pj_put_int(out,n,cast(int)f64_bits(r.compare.worst));n=pj_put_int(out,n,r.compare.tolerance);
    return n;
}
pj_record_digest :: (r:PkRecord,out:[]u8)->string {
    text:[1024]u8;n:=pj_record_text(r,text[..]);return pj_text_digest(text[0..<n],out);
}
```

A reader who holds a peer's records recomputes its ledger by counting them
again in order. A record counted twice is refused as `replayed`, at its
index.

<!-- source: psionic/lib/peer/audit.coax recount -->
```jai
// What the checker publishes for each counted receipt: the digests that
// bind the verdict to its evidence (the receipt, the peer's output and the
// checker's reference run), the use, and the decision. A reader holding the
// records recomputes the peer's ledger by counting them again in order.
PkRecord :: struct {receipt:string;output:string;reference:string;seq:int;use:BlUse;sampled:bool;compare:PkCompare;}
PkRecount :: struct {cause:PkCause;at:int=-1;standing:PkStanding;}

pk_recount :: (s:BlSchedule,records:[]PkRecord)->PkRecount #export_c {
    st:=PkStanding.{};
    for i:0..<records.count {
        r:=records[i];
        t:=pk_settle(st,s,r.seq,r.use,r.sampled,r.compare);
        if t.cause!=PkCause.none {return PkRecount.{cause=t.cause,at=i,standing=st};}
        st=t.standing;
    }
    return PkRecount.{standing=st};
}
```

## Run it

The program is [`peer-audit.coax`](../examples/peer-audit.coax):

```shell
coax/bin/coax run psionic/examples/peer-audit.coax
```

Three peers each answer ten requests. alpha runs `seq`, the checker's own
profile. beta runs `pair`. gamma claims `seq` and runs it honestly for
requests 0 to 5, then from request 6 sums only the first half of each row:
half the work for the full price. Request k bills 200 + 10k µs of device
time. The keys and the checker's secret are fixed demonstration bytes.

<!-- source: psionic/examples/peer-audit.coax example -->
```jai
main :: () {
    mh:[64]u8;
    setup:=ExSetup.{policy=PkPolicy.{probation=4,floor=32,high_value=50000},schedule=bl_test_schedule(),model=pj_model_digest(mh[..])};
    for i:0..<32 {setup.secret[i]=cast(u8)(200-i);}
    ps:[EX_PEERS]ExPeer;
    names:=[3]string.["alpha","beta","gamma"];
    for w:0..<EX_PEERS {ps[w].name=names[w];for i:0..<32 {ps[w].key[i]=cast(u8)(w*64+i);}ps[w].profile="stage-f32-seq-v1";}
    ps[1].backend=PjBackend.pair;ps[1].profile="stage-f32-pair-v1";
    ps[2].lazy_from=6;
    print("model %\n",setup.model);
    x:[PJ_IN]f32;a:[PJ_OUT]f32;b:[PJ_OUT]f32;pj_input(0,x[..]);pj_stage(x[..],a[..],PjBackend.seq);
    print("request 0, value 0: seq %, pair ",a[0]);pj_stage(x[..],b[..],PjBackend.pair);print("%\n",b[0]);

    // 1. Ten requests to each peer. gamma does half the work from request 6.
    for w:0..<EX_PEERS {for k:0..<EX_JOBS {ex_serve(ps[..],w,setup,k);}}

    // 2. Tampered receipts, against alpha's standing.
    print("tampering:\n");
    xi:[PJ_IN]f32;out:[PJ_OUT]f32;ex_work(ps[0],10,xi[..],out[..]);
    rq:[64]u8;ih:[64]u8;oh:[64]u8;other:[64]u8;
    r:=PkReceipt.{peer="alpha",seq=ps[0].seq+1,request=ex_request(10,rq[..]),stage=1,model=setup.model,profile="stage-f32-seq-v1",
        input=pj_values_digest(xi[..],ih[..]),output=pj_values_digest(out[..],oh[..]),use=BlUse.{status=BlStatus.answered,tokens_in=1,device_us=300}};
    mac:[32]u8;#assert pj_mac(ps[0].key[..],r,mac[..]);
    changed:=out;changed[5]+=0.25;
    print("an answer changed after its receipt: ");ex_check(ps[..],0,setup,r,mac[..],pj_values_digest(changed[..],other[..]),xi[..],changed[..]);
    swapped:=r;swapped.model=ex_request(99,other[..]);#assert pj_mac(ps[0].key[..],swapped,mac[..]);
    print("a receipt naming another model: ");ex_check(ps[..],0,setup,swapped,mac[..],r.output,xi[..],out[..]);
    #assert pj_mac(ps[0].key[..],r,mac[..]);
    altered:=r;altered.use.device_us=30;
    print("a receipt altered on the way: ");ex_check(ps[..],0,setup,altered,mac[..],r.output,xi[..],out[..]);
    old:=r;old.seq=3;#assert pj_mac(ps[0].key[..],old,mac[..]);
    print("a receipt counted before: ");ex_check(ps[..],0,setup,old,mac[..],r.output,xi[..],out[..]);
    #assert pj_mac(ps[0].key[..],r,mac[..]);
    print("the untouched receipt: ");ex_check(ps[..],0,setup,r,mac[..],r.output,xi[..],out[..]);
    // Two comparisons that change no ledger: beta's pairwise output claimed
    // as the exact profile, and a profile the checker pinned no tolerance for.
    pj_stage(xi[..],b[..],PjBackend.pair);pj_stage(xi[..],a[..],PjBackend.seq);profiles:=ex_profiles();
    c:=pk_compare(profiles[..],"stage-f32-seq-v1",b[..],a[..],true);print("beta's output under the exact profile: % % at value %\n",c.verdict,c.reason,c.at);
    c=pk_compare(profiles[..],"stage-f32-gpu-v1",b[..],a[..],true);print("a receipt for stage-f32-gpu-v1: % %\n",c.verdict,c.reason);

    // 3. The ledgers, and a reader's recount from the published records.
    for w:0..<EX_PEERS {ex_ledger(ps[w]);}
    for w:0..<EX_PEERS {
        p:=ps[w];rc:=pk_recount(setup.schedule,p.records[0..<p.n]);s:=p.standing;
        same:=rc.cause==PkCause.none && rc.standing.paid==s.paid && rc.standing.withheld==s.withheld && rc.standing.held==s.held && rc.standing.agreed==s.agreed && rc.standing.disagreed==s.disagreed && rc.standing.unsampled==s.unsampled && rc.standing.suspended==s.suspended && rc.standing.last_seq==s.last_seq;
        rh:[64]u8;
        print("recount %: % records, same ledger %; last record %\n",p.name,p.n,same,pj_record_digest(p.records[p.n-1],rh[..]));
    }
    dup:[16]PkRecord;for i:0..<ps[0].n {dup[i]=ps[0].records[i];}dup[ps[0].n]=dup[2];
    rc:=pk_recount(setup.schedule,dup[0..<ps[0].n+1]);print("alpha's records with one counted twice: refused, % at record %\n",rc.cause,rc.at);
}
```

Before running it, work alpha's first lines by hand. Request 0 bills 200 µs:
2,000,000 + 10,000,000 = 12,000,000 millionths, so 12 + 1000 = 1012 msat.
Request 1 bills 210 µs: 2,000,000 + 10,500,000 = 12,500,000 millionths, 12.5
msat, rounded up to 13: 1013 msat. alpha's first receipt has the digest
`87519f1a…`; the first byte of SHA-256 over the secret and that digest is
`0xd1`, 209. The same script that rebuilt the stage computes these, and the
checks pin them.

alpha starts on probation, so its first four receipts are checked whatever
the draw. They agree, so `agreed` is 4 and the threshold for the fifth is
204: draw 66 is checked and agrees, and the threshold falls to 1024/6, 170.
Draw 234 for the sixth is above both 170 and the floor, so it is not
checked and `agreed` stays 5. The seventh, draw 5, is checked: the
threshold falls to 1024/7, 146. The eighth, draw 26, is checked, and the
threshold falls to 128, which the ninth and tenth draws, 237 and 168, are
above.

beta's outputs are never equal to the checker's, and they agree within the
tolerance. Its first distance is 4.768e-7 on value 2. That value is about
3.32, where a unit in the last place is 2⁻²², so the backends differ by 2
units, about 84 times less than the tolerance.

gamma's seventh receipt is the first with half the work. Its draw, 87, is
below the threshold of 146, so it is checked, disagrees on value 0 and is
withheld. gamma is suspended, and its last three receipts are refused.

The transcript, the same in debug and release
([`peer-audit.out`](../fixtures/peer-audit.out)):

```text
model 4aa6751881c6f068f48932f6f82503815b312e797cb3c88d2d1b7ac316d56ca0
request 0, value 0: seq -0.4938019, pair -0.49380195
alpha #1: 1012 msat, draw 209 against 256 (floor 32): checked, agree exact
alpha #2: 1013 msat, draw 135 against 256 (floor 32): checked, agree exact
alpha #3: 1013 msat, draw 243 against 256 (floor 32): checked, agree exact
alpha #4: 1014 msat, draw 9 against 256 (floor 32): checked, agree exact
alpha #5: 1014 msat, draw 66 against 204 (floor 32): checked, agree exact
alpha #6: 1015 msat, draw 234 against 170 (floor 32): not checked
alpha #7: 1015 msat, draw 5 against 170 (floor 32): checked, agree exact
alpha #8: 1016 msat, draw 26 against 146 (floor 32): checked, agree exact
alpha #9: 1016 msat, draw 237 against 128 (floor 32): not checked
alpha #10: 1017 msat, draw 168 against 128 (floor 32): not checked
beta #1: 1012 msat, draw 54 against 256 (floor 32): checked, agree within, distance 4.76837158203125e-07 at value 2, tolerance 40000 ppb
beta #2: 1013 msat, draw 101 against 256 (floor 32): checked, agree within, distance 2.384185791015625e-07 at value 0, tolerance 40000 ppb
beta #3: 1013 msat, draw 26 against 256 (floor 32): checked, agree within, distance 3.5762786865234375e-07 at value 4, tolerance 40000 ppb
beta #4: 1014 msat, draw 211 against 256 (floor 32): checked, agree within, distance 4.76837158203125e-07 at value 2, tolerance 40000 ppb
beta #5: 1014 msat, draw 117 against 204 (floor 32): checked, agree within, distance 2.384185791015625e-07 at value 0, tolerance 40000 ppb
beta #6: 1015 msat, draw 233 against 170 (floor 32): not checked
beta #7: 1015 msat, draw 121 against 170 (floor 32): checked, agree within, distance 2.384185791015625e-07 at value 0, tolerance 40000 ppb
beta #8: 1016 msat, draw 106 against 146 (floor 32): checked, agree within, distance 2.384185791015625e-07 at value 7, tolerance 40000 ppb
beta #9: 1016 msat, draw 220 against 128 (floor 32): not checked
beta #10: 1017 msat, draw 37 against 128 (floor 32): checked, agree within, distance 4.172325134277344e-07 at value 2, tolerance 40000 ppb
gamma #1: 1012 msat, draw 224 against 256 (floor 32): checked, agree exact
gamma #2: 1013 msat, draw 68 against 256 (floor 32): checked, agree exact
gamma #3: 1013 msat, draw 176 against 256 (floor 32): checked, agree exact
gamma #4: 1014 msat, draw 10 against 256 (floor 32): checked, agree exact
gamma #5: 1014 msat, draw 150 against 204 (floor 32): checked, agree exact
gamma #6: 1015 msat, draw 111 against 170 (floor 32): checked, agree exact
gamma #7: 1015 msat, draw 87 against 146 (floor 32): checked, disagree bits at value 0
gamma #8: refused, suspended
gamma #9: refused, suspended
gamma #10: refused, suspended
tampering:
an answer changed after its receipt: alpha #11: refused, output
a receipt naming another model: alpha #11: refused, model
a receipt altered on the way: alpha #11: refused, forged
a receipt counted before: alpha #3: refused, replayed
the untouched receipt: alpha #11: 1017 msat, draw 47 against 128 (floor 32): checked, agree exact
beta's output under the exact profile: disagree bits at value 0
a receipt for stage-f32-gpu-v1: unverifiable profile
alpha ledger: paid 11162 msat, withheld 0, held 0; 8 agreed in a row, 0 disagreed, 0 unverifiable, 3 not checked, suspended false
beta ledger: paid 10145 msat, withheld 0, held 0; 8 agreed in a row, 0 disagreed, 0 unverifiable, 2 not checked, suspended false
gamma ledger: paid 6081 msat, withheld 1015, held 0; 0 agreed in a row, 1 disagreed, 0 unverifiable, 0 not checked, suspended true
recount alpha: 11 records, same ledger true; last record cbed120b23e7ff9791232d622e5c3dee7adde11bbfa28e524d469009776e01ad
recount beta: 10 records, same ledger true; last record 9af5b6d73c4bf3b55990444398d1e63dfd8383518e666214d819ad80ec68eab3
recount gamma: 7 records, same ledger true; last record a5ee85c8cff6ee1642ec4f04de36f6da816da646fd0c2a5d303503bd5fec8300
alpha's records with one counted twice: refused, replayed at record 11
```

The ledgers add up by hand. alpha is paid 1012 + 1013 + 1013 + 1014 + 1014 +
1015 + 1015 + 1016 + 1016 + 1017 = 10,145 msat for its ten receipts, plus
1017 for the eleventh: 11,162. gamma is paid 6081 msat for its six honest
answers, and the 1015 msat for the seventh is withheld.

Sampling has a cost that the transcript shows: alpha's and beta's unchecked
answers were paid without being checked. If gamma had cheated only on
answers whose draws fell above the threshold, it would have been paid for
them. Sampling makes cheating a gamble: each wrong answer is caught with
probability at least floor/256, here 1 in 8, and one catch costs the peer
its standing and its withheld pay.

## What remains

- **Signed receipts.** Receipts carry an HMAC under a key the peer shares
  with the checker. The checker can verify them, but a third party cannot.
  A reader who recounts the records trusts the checker's word that each
  receipt was authentic. Receipts that anyone can verify need BIP-340
  signatures, as chapter 26's receipt.v2 events have, and a published
  schema in `proto/`.
- **Real backends.** The two backends here are two summation orders on one
  CPU. A GPU stage, or a stage on another CPU's vector units, needs its own
  bound, or a measured limit like chapter 219's, and a qualified profile
  ([chapter 17](ch017-qualify-honestly.md)).
- **Exact replay of whole stages.** Chapter 234 replays traces of chapter
  226's machine; replaying a model stage step by step to find the first
  step that differs is not built.
- **Pipelines over the wire.** One stage runs in one process. Checking every
  stage of a multi-stage group, with activations carried over chapter 188's
  encrypted links, is not built.
- **Payment.** The ledger says what is owed in millisatoshis. Paying peers,
  or lifting a suspension, is an owner decision in the wallet, outside
  Psionic.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [Private semantic comparison](../docs/serving/mesh-comparison.md)
- N. J. Higham, *Accuracy and Stability of Numerical Algorithms*, 2nd ed.,
  SIAM 2002, chapter 4 (summation error bounds).
