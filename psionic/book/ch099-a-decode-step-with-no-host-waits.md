# 99. A decode step with no host waits

> **Status: partly implemented.** On the gpt-oss-20b CUDA engine, one
> recorded program is the whole decode step: the input token and position
> come from the device's control words, routes stay on the device, and the
> next token is drawn on the device by chapter 53's rules. The host launches
> it once per token and waits on the device zero times inside a step. Greedy
> and seeded outputs on the two fixture prompts equal the host paths token
> for token on one RTX 4080, where sampling on the device is 1.70 times
> faster than on the host and greedy decoding is not faster. The DeepSeek
> V4.1 step this chapter was planned for (chapter 96's expert cache and
> chapter 98's packed cache on the device), a host that schedules step N + 1
> with a speculative length learned one step late, cancellation inside a
> batch of launched steps, grammar masks and top-k above 64 on the device
> are not built; the last
> section lists them.

A decode step launches hundreds of small device operations. On gpt-oss-20b
there are 591: norms, products, routing, the expert products, attention and
the sampler. [Chapter 70](ch070-replay-the-device-program.md) already
records them as one CUDA graph and replays it, but it still does three
things on the host every token: it patches the arguments of about 120 launches
that hold the position, it waits for the device, and it reads the control
words to learn the token and whether to stop. A host-sampled step adds a
second wait and an 800 KB download of the logits.

This chapter removes all of that from the step. The program reads the
position and the input token from the control words that the previous step
left on the device, so the recorded graph is the same for every step. The
sampler runs on the device and writes the token where the next step reads
it. The host launches the graph sixteen times in a row and reads the control
words once.

The code is in [`device-sample.coax`](../lib/gptoss/device-sample.coax) (the
kernels) and [`device-step.coax`](../lib/gptoss/device-step.coax) (admission
and the program). Both are `#authority` files: the first decides which token
is emitted, the second which options are admitted and what the recorded
program does. The harness is [`gptoss_step.c`](../tests/gptoss_step.c), run
by [`check_device_step.sh`](../tests/check_device_step.sh).

## One draw, by hand

The device sampler has to give chapter 53's answer, so take its worked
example: logits 2, 1, 2, 0 and −1, temperature 1, top-k 5, top-p 1, seed
45001. Chapter 53 sorts the whole vocabulary. A GPU can't afford a stable
sort of 201,088 logits per token in one thread, and top-k only needs the
first k in the order. So the device keeps short lists.

**Lists.** Split the logits into parts of two (the example's size; the real
part is 256). Each part keeps its IDs in chapter 53's order, higher logit
first and equal logits to the lower ID:

```text
list 0: logits 2, 1 at IDs 0, 1   ->  0 1
list 1: logits 2, 0 at IDs 2, 3   ->  2 3
list 2: logit -1 at ID 4          ->  4
```

**Merge.** One item merges up to 32 lists by taking the best head each time.
Heads 0 (logit 2) and 2 (logit 2) tie; the lower ID goes first. Then 2, then
1 (logit 1) beats 3 (logit 0), then 3, then 4. The merged list 3 is
`0 2 1 3 4`, chapter 53's order. Because the comparison is a total order
(logit, then ID), the first k of any merge equal the first k of the full
stable sort, whatever the part size.

**Weights and top-p.** As in chapter 53: subtract the top logit, divide by
the temperature, exponentiate, in list order. The weights are 1, 1, e⁻¹,
e⁻² and e⁻³, summing to 2.55301; at top-p 1 all five are kept and each is
divided by the sum: 0.39170, 0.39170, 0.14410, 0.05301 and 0.01950.

**The draw.** Chapter 53 walks the probabilities in ID order, not list
order. The device ranks each kept ID among the kept IDs (ID 0 has rank 0, ID
1 rank 1, ...) and walks them by rank. Counter 0 gives the uniform 0.39237.
After ID 0 the running mass is 0.39170, not above it; after ID 1 it is
0.53580, so the token is 1. The counter is the number of tokens emitted, so
the next step uses counter 1 (0.29347, token 0) and then counter 2 (0.02290,
token 0). The third emission reaches the limit of 3 and sets stop code 3.
The example below prints this trace from the kernels themselves.

## The order on the device

<!-- source: psionic/lib/gptoss/device-sample.coax order -->
```jai
// Chapter 53's order: higher logit first; equal logits go to the lower ID.
gs_before :: (logits:[]f32,a:int,b:int)->bool {return logits[a]>logits[b] || (logits[a]==logits[b] && a<b);}
// Insert an ID into a list kept in that order, at most list.count long.
// Returns the new length. A full list drops its last entry when the new ID
// goes before it.
gs_insert :: (logits:[]f32,id:int,list:[]int,count:int)->int {
    k:=list.count;
    if count==k && !gs_before(logits,id,list[k-1]) {return count;}
    at:=count;if count==k {at=k-1;}
    start:=at;for step:0..<start {if !gs_before(logits,id,list[at-1]) {break;}list[at]=list[at-1];at-=1;}
    list[at]=id;if count<k {return count+1;}return count;
}
// Lists live in one buffer: list L holds ids[L*k ..< L*k+k] and counts[L].
// One item keeps the first k IDs of its `part` logits in order and flags a
// logit chapter 53 would refuse.
gs_top_part :: (item:int,part:int,k:int,logits:[]f32,ids:[]int,counts:[]int,flags:[]u8) #export_c {
    count:=0;bad:u8=0;first:=item*part;
    for j:0..<part {i:=first+j;if i<logits.count {if (f32_bits(logits[i])&0x7fffffff)>GS_LOGIT_LAST {bad=1;}count=gs_insert(logits,i,ids[item*k..<item*k+k],count);}}
    counts[item]=count;flags[item]=bad;
}
gs_min :: (a:int,b:int)->int {return (a+b-cast(int)abs(f64(a-b)))/2;}
// One item merges up to GS_FAN of the `lists` lists that start at list
// `from` into the first k of their union, written as list `to` + item.
gs_top_merge :: (item:int,from:int,lists:int,to:int,k:int,logits:[]f32,ids:[]int,counts:[]int,flags:[]u8) #export_c {
    heads:[GS_FAN]int;base:=from+item*GS_FAN;n:=gs_min(lists-item*GS_FAN,GS_FAN);bad:u8=0;
    for j:0..<n {bad|=flags[base+j];}
    count:=0;
    for slot:0..<k {
        best:=-1;
        for j:0..<n {if heads[j]<counts[base+j] {if best<0 || gs_before(logits,ids[(base+j)*k+heads[j]],ids[(base+best)*k+heads[best]]) {best=j;}}}
        if best<0 {break;}
        ids[(to+item)*k+slot]=ids[(base+best)*k+heads[best]];heads[best]+=1;count+=1;
    }
    counts[to+item]=count;flags[to+item]=bad;
}
```

`gs_top_part` runs one item per part. `gs_insert` keeps the list at most
`list.count` long: a full list drops its last entry when a better ID
arrives. `gs_top_merge` reads up to 32 lists and writes their first k as a
new list. All lists live in one buffer, list L at `ids[L*k ..]`, so a merge
names its inputs and output by list number. For the 20b vocabulary, 786
parts merge to 25 lists and then to one.

The part kernel also flags a logit whose magnitude is above the largest f32
within 10³⁰ (bit pattern `0x7149f2c9`), which also catches NaN and the
infinities. Chapter 53 refuses the same logits.

## The pick

<!-- source: psionic/lib/gptoss/device-sample.coax pick -->
```jai
// splitmix64-top53-v1 (chapter 64), the same words as cr_bits.
gs_bits :: (seed:u64,counter:u64)->u64 {
    z:=add_wrap(seed,mul_wrap((cast(u64)0x9e3779b97f4a7c15),add_wrap(counter,(cast(u64)1))));
    z=mul_wrap(z^(z>>30),(cast(u64)0xbf58476d1ce4e5b9));z=mul_wrap(z^(z>>27),(cast(u64)0x94d049bb133111eb));return z^(z>>31);
}
// nx_pick on the kept IDs in ascending ID order: the first whose running
// mass exceeds the uniform, else the last with positive mass.
gs_choose :: (uniform:f64,ids:[]int,mass:[]f64,take:int)->int {
    running:=0.0;
    for i:0..<take {running+=mass[i];if uniform<running {return ids[i];}}
    for step:0..<take {i:=take-1-step;if mass[i]>0 {return ids[i];}}
    return -1;
}
// One item: weights, top-p, the draw, then the control words. The final
// list is list `from`. The draw's counter is the emitted count, control[3].
gs_pick :: (item:int,from:int,k:int,temperature_bits:int,top_p_bits:int,seed:int,limit:int,logits:[]f32,ids:[]int,counts:[]int,flags:[]u8,weights:[]f64,sorted:[]int,by_id:[]f64,control:[]int,emitted_ids:[]int) #export_c {
    // One item decides; any other item of the launch does nothing.
    if item!=0 || control[0]!=0 {return;}
    if flags[from]!=0 {control[0]=GS_NUMBER;return;}
    temperature:=f64_from_bits(cast(u64)temperature_bits);top_p:=f64_from_bits(cast(u64)top_p_bits);
    uniform:=f64(gs_bits(cast(u64)seed,cast(u64)control[3])>>11)/9007199254740992.0;
    first:=from*k;token:=ids[first];
    if temperature!=0 {
        kept:=counts[from];top:=f64(logits[token]);total:=0.0;
        for i:0..<kept {v:=exp((f64(logits[ids[first+i]])-top)/temperature);weights[i]=v;total+=v;}
        mass:=0.0;take:=0;
        for i:0..<kept {if !(mass<top_p*total) {break;}mass+=weights[i];take+=1;}
        // Rank each kept ID among the kept IDs to walk them in ID order.
        for i:0..<take {rank:=0;for j:0..<take {if ids[first+j]<ids[first+i] {rank+=1;}}weights[i]/=mass;sorted[rank]=ids[first+i];by_id[rank]=weights[i];}
        token=gs_choose(uniform,sorted,by_id,take);
    }
    emitted:=control[3]+1;control[1]=token;control[3]=emitted;if emitted-1<emitted_ids.count {emitted_ids[emitted-1]=token;}
    stop:=0;if token==GS_RETURN {stop=1;}else if token==GS_CALL {stop=2;}else if emitted>=limit {stop=3;}control[4]=stop;
}
```

The weights are f64 and summed in list order, exactly as chapter 53 sums
them. Top-p adds the same weights again in the same order, so at top-p 1 the
mass ends equal to the total. Temperature 0 takes the first ID of the list
and still consumes one counter, as chapter 53 does. The stop codes are
chapter 70's: 1 for `<|return|>`, 2 for `<|call|>`, 3 for the limit, with a
stop token winning over the limit.

One difference from the CPU remains possible: CUDA's `exp` on doubles and
the C library's are both within an ulp of the true value but are not the
same function. The harness compares the device's weight bits with
`nx_distribution` on the device's own logits after every run; on the fixture
prompts they are equal (see below), and a difference would be printed and
fail the check.

## Kernels that read the position on the device

<!-- source: psionic/lib/gptoss/device-sample.coax position -->
```jai
// A step after a stop, or after a refused step, keeps the refusal and runs
// nothing that reads the position.
gs_begin :: (item:int,vocab:int,capacity:int,control:[]int) #export_c {
    if control[0]!=0 {return;}
    if control[4]!=0 {control[0]=GS_STOPPED;return;}
    if control[2]<0 || control[2]>=capacity {control[0]=9;}
    if control[1]<0 || control[1]>=vocab {control[0]=7;control[1]=0;}
}
// Sliding layers pass their window; full layers pass 0.
gs_first :: (position:int,window:int)->int {if window>0 && position>=window {return position+1-window;}return 0;}
gs_rotate :: (item:int,heads:int,kv_heads:int,qkv:[]f32,rope:[]f32,pending:[]f32,flags:[]u8,control:[]int) #export_c {
    if control[0]!=0 {return;}gk_rotate(item,heads,kv_heads,control[2],qkv,rope,pending,flags);
}
// Launched over heads × the largest visible span; items past the span return.
gs_scores :: (item:int,heads:int,kv_heads:int,window:int,capacity:int,stride:int,qkv:[]f32,cache:[]f32,pending:[]f32,scores:[]f32,control:[]int) #export_c {
    if control[0]!=0 {return;}position:=control[2];first:=gs_first(position,window);
    if item>=heads*(position-first+1) {return;}gk_scores(item,heads,kv_heads,position,first,capacity,stride,qkv,cache,pending,scores);
}
gs_prepare :: (item:int,window:int,stride:int,sinks:[]u16,scores:[]f32,denominators:[]f64,flags:[]u8,control:[]int) #export_c {
    if control[0]!=0 {return;}position:=control[2];gk_softmax_prepare(item,position-gs_first(position,window)+1,stride,sinks,scores,denominators,flags);
}
gs_divide :: (item:int,heads:int,window:int,stride:int,denominators:[]f64,scores:[]f32,control:[]int) #export_c {
    if control[0]!=0 {return;}position:=control[2];visible:=position-gs_first(position,window)+1;
    if item>=heads*visible {return;}gk_softmax_divide(item,visible,stride,denominators,scores);
}
gs_context :: (item:int,heads:int,kv_heads:int,window:int,capacity:int,stride:int,lanes:int,scores:[]f32,cache:[]f32,pending:[]f32,partial:[]f32,control:[]int) #export_c {
    if control[0]!=0 {return;}position:=control[2];
    gk_context_part(item,heads,kv_heads,position,gs_first(position,window),capacity,stride,lanes,scores,cache,pending,partial);
}
```

Each wrapper calls chapter 70's kernel with the position from `control[2]`.
The score and divide launches cover the largest visible span (128 for a
sliding layer, the context for a full one); items past the current span
return at once. `gs_begin` makes a step that runs after a stop, or after a
refused step, change nothing: it sets status 10 (or keeps the refusal), and
every later launch that reads the position returns, the pick does nothing
and chapter 70's publish commits nothing. That is what lets the host launch
sixteen steps without looking.

## Admission and the program

<!-- source: psionic/lib/gptoss/device-step.coax admit -->
```jai
// Chapter 53's options, with top-k bounded so one item can rank the kept IDs.
// Temperature 0 is greedy and needs a list of one.
gs_options :: (o:NxOptions,seed:u64,vocab:int)->GStatus #export_c {
    if !nx_options(o,vocab) || (o.temperature!=0 && (o.top_k<1 || o.top_k>GS_TOPK)) || seed>>63!=0 {return GStatus.shape;}
    return GStatus.ok;
}
gs_k :: (o:NxOptions)->int {if o.temperature==0 {return 1;}return o.top_k;}
gs_layout :: (p:GDPlan,vocab:int,k:int)->GSLayout #export_c {
    n:=gs_lists(vocab,GS_PART);total:=n;m:=n;
    while m>1 {m=gs_up(m);total+=m;}
    b:=16+GS_SCRATCH;r:=GSLayout.{lists=total,final=total-1};
    r.ids=GDRef.{buffer=b,count=total*k,elem=8};r.counts=GDRef.{buffer=b,offset=8*total*k,count=total,elem=8};
    r.weights=GDRef.{buffer=b,offset=8*(total*k+total),count=k,elem=8};r.sorted=GDRef.{buffer=b,offset=8*(total*k+total+k),count=k,elem=8};
    r.by_id=GDRef.{buffer=b,offset=8*(total*k+total+2*k),count=k,elem=8};r.flags=GDRef.{buffer=b,offset=8*(total*k+total+3*k),count=total,elem=1};
    if !gd_ref_fits(p,r.flags) {r.status=GStatus.capacity;}
    return r;
}
// The control words a sampled request starts from: the last prompt ID at
// its position, nothing emitted. Written once, before the first step.
gs_control :: (s:GShape,token:int,position:int,out:[]int)->GResult #export_c {
    if token<0 || token>=s.vocab || position<0 || position>=s.capacity {return GResult.{status=GStatus.state};}
    if out.count<8 {return GResult.{status=GStatus.capacity};}
    for i:0..<8 {out[i]=0;}out[1]=token;out[2]=position;return GResult.{written=8};
}
```

The device sampler admits top-k from 1 to 64, so one item can rank the kept
IDs. Top-k off at a nonzero temperature is refused (`shape`): it would need
the full sort. Seeds must fit in 63 bits because launch arguments are
signed. A sampled request starts from `gs_control`'s words: the last prompt
ID at its position, nothing emitted.

<!-- source: psionic/lib/gptoss/device-step.coax program -->
```jai
// gd_program's step with every position-dependent launch replaced: the same
// kernels run through gs_* wrappers that read control[2], launched over the
// largest span, and gk_argmax_part/gk_choose replaced by the sampler.
gs_program :: (s:GShape,p:GDPlan,params:[]GDParam,o:NxOptions,seed:u64,limit:int,ops:[]GDOp,args:[]GDArg)->GDProgram #export_c {
    if p.status!=GStatus.ok || !g_shape(s) || s.capacity>4096 || gs_options(o,seed,s.vocab)!=GStatus.ok {return GDProgram.{status=GStatus.shape};}
    if limit<1 || limit>s.capacity {return GDProgram.{status=GStatus.state};}
    k:=gs_k(o);lay:=gs_layout(p,s.vocab,k);
    if params.count!=3+15*s.layers || ops.count<p.max_ops || args.count<p.max_args || lay.status!=GStatus.ok {return GDProgram.{status=GStatus.capacity};}
    w:=GDWriter.{plan=p,ops=ops,args=args};d:=s.width;q:=s.heads*64;k2:=s.kv_heads*64*2;h:=s.hidden;z:=s.selected;
    control:=gd_all(p,24);x:=gd_all(p,0);normal:=gd_all(p,1);qkv:=gd_all(p,2);attention:=gd_all(p,3);projected:=gd_all(p,4);router:=gd_all(p,5);up:=gd_all(p,6);activated:=gd_all(p,7);down:=gd_all(p,8);
    w=gs_emit(w,48,1,2,1,GDScalars.{a=s.vocab,b=s.capacity},GDViews.{a=control});w=gd_emit(w,1,p.flags,0,1,GDScalars.{},GDViews.{a=gd_all(p,22)});
    w=gd_emit(w,2,d,1,4,GDScalars.{a=d},GDViews.{a=gd_weight(params,0,2),b=control,c=x,d=gd_flags(w,d)});w.result.flags+=d;
    for layer:0..<s.layers {base:=3+15*layer;ids:=gd_slice(p,14,layer*z,z);weights:=gd_slice(p,15,layer*z,z);pending:=gd_slice(p,12,layer*k2,k2);trace:=gd_slice(p,13,layer*2*d,d);cache:=gd_slice(p,16,g_cache_base(s,layer),k2*g_layer_capacity(s,layer));
        w=gd_norm(w,d,gd_weight(params,base,2),x,normal);
        w=gd_bf(w,q+k2,d,gd_weight(params,base+4,8),gd_weight(params,base+3,2),normal,qkv);
        rotations:=(s.heads+s.kv_heads)*32;w=gs_emit(w,49,rotations,2,5,GDScalars.{a=s.heads,b=s.kv_heads},GDViews.{a=qkv,b=gd_all(p,25),c=pending,d=gd_flags(w,rotations),e=control});w.result.flags+=rotations;
        window:=0;span:=s.capacity;if layer%2==0 {window=s.window;span=gs_min(s.window,s.capacity);}cap:=g_layer_capacity(s,layer);
        w=gs_emit(w,50,s.heads*span,5,5,GDScalars.{a=s.heads,b=s.kv_heads,c=window,d=cap,e=s.capacity},GDViews.{a=qkv,b=cache,c=pending,d=gd_all(p,11),e=control});
        w=gs_emit(w,51,s.heads,2,5,GDScalars.{a=window,b=s.capacity},GDViews.{a=gd_weight(params,base+5,2),b=gd_all(p,11),c=gd_all(p,10),d=gd_flags(w,s.heads),e=control});w.result.flags+=s.heads;
        w=gs_emit(w,52,s.heads*span,3,3,GDScalars.{a=s.heads,b=window,c=s.capacity},GDViews.{a=gd_all(p,10),b=gd_all(p,11),c=control});
        w=gs_emit(w,53,q*8,6,5,GDScalars.{a=s.heads,b=s.kv_heads,c=window,d=cap,e=s.capacity,f=8},GDViews.{a=gd_all(p,11),b=cache,c=pending,d=gd_all(p,9),e=control});
        w=gd_emit(w,17,q,2,3,GDScalars.{a=q,b=8},GDViews.{a=gd_all(p,9),b=attention,c=gd_flags(w,q)});w.result.flags+=q;
        w=gd_bf(w,d,q,gd_weight(params,base+2,8),gd_weight(params,base+1,2),attention,projected);
        w=gd_emit(w,9,d,0,4,GDScalars.{},GDViews.{a=x,b=projected,c=trace,d=gd_flags(w,d)});w.result.flags+=d;
        w=gd_norm(w,d,gd_weight(params,base+10,2),x,normal);
        w=gd_bf(w,s.experts,d,gd_weight(params,base+7,8),gd_weight(params,base+6,2),normal,router);
        w=gd_emit(w,11,1,1,4,GDScalars.{a=z},GDViews.{a=router,b=ids,c=weights,d=gd_flags(w,1)});w.result.flags+=1;
        w=gd_mx(w,2*h,d,z,0,gd_weight(params,base+11,8),gd_weight(params,base+12,1),gd_weight(params,base+8,2),ids,normal,up);
        w=gd_emit(w,10,z*h,1,3,GDScalars.{a=h},GDViews.{a=up,b=activated,c=gd_flags(w,z*h)});w.result.flags+=z*h;
        w=gd_mx(w,d,h,z,h,gd_weight(params,base+13,8),gd_weight(params,base+14,1),gd_weight(params,base+9,2),ids,activated,down);
        w=gd_emit(w,12,d,2,5,GDScalars.{a=d,b=z},GDViews.{a=x,b=down,c=weights,d=gd_slice(p,13,(2*layer+1)*d,d),e=gd_flags(w,d)});w.result.flags+=d;
    }
    w=gd_norm(w,d,gd_weight(params,1,2),x,normal);
    w=gd_bf(w,s.vocab,d,gd_weight(params,2,8),GDRef.{elem=2},normal,gd_all(p,17));
    summaries:=gs_lists(w.result.flags,256);w=gd_emit(w,18,summaries,0,2,GDScalars.{},GDViews.{a=gd_slice(p,22,0,w.result.flags),b=gd_slice(p,23,0,summaries)});
    w=gd_emit(w,19,1,0,2,GDScalars.{},GDViews.{a=gd_slice(p,23,0,summaries),b=control});
    items:=gs_max(gs_max(s.vocab,p.counts[12]),p.counts[13]);
    w=gd_emit(w,20,items,5,11,GDScalars.{a=s.layers,b=d,c=s.kv_heads,d=s.window,e=s.capacity},GDViews.{a=control,b=gd_all(p,12),c=gd_all(p,17),d=gd_all(p,13),e=gd_all(p,14),f=gd_all(p,15),g=gd_all(p,16),h=gd_all(p,18),i=gd_all(p,19),j=gd_all(p,20),k=gd_all(p,21)});
    w=gd_emit(w,21,1,0,1,GDScalars.{},GDViews.{a=control});
    logits:=gd_all(p,18);n:=gs_lists(s.vocab,GS_PART);
    w=gs_emit(w,54,n,2,4,GDScalars.{a=GS_PART,b=k},GDViews.{a=logits,b=lay.ids,c=lay.counts,d=lay.flags});
    from:=0;while n>1 {m:=gs_up(n);w=gs_emit(w,55,m,4,4,GDScalars.{a=from,b=n,c=from+n,d=k},GDViews.{a=logits,b=lay.ids,c=lay.counts,d=lay.flags});from+=n;n=m;}
    w=gs_emit(w,56,1,6,9,GDScalars.{a=from,b=k,c=cast(int)f64_bits(o.temperature),d=cast(int)f64_bits(o.top_p),e=cast(int)seed,f=limit},GDViews.{a=logits,b=lay.ids,c=lay.counts,d=lay.flags,e=lay.weights,f=lay.sorted,g=lay.by_id,h=control,i=gd_all(p,GS_EMITTED)});
    if w.result.flags>p.flags {return GDProgram.{status=GStatus.capacity};}return w.result;
}
```

`gs_program` is `gd_program` with six launches replaced and the greedy
choice replaced by the sampler. The sampler's lists use slot 40, prefill
scratch that no decode launch reads; emitted IDs go to slot 26, which holds
prompt IDs only while a prefill runs. The checks compare every other launch
with `gd_program`'s, argument for argument, on the tiny model and on four
layers of the 20b shape.

## Checks

`coax test` on each file, with `coax mutate` leaving no survivor in either
(326 mutants in `device-sample.coax`, 440 in `device-step.coax`):

- **Independent oracle.** `device-sample-oracle.coax` writes chapter 53's
  rule a second way, with a selection sort and no lists. For 48 seeds,
  vocabularies of 20 to 161 logits with many ties, four temperatures
  (including 0), four top-p values and parts of 1, 3, 7 and 256 (so zero,
  one and two merge levels), three draws each: the list order, token,
  counter, stop code and every kept weight's bits equal the oracle's.
- **Chapter 53 itself.** `device-step.coax` runs the same 48 cases against
  `nx_step` and `nx_distribution`: equal token, counter, emitted count and
  stop, the same first k IDs as chapter 53's order and the same weight bits.
- **Edges.** The rounding fallback, a top-p boundary that is reached
  exactly, stop tokens against the limit, a refusal in a merged list and the
  bound `0x7149f2c9` on both sides.
- **No position in the program.** No chapter 70 launch that takes a
  position (kinds 0, 13, 14, 16, 43 and 44) and no host choice (22, 23)
  remains; every other launch equals `gd_program`'s at a position past the
  window; each wrapper equals the kernel it wraps on small arrays at three
  windows.

## Run it

[`device-step.coax`](../examples/device-step.coax) replays the sampler's
launches on the CPU for the worked example and builds the recorded step.

```shell
coax/bin/coax run psionic/examples/device-step.coax
```

<!-- source: psionic/examples/device-step.coax example -->
```jai
main :: () {
    // Three draws from logits 2 1 2 0 -1, temperature 1, top-k 5, top-p 1,
    // seed 45001, limit 3. Lists of two logits, so three lists and a merge.
    x:=[5]f32.[2,1,2,0,-1];ids:[40]int;counts:[8]int;flags:[8]u8;weights:[5]f64;sorted:[5]int;by_id:[5]f64;control:[8]int;emitted:[3]int;
    for step:0..<3 {
        from:=gs_sample_cpu(x[..],2,5,1,1,45001,3,ids[..],counts[..],flags[..],weights[..],sorted[..],by_id[..],control[..],emitted[..]);
        print("draw % (counter %): uniform %, token %, final list % of %:",step,step,f64(gs_bits(45001,cast(u64)step)>>11)/9007199254740992.0,control[1],from,counts[from]);
        for i:0..<counts[from] {print(" %",ids[from*5+i]);}print("\n");
    }
    print("weights in list order:");for w:weights {print(" %",w);}print("\n");
    print("control words: status % token % position % emitted % stop %\n",control[0],control[1],control[2],control[3],control[4]);
    print("emitted: % % %\n",emitted[0],emitted[1],emitted[2]);
    // The recorded step for the tiny two-layer model, and the sampler tail
    // for the 20b vocabulary (four of its layers).
    pins:=GOWN_ARTIFACTS;specs:=GOWN_SPECS;profile:=GProfile.{shape=GT_SHAPE,pins=pins[..],specs=specs[..]};params:[33]GDParam;r:=gd_params(profile,params[..]);
    p:=gd_plan(GT_SHAPE,pins[..],1000000000,1000000000);ops:[192]GDOp;args:[4608]GDArg;
    g:=gs_program(GT_SHAPE,p,params[..],NxOptions.{temperature=0.7,top_k=8,top_p=0.9},45001,8,ops[..],args[..]);
    seen:[57]int;for i:0..<g.ops {seen[ops[i].kind]+=1;}
    print("tiny step: % launches, % arguments; position launches of chapter 70: %; step kernels:",g.ops,g.args,seen[0]+seen[13]+seen[14]+seen[16]+seen[43]+seen[44]);
    for kind:48..<57 {print(" %=%",gs_kernel_name(kind),seen[kind]);}print("\n");
    o:=g_original_shape(0,2048);o.layers=4;large:=G20_ARTIFACTS;q:=gd_plan(o,large[..],17179869184,17179869184);big:[63]GDParam;bops:[352]GDOp;bargs:[8448]GDArg;
    b:=gs_program(o,q,big[..],NxOptions.{top_k=64,top_p=0.95},45001,256,bops[..],bargs[..]);
    print("20b vocabulary sampler:");for i:b.ops-4..<b.ops {print(" %x%",gs_kernel_name(bops[i].kind),bops[i].items);}print("\n");
    l:=gs_layout(q,o.vocab,64);print("lists % (final %), scratch bytes in slot 40: %\n",l.lists,l.final,l.flags.offset+l.flags.count);
}
```

```text
draw 0 (counter 0): uniform 0.3923734995272653, token 1, final list 3 of 5: 0 2 1 3 4
draw 1 (counter 1): uniform 0.2934748440629037, token 0, final list 3 of 5: 0 2 1 3 4
draw 2 (counter 2): uniform 0.022896472815722047, token 0, final list 3 of 5: 0 2 1 3 4
weights in list order: 0.39169576881208695 0.39169576881208695 0.14409682053980902 0.05301025781476656 0.019501384021250404
control words: status 0 token 0 position 0 emitted 3 stop 3
emitted: 1 0 0
tiny step: 61 launches, 376 arguments; position launches of chapter 70: 0; step kernels: gs_begin=1 gs_rotate=2 gs_scores=2 gs_prepare=2 gs_divide=2 gs_context=2 gs_top_part=1 gs_top_merge=0 gs_pick=1
20b vocabulary sampler: gs_top_partx786 gs_top_mergex25 gs_top_mergex1 gs_pickx1
lists 812 (final 811), scratch bytes in slot 40: 424588
```

The list size of two gives three lists and one merge (list 3). The step for
the tiny two-layer model has 61 launches; none of chapter 70's position
launches remain. For the 20b vocabulary the sampler is four launches: 786
part items, merges of 25 and 1, and the pick.

## What is refused

[`device-step-refuse.coax`](../examples/device-step-refuse.coax) shows each
refusal with its typed reason.

<!-- source: psionic/examples/device-step-refuse.coax refuse -->
```jai
main :: () {
    v:=201088;
    show("temperature 1, top-k 64, top-p 0.95",gs_options(NxOptions.{top_k=64,top_p=0.95},45001,v));
    show("temperature 1, top-k off",gs_options(NxOptions.{},45001,v));
    show("temperature 1, top-k 65",gs_options(NxOptions.{top_k=65},45001,v));
    show("greedy, top-k off",gs_options(NxOptions.{temperature=0},45001,v));
    show("a seed of 2^63",gs_options(NxOptions.{top_k=8},(cast(u64)1)<<63,v));
    pins:=GOWN_ARTIFACTS;specs:=GOWN_SPECS;profile:=GProfile.{shape=GT_SHAPE,pins=pins[..],specs=specs[..]};params:[33]GDParam;r:=gd_params(profile,params[..]);
    p:=gd_plan(GT_SHAPE,pins[..],1000000000,1000000000);ops:[192]GDOp;args:[4608]GDArg;o:=NxOptions.{top_k=4};
    show("a limit of 9 in a context of 8",gs_program(GT_SHAPE,p,params[..],o,1,9,ops[..],args[..]).status);
    tight:=p;tight.counts[GS_SCRATCH]=1;show("no room for the lists in slot 40",gs_program(GT_SHAPE,tight,params[..],o,1,4,ops[..],args[..]).status);
    words:[8]int;show("start words for ID 16 of 16",gs_control(GT_SHAPE,16,7,words[..]).status);
    show("start words at position 8 of 8",gs_control(GT_SHAPE,3,8,words[..]).status);
    // A logit past 10^30: the pick sets status 11 and publishes nothing.
    x:=[4]f32.[1,2,1000000000000000000000000000000000.0,0];ids:[8]int;counts:[2]int;flags:[2]u8;w:[2]f64;so:[2]int;by:[2]f64;control:=[8]int.[0,3,9,0,0,0,0,0];emitted:[4]int;
    from:=gs_sample_cpu(x[..],4,2,1,1,7,4,ids[..],counts[..],flags[..],w[..],so[..],by[..],control[..],emitted[..]);
    print("a logit of 1e33: list % flagged %, status % token % emitted %\n",from,flags[from],control[0],control[1],control[3]);
    // A step after a stop changes nothing; its status says so.
    control=[8]int.[0,5,9,4,3,0,0,0];gs_begin(0,16,8,control[..]);
    print("a step after a limit stop: status % token % position % emitted %\n",control[0],control[1],control[2],control[3]);
}
```

```text
temperature 1, top-k 64, top-p 0.95: ok
temperature 1, top-k off: shape
temperature 1, top-k 65: shape
greedy, top-k off: ok
a seed of 2^63: shape
a limit of 9 in a context of 8: state
no room for the lists in slot 40: capacity
start words for ID 16 of 16: state
start words at position 8 of 8: state
a logit of 1e33: list 0 flagged 1, status 11 token 3 emitted 0
a step after a limit stop: status 10 token 5 position 9 emitted 4
```

Both examples exit with status 0; the transcripts are compared byte for byte
in debug and release builds.

## On the RTX 4080

```shell
COAX_CC=gcc psionic/tests/check_device_step.sh \
  ~/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee 5
```

The harness opens the original 20b, records two graphs (greedy, and
temperature 1, top-k 64, top-p 0.95, seed 45001) and runs four paths on
[chapter 71](ch071-throughput-is-a-gate.md)'s fixture prompts with a limit
of 256 IDs:

- **host greedy:** chapter 70's replayed step with a wait per token;
- **device greedy:** the recorded step, sixteen launches per read;
- **host sampled:** a step, a download of the logits and `nx_step` on the
  CPU;
- **device sampled:** the recorded step, sixteen launches per read.

`pc_waits` counts every call that blocks the host on the device.

```text
device: NVIDIA GeForce RTX 4080 sm_89
recorded step: 591 launches in one graph
recorded step: 591 launches in one graph
greedy prompt=0 host ids=24 word=447e9a1f919e90b3 | device ids=24 word=447e9a1f919e90b3 | equal
  host: 23 steps, 23 host waits; device: 32 launches, 0 host waits inside them, 2 at batch boundaries
seeded prompt=0 host ids=19 word=ee9349115d9c05a7 | device ids=19 word=ee9349115d9c05a7 | equal
  host: 19 steps, 38 host waits; device: 32 launches, 0 host waits inside them, 2 at batch boundaries
  last draw: 2 kept IDs; device order and weight bits differ from nx_distribution at 0
greedy prompt=1 host ids=256 word=7c6705e4ab014dfd | device ids=256 word=7c6705e4ab014dfd | equal
  host: 255 steps, 255 host waits; device: 256 launches, 0 host waits inside them, 16 at batch boundaries
seeded prompt=1 host ids=256 word=5a58c53a41602aa7 | device ids=256 word=5a58c53a41602aa7 | equal
  host: 256 steps, 512 host waits; device: 256 launches, 0 host waits inside them, 16 at batch boundaries
  last draw: 1 kept IDs; device order and weight bits differ from nx_distribution at 0
host greedy (replay, wait per token)   tokens=255 launches=255 n=5 mean_ns=3875861793 sd_ns=5129565 tokens_per_s=65.79
device greedy (recorded step)          tokens=255 launches=256 n=5 mean_ns=3895444036 sd_ns=4665849 tokens_per_s=65.46
host sampled (logits read, nx_step)    tokens=256 launches=256 n=5 mean_ns=8722577691 sd_ns=3558706 tokens_per_s=29.35
device sampled (recorded step)         tokens=256 launches=256 n=5 mean_ns=5144998662 sd_ns=6978623 tokens_per_s=49.76
all comparisons equal
```

- **Outputs.** Greedy outputs on both prompts equal chapter 71's reference
  words (`447e9a1f919e90b3`, `7c6705e4ab014dfd`). Seeded outputs equal the
  host path that downloads the logits and calls `nx_step`, ID for ID.
- **Weights.** After each sampled run, the last draw's kept IDs and weight
  bits on the device equal `nx_distribution` on the same logits.
- **Waits.** No host wait happens while steps are launched. The device path
  waits once per sixteen steps to read the control words; the host paths
  wait once (greedy) or twice (sampled) per token. Steps launched past a
  stop do nothing: on "Say hi." the greedy path needs 23 decode steps and
  launches 32, so 9 change nothing.

The timed workload is the list prompt to its limit of 256 IDs, one warm-up
and five runs per path, summarized with chapter 34's two-pass mean and
standard deviation (`cm_two_pass`):

| Path | Mean per run (sd) | Tokens/s |
| --- | ---: | ---: |
| host greedy (chapter 70 replay, a wait per token) | 3.875862 s (5.1 ms) | 65.79 |
| device greedy (recorded step) | 3.895444 s (4.7 ms) | 65.46 |
| host sampled (logits downloaded, `nx_step` on the CPU) | 8.722578 s (3.6 ms) | 29.35 |
| device sampled (recorded step) | 5.144999 s (7.0 ms) | 49.76 |

Sampling on the device is 1.70 times faster than sampling on the host: the
host path spends about 18.9 ms per token beyond a greedy step on the
download and chapter 53's full sort, the device path about 4.9 ms on its
lists and merges. Greedy decoding gains nothing: the device path is 19.6 ms
(0.5%) slower per run, a difference about six times its standard error of
3.1 ms. Chapter 70's step was already device-bound at 15.2 ms per token, so
removing the host's patching and waits saves less than the full-span score
and divide launches cost. The sampler's single-thread parts and merges are
the next thing to make faster; they are not tuned here.

## What remains

- **The DeepSeek V4.1 step.** This chapter's plan was the V4.1 decode step
  with [chapter 96](ch096-hot-experts-in-fast-memory.md)'s expert cache and
  [chapter 98](ch098-pack-the-cache-exactly.md)'s packed cache. Both are CPU
  transactions today; there is no V4.1 device engine to record.
- **A host one step ahead with late speculative lengths.** The host here
  launches a fixed batch and reads once. Scheduling step N + 1 while a
  verifier ([chapter 102](ch102-verify-drafts-by-expected-gain.md)) decides
  step N's accepted length, with buffers that stay valid until it arrives,
  is not built.
- **Cancellation inside a batch.** A request can be cancelled between
  batches. Launched steps run to the end of their batch, and their no-op
  behavior after a stop is the only guarantee.
- **Grammar masks and wide top-k.** Chapter 53's byte-grammar mask is not
  applied on the device, and top-k above 64 (or off) at a nonzero
  temperature is refused.
- **Bitwise `exp`.** The weights are compared after each run, not
  guaranteed: CUDA's and the C library's double `exp` may differ in the last
  bit on some input.
