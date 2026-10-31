# 46. Mammoth on CUDA

[Chapter 15](ch015-execute-mammoth.md) ran one Mammoth decision on the CPU in
about two and a half hours. [Chapter 44](ch044-elephant-on-cuda.md) ran
Elephant on an RTX 4080, and [chapter 45](ch045-share-the-card.md) let
every Elephant matrix stream from host memory. This chapter runs Mammoth's
decision through the same engine on the same card. Mammoth's text weights are
51.3 GB, more than three times the card, so most of every decision is copied
over PCIe while it runs. It answers in about 1.6 seconds, with chapter 15's
probabilities to six decimal places.

## The shape is data

Chapter 44's plan had Elephant's dimensions written into it: 32 layers, width
4,096, feed-forward width 12,288, 16 query heads, 21 head chunks. Mammoth has
the same layer pattern, head sizes, tokenizer and readout. Only the counts
differ, so the plan now takes them as a value:

<!-- source: psionic/lib/device/plan.coax model -->
```jai
// The decision's shape is data. Elephant (kind 0) and Mammoth (kind 1) share
// the layer pattern (every fourth layer is full attention), the head sizes
// (128 in the linear layers, 256 in the full ones), 16 key heads in the
// linear layers, 4 key/value heads in the full ones, and the vocabulary.
// They differ in depth, width, feed-forward width and head counts. Any other
// kind names Elephant here; the plans refuse it.
// chunks: how many pieces of up to `hidden` rows the head streams in.
DpModel :: struct {kind:int;layers:int;width:int;hidden:int;vocab:int;key_heads:int;value_heads:int;heads:int;kv_heads:int;chunks:int;}
dp_model :: (kind:int)->DpModel #export_c {
    if kind==1 {return DpModel.{kind=1,layers=64,width=5120,hidden=17408,vocab=248320,key_heads=16,value_heads=48,heads=24,kv_heads=4,chunks=15};}
    return DpModel.{layers=32,width=4096,hidden=12288,vocab=248320,key_heads=16,value_heads=32,heads=16,kv_heads=4,chunks=21};
}
```

Everything the plan derives comes from these fields: each weight's size, the
workspace regions, the streaming indices, the copy slots (one feed-forward
matrix each: 96 MiB for Elephant, 170 MiB for Mammoth), the head chunks, the
fork buffer and every kernel argument. The kernels did not change; they
already took their head counts as arguments.

Changing the plan must not change Elephant. Every Elephant check from
chapters 44, 45 and 47 still passes, including the digests of the resident,
streamed, fully streamed, short, Metal and forked plans, and the examples of
those chapters print the same bytes. New checks pin Mammoth's 1,026 weights
and their 51,254,491,136 bytes, its 527 streaming indices, its workspace and
fork buffer, and show, with chapter 45's checker, that a streamed Mammoth plan
launches what the resident plan launches, up to a readout that reads the head
in 15 chunks of up to 17,408 rows. Mutation testing of the plan leaves no
survivors among 2,856 mutants.

The [example](../examples/mammoth-device-plan.coax) prints Mammoth's plan at
the 4080's 14 GiB and at its smallest budget, without a GPU:

```shell
coax/bin/coax run psionic/examples/mammoth-device-plan.coax
```

```jai
model: 64 layers, width 5120, feed-forward 17408, 1026 weights, 527 streamable
14 GiB: 2829 operations; resident 14486179840 bytes, copied per decision 36768317440 bytes in 207 matrices, slots of 178257920 bytes
smallest budget 487526656 bytes: 3741 operations, copied per decision 51243909120 bytes; one byte less: memory
```

<!-- source: psionic/examples/mammoth-device-plan.coax example -->
```jai
main :: () {
    m:=dp_model(1);ops:[3800]DpOp;stream:[527]bool;
    print("model: % layers, width %, feed-forward %, % weights, % streamable\n",m.layers,m.width,m.hidden,dp_weights(m),dp_streamable(m));
    plan:=dp_plan_model(1,false,86,49,14*1073741824,stream[..],ops[..]);
    #assert plan.status==DpStatus.ok;
    copied:=0;for i:0..<plan.ops {if ops[i].kind==DpOpKind.copy {copied+=ops[i].args[0].count;}}
    print("14 GiB: % operations; resident % bytes, copied per decision % bytes in % matrices, slots of % bytes\n",plan.ops,plan.resident,copied,plan.streamed_tensors,plan.slot);
    // Everything streams: only the vectors, the workspace and two slots stay.
    vectors:=0;for w:0..<dp_weights(m) {if !dp_bf16(m,w) {vectors+=dp_align(dp_bytes(m,w));}}
    least:=plan.workspace+vectors+2*plan.slot;
    all:=dp_plan_model(1,false,86,49,least,stream[..],ops[..]);
    copied=0;for i:0..<all.ops {if ops[i].kind==DpOpKind.copy {copied+=ops[i].args[0].count;}}
    print("smallest budget % bytes: % operations, copied per decision % bytes; one byte less: %\n",least,all.ops,copied,dp_plan_model(1,false,86,49,least-1,stream[..],ops[..]).status);
}
```

## Run it

The host engine is chapter 44's. `eg_place_kind` names the model and refuses
a session whose shape is not that model's; it checks each weight's byte count
against the session's view before uploading or pinning it. The harness opens
Mammoth's session exactly as chapter 15 does, hashing both shards, then
places, decides and reports:

```shell
psionic/tests/run_mammoth_device.sh /path/to/mammoth-directory
```

The card is shared with gpt-oss-20b, so `PSIONIC_DEVICE_GATE=PATH` makes the
harness authenticate first and wait for PATH before it opens the device; the
card was freed only from then on, and gpt-oss restored after each run.

Before the first device run we froze the acceptance test, as chapter 44 did:
the choice must be chapter 15's, `yes`, and each of the four statistics must
be within 0.0005 of chapter 15's recorded CPU result
(`mammoth_reference_fixture.py --check-cpu`). The harness also checks them
against chapter 15's independent layer-streamed reference. With the card to
itself the plan kept 12.5 GiB resident and streamed 35.2 GiB per decision:

```jai
plan: workspace 167.5 MiB, resident 12.53 GiB, streamed 35.20 GiB in 228 copies through 2 slots of 170.0 MiB (budget 13.09 GiB)
session: authenticated in 220.6 s; placed in 33.2 s: uploaded 12.53 GiB resident in 33.22 s, pinned 35.20 GiB for streaming; peak rss 79.8 GiB
cold open: 255.2 s from process start to a placed engine (not counting 15.9 s waiting for the card)
run 1: time to response 1625.1 ms (2892 operations), p_yes 0.944615702
run 2: time to response 1601.4 ms (2892 operations), p_yes 0.944615702
run 3: time to response 1601.9 ms (2892 operations), p_yes 0.944615702
choice=yes p_yes=0.944615702 p_no=0.0553842984 slot_mass=0.472762848 confidence=0.889231403
```

Both checks pass. The CPU said p(yes) 0.94461191; the device says
0.944615702, a difference of 3.8e-6. The largest difference is slot mass,
2.2e-5, well inside the tolerance.

With almost none of the card, a 600 MiB budget, everything but the vectors
streams: 51.2 GB per decision. The answer is the same to every printed digit:

```shell
PSIONIC_DEVICE_BUDGET_MIB=600 psionic/tests/run_mammoth_device.sh /path/to/mammoth-directory
```

```jai
plan: workspace 167.5 MiB, resident 0.09 GiB, streamed 47.65 GiB in 508 copies through 2 slots of 170.0 MiB (budget 0.59 GiB)
run 1: time to response 2631.1 ms (3732 operations), p_yes 0.944615702
run 2: time to response 2706.8 ms (3732 operations), p_yes 0.944615702
run 3: time to response 2836.9 ms (3732 operations), p_yes 0.944615702
device memory in use by this process: about 0.88 GiB; 14.06 GiB left free
```

## Where the time goes

The bound is the link. Measured with the driver API alone, pushing all of
Mammoth's 54.7 GB of shard bytes from pinned memory through two 96 MiB slots
takes 2.31 s, a steady 23.7 GB/s. A decision streams the text matrices only:
37.8 GB when 12.5 GiB stay resident, 1.60 s at that rate, which is what the
decision takes. Streaming everything, 51.2 GB is 2.16 s at that rate; the
decision took 2.63 to 2.84 s, about 80% of the link. The kernels (Elephant's
take 146 ms; Mammoth has about three times the arithmetic) hide under the
copies in both cases. Against the 2.3 s floor for every shard byte, the
decision with most of the card is 30% faster, because 12.5 GiB never cross
the link, and the one with none of it is 15 to 25% slower.

A Lab request asks several questions about one message. Chapter 44's fork
runs the shared prefix once and each question's suffix from the kept state;
for Elephant that took the Lab's five questions from 1.84 to 1.17 s. For a
streamed Mammoth it does the opposite. Each pass, prefix or suffix, streams
every streamed matrix again, so five forked questions are six passes:

```jai
lab request (whole decisions), round 1: five questions in 8009.5 ms
lab request (forked, one prefix pass), round 1: five questions in 9494.2 ms
```

With everything streamed the two took 14.1 to 16.7 s and 14.2 to 15.4 s:
the same within the run-to-run noise.

The answers are identical either way (one question is chapter 15's, two more
say `yes`, two say `no`; the record lists them). When the decision is bound by
copies, what helps is fewer passes over the weights, not fewer tokens per
pass: running the five questions' suffixes together, one pass after the
prefix, would cost about two passes instead of six.

## On a 96 GB card

On a Google Cloud RTX PRO 6000 (Blackwell, compute 12.0, 94.97 GiB usable)
nothing has to stream. The same code runs unchanged: NVRTC compiles the
kernels for `compute_120`, and chapter 37's check of the tensor-core
primitive passes on sm_120. The whole model stays resident in 48.1 GiB, and
the decision passes the same frozen rule against chapter 15's CPU result
(largest difference 1.4e-5, slot mass):

| | RTX PRO 6000, all resident | Same card, 4080's 13.09 GiB budget | RTX 4080, 13.09 GiB budget |
| --- | --- | --- | --- |
| Streamed per decision | 0 | 37.8 GB | 37.8 GB |
| Single decision | P50 265.2 ms, P95 265.4 ms (100 runs) | 668 ms | 1.60 s |
| Lab request, five questions | 1.29 s | 3.34 s | 8.0 s |
| Device memory | 48.1 GiB | 13.3 GiB | 13.3 GiB |
| Cold open, first / restart | 325 s / 244 s | 250 s | 255 s |

p(yes) is 0.944614404 there against the 4080's 0.944615702: Blackwell's
tensor cores round some partial sums differently. With the 4080's budget the
same plan streams at about 57 GB/s instead of 24, so the link is still the
bound, just a faster one. The cold open is the checkpoint hash on one CPU core;
the upload of 47.7 GiB takes 1.8 s. The server does not place Mammoth yet, so
chapter 170's load generator was not run. Elephant on the same card answers in
90.5 ms. The [RTX PRO 6000 record](../../docs/verification/psionic-rtx6000-mammoth-2026-10-30.md)
has the rest.

## Limits

One request shape, one machine, card free during the runs. Every decision
copies tens of gigabytes again, so Mammoth on a 16 GB card is PCIe-bound:
about 1.6 s with most of the card, about 2.7 s with almost none of it.
Quantized weights (chapter 50) would halve the bytes. The fork as written
hurts a streamed model; a batched suffix pass is future work. Opening hashes
the 54.7 GB checkpoint on the CPU (about 220 s) and pins up to 51 GB of host
memory. Per-kernel qualification against the CPU (chapter 44's
`PSIONIC_DEVICE_QUALIFY`) has not been run for Mammoth; the kernels are the
ones chapter 44 qualified on Elephant's weights. The
[verification record](../../docs/verification/psionic-ch30b-2026-10-29.md)
has the measurements.
