# 64. Hot experts in fast memory

A cache may change where an expert's bytes live, but it must not change the
expert, its route weight or its arithmetic. This chapter builds the CPU
reference core of `deepseek-hot-exact-cpu-v1`: recorded-profile seeding,
decayed-frequency heat, deterministic replacements, private next-layer
prefetch, mapped/direct file misses and an atomic installation between steps.
The packed coefficients execute through chapter 63's existing expert path.

The resident arena here is CPU storage. This qualifies placement and lifetime
rules and exact CPU results. Device arenas, GPU expert execution, event fences,
overlap and measured acceleration remain integration work in chapters 67,
68 and 68a. A passing CPU cache is not evidence of GPU throughput or a
complete one-machine model.

## Count uses, then choose residency

The caller supplies a model/packet identity, a layer count, experts and slots
per layer, packet stride and a resident-byte budget. `dc_seed` admits a recorded
per-expert use-count profile and initializes its heat. Every slot starts empty:
a profile cannot announce a hit before its coefficient copy succeeds. The
first `dc_plan` chooses the hottest IDs for those empty slots. Equal heat picks
the lower expert ID.

Heat has 1,024 integer units per recorded use. A finished step adds its unique
selected keys, then every 256th finished step multiplies all heat by 9/10,
rounding down. A nonresident expert replaces a populated slot only when
`4 * incoming_heat > 5 * cold_heat + 4096`: more than 1.25 times its heat plus
one use. Equality keeps the old expert. This deterministic fixed-point policy
is pinned in [hot-policy.json](../fixtures/deepseek/hot-policy.json).

The design follows the recorded [pre-reset single-box work](https://github.com/OpenAgentsInc/openagents/blob/853417f290e7cae1df18c081b89b0818432e60af/docs/psionic/v41-single-box.md).
Our integer decay can choose different placements from that engine's floating
heat. It does not change coefficient bits or the selected model computation.
No old Rust workspace or upstream runtime is restored or executed.

At each layer, a plan pairs the coldest eligible slots with hottest original
outsiders. Empty slots come first; tied victims use the lowest physical slot.
A slot changes at most once per plan, and evicted IDs wait until the next plan
before reconsideration. A bounded presence set makes selection proportional
to `slots * (experts + slots)` per layer. Frozen caches may fill empty slots
but never replace a populated slot.

## Keep the slot map stable during a step

`DcState` borrows heat and slot spans. Its value header carries model identity,
finished-step number, residency revision and an active-step flag. The caller
owns one linear continuation and exclusive writes; headers are not locks.
Identity is a comparison against the caller's trusted model/format identity,
not authentication of arbitrary files or packets.

| Operation | Required phase | What becomes public |
| --- | --- | --- |
| `dc_seed` | Before serving | Admitted initial heat, empty slots and initial header |
| `dc_plan` | Between steps | Proposed slot map, copy moves and a step/revision-bound ticket |
| `dc_install` | Between steps | All copied coefficient bytes, new slot map and next revision |
| `dc_begin` | Between steps | Returned active header; no slots or heat change |
| `dc_dispatch` / `dc_prefetch` | Active step | Exact route locations or private miss keys |
| `dc_finish` | Active step | Complete next heat and returned idle header at the next step |

A plan includes source keys, destination slots and expected old occupants.
Installation rejects a stale step, revision or model identity, cross-layer
moves, repeated slots, substituted proposals and any incomplete copy receipt.
All validation finishes before copying public bytes or metadata. A late bad
receipt preserves both in full. Tickets must be successful unmodified plans;
ready receipts must describe completed copies of those same immutable packets.

During an active step, routing observes a stable map. A hit returns its global
slot; a miss returns slot -1 and the same global expert key. The cache never
changes IDs or weights selected by chapter 59's router. `dc_finish` consumes
the recorded selected keys, validates them all and stages the entire next
heat vector. Invalid keys, duplicate uses or heat overflow preserve public heat
and return no next continuation. The engine owns request cancellation,
concurrent request fencing and complete model-step publication.

<!-- source: psionic/lib/deepseek/hot-swap.coax plan -->
```jai
dc_plan :: (c:DcConfig,state:DcState,proposed:[]int,moves:[]DcMove,slots_work:[]int,moves_work:[]DcMove,overlap:bool)->DcPlan #export_c {
    status:=dc_state(c,state);if status!=DsStatus.ok {return DcPlan.{status=status};}if state.head.busy || state.head.revision==1048576 {return DcPlan.{status=DsStatus.state};}
    k:=c.layers*c.slots;if proposed.count<k || moves.count<k || slots_work.count<k || moves_work.count<k {return DcPlan.{status=DsStatus.capacity};}if overlap {return DcPlan.{status=DsStatus.alias};}
    for i:0..<k {slots_work[i]=state.slots[i];}count:=0;
    for layer:0..<c.layers {excluded:[384]bool;for j:0..<c.slots {id:=slots_work[layer*c.slots+j];if id>=0 {excluded[id]=true;}}for round:0..<c.slots {
        cold:=dc_cold_slot(c,state,slots_work,layer);
        hot:=-1;for id:0..<c.experts {if !excluded[id] {if hot<0 || state.heat[layer*c.experts+id]>state.heat[layer*c.experts+hot] {hot=id;}}}
        if hot<0 {break;}old:=slots_work[cold];if old>=0 {if c.frozen || !dc_replace(state.heat[layer*c.experts+hot],state.heat[layer*c.experts+old]) {break;}}
        excluded[hot]=true;slots_work[cold]=hot;moves_work[count]=DcMove.{slot=cold,old=old,key=layer*c.experts+hot};count+=1;
    }}
    for i:0..<k {proposed[i]=slots_work[i];}for i:0..<count {moves[i]=moves_work[i];}
    return DcPlan.{ticket=DcTicket.{step=state.head.step,revision=state.head.revision,identity=c.identity,count=count}};
}
```

## Prefetch next-layer misses without installing them

Given the next layer's gate-selected IDs, `dc_prefetch` returns only its misses
in route order. It reuses the exact dispatch check and leaves heat and
residency unchanged. The current step can fetch those immutable packets into
private buffers while retaining its lease. Prefetch is a hint about storage;
a missed hint still runs the requested expert from the host/file source.
There is no approximate gate or guessed replacement in this API.

`psionic_dc_fetch` supports lazy read-only mapping and `pread`. Coax admits
64-bit source extents and exact packet offsets before C performs I/O. Every
requested packet first enters private work; public packet output publishes
only after every read succeeds. Zero misses perform no file operation. A file
length mismatch, failed read or bad mapping returns a weights failure with
public bytes unchanged. Files must remain immutable through observation,
reading and mapping.

The source is an admitted packet artifact, with a fixed stride per global
layer/expert key. A packet concatenates gate codes/scales, up codes/scales and
down codes/scales. Repacking these six original spans is lossless: codes and
scales are copied exactly, not requantized. Binding checks width, hidden size
and exact packet extent, then returns borrowed `DeExpert` views. The original
5,120-by-2,304 routed expert occupies **18,800,640 bytes** in this layout.

<!-- source: psionic/lib/deepseek/hot-weights.coax bind -->
```jai
dc_bind :: (width:int,hidden:int,packet:[]u8)->DcBound #export_c {
    p:=dc_packet(width,hidden);if p.status!=DsStatus.ok {return DcBound.{status=p.status};}if packet.count!=p.bytes {return DcBound.{status=DsStatus.capacity};}
    matrices:[3]DeMatrix;for i:0..<3 {matrices[i]=DeMatrix.{kind=DeKind.fp4,rows=ifx i==2 then width else hidden,columns=ifx i==2 then hidden else width,data=packet[p.begin[2*i]..<p.begin[2*i+1]],scales=packet[p.begin[2*i+1]..<p.begin[2*i+2]]};}
    return DcBound.{value=DeExpert.{gate=matrices[0],up=matrices[1],down=matrices[2]}};
}
```

`psionic_dc_expert` observes the packet/input/output/work addresses, binds that
packet and calls the existing `de_expert`. Thus a resident hit, a host packet
and a file miss have identical matrix roles, activation grids, route weighting
and compensated CPU reductions. It does not sum a shared expert again or
change the model's selected-expert combine order. Whole-mixture/model wiring
must preserve those chapter 63 rules.

## Budget persistent and private bytes

For L layers, S slots per layer and packet stride P, persistent coefficient
storage is `L * S * P`; one byte below that configured budget refuses. The
library does not silently reduce slot count. At the original stride, 40 layers
with 116 slots require 87,234,969,600 bytes of coefficient storage alone.
That is a logical count, not a device-fit measurement.

Installation needs `moves * P` completed private bytes. Fetching them needs
another packet-work span of that size and one int64 offset per requested
packet. Those temporary allocations are separate from the resident budget.
Layer-local transactions can bound a caller's staging scope. Heat, slot maps,
plans, outputs, model activations, host/file storage, page cache and device
alignment are also outside the coefficient budget and must be accounted by
the engine. File requests may cause larger OS page reads and readahead.

All Coax work is borrowed caller storage. Selection uses a local 384-entry
presence set and installation a 16,512-entry destination set; prefetch has two local eight-route arrays. The C fetch adapter
uses a bounded 16,512-entry offset draft (132,096 bytes) on its stack. Coax owns
all numeric, range, placement and publication decisions. C observes native
addresses and file lengths and performs read/mapping effects only.

## Run and check it

The [example](../examples/deepseek-hot.coax) warms a profile-selected slot,
compares all expert output bits, refuses an active-step replacement and then
installs a hotter expert after a recorded change in routes.

```shell
coax/bin/coax run psionic/examples/deepseek-hot.coax
coax/bin/coax run psionic/examples/deepseek-hot.coax --release
psionic/tests/check_deepseek_hot.sh
psionic/check.sh --only deepseek/hot --jobs 8
```

<!-- source: psionic/examples/deepseek-hot.coax example -->
```jai
main :: () {
    c:=DcConfig.{layers=1,experts=3,slots=1,stride=3264,budget=3264,identity="owned-model"};
    profile:=[3]int.[10,1,0];heat:[3]int;other:[3]int;hw:[3]int;slots:[1]int;sw:[1]int;
    init:=dc_seed(c,profile[..],heat[..],slots[..],hw[..],sw[..],false);#assert init.status==DsStatus.ok && slots[0]==-1;
    state:=DcState.{head=init.head,heat=heat[..],slots=slots[..]};next:[1]int;moves:[1]DcMove;mw:[1]DcMove;
    p:=dc_plan(c,state,next[..],moves[..],sw[..],mw[..],false);#assert p.status==DsStatus.ok && moves[0].key==0;
    pending:[3264]u8;resident:[3264]u8;owned:=de_fixture_0(pending[..]);ready:=[1]u8.[1];
    installed:=dc_install(c,state,p.ticket,next[..],moves[..],ready[..],pending[..],resident[..],false);#assert installed.status==DsStatus.ok;state.head=installed.head;
    print("profile seed: expert % occupies one % byte slot\n",slots[0],c.stride);
    begin:=dc_begin(c,state);#assert begin.status==DsStatus.ok;state.head=begin.head;
    ids:=[2]int.[0,2];lanes:[2]DcLane;draft:[2]DcLane;dispatched:=dc_dispatch(c,state,0,ids[..],lanes[..],draft[..],false);#assert dispatched.status==DsStatus.ok && lanes[0].slot==0 && lanes[1].slot==-1;
    misses:[2]int;work:[2]int;prefetched:=dc_prefetch(c,state,0,ids[..],misses[..],work[..],false);#assert prefetched.status==DsStatus.ok && prefetched.written==1 && misses[0]==2;
    #assert dc_plan(c,state,next[..],moves[..],sw[..],mw[..],false).status==DsStatus.state;
    input:=DEG_INPUT;direct:[64]f32;cached:[64]f32;floats:[224]f32;wide:[64]f64;w:=DeWork.{floats=floats[..],wide=wide[..]};
    #assert de_expert(owned,input[..],0.137f32,10,direct[..],w,false).status==DsStatus.ok;
    bound:=dc_bind(64,32,resident[..]);#assert bound.status==DsStatus.ok;
    #assert de_expert(bound.value,input[..],0.137f32,10,cached[..],w,false).status==DsStatus.ok;
    for i:0..<64 {#assert f32_bits(direct[i])==f32_bits(cached[i]);}
    print("leased step: one hit, one prefetched miss; % exact output words\n",64);
    finish:=dc_finish(c,state,ids[..],other[..],hw[..],false);#assert finish.status==DsStatus.ok;state.head=finish.head;state.heat=other[..];
    // A phase change heats expert 2 through actual recorded uses.
    used:=[1]int.[2];for step:0..<20 {b:=dc_begin(c,state);#assert b.status==DsStatus.ok;state.head=b.head;
        f:=dc_finish(c,state,used[..],heat[..],hw[..],false);#assert f.status==DsStatus.ok;state.head=f.head;for i:0..<3 {other[i]=heat[i];}state.heat=other[..];}
    p=dc_plan(c,state,next[..],moves[..],sw[..],mw[..],false);#assert p.status==DsStatus.ok && p.ticket.count==1 && moves[0].key==2;
    replacement:=de_fixture_2(pending[..]);#assert replacement.gate.rows==32;
    installed=dc_install(c,state,p.ticket,next[..],moves[..],ready[..],pending[..],resident[..],false);#assert installed.status==DsStatus.ok && slots[0]==2;
    print("between steps: expert % replaces % at step %\n",slots[0],moves[0].old,installed.head.step);
}
```

An independent oracle derives the seed counts from an owned calibration-route
record, ranks candidates and old slots by sorting rather than the implementation's
search, and retains every heat vector, slot map, hit and move over 300 steps.
The workload changes its hot experts twice and crosses a decay boundary.
Native checks compare all of that state and 1,800 expert results bit for bit
across resident, mapped and direct-read paths. Coax and native checks cover zero slots,
complete residency, frozen replacements, exact capacities, real aliases,
incomplete receipts, stale plans, bad files and late invalid coefficients.

A separate native harness checks all 43 layers, 384 experts and 116 slots
per layer, including the final expert/slot and stale/active fencing, using
owned one-byte packets. That is metadata topology evidence, not allocation
of the original coefficient inventory.

An original layer-0 expert's six spans were packed from the chapter-63 admitted
shard. All 18,800,640 bytes survive file staging and installation unchanged;
all 5,120 output words match the already-recorded independent original
reference through the host, direct-miss and mapped-warmed resident paths.
The existing full-shard admission and original reference are reused. Routine
checks replay that evidence without original weights or an upstream runtime.

Executed modes, identities and remaining device integration are recorded in the
[chapter verification](../../docs/verification/psionic-ch64-2026-10-29.md).
