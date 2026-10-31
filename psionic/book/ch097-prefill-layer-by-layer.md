# 97. Prefill layer by layer

Token-major prefill visits every layer for one token, then repeats for the
next. A routed expert used by several tokens is fetched and decoded repeatedly.
Layer-major prefill completes one layer for a chunk, groups its selected routes
by expert and processes each group together. The original route order still
determines each token's final sum.

This chapter adds `deepseek-prefill-cpu-wide-v1`: time/work-budget chunk choice,
a receipt-driven layer-major cursor, stable expert groups, grouped block-scaled
products, batched expert FFNs and canonical routed-plus-shared combination.
It executes an owned causal two-layer stack and an original layer-0 MoE batch
against the existing serial arithmetic. The grouped CUDA product primitive
executes with exact CPU parity on the RTX 4080, including the original routed
projection shapes. Full original-model prefill and its GPU integration remain
chapter 100 work.

## A chunk is an explicit budget decision

`DpChunkConfig` supplies measured fixed, per-token and per-expert costs in
integer nanoseconds, plus a maximum chunk and workspace allowance. Costs can
represent the full stack: the native fixture sums its two layers' expert-file
costs. The estimate for N tokens is
`fixed + N * token_cost + min(experts, N * selected) * expert_cost`.
The workspace allowance limits N independently, using a caller-supplied live
byte inventory per token after fixed reservations.

`dp_chunk` chooses the largest fitting count. Grouped mode requires a maximum
of at least two. If time or memory only permits one token while more remain,
it refuses with a budget error. Only the final one-token tail or explicitly
requested serial mode uses a singleton. A configuration cannot silently turn
the grouped path off.

<!-- source: psionic/lib/deepseek/prefill-plan.coax chunk -->
```jai
dp_chunk :: (c:DpChunkConfig,remaining:int)->DpChunk #export_c {
    if !dp_batch(c.max_chunk) || (!c.serial && c.max_chunk==1) || !dp_positive_cost(c.bytes_per_token) || !dp_nonnegative(c.fixed_ns) || !dp_positive_cost(c.token_ns) || !dp_nonnegative(c.expert_ns) || !dp_positive_cost(c.target_ns) || !dp_memory(c.workspace) || !dp_expert_k(c.experts,c.k) || !dp_prefix(remaining) || remaining==0 {return DpChunk.{status=DsStatus.shape};}
    cap:=dp_min(c.max_chunk,remaining);cap=dp_min(cap,c.workspace/c.bytes_per_token);if c.serial {cap=dp_min(cap,1);}
    low:=0;high:=cap+1;while low+1<high {middle:=(low+high)/2;if dp_cost(c,middle)<=c.target_ns {low=middle;}else {high=middle;}}
    if low==0 || (!c.serial && remaining>1 && low<2) {return DpChunk.{status=DsStatus.budget};}
    return DpChunk.{count=low,predicted_ns=dp_cost(c,low),tail=remaining==1,serial=c.serial};
}
```

This is a calibrated estimate, not a hard real-time deadline. The harness
records actual chunk duration alongside the prediction; small chunks can
exceed it because of estimation error or concurrent work. A deployed engine
must maintain its calibration and account for attention/context length, I/O,
fixed work, model/device memory and scheduling overhead. This component does
not replace chapter 175's scheduler or assert a whole-model latency bound.

## Run each layer before advancing

`dp_start` creates an explicit chunk cursor. Its first/count, layer, expert
list, group ordinal, phase and epoch identify the next action. The action
sequence is attention, route, each active expert, shared expert and combine;
then the next layer begins. Only after the last layer's combine does the
cursor permit the chunk commit. A matching success receipt is required at
every transition. Wrong epoch, layer, phase, group or failure receipts refuse.
Completed cursors cannot advance again.

The caller supplies the actual attention and router outputs and keeps their
model/history identity coherent. Borrowed group keys stay immutable through
the layer. Intermediate hidden rows, contributions and cache state remain
private until commit. The cursor orders operations; complete model storage,
fencing, cancellation and publication remain the engine's responsibility.
The native fixture keeps a complete private cache shadow and checks failure
before both the first publication and continuation after an earlier chunk.

## Group routes without changing their meaning

Flatten selected IDs and weights in token/route order. `dp_groups` constructs
stable expert buckets, prefix offsets and ascending active expert keys. The
bucket stores each assignment's original flattened ordinal. Equal IDs from
different tokens remain distinct assignments; duplicate IDs within one token
are refused. Invalid routes or insufficient independent spans publish none of
the grouping metadata. Dirty work and spare tails are tested explicitly.

<!-- source: psionic/lib/deepseek/prefill-groups.coax group -->
```jai
dp_groups :: (experts:int,batch:int,k:int,ids:[]int,weights:[]f32,order:[]int,offsets:[]int,keys:[]int,order_stage:[]int,offset_stage:[]int,key_stage:[]int,overlap:bool)->DpResult #export_c {
    status:=dp_routes(experts,batch,k,ids,weights);if status!=DsStatus.ok {return DpResult.{status=status};}n:=batch*k;
    if order.count<n || offsets.count<experts+1 || keys.count<experts || order_stage.count<n || offset_stage.count<experts+1 || key_stage.count<experts {return DpResult.{status=DsStatus.capacity};}if overlap {return DpResult.{status=DsStatus.alias};}
    for i:0..<experts+1 {offset_stage[i]=0;}for id:ids {offset_stage[id+1]+=1;}for e:1..<experts {offset_stage[e+1]+=offset_stage[e];}
    cursors:[384]int;groups:=0;for e:0..<experts {cursors[e]=offset_stage[e];if offset_stage[e+1]>offset_stage[e] {key_stage[groups]=e;groups+=1;}}
    for a:0..<n {e:=ids[a];order_stage[cursors[e]]=a;cursors[e]+=1;}
    for i:0..<n {order[i]=order_stage[i];}for i:0..<experts+1 {offsets[i]=offset_stage[i];}for i:0..<groups {keys[i]=key_stage[i];}return DpResult.{written=n,groups=groups};
}
```

For each active expert, the engine fetches its packet once, gathers all input
rows and their own route weights, executes one batched expert, then scatters
results back to the original assignment ordinals. Packet effects reuse chapter
96's admitted fixed-stride file path. A packet retains the original six FP4
code/scale spans losslessly. An unselected expert is not fetched or evaluated.
The shared FP8 expert runs once for the chunk, with one output per token.

`dp_combine` requires completion receipts for every selected expert and the
shared result. It sums routed results in each token's original route order,
using the same compensated binary64 reduction as `de_mixture`, narrows that
routed sum to F32, then adds the shared result once with the original narrowing.
Reordering the sum by expert or moving shared addition before the routed cast
would change results. Missing receipts, nonfinite results or late overflow
preserve the whole public output and spare tail.

## Reuse coefficients across eight rows

The shared C/CUDA product kernel handles one matrix row and up to eight input
rows per work item. Each coefficient decodes once for that tile. For every
output, products still enter the error-free TwoSum/correction recurrence in
increasing column order. Interleaving tokens does not reorder any one output's
reduction. The final value narrows once to F32. FP4 keeps per-row/per-32 scales;
FP8 keeps per-32-by-32 scales. Invalid codes/scales and output overflow produce
private flags instead of a public partial result.

<!-- source: psionic/lib/deepseek/prefill-kernels.coax product -->
```jai
dpkg_project :: (item:int,kind:int,rows:int,columns:int,batch:int,data:[]u8,scales:[]u8,input:[]f64,out:[]f32,flags:[]u8) #export_c {
    row:=item%rows;first:=(item/rows)*8;sums:[8]f64;errors:[8]f64;bad:u8=0;
    for col:0..<columns {at:=row*columns+col;
        code:=data[ifx kind==0 then at/2 else at];scale:=scales[ifx kind==0 then at/32 else (row/32)*(columns/32)+col/32];
        if kind==0 {code=(code>>cast(u8)(4*(at%2)))&15;}else if (code&127)==127 {bad=1;}
        if scale==255 {bad=1;}weight:=ifx kind==0 then dpkg_fp4(code,scale) else dpkg_fp8(code,scale);
        for t:0..<8 {token:=first+t;if token<batch {x:=input[token*columns+col];product:=weight*x;total:=sums[t]+product;
            virtual_value:=total-sums[t];error:=(sums[t]-(total-virtual_value))+(product-virtual_value);errors[t]+=error;sums[t]=total;}}
    }
    for t:0..<8 {token:=first+t;if token<batch {at:=token*rows+row;total:=sums[t]+errors[t];flag:=bad;if !dpkg_fit(total) && flag==0 {flag=2;}flags[at]=flag;if flag==0 {out[at]=f32(total);}}}
}
```

CPU `dp_project` first quantizes each complete 32-value group using chapter 95's
same FP8 activation grid. A row-aligned flattened batch has exactly the same
groups as serial token calls. `dp_expert` batches gate and up, applies the same
clamp/SiLU and each row's route weight before down-input quantization, then
batches down. All final rows stage before publication. Work plans preserve the
different width/hidden roles and expose exact private view extents.

<!-- source: psionic/lib/deepseek/prefill-products.coax expert -->
```jai
dp_expert :: (e:DeExpert,batch:int,x:[]f32,routes:[]f32,limit:f32,out:[]f32,work:DpWork,overlap:bool)->DpResult #export_c {
    p:=dp_plan(e,batch);if p.status!=DsStatus.ok {return DpResult.{status=p.status};}n:=batch*p.width;
    if x.count!=n || routes.count!=batch || out.count<n || work.floats.count<p.floats || work.wide.count<p.wide || work.flags.count<p.flags {return DpResult.{status=DsStatus.capacity};}if overlap {return DpResult.{status=DsStatus.alias};}
    if !de_control(limit) {return DpResult.{status=DsStatus.number};}for route:routes {if !de_control(route) {return DpResult.{status=DsStatus.number};}}
    v:=dp_views(work.floats,p);
    g:=dp_project(e.gate,batch,x,v.gate,work.wide,v.stage,work.flags,false);if g.status!=DsStatus.ok {return g;}
    u:=dp_project(e.up,batch,x,v.up,work.wide,v.stage,work.flags,false);if u.status!=DsStatus.ok {return u;}
    r:DpResult;for token:0..<batch {for j:0..<p.hidden {at:=token*p.hidden+j;lane:=de_activation(v.gate[at],v.up[at],routes[token],limit);if lane.status!=DsStatus.ok {return DpResult.{status=lane.status};}v.activation[at]=lane.value;r.gate_clamped+=lane.gate_clamped;r.up_clamped+=lane.up_clamped;}}
    d:=dp_project(e.down,batch,v.activation,v.pending,work.wide,v.stage,work.flags,false);if d.status!=DsStatus.ok {return d;}
    for i:0..<n {out[i]=v.pending[i];}r.written=n;r.coefficients=g.coefficients+u.coefficients+d.coefficients;r.products=g.products+u.products+d.products;return r;
}
```

For N rows the coefficient-visit count is `ceil(N/8) * matrix_values`, instead
of `N * matrix_values`; the product count is unchanged. These are logical
visits, not measured DRAM traffic. The CPU library allocates no runtime heap.
Its expert work is N times `3*hidden + width + max(width,hidden)` F32 values,
`max(width,hidden)` F64 quantized values and that many byte flags. Group input,
route/contribution arrays, combine work, cache shadows, file packets and model
state add separate storage. The workspace setting must include the caller's
whole live inventory; the resident-weight budget does not cover it.

The CUDA implementation separates private product/flag generation, one flag
reduction and guarded publication. Its compiler options disable FMA and retain
precise division/sqrt and subnormals. NVRTC compiles the generated source for
compute_89. Actual RTX 4080 execution matches CPU output and flag words exactly
for FP4/FP8 batches 1, 9 and 17, invalid scales and overflow, and all three
original routed projection shapes at batch 9. This qualifies the product
primitive. Activation/quantization, complete GPU expert execution and device
performance remain separate integration work.

## Run and check it

The [example](../examples/deepseek-prefill.coax) chooses an estimated chunk,
groups routes, walks a complete two-layer cursor and executes a grouped expert.
Its illustrative nanosecond constants are not a hardware calibration.

```shell
coax/bin/coax run psionic/examples/deepseek-prefill.coax
coax/bin/coax run psionic/examples/deepseek-prefill.coax --release
psionic/tests/check_deepseek_prefill.sh
psionic/check.sh --only deepseek/prefill --jobs 8
psionic/tests/run_deepseek_prefill_cuda.sh --compile-only
# Requires a CUDA context and enough free VRAM; fails rather than silently skipping.
psionic/tests/run_deepseek_prefill_cuda.sh
```

<!-- source: psionic/examples/deepseek-prefill.coax example -->
```jai
main :: () {
    config:=DpChunkConfig.{max_chunk=8,workspace=32768,bytes_per_token=4096,fixed_ns=1000,token_ns=100,expert_ns=50,experts=3,k=2,target_ns=1750};
    chunk:=dp_chunk(config,13);#assert chunk.status==DsStatus.ok && chunk.count==6 && chunk.predicted_ns==1750;
    print("prefill chunk: % tokens, % ns predicted\n",chunk.count,chunk.predicted_ns);
    ids:=[8]int.[2,0,1,2,0,1,2,1];weights:=[8]f32.[0.2f32,0.8f32,0.3f32,0.7f32,0.4f32,0.6f32,0.5f32,0.5f32];order:[8]int;offsets:[4]int;keys:[3]int;so:[8]int;sf:[4]int;sk:[3]int;
    grouped:=dp_groups(3,4,2,ids[..],weights[..],order[..],offsets[..],keys[..],so[..],sf[..],sk[..],false);#assert grouped.status==DsStatus.ok;
    print("layer routes: % uses grouped into % expert fetches\n",grouped.written,grouped.groups);
    cursor:=dp_start(0,4,2,3,2,0).value;empty:[1]int;steps:=0;
    while cursor.phase!=DpPhase.done {receipt:=DpReceipt.{epoch=cursor.epoch,layer=cursor.layer,phase=cursor.phase,group=cursor.group,ok=true};selected:=empty[0..<0];if cursor.phase==DpPhase.route {selected=keys[..];}
        advanced:=dp_advance(cursor,receipt,selected);#assert advanced.status==DsStatus.ok;cursor=advanced.value;steps+=1;}
    print("two layers: % receipts before epoch % completes\n",steps,cursor.epoch);
    data:[3264]u8;e:=de_fixture_0(data[..]);x:[192]f32;for t:0..<3 {for j:0..<64 {x[t*64+j]=DEG_INPUT[j]+f32(t)/16;}}routes:=[3]f32.[0.137f32,0.75f32,0];out:[192]f32;work_f:[672]f32;work_q:[192]f64;flags:[192]u8;
    r:=dp_expert(e,3,x[..],routes[..],10,out[..],DpWork.{floats=work_f[..],wide=work_q[..],flags=flags[..]},false);#assert r.status==DsStatus.ok;
    print("grouped expert: % outputs, % coefficient decodes for % products\n",r.written,r.coefficients,r.products);
    config.target_ns=1000;#assert dp_chunk(config,13).status==DsStatus.budget;
    print("insufficient grouped budget refuses; serial mode is explicit\n");
}
```

The independent scheduling oracle sorts bounded fixture-gate priorities and
constructs stable assignment buckets without using the implementation's count/
prefix/cursor algorithm. Native checks compare every group, key, offset,
assignment, route and weight across chunks 2, 4, 7 and 13. All 832 final words
and the entire causal cache equal token-major execution bit for bit. The
attention fixture is an owned causal prefix mean with a residual, not V4.1's
learned attention. It makes cross-token and cross-layer dependencies real while
keeping this scheduling test bounded.

The two-layer fixture performs 33, 22, 12 and 6 routed packet fetches for those
chunk sizes, versus 52 token-major routed uses. Each selected expert is fetched
once per layer/chunk. Late final-layer failure preserves both an empty public
prefix and a previously committed prefix. Actual aliases and publication tails
are covered in debug/release plain and ASan/UBSan builds.

The original component uses all 384 routed descriptors, selects six by the
original gate and includes the shared FP8 expert. Four identical retained
reference input rows yield **20,480 bit-identical output words**. Six routed
packets serve 24 routed uses. Grouping reduces logical coefficient decodes from
990,904,320 to 247,726,080. This is one original layer/input point; complete
original-model prefill, original GPU distributions and end-to-end performance
are still separate requirements. Source identities, exact records, observed
CPU timings and exact CUDA parity evidence are in the
[verification record](../../docs/verification/psionic-ch65-2026-10-29.md).
