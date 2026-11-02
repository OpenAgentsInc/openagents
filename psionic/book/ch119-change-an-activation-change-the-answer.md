# 119. Change an activation, change the answer

[Chapter 253](ch253-read-and-repair-a-models-weights.md) uses these interventions
to check permanent parameter edits and tests them across fresh model seeds.

An attention picture can suggest how a model works, but it does not show
cause. This chapter tests an explanation by intervention. You train a tiny
decoder on a task with a known shortcut, then replace activations at named
places inside it and measure whether the shortcut's effect goes away and
comes back.

The runnable profile is `patch-cpu-f64-v1`: binary64 on the CPU, one
thread, every sum in a fixed order, no pretrained weights and no GPU.
The library is [patch.coax](../lib/transformer/patch.coax). It reuses:

- [Chapter 113](ch113-differentiate-a-decoder.md): the decoder, its work
  buffer layout, its initialization and its gradients (`td_init`,
  `td_forward`, `td_gradients`).
- [Chapter 124](ch124-adamw-and-explicit-precision.md): AdamW (`aw_step`).

[Chapter 115](ch115-train-the-whole-tiny-decoder.md) trains the same decoder
on prose. Prose has no shortcut you planted and can check, so this chapter
trains the decoder on a small task of its own instead.

## A task with a known shortcut

Each sequence has five symbols: the boundary (BOS), an answer `a` from 1 to
6, a hint, a query mark, then `a` again. The rule is "the symbol after the
query mark is the one three places back". The hint `6+a` also names the
answer. That is the shortcut: whenever the hint is present it is right, so a
model can get the answer from the hint without reading `a` at all. Hint 13
names nothing.

<!-- source: psionic/lib/transformer/patch.coax task -->
```jai
// The task: BOS, an answer a in 1..6, a hint, the query mark, then a. The
// rule is "copy the symbol three places back". The hint 6+a names the answer
// too; that is the shortcut. Hint 13 names nothing.
AX_SHAPE :: TdShape.{vocab=16,width=8,heads=2,kv_heads=2,head_width=4,hidden=8,layers=1,context=5};
AX_BOS :: 15;
AX_NONE :: 13;
AX_QUERY :: 14;
AX_ANSWER :: 3;
ax_sequence :: (a:int,hint:int)->[5]int {return [5]int.[AX_BOS,a,hint,AX_QUERY,a];}
```

`AX_ANSWER` is position 3, the query mark: its output is the prediction of
the answer, so that is where every measurement below reads the logits.

The training set is twelve sequences: each answer with its hint and each
answer with hint 13. The hinted sequences carry three quarters of the
weight. Every step uses all twelve (a full batch), so there is no sampling
and no data order to record.

<!-- source: psionic/lib/transformer/patch.coax train -->
```jai
// One sequence's share of a step: adds weight times its gradient to total
// and returns weight times its loss.
ax_term :: (p:[]f64,x:[5]int,weight:f64,total:[]f64)->f64 {
    w:[640]f64;dw:[640]f64;g:[664]f64;t:=x;r:=td_gradients(AX_SHAPE,p,t[..],w[..],dw[..],g[..],false);#assert r.status==TdStatus.ok;
    ax_add(total,weight,g[..]);return weight*r.loss;
}
// Full-batch training on the twelve sequences: each answer with its hint
// (weight 3/24) and without (weight 1/24), so the hint is right whenever it
// is present and present three times in four. Returns the last step's loss.
ax_train :: (p:[]f64,steps:int)->f64 {
    s:=AX_SHAPE;seed:=td_init(s,CrSeed.{seed=AX_INIT},p);#assert seed.counter==AX_PARAMETERS;
    total:[664]f64;cleared:[664]f64;m:[664]f64;v:[664]f64;state:AwState;none:[1]f32;loss:=0.0;
    groups:=[1]AwGroup.[AwGroup.{first=0,count=AX_PARAMETERS,lr=AX_RATE,beta1=0.9,beta2=0.99,eps=0.00000001,decay=0}];
    for step:0..<steps {
        loss=0;total=cleared;
        for a:1..<7 {loss+=ax_term(p,ax_sequence(a,a+6),AX_HINTED/6,total[..]);loss+=ax_term(p,ax_sequence(a,AX_NONE),(1-AX_HINTED)/6,total[..]);}
        u:=aw_step(groups[..],AwPolicy.{},state,p,total[..],m[..],v[..],none[0..<0],false);#assert u.status==AwStatus.ok;state=u.state;
    }
    return loss;
}
```

The decoder is chapter 113's with one block, two heads of width 4 with their
own keys and values, an FFN of 8 units and a context of 5: 664 parameters.

The interesting inputs are never trained on. A *conflict* keeps the answer
but swaps the hint for another answer's hint: `BOS 4 9 query 4` has answer 4
and the hint for 3. The rule says 4; the shortcut says 3. Each conflict is
paired with its *clean* input, `BOS 4 10 query 4`, whose hint agrees. There
are 30 conflicts. The 15 with answers 1 to 3 are *development* pairs, used
to choose where to intervene. The 15 with answers 4 to 6 are *held out*,
used only to measure.

## Naming an activation

An intervention needs a name for the thing it changes. A *site* is one of
three activations at one position:

- `resid`: the residual stream entering a layer, or, at `layer == layers`,
  entering the final norm (8 values);
- `head`: one attention head's context vector, before the output projection
  mixes the heads (4 values);
- `unit`: one FFN unit's pre-activation, before the squared ReLU (1 value).

<!-- source: psionic/lib/transformer/patch.coax site -->
```jai
// A named activation: the residual stream entering a layer (or, at layer ==
// layers, entering the final norm), one attention head's context before the
// output projection, or one FFN unit's pre-activation, at one position.
AxPart :: enum u8 {none;resid;head;unit;}
AxSite :: struct {part:AxPart;layer:int;index:int;position:int;}
```

Chapter 113's forward pass already keeps every one of these in its work
buffer, so capture is a read at a known offset. Each position's slot holds,
in order, the input stream (8), its norm (8), two inverse RMS values, the
rotated queries (8), the head contexts (8), the mid stream (8), its norm (8),
the FFN pre-activations (8), and 2 heads times 5 attention weights: 68
values. Work it out for head 1 at position 3: the slot starts at 3 × 68 =
204, the contexts start 16 + 2 + 8 = 26 later at 230, and head 1 starts one
head width after that, at 234. Unit 5 at the same position is at
230 + 8 + 16 + 5 = 259. The stream entering the final norm lives after the
layer's 5 × 84 values, 33 per position, so at position 3 it is at
420 + 99 = 519. A check asserts these four offsets.

<!-- source: psionic/lib/transformer/patch.coax offset -->
```jai
// Where a site's values live in the decoder's work buffer, and how many.
ax_width :: (s:TdShape,site:AxSite)->int {
    if site.part==AxPart.resid {return s.width;}if site.part==AxPart.head {return s.head_width;}if site.part==AxPart.unit {return 1;}return 0;
}
ax_offset :: (s:TdShape,site:AxSite)->int {
    if site.part==AxPart.resid {if site.layer==s.layers {return td_final(s,site.position);}return td_slot(s,site.layer,site.position);}
    o:=td_slot(s,site.layer,site.position);cs:=o+2*s.width+2+s.heads*s.head_width;
    if site.part==AxPart.head {return cs+site.index*s.head_width;}
    return cs+s.heads*s.head_width+2*s.width+site.index;
}
ax_valid :: (s:TdShape,site:AxSite,n:int)->bool {
    if site.position<0 || site.position>=n || site.layer<0 {return false;}
    if site.part==AxPart.resid {return site.layer<=s.layers && site.index==0;}
    if site.layer>=s.layers || site.index<0 {return false;}
    if site.part==AxPart.head {return site.index<s.heads;}
    return site.part==AxPart.unit && site.index<s.hidden;
}
```

<!-- source: psionic/lib/transformer/patch.coax capture -->
```jai
// Copies a site's values out of a work buffer a forward pass has filled.
ax_capture :: (s:TdShape,w:[]f64,site:AxSite,out:[]f64) {o:=ax_offset(s,site);for i:0..<ax_width(s,site) {out[i]=w[o+i];}}
```

## Replacing an activation

Replacement has to happen inside the forward pass: a head's context at
position 3 feeds the rest of position 3's computation. So this chapter
writes the per-position computation again, apart from chapter 113's
`td_advance`, in the same work-buffer layout. It has one hook, `ax_put`,
called with the offset of the values just computed. When that offset is the
patched site's, the values are overwritten before anything reads them.

Two steps are written differently from chapter 113, so that the two forward
passes check each other. Attention weights are `exp(score)` over their sum,
without first subtracting the largest score. The squared ReLU is `r·r` with
`r = (z + |z|)/2`, which is `z` when `z > 0` and 0 otherwise.

<!-- source: psionic/lib/transformer/patch.coax advance -->
```jai
// Chapter 113's per-position computation, written apart from td_advance, in
// the same work-buffer layout. One hook: when `here`, the offset of values
// just computed, is the patched site's offset, they are overwritten before
// anything reads them. Two steps are written differently from td_advance:
// attention weights are exp(score) over their sum, without subtracting the
// largest score, and the squared ReLU is r*r with r = (z+|z|)/2.
ax_put :: (w:[]f64,at:int,here:int,values:[]f64) {if at==here {for i:0..<values.count {w[at+i]=values[i];}}}
ax_advance :: (s:TdShape,p:[]f64,w:[]f64,token:int,t:int,at:int,values:[]f64) {
    d:=s.width;e:=s.head_width;q:=s.heads*e;k:=s.kv_heads*e;group:=s.heads/s.kv_heads;scale:=1/sqrt(f64(e));
    first:=td_slot(s,0,t);for i:0..<d {w[first+i]=p[token*d+i];}
    for l:0..<s.layers {
        b:=td_block(s,l);o:=td_slot(s,l,t);n1:=o+d;r:=o+2*d;qs:=r+2;cs:=qs+q;m:=cs+q;n2:=m+d;u:=n2+d;a:=u+s.hidden;
        keys:=td_keys(s,l);values_at:=td_values(s,l);ax_put(w,at,o,values);
        w[r]=td_norm(w,o,p,b.norm1,d,w,n1);
        td_matvec(p,b.wq,q,d,w,n1,w,qs);td_matvec(p,b.wk,k,d,w,n1,w,keys+t*k);td_matvec(p,b.wv,k,d,w,n1,w,values_at+t*k);
        td_rope(w,qs,s.heads,e,f64(t));td_rope(w,keys+t*k,s.kv_heads,e,f64(t));
        for h:0..<s.heads {g:=h/group;row:=a+h*s.context;total:=0.0;
            for j:0..<t+1 {score:=0.0;for x:0..<e {score+=w[qs+h*e+x]*w[keys+j*k+g*e+x];}w[row+j]=exp(score*scale);total+=w[row+j];}
            for j:0..<t+1 {w[row+j]/=total;}
            for x:0..<e {acc:=0.0;for j:0..<t+1 {acc+=w[row+j]*w[values_at+j*k+g*e+x];}w[cs+h*e+x]=acc;}
            ax_put(w,at,cs+h*e,values);
        }
        td_matvec(p,b.wo,d,q,w,cs,w,m);for i:0..<d {w[m+i]+=w[o+i];}
        w[r+1]=td_norm(w,m,p,b.norm2,d,w,n2);td_matvec(p,b.w1,s.hidden,d,w,n2,w,u);
        for f:0..<s.hidden {ax_put(w,at,u+f,values);}
        next:=td_slot(s,l+1,t);if l+1==s.layers {next=td_final(s,t);}
        for i:0..<d {acc:=0.0;for f:0..<s.hidden {z:=(w[u+f]+abs(w[u+f]))/2;acc+=p[b.w2+i*s.hidden+f]*z*z;}w[next+i]=w[m+i]+acc;}
    }
    f:=td_final(s,t);ax_put(w,at,f,values);
    w[f+2*d]=td_norm(w,f,p,td_final_norm(s),d,w,f+d);td_matvec(p,td_unembed(s),s.vocab,d,w,f+d,w,f+2*d+1);
}
```

Two properties are checked. A run with no site agrees with chapter 113's
`td_forward` to within 1e-12 in every logit, on this chapter's model and on
chapter 113's two-layer shape with a shared key-value head. And a patch that
writes back the value already there changes no bit of any output, because
the hook overwrites a number with itself and everything after it is the
same arithmetic in the same order. That is what makes "restore" exact: put
the original activation back, and the original output comes back exactly,
not approximately.

The forward pass checks the site and the values before it writes anything.

<!-- source: psionic/lib/transformer/patch.coax forward -->
```jai
// The forward pass with one site replaced by `values`. A site of part none
// patches nothing and takes no values. Every refusal comes before anything
// is written. The unshifted attention needs every score below about 709,
// where exp overflows; the scores of this chapter's models are far smaller.
ax_forward :: (s:TdShape,p:[]f64,tokens:[]int,w:[]f64,logits:[]f64,site:AxSite,values:[]f64)->AxRun {
    if td_admit(s,p,w)!=TdStatus.ok || td_tokens(s,tokens)!=TdStatus.ok || logits.count<tokens.count*s.vocab {return AxRun.{status=AxStatus.model};}
    if site.part!=AxPart.none && !ax_valid(s,site,tokens.count) {return AxRun.{status=AxStatus.site};}
    if values.count!=ax_width(s,site) {return AxRun.{status=AxStatus.values};}
    for x:values {if !(x>=-TD_NUMBER && x<=TD_NUMBER) {return AxRun.{status=AxStatus.values};}}
    at:=ax_offset(s,site);
    for t:0..<tokens.count {ax_advance(s,p,w,tokens[t],t,at,values);}
    for t:0..<tokens.count {z:=td_final(s,t)+2*s.width+1;for v:0..<s.vocab {logits[t*s.vocab+v]=w[z+v];}}
    return AxRun.{loss=td_loss(s,w,tokens)};
}
```

## Measuring the shortcut's effect by hand

For a pair with answer `a` and conflicting hint for `b`, the *gap* is logit
`b` minus logit `a` at position 3, where the answer is predicted. The
hint's *effect* is how far the conflict raises the gap above the clean
input's gap. A patch that removes the shortcut should bring the gap back
down.

Take the held-out pair with answer 4 and the hint for 3. The run below prints
these logits:

```text
clean:     logit 4 = 11.730282, logit 3 = 2.476594
corrupted: logit 4 =  4.438364, logit 3 = 9.093758
patched:   logit 4 = 11.682883, logit 3 = 3.091788
```

The clean gap is 2.476594 − 11.730282 = −9.253688. The corrupted gap is
9.093758 − 4.438364 = 4.655394, so the conflicting hint has an effect of
4.655394 − (−9.253688) = 13.909082 nats, and on this input it wins: the
model answers 3. Now replace head 1's context in the corrupted run with the
4 values it had in the clean run. The patched gap is
3.091788 − 11.682883 = −8.591095, an effect of −8.591095 + 9.253688 =
0.662593. The patch removed (13.909082 − 0.662593) / 13.909082 = 0.9524 of
the effect, and the model answers 4 again.

That is one pair. The measurement averages the effect over a set of pairs,
in one of three modes: plain, patched from the clean run, or with the site
set to zero in both runs (an ablation).

<!-- source: psionic/lib/transformer/patch.coax measure -->
```jai
// A pair is a clean input, whose hint agrees with the answer a, and a
// corrupted one, whose hint names b instead. The gap is logit b minus logit
// a where the answer is predicted; the hint's effect is how far the
// corrupted gap moves above the clean one. Development pairs have answers
// 1..3, held-out pairs 4..6; no conflict is ever trained on.
AxMode :: enum u8 {plain;patched;ablated;}
AxEffect :: struct {effect:f64;follows:int;rule:int;pairs:int;}
ax_gap :: (logits:[]f64,a:int,b:int)->f64 {return logits[AX_ANSWER*16+b]-logits[AX_ANSWER*16+a];}
ax_answer :: (logits:[]f64)->int {return td_argmax(logits[AX_ANSWER*16..<AX_ANSWER*16+16]);}
// One run of AX_SHAPE on a five-token sequence; the trained model always admits.
ax_run :: (p:[]f64,x:[5]int,site:AxSite,values:[]f64,w:[]f64,logits:[]f64) {
    t:=x;r:=ax_forward(AX_SHAPE,p,t[..],w,logits,site,values);#assert r.status==AxStatus.ok;
}
// plain: no patch. patched: the corrupted run takes the site's values from
// the clean run. ablated: the site is zero in both runs. Over the pairs whose
// answer is first..last.
ax_measure :: (p:[]f64,site:AxSite,mode:AxMode,first:int,last:int)->AxEffect {
    w:[640]f64;lg:[80]f64;clean:[8]f64;zero:[8]f64;none:AxSite;n:=ax_width(AX_SHAPE,site);out:AxEffect;
    for a:first..<last+1 {for b:1..<7 {if b!=a {
        c:=ax_sequence(a,a+6);k:=ax_sequence(a,b+6);
        if mode==AxMode.ablated {ax_run(p,c,site,zero[0..<n],w[..],lg[..]);}else {ax_run(p,c,none,zero[0..<0],w[..],lg[..]);}
        gc:=ax_gap(lg[..],a,b);ax_capture(AX_SHAPE,w[..],site,clean[..]);
        if mode==AxMode.plain {ax_run(p,k,none,zero[0..<0],w[..],lg[..]);}
        else if mode==AxMode.patched {ax_run(p,k,site,clean[0..<n],w[..],lg[..]);}
        else {ax_run(p,k,site,zero[0..<n],w[..],lg[..]);}
        out.effect+=ax_gap(lg[..],a,b)-gc;out.pairs+=1;
        if ax_answer(lg[..])==b {out.follows+=1;}if ax_answer(lg[..])==a {out.rule+=1;}
    }}}
    out.effect/=f64(out.pairs);return out;
}
// The share of the hint's effect that patching the site from the clean run removes.
ax_recovered :: (p:[]f64,site:AxSite,first:int,last:int)->f64 {
    e:=ax_measure(p,site,AxMode.plain,first,last).effect;return (e-ax_measure(p,site,AxMode.patched,first,last).effect)/e;
}
```

## Choosing a site on development pairs

The candidates are the two heads and the eight FFN units at position 3.
Each is scored on the development pairs by the share of the summed effect
that patching it from the clean run removes. The highest score is the
chosen site. The score nearest zero names the *unrelated* control: a site
the development pairs say does not carry the shortcut.

<!-- source: psionic/lib/transformer/patch.coax select -->
```jai
// The candidates: each head and each FFN unit where the answer is predicted.
AX_SITES :: 10;
ax_candidate :: (i:int)->AxSite {
    if i<2 {return AxSite.{part=AxPart.head,layer=0,index=i,position=AX_ANSWER};}
    return AxSite.{part=AxPart.unit,layer=0,index=i-2,position=AX_ANSWER};
}
// ax_recovered for every candidate at once: per pair, one clean run that
// every candidate's values are captured from, one plain corrupted run, and
// one patched run per candidate.
ax_scores :: (p:[]f64,first:int,last:int)->[10]f64 {
    w:[640]f64;lg:[80]f64;clean:[10][8]f64;empty:[1]f64;none:AxSite;plain:=0.0;moved:[10]f64;
    for a:first..<last+1 {for b:1..<7 {if b!=a {
        ax_run(p,ax_sequence(a,a+6),none,empty[0..<0],w[..],lg[..]);gc:=ax_gap(lg[..],a,b);
        for i:0..<AX_SITES {ax_capture(AX_SHAPE,w[..],ax_candidate(i),clean[i][..]);}
        k:=ax_sequence(a,b+6);ax_run(p,k,none,empty[0..<0],w[..],lg[..]);gk:=ax_gap(lg[..],a,b);plain+=gk-gc;
        for i:0..<AX_SITES {site:=ax_candidate(i);ax_run(p,k,site,clean[i][0..<ax_width(AX_SHAPE,site)],w[..],lg[..]);moved[i]+=gk-ax_gap(lg[..],a,b);}
    }}}
    for i:0..<AX_SITES {moved[i]/=plain;}return moved;
}
// The site whose patch removes the most of the hint's effect, and the one
// whose patch moves it least (the unrelated control). Ties go to the first.
AxChoice :: struct {best:int;least:int;}
ax_pick :: (scores:[10]f64)->AxChoice {
    c:AxChoice;
    for i:1..<AX_SITES {if scores[i]>scores[c.best] {c.best=i;}if abs(scores[i])<abs(scores[c.least]) {c.least=i;}}
    return c;
}
```

## Controls first, then the verdict

The controls run before any effect is reported:

- **No-op patches.** On all 42 training and conflict sequences, every
  candidate site at every position is patched with its own value. No bit of
  any logit may change.
- **Unrelated patch.** The unrelated site's patch, on held-out pairs, may
  move the effect by at most a tenth of it.
- **Remove, then restore.** On each held-out conflict, zero the chosen site,
  then write back the value captured from the plain run. The zeroing must
  change the output, and the restored output must equal the plain one in
  every bit.

<!-- source: psionic/lib/transformer/patch.coax controls -->
```jai
// Logits compared, and how many of them differ in any bit.
AxCompare :: struct {logits:int;differ:int;}
ax_compare :: (a:[]f64,b:[]f64,into:AxCompare)->AxCompare {
    c:=into;for i:0..<a.count {c.logits+=1;if f64_bits(a[i])!=f64_bits(b[i]) {c.differ+=1;}}return c;
}
// No-op patches: on each sequence, every candidate site at every position
// gets back the values its own plain run computed.
ax_noop :: (p:[]f64,sequences:[][5]int)->AxCompare {
    w:[640]f64;lg:[80]f64;base:[80]f64;own:[50][8]f64;empty:[1]f64;none:AxSite;out:AxCompare;
    for x:sequences {ax_run(p,x,none,empty[0..<0],w[..],base[..]);
        for i:0..<50 {site:=ax_candidate(i/5);site.position=i%5;ax_capture(AX_SHAPE,w[..],site,own[i][..]);}
        for i:0..<50 {site:=ax_candidate(i/5);site.position=i%5;ax_run(p,x,site,own[i][0..<ax_width(AX_SHAPE,site)],w[..],lg[..]);out=ax_compare(lg[..],base[..],out);}}
    return out;
}
// Remove, then restore: on each conflict with answer first..last, zero the
// site, then write back the values captured from the plain run. Returns how
// many inputs the zeroing changed, and the restored logits compared with
// the plain ones.
AxRestore :: struct {changed:int;restored:AxCompare;}
ax_restore :: (p:[]f64,site:AxSite,first:int,last:int)->AxRestore {
    w:[640]f64;base:[80]f64;lg:[80]f64;v:[8]f64;zero:[8]f64;none:AxSite;n:=ax_width(AX_SHAPE,site);out:AxRestore;
    for a:first..<last+1 {for b:1..<7 {if b!=a {x:=ax_sequence(a,b+6);
        ax_run(p,x,none,v[0..<0],w[..],base[..]);ax_capture(AX_SHAPE,w[..],site,v[..]);
        ax_run(p,x,site,zero[0..<n],w[..],lg[..]);if !ax_same(lg[..],base[..]) {out.changed+=1;}
        ax_run(p,x,site,v[0..<n],w[..],lg[..]);out.restored=ax_compare(lg[..],base[..],out.restored);
    }}}
    return out;
}
// Inputs without a conflict (each answer first..last with its hint and with
// none) that get the rule's answer, with the site plain or zeroed.
ax_retained :: (p:[]f64,site:AxSite,ablate:bool,first:int,last:int)->int {
    w:[640]f64;lg:[80]f64;zero:[8]f64;none:AxSite;n:=ax_width(AX_SHAPE,site);right:=0;
    for a:first..<last+1 {for h:[2]int.[a+6,AX_NONE] {x:=ax_sequence(a,h);
        if ablate {ax_run(p,x,site,zero[0..<n],w[..],lg[..]);}else {ax_run(p,x,none,zero[0..<0],w[..],lg[..]);}
        if ax_answer(lg[..])==a {right+=1;}}}
    return right;
}
```

The thresholds were frozen before the first run. The chosen site isolates
the shortcut if zeroing it removes at least half of the held-out effect
while every held-out input without a conflict (each answer 4 to 6, with its
hint and with none) still gets the rule's answer. If any control fails, the
verdict is no.

<!-- source: psionic/lib/transformer/patch.coax verdict -->
```jai
// Frozen before the first run. No-op and restoring patches must leave every
// logit's bits unchanged; the unrelated site's patch may move the held-out
// effect by at most AX_UNRELATED of it. The chosen site isolates the
// shortcut if zeroing it removes at least AX_ISOLATE of the held-out effect
// while all six held-out inputs without a conflict keep the rule's answer.
AX_UNRELATED :: 0.1;
AX_ISOLATE :: 0.5;
AxVerdict :: struct {controls:bool;isolated:bool;}
ax_verdict :: (noop:AxCompare,restored:AxCompare,unrelated:f64,plain:f64,ablated:f64,kept:int)->AxVerdict {
    controls:=noop.differ==0 && restored.differ==0 && abs(unrelated)<=AX_UNRELATED;
    return AxVerdict.{controls=controls,isolated=controls && ablated<=(1-AX_ISOLATE)*plain && kept==6};
}
```

The attention picture, for comparison, is each head's mean attention weight
at position 3, read from the work buffer.

<!-- source: psionic/lib/transformer/patch.coax picture -->
```jai
// The attention picture: head h's mean weight, where the answer is
// predicted, on position j, over the held-out conflicts.
ax_attention :: (p:[]f64,h:int,j:int)->f64 {
    s:=AX_SHAPE;w:[640]f64;lg:[80]f64;v:[1]f64;none:AxSite;total:=0.0;
    // The weights follow the FFN pre-activations: one row of `context` per head.
    row:=ax_offset(s,AxSite.{part=AxPart.unit,index=s.hidden,position=AX_ANSWER})+h*s.context;
    for a:4..<7 {for b:1..<7 {if b!=a {ax_run(p,ax_sequence(a,b+6),none,v[0..<0],w[..],lg[..]);total+=w[row+j];}}}
    return total/15;
}
```

## The run

[The example](../examples/patch-shortcut.coax) trains the model, checks
that its weights equal the recorded
[fixture](../fixtures/patch-shortcut/weights.coax) bit for bit, prints the
picture and the pair worked above, runs the controls, chooses a site,
measures it on held-out pairs, and ends with two deliberately broken
patches: a site at position 5 of a 5-token input, and three values for a
head that has four.

<!-- source: psionic/examples/patch-shortcut.coax example -->
```jai
// Train the decoder on the hint task, look at the attention picture, then
// test it by intervention: controls first, a site chosen on development
// pairs, its effect measured on held-out pairs, and two refused patches.
AX_NAMES :: [10]string.["head 0","head 1","unit 0","unit 1","unit 2","unit 3","unit 4","unit 5","unit 6","unit 7"];
main :: () {
    s:=AX_SHAPE;p:[664]f64;#assert td_parameter_count(s)==AX_PARAMETERS;
    loss:=ax_train(p[..],AX_STEPS);
    print("decoder: % parameters; % full-batch AdamW steps on 12 sequences; last loss %\n",AX_PARAMETERS,AX_STEPS,loss);
    recorded:=ax_trained();print("weights equal the recorded fixture, bit for bit: %\n",ax_same(p[..],recorded[..]));
    for h:0..<2 {print("picture: head % puts % of its attention on the answer, % on the hint\n",h,ax_attention(p[..],h,1),ax_attention(p[..],h,2));}
    // One held-out pair by hand: answer 4, hint for 3.
    w:[640]f64;lg:[80]f64;v:[8]f64;none:AxSite;head:=ax_candidate(1);
    ax_run(p[..],ax_sequence(4,10),none,v[0..<0],w[..],lg[..]);print("pair 4/3 clean:     logit 4 %, logit 3 %\n",lg[52],lg[51]);ax_capture(s,w[..],head,v[..]);
    ax_run(p[..],ax_sequence(4,9),none,v[0..<0],w[..],lg[..]);print("pair 4/3 corrupted: logit 4 %, logit 3 %\n",lg[52],lg[51]);
    ax_run(p[..],ax_sequence(4,9),head,v[0..<4],w[..],lg[..]);print("pair 4/3 patched:   logit 4 %, logit 3 %\n",lg[52],lg[51]);
    // Controls first: no-op patches at every candidate site and position.
    all:=ax_all();noop:=ax_noop(p[..],all[..]);print("control, no-op patches: % of % logits differ\n",noop.differ,noop.logits);
    // Sites are chosen on development pairs (answers 1..3) only.
    scores:=ax_scores(p[..],1,3);c:=ax_pick(scores);site:=ax_candidate(c.best);
    for i:0..<AX_SITES {print("development: patching % from the clean run removes % of the effect\n",AX_NAMES[i],scores[i]);}
    print("chosen: %; unrelated control: %\n",AX_NAMES[c.best],AX_NAMES[c.least]);
    unrelated:=ax_recovered(p[..],ax_candidate(c.least),4,6);
    print("control, unrelated patch on held-out pairs removes % of the effect (bound %)\n",unrelated,AX_UNRELATED);
    restore:=ax_restore(p[..],site,4,6);
    print("control, restore: zeroing changed % of 15 inputs; % of % restored logits differ\n",restore.changed,restore.restored.differ,restore.restored.logits);
    modes:=[3]AxMode.[AxMode.plain,AxMode.patched,AxMode.ablated];labels:=[3]string.["plain","patched from clean","site zeroed"];effects:[3]AxEffect;
    for i:0..<3 {e:=ax_measure(p[..],site,modes[i],4,6);effects[i]=e;print("held-out, %: effect % nats; follows the hint % of %, answers by the rule %\n",labels[i],e.effect,e.follows,e.pairs,e.rule);}
    retained:=ax_retained(p[..],site,false,4,6);kept:=ax_retained(p[..],site,true,4,6);
    print("retained: % of 6 held-out inputs without a conflict plain, % of 6 with the site zeroed\n",retained,kept);
    v2:=ax_verdict(noop,restore.restored,unrelated,effects[0].effect,effects[2].effect,kept);
    print("controls pass: %; % isolates the shortcut: %\n",v2.controls,AX_NAMES[c.best],v2.isolated);
    // Deliberately broken patches are refused before anything runs.
    x:=ax_sequence(4,9);far:=head;far.position=5;
    print("refused: % at position 5 of 5 tokens: %\n",AX_NAMES[1],ax_forward(s,p[..],x[..],w[..],lg[..],far,v[0..<4]).status);
    print("refused: 3 values for a 4-value head: %\n",ax_forward(s,p[..],x[..],w[..],lg[..],head,v[0..<3]).status);
    #assert v2.isolated;
}
```

```shell
coax/bin/coax run psionic/examples/patch-shortcut.coax
```

```text
decoder: 664 parameters; 300 full-batch AdamW steps on 12 sequences; last loss 0.5909210872453994
weights equal the recorded fixture, bit for bit: true
picture: head 0 puts 0.5967093514904882 of its attention on the answer, 0.03267574302689099 on the hint
picture: head 1 puts 0.16001057111927858 of its attention on the answer, 0.2994069476444334 on the hint
pair 4/3 clean:     logit 4 11.730281742400319, logit 3 2.476593716276513
pair 4/3 corrupted: logit 4 4.43836355002113, logit 3 9.09375766993636
pair 4/3 patched:   logit 4 11.682882638710712, logit 3 3.0917884303387426
control, no-op patches: 0 of 168000 logits differ
development: patching head 0 from the clean run removes 0.19405291149120457 of the effect
development: patching head 1 from the clean run removes 0.8017223257374475 of the effect
development: patching unit 0 from the clean run removes 0.14257606227744657 of the effect
development: patching unit 1 from the clean run removes 0.1018263986345193 of the effect
development: patching unit 2 from the clean run removes 0.013604022915406569 of the effect
development: patching unit 3 from the clean run removes 0.07300148674270075 of the effect
development: patching unit 4 from the clean run removes 0.029119843633768783 of the effect
development: patching unit 5 from the clean run removes 0.20579126656634092 of the effect
development: patching unit 6 from the clean run removes 0.006108556838044004 of the effect
development: patching unit 7 from the clean run removes 0.07717084298832813 of the effect
chosen: head 1; unrelated control: unit 6
control, unrelated patch on held-out pairs removes -0.06576079922616156 of the effect (bound 0.1)
control, restore: zeroing changed 15 of 15 inputs; 0 of 1200 restored logits differ
held-out, plain: effect 2.8647496159351484 nats; follows the hint 2 of 15, answers by the rule 12
held-out, patched from clean: effect 0.4591226978807987 nats; follows the hint 0 of 15, answers by the rule 15
held-out, site zeroed: effect 0.5349883248474029 nats; follows the hint 1 of 15, answers by the rule 14
retained: 6 of 6 held-out inputs without a conflict plain, 6 of 6 with the site zeroed
controls pass: true; head 1 isolates the shortcut: true
refused: head 1 at position 5 of 5 tokens: site
refused: 3 values for a 4-value head: values
```

## What the run shows

The model learned the rule. It answers all six held-out inputs without a
conflict correctly, and on 12 of 15 held-out conflicts it still answers by
the rule. But the hint pulls: on average a conflicting hint raises the gap
by 2.86 nats, and on 2 of the 15 it wins. The effect is uneven. The hint
for 3 is strong (the pair worked above, 13.9 nats), while some other hints
barely move the answer.

The picture says head 0 looks at the answer symbol (0.60 of its attention)
and almost never at the hint (0.03). Head 1 splits its attention: 0.30 on
the hint, 0.16 on the answer, the rest on the boundary and the query mark.
From the picture alone you could guess that head 1 carries the hint, but you
could not say how much of the shortcut goes through it, or whether the FFN
also carries it.

The interventions answer that. On development pairs, patching head 1 removes
0.80 of the effect; the next best site, unit 5, removes 0.21. On held-out
pairs, chosen without them:

- patching head 1 from the clean run leaves 0.46 of 2.86 nats (84% removed),
  and the model follows the hint on none of the 15;
- zeroing head 1 leaves 0.53 nats (81% removed), above the frozen half, and
  all six held-out inputs without a conflict keep the rule's answer;
- restoring head 1's value after zeroing it gives back the original logits
  in every bit, on all 15 conflicts that the zeroing had changed.

The controls passed: 168,000 no-op logits unchanged, and unit 6's patch moved
the held-out effect by −0.066, inside the 0.1 bound. So the verdict is that
head 1 isolates most of the shortcut in this model. Not all of it: after
zeroing, one conflict still follows the hint, and 0.53 nats of effect
remain, spread over the other sites.

The broken patches are refused with `site` and `values` before the forward
pass writes anything.

## Checks

The checks in [patch.coax](../lib/transformer/patch.coax) finish in
seconds. They load the trained weights from the
[fixture](../fixtures/patch-shortcut/weights.coax) instead of training,
and measure small slices of the experiment (the five pairs of one answer);
the example runs the whole experiment and pins its transcript.

- **Independent reference.** A run with no site agrees with chapter 113's
  `td_forward` within 1e-12 in every logit and the loss: on the trained and
  the initialized model, and on chapter 113's two-layer tiny shape whose
  two query heads share one key-value head.
- **The worked pair.** The gaps −9.253688, 4.655394 and −8.591095 and the
  share 0.9524 above, from the trained weights.
- **Hand values.** The four offsets worked above. Captured values satisfy
  the decoder's equations recomputed in the check: the input stream is the
  embedding row; the mid stream is the output projection of both captured
  heads plus the input; the units are the FFN's first matrix times its
  norm; the final stream adds `w2[i,f]·z²` for each positive unit; and the
  logits follow from it. Patching a positive unit to 2 moves the final
  stream by `w2[i,f]·(4 − z²)`, and to 0 by `−w2[i,f]·z²`, within 1e-12.
- **Causality.** In this one-layer model a head patch at position 2 changes
  only position 2's logits, and so does a second-layer head patch in the
  two-layer shape; a patch of the stream entering the layer at
  position 2 changes positions 2 to 4; a patch of the final stream at
  position 4 changes only position 4. A negative unit value gives the same
  bits as zero, as squared ReLU requires.
- **Two routes.** Each measurement mode equals the same runs written out in
  the check. The all-candidate development scores equal each site's own
  recovery computed separately. The rule counts with head 0 or head 1
  zeroed equal a recount, and zeroing head 0, which reads the answer,
  loses held-out answers where zeroing head 1 does not. Each head's
  attention weights at the answer sum to one.
- **Controls.** No-op patches change no bit. Zeroing a unit changes exactly
  the inputs where its pre-activation is positive, and restoring changes no
  bit.
- **Training.** The first step's loss is the weighted mean of the twelve
  sequences' losses at initialization, computed separately; two three-step
  runs give the same bits, and their loss and two parameters match pinned
  bits. The example re-trains all 300 steps and matches the fixture.
- **Refusals.** Sites at a position past the input or below zero, a missing
  layer, a head or unit index out of range, and a stream site with an index;
  values of the wrong count (a site of part none takes none), non-finite or
  beyond ±2^20; and a bad token,
  short buffers or a non-finite parameter. Each leaves the buffers
  untouched, and the limits themselves are admitted.
- **Boundaries.** The verdict at exactly the unrelated bound and exactly
  half the effect, with a failed control, and with five of six inputs kept.
  Ties in site selection go to the first site.

`coax mutate --root-only` on the library leaves no surviving mutant.

## Limits

- One task, one seed, one trained model. The run shows that this model
  routes most of this shortcut through head 1; it does not show how often
  that happens across seeds or tasks.
- Zeroing is one kind of ablation. Zero is a value head 1 never produces on
  real inputs, so a zeroed run is partly out of distribution; patching from
  the clean run, which uses real values, is reported beside it.
- Sites are whole heads and single units at the answer's position. Paths
  (for example, head 1 only through the FFN) and positions other than the
  answer's are not searched.
- The patched forward pass subtracts nothing before `exp`, so it needs
  every attention score below about 709. The scores here are far smaller,
  but chapter 113 admits weights for which they would not be; use its
  `td_forward` for those.
- Capture reads chapter 113's work buffer directly. The general inspection
  tools of appendix D in the [chapter plan](PLAN.md), such as a logit lens,
  are not built yet; this chapter needs only capture and replacement.
