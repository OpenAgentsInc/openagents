# 222. A decision in milliseconds

> **Status: partly implemented.** A small ModernBERT-shaped encoder
> (profile fe-tiny-v1) with fixed decision heads runs on two CPU engines: a
> binary64 reference and a q8 engine with uint8 embedding rows and chapter
> 48's int8 products. Parity against the reference, chapter 50's quality card
> and a 20 ms P99 gate at 512 tokens run, and a receipt records cold and warm
> latency on one CPU. The real s1-llm-auto-router (11-layer mmBERT-small)
> loaded through [chapter 220](ch220-read-a-bounded-onnx-graph.md), rotary
> positions, a GPU engine and multi-core execution are not built; the last
> section lists them.

Some decisions must arrive in milliseconds: a router that picks a model before
the request is sent, a filter in front of a voice turn. A large decoder cannot
answer that fast. An encoder can: it reads the whole question once, in both
directions, and fixed heads turn its pooled state into class probabilities. No
token is generated.

This chapter builds such an encoder ([`fast-encoder.coax`](../lib/fast-encoder.coax)) small enough to check completely, runs it
on two engines, and asks three separate questions about the faster one:

- **Execution parity:** does it make the same decisions as the reference, and
  how far apart are the logits?
- **Decision quality:** does chapter 50's quality card promote it, measured on
  the probabilities?
- **Latency:** is P99 at 512 tokens within 20 ms on this CPU?

The answers are kept apart. A fast engine that changes decisions is refused
before its latency is looked at.

```shell
coax/bin/coax test psionic/lib/fast-encoder.coax
coax/bin/coax run psionic/examples/fast-decision.coax
psionic/tests/check_fast_encoder.sh 200 20 RECEIPT.json   # times, on this CPU
```

## The model

The pre-reset work used the s1-llm-auto-router: an 11-layer mmBERT-small
encoder with 119.4M parameters (98.3M of them `uint8` embeddings) and seven
heads, exported as ONNX. Its vendor reports 14 ms P50 on four CPU cores. Those
are historical and vendor figures, not Psionic results, and that model is not
loaded here.

fe-tiny-v1 keeps the architecture's parts at a size every check can cover:

- **Embedding rows:** 64 token IDs of width 16.
- **Three pre-norm layers.** Each layer normalizes the residual stream
  (layer norm without parameters, epsilon 10^-5), projects queries, keys and
  values (48 rows), attends with two heads of 8, projects back (16 rows) and
  adds; then normalizes again, runs a GeGLU feed-forward layer (64 rows up,
  GELU of the first 32 times the second 32, 16 rows down) and adds.
- **Local and global attention,** as in ModernBERT: layer 0 attends to every
  key; layers 1 and 2 only to keys at most 64 positions away (a window of
  129).
- **Mean pooling and three heads** of 2, 3 and 4 classes. Each head turns
  logits into probabilities with its own temperature: 1, 1/2 and 2.
- **No position encoding.** ModernBERT uses rotary positions; the stand-in
  omits them, so only the local windows make position matter.

The weights are SplitMix64 words at a counter (seed 222), scaled per part. The
model's identity is FNV-1a over the 64 bits of every weight. The engine's
identity is its name. They are separate: the same model has two engines here,
and a receipt names both.

<!-- source: psionic/lib/fast-encoder.coax shape -->
```jai
FE_PROFILE :: "fe-tiny-v1";
FE_VOCAB :: 64;
FE_D :: 16;         // width
FE_HD :: 8;         // width of each of the two attention heads
FE_LAYERS :: 3;
FE_FF :: 32;        // GeGLU width: the up product has 64 rows
FE_WINDOW :: 64;    // a local layer sees keys at most 64 positions away
FE_GLOBAL :: 3;     // layers 0, 3, 6, ... attend globally
FE_TOKENS :: 512;   // the most token IDs one decision reads
FE_BLOCK :: 16;     // int8 block width
FE_CHUNK :: 128;    // tokens per int8 product call
FE_EMB :: 1024;     // 64 rows of 16
FE_LAYER_W :: 2560; // qkv 48 x 16, output 16 x 16, up 64 x 16, down 16 x 32
FE_HEAD_AT :: 8704; // 1024 + 3 x 2560
FE_WEIGHTS :: 8848; // and 9 head rows of 16
FE_CODES :: 7680;   // the int8 codes of the three layers
FE_LOGITS :: 9;
FE_CLASSES :: [3]int.[2,3,4];
FE_CLASS_AT :: [3]int.[0,2,5];
FE_TEMPS :: [3]f64.[1,0.5,2];
```

## Work it by hand first

**Multiply-adds.** Each layer's four products cost 48×16 + 16×16 + 64×16 +
16×32 = 2,560 per token, so 3 × 512 × 2,560 = 3,932,160 at 512 tokens.
Attention costs 32 per query-key pair (8 for each head's score and 8 for each
head's value, two heads). Layer 0 reads every pair: 512² × 32 = 8,388,608.
A local layer reads fewer keys at the edges: queries 0 to 63 read 65 to 128
keys (64 × 65 + 2,016 = 6,176), queries 64 to 447 read 129 each (49,536), and
the last 64 mirror the first (6,176), so 61,888 pairs, or 1,980,416 per local
layer. The heads add 9 × 16 = 144. The total is

    3,932,160 + 8,388,608 + 2 × 1,980,416 + 144 = 16,281,744

multiply-adds per decision, more than half of them in the one global layer.
`fe_macs(512)` must equal this, and a check asserts it. Two consequences
follow before anything is timed. First, the q8 engine quantizes only the
products, about a quarter of the work, so it cannot be much faster than the
reference on this shape; whatever it saves must also pay for quantizing every
activation. Second, at one multiply-add per nanosecond on one core, a decision
takes about 16 ms: the 20 ms budget is close.

**A uint8 embedding row.** A row's least value becomes `low`, and
`step = (most - least)/255`. Each value is stored as the nearest whole number
of steps above `low`, so the least value is code 0, the largest is code 255,
and every value is within half a step. The 64 rows of 16 take 1,024 bytes
plus 64 lows and steps.

**An int8 weight block.** Take a block of 16 weights that are all 0 except the
last, 0.125. Chapter 48's scale is the least f32 at or above 0.125/127, and
each code is the nearest whole number of scales: 0 for the zeros, and 127 for
0.125. A block of 16 zeros has no scale and is refused. A check builds exactly
this block.

**Attention without overflow.** Give query 0 and key 0 of one head the
vector (50, 0, ..., 0) and key 1 (-50, 0, ..., 0). The scores are
±2,500/√8 ≈ ±883.9, and exp(883.9) overflows binary64. Subtracting the largest
score first gives exp(0) = 1 and exp(-1,767.8) = 0, so the output is key 0's
value exactly. A check runs this case.

**Nearest rank.** For 100 warm times, P50 is the 50th smallest and P99 the 99th
(chapter 18's rule: rank ceil(p n/100)). With fewer than 100 times, P99 is the
maximum, so the gate refuses fewer than 100.

## Two engines

`fe-f64-reference-v1` computes everything in binary64 from the original
weights. `fe-q8-cpu-v1` reads the uint8 embedding rows and runs each layer's
four products through chapter 48's row kernel: activations are quantized to
int8 blocks of 16, in chunks of 128 tokens so each call stays within chapter
48's work limit. Attention, norms, GELU and the heads stay binary64 in both.
The q8 copy takes 14,720 bytes; the binary64 original takes 70,784.

<!-- source: psionic/lib/fast-encoder.coax quantize -->
```jai
fe_q8_bytes :: ()->int {return FE_EMB+2*8*FE_VOCAB+FE_CODES+2*4*FE_CODES/FE_BLOCK+8*(FE_WEIGHTS-FE_HEAD_AT);}
// Embedding rows: low = the row's least value, step = (most - least)/255,
// code = the nearest whole number of steps. Layers: chapter 48's quantizer,
// each 16 weights one block with its own scale.
fe_quantize :: (w:[]f64,q:FeQ8)->FeStatus #export_c {
    if w.count!=FE_WEIGHTS || !fe_q8_ok(q) {return FeStatus.storage;}
    for v:w {if !(abs(v)<=1000000) {return FeStatus.number;}}
    for r:0..<FE_VOCAB {
        lo:=w[r*FE_D];hi:=lo;
        for k:1..<FE_D {lo=-qz_larger(-lo,-w[r*FE_D+k]);hi=qz_larger(hi,w[r*FE_D+k]);}
        if !(hi>lo) {return FeStatus.number;}
        q.low[r]=lo;q.step[r]=(hi-lo)/255;
        for k:0..<FE_D {q.emb[r*FE_D+k]=cast(u8)int8_nearest((w[r*FE_D+k]-lo)/q.step[r]);}
    }
    // Chapter 48's rounding: the scale is the least f32 at or above amax/127.
    for b:0..<FE_CODES/FE_BLOCK {
        top:=0.0;
        for k:0..<FE_BLOCK {top=qz_larger(top,abs(f64(f32(w[FE_EMB+b*FE_BLOCK+k]))));}
        if top==0 {return FeStatus.number;}
        d:=int8_scale(f32(top));q.scales[b]=d;q.offsets[b]=0;
        for k:0..<FE_BLOCK {q.codes[b*FE_BLOCK+k]=cast(s8)int8_nearest(f64(f32(w[FE_EMB+b*FE_BLOCK+k]))/f64(d));}
    }
    return FeStatus.ok;
}
```

<!-- source: psionic/lib/fast-encoder.coax linear -->
```jai
// rows x width weights at w[at] times t inputs at x[xa], into y[ya]: binary64,
// or chapter 48's int8 row kernel in chunks of 128 tokens.
fe_linear :: (e:FeEngine,w:[]f64,q:FeQ8,k:FeWork,at:int,rows:int,width:int,t:int,x:[]f64,xa:int,y:[]f64,ya:int)->FeStatus {
    if e==FeEngine.reference {
        for i:0..<t {for r:0..<rows {
            acc:=0.0;
            for c:0..<width {acc+=w[at+r*width+c]*x[xa+i*width+c];}
            y[ya+i*rows+r]=acc;
        }}
        return FeStatus.ok;
    }
    c0:=at-FE_EMB;nb:=width/FE_BLOCK;
    wq:=Int8Weights.{codes=q.codes[c0..<c0+rows*width],scales=q.scales[c0/FE_BLOCK..<(c0+rows*width)/FE_BLOCK],offsets=q.offsets[c0/FE_BLOCK..<(c0+rows*width)/FE_BLOCK]};
    for s:0..<(t+FE_CHUNK-1)/FE_CHUNK {
        n:=fe_imin(FE_CHUNK,t-s*FE_CHUNK);r:=Int8Request.{rows=rows,width=width,tokens=n,block=FE_BLOCK};
        for i:0..<n*width {k.a[i]=f32(x[xa+s*FE_CHUNK*width+i]);}
        a:=Int8Activations.{codes=k.codes[0..<n*width],scales=k.scales[0..<n*nb],sums=k.sums[0..<n*nb]};
        st:=int8_quantize(r,k.a[0..<n*width],a).status;
        if st==Int8Status.ok {st=int8_row(r,wq,a,k.dots,y[ya+s*FE_CHUNK*rows..<ya+(s*FE_CHUNK+n)*rows]).status;}
        if st!=Int8Status.ok {return FeStatus.int8;}
    }
    return FeStatus.ok;
}
```

The forward pass checks everything before it computes: the engine, 1 to 512
token IDs within 0 to 63, every slice's length and the work storage. The work
is 192 binary64 values per token, laid out region after region. Attention's
output comes last, so writing past it runs past the whole buffer, which the
checks size exactly; the residual stream sits just before it, so the
feed-forward layer writing past its own region would corrupt live values. A
layout mistake fails a check instead of passing silently.

<!-- source: psionic/lib/fast-encoder.coax forward -->
```jai
fe_plan_f64 :: (t:int)->int {return t*FE_ROW;}
fe_q8_ok :: (q:FeQ8)->bool {
    return q.emb.count==FE_EMB && q.low.count==FE_VOCAB && q.step.count==FE_VOCAB && q.codes.count==FE_CODES
        && q.scales.count==FE_CODES/FE_BLOCK && q.offsets.count==FE_CODES/FE_BLOCK;
}
fe_forward :: (e:FeEngine,w:[]f64,q:FeQ8,k:FeWork,ids:[]int,z:[]f64)->FeStatus #export_c {
    t:=ids.count;
    if e==FeEngine.none {return FeStatus.engine;}
    if t<1 || t>FE_TOKENS {return FeStatus.tokens;}
    for v:ids {if v<0 || v>=FE_VOCAB {return FeStatus.id;}}
    if w.count!=FE_WEIGHTS || z.count!=FE_LOGITS || (e==FeEngine.q8 && !fe_q8_ok(q)) {return FeStatus.storage;}
    if k.f.count<fe_plan_f64(t) {return FeStatus.capacity;}
    if e==FeEngine.q8 {
        if k.a.count<FE_CHUNK*FE_FF || k.codes.count<FE_CHUNK*FE_FF || k.scales.count<FE_CHUNK*FE_FF/FE_BLOCK {return FeStatus.capacity;}
        if k.sums.count<FE_CHUNK*FE_FF/FE_BLOCK || k.dots.count<FE_CHUNK*2*FE_FF {return FeStatus.capacity;}
    }
    // Rows per token, in order: h (16), qkv (48), u (64), g (32), the residual
    // stream x (16) and att (16). Attention's scores use u, which is free then.
    f:=k.f;h:=0;qkv:=h+t*FE_D;u:=qkv+t*3*FE_D;g:=u+t*2*FE_FF;x:=g+t*FE_FF;att:=fe_plan_f64(t)-t*FE_D;
    for i:0..<t {for c:0..<FE_D {
        f[x+i*FE_D+c]=ifx e==FeEngine.reference then w[ids[i]*FE_D+c] else q.low[ids[i]]+q.step[ids[i]]*f64(q.emb[ids[i]*FE_D+c]);
    }}
    for l:0..<FE_LAYERS {
        at:=FE_EMB+l*FE_LAYER_W;
        for i:0..<t {fe_norm(f,x+i*FE_D,f,h+i*FE_D);}
        st:=fe_linear(e,w,q,k,at,48,FE_D,t,f,h,f,qkv);
        if st!=FeStatus.ok {return st;}
        fe_attend(l,t,f,qkv,att,u);
        st=fe_linear(e,w,q,k,at+768,FE_D,FE_D,t,f,att,f,h);
        if st!=FeStatus.ok {return st;}
        for i:0..<t {for c:0..<FE_D {f[x+i*FE_D+c]+=f[h+i*FE_D+c];}}
        for i:0..<t {fe_norm(f,x+i*FE_D,f,h+i*FE_D);}
        st=fe_linear(e,w,q,k,at+1024,2*FE_FF,FE_D,t,f,h,f,u);
        if st!=FeStatus.ok {return st;}
        for i:0..<t {for c:0..<FE_FF {f[g+i*FE_FF+c]=fe_gelu(f[u+i*2*FE_FF+c])*f[u+i*2*FE_FF+FE_FF+c];}}
        st=fe_linear(e,w,q,k,at+2048,FE_D,FE_FF,t,f,g,f,h);
        if st!=FeStatus.ok {return st;}
        for i:0..<t {for c:0..<FE_D {f[x+i*FE_D+c]+=f[h+i*FE_D+c];}}
    }
    // Mean of the normed rows, then the heads in binary64.
    pool:[16]f64;
    for i:0..<t {fe_norm(f,x+i*FE_D,f,h);for c:0..<FE_D {pool[c]+=f[h+c];}}
    for o:0..<FE_LOGITS {
        acc:=0.0;
        for c:0..<FE_D {acc+=w[FE_HEAD_AT+o*FE_D+c]*(pool[c]/f64(t));}
        z[o]=acc;
    }
    return FeStatus.ok;
}
```

An engine is chosen by name. `fe-q8-cuda-v1` is a known name with no engine
built here, so it is refused; nothing runs in its place.

<!-- source: psionic/lib/fast-encoder.coax select -->
```jai
fe_engine_name :: (e:FeEngine)->string {names:=[3]string.["none","fe-f64-reference-v1","fe-q8-cpu-v1"];return names[cast(int)e];}
// An engine is named, never inferred. A known engine that is not built here
// is refused; nothing else runs in its place.
fe_select :: (name:string)->FeChoice {
    for e:1..<3 {if name==fe_engine_name(cast(FeEngine)e) {return FeChoice.{engine=cast(FeEngine)e};}}
    if name=="fe-q8-cuda-v1" {return FeChoice.{status=FeStatus.unavailable};}
    return FeChoice.{status=FeStatus.engine};
}
```

## Parity, the card and the gate

Three bounds were frozen before anything was measured:

<!-- source: psionic/lib/fast-encoder.coax frozen -->
```jai
// Frozen before any measurement.
FE_LOGIT_BOUND :: 0.125;        // the largest |q8 logit - reference logit|
FE_CARD_KL :: 0.001;            // chapter 50's card: mean KL, pessimistic end
FE_CARD_AGREEMENT :: 0.95;      // and top-1 agreement, pessimistic end
FE_P99_LIMIT :: 20000000;       // P99 of a 512-token decision: 20 ms, in ns
FE_SAMPLES_MIN :: 100;          // fewer and nearest-rank P99 is the maximum
```

Parity compares every head's argmax and the largest absolute logit difference.
The card is chapter 50's `qz_card` over every (request, head) pair. The
stand-in has no labeled data, so each item's gold class is the reference's
decision: the card measures how far quantization moves the decisions, not
whether they are right.

<!-- source: psionic/lib/fast-encoder.coax parity -->
```jai
fe_argmax :: (z:[]f64,at:int,n:int)->int {b:=at;for c:at+1..<at+n {if z[c]>z[b] {b=c;}}return b;}
// n requests of 9 logits each: every head's argmax must match, and the
// largest difference must be within the frozen bound.
fe_parity :: (reference:[]f64,derived:[]f64,n:int)->FeParity #export_c {
    if n<1 || reference.count!=n*FE_LOGITS || derived.count!=reference.count {return FeParity.{};}
    p:=FeParity.{decisions=3*n};
    for r:0..<n {for h:0..<3 {
        at:=r*FE_LOGITS+FE_CLASS_AT[h];
        if fe_argmax(reference,at,FE_CLASSES[h])==fe_argmax(derived,at,FE_CLASSES[h]) {p.equal+=1;}
    }}
    for i:0..<reference.count {p.worst=qz_larger(p.worst,abs(derived[i]-reference[i]));}
    p.ok=p.equal==p.decisions && p.worst<=FE_LOGIT_BOUND;
    return p;
}
// Chapter 50's card over every (request, head): gold is the reference
// decision, since the stand-in has no labels; the card measures how far
// quantization moves the decisions.
fe_card :: (reference:[]f64,derived:[]f64,n:int,items:[]PqItem)->QzCard #export_c {
    if items.count<3*n {return QzCard.{status=QzStatus.capacity};}
    for r:0..<n {for h:0..<3 {
        at:=r*FE_LOGITS+FE_CLASS_AT[h];
        items[3*r+h]=PqItem.{kind=QrKind.choice,count=FE_CLASSES[h],start=at,gold=fe_argmax(reference,at,FE_CLASSES[h])-at,abstain=-1};
    }}
    return qz_card(QzBound.{kl=FE_CARD_KL,agreement=FE_CARD_AGREEMENT},items[0..<3*n],reference,derived,fe_q8_bytes());
}
```

The gate refuses unless the q8 engine ran the frozen 512-token workload with
parity and a promoted card, on 100 to 1,024 positive warm times, against the
frozen limit. Only then does it compare P99 with 20 ms. Mean and standard
deviation come from chapter 34's Welford pass.

<!-- source: psionic/lib/fast-encoder.coax gate -->
```jai
// Chapter 18's nearest rank: the value at rank ceil(p n / 100) in sorted order,
// counting from 1; it holds that rank when fewer values are below it and at
// least that many are at or below it.
fe_rank :: (x:[]int,p:int)->int #export_c {
    r:=(p*x.count+99)/100;
    for v:x {
        below:=0;atmost:=0;
        for u:x {if u<v {below+=1;} if u<=v {atmost+=1;}}
        if below<r && atmost>=r {return v;}
    }
    return 0;
}
// Refused unless the q8 engine ran the frozen workload with parity and a
// promoted card, on 100 to 1,024 positive warm times, against the frozen limit.
fe_gate :: (e:FeEngine,tokens:int,p:FeParity,c:QzCard,ns:[]int,limit:int)->FeGate #export_c {
    g:=FeGate.{limit=limit};
    if e!=FeEngine.q8 {g.reason=FeReason.engine;return g;}
    if tokens!=FE_TOKENS || limit!=FE_P99_LIMIT {g.reason=FeReason.workload;return g;}
    if !p.ok {g.reason=FeReason.parity;return g;}
    if !c.promoted {g.reason=FeReason.card;return g;}
    st:=cm_welford(ns);
    if ns.count<FE_SAMPLES_MIN || st.status!=CmStatus.ok {g.reason=FeReason.samples;return g;}
    g.n=ns.count;g.p50=fe_rank(ns,50);g.p99=fe_rank(ns,99);g.mean=st.mean;g.sd=st.sd;
    g.verdict=ifx g.p99<=limit then FeVerdict.met else FeVerdict.missed;
    return g;
}
```

## Run it

The [example](../examples/fast-decision.coax) builds the model, quantizes it, selects engines and runs eight
requests of 512 token IDs on both engines:

<!-- source: psionic/examples/fast-decision.coax example -->
```jai
main :: () {
    w:[8848]f64;e:[1024]u8;lo:[64]f64;st:[64]f64;c:[7680]s8;s:[480]f32;o:[480]f32;
    #assert fe_weights(w[..])==FeStatus.ok;
    q:=FeQ8.{emb=e[..],low=lo[..],step=st[..],codes=c[..],scales=s[..],offsets=o[..]};
    #assert fe_quantize(w[..],q)==FeStatus.ok;
    print("model %: % weights, identity %\n",FE_PROFILE,FE_WEIGHTS,fe_model_id(w[..]));
    print("q8 copy: % bytes; binary64 original: % bytes\n",fe_q8_bytes(),8*FE_WEIGHTS);
    names:=[3]string.["fe-f64-reference-v1","fe-q8-cpu-v1","fe-q8-cuda-v1"];
    for name:names {ch:=fe_select(name);print("engine %: %\n",name,ifx ch.status==FeStatus.ok then "selected" else fe_status_name(ch.status));}
    f:[98816]f64;a:[4096]f32;ac:[4096]s8;as:[256]f32;am:[256]s32;d:[8192]s32;
    k:=FeWork.{f=f[..],a=a[..],codes=ac[..],scales=as[..],sums=am[..],dots=d[..]};
    ref:[72]f64;der:[72]f64;ids:[512]int;
    for r:0..<N {
        for i:0..<512 {ids[i]=cast(int)(cr_bits(cast(u64)(2230+r),cast(u64)i)%64);}
        #assert fe_forward(FeEngine.reference,w[..],q,k,ids[..],ref[9*r..<9*r+9])==FeStatus.ok;
        #assert fe_forward(FeEngine.q8,w[..],q,k,ids[..],der[9*r..<9*r+9])==FeStatus.ok;
        worst:=0.0;for i:9*r..<9*r+9 {worst=qz_larger(worst,abs(der[i]-ref[i]));}
        print("request %, 512 IDs: reference classes % % %, q8 classes % % %, largest logit difference %\n",r,
            fe_argmax(ref[..],9*r,2)-9*r,fe_argmax(ref[..],9*r+2,3)-9*r-2,fe_argmax(ref[..],9*r+5,4)-9*r-5,
            fe_argmax(der[..],9*r,2)-9*r,fe_argmax(der[..],9*r+2,3)-9*r-2,fe_argmax(der[..],9*r+5,4)-9*r-5,worst);
    }
    p:=fe_parity(ref[..],der[..],N);
    print("parity: % of % head decisions equal, largest difference % against the frozen %: %\n",p.equal,p.decisions,p.worst,FE_LOGIT_BOUND,ifx p.ok then "pass" else "fail");
    pr:[72]f64;pd:[72]f64;
    for r:0..<N {fe_probs(ref[9*r..<9*r+9],pr[9*r..<9*r+9]);fe_probs(der[9*r..<9*r+9],pd[9*r..<9*r+9]);}
    items:[24]PqItem;card:=fe_card(pr[..],pd[..],N,items[..]);
    print("card: % items, agreement % (low end %), mean KL % (high end %), promoted: %\n",card.items,card.agreement,card.agreement_low,card.kl,card.kl_high,card.promoted);
    print("multiply-adds per 512-token decision: %\n",fe_macs(512));
}
```

```text
model fe-tiny-v1: 8848 weights, identity 872817745440994249
q8 copy: 14720 bytes; binary64 original: 70784 bytes
engine fe-f64-reference-v1: selected
engine fe-q8-cpu-v1: selected
engine fe-q8-cuda-v1: engine not built here
request 0, 512 IDs: reference classes 1 1 1, q8 classes 1 1 1, largest logit difference 0.012599590954098572
request 1, 512 IDs: reference classes 0 0 1, q8 classes 0 0 1, largest logit difference 0.019142544816567852
request 2, 512 IDs: reference classes 1 0 1, q8 classes 1 0 1, largest logit difference 0.012181087788577488
request 3, 512 IDs: reference classes 1 0 1, q8 classes 1 0 1, largest logit difference 0.01709745657637496
request 4, 512 IDs: reference classes 1 1 1, q8 classes 1 1 1, largest logit difference 0.003513975769895983
request 5, 512 IDs: reference classes 1 0 1, q8 classes 1 0 1, largest logit difference 0.015359540811110323
request 6, 512 IDs: reference classes 1 0 1, q8 classes 1 0 1, largest logit difference 0.015258047307834532
request 7, 512 IDs: reference classes 1 0 1, q8 classes 1 0 1, largest logit difference 0.015161013652510102
parity: 24 of 24 head decisions equal, largest difference 0.019142544816567852 against the frozen 0.125: pass
card: 24 items, agreement 1.0 (low end 1.0), mean KL 0.000030248108743864254 (high end 0.00004349472788954366), promoted: true
multiply-adds per 512-token decision: 16281744
```

Parity passes with the largest logit difference about a sixth of the frozen
bound, and the card promotes with a mean KL divergence near 3 × 10^-5. Most
heads choose the same class on most requests: the weights are random, so
these decisions mean nothing beyond what they test.

## What is refused

The [refusal example](../examples/fast-decision-refuse.coax) breaks one
thing at a time:

<!-- source: psionic/examples/fast-decision-refuse.coax refuse -->
```jai
main :: () {
    for name:[3]string.["fe-q8-cuda-v1","fe-q8-gpu-v1","fe-q8-cpu-v1"] {print("engine %: %\n",name,fe_status_name(fe_select(name).status));}
    w:[8848]f64;e:[1024]u8;lo:[64]f64;st:[64]f64;c:[7680]s8;s:[480]f32;o:[480]f32;
    #assert fe_weights(w[..])==FeStatus.ok;
    q:=FeQ8.{emb=e[..],low=lo[..],step=st[..],codes=c[..],scales=s[..],offsets=o[..]};
    #assert fe_quantize(w[..],q)==FeStatus.ok;
    f:[1728]f64;a:[4096]f32;ac:[4096]s8;as:[256]f32;am:[256]s32;d:[8192]s32;
    k:=FeWork.{f=f[..],a=a[..],codes=ac[..],scales=as[..],sums=am[..],dots=d[..]};
    ids:=[10]int.[1,2,3,4,5,6,7,8,9,64];z:[9]f64;
    print("token ID 64: %\n",fe_status_name(fe_forward(FeEngine.q8,w[..],q,k,ids[..],z[..])));
    ids[9]=10;
    print("no engine named: %\n",fe_status_name(fe_forward(FeEngine.none,w[..],q,k,ids[..],z[..])));
    print("work for 9 tokens, 10 given: %\n",fe_status_name(fe_forward(FeEngine.q8,w[..],q,k,ids[..],z[..])));
    big:[2000]f64;k.f=big[..];c[100]=-128;
    print("a code of -128 in layer 0: %\n",fe_status_name(fe_forward(FeEngine.q8,w[..],q,k,ids[..],z[..])));
    for i:0..<16 {w[16*7+i]=0.25;}
    print("embedding row 7 constant: %\n",fe_status_name(fe_quantize(w[..],q)));
    // Gates: teaching times in ns, not measurements.
    ns:[100]int;for i:0..<100 {ns[i]=12000000+i*50000;}
    ok:=FeParity.{decisions=24,equal=24,worst=0.01,ok=true};card:=QzCard.{promoted=true};
    judge("the frozen workload",FeEngine.q8,512,ok,card,ns[..]);
    judge("timed on the reference engine",FeEngine.reference,512,ok,card,ns[..]);
    judge("256 token IDs",FeEngine.q8,256,ok,card,ns[..]);
    flip:=ok;flip.equal=23;flip.ok=false;judge("one head decision flipped",FeEngine.q8,512,flip,card,ns[..]);
    judge("card not promoted",FeEngine.q8,512,ok,QzCard.{},ns[..]);
    judge("99 warm runs",FeEngine.q8,512,ok,card,ns[0..<99]);
    ns[98]=21000000;ns[99]=22000000;judge("two runs over 20 ms",FeEngine.q8,512,ok,card,ns[..]);
}
```

```text
engine fe-q8-cuda-v1: engine not built here
engine fe-q8-gpu-v1: no such engine
engine fe-q8-cpu-v1: ok
token ID 64: a token ID outside 0 to 63
no engine named: no such engine
work for 9 tokens, 10 given: work storage too small
a code of -128 in layer 0: refused by the int8 product
embedding row 7 constant: a row with no spread
the frozen workload: met (none), P99 16900000 ns
timed on the reference engine: refused (the engine is not fe-q8-cpu-v1), P99 0 ns
256 token IDs: refused (not the frozen 512-token workload), P99 0 ns
one head decision flipped: refused (q8 decisions differ from the reference), P99 0 ns
card not promoted: refused (the quality card did not promote), P99 0 ns
99 warm runs: refused (fewer than 100 or more than 1,024 warm runs, or a time that is not positive), P99 0 ns
two runs over 20 ms: missed (none), P99 21000000 ns
```

The gate lines use teaching times, not measurements. The last line is not a
refusal: two of 100 runs over 20 ms put the 99th smallest over the limit, and
the gate says missed.

## Measured on this CPU

[`check_fast_encoder.sh`](../tests/check_fast_encoder.sh) runs the checks
and both transcripts, compiles the library to C in release mode with
[`fast_encoder_bench.c`](../tests/fast_encoder_bench.c), and measures. The C
host only reads the monotonic clock and writes the receipt; the forward pass,
parity, card, percentiles and verdict are the Coax functions above. Cold
times come from fresh processes, one decision each, alternating engines.
Warm times follow one untimed decision per engine, as interleaved pairs whose
order alternates, over the example's eight requests. One thread.

On an Intel Core i7-14700K (Linux 6.18, GCC 15.2, `-O2`), one thread, a
shared host with a load average near 9, 20 cold processes per engine and 200
warm pairs:

| Engine | Cold P50 | Cold P99 (the maximum of 20) | Warm P50 | Warm P99 | Warm mean ± sd |
| --- | --- | --- | --- | --- | --- |
| fe-q8-cpu-v1 | 13.46 ms | 15.30 ms | 12.32 ms | 12.75 ms | 12.33 ± 0.12 ms |
| fe-f64-reference-v1 | 11.30 ms | 16.89 ms | 10.07 ms | 10.35 ms | 10.08 ± 0.11 ms |

Parity passed (24 of 24 head decisions, largest logit difference 0.0191),
the card promoted, and the gate met its frozen limit: warm P99 12.75 ms
against 20 ms. The hand count predicted the order: 16,281,744 multiply-adds in
10.07 ms is 1.6 per nanosecond on the reference engine. The q8 engine is
**slower**, by about 2.3 ms. Its products are only a quarter of the work, and
each call quantizes every activation and re-admits the weights (chapter 48's
row kernel checks every code and scale on each call), which costs more than
integer arithmetic saves at width 16. The 20 ms target is met because the
model is small, not because of quantization. Quantization here buys memory
(14,720 bytes against 70,784), which matters for the real model's 98.3M
embedding values, not speed.

The [receipt](../fixtures/fast-encoder/receipt-cpu-2026-11-01.json) keeps
every raw time, the build, commit, machine and load. Times are a shared host's
observations, not a capacity claim.

## The independent checks

- **A second implementation.** [`fast_encoder_oracle.py`](../tests/fast_encoder_oracle.py)
  writes the model again in plain Python binary64 and records the reference
  logits for 1, 5 and 70 token IDs (70 reaches past a local window). The
  reference engine matches within 10^-12, and the model identity matches
  exactly.
- **Per-product bounds.** For every layer and product, the q8 result is
  within chapter 48's `int8_bound` plus the weights' half steps, on 5 tokens,
  and within a looser bound on 128 and 129 tokens, where the chunks split.
- **Hand values:** layer norm, GELU, tanh, the tempered softmax on logits
  chosen to give exact fractions, the attention overflow case, the
  multiply-add count, the window edges, nearest ranks on descending and tied
  inputs.
- **Refusals before work:** each refusal in the transcript is a check, and
  `coax mutate` leaves no mutant of the library alive.

## What is not built

- **The real model.** The s1-llm-auto-router's ONNX graph uses operators
  chapter 220's reader refuses, and its 119.4M parameters are not loaded.
  Parity against it, and its seven heads, wait on that reader.
- **Rotary positions** and ModernBERT's alternating attention period of 3 at
  depth (the stand-in has one global layer).
- **A GPU engine.** `fe-q8-cuda-v1` is named and refused; P50 and P99 on a
  named GPU are not reported.
- **Faster CPU kernels.** One thread, and attention in scalar binary64. The
  vendor's figure used four cores.

Next, [chapter 223](ch223-distill-elephant-into-a-small-router.md) distills
Elephant's decisions into a small router of this kind.
