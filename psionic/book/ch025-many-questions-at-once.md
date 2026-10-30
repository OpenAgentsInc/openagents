# 25. Many questions at once

A Lab request asks several typed questions about one message. [Chapter
8](ch008-decisions-have-a-different-mask.md) forks a decision: the message's
prefix block runs once, and each question's suffix starts from a copy of its
state. [Chapter 18](ch018-make-the-numbers-move.md) does that on the GPU, one
suffix pass per question. When the weights are resident, a suffix pass is
cheaper than a whole decision. When they stream, as Mammoth's do on a 16 GB
card ([chapter 30b](ch030b-mammoth-on-cuda.md)), every pass copies the same
34 GiB over PCIe. Five forked questions are then six passes, slower than five
whole decisions.

This chapter runs all of a request's suffixes as one pass over the weights. A
five-question Lab request on Mammoth takes 3.3 s instead of 9.3 s, and 1.8 s
when the next request repeats the same message. Each question's logits are
bit-identical to its own suffix pass.

## One pass, each question's own arithmetic

The questions' suffix tokens are placed one after another in the workspace.
Launches that treat each token separately take all of them at once. Those are
the norms, every projection, the gates, the feed-forward and the residual
adds, and they are where the weights are read. Three things carry state along
a sequence: the convolution, the delta rule and attention. They run once per
question, on that question's rows only. Each starts from the kept prefix
state, so no question sees another's tokens. Each question then projects its
own values into the fork buffer's rows after the prefix and attends to the
prefix keys and its own, just as its own suffix pass does. The next question
overwrites those rows. The value matrix's slot, if it streams, is held from
the first question's projection to the last. The readout reads each head
chunk once per question before releasing its slot.

Bit-identity also needs every projection to take the same kernel path as in
the question's own pass. From 16 tokens up, projections run on the tensor
cores, which split activations into BF16 halves; below 16 they use the
work-item kernels. These give different roundings. A batch is admitted only
when one path serves every question:

<!-- source: psionic/lib/device/plan.coax batch -->
```jai
// Many questions at once (chapter 25): the suffixes of up to DP_BATCH
// questions that share one kept prefix run as one pass over the weights.
// Their tokens sit one question after another in the workspace. Every
// token-wise launch (norms, projections, gates, feed-forward) takes all of
// them at once; the launches that carry state along a sequence (the
// convolution, the delta rule, attention) and the readout run once per
// question on that question's rows, starting from the same kept prefix.
// Each question's arithmetic is then its own suffix pass's, provided every
// projection takes the same path: all questions have at least DP_MMA_TOKENS
// tokens (tensor cores), or all of them together have fewer (work items).
// Other batches, and Metal's tiles, are refused; the host runs those
// questions one suffix at a time.
DP_BATCH :: 8;
dp_batch_tokens :: (lengths:[]int)->int #export_c {n:=0;for t:lengths {n+=t;}return n;}
dp_batch_longest :: (lengths:[]int)->int {n:=0;for t:lengths {n=dp_wider(n,t);}return n;}
dp_batch_admits :: (metal:bool,start:int,lengths:[]int)->bool #export_c {
    if metal || start<1 || lengths.count<1 || lengths.count>DP_BATCH {return false;}
    total:=0;short:=false;
    for t:lengths {
        if t<1 || start+t>DP_MAX_TOKENS {return false;}
        if t<DP_MMA_TOKENS {short=true;}
        total+=t;
    }
    if total>DP_MAX_TOKENS {return false;}
    return !short || total<DP_MMA_TOKENS;
}
```

The host runs a batch the plan refuses one suffix at a time, so a refusal
never changes an answer. Metal's matrix-unit tiles are refused until they are
qualified the same way. For one question the batched plan is the suffix plan,
operation for operation: the checks compare their digests for Elephant
(resident and fully streamed) and for Mammoth. Plans for unbatched requests
are untouched, and every Elephant and Mammoth plan digest from chapters 30
to 31 still holds.

## The proof, before the device

For more than one question, a check pulls each question's stateful launches
out of the batched plan and compares them with its own suffix plan. It
compares every kernel, work-item count and argument, including the
fork-buffer rows, and leaves out only the workspace offsets:

<!-- source: psionic/lib/device/plan.coax perquestion -->
```jai
// The launches that carry a question's state (convolution, delta rule,
// attention), with the workspace offsets left out: a batch's question i
// must make exactly the launches its own suffix pass makes, in order, on the
// same fork-buffer rows. Occurrence j of a stateful kernel in a batch of q
// questions is question j%q's (they run question after question).
dp_stateful :: (op:DpOp)->bool {
    k:=op.kernel;
    return op.kind==DpOpKind.launch && (k==cast(u8)DpKernel.conv_state || k==cast(u8)DpKernel.scan_state || k==cast(u8)DpKernel.prepare || k==cast(u8)DpKernel.scores || k==cast(u8)DpKernel.softmax || k==cast(u8)DpKernel.attend);
}
dp_state_digest :: (ops:[]DpOp,n:int,questions:int,which:int)->u64 {
    h:u64=14695981039346656037;seen:[DP_KERNELS]int;
    for i:0..<n {
        op:=ops[i];
        if dp_stateful(op) {
            j:=seen[op.kernel];seen[op.kernel]=j+1;
            if j%questions==which {
                fields:=[4]int.[cast(int)op.kernel,op.items,op.lanes,op.count];
                for f:fields {h=mul_wrap(h^cast,trunc(u64)f,1099511628211);}
                for k:0..<op.count {
                    a:=op.args[k];offset:=a.offset;if a.buffer==0 {offset=-1;}
                    more:=[5]int.[cast(int)a.kind,a.value,a.buffer,offset,a.count];
                    for f:more {h=mul_wrap(h^cast,trunc(u64)f,1099511628211);}
                }
            }
        }
    }
    return h;
}
```

Other checks pin each question's workspace rows (question 4 of the sample
batch starts at token 148), its final-norm row and logits, the refusals, and
the exact workspace a batch needs. With chapter 30a's checker, they show that
a fully streamed batch launches what the resident batch launches up to the
readout, with every copy used once and one value slot held across each full
layer's questions. `coax test psionic/lib/device/plan.coax` passes 82 checks
in debug and release builds. Mutation testing of the plan, run as two shards
of `--jobs 8`, leaves no survivors (see the
[record](../../docs/verification/psionic-ch25-2026-10-30.md)).

The [example](../examples/many-questions.coax) plans the Lab request without
a GPU, for Elephant (model 0) and Mammoth (model 1), each placed for 256
tokens in 13 GiB:

```shell
coax/bin/coax run psionic/examples/many-questions.coax
```

```jai
admitted: lab true, on Metal false, 37 and 8 tokens false, 7 and 8 true, 8 and 8 false
batch tokens: 188; first rows: 0 37 75 113 151
model 0: batch workspace 187457536 bytes; logits at 166533120 183484416 184477696 185470976 186464256
model 0: five suffix passes 5820 operations, 12079595520 bytes copied; one batched pass 1596 operations, 2415919104 bytes copied
model 1: batch workspace 252292864 bytes; logits at 231352064 248319744 249313024 250306304 251299584
model 1: five suffix passes 14535 operations, 190157619200 bytes copied; one batched pass 3867 operations, 38031523840 bytes copied
```

One batched pass copies a fifth of what five suffix passes copy. An
independent oracle derives the admission rule, the question rows, the
workspace and the logits offsets from the written rule and the two models'
shapes, and checks the transcript:

```shell
python3 -I psionic/tests/batch_oracle.py psionic/fixtures/many-questions.out
```

## On the RTX 4080

`eg_decide_batch` in `psionic/platform/elephant-gpu.c` checks that every
question shares the prefix tokens. It runs the prefix pass unless the fork
buffer already holds those tokens, stages every question's embedding rows and
rotary coefficients at its own rows, runs the batched plan and reads back
each question's logits. The chapter 30 and 30b harnesses
(`psionic/tests/elephant_device.c`, `mammoth_device.c`) now run the Lab
request four ways through `psionic/tests/lab_request.h`. The five questions
about `Water is wet.` have 86 or 87 tokens and a 49-token prefix. Each
harness was placed for 256 tokens, so a batch's 188 suffix tokens fit.

| Lab request, 5 questions | Elephant | Mammoth |
| --- | --- | --- |
| Placement | 14.13 GiB resident, 0.66 GiB streamed (7 matrices) | 13.49 GiB resident, 34.24 GiB streamed (207 copies) |
| One whole decision | 145-147 ms | 1556-1560 ms |
| Whole decisions | 724.1 ms | 7777.7 ms |
| Forked, one suffix at a time | 722.8 ms | 9304.7 ms |
| Batched: prefix pass + one suffix pass | 513.9 ms | 3285.0 ms |
| Batched, prefix kept from the last request | 392.3 ms | 1832.5 ms |

The numbers are best of three rounds for Elephant and best of two for Mammoth.
For all five questions on both models, the batched logits match the
sequential forked logits bit for bit: 0 of 248,320 differ. They also match a
second batched request that reused the kept prefix. The answers are chapter
30b's. The whole decision passes the frozen 0.0005 rule against the CPU
references (`elephant_reference_fixture.py`,
`mammoth_reference_fixture.py`), and since the batched first question equals
it bit for bit, so does the batch.

On Mammoth the request is now two passes over the streamed weights instead
of six, 2.8 times faster than forking one question at a time and 2.4 times
faster than whole decisions. One suffix pass costs 1.56 s at the link's rate.
The batched suffix pass for all five questions costs 1.83 s (the
kept-prefix row), and the prefix pass is the other 1.45 s. Elephant gains less: a resident suffix pass is cheaper, and part of its
time is the five questions' attention and readout, which a batch still runs
once per question.

## Prefix reuse across requests

The engine keeps one prefix: the fork buffer and the exact tokens it was
computed from. A request whose questions share those exact tokens skips the
prefix pass, as the last table row shows. Any other prefix replaces it. The
key is the engine's placed model and the exact token prefix, never a string
prefix. So the cache holds one entry, and every miss evicts it. A larger
cache would keep several fork buffers (82 MiB each for Elephant, 214 MiB for
Mammoth) with an eviction rule; it is not built.

## What remains

- The `/v1/systemone` server still asks its questions one at a time.
  Batching there means staging all of a request's questions before the first
  decision, so the server's measured Lab latency has not changed yet.
- The Metal plan refuses batches.
- A multi-entry prefix cache is not built.
- Batches that mix short and long suffixes run one question at a time.
