# 74. Fit 120b on the right device

> **Status: partly implemented.** The device-choice planner runs (profile
> fc-120b-v1, an `#authority` file): for each device it decides resident,
> dense-resident with streamed experts, or refused, with exact bytes, and
> picks the admitted device with the best worst-case decode rate. Its
> inputs are measured: chapter 72's two 20b decode runs and this chapter's
> expert upload receipt from the RTX 4080. Still to do: the 120b
> generation runs themselves. The original 120b shards are not on this
> host (65.2 GB needed, about 35 GB of disk free). Chapter 66's CUDA engine
> has no expert cache yet, and chapter 72's Metal harness has fixed
> 20b buffer sizes. The greedy-token comparison with chapter 73's
> references, measured decode and prefill rates, and the Mac run (recorded in
> `NEEDS_OWNER.md`) wait on those.

[Previous: gpt-oss-120b](ch073-execute-gpt-oss-120b.md) · [Contents](SUMMARY.md) ·
[Next: speculative decoding](ch075-let-the-small-model-guess.md)

gpt-oss-120b's original files hold 65,248,815,744 weight bytes. The RTX 4080
has 16 GB. The Apple M5 Max of [chapter 72](ch072-gpt-oss-on-metal.md) has
128 GB of unified memory. Before any engine allocates anything, this chapter
answers two questions with exact arithmetic: can a device run 120b, and how?
Which admitted device is best?

The planner is [`device-choice.coax`](../lib/gptoss/device-choice.coax). It
is an `#authority` file because it decides admission. It reuses
[chapter 33](ch033-measure-the-work-and-plan-the-memory.md)'s byte accounting
(`rp_weights`, `rp_state`, `rp_scratch`, `rp_decode`) unchanged.

```shell
coax/bin/coax run psionic/examples/fit-120b.coax
coax/bin/coax run psionic/examples/fit-120b-refuse.coax
```

## Three ways a device can run 120b

Most of 120b is experts, and a token uses only 4 of each layer's 128. That
leaves three cases:

- **Resident.** Every weight, the cache state and the scratch fit on the
  device, and the weights fit in one buffer (chapter 72's engine keeps them
  in one). Nothing crosses a link while it runs.
- **Experts streamed.** The device holds the dense weights (attention,
  router, norms, embedding and head), the state, the scratch, and an expert
  cache. The host pins every expert. In the worst case a decode step misses
  every selected expert and copies it over the link first. The cache must
  hold at least two layers' selections, 8 experts: one layer's in use while
  the next layer's are copied.
- **Refused**, with the first reason that applies: the state and scratch
  alone do not fit; memory is unified, so nothing can stream; the dense part
  does not fit; or the host cannot pin every expert.

A device is described by five numbers. Two are allocation limits (bytes the
engine may allocate, largest single buffer). Two are rates (how fast the
engine reads resident bytes, how fast bytes cross the host link, 0 for
unified memory). The last is how many host bytes may be pinned.

## Work it by hand first

One original expert has an up-and-gate matrix of 2 × 2,880 × 2,880 values and
a down matrix of 2,880 × 2,880, stored as MXFP4: half a byte a value and one
scale byte per 32 values, plus BF16 biases:

```text
up-and-gate  16,588,800 values: 8,294,400 + 518,400 bytes
down          8,294,400 values: 4,147,200 + 259,200 bytes
biases        2 * (5,760 + 2,880)  =    17,280 bytes
one expert                         = 13,236,480 bytes
all experts   36 * 128 * 13,236,480 = 60,993,699,840 bytes
dense         65,248,815,744 - 60,993,699,840 = 4,255,115,904 bytes
```

For a 4,096-token cache with prompt chunks of 256, chapter 33 gives the
state and scratch:

```text
state    4 * 1,024 * 18 * (4,096 + 128)               = 311,427,072
scratch  4 * (256 * (5,760 + 5,888) + 64 * 4,096 + 201,088) = 13,780,480
live     state + scratch                               = 325,207,552
```

A decode step at position 4,096 reads every weight except the embedding
table and the 124 unused experts of each layer, plus the cache:

```text
65,248,815,744 - 1,158,266,880 - 36 * 124 * 13,236,480 = 5,002,902,144
+ the residual row, 5,760, + keys and values, 18 * 4,096 * (4,097 + 129)
= 5,314,482,432 bytes a step
```

The rates come from measurements, not data sheets. Chapter 71's workload
times 128 decode steps of 20b from position 1,025. Its median was
3,683,000,000 ns on the Mac (chapter 72's run) and 2,279,540,283 ns on the
4080 (chapter 71's receipt, five runs). Chapter 33 counts 482,301,542,400 bytes for those 128 steps, so
the engine's effective read rate is 130,953 bytes a microsecond on the Mac
and 211,578 on the 4080. The link rate is this chapter's receipt (below):
23,826 bytes a microsecond.

**The Mac.** It may allocate its recommended working set, 107.5 GiB =
115,427,246,080 bytes, and its largest buffer is 80.6 GiB. 65,248,815,744 +
325,207,552 = 65,574,023,296 fits, and the weights fit in one buffer, so the
Mac runs 120b resident. A step takes 5,314,482,432 / 130,953 = 40,583.1,
rounded up to 40,584 µs, which is 24.640 tokens/s.

**The 4080.** 16,302,145,536 bytes were free, less chapter 45's 256 MiB
reserve: 16,033,710,080. All of 120b does not fit. The dense part, state,
scratch and 8 experts take 4,686,215,296 bytes, which fits, and the host has
127,165,370,368 bytes available, more than the 60,993,699,840 to pin. The
cache gets (16,033,710,080 - 4,580,323,456) / 13,236,480 = 865 experts, 18.8%
of the 4,608. In the worst case a step copies 144 experts, 1,906,053,120
bytes, in 79,999 µs, then reads 5,314,482,432 bytes in 25,119 µs:
105,118 µs, or 9.513 tokens/s.

The Mac admits 120b at 24.640 tokens/s, the 4080 at 9.513. The planner picks
the Mac.

## The planner

The read rate is the bytes of a measured run divided by its time:

<!-- source: psionic/lib/gptoss/device-choice.coax rate -->
```jai
// An effective read rate from a measured decode run: steps decode steps
// starting at position first took ns nanoseconds; chapter 33 counts the
// bytes each step reads. Bytes a microsecond, rounded down; 0 for a run
// outside the model's bounds.
fc_rate :: (m:RpModel,first:int,steps:int,ns:int)->int {
    if steps<1 || ns<1 || rp_decode(m,first).status!=RpStatus.ok || rp_decode(m,first+steps-1).status!=RpStatus.ok {return 0;}
    bytes:=0;
    for n:first..<first+steps {bytes+=rp_decode(m,n).bytes;}
    return bytes*1000/ns;
}
```

A plan checks the model, the request and the device before any arithmetic.
It then tries resident and falls through to experts streamed or a refusal:

<!-- source: psionic/lib/gptoss/device-choice.coax plan -->
```jai
// The streamed mode keeps at least two layers' selections in the cache:
// one layer's experts in use while the next layer's are copied.
fc_plan :: (m:RpModel,r:RpRequest,d:FcDevice)->FcPlan {
    if !rp_valid(m) || m.arch!=RpArch.moe {return FcPlan.{status=FcStatus.model};}
    if r.capacity<1 || r.capacity>m.max_tokens || r.chunk<1 || r.chunk>r.capacity {return FcPlan.{status=FcStatus.request};}
    if d.memory<0 || d.buffer<0 || d.read_rate<1 || d.link_rate<0 || d.host<0 {return FcPlan.{status=FcStatus.device};}
    w:=rp_weights(m);expert:=w.expert;
    p:=FcPlan.{experts=rp_layers(m)*m.experts*expert,state=rp_state(m,r.capacity),scratch=rp_scratch(m,r.chunk,r.capacity)};
    p.dense=w.total-p.experts;p.read=rp_decode(m,r.capacity).bytes;
    live:=p.state+p.scratch;
    if w.total+live<=d.memory && w.total<=d.buffer {
        p.mode=FcMode.resident;p.device=w.total+live;
    } else {
        if live>d.memory {p.status=FcStatus.state;return p;}
        if d.link_rate==0 {p.status=FcStatus.memory;return p;}
        if p.dense+live+2*m.selected*expert>d.memory {p.status=FcStatus.dense;return p;}
        if p.experts>d.host {p.status=FcStatus.host;return p;}
        p.mode=FcMode.experts;p.slots=(d.memory-p.dense-live)/expert;
        p.device=p.dense+live+p.slots*expert;p.pinned=p.experts;p.copied=rp_layers(m)*m.selected*expert;
        p.step_us=fc_us(p.copied,d.link_rate);
    }
    p.step_us+=fc_us(p.read,d.read_rate);
    p.milli=1000000000/p.step_us;
    return p;
}
```

The worst case is deliberate. It assumes no cache hits and no overlap
between copies and computation, so an admitted device can only do better.
Picking compares those worst-case rates:

<!-- source: psionic/lib/gptoss/device-choice.coax pick -->
```jai
// Plan every device and return the index of the admitted one with the
// highest worst-case rate (the first on a tie), or -1 when none admits.
fc_pick :: (m:RpModel,r:RpRequest,devices:[]FcDevice,plans:[]FcPlan)->int {
    best:=-1;top:int;
    for i:0..<devices.count {
        p:=fc_plan(m,r,devices[i]);
        if i<plans.count {plans[i]=p;}
        if p.status==FcStatus.ok && (best<0 || p.milli>top) {best=i;top=p.milli;}
    }
    return best;
}
```

## Measure the link

The streamed case depends on one rate this repository had not measured for
experts: original-size expert copies from pinned host memory to the 4080.
[`expert_upload_bench.c`](../tests/expert_upload_bench.c) is a platform
harness on [`cuda-host.c`](../platform/cuda-host.c) with no model
arithmetic. It pins 288 distinct 13,236,480-byte expert images (two tokens'
worth, so consecutive tokens copy different bytes). For each token it copies
144 of them one at a time into 8 device slots and times the copies with
device events. It then checks that the last slot matches its source byte for
byte:

```shell
gcc -std=c11 -O2 -Ipsionic/platform psionic/platform/cuda-host.c psionic/tests/expert_upload_bench.c -ldl -lm -o expert-upload-bench
./expert-upload-bench 11
```

The run on coderos-4080 (2026-11-02, the card otherwise idle, load
average 4.4) is kept in
[`rtx4080-upload.log`](../fixtures/fit-120b/rtx4080-upload.log), and its
times are in [`rtx4080-upload.coax`](../fixtures/fit-120b/rtx4080-upload.coax):
one warm-up token of 80,020 µs, then ten tokens between 79,986 and 80,008 µs.
The example summarizes them under
[chapter 34](ch034-benchmark-against-llama-cpp-and-cublas.md)'s protocol: one
warm-up, at least five timed runs, mean and sample standard deviation by two
passes and checked by Welford's method.

## Run it

[The example](../examples/fit-120b.coax) plans 120b on both devices with these
measured inputs and picks one:

<!-- source: psionic/examples/fit-120b.coax example -->
```jai
// 128-step decode medians from position 1,025, in nanoseconds: chapter
// 72's Mac run and chapter 71's 4080 receipt (five runs each).
MAC_DECODE_NS :: 3683000000;RTX_DECODE_NS :: 2279540283;
MAC_WORKING_SET :: 115427246080;MAC_BUFFER :: 86543591014;   // 107.5 GiB and 80.6 GiB, as chapter 72 printed them
RTX_RESERVE :: 268435456;                                    // chapter 45's 256 MiB
main :: () {
    g:=rp_gptoss_120b();w:=rp_weights(g);r:=RpRequest.{capacity=4096,chunk=256};
    print("120b: % weight bytes, % in % experts of %, % dense\n",w.total,36*128*w.expert,36*128,w.expert,w.total-36*128*w.expert);
    ns:[10]int;total:=0;
    for us,i:FU_TOKEN_US {ns[i]=us*1000;total+=us;}
    st:=cm_two_pass(ns[..]);wf:=cm_welford(ns[..]);
    link:=10*FU_TOKEN_BYTES/total;
    print("upload receipt: % tokens of % bytes, mean % ns, sd % ns (Welford agrees: %), % bytes/us\n",
        st.n,FU_TOKEN_BYTES,cast(int)(st.mean+0.5),cast(int)(st.sd+0.5),cast(int)(wf.mean+0.5)==cast(int)(st.mean+0.5),link);
    mac:=FcDevice.{memory=MAC_WORKING_SET,buffer=MAC_BUFFER,read_rate=fc_rate(rp_gptoss_20b(),1025,128,MAC_DECODE_NS)};
    rtx:=FcDevice.{memory=FU_FREE-RTX_RESERVE,buffer=FU_FREE-RTX_RESERVE,read_rate=fc_rate(rp_gptoss_20b(),1025,128,RTX_DECODE_NS),
        link_rate=link,host=FU_HOST_AVAILABLE};
    print("read rates from the 20b runs: Mac % bytes/us, RTX 4080 % bytes/us\n",mac.read_rate,rtx.read_rate);
    devices:=[2]FcDevice.[rtx,mac];plans:[2]FcPlan;
    best:=fc_pick(g,r,devices[..],plans[..]);
    show("RTX 4080 16 GB",plans[0]);show("Apple M5 Max 128 GB",plans[1]);
    names:=[2]string.["RTX 4080 16 GB","Apple M5 Max 128 GB"];
    print("pick: %\n",names[best]);
    // What the 4080's cache could save, on chapter 191's synthetic router
    // (not gpt-oss routing): 384 tokens, the last 256 counted.
    keys:=36*128;per:=36*4;trace:[384*36*4]int;gate:[36*128*XS_WIDTH]f32;f:[128]f64;work:[865+3*36*128]int;none:[1]int;
    t:=xs_trace(1,2,384,36,128,4,trace[..],gate[..],f[..]);
    c:=xs_replay(XsPolicy.lru,trace[..],per,128*per,plans[0].slots,keys,none[0..<0],work[..]);
    cost:=xs_cost(c.misses,256,w.expert,link,plans[0].read,rtx.read_rate);
    print("synthetic trace %, LRU over % slots: % of % hits, % copies a token, tokens/s ",t.status,plans[0].slots,c.hits,c.accesses,c.misses/256);
    milli(cost.serial_milli);print("\n");
}
```

Expected output, the same in debug and release:

```text
120b: 65248815744 weight bytes, 60993699840 in 4608 experts of 13236480, 4255115904 dense
upload receipt: 10 tokens of 1906053120 bytes, mean 79996000 ns, sd 6377 ns (Welford agrees: true), 23826 bytes/us
read rates from the 20b runs: Mac 130953 bytes/us, RTX 4080 211578 bytes/us
RTX 4080 16 GB: experts streamed (ok)
  device 16029878656 bytes, host pinned 60993699840 bytes, expert cache 865 slots
  worst-case step: read 5314482432 bytes, copy 1906053120 bytes, 105118 us, tokens/s 9.513
Apple M5 Max 128 GB: resident (ok)
  device 65574023296 bytes, host pinned 0 bytes, expert cache 0 slots
  worst-case step: read 5314482432 bytes, copy 0 bytes, 40584 us, tokens/s 24.640
pick: Apple M5 Max 128 GB
synthetic trace ok, LRU over 865 slots: 28988 of 36864 hits, 30 copies a token, tokens/s 23.691
```

Every number matches the hand calculation. The 4080's link moves 23.8 GB/s,
the same rate chapter 45 measured for streamed Elephant matrices. Copying
every selected expert costs 80 ms a token, three times the 25 ms the card
spends reading.

The last line is not a gpt-oss prediction. It replays
[chapter 191](ch191-experts-across-machines-and-from-disk.md)'s synthetic
router, whose four topics make routing much more repetitive than a real
model's. It shows how much the cache could matter: at 79% hits the 4080 would
come close to the Mac. Whether real 120b routing gets near that needs a
routing trace recorded from the real model. Until then the planner ranks
devices by the worst case.

## What the planner refuses

[The refusal example](../examples/fit-120b-refuse.coax) changes one input at a
time:

<!-- source: psionic/examples/fit-120b-refuse.coax refuse -->
```jai
main :: () {
    g:=rp_gptoss_120b();r:=RpRequest.{capacity=4096,chunk=256};
    rtx:=FcDevice.{memory=16033710080,buffer=16033710080,read_rate=211578,link_rate=23826,host=127165370368};
    mac:=FcDevice.{memory=115427246080,buffer=86543591014,read_rate=130953};
    small:=mac;small.memory=48*1073741824;small.buffer=small.memory*3/4;
    say("a 64 GB Mac with a 48 GiB working set",fc_plan(g,r,small));
    big:=mac;big.buffer=60*1073741824;
    say("the 128 GB Mac if its largest buffer were 60 GiB",fc_plan(g,r,big));
    host:=rtx;host.host=32*1073741824;
    say("the RTX 4080 on a host with 32 GiB to pin",fc_plan(g,r,host));
    card:=rtx;card.memory=4*1073741824;card.buffer=card.memory;
    say("a 4 GiB card",fc_plan(g,r,card));
    card.memory=256*1048576;
    say("256 MiB of card for a 4,096-token cache",fc_plan(g,r,card));
    say("capacity 131,073 for a 131,072-token model",fc_plan(g,RpRequest.{capacity=131073,chunk=1},mac));
    no_rate:=mac;no_rate.read_rate=0;
    say("a device with no measured read rate",fc_plan(g,r,no_rate));
    say("Elephant, which has no experts",fc_plan(rp_elephant(),r,mac));
    plans:[2]FcPlan;none:=[2]FcDevice.[small,card];
    print("pick among the 64 GB Mac and the 256 MiB card: %\n",fc_pick(g,r,none[..],plans[..]));
}
```

The example exits with status 0 after printing each refusal:

```text
a 64 GB Mac with a 48 GiB working set: refused (memory: the weights exceed unified memory and nothing can stream)
the 128 GB Mac if its largest buffer were 60 GiB: refused (memory: the weights exceed unified memory and nothing can stream)
the RTX 4080 on a host with 32 GiB to pin: refused (host: the host cannot pin every expert)
a 4 GiB card: refused (dense: the dense weights, state, scratch and two layers of experts exceed the device)
256 MiB of card for a 4,096-token cache: refused (state: the state and scratch alone exceed the device)
capacity 131,073 for a 131,072-token model: refused (request: capacity or chunk outside 1..max tokens)
a device with no measured read rate: refused (device: a negative size or a rate below 1)
Elephant, which has no experts: refused (model: not a valid mixture-of-experts shape)
pick among the 64 GB Mac and the 256 MiB card: -1
```

The 4 GiB card is refused because the dense 4,255,115,904 bytes nearly fill
its 4,294,967,296; with the state, scratch and 8 experts the smallest
streamed plan needs 4,686,215,296.

## Checks

The checks in `device-choice.coax` compare the planner with the hand values
above, worked independently from the pinned 120b inventory and the two
measured runs. They also cover each boundary one byte either side: all
of 120b plus live bytes is exactly resident, one byte less is refused or
streamed; the dense part plus 8 experts is exactly the smallest streamed
plan. Further checks cover the host one byte short, every malformed device
field, and ties in the pick. `coax mutate` leaves zero survivors.

```shell
coax/bin/coax test psionic/lib/gptoss/device-choice.coax
coax/bin/coax mutate psionic/lib/gptoss/device-choice.coax --root-only
```

## What remains

- **Weights on this host.** The seven original 120b shards (65.2 GB) are not
  here, and the disk has about 35 GB free. Chapter 73's run was on the Mac.
- **The 4080 engine path.** Chapter 66's CUDA engine plans every expert
  resident. Streaming needs an expert cache on the card, per-layer copies
  after routing (routing decides which experts a layer needs, so a layer
  cannot start before its copies finish unless they are prefetched), and
  chapter 70's replay rebuilt around them. Then the greedy token IDs must
  equal chapter 73's CPU engine on the fixture prompts. Its recorded layer,
  route and logit references are in
  [`qualification/`](../fixtures/gptoss/original/120b/qualification/). Decode
  and prefill must be measured under chapter 34's protocol.
- **The Mac run.** The planner's choice. Chapter 72's device plan takes the
  shape as a parameter, but its harness has 20b buffer sizes (24 layers of
  routes and traces). No documented access to the Mac exists from this
  host, so the run is an owner action in `NEEDS_OWNER.md`. It must show the
  admitted allocation succeeding with measured headroom, logits and routes
  within chapter 64's frozen tolerance, equal greedy IDs, and decode and
  prefill rates.
- **Real routing.** The cache estimate uses a synthetic router. A trace
  recorded from 120b would replace it.

## Sources

- [Chapter plan](PLAN.md), GPT-OSS device delivery order, step 9.
- [gpt-oss target](../docs/targets/gpt-oss.md): 120b shape and pinned files.
- [Chapter 72's Metal record](../../docs/verification/psionic-gptoss-metal-2026-10-29.md):
  the Mac's working set, largest buffer and decode medians.
