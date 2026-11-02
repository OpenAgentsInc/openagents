# 183. Split a model across devices

> **Status: partly implemented.** Tensor parallelism for chapter 113's
> decoder runs on the CPU (profile tensor-parallel-v1). A planner splits
> the attention heads and the FFN columns over 1 to 8 logical devices,
> refuses splits that do not divide them or that exceed a device's memory
> budget, and counts each token's all-reduce bytes with chapter 133's
> `sd_cost`. Every device runs as a POSIX thread with the same bits as the
> devices run in turn, and the split matches the one-device forward bit for
> bit at one device and within 1e-12 at 2, 4 and 8. Several GPUs, the RTX
> 4080 as a member, expert parallelism, a vocabulary-parallel head and
> pipeline stages are not built; the last section lists them.

Some models are too large or too slow for one device. Tensor parallelism
splits every layer's weights over several devices in one machine, so each
device holds part of every layer and does part of every token's work. The
devices must then combine their parts after each layer, and that
communication is the price. This chapter is the tightly coupled case: one
machine with a fast interconnect. Part XX handles devices spread across
machines.

The split here is the one Megatron-LM uses for a transformer block. It
builds on four earlier chapters:

- [chapter 113](ch113-differentiate-a-decoder.md)'s tiny decoder (`td_*`)
  is the model, and its `td_forward` is the one-device reference;
- [chapter 133](ch133-a-tensor-spread-over-devices.md)'s sharded layouts
  give the rules that make the split work and `sd_cost` gives its bytes;
- [chapter 132](ch132-train-across-devices.md)'s device group shows how to
  run logical devices as threads and keep the bits fixed;
- [chapter 33](ch033-measure-the-work-and-plan-the-memory.md)'s question, whether a model fits a
  device's memory, becomes how many devices a model needs.

The decisions are in an `#authority` file,
[`tensor-parallel.coax`](../lib/tensor-parallel.coax): which splits are
allowed, what each device holds, which runs are admitted, and the order of
every addition in the all-reduce. The arithmetic of each half of a block is
in [`tensor-parallel-run.coax`](../lib/tensor-parallel-run.coax).

## One all-reduce per half block

A decoder block has two halves, and each half reads the replicated
residual `x` and adds a result back to it:

- attention: `x + Wo · attend(Wq · n(x), Wk · n(x), Wv · n(x))`;
- FFN: `x + W2 · relu(W1 · n(x))²`, with `n` the RMS norm.

Split Wq, Wk, Wv and W1 by their output rows. Each device then computes
its own queries, keys and values, so it holds whole attention heads, and
its own FFN columns. In chapter 133's terms the input is replicated and
the weight is sharded on columns, and `sd_matmul_rule` gives a result
sharded on columns. Attention works head by head and the squared ReLU
works column by column, so neither needs any other device's data.

Split Wo and W2 by their input columns, the same heads and FFN columns.
The contraction is now split, and chapter 133's rule gives a partial: each
device holds an addend of the full result. Turning a partial into a
replicated tensor is an all-reduce. So each half needs exactly one
all-reduce and nothing else, which this check states with chapter 133's
own rules:

```text
replicated  ·  shard(columns)  =  shard(columns)      Wq, Wk, Wv, W1
relu(shard(columns))           =  shard(columns)      attention, squared ReLU
shard(columns) · shard(rows)   =  partial             Wo, W2
partial -> replicated          =  all-reduce          one per half block
```

The norms, the embedding and the vocabulary head stay replicated: every
device normalizes its own copy of `x`. A split of the norm's input would
need an all-gather first.

## The plan

`tl_plan` takes a shape, a device count and a per-device budget in bytes.
It refuses, in order: a shape chapter 113 does not accept, a device count
outside 1 to 8, a count that does not divide the heads, one that would
give a device part of a key-value head, one that does not divide the FFN
columns, and a device whose weights and cache exceed the budget.

<!-- source: psionic/lib/tensor-parallel.coax plan -->
```jai
// What one device holds and what the split moves. heads, kv_heads and hidden
// are per device; shared is true when several devices hold the same
// key-value head. shard and cache count values, bytes is 8 a value; work is
// the device's scratch beyond its cache. reduce is the bytes one all-reduce
// of one position moves; token is the bytes per token for the whole model.
TlPlan :: struct {status:TlStatus;shape:TdShape;devices:int;heads:int;kv_heads:int;shared:bool;hidden:int;shard:int;cache:int;bytes:int;work:int;reduce:int;token:int;}
tl_plan :: (s:TdShape,devices:int,budget:int)->TlPlan #export_c {
    p:=TlPlan.{shape=s,devices=devices};
    if !td_valid(s) {p.status=TlStatus.shape;return p;}
    if devices<1 || devices>TL_MAX_DEVICES {p.status=TlStatus.devices;return p;}
    if s.heads%devices!=0 {p.status=TlStatus.heads;return p;}
    // A device's heads must read whole key-value heads: either the key-value
    // heads split evenly too, or several devices share one.
    if s.kv_heads%devices!=0 && devices%s.kv_heads!=0 {p.status=TlStatus.groups;return p;}
    if s.hidden%devices!=0 {p.status=TlStatus.hidden;return p;}
    p.heads=s.heads/devices;p.shared=devices>s.kv_heads;p.kv_heads=1;if !p.shared {p.kv_heads=s.kv_heads/devices;}p.hidden=s.hidden/devices;
    d:=s.width;q:=p.heads*s.head_width;k:=p.kv_heads*s.head_width;
    p.shard=2*s.vocab*d+d+s.layers*(2*d+q*d+2*k*d+d*q+2*p.hidden*d);
    p.cache=s.layers*s.context*2*k;p.bytes=8*(p.shard+p.cache);
    p.work=s.context*d+d+q+s.context+p.hidden;
    p.reduce=sd_cost(1,d,devices,sd_partial(),sd_replicated());p.token=2*s.layers*p.reduce;
    if p.bytes>budget {p.status=TlStatus.memory;}
    return p;
}
// The fewest devices, up to `most`, whose split is allowed and fits the
// budget. When none fits, the memory refusal at the most devices that divide.
tl_choose :: (s:TdShape,most:int,budget:int)->TlPlan #export_c {
    best:=tl_plan(s,1,budget);
    if most<1 {best.status=TlStatus.devices;}
    for n:2..<most+1 {
        if best.status!=TlStatus.memory {return best;}
        p:=tl_plan(s,n,budget);if p.status==TlStatus.ok || p.status==TlStatus.memory {best=p;}
    }
    return best;
}
```

### Key-value heads

Chapter 113's attention is grouped: several query heads read one
key-value head. Device `d` holds query heads `d * H / N` up to
`(d + 1) * H / N`. Its heads read whole key-value heads in two cases:

- the key-value heads split evenly too (`kv_heads % N == 0`), so each
  device holds its own;
- several devices share one (`N % kv_heads == 0`), so each of them holds a
  copy of it. The plan reports this as `shared`.

With 6 heads over 3 key-value heads and 2 devices, neither holds. Device 0
would get heads 0, 1 and 2, but heads 0 and 1 read key-value head 0 and
head 2 reads key-value head 1, which device 1 also needs. The plan refuses
that split.

### A plan by hand

The chapter's shape `TL_SPLIT` has vocabulary 6, width 8, 4 heads of width
2 over 2 key-value heads, 8 FFN columns, 2 layers and a context of 8:
chapter 113's parameter layout gives 776 parameters. Split over 2 devices:

- heads: 4 / 2 = 2 a device, so q = 2 * 2 = 4 query values;
- key-value heads: 2 % 2 = 0, so they split, 1 a device, k = 2 values;
- FFN columns: 8 / 2 = 4 a device.

One device's shard is both embedding tables and the final norm, then per
layer two norms, its rows of Wq, Wk and Wv, its columns of Wo and its rows
and columns of W1 and W2:

```text
2 * 6 * 8 + 8                                  = 104
2 * (2*8 + 4*8 + 2*2*8 + 8*4 + 2*4*8)          = 2 * 176 = 352
shard                                          = 456 values
cache   2 layers * 8 positions * 2 * 2         = 64 values
bytes   8 * (456 + 64)                         = 4,160
```

One device alone holds 776 values and a 128-value cache, 7,232 bytes.
Four devices hold 328 and 64, 3,136 bytes: the embedding and head, which
every device keeps, are 104 of every 328. `tl_choose` asks for the fewest
devices that fit a budget: 8,000 bytes gives 1 device, 4,200 gives 2, and
3,100 gives none (the closest, 4 devices, needs 3,136).

The bytes count weights and the key-value cache. Each device also needs
scratch for one row of attention probabilities, a normed row, its queries
and its FFN columns, `work` values in the plan, and the shared addend and
sum rows; those are buffers the caller allocates, not part of the budget.

## What each device holds

Chapter 113 stores each weight row-major with one row per output. A
device's rows of Wq, Wk, Wv and W1 are one contiguous run; its columns of
Wo and W2 are a run inside every row. `tl_scatter` copies device `d`'s
pieces into its shard, with the same block layout as chapter 113 at the
smaller sizes.

<!-- source: psionic/lib/tensor-parallel.coax scatter -->
```jai
// Where each piece sits in one device's shard: the embedding, every block's
// norms and its slices of the projections, the final norm and the head.
tl_block :: (p:TlPlan,layer:int)->TdBlock {
    s:=p.shape;d:=s.width;q:=p.heads*s.head_width;k:=p.kv_heads*s.head_width;b:TdBlock;
    b.norm1=s.vocab*d+layer*(2*d+q*d+2*k*d+d*q+2*p.hidden*d);
    b.wq=b.norm1+d;b.wk=b.wq+q*d;b.wv=b.wk+k*d;b.wo=b.wv+k*d;b.norm2=b.wo+d*q;b.w1=b.norm2+d;b.w2=b.w1+p.hidden*d;return b;
}
tl_final_norm :: (p:TlPlan)->int {return tl_block(p,p.shape.layers).norm1;}
tl_unembed :: (p:TlPlan)->int {return tl_final_norm(p)+p.shape.width;}
// Device d's first query head and first key-value head.
tl_head0 :: (p:TlPlan,d:int)->int {return d*p.heads;}
tl_kv0 :: (p:TlPlan,d:int)->int {return tl_head0(p,d)/(p.shape.heads/p.shape.kv_heads);}
tl_rows :: (from:[]f64,at:int,to:[]f64,dest:int,n:int) {for i:0..<n {to[dest+i]=from[at+i];}}
// Copies device d's slice of the full parameters into its shard. Wq, Wk, Wv
// and W1 are row-major with one row per output, so a device's rows are one
// run; Wo and W2 take a run of columns from every row.
tl_scatter :: (p:TlPlan,d:int,params:[]f64,shard:[]f64) #export_c {
    s:=p.shape;w:=s.width;e:=s.head_width;q:=p.heads*e;k:=p.kv_heads*e;base:=d*p.shard;
    tl_rows(params,0,shard,base,s.vocab*w);
    for l:0..<s.layers {g:=td_block(s,l);b:=tl_block(p,l);
        for r:0..<w {tl_rows(params,g.wo+r*s.heads*e+tl_head0(p,d)*e,shard,base+b.wo+r*q,q);
            tl_rows(params,g.w2+r*s.hidden+d*p.hidden,shard,base+b.w2+r*p.hidden,p.hidden);}
        tl_rows(params,g.w1+d*p.hidden*w,shard,base+b.w1,p.hidden*w);
        tl_rows(params,g.wv+tl_kv0(p,d)*e*w,shard,base+b.wv,k*w);tl_rows(params,g.wk+tl_kv0(p,d)*e*w,shard,base+b.wk,k*w);
        tl_rows(params,g.wq+tl_head0(p,d)*e*w,shard,base+b.wq,q*w);
        tl_rows(params,g.norm2,shard,base+b.norm2,w);tl_rows(params,g.norm1,shard,base+b.norm1,w);}
    tl_rows(params,td_unembed(s),shard,base+tl_unembed(p),s.vocab*w);tl_rows(params,td_final_norm(s),shard,base+tl_final_norm(p),w);
}
```

## The two halves

Each half runs on one device for a run of positions. The arithmetic is
chapter 113's `td_advance`, operation for operation, restricted to the
device's heads and columns. Positions before `first` are already in the
device's key-value cache, so the same call prefills a prompt (`first = 0`,
all positions) or decodes one token (`n = 1`).

<!-- source: psionic/lib/tensor-parallel-run.coax halves -->
```jai
// The attention half of layer l on device d for positions first..<first+n:
// its heads' queries, keys and values, causal attention over the keys it
// holds, and its run of Wo's columns. The result is device d's addend of the
// output, written to its partials. Same arithmetic, in the same order, as
// chapter 113's td_advance.
tl_attend :: (p:TlPlan,d:int,l:int,shards:[]f64,works:[]f64,first:int,n:int,partials:[]f64) #export_c {
    s:=p.shape;w:=s.width;e:=s.head_width;q:=p.heads*e;k:=p.kv_heads*e;group:=s.heads/s.kv_heads;scale:=1/sqrt(f64(e));
    sh:=shards[d*p.shard..<(d+1)*p.shard];x:=works[d*tl_work_count(p)..<(d+1)*tl_work_count(p)];b:=tl_block(p,l);
    row:=tl_scratch(p);nm:=row+s.context;qs:=nm+w;cs:=nm;keys:=tl_keys(p,l);values:=tl_values(p,l);
    for t:first..<first+n {key:=keys+t*k;value:=values+t*k;
        _:=td_norm(x,t*w,sh,b.norm1,w,x,nm);
        td_matvec(sh,b.wq,q,w,x,nm,x,qs);td_matvec(sh,b.wk,k,w,x,nm,x,key);td_matvec(sh,b.wv,k,w,x,nm,x,value);
        td_rope(x,qs,p.heads,e,f64(t));td_rope(x,key,p.kv_heads,e,f64(t));
        for h:0..<p.heads {g:=(tl_head0(p,d)+h)/group-tl_kv0(p,d);
            for j:0..<t+1 {score:=0.0;for c:0..<e {score+=x[qs+h*e+c]*x[keys+j*k+g*e+c];}x[row+j]=score*scale;}
            top:=x[row+td_argmax(x[row..<row+t+1])];
            total:=0.0;for j:0..<t+1 {x[row+j]=exp(x[row+j]-top);total+=x[row+j];}for j:0..<t+1 {x[row+j]/=total;}
            for c:0..<e {acc:=0.0;for j:0..<t+1 {acc+=x[row+j]*x[values+j*k+g*e+c];}x[cs+h*e+c]=acc;}}
        td_matvec(sh,b.wo,w,q,x,cs,partials,(d*s.context+t)*w);}
}
// The FFN half: its FFN columns of W1, the squared ReLU, and its run of W2's
// columns, again as an addend. The ReLU is (z + |z|) / 2, exact, so a column
// at or below zero adds W2 * 0 * 0, a zero that leaves the sum's bits as
// chapter 113's skipped term does.
tl_ffn :: (p:TlPlan,d:int,l:int,shards:[]f64,works:[]f64,first:int,n:int,partials:[]f64) #export_c {
    s:=p.shape;w:=s.width;
    sh:=shards[d*p.shard..<(d+1)*p.shard];x:=works[d*tl_work_count(p)..<(d+1)*tl_work_count(p)];b:=tl_block(p,l);
    nm:=tl_scratch(p);u:=tl_work_count(p)-p.hidden;
    for t:first..<first+n {_:=td_norm(x,t*w,sh,b.norm2,w,x,nm);td_matvec(sh,b.w1,p.hidden,w,x,nm,x,u);
        for f:0..<p.hidden {z:=x[u+f];x[u+f]=(z+abs(z))*0.5;}
        for i:0..<w {acc:=0.0;for f:0..<p.hidden {z:=x[u+f];acc+=sh[b.w2+i*p.hidden+f]*z*z;}partials[(d*s.context+t)*w+i]=acc;}}
}
```

## The all-reduce

Every device's addend goes to a shared array of partials. The all-reduce
is a reduce-scatter and an all-gather. Device `d` owns the columns chapter
132's split gives it, sums every device's addend for those columns in
device order, adds the residual, and writes the result to a shared row.
Then every device copies the whole row into its own residual.

<!-- source: psionic/lib/tensor-parallel.coax reduce -->
```jai
// The all-reduce, as a reduce-scatter and an all-gather. Device d owns
// columns sd_first(width, N, d) up to sd_first(width, N, d + 1): it adds
// every device's addend for them in device order, then its own residual,
// and writes the sums to the shared row. Then every device copies the whole
// row into its residual. Every column is summed once, in one order, so the
// bits do not depend on which device finishes first.
tl_reduce :: (p:TlPlan,d:int,works:[]f64,first:int,n:int,partials:[]f64,reduced:[]f64) #export_c {
    s:=p.shape;w:=s.width;x:=d*tl_work_count(p);
    for t:first..<first+n {for i:sd_first(w,p.devices,d)..<sd_first(w,p.devices,d+1) {v:=partials[t*w+i];
        for e:1..<p.devices {v+=partials[(e*s.context+t)*w+i];}reduced[t*w+i]=v+works[x+t*w+i];}}
}
tl_gather :: (p:TlPlan,d:int,works:[]f64,first:int,n:int,reduced:[]f64) #export_c {
    w:=p.shape.width;for t:first..<first+n {tl_rows(reduced,t*w,works,d*tl_work_count(p)+t*w,w);}
}
```

Each column is summed once, by one device, in one order. The bits do not
depend on which thread finishes first, so the threads give the same bits
as the devices run in turn.

### An all-reduce by hand

Take 4 devices and width 8: device 1 owns columns 2 and 3. For one
position the residual holds `i / 8` in column `i`. In column 2 the devices
hold addends 1, 2, 3 and 4: device 1 computes `1 + 2 + 3 + 4 + 0.25 =
10.25`. In column 3 they hold 1, 2^53, -2^53 and 1:

```text
1 + 2^53           = 2^53      2^53 + 1 is halfway; it rounds to the even 2^53
2^53 + (-2^53)     = 0
0 + 1              = 1
1 + 0.375          = 1.375     the exact sum is 2.375
```

Device order gives 1.375 every time. Reverse device order gives other
bits: 1 + (-2^53) = -(2^53 - 1) exactly, then 1, then 2, and 2.375. The
order is part of the result, which is why the plan fixes it.

### Communication by hand

The all-reduce of one position turns a partial row of `width` values into
a replicated one. Chapter 133's `sd_cost` counts what it moves: the
reduce-scatter sends each column's owner `N - 1` addends, and the
all-gather sends every device the `N - 1` pieces it lacks, so
`2 * (N - 1) * width` values, 16 bytes per value per extra device:

```text
2 devices, width 8:   16 * 1 * 8   = 128 bytes an all-reduce
a token:              2 halves * 2 layers * 128 = 512 bytes
4 devices, width 8:   16 * 3 * 8   = 384 bytes, 1,536 bytes a token
```

The threads harness's larger decoder (width 128, 4 layers) moves
16 * 1 * 128 * 8 = 16,384 bytes a token at 2 devices, 49,152 at 4 and
114,688 at 8. A one-device run moves nothing.

## The run

`tl_admit` checks the plan, every buffer's size, the positions and the
tokens before anything is written. `tl_run` then embeds the tokens on
every device, runs each layer as attention, all-reduce, FFN, all-reduce,
and computes the logits on device 0.

<!-- source: psionic/lib/tensor-parallel-run.coax run -->
```jai
// Positions first..<first+tokens.count on every device, the earlier ones
// already in each device's cache. Each layer is the attention half on every
// device, an all-reduce, the FFN half, an all-reduce. Here the devices run
// one after another; the threads harness runs the same calls in parallel
// with a barrier between steps.
tl_run :: (p:TlPlan,shards:[]f64,works:[]f64,partials:[]f64,reduced:[]f64,tokens:[]int,first:int,logits:[]f64)->TlResult #export_c {
    status:=tl_admit(p,shards.count,works.count,partials.count,reduced.count,tokens,first,logits.count);if status!=TlStatus.ok {return TlResult.{status=status};}
    n:=tokens.count;
    for d:0..<p.devices {tl_embed(p,d,shards,works,tokens,first);}
    for l:0..<p.shape.layers {
        for d:0..<p.devices {tl_attend(p,d,l,shards,works,first,n,partials);}
        for d:0..<p.devices {tl_reduce(p,d,works,first,n,partials,reduced);}
        for d:0..<p.devices {tl_gather(p,d,works,first,n,reduced);}
        for d:0..<p.devices {tl_ffn(p,d,l,shards,works,first,n,partials);}
        for d:0..<p.devices {tl_reduce(p,d,works,first,n,partials,reduced);}
        for d:0..<p.devices {tl_gather(p,d,works,first,n,reduced);}
    }
    tl_head(p,shards,works,first,n,logits);
    return TlResult.{written=n*p.shape.vocab};
}
```

## Run it

The example, [split-model.coax](../examples/split-model.coax), splits
`TL_SPLIT` over 1, 2 and 4 devices, prints each plan,
chooses device counts for three budgets, and runs tokens 1, 3, 0, 2, 5, 4
on each split.

<!-- source: psionic/examples/split-model.coax example -->
```jai
// Chapter 183: one decoder split over 1, 2 and 4 logical devices. The plan
// says what each device holds and what each token sends; the run compares
// the split's logits with chapter 113's one-device forward and with the
// independent Python oracle. The devices run one after another on this CPU;
// psionic/tests/check_tensor_parallel.sh runs them as threads.
main :: () {
    s:=TL_SPLIT;
    print("decoder: vocabulary %, width %, % heads over % key-value heads, % FFN columns, % layers, context %; % parameters\n",
        s.vocab,s.width,s.heads,s.kv_heads,s.hidden,s.layers,s.context,td_parameter_count(s));
    for n:[3]int.[1,2,4] {p:=tl_plan(s,n,100000);
        print("% devices: %; each holds % heads, % key-value heads (shared: %), % FFN columns\n",n,tl_why(p.status),p.heads,p.kv_heads,p.shared,p.hidden);
        print("  % weight values + % cache values = % bytes a device; one all-reduce % bytes, % bytes a token\n",p.shard,p.cache,p.bytes,p.reduce,p.token);}
    for budget:[3]int.[8000,4200,3100] {c:=tl_choose(s,4,budget);print("budget % bytes a device: % devices, %, % bytes\n",budget,c.devices,tl_why(c.status),c.bytes);}

    tokens:=[6]int.[1,3,0,2,5,4];p:[776]f64;td_formula(p[..]);w:[2000]f64;ref:[36]f64;oracle:=TP_ORACLE_LOGITS;
    r:=td_forward(s,p[..],tokens[..],w[..],ref[..],false);
    print("chapter 113's forward on tokens 1 3 0 2 5 4: %, largest gap to the oracle %\n",r.status,tlt_gap(ref[..],oracle[..]));
    g:[6]int;for k:0..<6 {g[k]=td_argmax(ref[6*k..<6*k+6]);}print("  its greedy next tokens: % % % % % %\n",g[0],g[1],g[2],g[3],g[4],g[5]);
    for n:[3]int.[1,2,4] {x:TlBuffers;td_formula(x.p[..]);plan:=tl_plan(s,n,100000);st:=tl_split(plan,x.p[..],x.shards[..]);
        t:=tl_run(plan,x.shards[..],x.works[..],x.partials[..],x.reduced[..],tokens[..],0,x.logits[..]);
        greedy:=true;for k:0..<6 {if td_argmax(x.logits[6*k..<6*k+6])!=td_argmax(ref[6*k..<6*k+6]) {greedy=false;}}
        print("% devices: split %, run %, % logits; equal to one device bit for bit: %; largest gap % to one device, % to the oracle; greedy tokens agree: %\n",
            n,tl_why(st),tl_why(t.status),t.written,tlt_same(x.logits[0..<36],ref[..]),tlt_gap(x.logits[0..<36],ref[..]),tlt_gap(x.logits[0..<36],oracle[..]),greedy);}

}
```

```shell
coax/bin/coax run psionic/examples/split-model.coax
```

```text
decoder: vocabulary 6, width 8, 4 heads over 2 key-value heads, 8 FFN columns, 2 layers, context 8; 776 parameters
1 devices: admitted; each holds 4 heads, 2 key-value heads (shared: false), 8 FFN columns
  776 weight values + 128 cache values = 7232 bytes a device; one all-reduce 0 bytes, 0 bytes a token
2 devices: admitted; each holds 2 heads, 1 key-value heads (shared: false), 4 FFN columns
  456 weight values + 64 cache values = 4160 bytes a device; one all-reduce 128 bytes, 512 bytes a token
4 devices: admitted; each holds 1 heads, 1 key-value heads (shared: true), 2 FFN columns
  328 weight values + 64 cache values = 3136 bytes a device; one all-reduce 384 bytes, 1536 bytes a token
budget 8000 bytes a device: 1 devices, admitted, 7232 bytes
budget 4200 bytes a device: 2 devices, admitted, 4160 bytes
budget 3100 bytes a device: 4 devices, refused: a device's weights and cache exceed its budget, 3136 bytes
chapter 113's forward on tokens 1 3 0 2 5 4: ok, largest gap to the oracle 1.1449174941446927e-16
  its greedy next tokens: 0 0 0 0 3 0
1 devices: split admitted, run admitted, 36 logits; equal to one device bit for bit: true; largest gap 0.0 to one device, 1.1449174941446927e-16 to the oracle; greedy tokens agree: true
2 devices: split admitted, run admitted, 36 logits; equal to one device bit for bit: false; largest gap 2.220446049250313e-16 to one device, 1.6653345369377348e-16 to the oracle; greedy tokens agree: true
4 devices: split admitted, run admitted, 36 logits; equal to one device bit for bit: false; largest gap 1.1102230246251565e-16 to one device, 1.1449174941446927e-16 to the oracle; greedy tokens agree: true
```

At one device the split is chapter 113's forward bit for bit. At 2 and 4
devices the bits differ: one device sums a row of Wo over all 8 query
values in one run, while 2 devices sum two runs of 4 and add the two
results. The largest gap is 2.2e-16, one or two units in the last place.
The check is against a frozen tolerance, `TL_TOLERANCE = 1e-12`, the same
bound chapter 113 uses against its oracle, and the greedy token at every
position must agree.

## Checked against an independent definition

[decoder_oracle.py](../tests/decoder_oracle.py) defines chapter 113's
decoder in plain Python from the equations, not from the Coax code. Its
`--split` flag prints the logits for `TL_SPLIT`, which the checks hold as
`TP_ORACLE_LOGITS` and
[`check_tensor_parallel.sh`](../tests/check_tensor_parallel.sh) compares
with the script's output. The checks then require:

- at 1 device, `td_forward`'s logits bit for bit, for `TL_SPLIT` and
  for chapter 113's own `TD_TINY`, and within 1e-12 of both oracles;
- at 2 and 4 devices, within 1e-12 of `td_forward` and of the oracle,
  with the same greedy token at every position, and not bit for bit at 2
  devices, so the tolerance is shown to be needed;
- prefill in one call equals one position at a time, and three positions
  after three, bit for bit at 1, 2 and 4 devices;
- a whole context of 8 positions, decoded one at a time with each next
  token chosen greedily, gives the same tokens as `td_decode`;
- device `d`'s shard holds exactly its rows and columns, and at 4 devices
  devices 0 and 1 both hold key-value head 0.

## Refusals

[split-model-refuse.coax](../examples/split-model-refuse.coax) shows
splits the plan refuses, and runs the admission refuses. A refused run
writes nothing: logits, caches, addends and sums keep their values.

<!-- source: psionic/examples/split-model-refuse.coax refuse -->
```jai
// Chapter 183: splits the plan refuses, and runs the admission refuses. A
// refused run writes no logits, no cache, no addend and no sum.
main :: () {
    s:=TL_SPLIT;
    print("chapter 113's shape (3 FFN columns) on 2 devices: %\n",tl_why(tl_plan(TD_TINY,2,100000).status));
    print("4 heads on 3 devices: %\n",tl_why(tl_plan(s,3,100000).status));
    g:=TdShape.{vocab=6,width=12,heads=6,kv_heads=3,head_width=2,hidden=6,layers=1,context=4};
    print("6 heads over 3 key-value heads on 2 devices: %\n",tl_why(tl_plan(g,2,100000).status));
    print("0 devices: %\n",tl_why(tl_plan(s,0,100000).status));
    m:=tl_plan(s,2,4159);print("2 devices with 4159 bytes each: %, it needs % bytes\n",tl_why(m.status),m.bytes);
    c:=tl_choose(s,4,3000);print("the fewest of up to 4 devices with 3000 bytes each: %, % devices need % bytes each\n",tl_why(c.status),c.devices,c.bytes);

    x:TlBuffers;td_formula(x.p[..]);plan:=tl_plan(s,2,100000);_:=tl_split(plan,x.p[..],x.shards[..]);tokens:=[6]int.[1,3,0,2,5,4];
    for i:0..<48 {x.logits[i]=-7;}for i:0..<4*240 {x.works[i]=-7;}for i:0..<4*64 {x.partials[i]=-7;}for i:0..<64 {x.reduced[i]=-7;}
    r:=tl_run(plan,x.shards[..],x.works[0..<303],x.partials[..],x.reduced[..],tokens[..],0,x.logits[..]);
    print("work buffers of 303 values for 2 devices of 152: %\n",tl_why(r.status));
    r=tl_run(plan,x.shards[..],x.works[..],x.partials[..],x.reduced[..],tokens[..],3,x.logits[..]);
    print("6 positions from position 3 in a context of 8: %\n",tl_why(r.status));
    tokens[4]=6;r=tl_run(plan,x.shards[..],x.works[..],x.partials[..],x.reduced[..],tokens[..],0,x.logits[..]);
    print("token 6 in a vocabulary of 6: %\n",tl_why(r.status));
    untouched:=true;
    for i:0..<48 {if x.logits[i]!=-7 {untouched=false;}}for i:0..<4*240 {if x.works[i]!=-7 {untouched=false;}}
    for i:0..<4*64 {if x.partials[i]!=-7 {untouched=false;}}for i:0..<64 {if x.reduced[i]!=-7 {untouched=false;}}
    print("logits, caches, addends and sums unchanged: %\n",untouched);
}
```

```shell
coax/bin/coax run psionic/examples/split-model-refuse.coax
```

```text
chapter 113's shape (3 FFN columns) on 2 devices: refused: the devices do not divide the FFN columns
4 heads on 3 devices: refused: the devices do not divide the attention heads
6 heads over 3 key-value heads on 2 devices: refused: a device's heads would read part of a key-value head
0 devices: refused: from 1 to 8 devices
2 devices with 4159 bytes each: refused: a device's weights and cache exceed its budget, it needs 4160 bytes
the fewest of up to 4 devices with 3000 bytes each: refused: a device's weights and cache exceed its budget, 4 devices need 3136 bytes each
work buffers of 303 values for 2 devices of 152: refused: a buffer is smaller than the plan needs
6 positions from position 3 in a context of 8: refused: the positions are empty or past the context
token 6 in a vocabulary of 6: refused: a token is outside the vocabulary
logits, caches, addends and sums unchanged: true
```

The example exits with status 0; the book checks its transcript in debug
and release.

## On host threads

[`tensor_parallel_threads.c`](../tests/tensor_parallel_threads.c) runs
each device as a POSIX thread. It calls the same Coax functions in the
same order as `tl_run`, with a barrier after each attention half, each
FFN half, each reduce-scatter and the head: four barriers a layer. The C
file starts threads and times them; it decides nothing. The model is
chapter 113's family at vocabulary 256, width 128, 8 heads over 4
key-value heads, 256 FFN columns, 4 layers and context 64 (525,440
parameters from `td_init`), over 64 positions.

```shell
psionic/tests/check_tensor_parallel.sh
```

The script runs both files' checks in debug and release, the examples in
both modes, the oracle comparison, a short run under ThreadSanitizer, and
then the harness. At 1, 2, 4 and 8 devices it checks that the threads
give `tl_run`'s logits bit for bit, that decoding one position at a time
gives the prefill's logits bit for bit, that one device is `td_forward`
bit for bit, and that every split is within 1e-12 of it with the same
greedy tokens. The largest gaps were 3.1e-15 (2 devices), 3.3e-15 (4) and
2.9e-15 (8).

### A receipt

One run of `check_tensor_parallel.sh 20` on this host's 28-thread CPU,
20 prefills timed, while other work kept the load average between 31 and
33. Times vary from run to run, by a factor of two or more under that
load; this is a receipt, not a transcript.

```text
receipt: td_forward, one thread: 109.81 ms for 64 positions
receipt: 1 devices: tl_run in turn 52.58 ms; threads: prefill 52.20 ms (1.00x one device), decode 607.0 us a position (1.00x)
receipt: 2 devices: tl_run in turn 48.96 ms; threads: prefill 37.63 ms (1.39x one device), decode 685.4 us a position (0.89x)
receipt: 4 devices: tl_run in turn 53.99 ms; threads: prefill 31.49 ms (1.66x one device), decode 1046.9 us a position (0.58x)
receipt: 8 devices: tl_run in turn 43.52 ms; threads: prefill 28.76 ms (1.82x one device), decode 9918.4 us a position (0.06x)
```

Per device the memory falls from 4,465,664 bytes at 1 device to
2,499,584 at 2, 1,516,544 at 4 and 1,123,328 at 8; the replicated
embedding and head (65,664 values) are why it does not halve each time.
The prefill of 64 positions gets faster with more threads, but far from
linearly. Decoding one position gets slower with every added thread: each
position does about
2 MFLOP of work but crosses 17 barriers, and on a loaded CPU a barrier
that waits for a descheduled thread costs more than the arithmetic it
separates. Tensor parallelism pays when each device's share of a layer is
much larger than a synchronization, which is the case for large models on
GPUs joined by a fast link, not for this decoder on shared CPU cores.

## What remains

- **GPUs.** Every device here is a CPU thread. Running the halves as
  device kernels on several GPUs, the RTX 4080 as one member next to
  threads as chapter 132 does, and a real interconnect (NVLink, PCIe peer
  copies) with its measured all-reduce time are not built. This host has
  one GPU.
- **Expert parallelism.** Placing chapter 61's experts on different
  devices, moving routed tokens to them and back with an all-to-all,
  rebalancing redundant experts from a load window, and reporting balance
  as busiest over mean by time are not built.
- **A vocabulary-parallel head.** The embedding and head are replicated
  and device 0 computes the logits alone. Splitting them over devices
  needs a distributed softmax or argmax.
- **Pipeline stages.** Splitting the layers into stages, with chapter
  182's checked handoff between them, is not built.
- **Real models.** The splits run on chapter 113's decoder. Splitting a
  real checkpoint (gpt-oss, DeepSeek) and comparing it with the
  one-device run under chapter 17's frozen distribution checks on fixture
  prompts are not built, and neither is the latency measurement against
  one device that chapter 172 calls for.

## Dependencies

- [Chapter 113](ch113-differentiate-a-decoder.md): the decoder and its
  one-device forward.
- [Chapter 132](ch132-train-across-devices.md): logical devices as
  threads, and fixed-order reductions.
- [Chapter 133](ch133-a-tensor-spread-over-devices.md): sharded layouts,
  the matrix product rules and `sd_cost`.
- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): what fits on one device.
- [Chapter 61](ch061-a-few-experts-then-a-router.md),
  [chapter 17](ch017-qualify-honestly.md) and
  [chapter 172](ch172-benchmark-a-service-not-a-kernel.md): for the parts
  that remain.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [DeepSeek target](../docs/targets/deepseek.md)
- Shoeybi et al., *Megatron-LM: Training Multi-Billion Parameter Language
  Models Using Model Parallelism* (2019), for the column- and row-parallel
  split of a transformer block.
