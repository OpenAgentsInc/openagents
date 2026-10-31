# 14. Execute Elephant

We can now follow one text decision from the original Elephant files to a
choice. The model reads the shared state and the question as two blocks: full
attention is bidirectional inside each block and sees earlier blocks; the
linear layers remain recurrent. The last token's vocabulary logits become
option probabilities as in chapter 13.

Start with the small [target example](../examples/elephant-target.coax). It
runs without downloading model weights:

```shell
coax/bin/coax run psionic/examples/elephant-target.coax
```

```jai
model: baselabs/elephant-1
layers: 24 linear, 8 full attention
vocabulary: 248320, tensors: 760
```

<!-- source: psionic/examples/elephant-target.coax example -->
```jai
main :: () {
    target:=ps_target(0,128);
    #assert target.status==PsStatus.ok;
    print("model: %\n",target.origin);
    print("layers: % linear, % full attention\n",target.shape.state.linear_layers,target.shape.state.full_layers);
    print("vocabulary: %, tensors: %\n",target.vocab,target.tensors);
}
```

The target is more than a layer count. It pins the upstream revision, hashes,
shapes, tokenizer and readout configuration. The original checkpoint also
contains vision tensors. We validate the complete inventory, including its
rank-five tensors, even though this chapter executes the text path.

## Bind original weights

The caller supplies the pinned `config.json`, `tokenizer.json`, `readout.json`
and `model.safetensors`, plus the checked packed tokenizer table and the
[trusted tensor manifest](../fixtures/models/elephant/manifest.json). The
session checks artifact hashes and the complete manifest before binding layer
names and shapes. Matrices borrow the original bytes; small normalization and
convolution vectors are decoded into caller-owned storage. Neither borrowed
files nor the resulting session may be changed or freed while in use. A failed
open does not publish a usable session, although scratch can change.

The target fixes BF16 weights, width 4,096 and a vocabulary of 248,320. A
capacity selected by the caller determines cache and scratch requirements; a
caller must provide truthful, disjoint allocations and maintain their lifetime.
The C adapters check observed overlaps, not allocation validity, alignment or
exclusive ownership. This is a CPU reference path with substantial memory and
runtime costs, not an optimized serving engine.

## Prepare and execute a typed decision

`ps_prepare` renders a typed state, question and options, tokenizes the
complete prompt and finds a safe shared-prefix boundary. `ps_answer` uses the
pinned slot IDs, starts a fresh recurrent state, generates positions, executes
the prefix and suffix, and maps the winning slot back to its option name. It
projects every vocabulary row in bounded chunks so that the slot mass has the
full-vocabulary denominator. Probabilities and live state publish only when
the calculation succeeds; workspace may change on failure.

The final assembly stays in Coax:

<!-- source: psionic/lib/decision-answer.coax answer -->
```jai
ps_answer :: (session:PsSession,prepared:PrResult,tokens:[]int,options:[]PtOption,temperature:f32,state:[]f32,out:[]f64,prob:[]f64,work:[]f32,overlap:bool)->PsAnswer #export_c {
    tag:=cast(u8)prepared.kind;
    if session.status!=PsOpenStatus.ok || prepared.status!=PrStatus.ok || !pt_question_count(tag,options.count) || !qr_valid(tag,prepared.slots,temperature) {return PsAnswer.{status=QxStatus.shape};}
    if tokens.count!=prepared.written {return PsAnswer.{status=QxStatus.capacity};}
    if prepared.kind!=PtQuestionKind.noul && prepared.slots!=options.count {return PsAnswer.{status=QxStatus.shape};}
    if overlap {return PsAnswer.{status=QxStatus.alias};}
    slots:[255]int;for i:0..<prepared.slots {slots[i]=qr_pinned_slot(i);}
    result:=qs_decide(session.model,tokens,prepared.prefix,state,cast(QrKind)tag,temperature,slots[0..<prepared.slots],out,prob,work,false);
    if result.status!=QxStatus.ok {return PsAnswer.{status=result.status,cause=result.cause};}
    answer:=PsAnswer.{statistics=result.answer};
    if prepared.kind==PtQuestionKind.choice {answer.choice=options[result.answer.best].name;}
    return answer;
}
```

The caller keeps the prepared tokens, option names and authenticated session
together. A fabricated session or a mismatched tokenizer, prefix, or option
list is not an authenticated request. This chapter takes an explicit positive
temperature and does not provide calibration, request serialization, batching,
or a network service.

## Try the original files

Obtain the pinned `baselabs/elephant-1` revision described in the
[original bundle record](../docs/original-bundles.md), then place
its four named files together in a local directory. No weights are included in
this repository. The following command compiles an owned C host harness and
runs the Coax session, preparation and answer path. It does not download files.
The full SHA-256 check reads the 18.8 GB checkpoint and the reference CPU
calculation is slow; allow substantial memory and time.

```shell
coax/build.sh
psionic/tests/run_elephant.sh /path/to/elephant-directory
```

The sample asks whether water is wet, with `yes` and `no` options. Its Coax
preparation yields 86 tokens: 49 in the shared prefix and 37 in the suffix.
On the tested CPU profile it chose `yes` with probability about 0.757196;
the independently calculated result agrees within the recorded tolerance.
See the [verification record](../../docs/verification/psionic-ch10-2026-10-29.md)
for measured results and their numerical limits.

## How fast is it?

Slow. This chapter's path is a correctness reference, and it is timed as one.
On a Linux machine with an Intel i7-14700K and 125 GB of memory, the sample
request took:

| Phase | Time |
| --- | --- |
| Prepare the request (render, normalize, tokenize 86 tokens) | 0.63 ms |
| Open the session (hash 18.8 GB, inventory every tensor) | 83.5 s |
| Execute the decision | 2,092 s, about 35 minutes |

That is about 0.04 prompt tokens per second. The unchanged harness above,
including its compilation, took 29 minutes 17 seconds on the same machine
under lighter load. Peak resident memory was about 17.8 GiB, almost
all of it the mapped checkpoint. Other processes shared the processor; the
[verification record](../../docs/verification/psionic-ch10-2026-10-29.md)
lists the load and repeat to reproduce it:

```shell
psionic/tests/bench_elephant.sh /path/to/elephant-directory
```

The time goes into arithmetic, not memory. The decision needs about 730
billion multiply-adds: every weight of the text stack for each of the 86
tokens, plus one full-vocabulary readout. This chapter's profile widens each
BF16 weight and accumulates every product in compensated binary64, in one
thread. That ran at about 0.52 billion multiply-adds per second on one core of
this machine, which accounts for the measured time. One core reads memory at
about 13.8 GB/s, so all the weights could be read in under two seconds; the
compensated sum's fixed order also keeps the compiler from using vector
instructions.

Three things matter beyond the arithmetic:

- **Opening the session is per process, not per request.** Hashing the
  checkpoint proves its identity once; chapter 19 keeps an authenticated
  session resident so later decisions skip those 83.5 seconds.
- **The shared prefix need not be recomputed.** Here 49 of the 86 tokens are
  the shared state. A request with several questions computes them once and
  forks per question, as chapter 12 describes.
- **A response time below one second needs a GPU.** Chapters 37 and 44 give
  Coax a compute subset and a CUDA backend. An RTX 4080 has about 16 GB of
  memory, slightly less than the BF16 weights, so the decision runs layer by
  layer over the whole prompt, keeps most layers resident and streams the
  rest over PCIe. These are plans with stated gates, not measured results.

Every faster profile must reproduce this chapter's answer within a stated,
frozen tolerance (chapter 17). This slow path remains the reference.

## What the checks establish

Small independent fixtures exercise nonzero recurrent state, rotary positions,
separate attention layers, prefix and suffix execution, and full-vocabulary
readout across chunk boundaries. Native checks exercise the adapters and
refusal paths in debug, release and sanitizer builds. The original files are
also authenticated and compared with an independent CPU implementation at an
intermediate layer and at model output. These comparisons apply to the recorded
model revision, text input and arithmetic profile; they do not establish
accuracy on arbitrary requests or multimodal support.

The [next chapter](ch015-execute-mammoth.md) runs the same decision on
Mammoth with its own dimensions, shards and memory plan.
